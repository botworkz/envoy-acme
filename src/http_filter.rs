use std::collections::HashSet;
use std::sync::Arc;

use envoy_proxy_dynamic_modules_rust_sdk::abi;
use envoy_proxy_dynamic_modules_rust_sdk::http::{EnvoyHttpFilter, HttpFilter, HttpFilterConfig};

use crate::challenge_store;

const PREFIX: &[u8] = b"/.well-known/acme-challenge/";
const CONTENT_TYPE: &[u8] = b"application/octet-stream";
const NOT_FOUND: &[u8] = b"acme challenge not found";
const MAX_TOKEN_LEN: usize = 256;

pub struct AcmeHttpFilterConfig {
    domains: Arc<HashSet<String>>,
}

impl AcmeHttpFilterConfig {
    /// Construct from raw config bytes (JSON or YAML).
    ///
    /// The expected format is a mapping with a single `domains` list, e.g.:
    /// ```yaml
    /// domains:
    ///   - example.com
    /// ```
    /// If the bytes are empty or cannot be parsed, an empty domain set is
    /// used, which causes the filter to fall through on every request.
    pub fn from_bytes(raw: &[u8]) -> Self {
        #[derive(serde::Deserialize, Default)]
        struct RawCfg {
            #[serde(default)]
            domains: Vec<String>,
        }
        let cfg: RawCfg = serde_json::from_slice(raw)
            .or_else(|_| serde_yaml::from_slice(raw))
            .unwrap_or_default();
        Self::new(cfg.domains)
    }

    /// Construct from a pre-validated domain list.
    ///
    /// Domains are lower-cased at construction time so that per-request
    /// comparisons are a single `HashSet` lookup against a normalised key.
    pub fn new(domains: Vec<String>) -> Self {
        Self {
            domains: Arc::new(domains.into_iter().map(|d| d.to_lowercase()).collect()),
        }
    }
}

impl<EHF: EnvoyHttpFilter> HttpFilterConfig<EHF> for AcmeHttpFilterConfig {
    fn new_http_filter(&self, _envoy: &mut EHF) -> Box<dyn HttpFilter<EHF>> {
        Box::new(AcmeHttpFilter {
            domains: Arc::clone(&self.domains),
        })
    }
}

struct AcmeHttpFilter {
    domains: Arc<HashSet<String>>,
}

impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for AcmeHttpFilter {
    fn on_request_headers(
        &mut self,
        envoy: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_request_headers_status {
        let Some(path) = envoy.get_request_header_value(":path") else {
            return abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue;
        };

        let bytes = path.as_slice();
        if !bytes.starts_with(PREFIX) {
            return abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue;
        }

        let Some(token) = extract_token(bytes) else {
            return abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue;
        };

        // Envoy normalises the HTTP/1.1 `Host` header into `:authority` for
        // both HTTP/1 and HTTP/2 connections, so querying `:authority` alone
        // covers both protocol versions.
        let Some(authority) = envoy.get_request_header_value(":authority") else {
            tracing::debug!("acme: missing :authority header, falling through");
            return abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue;
        };
        let host = normalize_host(authority.as_slice());
        if !self.domains.contains(&host) {
            tracing::debug!(
                host = %host,
                "acme: host not in configured domains, falling through"
            );
            return abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue;
        }

        if let Some(key_authorization) = challenge_store::lookup(token) {
            envoy.send_response(
                200,
                &[("content-type", CONTENT_TYPE)],
                Some(key_authorization.as_bytes()),
                Some("acme_challenge_hit"),
            );
            return abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration;
        }

        envoy.send_response(
            404,
            &[("content-type", CONTENT_TYPE)],
            Some(NOT_FOUND),
            Some("acme_challenge_not_found"),
        );
        abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
    }
}

/// Normalise a `Host` / `:authority` header value for domain matching.
///
/// - Converts to lowercase.
/// - Strips a trailing port (`:8080`) when the suffix after the last `:` is
///   all ASCII digits.  IPv6 addresses wrapped in brackets are left unchanged
///   because `rfind(':')` would point inside the bracket group, not at the
///   optional port separator.
fn normalize_host(host: &[u8]) -> String {
    let s = String::from_utf8_lossy(host).to_lowercase();
    if !s.starts_with('[') {
        if let Some(colon) = s.rfind(':') {
            if s[colon + 1..].bytes().all(|b| b.is_ascii_digit()) {
                return s[..colon].to_string();
            }
        }
    }
    s
}

fn extract_token(path: &[u8]) -> Option<&str> {
    let token_bytes = path.get(PREFIX.len()..)?;
    if token_bytes.is_empty() || token_bytes.len() > MAX_TOKEN_LEN {
        return None;
    }
    if token_bytes.contains(&b'/') {
        return None;
    }
    std::str::from_utf8(token_bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use envoy_proxy_dynamic_modules_rust_sdk::{http::MockEnvoyHttpFilter, EnvoyBuffer};
    use mockall::predicate::eq;

    fn make_filter(domains: &[&str]) -> AcmeHttpFilter {
        AcmeHttpFilter {
            domains: Arc::new(domains.iter().map(|d| d.to_string()).collect()),
        }
    }

    // ── normalize_host ────────────────────────────────────────────────────────

    #[test]
    fn normalize_host_strips_port() {
        assert_eq!(normalize_host(b"example.com:8080"), "example.com");
        assert_eq!(normalize_host(b"example.com:80"), "example.com");
    }

    #[test]
    fn normalize_host_lowercases() {
        assert_eq!(normalize_host(b"A.Example.TEST"), "a.example.test");
    }

    #[test]
    fn normalize_host_strips_port_and_lowercases() {
        assert_eq!(normalize_host(b"A.Example.Test:8080"), "a.example.test");
    }

    #[test]
    fn normalize_host_no_port() {
        assert_eq!(normalize_host(b"example.com"), "example.com");
    }

    // ── on_request_headers ────────────────────────────────────────────────────

    #[test]
    fn continue_on_non_challenge_path() {
        let mut filter = make_filter(&[]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/health")));

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
    }

    #[test]
    fn respond_200_on_challenge_hit() {
        challenge_store::insert("token-1".to_string(), "key-auth-1".to_string());

        let mut filter = make_filter(&["a.example.test"]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/.well-known/acme-challenge/token-1")));

        envoy
            .expect_get_request_header_value()
            .with(eq(":authority"))
            .returning(|_| Some(EnvoyBuffer::new(b"a.example.test")));

        envoy.expect_send_response().returning(|_, _, _, _| {});

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        );

        challenge_store::remove("token-1");
    }

    #[test]
    fn continue_on_unrecognized_host() {
        challenge_store::insert("token-2".to_string(), "key-auth-2".to_string());

        let mut filter = make_filter(&["a.example.test"]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/.well-known/acme-challenge/token-2")));

        envoy
            .expect_get_request_header_value()
            .with(eq(":authority"))
            .returning(|_| Some(EnvoyBuffer::new(b"other.example.invalid")));

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );

        challenge_store::remove("token-2");
    }

    #[test]
    fn respond_200_on_host_with_port_and_mixed_case() {
        challenge_store::insert("token-3".to_string(), "key-auth-3".to_string());

        let mut filter = make_filter(&["a.example.test"]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/.well-known/acme-challenge/token-3")));

        envoy
            .expect_get_request_header_value()
            .with(eq(":authority"))
            .returning(|_| Some(EnvoyBuffer::new(b"A.Example.Test:8080")));

        envoy.expect_send_response().returning(|_, _, _, _| {});

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        );

        challenge_store::remove("token-3");
    }

    #[test]
    fn respond_404_on_unknown_token_with_matching_host() {
        let mut filter = make_filter(&["a.example.test"]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| {
                Some(EnvoyBuffer::new(
                    b"/.well-known/acme-challenge/no-such-token",
                ))
            });

        envoy
            .expect_get_request_header_value()
            .with(eq(":authority"))
            .returning(|_| Some(EnvoyBuffer::new(b"a.example.test")));

        envoy.expect_send_response().returning(|_, _, _, _| {});

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        );
    }
}
