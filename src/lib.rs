#![allow(clippy::incompatible_msrv)]
// The `declare_all_init_functions!` macro from envoy-proxy-dynamic-modules-rust-sdk
// performs function-pointer comparisons internally for duplicate-registration detection;
// the lint cannot be suppressed per-invocation because #[allow] on a macro call is
// ignored for code generated inside the macro.
#![allow(unpredictable_function_pointer_comparisons)]
mod acme;
mod atomic_write;
mod bootstrap;
mod cert_sink;
mod challenge_store;
mod config;
mod errors;
mod http_filter;
mod metrics;
mod runtime;

use envoy_proxy_dynamic_modules_rust_sdk::*;

use crate::bootstrap::AcmeBootstrapConfig;
use crate::http_filter::AcmeHttpFilterConfig;

fn program_init() -> bool {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // rustls 0.23 requires a process-wide CryptoProvider to be installed
    // before any ClientConfig::builder() runs. Without this the first
    // HTTPS call from the ACME state machine panics on the tokio
    // runtime thread, which unwinds block_on, drops the channel
    // receiver, and leaves every subsequent tick logging
    // "runtime thread already stopped". install_default returns Err if
    // a provider is already installed; ignore that.
    let _ = rustls::crypto::ring::default_provider().install_default();

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
