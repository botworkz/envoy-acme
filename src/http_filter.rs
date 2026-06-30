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
/// - For plain hostnames / IPv4: strips a trailing port (`:8080`) when the
///   suffix after the last `:` is all ASCII digits.
/// - For IPv6 addresses in brackets: strips a port after the closing bracket
///   (e.g. `[::1]:8080` → `[::1]`).  A malformed value without a closing
///   bracket is returned unchanged (it will not match any configured domain).
fn normalize_host(host: &[u8]) -> String {
    let s = String::from_utf8_lossy(host).to_lowercase();
    if s.starts_with('[') {
        // IPv6 literal: strip port that follows the closing ']', if present.
        if let Some(bracket_end) = s.find(']') {
            let after = &s[bracket_end + 1..];
            if after.starts_with(':') && after[1..].bytes().all(|b| b.is_ascii_digit()) {
                return s[..=bracket_end].to_string();
            }
        }
    } else if let Some(colon) = s.rfind(':') {
        if s[colon + 1..].bytes().all(|b| b.is_ascii_digit()) {
            return s[..colon].to_string();
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
            domains: Arc::new(domains.iter().map(|d| d.to_lowercase()).collect()),
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

    #[test]
    fn normalize_host_ipv6_strips_port() {
        assert_eq!(normalize_host(b"[::1]:8080"), "[::1]");
        assert_eq!(normalize_host(b"[::1]"), "[::1]");
    }

    #[test]
    fn normalize_host_ipv6_malformed_no_change() {
        // Missing closing bracket — returned unchanged (will not match any domain)
        assert_eq!(normalize_host(b"[::1:8080"), "[::1:8080");
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

        envoy
            .expect_send_response()
            .withf(|status, headers, body, details| {
                *status == 200
                    && headers.len() == 1
                    && headers[0].0 == "content-type"
                    && headers[0].1 == CONTENT_TYPE
                    && *body == Some(b"key-auth-1".as_slice())
                    && *details == Some("acme_challenge_hit")
            })
            .times(1)
            .returning(|_, _, _, _| {});

        // `end_of_stream` is ignored by the implementation; passing `false`
        // here confirms the challenge-hit path is unchanged.
        let status = filter.on_request_headers(&mut envoy, false);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        );

        challenge_store::remove("token-1");
    }

    #[test]
    fn config_factory_creates_filter() {
        let config = AcmeHttpFilterConfig::new(vec![]);
        let mut envoy = MockEnvoyHttpFilter::new();
        let mut filter = config.new_http_filter(&mut envoy);

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| None);

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
    }

    #[test]
    fn continue_when_path_header_missing() {
        let mut filter = make_filter(&[]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| None);

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
    }

    #[test]
    fn continue_when_challenge_path_has_empty_token() {
        let mut filter = make_filter(&[]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/.well-known/acme-challenge/")));

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
    }

    #[test]
    fn continue_when_challenge_path_token_contains_slash() {
        let mut filter = make_filter(&[]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/.well-known/acme-challenge/abc/def")));

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
    }

    #[test]
    fn continue_when_challenge_path_token_exceeds_max_len() {
        let mut filter = make_filter(&[]);
        let mut envoy = MockEnvoyHttpFilter::new();
        let long_token = "a".repeat(MAX_TOKEN_LEN + 1);
        let path = format!("/.well-known/acme-challenge/{long_token}");

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(move |_| Some(EnvoyBuffer::new(path.as_bytes())));

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
    }

    #[test]
    fn continue_when_challenge_path_token_is_non_utf8() {
        let mut filter = make_filter(&[]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/.well-known/acme-challenge/\xff")));

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
    }

    #[test]
    fn respond_404_on_challenge_miss() {
        let token = "miss-token-abc-123";
        let path = format!("/.well-known/acme-challenge/{token}");

        let mut filter = make_filter(&["a.example.test"]);
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(move |_| Some(EnvoyBuffer::new(path.as_bytes())));

        envoy
            .expect_get_request_header_value()
            .with(eq(":authority"))
            .returning(|_| Some(EnvoyBuffer::new(b"a.example.test")));

        envoy
            .expect_send_response()
            .withf(|status, headers, body, details| {
                *status == 404
                    && headers.len() == 1
                    && headers[0].0 == "content-type"
                    && headers[0].1 == CONTENT_TYPE
                    && *body == Some(NOT_FOUND)
                    && *details == Some("acme_challenge_not_found")
            })
            .times(1)
            .returning(|_, _, _, _| {});

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        );
        challenge_store::remove(token);
    }

    #[test]
    fn extract_token_validation_cases() {
        assert_eq!(
            extract_token(b"/.well-known/acme-challenge/token-1"),
            Some("token-1")
        );
        assert_eq!(extract_token(PREFIX), None);
        assert_eq!(extract_token(b"/.well-known/acme-challenge/abc/def"), None);
        assert_eq!(
            extract_token(
                format!(
                    "{}{}",
                    std::str::from_utf8(PREFIX).expect("prefix utf8"),
                    "a".repeat(MAX_TOKEN_LEN + 1)
                )
                .as_bytes()
            ),
            None
        );
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

        envoy
            .expect_send_response()
            .withf(|status, headers, body, details| {
                *status == 200
                    && headers.len() == 1
                    && headers[0].0 == "content-type"
                    && headers[0].1 == CONTENT_TYPE
                    && *body == Some(b"key-auth-3".as_slice())
                    && *details == Some("acme_challenge_hit")
            })
            .times(1)
            .returning(|_, _, _, _| {});

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        );

        challenge_store::remove("token-3");
    }
}
