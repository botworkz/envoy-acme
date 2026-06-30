use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fs, path::Path};

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
use crate::metrics;
use crate::runtime::RuntimeBridge;
use crate::state_lock::StateLock;

pub struct AcmeBootstrapConfig {
    runtime: RuntimeBridge,
    domains: Vec<String>,
    state_dir: std::path::PathBuf,
    renewal_window_days: u64,
    tick_seconds: u64,
    last_dead_runtime_log: Arc<Mutex<Option<Instant>>>,
    _timer: Arc<Mutex<Option<Box<dyn EnvoyBootstrapExtensionTimer>>>>,
    #[allow(dead_code)] // kept alive solely to hold the flock
    _state_lock: StateLock,
}

impl AcmeBootstrapConfig {
    pub fn new(
        envoy_config: &mut dyn EnvoyBootstrapExtensionConfig,
        config: Config,
    ) -> Result<Self, RuntimeError> {
        let state_lock = StateLock::acquire(&config.acme.state_dir)?;
        for (dir, label) in [
            (&config.acme.state_dir, "acme.state_dir"),
            (&config.acme.cert_sink.cert_dir, "acme.cert_sink.cert_dir"),
        ] {
            if let Err(e) = probe_writable(dir, label) {
                envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!("envoy-acme: {e}");
                return Err(RuntimeError::Bootstrap(e));
            }
        }

        // `?` works directly here: errors.rs has `impl From<envoy_dynamic_module_type_metrics_result> for RuntimeError`.
        metrics::init(envoy_config)?;
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
            last_dead_runtime_log: Arc::new(Mutex::new(None)),
            _timer: Arc::new(Mutex::new(Some(timer))),
            _state_lock: state_lock,
        })
    }
}

const DEAD_RUNTIME_LOG_MESSAGE: &str =
    "envoy-acme renewal engine is dead; certificates will not renew; restart the proxy to recover. See earlier log lines for the panic cause.";

fn handle_runtime_tick_result<F>(
    tick_result: Result<(), RuntimeError>,
    runtime_alive: bool,
    last_dead_runtime_log: &Mutex<Option<Instant>>,
    now: Instant,
    mut log_error: F,
) where
    F: FnMut(&str),
{
    match tick_result {
        Ok(()) => {
            *last_dead_runtime_log.lock() = None;
        }
        Err(RuntimeError::Stopped) if !runtime_alive => {
            let mut last_log = last_dead_runtime_log.lock();
            let should_log = last_log
                .map(|t| now.duration_since(t) >= Duration::from_secs(60))
                .unwrap_or(true);
            if should_log {
                // TODO(v0.2 #4 follow-up): expose runtime_alive gauge.
                log_error(DEAD_RUNTIME_LOG_MESSAGE);
                *last_log = Some(now);
            }
        }
        Err(e) => {
            log_error(&format!("envoy-acme: tick failed: {e}"));
        }
    }
}

