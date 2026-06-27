//! Envoy `ext_proc` external processor that serves ACME HTTP-01 challenges.
//!
//! Envoy is configured to mirror request headers to this gRPC service. When a
//! request targets `/.well-known/acme-challenge/<token>`, we answer directly
//! with an `ImmediateResponse` carrying the key authorization, so the ACME
//! server can validate the challenge without the request reaching any upstream.
use std::pin::Pin;

use envoy_types::pb::envoy::config::core::v3::HeaderMap;
use envoy_types::pb::envoy::r#type::v3::HttpStatus;
use envoy_types::pb::envoy::service::ext_proc::v3::{
    common_response::ResponseStatus,
    external_processor_server::{ExternalProcessor, ExternalProcessorServer},
    processing_request, processing_response, CommonResponse, HeadersResponse, HttpHeaders,
    ImmediateResponse, ProcessingRequest, ProcessingResponse,
};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, warn};

use crate::challenge_store::ChallengeStore;

/// URL path prefix used by ACME HTTP-01 challenges (RFC 8555 §8.3).
pub const ACME_CHALLENGE_PREFIX: &str = "/.well-known/acme-challenge/";

/// Extract the challenge token from an ACME HTTP-01 challenge path.
///
/// Returns `None` if the path is not an ACME challenge path or the token is
/// malformed (empty or containing additional path segments).
pub fn acme_challenge_token(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(ACME_CHALLENGE_PREFIX)?;
    let token = rest.split(['?', '#']).next().unwrap_or(rest);
    if token.is_empty() || token.contains('/') {
        None
    } else {
        Some(token)
    }
}

/// Look up a header value by (case-insensitive) name within an Envoy `HeaderMap`.
fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case(name))
        .map(|h| {
            if !h.raw_value.is_empty() {
                String::from_utf8_lossy(&h.raw_value).into_owned()
            } else {
                h.value.clone()
            }
        })
}

/// Build an `ImmediateResponse` with the given status code and body.
fn immediate_response(status_code: i32, body: Vec<u8>) -> ProcessingResponse {
    ProcessingResponse {
        response: Some(processing_response::Response::ImmediateResponse(
            ImmediateResponse {
                status: Some(HttpStatus { code: status_code }),
                headers: None,
                body,
                grpc_status: None,
                details: String::new(),
            },
        )),
        ..Default::default()
    }
}

/// Build a `CONTINUE` response that lets Envoy proceed normally.
fn continue_response() -> ProcessingResponse {
    ProcessingResponse {
        response: Some(processing_response::Response::RequestHeaders(
            HeadersResponse {
                response: Some(CommonResponse {
                    status: ResponseStatus::Continue as i32,
                    ..Default::default()
                }),
            },
        )),
        ..Default::default()
    }
}

/// Decide how to respond to a single request-headers message.
async fn handle_request_headers(
    store: &ChallengeStore,
    headers: &HttpHeaders,
) -> ProcessingResponse {
    let Some(map) = headers.headers.as_ref() else {
        return continue_response();
    };
    let Some(path) = header_value(map, ":path") else {
        return continue_response();
    };
    let Some(token) = acme_challenge_token(&path) else {
        return continue_response();
    };

    match store.get(token).await {
        Some(key_auth) => {
            debug!(token, "serving ACME challenge");
            immediate_response(200, key_auth.into_bytes())
        }
        None => {
            warn!(token, "ACME challenge token not found");
            immediate_response(404, b"acme challenge not found\n".to_vec())
        }
    }
}

/// ext_proc service backed by the shared [`ChallengeStore`].
#[derive(Clone)]
pub struct ExtProcService {
    challenge_store: ChallengeStore,
}

impl ExtProcService {
    pub fn new(challenge_store: ChallengeStore) -> Self {
        Self { challenge_store }
    }

    /// Wrap this service into a tonic gRPC server service.
    pub fn into_server(self) -> ExternalProcessorServer<Self> {
        ExternalProcessorServer::new(self)
    }
}

type ProcessResponseStream =
    Pin<Box<dyn futures::Stream<Item = Result<ProcessingResponse, Status>> + Send>>;

