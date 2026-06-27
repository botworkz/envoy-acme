#![allow(clippy::incompatible_msrv)]
mod acme;
mod bootstrap;
mod cert_sink;
mod challenge_store;
mod config;
mod errors;
mod http_filter;
mod runtime;

use envoy_proxy_dynamic_modules_rust_sdk::*;

use crate::bootstrap::AcmeBootstrapConfig;
use crate::http_filter::AcmeHttpFilterConfig;

fn program_init() -> bool {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    true
}

declare_all_init_functions!(
  program_init,
  bootstrap: new_bootstrap_extension_config,
  http: new_http_filter_config,
);

fn new_bootstrap_extension_config(
    envoy_config: &mut dyn EnvoyBootstrapExtensionConfig,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn BootstrapExtensionConfig>> {
    let cfg = match crate::config::Config::from_bytes(config) {
        Ok(c) => c,
        Err(e) => {
            envoy_log_error!("envoy-acme: invalid bootstrap config: {e}");
            return None;
        }
    };

    crate::challenge_store::init();

    match AcmeBootstrapConfig::new(envoy_config, cfg) {
        Ok(v) => Some(Box::new(v)),
        Err(e) => {
            envoy_log_error!("envoy-acme: failed to initialize bootstrap config: {e}");
            None
        }
    }
}

fn new_http_filter_config<EC: http::EnvoyHttpFilterConfig, EHF: http::EnvoyHttpFilter>(
    _envoy: &mut EC,
    _name: &str,
    _config: &[u8],
) -> Option<Box<dyn http::HttpFilterConfig<EHF>>> {
    Some(Box::new(AcmeHttpFilterConfig))
}
