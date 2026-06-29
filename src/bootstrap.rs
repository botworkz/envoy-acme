use std::sync::Arc;
use std::{fs, path::Path};

use envoy_proxy_dynamic_modules_rust_sdk::bootstrap::EnvoyBootstrapExtensionTimer;
use envoy_proxy_dynamic_modules_rust_sdk::{
    abi, BootstrapExtension, BootstrapExtensionConfig, CompletionCallback, EnvoyBootstrapExtension,
    EnvoyBootstrapExtensionConfig,
};
use parking_lot::Mutex;
use rand::Rng;

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
        for (dir, label) in [
            (&config.acme.state_dir, "acme.state_dir"),
            (&config.acme.cert_sink.cert_dir, "acme.cert_sink.cert_dir"),
        ] {
            if let Err(e) = probe_writable(dir, label) {
                envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!("envoy-acme: {e}");
                return Err(RuntimeError::Bootstrap(e));
            }
        }

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

pub(crate) fn probe_writable(dir: &Path, label: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{label} {dir:?} cannot be created: {e}"))?;
    let probe = dir.join(".envoy_acme_probe");
    fs::write(&probe, b"").map_err(|e| format!("{label} {dir:?} is not writable: {e}"))?;
    let _ = fs::remove_file(&probe);
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::probe_writable;

    #[test]
    fn probe_writable_accepts_writable_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let inside = tmp.path().join("subdir");
        probe_writable(&inside, "test").expect("should succeed");
    }

    #[cfg(unix)]
    #[test]
    fn probe_writable_rejects_readonly_dir() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        let readonly = tmp.path().join("readonly");
        std::fs::create_dir_all(&readonly).expect("create readonly dir");
        std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o555))
            .expect("set readonly perms");

        let inside = readonly.join("subdir");
        let err = probe_writable(&inside, "test").expect_err("should fail");
        assert!(err.contains("test") && err.contains(&format!("{inside:?}")));

        std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o755))
            .expect("restore perms");
    }
}
