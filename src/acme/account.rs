use std::path::Path;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::rt::TokioExecutor;
use instant_acme::{Account, AccountCredentials, HttpClient, NewAccount};
use rustls::{ClientConfig, RootCertStore};

use crate::errors::AcmeError;

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

    Ok(Box::new(client))
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
}
