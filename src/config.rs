use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::errors::ConfigError;

const LE_STAGING_URL: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
const LE_PRODUCTION_URL: &str = "https://acme-v02.api.letsencrypt.org/directory";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub acme: AcmeConfig,
    #[serde(default)]
    pub log: LogConfig,
}

/// Selects a known ACME directory endpoint by name.
///
/// When set, the appropriate URL is resolved automatically.  Supply
/// `directory_uri` only when using `custom`; for `staging` and `production`
/// the URL is fixed and `directory_uri` may be omitted (or set to the
/// expected value for documentation purposes).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryProfile {
    Staging,
    Production,
    Custom,
}

/// Raw deserialization form – `directory_uri` is optional so that
/// `staging`/`production` profiles do not require it.
#[derive(Deserialize)]
struct RawAcmeConfig {
    #[serde(default)]
    directory_profile: Option<DirectoryProfile>,
    #[serde(default)]
    directory_uri: Option<String>,
    /// Path to a PEM file containing a CA certificate bundle to trust when
    /// connecting to the ACME directory.  Primarily used in integration tests
    /// to trust Pebble's self-signed CA.  When absent, the system native roots
    /// are used.
    #[serde(default)]
    directory_ca_file: Option<PathBuf>,
    contact: String,
    domains: Vec<String>,
    #[serde(default = "default_renewal_window_days")]
    renewal_window_days: u64,
    state_dir: PathBuf,
    cert_sink: CertSinkConfig,
    /// How often (in seconds) the ACME state machine timer fires to check for
    /// renewal.  Defaults to 60.  Set lower in integration environments.
    #[serde(default = "default_tick_seconds")]
    tick_seconds: u64,
}

impl TryFrom<RawAcmeConfig> for AcmeConfig {
    type Error = String;

