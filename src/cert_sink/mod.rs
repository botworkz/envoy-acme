//! Certificate sink abstraction and shared bundle type.
use crate::errors::SinkError;

pub mod filesystem;

/// A PEM-encoded certificate and private key pair issued by an ACME CA.
#[derive(Clone, Debug)]
pub struct CertBundle {
    /// PEM-encoded certificate chain (leaf certificate first).
    pub cert_pem: Vec<u8>,
    /// PEM-encoded private key corresponding to the certificate.
    pub key_pem: Vec<u8>,
}

/// Sink that receives newly issued or renewed [`CertBundle`]s and makes them available to Envoy.
pub trait CertSink: Send + Sync {
    /// Persist `bundle` under the given `name` (typically the domain), atomically replacing any previous bundle.
    fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError>;
}