/// Verify that `dir` can be created (if missing) and written to.
///
/// Used at bootstrap to fail fast on permission/disk errors rather than
/// silently failing every issuance attempt at runtime.
///
/// The probe filename includes the PID so that two envoy-acme instances
/// pointed at the same `cert_dir` but different `state_dir`s — a config
/// the state_dir flock cannot prevent — don't collide on the probe file.
pub(crate) fn probe_writable(dir: &Path, label: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{label} {dir:?} cannot be created: {e}"))?;
    let probe = dir.join(format!(".envoy_acme_probe.{}", std::process::id()));
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
        handle_runtime_tick_result(
            self.runtime.tick(),
            self.runtime.is_alive(),
            &self.last_dead_runtime_log,
            Instant::now(),
            |msg| envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!("{msg}"),
        );
        // Add ±10 % random jitter to the next tick interval so that multiple
        // instances that started at the same moment don't stay in lockstep.
        let jitter: f64 = rand::thread_rng().gen_range(0.9..=1.1);
        let jittered = ((self.tick_seconds as f64) * jitter) as u64;
        timer.enable(std::time::Duration::from_secs(jittered));
    }

    fn on_scheduled(
        &self,
        envoy_extension_config: &mut dyn EnvoyBootstrapExtensionConfig,
        event_id: u64,
    ) {
        metrics::on_scheduled(envoy_extension_config, event_id);
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use envoy_proxy_dynamic_modules_rust_sdk::bootstrap::{
        MockEnvoyBootstrapExtensionConfig, MockEnvoyBootstrapExtensionConfigScheduler,
        MockEnvoyBootstrapExtensionTimer,
    };
    use envoy_proxy_dynamic_modules_rust_sdk::{
        EnvoyCounterVecId, EnvoyGaugeVecId, EnvoyHistogramId,
    };
    use tracing_test::traced_test;

    use super::*;
    use crate::config::{AcmeConfig, CertSinkConfig, Config, Layout, LogConfig};
    use crate::metrics;

    // =========================================================================
    // Hand-rolled fake for EnvoyBootstrapExtension.
    // The SDK does NOT annotate this trait with #[automock], so no
    // MockEnvoyBootstrapExtension is generated; we hand-roll a minimal no-op.
    // =========================================================================
    struct FakeEnvoyBootstrapExtension;

    impl EnvoyBootstrapExtension for FakeEnvoyBootstrapExtension {
        fn get_counter_value(&self, _name: &str) -> Option<u64> {
            None
        }
        fn get_gauge_value(&self, _name: &str) -> Option<u64> {
            None
        }
        fn get_histogram_summary(&self, _name: &str) -> Option<(u64, f64)> {
            None
        }
        fn iterate_counters(&self, _callback: &mut dyn FnMut(&str, u64) -> bool) {}
        fn iterate_gauges(&self, _callback: &mut dyn FnMut(&str, u64) -> bool) {}
    }

    // =========================================================================
    // Helpers (per-file copies as per repo convention — no shared helper module)
    // =========================================================================

    /// Build a minimal test `Config`.
    fn test_config(state_dir: &std::path::Path, cert_dir: &std::path::Path) -> Config {
        Config {
            acme: AcmeConfig {
                directory_profile: None,
                directory_uri: "https://acme.invalid/directory".into(),
                directory_ca_file: None,
                contact: "mailto:test@example.test".into(),
                domains: vec!["example.test".into()],
                renewal_window_days: 30,
                state_dir: state_dir.to_path_buf(),
                cert_sink: CertSinkConfig {
                    sink_type: "filesystem".into(),
                    cert_dir: cert_dir.to_path_buf(),
                    layout: Layout::PerDomain,
                },
                tick_seconds: 60,
                issuance_timeout_seconds: 120,
            },
            log: LogConfig::default(),
        }
    }

    /// Build a `MockEnvoyBootstrapExtensionConfig` with the full set of
    /// metrics-init expectations + `new_timer` (returning `timer`) +
    /// `signal_init_complete` (called once).
    ///
    /// Used by tests that call `AcmeBootstrapConfig::new` successfully.
    fn make_bootstrap_mock(
        timer: MockEnvoyBootstrapExtensionTimer,
    ) -> MockEnvoyBootstrapExtensionConfig {
        let mut scheduler = MockEnvoyBootstrapExtensionConfigScheduler::new();
        scheduler.expect_commit().return_const(()); // allow any number of commits from runtime ticks

        let mut envoy_config = MockEnvoyBootstrapExtensionConfig::new();
        envoy_config
            .expect_define_counter_vec()
            .once()
            .withf(|name, labels| name == "envoy_acme_issuance_total" && labels == ["result"])
            .return_once(|_, _| Ok(EnvoyCounterVecId(1)));
        envoy_config
            .expect_define_gauge_vec()
            .once()
            .withf(|name, labels| name == "envoy_acme_consecutive_failures" && labels == ["domain"])
            .return_once(|_, _| Ok(EnvoyGaugeVecId(2)));
        envoy_config
            .expect_define_gauge_vec()
            .once()
            .withf(|name, labels| {
                name == "envoy_acme_next_retry_at_seconds" && labels == ["domain"]
            })
            .return_once(|_, _| Ok(EnvoyGaugeVecId(3)));
        envoy_config
            .expect_define_gauge_vec()
            .once()
            .withf(|name, labels| {
                name == "envoy_acme_cert_not_after_seconds" && labels == ["domain"]
            })
            .return_once(|_, _| Ok(EnvoyGaugeVecId(4)));
        envoy_config
            .expect_define_gauge_vec()
            .once()
            .withf(|name, labels| name == "envoy_acme_account_state" && labels == ["domain"])
            .return_once(|_, _| Ok(EnvoyGaugeVecId(5)));
        envoy_config
            .expect_define_histogram()
            .once()
            .withf(|name| name == "envoy_acme_issuance_duration_seconds")
            .return_once(|_| Ok(EnvoyHistogramId(5)));
        envoy_config
            .expect_new_scheduler()
            .once()
            .return_once(move || Box::new(scheduler));
        envoy_config
            .expect_new_timer()
            .once()
            .return_once(move || Box::new(timer));
        envoy_config
            .expect_signal_init_complete()
            .once()
            .return_const(());

        envoy_config
    }

    /// Construct a happy-path `AcmeBootstrapConfig` against `state_dir`/`cert_dir`.
    /// Caller is responsible for calling `bc.runtime.shutdown()` + `join_for_test()`.
    fn new_config_for_test(
        state_dir: &std::path::Path,
        cert_dir: &std::path::Path,
    ) -> AcmeBootstrapConfig {
        let config = test_config(state_dir, cert_dir);
        let tick_seconds = config.acme.tick_seconds;
        let mut timer = MockEnvoyBootstrapExtensionTimer::new();
        timer
            .expect_enable()
            .once()
            .withf(move |d| {
                let min = Duration::from_secs((tick_seconds as f64 * 0.9) as u64);
                let max = Duration::from_secs((tick_seconds as f64 * 1.1) as u64);
                *d >= min && *d <= max
            })
            .return_const(());
        let mut envoy_config = make_bootstrap_mock(timer);
        AcmeBootstrapConfig::new(&mut envoy_config, config).expect("happy path should succeed")
    }

    /// Spin-wait up to `deadline` for `condition`. Mirrors `runtime.rs`'s helper.
    fn wait_for(deadline: Instant, condition: impl Fn() -> bool) -> bool {
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        condition()
    }

    /// Generate a self-signed cert whose `not_after` is `not_after_unix`.
    /// Mirrors the helper in `acme/mod.rs` tests.
    fn generate_cert(not_after_unix: i64) -> (Vec<u8>, Vec<u8>) {
        use rcgen::{CertificateParams, KeyPair};
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        let not_after = time::OffsetDateTime::from_unix_timestamp(not_after_unix).unwrap();
        params.not_before = not_after - Duration::from_secs(90 * 86_400);
        params.not_after = not_after;
        let cert = params.self_signed(&key).unwrap();
        (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
    }

    // =========================================================================
    // Existing tests (kept exactly as written)
    // =========================================================================

    #[test]
    fn dead_runtime_logging_is_rate_limited() {
        let last_dead_runtime_log = Mutex::new(None);
        let mut logs = Vec::<String>::new();
        let now = Instant::now();

        handle_runtime_tick_result(
            Err(RuntimeError::Stopped),
            false,
            &last_dead_runtime_log,
            now,
            |msg| logs.push(msg.to_string()),
        );
        handle_runtime_tick_result(
            Err(RuntimeError::Stopped),
            false,
            &last_dead_runtime_log,
            now + Duration::from_secs(1),
            |msg| logs.push(msg.to_string()),
        );
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0], DEAD_RUNTIME_LOG_MESSAGE);

        handle_runtime_tick_result(
            Err(RuntimeError::Stopped),
            false,
            &last_dead_runtime_log,
            now + Duration::from_secs(61),
            |msg| logs.push(msg.to_string()),
        );
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[1], DEAD_RUNTIME_LOG_MESSAGE);
    }

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

    // =========================================================================
    // Test A: AcmeBootstrapConfig::new — happy path
    // =========================================================================

    #[test]
    fn new_bootstrap_config_happy_path() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();
        let config = test_config(state_tmp.path(), cert_tmp.path());
        let tick_seconds = config.acme.tick_seconds;

        let mut timer = MockEnvoyBootstrapExtensionTimer::new();
        timer
            .expect_enable()
            .once()
            .withf(move |d| {
                let min = Duration::from_secs((tick_seconds as f64 * 0.9) as u64);
                let max = Duration::from_secs((tick_seconds as f64 * 1.1) as u64);
                *d >= min && *d <= max
            })
            .return_const(());

        let mut envoy_config = make_bootstrap_mock(timer);
        let bc = AcmeBootstrapConfig::new(&mut envoy_config, config).expect("should succeed");

        bc.runtime.shutdown().unwrap();
        bc.runtime.join_for_test();
    }

    // =========================================================================
    // Test B: AcmeBootstrapConfig::new — StateLock already held → LockAcquisition
    // =========================================================================

    #[test]
    fn new_bootstrap_config_lock_contention() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();

        // First acquisition — must succeed.
        let mut timer1 = MockEnvoyBootstrapExtensionTimer::new();
        timer1.expect_enable().once().return_const(());
        let mut envoy_config1 = make_bootstrap_mock(timer1);
        let bc1 = AcmeBootstrapConfig::new(
            &mut envoy_config1,
            test_config(state_tmp.path(), cert_tmp.path()),
        )
        .expect("first acquisition should succeed");

        // Second acquisition with the same state_dir — must fail at StateLock before
        // metrics::init is reached (so the mock needs no metric expectations).
        let mut envoy_config2 = MockEnvoyBootstrapExtensionConfig::new();
        let err = match AcmeBootstrapConfig::new(
            &mut envoy_config2,
            test_config(state_tmp.path(), cert_tmp.path()),
        ) {
            Ok(_) => panic!("second acquisition should have failed"),
            Err(e) => e,
        };

        assert!(
            matches!(err, RuntimeError::LockAcquisition(_)),
            "expected LockAcquisition, got: {err:?}"
        );
        if let RuntimeError::LockAcquisition(e) = &err {
            assert_eq!(e.kind(), std::io::ErrorKind::WouldBlock);
        }

        bc1.runtime.shutdown().unwrap();
        bc1.runtime.join_for_test();
    }

    // =========================================================================
    // Test C: AcmeBootstrapConfig::new — probe_writable fails for cert_dir
    // =========================================================================
    // Note: StateLock is acquired before probe_writable, and probe_writable for
    // state_dir is checked first. So putting cert_dir inside a readonly parent
    // exercises the RuntimeError::Bootstrap branch that fires when cert_dir
    // cannot be created.

    #[cfg(unix)]
    #[test]
    fn new_bootstrap_config_cert_dir_not_writable() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();

        // Create a readonly parent for cert_dir. probe_writable calls
        // create_dir_all, which fails on a 0o555 parent.
        let ro_parent = state_tmp.path().join("ro_parent");
        std::fs::create_dir_all(&ro_parent).unwrap();
        std::fs::set_permissions(&ro_parent, std::fs::Permissions::from_mode(0o555)).unwrap();
        let cert_dir = ro_parent.join("certs");

        // cert_dir probe runs after StateLock (state_dir) and after the
        // state_dir probe. Both succeed because state_tmp is writable.
        // The cert_dir probe fails → RuntimeError::Bootstrap before metrics::init.
        let mut envoy_config = MockEnvoyBootstrapExtensionConfig::new();
        let err = match AcmeBootstrapConfig::new(
            &mut envoy_config,
            test_config(state_tmp.path(), &cert_dir),
        ) {
            Ok(_) => panic!("should have failed with readonly cert_dir"),
            Err(e) => e,
        };

        // Restore permissions so tempdir cleanup succeeds.
        std::fs::set_permissions(&ro_parent, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            matches!(err, RuntimeError::Bootstrap(_)),
            "expected RuntimeError::Bootstrap, got: {err:?}"
        );
        if let RuntimeError::Bootstrap(msg) = &err {
            assert!(
                msg.contains("acme.cert_sink.cert_dir"),
                "error should name the offending field, got: {msg}"
            );
        }
    }

    // =========================================================================
    // Test D: AcmeBootstrapConfig::new — metrics::init returns Err
    // =========================================================================
    // Failure is injected via define_counter_vec returning a non-success result.
    // StateLock + probe_writables succeed first; no RuntimeBridge thread is spawned.

    #[test]
    fn new_bootstrap_config_metrics_init_fails() {
        use envoy_proxy_dynamic_modules_rust_sdk::abi::envoy_dynamic_module_type_metrics_result;

        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();

        let mut envoy_config = MockEnvoyBootstrapExtensionConfig::new();
        envoy_config
            .expect_define_counter_vec()
            .once()
            .return_once(|_, _| Err(envoy_dynamic_module_type_metrics_result::MetricNotFound));

        let err = match AcmeBootstrapConfig::new(
            &mut envoy_config,
            test_config(state_tmp.path(), cert_tmp.path()),
        ) {
            Ok(_) => panic!("should have failed with metrics error"),
            Err(e) => e,
        };

        assert!(
            matches!(err, RuntimeError::Metrics(_)),
            "expected RuntimeError::Metrics, got: {err:?}"
        );
    }

    // =========================================================================
    // Test E: new_bootstrap_extension returns a Box<dyn BootstrapExtension>
    // =========================================================================

    #[test]
    fn new_bootstrap_extension_returns_boxed() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();

        let bc = new_config_for_test(state_tmp.path(), cert_tmp.path());
        let mut fake_ext = FakeEnvoyBootstrapExtension;
        let _ext = bc.new_bootstrap_extension(&mut fake_ext);

        bc.runtime.shutdown().unwrap();
        bc.runtime.join_for_test();
    }

    // =========================================================================
    // Test F: on_timer_fired — re-enables timer with ±10 % jittered duration
    // =========================================================================

    #[test]
    fn on_timer_fired_enables_timer_with_jitter() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();

        let bc = new_config_for_test(state_tmp.path(), cert_tmp.path());
        let tick_seconds = bc.tick_seconds;

        // The `_envoy_extension_config` param is unused by on_timer_fired.
        let mut mock_cfg = MockEnvoyBootstrapExtensionConfig::new();

        // Timer passed to on_timer_fired (distinct from the one stored in bc._timer).
        let mut fired_timer = MockEnvoyBootstrapExtensionTimer::new();
        fired_timer
            .expect_enable()
            .once()
            .withf(move |d| {
                let min = Duration::from_secs((tick_seconds as f64 * 0.9) as u64);
                let max = Duration::from_secs((tick_seconds as f64 * 1.1) as u64);
                *d >= min && *d <= max
            })
            .return_const(());

        bc.on_timer_fired(&mut mock_cfg, &fired_timer);

        bc.runtime.shutdown().unwrap();
        bc.runtime.join_for_test();
    }

    // =========================================================================
    // Test G: on_scheduled — delegates to metrics::on_scheduled
    // =========================================================================

    #[test]
    fn on_scheduled_delegates_to_metrics() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();

        let bc = new_config_for_test(state_tmp.path(), cert_tmp.path());

        // No pending metric updates → mock_cfg needs no expectations.
        let mut mock_cfg = MockEnvoyBootstrapExtensionConfig::new();
        bc.on_scheduled(&mut mock_cfg, 1); // METRICS_EVENT_ID = 1

        bc.runtime.shutdown().unwrap();
        bc.runtime.join_for_test();
    }

    // =========================================================================
    // Test H: on_http_callout_done — empty body, must not panic
    // =========================================================================

    #[test]
    fn on_http_callout_done_no_panic() {
        use envoy_proxy_dynamic_modules_rust_sdk::abi::envoy_dynamic_module_type_http_callout_result;

        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();

        let bc = new_config_for_test(state_tmp.path(), cert_tmp.path());
        let mut mock_cfg = MockEnvoyBootstrapExtensionConfig::new();

        bc.on_http_callout_done(
            &mut mock_cfg,
            0,
            envoy_dynamic_module_type_http_callout_result::Success,
            None,
            None,
        );

        bc.runtime.shutdown().unwrap();
        bc.runtime.join_for_test();
    }

    // =========================================================================
    // Tests I: AcmeBootstrapExtension::on_server_initialized — three variants
    // =========================================================================
    // These tests construct AcmeBootstrapExtension directly (private fields are
    // accessible because this module is a child of the bootstrap module).
    //
    // RuntimeBridge::start() is called inside on_server_initialized. The spawned
    // runtime thread will attempt an ACME tick against acme.invalid and fail
    // quickly (DNS NXDOMAIN for .invalid TLD). The test doesn't assert about the
    // tick outcome — only the synchronous log lines from on_server_initialized.

    #[traced_test]
    #[test]
    fn on_server_initialized_no_cert_cached() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();
        let config = test_config(state_tmp.path(), cert_tmp.path());
        let runtime = RuntimeBridge::new(config);

        let mut ext = AcmeBootstrapExtension {
            runtime: runtime.clone(),
            domains: vec!["example.test".into()],
            state_dir: state_tmp.path().to_path_buf(),
            renewal_window_days: 30,
        };

        let mut fake_ext = FakeEnvoyBootstrapExtension;
        ext.on_server_initialized(&mut fake_ext);

        runtime.shutdown().unwrap();
        runtime.join_for_test();

        assert!(logs_contain("no-cached-cert"));
    }

    #[traced_test]
    #[test]
    fn on_server_initialized_cert_cached() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();
        let config = test_config(state_tmp.path(), cert_tmp.path());

        // Write a valid cert with plenty of time until expiry so the runtime
        // does not attempt issuance on the first tick.
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let not_after = now_unix + 120 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(state_tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(state_tmp.path().join("key.pem"), &key_pem).unwrap();

        let runtime = RuntimeBridge::new(config);

        let mut ext = AcmeBootstrapExtension {
            runtime: runtime.clone(),
            domains: vec!["example.test".into()],
            state_dir: state_tmp.path().to_path_buf(),
            renewal_window_days: 30,
        };

        let mut fake_ext = FakeEnvoyBootstrapExtension;
        ext.on_server_initialized(&mut fake_ext);

        runtime.shutdown().unwrap();
        runtime.join_for_test();

        assert!(logs_contain("cached-cert"));
    }

    #[traced_test]
    #[test]
    fn on_server_initialized_cert_cached_but_invalid() {
        let _guard = metrics::test_lock();
        metrics::reset_test_state();

        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();
        let config = test_config(state_tmp.path(), cert_tmp.path());

        // Write malformed PEM files — cert.pem exists so inspect_state reads it,
        // then fails to parse the expiry → CertCachedButInvalid.
        std::fs::write(state_tmp.path().join("cert.pem"), b"not a cert").unwrap();
        std::fs::write(state_tmp.path().join("key.pem"), b"not a key").unwrap();

        let runtime = RuntimeBridge::new(config);

        let mut ext = AcmeBootstrapExtension {
            runtime: runtime.clone(),
            domains: vec!["example.test".into()],
            state_dir: state_tmp.path().to_path_buf(),
            renewal_window_days: 30,
        };

        let mut fake_ext = FakeEnvoyBootstrapExtension;
        ext.on_server_initialized(&mut fake_ext);

        runtime.shutdown().unwrap();
        runtime.join_for_test();

        assert!(logs_contain("cached-cert-invalid"));
    }

    // =========================================================================
    // Test J: AcmeBootstrapExtension::on_drain_started — calls runtime.shutdown()
    // =========================================================================

    #[test]
    fn on_drain_started_shuts_down_runtime() {
        let state_tmp = tempfile::tempdir().unwrap();
        let cert_tmp = tempfile::tempdir().unwrap();
        let config = test_config(state_tmp.path(), cert_tmp.path());
        let runtime = RuntimeBridge::new(config);

        // Wait for the runtime thread to be live before draining.
        assert!(
            wait_for(Instant::now() + Duration::from_millis(500), || {
                runtime.is_alive()
            }),
            "runtime should become alive within 500 ms"
        );

        let mut ext = AcmeBootstrapExtension {
            runtime: runtime.clone(),
            domains: vec!["example.test".into()],
            state_dir: state_tmp.path().to_path_buf(),
            renewal_window_days: 30,
        };

        let mut fake_ext = FakeEnvoyBootstrapExtension;
        ext.on_drain_started(&mut fake_ext);

        // Runtime should stop after the drain signal.
        assert!(
            wait_for(Instant::now() + Duration::from_millis(500), || {
                !runtime.is_alive()
            }),
            "runtime should stop within 500 ms of drain"
        );

        runtime.join_for_test();
    }

    // =========================================================================
    // Intentionally untested branches (documented)
    // =========================================================================
    //
    // Test K (on_shutdown / CompletionCallback): skipped because
    //   `CompletionCallback::new` is `pub(crate)` inside the SDK — there is no
    //   public factory or mock. The SDK needs an upstream change before this
    //   branch can be unit-tested.
    //
    // Test L (on_drain_started / on_shutdown when runtime.shutdown() returns Err):
    //   skipped because `RuntimeBridge::panic_for_test()` is a private `fn` in
    //   `runtime.rs`; tests in `bootstrap.rs` cannot call it. Making it
    //   `pub(crate)` did not measurably close the gap against the 60 % floor, so
    //   option (a) — leave it uncovered — was chosen.
}