    fn try_from(raw: RawAcmeConfig) -> Result<Self, Self::Error> {
        let directory_uri = match (raw.directory_profile, raw.directory_uri.as_deref()) {
            (None, None) => {
                return Err(
                    "acme.directory_uri is required when directory_profile is not set".to_string(),
                )
            }
            (None, Some(uri)) => uri.to_string(),
            (Some(DirectoryProfile::Staging), None) => LE_STAGING_URL.to_string(),
            (Some(DirectoryProfile::Staging), Some(uri)) => {
                if uri != LE_STAGING_URL {
                    return Err(format!(
                        "acme.directory_uri '{uri}' does not match the Let's Encrypt staging URL \
                        '{LE_STAGING_URL}'. \
                        Remove directory_uri or change directory_profile to 'custom'."
                    ));
                }
                LE_STAGING_URL.to_string()
            }
            (Some(DirectoryProfile::Production), None) => LE_PRODUCTION_URL.to_string(),
            (Some(DirectoryProfile::Production), Some(uri)) => {
                if uri != LE_PRODUCTION_URL {
                    return Err(format!(
                        "acme.directory_uri '{uri}' does not match the Let's Encrypt production URL \
                        '{LE_PRODUCTION_URL}'. \
                        Remove directory_uri or change directory_profile to 'custom'."
                    ));
                }
                LE_PRODUCTION_URL.to_string()
            }
            (Some(DirectoryProfile::Custom), None) => {
                return Err(
                    "acme.directory_uri is required when directory_profile is 'custom'".to_string(),
                )
            }
            (Some(DirectoryProfile::Custom), Some(uri)) => {
                if uri == LE_STAGING_URL {
                    return Err(format!(
                        "acme.directory_profile is 'custom' but directory_uri is the Let's Encrypt staging URL '{LE_STAGING_URL}'. Use directory_profile: staging instead."
                    ));
                }
                if uri == LE_PRODUCTION_URL {
                    return Err(format!(
                        "acme.directory_profile is 'custom' but directory_uri is the Let's Encrypt production URL '{LE_PRODUCTION_URL}'. Use directory_profile: production instead."
                    ));
                }
                uri.to_string()
            }
        };

        if raw.tick_seconds == 0 {
            return Err("acme.tick_seconds must be >= 1 (got 0)".to_string());
        }

        if raw.domains.is_empty() {
            return Err("acme.domains must contain at least one domain".to_string());
        }
        for (i, value) in raw.domains.iter().enumerate() {
            let reason = if value.is_empty() {
                "must be non-empty"
            } else if value.starts_with('.') {
                "must not start with '.'"
            } else if value.chars().any(char::is_whitespace) {
                "must not contain whitespace"
            } else {
                continue;
            };
            return Err(format!(
                "acme.domains[{i}]: invalid domain {value:?}: {reason}"
            ));
        }

        if raw.contact.trim().is_empty() {
            return Err("acme.contact must be non-empty".to_string());
        }

        if !(1..=365).contains(&raw.renewal_window_days) {
            return Err(format!(
                "acme.renewal_window_days must be between 1 and 365 (got {})",
                raw.renewal_window_days
            ));
        }

        if raw.cert_sink.sink_type != "filesystem" {
            return Err(format!(
                "acme.cert_sink.type {:?} is not supported; only \"filesystem\" is currently supported",
                raw.cert_sink.sink_type
            ));
        }

        Ok(AcmeConfig {
            directory_profile: raw.directory_profile,
            directory_uri,
            directory_ca_file: raw.directory_ca_file,
            contact: raw.contact,
            domains: raw.domains,
            renewal_window_days: raw.renewal_window_days,
            state_dir: raw.state_dir,
            cert_sink: raw.cert_sink,
            tick_seconds: raw.tick_seconds,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "RawAcmeConfig")]
pub struct AcmeConfig {
    /// Which ACME directory profile to use.  When set, `directory_uri` is
    /// resolved from the profile; see [`DirectoryProfile`] for details.
    #[serde(default)]
    pub directory_profile: Option<DirectoryProfile>,
    /// Resolved ACME directory URL.  Always populated after validation.
    pub directory_uri: String,
    /// Path to a PEM file containing a CA certificate bundle to trust when
    /// connecting to the ACME directory.  Primarily used in integration tests
    /// to trust Pebble's self-signed CA.  When absent, the system native roots
    /// are used.
    #[serde(default)]
    pub directory_ca_file: Option<PathBuf>,
    pub contact: String,
    pub domains: Vec<String>,
    #[serde(default = "default_renewal_window_days")]
    pub renewal_window_days: u64,
    pub state_dir: PathBuf,
    pub cert_sink: CertSinkConfig,
    /// How often (in seconds) the ACME state machine timer fires to check for
    /// renewal.  Defaults to 60.  Set lower in integration environments.
    #[serde(default = "default_tick_seconds")]
    pub tick_seconds: u64,
}

impl Serialize for AcmeConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("AcmeConfig", 9)?;
        if let Some(profile) = self.directory_profile {
            let profile = match profile {
                DirectoryProfile::Staging => "staging",
                DirectoryProfile::Production => "production",
                DirectoryProfile::Custom => "custom",
            };
            state.serialize_field("directory_profile", profile)?;
        }
        match self.directory_profile {
            Some(DirectoryProfile::Staging) | Some(DirectoryProfile::Production) => {}
            None | Some(DirectoryProfile::Custom) => {
                state.serialize_field("directory_uri", &self.directory_uri)?;
            }
        }
        state.serialize_field("directory_ca_file", &self.directory_ca_file)?;
        state.serialize_field("contact", &self.contact)?;
        state.serialize_field("domains", &self.domains)?;
        state.serialize_field("renewal_window_days", &self.renewal_window_days)?;
        state.serialize_field("state_dir", &self.state_dir)?;
        state.serialize_field("cert_sink", &self.cert_sink)?;
        state.serialize_field("tick_seconds", &self.tick_seconds)?;
        state.end()
    }
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

fn default_tick_seconds() -> u64 {
    60
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

    fn base_yaml(extra: &str) -> String {
        format!(
            r#"
acme:
  {extra}
  contact: mailto:admin@example.test
  domains: [example.test]
  renewal_window_days: 10
  state_dir: /tmp/acme
  cert_sink:
    type: filesystem
    cert_dir: /tmp/certs
    layout: per_domain
"#
        )
    }

    // ── profile=unset, directory_uri present → use verbatim ──────────────────

    #[test]
    fn parses_yaml_no_profile_with_uri() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory");
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(cfg.acme.domains, vec!["example.test".to_string()]);
        assert_eq!(cfg.acme.renewal_window_days, 10);
        assert_eq!(cfg.acme.directory_uri, "https://example.invalid/directory");
        assert_eq!(cfg.acme.directory_profile, None);
    }

