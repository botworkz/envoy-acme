use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Debug, Error)]
pub enum SinkError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("persist error: {0}")]
    Persist(#[from] tempfile::PersistError),
}

#[derive(Debug, Error)]
pub enum AcmeError {
    #[error("acme protocol error: {0}")]
    Protocol(#[from] instant_acme::Error),
    #[error("rcgen error: {0}")]
    CertGen(#[from] rcgen::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("order failed: {0}")]
    OrderFailed(String),
    #[error("sink error: {0}")]
    Sink(#[from] SinkError),
    #[error("missing http-01 challenge for domain: {0}")]
    NoChallenge(String),
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to acquire state_dir lock: {0}")]
    LockAcquisition(#[from] std::io::Error),
    #[error("runtime thread already stopped")]
    Stopped,
}
