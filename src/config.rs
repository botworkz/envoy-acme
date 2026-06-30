use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::errors::ConfigError;

const LE_STAGING_URL: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
const LE_PRODUCTION_URL: &str = "https://acme-v02.api.letsencrypt.org/directory";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
    /// Permit a plain-HTTP `directory_uri` when `directory_profile` is
    /// `custom`.  Must be explicitly set to `true`; credentials and nonces
    /// will traverse the network in cleartext.  Only intended for local
    /// integration-test environments (e.g. Pebble without TLS).
    #[serde(default)]
    allow_insecure_directory: Option<bool>,
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
    /// Wall-clock budget for a single issuance attempt (HTTPS calls to the
    /// ACME directory included).  If the attempt does not complete within this
    /// many seconds the tick returns `AcmeError::Timeout` and the next tick
    /// gets a fresh attempt.  Defaults to 120.  Must be in [5, 600].
    #[serde(default = "default_issuance_timeout_seconds")]
    issuance_timeout_seconds: u64,
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

        // Enforce HTTPS scheme.  For `staging`/`production` the resolved URL
        // is always https:// already.  For `custom` (and no-profile), reject
        // plain HTTP unless the operator has explicitly opted in via
        // `allow_insecure_directory: true`.
        let scheme = directory_uri
            .split_once("://")
            .map(|(s, _)| s)
            .unwrap_or("");
        match scheme {
            "https" => {}
            "http" if raw.directory_profile == Some(DirectoryProfile::Custom) => {
                if !raw.allow_insecure_directory.unwrap_or(false) {
                    return Err("acme.directory_uri uses plain HTTP; set \
                        acme.allow_insecure_directory: true to permit this \
                        (only valid for directory_profile: custom)"
                        .to_string());
                }
                tracing::warn!(
                    directory_uri = %directory_uri,
                    "envoy-acme: directory_uri uses plain HTTP; credentials and nonces will \
                    traverse the network in cleartext",
                );
            }
            "http" => {
                let profile = match raw.directory_profile {
                    Some(DirectoryProfile::Staging) => "staging",
                    Some(DirectoryProfile::Production) => "production",
                    None => "unset (no directory_profile)",
                    Some(DirectoryProfile::Custom) => unreachable!(),
                };
                return Err(format!(
                    "acme.directory_uri must use https:// scheme; plain HTTP is not permitted \
                    for directory_profile: {profile}"
                ));
            }
            other => {
                return Err(format!(
                    "acme.directory_uri must use https:// scheme (got {other:?})"
                ));
            }
        }

        if raw.tick_seconds == 0 {
            return Err("acme.tick_seconds must be >= 1 (got 0)".to_string());
        }

        if !(5..=600).contains(&raw.issuance_timeout_seconds) {
            return Err(format!(
                "acme.issuance_timeout_seconds must be between 5 and 600 (got {})",
                raw.issuance_timeout_seconds
            ));
        }

        if raw.domains.is_empty() {
            return Err("acme.domains must contain at least one domain".to_string());
        }
        for (i, value) in raw.domains.iter().enumerate() {
            // Wildcard domains are the only domain-syntax footgun we reject
            // explicitly: they require dns-01 challenges, which this module
            // does not implement. Issuance would silently fail at the first
            // tick with an opaque CA error; rejecting up front gives the
            // operator a clear message at config-load time.
            //
            // Other RFC-1035 / IDN / CA-policy concerns (consecutive dots,
            // leading/trailing hyphens, trailing dots, case, etc.) are not
            // validated here — they are a rathole, and the CA will return a
            // structured error if the domain is malformed.
            let reason = if value.is_empty() {
                "must be non-empty"
            } else if value.starts_with("*.") {
                "wildcards are not supported (http-01 challenges only)"
            } else if value.starts_with('.') {
                "must not start with '.'"
            } else if value.chars().any(char::is_whitespace) {
                "must not contain whitespace"
            } else if !value.is_ascii() {
                "non-ASCII characters are not yet supported; \
                 convert to A-label form (e.g. xn--mnchen-3ya.example) or wait for IDNA support in a future release"
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
            issuance_timeout_seconds: raw.issuance_timeout_seconds,
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
    /// Wall-clock budget for a single issuance attempt.  If the attempt does
    /// not complete within this many seconds the tick returns
    /// `AcmeError::Timeout` and the next tick gets a fresh attempt.
    /// Defaults to 120.  Must be in [5, 600].
    #[serde(default = "default_issuance_timeout_seconds")]
    pub issuance_timeout_seconds: u64,
}

impl Serialize for AcmeConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("AcmeConfig", 10)?;
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
        state.serialize_field("issuance_timeout_seconds", &self.issuance_timeout_seconds)?;
        state.end()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

fn default_issuance_timeout_seconds() -> u64 {
    120
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
    fn rejects_wildcard_domain() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("domains: [example.test]", r#"domains: ["*.example.test"]"#);
        let err = Config::from_bytes(raw.as_bytes()).expect_err("wildcard domain should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domains[0]") && msg.contains("wildcard"),
            "error should mention wildcard rejection, got: {msg}"
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
    fn rejects_non_ascii_domain() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("domains: [example.test]", "domains: [münchen.example]");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("non-ASCII domain should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domains[0]") && msg.contains("non-ASCII") && msg.contains("A-label"),
            "error should mention non-ASCII and A-label workaround, got: {msg}"
        );
    }

    #[test]
    fn accepts_already_punycoded_domain() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory").replace(
            "domains: [example.test]",
            "domains: [xn--mnchen-3ya.example]",
        );
        Config::from_bytes(raw.as_bytes())
            .expect("punycoded A-label domain should parse successfully");
    }

    #[test]
    fn rejects_mixed_unicode_in_subdomain() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("domains: [example.test]", "domains: [api.müller.test]");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("unicode subdomain should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domains[0]") && msg.contains("non-ASCII"),
            "error should mention domains[0] and non-ASCII, got: {msg}"
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

    // ── HTTPS scheme enforcement ──────────────────────────────────────────────

    // staging + http override → rejected (staging never allows http)
    #[test]
    fn staging_profile_http_uri_rejected() {
        let raw = base_yaml(&format!(
            "directory_profile: staging\n  directory_uri: {LE_STAGING_URL}"
        ))
        .replace("https://", "http://");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("http staging URI should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("https") || msg.contains("staging"),
            "error should mention https or staging, got: {msg}"
        );
    }

    // production + http override → rejected
    #[test]
    fn production_profile_http_uri_rejected() {
        let raw = base_yaml(&format!(
            "directory_profile: production\n  directory_uri: {LE_PRODUCTION_URL}"
        ))
        .replace("https://", "http://");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("http production URI should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("https") || msg.contains("production"),
            "error should mention https or production, got: {msg}"
        );
    }

    // custom + http without opt-in → rejected
    #[test]
    fn custom_profile_http_uri_without_opt_in_rejected() {
        let raw =
            base_yaml("directory_profile: custom\n  directory_uri: http://internal-acme.test/dir");
        let err = Config::from_bytes(raw.as_bytes())
            .expect_err("http custom URI without opt-in should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("allow_insecure_directory"),
            "error should mention allow_insecure_directory, got: {msg}"
        );
    }

    // custom + http + allow_insecure_directory: true → accepted
    #[test]
    fn custom_profile_http_uri_with_opt_in_accepted() {
        let raw = base_yaml(
            "directory_profile: custom\n  directory_uri: http://internal-acme.test/dir\n  allow_insecure_directory: true",
        );
        let cfg = Config::from_bytes(raw.as_bytes())
            .expect("http custom URI with opt-in should be accepted");
        assert_eq!(cfg.acme.directory_uri, "http://internal-acme.test/dir");
    }

    // custom + https → accepted (no warn needed, standard path)
    #[test]
    fn custom_profile_https_uri_accepted() {
        let raw = base_yaml("directory_profile: custom\n  directory_uri: https://pebble:14000/dir");
        let cfg = Config::from_bytes(raw.as_bytes()).expect("https custom URI should be accepted");
        assert_eq!(cfg.acme.directory_uri, "https://pebble:14000/dir");
    }

    // no profile + http → rejected (not custom, so no opt-in path)
    #[test]
    fn no_profile_http_uri_rejected() {
        let raw = base_yaml("directory_uri: http://example.invalid/directory");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("http no-profile URI should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("https") && msg.contains("unset"),
            "error should mention https and the unset profile, got: {msg}"
        );
    }

    // unknown scheme → rejected
    #[test]
    fn unknown_scheme_rejected() {
        let raw = base_yaml("directory_uri: ftp://example.invalid/directory");
        let err = Config::from_bytes(raw.as_bytes()).expect_err("ftp URI should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("https") || msg.contains("ftp"),
            "error should mention https or the bad scheme, got: {msg}"
        );
    }

    // ── issuance_timeout_seconds validation ──────────────────────────────────

    #[test]
    fn issuance_timeout_defaults_to_120() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory");
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(cfg.acme.issuance_timeout_seconds, 120);
    }

    #[test]
    fn issuance_timeout_zero_rejected() {
        let raw = base_yaml(
            "directory_uri: https://example.invalid/directory\n  issuance_timeout_seconds: 0",
        );
        let err =
            Config::from_bytes(raw.as_bytes()).expect_err("issuance_timeout_seconds=0 should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("issuance_timeout_seconds") && msg.contains("between 5 and 600"),
            "error should mention issuance_timeout_seconds range, got: {msg}"
        );
    }

    #[test]
    fn issuance_timeout_too_large_rejected() {
        let raw = base_yaml(
            "directory_uri: https://example.invalid/directory\n  issuance_timeout_seconds: 601",
        );
        let err = Config::from_bytes(raw.as_bytes())
            .expect_err("issuance_timeout_seconds=601 should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("issuance_timeout_seconds") && msg.contains("between 5 and 600"),
            "error should mention issuance_timeout_seconds range, got: {msg}"
        );
    }

    #[test]
    fn issuance_timeout_valid_value_accepted() {
        let raw = base_yaml(
            "directory_uri: https://example.invalid/directory\n  issuance_timeout_seconds: 300",
        );
        let cfg = Config::from_bytes(raw.as_bytes()).expect("yaml parse");
        assert_eq!(cfg.acme.issuance_timeout_seconds, 300);
    }

    // ── Serialize for AcmeConfig ──────────────────────────────────────────────

    fn make_acme_config(profile: Option<DirectoryProfile>, uri: &str) -> AcmeConfig {
        AcmeConfig {
            directory_profile: profile,
            directory_uri: uri.to_string(),
            directory_ca_file: None,
            contact: "mailto:test@example.test".to_string(),
            domains: vec!["example.test".to_string()],
            renewal_window_days: 30,
            state_dir: std::path::PathBuf::from("/tmp/acme"),
            cert_sink: CertSinkConfig {
                sink_type: "filesystem".to_string(),
                cert_dir: std::path::PathBuf::from("/tmp/certs"),
                layout: Layout::PerDomain,
            },
            tick_seconds: 60,
            issuance_timeout_seconds: 120,
        }
    }

    #[test]
    fn serialize_staging_profile_omits_directory_uri() {
        let cfg = make_acme_config(Some(DirectoryProfile::Staging), LE_STAGING_URL);
        let val = serde_json::to_value(&cfg).expect("serialize");
        assert_eq!(val["directory_profile"], "staging");
        assert!(
            val.get("directory_uri").is_none(),
            "staging must not emit directory_uri"
        );
    }

    #[test]
    fn serialize_production_profile_omits_directory_uri() {
        let cfg = make_acme_config(Some(DirectoryProfile::Production), LE_PRODUCTION_URL);
        let val = serde_json::to_value(&cfg).expect("serialize");
        assert_eq!(val["directory_profile"], "production");
        assert!(
            val.get("directory_uri").is_none(),
            "production must not emit directory_uri"
        );
    }

    #[test]
    fn serialize_custom_profile_emits_directory_uri() {
        let uri = "https://pebble:14000/dir";
        let cfg = make_acme_config(Some(DirectoryProfile::Custom), uri);
        let val = serde_json::to_value(&cfg).expect("serialize");
        assert_eq!(val["directory_profile"], "custom");
        assert_eq!(val["directory_uri"], uri);
    }

    #[test]
    fn serialize_no_profile_emits_directory_uri() {
        let uri = "https://acme.example.invalid/directory";
        let cfg = make_acme_config(None, uri);
        let val = serde_json::to_value(&cfg).expect("serialize");
        assert!(
            val.get("directory_profile").is_none(),
            "absent profile must not emit directory_profile key"
        );
        assert_eq!(val["directory_uri"], uri);
    }

    #[test]
    fn serialize_always_emits_common_fields() {
        let cfg = make_acme_config(None, "https://acme.example.invalid/directory");
        let val = serde_json::to_value(&cfg).expect("serialize");
        assert!(val.get("contact").is_some());
        assert!(val.get("domains").is_some());
        assert!(val.get("renewal_window_days").is_some());
        assert!(val.get("state_dir").is_some());
        assert!(val.get("cert_sink").is_some());
        assert!(val.get("tick_seconds").is_some());
        assert!(val.get("issuance_timeout_seconds").is_some());
        assert!(val.get("directory_ca_file").is_some()); // serializes as null when None
    }

    // ── Config::from_bytes JSON path ──────────────────────────────────────────

    #[test]
    fn from_bytes_parses_valid_json() {
        let json = serde_json::json!({
            "acme": {
                "directory_uri": "https://acme.example.invalid/directory",
                "contact": "mailto:test@example.test",
                "domains": ["example.test"],
                "state_dir": "/tmp/acme",
                "cert_sink": {
                    "type": "filesystem",
                    "cert_dir": "/tmp/certs"
                }
            }
        })
        .to_string();

        let cfg = Config::from_bytes(json.as_bytes()).expect("json parse");
        assert_eq!(
            cfg.acme.directory_uri,
            "https://acme.example.invalid/directory"
        );
        assert_eq!(cfg.acme.domains, vec!["example.test".to_string()]);
    }

    // ── LogConfig::default ────────────────────────────────────────────────────

    #[test]
    fn log_config_default_level_is_info() {
        let log = LogConfig::default();
        assert_eq!(log.level, "info");
    }

    // ── deny_unknown_fields rejection ────────────────────────────────────────

    // Typo in a top-level acme field (directoty_uri instead of directory_uri)
    #[test]
    fn rejects_unknown_acme_field_typo() {
        let raw = base_yaml("directoty_uri: https://example.invalid/directory");
        let err =
            Config::from_bytes(raw.as_bytes()).expect_err("typo'd acme field should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("directoty_uri"),
            "error should name the unknown field, got: {msg}"
        );
    }

    // Typo in cert_sink field (celr_dir instead of cert_dir)
    #[test]
    fn rejects_unknown_cert_sink_field_typo() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory")
            .replace("cert_dir: /tmp/certs", "celr_dir: /tmp/certs");
        let err = Config::from_bytes(raw.as_bytes())
            .expect_err("typo'd cert_sink field should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("celr_dir"),
            "error should name the unknown field, got: {msg}"
        );
    }

    // Unknown top-level key alongside acme (aceme: instead of acme:)
    #[test]
    fn rejects_unknown_top_level_key() {
        let raw = r#"
acme:
  directory_uri: https://example.invalid/directory
  contact: mailto:admin@example.test
  domains: [example.test]
  state_dir: /tmp/acme
  cert_sink:
    type: filesystem
    cert_dir: /tmp/certs
aceme:
  directory_uri: https://example.invalid/directory
"#;
        let err = Config::from_bytes(raw.as_bytes())
            .expect_err("unknown top-level key should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("aceme"),
            "error should name the unknown field, got: {msg}"
        );
    }

    // Unknown key inside cert_sink alongside valid keys
    #[test]
    fn rejects_unknown_cert_sink_extra_key() {
        let raw = base_yaml("directory_uri: https://example.invalid/directory").replace(
            "cert_dir: /tmp/certs",
            "cert_dir: /tmp/certs\n    s3_bucket: my-bucket",
        );
        let err = Config::from_bytes(raw.as_bytes())
            .expect_err("unknown cert_sink key should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("s3_bucket"),
            "error should name the unknown field, got: {msg}"
        );
    }
}
