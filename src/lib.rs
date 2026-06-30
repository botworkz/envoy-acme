// This file is excluded from cargo tarpaulin coverage; see CONTRIBUTING.md
// section "Coverage exclusions" for the policy. The functions below are
// FFI entry points loaded by Envoy and cannot be invoked from a Rust
// unit test.
//! Envoy dynamic module that obtains and renews TLS certificates via the ACME
//! protocol (RFC 8555), serving HTTP-01 challenges from the same Envoy process
//! and publishing issued certificates to a cert sink (currently filesystem).
//!
//! The `AcmeStateMachine` drives the renewal loop; `AcmeBootstrapConfig` is the
//! entry point that Envoy calls at startup.
#![deny(unsafe_code)]
#![deny(missing_docs)]
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
mod state_lock;

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

// `declare_all_init_functions!` generates a `pub extern "C"` entry-point
// function whose doc comment cannot be supplied from outside the macro.
// The submodule below provides a scoped `#![allow(missing_docs)]` so the
// generated function does not trigger the crate-level deny, while keeping
// the lint active for all hand-written `pub` items.
mod _init {
    #![allow(missing_docs)]

    use super::{new_bootstrap_extension_config, new_http_filter_config, program_init};
    use envoy_proxy_dynamic_modules_rust_sdk::*;

    declare_all_init_functions!(
      program_init,
      bootstrap: new_bootstrap_extension_config,
      http: new_http_filter_config,
    );
}

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
    config: &[u8],
) -> Option<Box<dyn http::HttpFilterConfig<EHF>>> {
    Some(Box::new(AcmeHttpFilterConfig::from_bytes(config)))
}