    // ── profile=unset, no directory_uri → error ───────────────────────────────

    #[test]
    fn rejects_no_profile_no_uri() {
        let raw = base_yaml("");
        let err =
            Config::from_bytes(raw.as_bytes()).expect_err("should fail without uri or profile");
        let msg = err.to_string();
        assert!(
            msg.contains("directory_uri"),
            "error should mention directory_uri, got: {msg}"
        );
    }

    // ── profile=staging, no directory_uri → force staging URL ────────────────

    #[test]
    fn staging_profile_no_uri_resolves_staging_url() {
        let raw = base_yaml("directory_profile: staging");
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(cfg.acme.directory_profile, Some(DirectoryProfile::Staging));
        assert_eq!(cfg.acme.directory_uri, LE_STAGING_URL);
    }

    // ── profile=staging, correct staging URI → ok ────────────────────────────

    #[test]
    fn staging_profile_matching_uri_ok() {
        let raw = base_yaml(&format!(
            "directory_profile: staging\n  directory_uri: {LE_STAGING_URL}"
        ));
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(cfg.acme.directory_uri, LE_STAGING_URL);
    }

    // ── profile=staging, wrong URI → error ───────────────────────────────────

    #[test]
    fn staging_profile_wrong_uri_rejected() {
        let raw = base_yaml(
            "directory_profile: staging\n  directory_uri: https://example.invalid/directory",
        );
        let err = Config::from_bytes(raw.as_bytes()).expect_err("should reject wrong uri");
        let msg = err.to_string();
        assert!(
            msg.contains("staging") || msg.contains("directory_uri"),
            "error should mention staging or directory_uri, got: {msg}"
        );
    }

    // ── profile=production, no directory_uri → force production URL ──────────

    #[test]
    fn production_profile_no_uri_resolves_production_url() {
        let raw = base_yaml("directory_profile: production");
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(
            cfg.acme.directory_profile,
            Some(DirectoryProfile::Production)
        );
        assert_eq!(cfg.acme.directory_uri, LE_PRODUCTION_URL);
    }

    // ── profile=production, correct production URI → ok ──────────────────────

    #[test]
    fn production_profile_matching_uri_ok() {
        let raw = base_yaml(&format!(
            "directory_profile: production\n  directory_uri: {LE_PRODUCTION_URL}"
        ));
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(cfg.acme.directory_uri, LE_PRODUCTION_URL);
    }

    // ── profile=production, wrong URI → error ────────────────────────────────

    #[test]
    fn production_profile_wrong_uri_rejected() {
        let raw = base_yaml(
            "directory_profile: production\n  directory_uri: https://example.invalid/directory",
        );
        let err = Config::from_bytes(raw.as_bytes()).expect_err("should reject wrong uri");
        let msg = err.to_string();
        assert!(
            msg.contains("production") || msg.contains("directory_uri"),
            "error should mention production or directory_uri, got: {msg}"
        );
    }

    // ── profile=custom, directory_uri present → use verbatim ─────────────────

    #[test]
    fn custom_profile_with_uri_ok() {
        let raw = base_yaml("directory_profile: custom\n  directory_uri: https://pebble:14000/dir");
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(cfg.acme.directory_profile, Some(DirectoryProfile::Custom));
        assert_eq!(cfg.acme.directory_uri, "https://pebble:14000/dir");
    }

