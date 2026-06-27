use envoy_proxy_dynamic_modules_rust_sdk::abi;
use envoy_proxy_dynamic_modules_rust_sdk::http::{EnvoyHttpFilter, HttpFilter, HttpFilterConfig};

use crate::challenge_store;

const PREFIX: &[u8] = b"/.well-known/acme-challenge/";
const CONTENT_TYPE: &[u8] = b"application/octet-stream";
const NOT_FOUND: &[u8] = b"acme challenge not found";
const MAX_TOKEN_LEN: usize = 256;

pub struct AcmeHttpFilterConfig;

impl<EHF: EnvoyHttpFilter> HttpFilterConfig<EHF> for AcmeHttpFilterConfig {
    fn new_http_filter(&self, _envoy: &mut EHF) -> Box<dyn HttpFilter<EHF>> {
        Box::new(AcmeHttpFilter)
    }
}

struct AcmeHttpFilter;

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

    #[test]
    fn continue_on_non_challenge_path() {
        let mut filter = AcmeHttpFilter;
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

        let mut filter = AcmeHttpFilter;
        let mut envoy = MockEnvoyHttpFilter::new();

        envoy
            .expect_get_request_header_value()
            .with(eq(":path"))
            .returning(|_| Some(EnvoyBuffer::new(b"/.well-known/acme-challenge/token-1")));

        envoy.expect_send_response().returning(|_, _, _, _| {});

        let status = filter.on_request_headers(&mut envoy, true);
        assert_eq!(
            status,
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        );

        challenge_store::remove("token-1");
    }
}
