use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::errors::ConfigError;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub acme: AcmeConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AcmeConfig {
    pub directory_cluster: String,
    pub directory_uri: String,
    pub contact: String,
    pub domains: Vec<String>,
    #[serde(default = "default_renewal_window_days")]
    pub renewal_window_days: u64,
    pub state_dir: PathBuf,
    pub cert_sink: CertSinkConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CertSinkConfig {
    #[serde(rename = "type")]
    pub sink_type: String,
    pub cert_dir: PathBuf,
    #[serde(default)]
    pub layout: Layout,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    PerDomain,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_renewal_window_days() -> u64 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    pub fn from_bytes(raw: &[u8]) -> Result<Self, ConfigError> {
        match serde_json::from_slice(raw) {
            Ok(v) => Ok(v),
            Err(_json_err) => Ok(serde_yaml::from_slice(raw)?),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yaml() {
        let raw = br#"
acme:
  directory_cluster: acme_directory
  directory_uri: https://example.invalid/directory
  contact: mailto:admin@example.test
  domains: [example.test]
  renewal_window_days: 10
  state_dir: /tmp/acme
  cert_sink:
    type: filesystem
    cert_dir: /tmp/certs
    layout: per_domain
"#;

        let cfg = Config::from_bytes(raw).expect("yaml parse");
        assert_eq!(cfg.acme.domains, vec!["example.test".to_string()]);
        assert_eq!(cfg.acme.renewal_window_days, 10);
    }
}