    #[test]
    fn custom_profile_with_le_staging_uri_rejected() {
        let raw = base_yaml(&format!(
            "directory_profile: custom\n  directory_uri: {LE_STAGING_URL}"
        ));
        let err = Config::from_bytes(raw.as_bytes()).expect_err("should reject LE staging URL");
        let msg = err.to_string();
        assert!(
            msg.contains("custom") && msg.contains("staging"),
            "error should mention custom and staging, got: {msg}"
        );
    }

    #[test]
    fn custom_profile_with_le_production_uri_rejected() {
        let raw = base_yaml(&format!(
            "directory_profile: custom\n  directory_uri: {LE_PRODUCTION_URL}"
        ));
        let err = Config::from_bytes(raw.as_bytes()).expect_err("should reject LE production URL");
        let msg = err.to_string();
        assert!(
            msg.contains("custom") && msg.contains("production"),
            "error should mention custom and production, got: {msg}"
        );
    }

    // ── profile=custom, no directory_uri → error ─────────────────────────────

    #[test]
    fn custom_profile_no_uri_rejected() {
        let raw = base_yaml("directory_profile: custom");
        let err =
            Config::from_bytes(raw.as_bytes()).expect_err("custom profile requires directory_uri");
        let msg = err.to_string();
        assert!(
            msg.contains("directory_uri") || msg.contains("custom"),
            "error should mention directory_uri or custom, got: {msg}"
        );
    }

    #[test]
    fn rejects_tick_seconds_zero() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory\n  tick_seconds: 0");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("tick_seconds=0 should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("tick_seconds") && msg.contains(">= 1"),
            "error should mention tick_seconds lower bound, got: {msg}"
        );
    }

    #[test]
    fn rejects_empty_domains_list() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("domains: [example.test]", "domains: []");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("empty domains should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domains") && msg.contains("at least one"),
            "error should mention domains non-empty, got: {msg}"
        );
    }

    #[test]
    fn rejects_empty_domain_entry() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("domains: [example.test]", r#"domains: [""]"#);
        let err = Config::from_bytes(raw.as_bytes()).expect_err("empty domain should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domains[0]") && msg.contains("non-empty"),
            "error should mention invalid empty domain, got: {msg}"
        );
    }

    #[test]
    fn rejects_domain_starting_with_dot() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("domains: [example.test]", "domains: [.example.test]");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("leading dot domain should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domains[0]") && msg.contains("start"),
            "error should mention leading dot domain, got: {msg}"
        );
    }

    #[test]
    fn rejects_domain_with_whitespace() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("domains: [example.test]", r#"domains: ["example .test"]"#);
        let err = Config::from_bytes(raw.as_bytes()).expect_err("whitespace domain should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domains[0]") && msg.contains("whitespace"),
            "error should mention whitespace domain, got: {msg}"
        );
    }

    #[test]
    fn rejects_empty_contact() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("contact: mailto:admin@example.test", "contact: \"   \"");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("empty contact should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("contact") && msg.contains("non-empty"),
            "error should mention non-empty contact, got: {msg}"
        );
    }

    #[test]
    fn rejects_renewal_window_days_out_of_range() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("renewal_window_days: 10", "renewal_window_days: 0");
        let err =
            Config::from_bytes(raw.as_bytes()).expect_err("renewal_window_days=0 should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("renewal_window_days") && msg.contains("between 1 and 365"),
            "error should mention renewal_window_days range, got: {msg}"
        );
    }

    #[test]
    fn rejects_unsupported_cert_sink_type() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("type: filesystem", "type: s3");
        let err =
            Config::from_bytes(raw.as_bytes()).expect_err("unsupported sink type should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("cert_sink.type") && msg.contains("filesystem"),
            "error should mention cert_sink.type support, got: {msg}"
        );
    }
}
