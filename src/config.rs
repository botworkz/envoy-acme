//! Configuration structs and loader.
use figment::{
    providers::{Env, Format, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::errors::ConfigError;

/// Top-level configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub acme: AcmeConfig,
    #[serde(default)]
    pub ext_proc: ExtProcConfig,
    #[serde(default)]
    pub sds: SdsConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcmeConfig {
    /// ACME directory URL (e.g. Let's Encrypt staging or production).
    pub directory_url: String,
    /// Contact email, e.g. "mailto:admin@example.com".
    pub contact: String,
    /// Domains to manage certificates for.
    pub domains: Vec<String>,
    /// Days before expiry to trigger renewal (default 30).
    #[serde(default = "default_renewal_window_days")]
    pub renewal_window_days: u64,
    /// Directory for persistent state (account.json, certs, keys).
    pub state_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtProcConfig {
    /// Listen address for the ext_proc gRPC server.
    #[serde(default = "default_ext_proc_listen")]
    pub listen: String,
}

impl Default for ExtProcConfig {
    fn default() -> Self {
        Self {
            listen: default_ext_proc_listen(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SdsConfig {
    /// Listen address for the SDS gRPC server.
    #[serde(default = "default_sds_listen")]
    pub listen: String,
    /// The resource name Envoy will request (default: "acme_cert").
    #[serde(default = "default_resource_name")]
    pub resource_name: String,
}

impl Default for SdsConfig {
    fn default() -> Self {
        Self {
            listen: default_sds_listen(),
            resource_name: default_resource_name(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    /// Log format: "json" or "pretty".
    #[serde(default = "default_log_format")]
    pub format: String,
    /// Log level (default: "info").
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            format: default_log_format(),
            level: default_log_level(),
        }
    }
}

fn default_renewal_window_days() -> u64 {
    30
}
fn default_ext_proc_listen() -> String {
    "0.0.0.0:9000".to_string()
}
fn default_sds_listen() -> String {
    "0.0.0.0:9001".to_string()
}
fn default_resource_name() -> String {
    "acme_cert".to_string()
}
fn default_log_format() -> String {
    "json".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    /// Load configuration from a YAML file, with environment variable overrides.
    /// Env var format: `ENVOY_ACME__SECTION__FIELD` (double underscore separator).
    #[allow(clippy::result_large_err)]
    pub fn load(path: &std::path::Path) -> Result<Self, ConfigError> {
        let config = Figment::new()
            .merge(Yaml::file(path))
            .merge(Env::prefixed("ENVOY_ACME__").split("__"))
            .extract()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_defaults() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(
            file,
            r#"
acme:
  directory_url: "https://example.com/directory"
  contact: "mailto:admin@example.com"
  domains:
    - "example.com"
  state_dir: "/tmp/state"
"#
        )
        .unwrap();
        let config = Config::load(file.path()).unwrap();
        assert_eq!(config.acme.domains, vec!["example.com".to_string()]);
        assert_eq!(config.acme.renewal_window_days, 30);
        assert_eq!(config.ext_proc.listen, "0.0.0.0:9000");
        assert_eq!(config.sds.resource_name, "acme_cert");
        assert_eq!(config.log.format, "json");
    }
}
