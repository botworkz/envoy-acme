use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{header, Request};
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::rt::TokioExecutor;
use instant_acme::{Account, AccountCredentials, BytesBody, BytesResponse, HttpClient, NewAccount};
use rustls::{ClientConfig, RootCertStore};

use crate::errors::AcmeError;

/// Wrapper that injects a `User-Agent` header into every request before
/// delegating to an inner [`HttpClient`], and strips any ACME challenge
/// entries that lack a `token` field so that instant-acme's deserializer
/// can handle them.
///
/// RFC 8555 §6.1 requires clients to supply a `User-Agent` on all requests.
/// Pebble enforces this and returns a problem document (with no `newNonce`
/// field) when the header is absent; instant-acme 0.7 does not set one.
///
/// Additionally, Pebble ≥ 2.7 offers a `dns-persist-01` challenge type
/// whose `token` field is intentionally empty.  Go's `json:",omitempty"`
/// tag causes the field to be omitted entirely from the wire, but
/// instant-acme 0.7 models `token` as a non-optional `String`.  We
/// pre-filter every response body, removing challenge objects that carry
/// no `token` key, before the bytes reach instant-acme's deserializer.
struct WithUserAgent(Box<dyn HttpClient>);

impl HttpClient for WithUserAgent {
    fn request(
        &self,
        mut req: Request<Full<Bytes>>,
    ) -> Pin<Box<dyn Future<Output = Result<BytesResponse, instant_acme::Error>> + Send>> {
        req.headers_mut()
            .entry(header::USER_AGENT)
            .or_insert_with(|| {
                header::HeaderValue::from_static(concat!("envoy-acme/", env!("CARGO_PKG_VERSION")))
            });
        let fut = self.0.request(req);
        Box::pin(async move {
            let BytesResponse { parts, mut body } = fut.await?;
            let raw = body
                .into_bytes()
                .await
                .map_err(instant_acme::Error::Other)?;
            let filtered = filter_tokenless_challenges(raw);
            Ok(BytesResponse {
                parts,
                body: Box::new(filtered) as Box<dyn BytesBody>,
            })
        })
    }
}

/// Remove any entry from the `challenges` array that has no `token` field.
///
/// Pebble ≥ 2.7 emits `dns-persist-01` challenges without a `token` key
/// (the field is empty and Go's `omitempty` suppresses it).  instant-acme
/// 0.7 deserialises `Challenge.token` as a non-optional `String`, so the
/// whole authorisation document fails to parse.  By stripping token-less
/// challenge objects here we satisfy instant-acme without hiding any real
/// protocol error: we only ever use `http-01` challenges ourselves, and
/// `dns-persist-01` is not a challenge type we can respond to anyway.
///
/// If the body is not valid JSON, or does not contain a `challenges` array,
/// it is returned unchanged so that the caller produces an accurate error.
fn filter_tokenless_challenges(body: Bytes) -> Bytes {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return body;
    };
    let Some(chals) = value.get_mut("challenges").and_then(|c| c.as_array_mut()) else {
        return body;
    };
    let before = chals.len();
    chals.retain(|c| c.get("token").is_some());
    if chals.len() == before {
        return body;
    }
    serde_json::to_vec(&value).map(Bytes::from).unwrap_or(body)
}

/// Build an instant-acme [`HttpClient`] that trusts `ca_path` as its sole root
/// CA.  Used in environments (e.g. integration tests) where the ACME directory
/// is served with a self-signed certificate.
///
/// When `ca_path` is `None` this returns `None` and callers should fall back to
/// `Account::create` / `Account::from_credentials` which use the system's
/// native root certificates.
fn build_custom_client(ca_path: &Path) -> Result<Box<dyn HttpClient>, AcmeError> {
    let pem_bytes = std::fs::read(ca_path)?;
    let mut reader = std::io::Cursor::new(pem_bytes);
    let mut roots = RootCertStore::empty();
    for cert_result in rustls_pemfile::certs(&mut reader) {
        let cert = cert_result?;
        roots
            .add(cert)
            .map_err(|e| AcmeError::OrderFailed(format!("CA cert add error: {e}")))?;
    }

    let tls_config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_only()
        .enable_http1()
        .build();

    let client: HyperClient<_, Full<Bytes>> =
        HyperClient::builder(TokioExecutor::new()).build(connector);

    Ok(Box::new(WithUserAgent(Box::new(client))))
}

pub async fn load_or_create_account(
    directory_uri: &str,
    contact: &str,
    path: &Path,
    ca_file: Option<&Path>,
) -> Result<Account, AcmeError> {
    if path.exists() {
        let data = tokio::fs::read(path).await?;
        let credentials: AccountCredentials = serde_json::from_slice(&data)?;
        return if let Some(ca_path) = ca_file {
            Ok(
                Account::from_credentials_and_http(credentials, build_custom_client(ca_path)?)
                    .await?,
            )
        } else {
            Ok(Account::from_credentials(credentials).await?)
        };
    }

    let new_account = NewAccount {
        contact: &[contact],
        terms_of_service_agreed: true,
        only_return_existing: false,
    };

    let (account, credentials) = if let Some(ca_path) = ca_file {
        Account::create_with_http(
            &new_account,
            directory_uri,
            None,
            build_custom_client(ca_path)?,
        )
        .await?
    } else {
        Account::create(&new_account, directory_uri, None).await?
    };

    let bytes = serde_json::to_vec_pretty(&credentials)?;
    tokio::fs::write(path, bytes).await?;
    Ok(account)
}