#[tonic::async_trait]
impl ExternalProcessor for ExtProcService {
    type ProcessStream = ProcessResponseStream;

    async fn process(
        &self,
        request: Request<Streaming<ProcessingRequest>>,
    ) -> Result<Response<Self::ProcessStream>, Status> {
        let mut inbound = request.into_inner();
        let store = self.challenge_store.clone();
        let (tx, rx) = mpsc::channel(16);

        tokio::spawn(async move {
            while let Some(message) = inbound.next().await {
                let response = match message {
                    Ok(req) => match req.request {
                        Some(processing_request::Request::RequestHeaders(headers)) => {
                            handle_request_headers(&store, &headers).await
                        }
                        _ => continue_response(),
                    },
                    Err(status) => {
                        let _ = tx.send(Err(status)).await;
                        break;
                    }
                };
                if tx.send(Ok(response)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envoy_types::pb::envoy::config::core::v3::HeaderValue;

    #[test]
    fn token_extracted_from_valid_path() {
        assert_eq!(
            acme_challenge_token("/.well-known/acme-challenge/abc123"),
            Some("abc123")
        );
    }

    #[test]
    fn token_strips_query_string() {
        assert_eq!(
            acme_challenge_token("/.well-known/acme-challenge/abc123?foo=bar"),
            Some("abc123")
        );
    }

    #[test]
    fn non_challenge_paths_rejected() {
        assert_eq!(acme_challenge_token("/"), None);
        assert_eq!(acme_challenge_token("/index.html"), None);
        assert_eq!(acme_challenge_token("/.well-known/other/abc"), None);
    }

    #[test]
    fn malformed_tokens_rejected() {
        assert_eq!(acme_challenge_token("/.well-known/acme-challenge/"), None);
        assert_eq!(
            acme_challenge_token("/.well-known/acme-challenge/a/b"),
            None
        );
    }

    #[test]
    fn header_value_prefers_raw_value() {
        let map = HeaderMap {
            headers: vec![HeaderValue {
                key: ":path".to_string(),
                value: String::new(),
                raw_value: b"/.well-known/acme-challenge/tok".to_vec(),
            }],
        };
        assert_eq!(
            header_value(&map, ":path"),
            Some("/.well-known/acme-challenge/tok".to_string())
        );
    }

    #[tokio::test]
    async fn serves_known_token_with_200() {
        let store = ChallengeStore::new();
        store.insert("tok".to_string(), "keyauth".to_string()).await;
        let headers = HttpHeaders {
            headers: Some(HeaderMap {
                headers: vec![HeaderValue {
                    key: ":path".to_string(),
                    value: "/.well-known/acme-challenge/tok".to_string(),
                    raw_value: Vec::new(),
                }],
            }),
            ..Default::default()
        };
        let resp = handle_request_headers(&store, &headers).await;
        match resp.response {
            Some(processing_response::Response::ImmediateResponse(ir)) => {
                assert_eq!(ir.status.unwrap().code, 200);
                assert_eq!(ir.body, b"keyauth");
            }
            other => panic!("expected immediate response, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_token_returns_404() {
        let store = ChallengeStore::new();
        let headers = HttpHeaders {
            headers: Some(HeaderMap {
                headers: vec![HeaderValue {
                    key: ":path".to_string(),
                    value: "/.well-known/acme-challenge/missing".to_string(),
                    raw_value: Vec::new(),
                }],
            }),
            ..Default::default()
        };
        let resp = handle_request_headers(&store, &headers).await;
        match resp.response {
            Some(processing_response::Response::ImmediateResponse(ir)) => {
                assert_eq!(ir.status.unwrap().code, 404);
            }
            other => panic!("expected immediate response, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn non_challenge_request_continues() {
        let store = ChallengeStore::new();
        let headers = HttpHeaders {
            headers: Some(HeaderMap {
                headers: vec![HeaderValue {
                    key: ":path".to_string(),
                    value: "/index.html".to_string(),
                    raw_value: Vec::new(),
                }],
            }),
            ..Default::default()
        };
        let resp = handle_request_headers(&store, &headers).await;
        assert!(matches!(
            resp.response,
            Some(processing_response::Response::RequestHeaders(_))
        ));
    }
}
