//! In-process HTTPS mock ACME server for unit-testing `src/acme/account.rs`.
//!
//! This file is a test fixture, not production code or a test. It is
//! excluded from coverage measurement (see CONTRIBUTING.md "Coverage
//! exclusions").

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use parking_lot::Mutex;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::PrivatePkcs8KeyDer;
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;

/// A canned HTTP response keyed by `(method, path)`.
#[derive(Clone, Debug)]
pub(crate) struct CannedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Maps `(method, path)` → [`CannedResponse`]. Constructed by each test.
pub(crate) struct ResponseTable {
    entries: HashMap<(String, String), CannedResponse>,
}

impl ResponseTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add an ACME directory document response at `GET /directory`.
    ///
    /// `base_url` is the `https://127.0.0.1:PORT` prefix the server is bound
    /// to. Pass `&server.base_url()` (available from the `make_table` closure
    /// argument) to embed absolute URLs into the directory document.
    pub fn with_directory(self, base_url: &str) -> Self {
        let body = serde_json::json!({
            "newNonce":   format!("{base_url}/acme/new-nonce"),
            "newAccount": format!("{base_url}/acme/new-acct"),
            "newOrder":   format!("{base_url}/acme/new-order"),
            "revokeCert": format!("{base_url}/acme/revoke-cert"),
            "keyChange":  format!("{base_url}/acme/key-change"),
        });
        self.with(
            "GET",
            "/directory",
            CannedResponse {
                status: 200,
                headers: vec![("content-type".into(), "application/json".into())],
                body: serde_json::to_vec(&body).expect("directory json"),
            },
        )
    }

    /// Add a `200` response on `HEAD /acme/new-nonce` with a `Replay-Nonce`
    /// header set to `nonce`.
    pub fn with_new_nonce(self, nonce: &str) -> Self {
        self.with(
            "HEAD",
            "/acme/new-nonce",
            CannedResponse {
                status: 200,
                headers: vec![("Replay-Nonce".into(), nonce.to_owned())],
                body: vec![],
            },
        )
    }

    /// Add a `201` response on `POST /acme/new-acct`.
    ///
    /// `location` is placed in the `Location` header (typically
    /// `https://127.0.0.1:PORT/acme/acct/1`).
    pub fn with_new_account_success(self, location: &str) -> Self {
        self.with(
            "POST",
            "/acme/new-acct",
            CannedResponse {
                status: 201,
                headers: vec![
                    ("Location".into(), location.to_owned()),
                    ("content-type".into(), "application/json".into()),
                ],
                body: br#"{"status":"valid"}"#.to_vec(),
            },
        )
    }

    /// Add a custom response at `(method, path)`.
    pub fn with(mut self, method: &str, path: &str, response: CannedResponse) -> Self {
        self.entries
            .insert((method.to_uppercase(), path.to_owned()), response);
        self
    }
}

/// In-process TLS mock ACME server.
///
/// Each test starts its own server via [`MockAcmeServer::start`], which binds
/// to `127.0.0.1:0` (kernel-assigned port) and serves TLS with a fresh
/// self-signed certificate. The server shuts down when dropped.
pub(crate) struct MockAcmeServer {
    addr: SocketAddr,
    ca_cert_pem: String,
    shutdown: Option<oneshot::Sender<()>>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    _join: tokio::task::JoinHandle<()>,
}

impl MockAcmeServer {
    /// Start the server.
    ///
    /// `make_table` receives the server's `base_url` (e.g.
    /// `https://127.0.0.1:PORT`) so it can embed absolute URLs into the
    /// directory document and `Location` headers before returning the
    /// [`ResponseTable`].
    ///
    /// Returns once the socket is bound and the accept loop is running.
    pub async fn start(make_table: impl FnOnce(&str) -> ResponseTable) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind to 127.0.0.1:0");
        let addr = listener.local_addr().expect("local_addr");
        let base_url = format!("https://{addr}");

        // Generate a self-signed TLS certificate with the server IP as SAN so
        // that build_custom_client (hyper-rustls with https_only) accepts it.
        let CertifiedKey { cert, key_pair } =
            generate_simple_self_signed(vec!["127.0.0.1".to_string()])
                .expect("generate self-signed cert");
        let ca_cert_pem = cert.pem();
        let cert_der = cert.der().clone();
        let key_der = PrivatePkcs8KeyDer::from(key_pair.serialize_der());

        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der.into())
            .expect("build ServerConfig");
        let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

        let table = Arc::new(make_table(&base_url));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let requests: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

        let requests_srv = Arc::clone(&requests);
        let join = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _)) => {
                                let acceptor = tls_acceptor.clone();
                                let reqs = Arc::clone(&requests_srv);
                                let tbl = Arc::clone(&table);
                                tokio::spawn(async move {
                                    if let Ok(tls_stream) = acceptor.accept(stream).await {
                                        let io = TokioIo::new(tls_stream);
                                        let _ = Builder::new(TokioExecutor::new())
                                            .serve_connection(
                                                io,
                                                service_fn(move |req: Request<hyper::body::Incoming>| {
                                                    let reqs = Arc::clone(&reqs);
                                                    let tbl = Arc::clone(&tbl);
                                                    async move {
                                                        Ok::<_, hyper::Error>(
                                                            handle_request(req, &reqs, &tbl).await,
                                                        )
                                                    }
                                                }),
                                            )
                                            .await;
                                    }
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    _ = &mut shutdown_rx => break,
                }
            }
        });

        Self {
            addr,
            ca_cert_pem,
            shutdown: Some(shutdown_tx),
            requests,
            _join: join,
        }
    }

    /// Returns `https://127.0.0.1:PORT` (no trailing slash).
    pub fn base_url(&self) -> String {
        format!("https://{}", self.addr)
    }

    /// Returns `https://127.0.0.1:PORT/directory`.
    pub fn directory_uri(&self) -> String {
        format!("{}/directory", self.base_url())
    }

    /// Returns the PEM-encoded self-signed CA certificate for this server.
    ///
    /// Write this to a temporary file and pass the path as the `ca_file`
    /// argument to `load_or_create_account` so that `build_custom_client`
    /// trusts the server.
    pub fn ca_cert_pem(&self) -> &str {
        &self.ca_cert_pem
    }

    /// Returns the count of recorded requests that match `(method, path)`.
    pub fn request_count(&self, method: &str, path: &str) -> usize {
        let method_upper = method.to_uppercase();
        self.requests
            .lock()
            .iter()
            .filter(|(m, p)| *m == method_upper && p == path)
            .count()
    }
}

impl Drop for MockAcmeServer {
    fn drop(&mut self) {
        // Send the shutdown signal; ignore the error if the receiver is gone.
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    requests: &Arc<Mutex<Vec<(String, String)>>>,
    table: &Arc<ResponseTable>,
) -> Response<Full<Bytes>> {
    let method = req.method().as_str().to_uppercase();
    let path = req.uri().path().to_owned();

    requests.lock().push((method.clone(), path.clone()));

    match table.entries.get(&(method.clone(), path)) {
        Some(canned) => {
            let mut builder = Response::builder().status(canned.status);
            for (k, v) in &canned.headers {
                builder = builder.header(k.as_str(), v.as_str());
            }
            // HEAD responses must carry an empty body per HTTP spec.
            let body = if method == "HEAD" {
                Bytes::new()
            } else {
                Bytes::from(canned.body.clone())
            };
            builder.body(Full::new(body)).expect("build response")
        }
        None => Response::builder()
            .status(404)
            .body(Full::new(Bytes::from_static(b"not found")))
            .expect("build 404"),
    }
}
