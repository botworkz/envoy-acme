//! ACME account credential management: loading existing credentials or registering a new account.
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
    match serde_json::to_vec(&value) {
        Ok(filtered) => Bytes::from(filtered),
        Err(e) => {
            tracing::debug!("filter_tokenless_challenges: re-serialization failed: {e}");
            body
        }
    }
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

/// Load an existing ACME account from `path`, or create and persist a new one if the file is absent.
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
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || crate::atomic_write::write_atomic(&path, &bytes, true))
        .await
        .map_err(std::io::Error::other)??;
    Ok(account)
}

#[cfg(test)]
mod tests {
    use super::*;
    use instant_acme::{Authorization, AuthorizationStatus, ChallengeType, Identifier};
    use std::sync::{Arc, Mutex};

    struct MockHttpClient {
        seen_user_agent: Arc<Mutex<Option<header::HeaderValue>>>,
        body: Bytes,
    }

    impl HttpClient for MockHttpClient {
        fn request(
            &self,
            req: Request<Full<Bytes>>,
        ) -> Pin<Box<dyn Future<Output = Result<BytesResponse, instant_acme::Error>> + Send>>
        {
            *self.seen_user_agent.lock().expect("mutex lock") =
                req.headers().get(header::USER_AGENT).cloned();
            let (parts, _) = hyper::Response::builder()
                .status(200)
                .body(())
                .expect("response")
                .into_parts();
            let body = self.body.clone();
            Box::pin(async move {
                Ok(BytesResponse {
                    parts,
                    body: Box::new(body) as Box<dyn BytesBody>,
                })
            })
        }
    }

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

        assert!(
            serde_json::from_slice::<Authorization>(&body).is_err(),
            "should fail to parse before filtering"
        );

        let filtered = filter_tokenless_challenges(body);

        let authz: Authorization =
            serde_json::from_slice(&filtered).expect("should parse after filtering");

        assert_eq!(authz.status, AuthorizationStatus::Pending);
        assert_eq!(authz.identifier, Identifier::Dns("example.test".into()));
        assert_eq!(authz.challenges.len(), 3);
        assert!(authz
            .challenges
            .iter()
            .any(|c| c.r#type == ChallengeType::Http01));
        assert!(authz.challenges.iter().all(|c| !c.token.is_empty()));
    }

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

    #[test]
    fn filter_leaves_invalid_json_body_unchanged() {
        let original = b"<<<not json>>>";
        let body = Bytes::from_static(original);
        let result = filter_tokenless_challenges(body);
        assert_eq!(result.as_ref(), original);
    }

    #[tokio::test]
    async fn with_user_agent_sets_or_preserves_and_filters_response() {
        let filtered_source = Bytes::from(
            serde_json::json!({
                "challenges": [
                    {"type": "http-01", "token": "present"},
                    {"type": "dns-persist-01"}
                ]
            })
            .to_string(),
        );
        let seen = Arc::new(Mutex::new(None));
        let client = WithUserAgent(Box::new(MockHttpClient {
            seen_user_agent: Arc::clone(&seen),
            body: filtered_source,
        }));

        let req = Request::builder()
            .uri("https://example.invalid/acme")
            .body(Full::new(Bytes::new()))
            .expect("request");
        let mut rsp = client.request(req).await.expect("request ok");
        let body = rsp.body.into_bytes().await.expect("bytes");
        let parsed: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(parsed["challenges"].as_array().expect("array").len(), 1);
        assert_eq!(
            seen.lock().expect("mutex lock").clone(),
            Some(header::HeaderValue::from_static(concat!(
                "envoy-acme/",
                env!("CARGO_PKG_VERSION")
            )))
        );

        let custom_ua = header::HeaderValue::from_static("custom-agent/1.0");
        let seen_custom = Arc::new(Mutex::new(None));
        let client_custom = WithUserAgent(Box::new(MockHttpClient {
            seen_user_agent: Arc::clone(&seen_custom),
            body: Bytes::from_static(b"{\"challenges\":[]}"),
        }));
        let req_custom = Request::builder()
            .uri("https://example.invalid/acme")
            .header(header::USER_AGENT, custom_ua.clone())
            .body(Full::new(Bytes::new()))
            .expect("request with ua");
        let _ = client_custom
            .request(req_custom)
            .await
            .expect("request with ua ok");
        assert_eq!(
            seen_custom.lock().expect("mutex lock").clone(),
            Some(custom_ua)
        );
    }

    #[test]
    fn build_custom_client_missing_file_returns_io_error() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let missing = tmp.path().join("missing.pem");
        let result = build_custom_client(&missing);
        assert!(matches!(result, Err(AcmeError::Io(_))));
    }

