use std::sync::Arc;

use envoy_proxy_dynamic_modules_rust_sdk::bootstrap::EnvoyBootstrapExtensionTimer;
use envoy_proxy_dynamic_modules_rust_sdk::{
    abi, BootstrapExtension, BootstrapExtensionConfig, CompletionCallback, EnvoyBootstrapExtension,
    EnvoyBootstrapExtensionConfig,
};
use parking_lot::Mutex;
use rand::Rng;

use crate::acme::{inspect_state, StateSummary};
use crate::config::Config;
use crate::errors::RuntimeError;
use crate::runtime::RuntimeBridge;

pub struct AcmeBootstrapConfig {
    runtime: RuntimeBridge,
    domains: Vec<String>,
    state_dir: std::path::PathBuf,
    renewal_window_days: u64,
    tick_seconds: u64,
    _timer: Arc<Mutex<Option<Box<dyn EnvoyBootstrapExtensionTimer>>>>,
}

impl AcmeBootstrapConfig {
    pub fn new(
        envoy_config: &mut dyn EnvoyBootstrapExtensionConfig,
        config: Config,
    ) -> Result<Self, RuntimeError> {
        let domains = config.acme.domains.clone();
        let state_dir = config.acme.state_dir.clone();
        let renewal_window_days = config.acme.renewal_window_days;
        let tick_seconds = config.acme.tick_seconds;
        let runtime = RuntimeBridge::new(config);
        let timer = envoy_config.new_timer();
        timer.enable(std::time::Duration::from_secs(tick_seconds));
        envoy_config.signal_init_complete();

        Ok(Self {
            runtime,
            domains,
            state_dir,
            renewal_window_days,
            tick_seconds,
            _timer: Arc::new(Mutex::new(Some(timer))),
        })
    }
}

impl BootstrapExtensionConfig for AcmeBootstrapConfig {
    fn new_bootstrap_extension(
        &self,
        _envoy_extension: &mut dyn EnvoyBootstrapExtension,
    ) -> Box<dyn BootstrapExtension> {
        Box::new(AcmeBootstrapExtension {
            runtime: self.runtime.clone(),
            domains: self.domains.clone(),
            state_dir: self.state_dir.clone(),
            renewal_window_days: self.renewal_window_days,
        })
    }

    fn on_timer_fired(
        &self,
        _envoy_extension_config: &mut dyn EnvoyBootstrapExtensionConfig,
        timer: &dyn EnvoyBootstrapExtensionTimer,
    ) {
        if let Err(e) = self.runtime.tick() {
            envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!("envoy-acme: tick failed: {e}");
        }
        // Add ±10 % random jitter to the next tick interval so that multiple
        // instances that started at the same moment don't stay in lockstep.
        let jitter: f64 = rand::thread_rng().gen_range(0.9..=1.1);
        let jittered = ((self.tick_seconds as f64) * jitter) as u64;
        timer.enable(std::time::Duration::from_secs(jittered));
    }

    fn on_http_callout_done(
        &self,
        _envoy_extension_config: &mut dyn EnvoyBootstrapExtensionConfig,
        _callout_id: u64,
        _result: abi::envoy_dynamic_module_type_http_callout_result,
        _response_headers: Option<
            &[(
                envoy_proxy_dynamic_modules_rust_sdk::EnvoyBuffer,
                envoy_proxy_dynamic_modules_rust_sdk::EnvoyBuffer,
            )],
        >,
        _response_body: Option<&[envoy_proxy_dynamic_modules_rust_sdk::EnvoyBuffer]>,
    ) {
    }
}

pub struct AcmeBootstrapExtension {
    runtime: RuntimeBridge,
    domains: Vec<String>,
    state_dir: std::path::PathBuf,
    renewal_window_days: u64,
}

impl BootstrapExtension for AcmeBootstrapExtension {
    fn on_server_initialized(&mut self, _envoy_extension: &mut dyn EnvoyBootstrapExtension) {
        for domain in &self.domains {
            match inspect_state(&self.state_dir, domain, self.renewal_window_days) {
                StateSummary::NoCertCached => {
                    tracing::info!(
                        domain = %domain,
                        state = "no-cached-cert",
                        "envoy-acme startup: no cached certificate; will issue at first tick"
                    );
                }
                StateSummary::CertCached {
                    not_after_unix,
                    days_until_renewal,
                } => {
                    tracing::info!(
                        domain = %domain,
                        state = "cached-cert",
                        not_after_unix,
                        days_until_renewal,
                        "envoy-acme startup: cached certificate present"
                    );
                }
                StateSummary::CertCachedButInvalid { reason } => {
                    tracing::warn!(
                        domain = %domain,
                        state = "cached-cert-invalid",
                        reason = %reason,
                        "envoy-acme startup: cached certificate present but unusable; will re-issue"
                    );
                }
            }
        }

        if let Err(e) = self.runtime.start() {
            envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                "envoy-acme: on_server_initialized start failed: {e}"
            );
        }
    }

    fn on_drain_started(&mut self, _envoy_extension: &mut dyn EnvoyBootstrapExtension) {
        if let Err(e) = self.runtime.shutdown() {
            envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                "envoy-acme: drain shutdown failed: {e}"
            );
        }
    }

    fn on_shutdown(
        &mut self,
        _envoy_extension: &mut dyn EnvoyBootstrapExtension,
        completion: CompletionCallback,
    ) {
        let _ = self.runtime.shutdown();
        completion.done();
    }
}