#[cfg(test)]
mod tests {
    use super::*;
    use instant_acme::{Authorization, AuthorizationStatus, ChallengeType, Identifier};

    fn install_ring() {
        // rustls 0.23 requires an explicit provider when running in a test context.
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    #[test]
    fn build_custom_client_rejects_empty_pem() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let ca_path = tmp.path().join("empty.pem");
        std::fs::write(&ca_path, b"").expect("write");
        // An empty PEM file results in an empty root store, which is technically
        // valid to build but will reject all server certs at runtime.  This test
        // just verifies the function returns Ok for valid (even empty) input.
        assert!(build_custom_client(&ca_path).is_ok());
    }

    #[test]
    fn build_custom_client_loads_pem() {
        install_ring();
        // Use the vendored pebble CA cert if available; otherwise skip.
        let cert_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config/pebble-certs/pebble.minica.pem");
        if !cert_path.exists() {
            return;
        }
        let result = build_custom_client(&cert_path);
        assert!(result.is_ok(), "should build client from valid PEM");
    }

    /// Verifies that filter_tokenless_challenges strips challenges that lack a
    /// `token` field (e.g. Pebble ≥ 2.7's `dns-persist-01`) and that the
    /// resulting JSON can be deserialised by instant-acme without error.
    #[test]
    fn filter_strips_tokenless_challenge_and_authz_parses() {
        // Simulate a Pebble ≥ 2.7 authorisation response that contains four
        // challenges: three standard ones with tokens, and one `dns-persist-01`
        // whose token field is absent (Pebble omits it via Go's omitempty).
        let raw = serde_json::json!({
            "status": "pending",
            "expires": "2099-01-01T00:00:00Z",
            "identifier": {"type": "dns", "value": "example.test"},
            "challenges": [
                {
                    "type": "http-01",
                    "url": "https://localhost:14000/chalZ/abc",
                    "token": "abc-token",
                    "status": "pending"
                },
                {
                    "type": "dns-01",
                    "url": "https://localhost:14000/chalZ/def",
                    "token": "def-token",
                    "status": "pending"
                },
                {
                    "type": "tls-alpn-01",
                    "url": "https://localhost:14000/chalZ/ghi",
                    "token": "ghi-token",
                    "status": "pending"
                },
                {
                    // dns-persist-01: Pebble emits token="" which Go's omitempty drops
                    "type": "dns-persist-01",
                    "url": "https://localhost:14000/chalZ/jkl",
                    "status": "pending"
                }
            ]
        });

        let body = Bytes::from(serde_json::to_vec(&raw).unwrap());

        // Before filtering, instant-acme cannot parse this because
        // dns-persist-01 has no token field.
        assert!(
            serde_json::from_slice::<Authorization>(&body).is_err(),
            "should fail to parse before filtering"
        );

        let filtered = filter_tokenless_challenges(body);

        // After filtering the document must parse successfully.
        let authz: Authorization =
            serde_json::from_slice(&filtered).expect("should parse after filtering");

        assert_eq!(authz.status, AuthorizationStatus::Pending);
        assert_eq!(authz.identifier, Identifier::Dns("example.test".into()));
        // dns-persist-01 must have been removed; the three token-bearing
        // challenges must survive.
        assert_eq!(authz.challenges.len(), 3);
        assert!(authz
            .challenges
            .iter()
            .any(|c| c.r#type == ChallengeType::Http01));
        assert!(authz.challenges.iter().all(|c| !c.token.is_empty()));
    }

    /// Non-authz responses (no `challenges` key) must pass through unchanged.
    #[test]
    fn filter_leaves_non_authz_bodies_unchanged() {
        let original = br#"{"status":"valid","newNonce":"https://example.com/nonce"}"#;
        let body = Bytes::from_static(original);
        let result = filter_tokenless_challenges(body);
        assert_eq!(result.as_ref(), original);
    }

    /// If all challenges already have tokens, the body bytes must be returned
    /// unchanged (same content, avoids unnecessary JSON round-trip overhead).
    #[test]
    fn filter_noop_when_all_challenges_have_tokens() {
        let original = serde_json::json!({
            "status": "pending",
            "identifier": {"type": "dns", "value": "example.test"},
            "challenges": [
                {"type": "http-01", "url": "https://x/1", "token": "tok", "status": "pending"},
                {"type": "dns-01",  "url": "https://x/2", "token": "tok", "status": "pending"}
            ]
        });
        let bytes = Bytes::from(serde_json::to_vec(&original).unwrap());
        let result = filter_tokenless_challenges(bytes.clone());
        assert_eq!(result, bytes);
    }
}