    #[test]
    fn build_custom_client_invalid_pem_returns_io_error() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let ca_path = tmp.path().join("junk.pem");
        std::fs::write(
            &ca_path,
            b"-----BEGIN CERTIFICATE-----\n@@@\n-----END CERTIFICATE-----\n",
        )
        .expect("write");
        let result = build_custom_client(&ca_path);
        assert!(matches!(result, Err(AcmeError::Io(_))));
    }

    #[tokio::test]
    async fn load_or_create_account_invalid_json_returns_json_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("account.json");
        std::fs::write(&path, b"{not-json").expect("write");
        let result = load_or_create_account(
            "https://example.invalid/directory",
            "mailto:test@example.com",
            &path,
            None,
        )
        .await;
        assert!(matches!(result, Err(AcmeError::Json(_))));
    }

    // =========================================================================
    // Network-path tests using the in-process mock ACME server.
    // =========================================================================

    use crate::acme::account_test_server::{CannedResponse, MockAcmeServer, ResponseTable};

    /// Create a new account when no credentials file exists.
    /// Exercises the `Account::create_with_http` → write-to-file path.
    #[tokio::test]
    async fn load_or_create_account_creates_new_when_file_absent() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let account_path = tmp.path().join("account.json");
        let ca_path = tmp.path().join("ca.pem");

        let server = MockAcmeServer::start(|base_url| {
            ResponseTable::new()
                .with_directory(base_url)
                .with_new_nonce("nonce-create-1") // lgtm[rust/hard-coded-cryptographic-value]
                .with_new_account_success(&format!("{base_url}/acme/acct/1"))
        })
        .await;

        std::fs::write(&ca_path, server.ca_cert_pem()).expect("write ca cert");

        let result = load_or_create_account(
            &server.directory_uri(),
            "mailto:test@example.test",
            &account_path,
            Some(&ca_path),
        )
        .await;

        assert!(result.is_ok(), "expected Ok, got error");
        assert!(account_path.exists(), "credentials file should be written");
        assert_eq!(
            server.request_count("GET", "/directory"),
            1,
            "GET /directory once"
        );
        assert_eq!(
            server.request_count("HEAD", "/acme/new-nonce"),
            1,
            "HEAD /acme/new-nonce once"
        );
        assert_eq!(
            server.request_count("POST", "/acme/new-acct"),
            1,
            "POST /acme/new-acct once"
        );
    }

    /// Load existing credentials from file (with custom CA).
    /// Exercises the `Account::from_credentials_and_http` path.
    #[tokio::test]
    async fn load_or_create_account_loads_existing_when_file_present() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let account_path = tmp.path().join("account.json");
        let ca_path = tmp.path().join("ca.pem");

        // Start a server that serves the directory document only.
        // The `from_credentials_and_http` path fetches the directory to
        // obtain fresh DirectoryUrls; it does NOT POST to new-acct.
        let server = MockAcmeServer::start(|base_url| {
            ResponseTable::new()
                .with_directory(base_url)
                .with_new_nonce("nonce-load-1") // lgtm[rust/hard-coded-cryptographic-value]
        })
        .await;

        std::fs::write(&ca_path, server.ca_cert_pem()).expect("write ca cert");

        // Pre-write valid credentials JSON.  The `directory` field causes
        // `instant_acme` to re-fetch the directory on load (no stored URLs),
        // which is the code path we want to exercise.
        let creds_json = serde_json::json!({
            "id": format!("{}/acme/acct/1", server.base_url()),
            // A real ECDSA P-256 private key, taken from instant-acme's own test suite.
            "key_pkcs8": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgJVWC_QzOTCS5vtsJp2IG-UDc8cdDfeoKtxSZxaznM-mhRANCAAQenCPoGgPFTdPJ7VLLKt56RxPlYT1wNXnHc54PEyBg3LxKaH0-sJkX0mL8LyPEdsfL_Oz4TxHkWLJGrXVtNhfH",
            "directory": server.directory_uri()
        });
        std::fs::write(&account_path, serde_json::to_vec(&creds_json).unwrap())
            .expect("write creds");

        let result = load_or_create_account(
            &server.directory_uri(),
            "mailto:test@example.test",
            &account_path,
            Some(&ca_path),
        )
        .await;

        assert!(result.is_ok(), "expected Ok, got error");
        assert_eq!(
            server.request_count("POST", "/acme/new-acct"),
            0,
            "must not create a new account when loading"
        );
        assert_eq!(
            server.request_count("GET", "/directory"),
            1,
            "GET /directory once"
        );
    }

    /// Load existing credentials when no network is needed.
    /// Credentials use the legacy `urls` field (no `directory`), so
    /// `Account::from_credentials` constructs the client without any HTTP
    /// requests.  This exercises the `ca_file = None` load path.
    #[tokio::test]
    async fn load_or_create_account_loads_existing_no_network() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let account_path = tmp.path().join("account.json");

        // Credentials with `urls` only (no `directory`) — instant-acme
        // uses the stored URLs directly and makes no network call on load.
        let creds_json = serde_json::json!({
            "id": "https://acme.example.invalid/acct/1",
            "key_pkcs8": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgJVWC_QzOTCS5vtsJp2IG-UDc8cdDfeoKtxSZxaznM-mhRANCAAQenCPoGgPFTdPJ7VLLKt56RxPlYT1wNXnHc54PEyBg3LxKaH0-sJkX0mL8LyPEdsfL_Oz4TxHkWLJGrXVtNhfH",
            "urls": {
                "newNonce":   "https://acme.example.invalid/nonce",
                "newAccount": "https://acme.example.invalid/new-acct",
                "newOrder":   "https://acme.example.invalid/new-order",
                "revokeCert": "https://acme.example.invalid/revoke"
            }
        });
        std::fs::write(&account_path, serde_json::to_vec(&creds_json).unwrap())
            .expect("write creds");

        let result = load_or_create_account(
            "https://acme.example.invalid/directory",
            "mailto:test@example.test",
            &account_path,
            None,
        )
        .await;

        assert!(result.is_ok(), "expected Ok, got error");
    }

    /// The ACME server returns 400 on new-account → expect a protocol error.
    #[tokio::test]
    async fn load_or_create_account_new_acct_returns_400() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let account_path = tmp.path().join("account.json");
        let ca_path = tmp.path().join("ca.pem");

        let server = MockAcmeServer::start(|base_url| {
            ResponseTable::new()
                .with_directory(base_url)
                .with_new_nonce("nonce-400-1") // lgtm[rust/hard-coded-cryptographic-value]
                .with(
                    "POST",
                    "/acme/new-acct",
                    CannedResponse {
                        status: 400,
                        headers: vec![(
                            "content-type".into(),
                            "application/problem+json".into(),
                        )],
                        body: br#"{"type":"urn:ietf:params:acme:error:malformed","detail":"bad request","status":400}"#
                            .to_vec(),
                    },
                )
        })
        .await;

        std::fs::write(&ca_path, server.ca_cert_pem()).expect("write ca cert");

        let result = load_or_create_account(
            &server.directory_uri(),
            "mailto:test@example.test",
            &account_path,
            Some(&ca_path),
        )
        .await;

        assert!(
            matches!(result, Err(AcmeError::Protocol(_))),
            "expected Protocol error, got non-Protocol result"
        );
    }

    /// The ACME directory endpoint returns 404 → expect an error.
    /// Covers the "wrong directory URL / server unreachable" path.
    #[tokio::test]
    async fn load_or_create_account_directory_returns_404() {
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let account_path = tmp.path().join("account.json");
        let ca_path = tmp.path().join("ca.pem");

        // Start a server with no entries — all requests return 404.
        let server = MockAcmeServer::start(|_base_url| ResponseTable::new()).await;

        std::fs::write(&ca_path, server.ca_cert_pem()).expect("write ca cert");

        let result = load_or_create_account(
            &server.directory_uri(),
            "mailto:test@example.test",
            &account_path,
            Some(&ca_path),
        )
        .await;

        assert!(result.is_err(), "expected Err, got Ok");
    }

    /// Writing the credentials file fails (read-only parent directory).
    /// Exercises the `write_atomic` error path.
    #[cfg(unix)]
    #[tokio::test]
    async fn load_or_create_account_credentials_write_fails() {
        use std::os::unix::fs::PermissionsExt;
        install_ring();
        let tmp = tempfile::tempdir().expect("tempdir");
        let ca_path = tmp.path().join("ca.pem");

        let server = MockAcmeServer::start(|base_url| {
            ResponseTable::new()
                .with_directory(base_url)
                .with_new_nonce("nonce-write-fail") // lgtm[rust/hard-coded-cryptographic-value]
                .with_new_account_success(&format!("{base_url}/acme/acct/1"))
        })
        .await;

        std::fs::write(&ca_path, server.ca_cert_pem()).expect("write ca cert");

        // Make a read-only directory so the credentials write fails.
        let readonly_dir = tmp.path().join("readonly");
        std::fs::create_dir_all(&readonly_dir).expect("create dir");
        std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o555))
            .expect("set readonly");
        let account_path = readonly_dir.join("account.json");

        let result = load_or_create_account(
            &server.directory_uri(),
            "mailto:test@example.test",
            &account_path,
            Some(&ca_path),
        )
        .await;

        // Restore permissions so tempdir cleanup succeeds.
        std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore perms");

        assert!(
            matches!(result, Err(AcmeError::Io(_))),
            "expected Io error, got non-Io result"
        );
    }
}
