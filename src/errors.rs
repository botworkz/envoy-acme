use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcmeError {
    #[error("ACME protocol error: {0}")]
    Protocol(#[from] instant_acme::Error),
    #[error("Certificate generation error: {0}")]
    CertGen(#[from] rcgen::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("No valid challenge found for domain: {0}")]
    NoChallenge(String),
    #[error("Order failed with status: {0}")]
    OrderFailed(String),
}

/// Error type for the ext_proc gRPC server task.
///
/// TODO: surface these at the binary boundary once the server startup path
/// returns structured errors instead of `anyhow`.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ExtProcError {
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),
}

/// Error type for the SDS gRPC server task.
///
/// TODO: surface these at the binary boundary once the server startup path
/// returns structured errors instead of `anyhow`.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum SdsError {
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration error: {0}")]
    Figment(#[from] figment::Error),
    /// TODO: used when additional config validation is added on top of figment's
    /// own required-field checking.
    #[allow(dead_code)]
    #[error("Missing required field: {0}")]
    MissingField(String),
}
