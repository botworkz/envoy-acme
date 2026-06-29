use std::sync::Arc;

use envoy_proxy_dynamic_modules_rust_sdk::bootstrap::EnvoyBootstrapExtensionTimer;
use envoy_proxy_dynamic_modules_rust_sdk::{
    abi, BootstrapExtension, BootstrapExtensionConfig, CompletionCallback, EnvoyBootstrapExtension,
    EnvoyBootstrapExtensionConfig,
};
use parking_lot::Mutex;

use crate::config::Config;
use crate::errors::RuntimeError;
use crate::runtime::RuntimeBridge;

pub struct AcmeBootstrapConfig {
    runtime: RuntimeBridge,
    tick_seconds: u64,
    _timer: Arc<Mutex<Option<Box<dyn EnvoyBootstrapExtensionTimer>>>>,
}

impl AcmeBootstrapConfig {
    pub fn new(
        envoy_config: &mut dyn EnvoyBootstrapExtensionConfig,
        config: Config,
    ) -> Result<Self, RuntimeError> {
        let tick_seconds = config.acme.tick_seconds;
        let runtime = RuntimeBridge::new(config);
        let timer = envoy_config.new_timer();
        timer.enable(std::time::Duration::from_secs(tick_seconds));
        envoy_config.signal_init_complete();

        Ok(Self {
            runtime,
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
        timer.enable(std::time::Duration::from_secs(self.tick_seconds));
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
}

impl BootstrapExtension for AcmeBootstrapExtension {
    fn on_server_initialized(&mut self, _envoy_extension: &mut dyn EnvoyBootstrapExtension) {
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
