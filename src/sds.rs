//! Secret Discovery Service (SDS) that pushes the current TLS cert bundle to Envoy.
//!
//! Envoy subscribes to a single secret (named by [`SdsConfig::resource_name`]).
//! Whenever the [`CertStore`] is updated by the ACME manager, a fresh
//! `DiscoveryResponse` carrying an inline `TlsCertificate` is streamed to Envoy,
//! which hot-reloads the listener certificate without a restart.
//!
//! [`SdsConfig::resource_name`]: crate::config::SdsConfig::resource_name
use std::pin::Pin;

use envoy_types::pb::envoy::config::core::v3::{data_source, DataSource};
use envoy_types::pb::envoy::extensions::transport_sockets::tls::v3::{
    secret, Secret, TlsCertificate,
};
use envoy_types::pb::envoy::service::discovery::v3::{
    DeltaDiscoveryResponse, DiscoveryRequest, DiscoveryResponse,
};
use envoy_types::pb::envoy::service::secret::v3::secret_discovery_service_server::{
    SecretDiscoveryService, SecretDiscoveryServiceServer,
};
use envoy_types::pb::google::protobuf::Any;
use futures::StreamExt;
use prost::Name;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, warn};

use crate::cert_store::{CertBundle, CertStore};

/// Build a `DiscoveryResponse` containing the cert bundle as an inline secret.
fn build_secret_response(
    resource_name: &str,
    bundle: &CertBundle,
) -> Result<DiscoveryResponse, prost::EncodeError> {
    let secret = Secret {
        name: resource_name.to_string(),
        r#type: Some(secret::Type::TlsCertificate(TlsCertificate {
            certificate_chain: Some(DataSource {
                specifier: Some(data_source::Specifier::InlineBytes(
                    bundle.cert_chain_pem.clone(),
                )),
                ..Default::default()
            }),
            private_key: Some(DataSource {
                specifier: Some(data_source::Specifier::InlineBytes(
                    bundle.private_key_pem.clone(),
                )),
                ..Default::default()
            }),
            ..Default::default()
        })),
    };

    let mut value = Vec::new();
    prost::Message::encode(&secret, &mut value)?;

    let type_url = Secret::type_url();
    Ok(DiscoveryResponse {
        version_info: bundle.version.to_string(),
        resources: vec![Any {
            type_url: type_url.clone(),
            value,
        }],
        type_url,
        nonce: bundle.version.to_string(),
        ..Default::default()
    })
}

/// SDS service backed by the shared [`CertStore`].
#[derive(Clone)]
pub struct SdsService {
    cert_store: CertStore,
    resource_name: String,
}

impl SdsService {
    pub fn new(cert_store: CertStore, resource_name: String) -> Self {
        Self {
            cert_store,
            resource_name,
        }
    }

    /// Wrap this service into a tonic gRPC server service.
    pub fn into_server(self) -> SecretDiscoveryServiceServer<Self> {
        SecretDiscoveryServiceServer::new(self)
    }

    /// Build a response for the current cert bundle, if one is available.
    fn current_response(&self) -> Option<DiscoveryResponse> {
        let bundle = self.cert_store.current();
        if bundle.cert_chain_pem.is_empty() {
            return None;
        }
        match build_secret_response(&self.resource_name, &bundle) {
            Ok(resp) => Some(resp),
            Err(err) => {
                warn!(%err, "failed to encode SDS secret");
                None
            }
        }
    }
}

type SecretResponseStream =
    Pin<Box<dyn futures::Stream<Item = Result<DiscoveryResponse, Status>> + Send>>;
type DeltaResponseStream =
    Pin<Box<dyn futures::Stream<Item = Result<DeltaDiscoveryResponse, Status>> + Send>>;

#[tonic::async_trait]
impl SecretDiscoveryService for SdsService {
    type StreamSecretsStream = SecretResponseStream;
    type DeltaSecretsStream = DeltaResponseStream;

    async fn stream_secrets(
        &self,
        request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamSecretsStream>, Status> {
        let mut inbound = request.into_inner();
        let resource_name = self.resource_name.clone();
        let mut cert_rx = self.cert_store.subscribe();
        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            let send_current = |bundle: CertBundle| {
                if bundle.cert_chain_pem.is_empty() {
                    return None;
                }
                match build_secret_response(&resource_name, &bundle) {
                    Ok(resp) => Some(resp),
                    Err(err) => {
                        warn!(%err, "failed to encode SDS secret");
                        None
                    }
                }
            };

            loop {
                tokio::select! {
                    message = inbound.next() => {
                        match message {
                            Some(Ok(req)) => {
                                debug!(version = %req.version_info, "SDS discovery request");
                                let bundle = cert_rx.borrow().clone();
                                if let Some(resp) = send_current(bundle) {
                                    if tx.send(Ok(resp)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Some(Err(status)) => {
                                let _ = tx.send(Err(status)).await;
                                break;
                            }
                            None => break,
                        }
                    }
                    changed = cert_rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let bundle = cert_rx.borrow_and_update().clone();
                        if let Some(resp) = send_current(bundle) {
                            if tx.send(Ok(resp)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn fetch_secrets(
        &self,
        _request: Request<DiscoveryRequest>,
    ) -> Result<Response<DiscoveryResponse>, Status> {
        match self.current_response() {
            Some(resp) => Ok(Response::new(resp)),
            None => Err(Status::unavailable("no certificate available yet")),
        }
    }

    async fn delta_secrets(
        &self,
        _request: Request<
            Streaming<envoy_types::pb::envoy::service::discovery::v3::DeltaDiscoveryRequest>,
        >,
    ) -> Result<Response<Self::DeltaSecretsStream>, Status> {
        Err(Status::unimplemented("delta SDS is not supported"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_contains_inline_secret() {
        let bundle = CertBundle {
            cert_chain_pem: b"-----BEGIN CERTIFICATE-----".to_vec(),
            private_key_pem: b"-----BEGIN PRIVATE KEY-----".to_vec(),
            version: 7,
        };
        let resp = build_secret_response("acme_cert", &bundle).unwrap();
        assert_eq!(resp.version_info, "7");
        assert_eq!(resp.resources.len(), 1);
        assert_eq!(resp.type_url, Secret::type_url());
        assert_eq!(resp.resources[0].type_url, Secret::type_url());

        let decoded: Secret = prost::Message::decode(resp.resources[0].value.as_slice()).unwrap();
        assert_eq!(decoded.name, "acme_cert");
        match decoded.r#type {
            Some(secret::Type::TlsCertificate(tls)) => {
                match tls.certificate_chain.unwrap().specifier.unwrap() {
                    data_source::Specifier::InlineBytes(b) => {
                        assert_eq!(b, b"-----BEGIN CERTIFICATE-----");
                    }
                    other => panic!("unexpected specifier {other:?}"),
                }
            }
            other => panic!("unexpected secret type {other:?}"),
        }
    }

    #[test]
    fn no_response_when_cert_missing() {
        let store = CertStore::new();
        let svc = SdsService::new(store, "acme_cert".to_string());
        assert!(svc.current_response().is_none());
    }
}
