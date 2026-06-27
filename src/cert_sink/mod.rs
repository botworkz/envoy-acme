use crate::errors::SinkError;

pub mod filesystem;

#[derive(Clone, Debug)]
pub struct CertBundle {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

pub trait CertSink: Send + Sync {
    fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError>;
}
