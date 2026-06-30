use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
#[cfg(test)]
use std::time::{Duration, Instant};

use tokio::runtime::Builder;
use tokio::sync::mpsc;

use crate::acme::AcmeStateMachine;
use crate::cert_sink::filesystem::FilesystemSink;
use crate::cert_sink::CertSink;
use crate::config::Config;
use crate::errors::RuntimeError;

enum Command {
    Start,
    Tick,
    Shutdown,
    #[cfg(test)]
    PanicForTest,
}

#[derive(Clone)]
pub struct RuntimeBridge {
    tx: Arc<mpsc::Sender<Command>>,
    /// Counts `Tick` commands dropped since the last tick run completed.
    /// Incremented by `tick()` on every coalesced send; reset to zero by the
    /// runtime thread at the end of each `sm.tick()` call.
    dropped_tick_count: Arc<AtomicUsize>,
    runtime_alive: Arc<AtomicBool>,
    #[cfg(test)]
    thread_handle: Arc<parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl RuntimeBridge {
    pub fn new(config: Config) -> Self {
        Self::new_impl(config, |acme_config, sink| {
            AcmeStateMachine::new(acme_config, sink)
        })
    }

    /// Construct a `RuntimeBridge` with a custom `Issuer` injected into the
    /// state machine.  Only available in tests; used to supply a `MockIssuer`
    /// that does not touch the network.
    #[cfg(test)]
    pub(crate) fn new_with_issuer(config: Config, issuer: Box<dyn crate::acme::Issuer>) -> Self {
        Self::new_impl(config, move |acme_config, sink| {
            AcmeStateMachine::new_with_issuer(acme_config, sink, issuer)
        })
    }

    fn new_impl<F>(config: Config, make_sm: F) -> Self
    where
        F: FnOnce(crate::config::AcmeConfig, Box<dyn CertSink>) -> AcmeStateMachine
            + Send
            + 'static,
    {
        // Capacity-1 bounded channel: at most one Tick can queue behind the
        // currently-running tick.  Additional Tick sends are coalesced (dropped
        // with a debug log) by `tick()`, which uses `try_send`.
        let (tx, mut rx) = mpsc::channel::<Command>(1);
        let runtime_alive = Arc::new(AtomicBool::new(false));
        let runtime_alive_thread = runtime_alive.clone();
        let dropped_tick_count = Arc::new(AtomicUsize::new(0));
        let dropped_tick_count_thread = dropped_tick_count.clone();

        let _handle = thread::spawn(move || {
            // `runtime_alive` is the operator-facing "this thread is actually
            // serving ticks" signal. We only flip it true *after* the tokio
            // runtime has been successfully constructed — if construction
            // itself were to panic (extremely unlikely, but `Builder::build`
            // is not contractually panic-free), the AtomicBool stays false
            // and `is_alive()` returns the truth rather than reporting a
            // dead thread as alive.
            //
            // enable_all so the IO driver is present for the rustls/hyper
            // HTTPS transport used by instant-acme. Without IO, the first
            // connect attempt would fail and the runtime would drop.
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                        "envoy-acme: failed to create tokio runtime: {e}"
                    );
                    // `runtime_alive` was never set to true; nothing to clear.
                    return;
                }
            };
            runtime_alive_thread.store(true, Ordering::Release);

            // Catch any panic that escapes block_on so it surfaces as an
            // envoy log line. Without this, a panic on this thread just
            // exits silently and every subsequent tick logs
            // "runtime thread already stopped" with no clue to the cause.
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                runtime.block_on(async move {
                    let sink: Box<dyn CertSink> = Box::new(FilesystemSink::new(
                        config.acme.cert_sink.cert_dir.clone(),
                        config.acme.cert_sink.layout,
                    ));

                    let mut sm = make_sm(config.acme.clone(), sink);

                    while let Some(command) = rx.recv().await {
                        match command {
                            Command::Start | Command::Tick => {
                                if let Err(e) = sm.tick().await {
                                    envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                                        "envoy-acme: state-machine command failed: {e}"
                                    );
                                }
                                // After each tick attempt (success or failure),
                                // read and reset the coalesced-drop counter so
                                // `dropped_since_last_run` in the next coalescing
                                // log reflects only drops since this run.
                                let dropped =
                                    dropped_tick_count_thread.swap(0, Ordering::Relaxed);
                                if dropped > 0 {
                                    tracing::debug!(
                                        dropped_since_last_run = dropped,
                                        "envoy-acme: {} tick(s) coalesced while previous run was in flight",
                                        dropped
                                    );
                                }
                            }
                            Command::Shutdown => {
                                sm.clear_challenges();
                                break;
                            }
                            #[cfg(test)]
                            Command::PanicForTest => panic!("panic injected for runtime test"),
                        }
                    }
                });
            }));
            runtime_alive_thread.store(false, Ordering::Release);

            if let Err(panic) = result {
                let msg = panic_message(panic.as_ref());
                envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                    "envoy-acme: runtime thread panicked: {msg}"
                );
            }
        });

        Self {
            tx: Arc::new(tx),
            dropped_tick_count,
            runtime_alive,
            #[cfg(test)]
            thread_handle: Arc::new(parking_lot::Mutex::new(Some(_handle))),
        }
    }

    pub fn start(&self) -> Result<(), RuntimeError> {
        // At boot the channel is empty; `try_send` will always succeed.
        // If it somehow races with a queued Tick (extremely unlikely), the
        // Tick performs the same work, so dropping Start is harmless.
        match self.tx.try_send(Command::Start) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(RuntimeError::Stopped),
        }
    }

    pub fn tick(&self) -> Result<(), RuntimeError> {
        match self.tx.try_send(Command::Tick) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                // A tick is already in flight (or queued); coalesce this one.
                let count = self.dropped_tick_count.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::debug!(
                    dropped_since_last_run = count,
                    "envoy-acme: tick coalesced — previous tick still running"
                );
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(RuntimeError::Stopped),
        }
    }

    pub fn shutdown(&self) -> Result<(), RuntimeError> {
        // `blocking_send` waits for buffer space if the channel is full,
        // ensuring Shutdown is never dropped.  Blocking briefly at shutdown
        // time is acceptable.
        self.tx
            .blocking_send(Command::Shutdown)
            .map_err(|_| RuntimeError::Stopped)
    }

    pub fn is_alive(&self) -> bool {
        self.runtime_alive.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn panic_for_test(&self) -> Result<(), RuntimeError> {
        match self.tx.try_send(Command::PanicForTest) {
            Ok(()) => Ok(()),
            Err(_) => Err(RuntimeError::Stopped),
        }
    }

    #[cfg(test)]
    pub(crate) fn join_for_test(&self) {
        if let Some(handle) = self.thread_handle.lock().take() {
            let _ = handle.join();
        }
    }
}

fn panic_message(panic: &(dyn Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering};

    use tracing_test::traced_test;

    use super::*;
    use crate::acme::Issuer;
    use crate::cert_sink::CertBundle;
    use crate::config::{AcmeConfig, CertSinkConfig, Config, Layout, LogConfig};
    use crate::errors::{AcmeError, RuntimeError};

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

    fn test_config() -> Config {
        let id = NEXT_TEST_ID.fetch_add(1, AtomicOrdering::Relaxed);
        let base = std::env::temp_dir().join(format!("envoy-acme-runtime-test-{id}"));
        let state_dir = base.join("state");
        let cert_dir = base.join("certs");

        Config {
            acme: AcmeConfig {
                directory_profile: None,
                directory_uri: "https://acme.invalid/directory".into(),
                directory_ca_file: None,
                contact: "mailto:test@example.test".into(),
                domains: vec!["example.test".into()],
                renewal_window_days: 30,
                state_dir,
                cert_sink: CertSinkConfig {
                    sink_type: "filesystem".into(),
                    cert_dir,
                    layout: Layout::PerDomain,
                },
                tick_seconds: 60,
                issuance_timeout_seconds: 120,
            },
            log: LogConfig::default(),
        }
    }

    fn wait_for(deadline: Instant, condition: impl Fn() -> bool) -> bool {
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        condition()
    }

    /// An `Issuer` that sleeps for `delay` per call and counts how many times
    /// `issue()` has been invoked.  Always returns `Err(AcmeError::Timeout)`
    /// so the state machine does not try to persist a bundle.
    struct SlowIssuer {
        call_count: Arc<AtomicUsize>,
        delay: Duration,
    }

    impl Issuer for SlowIssuer {
        fn issue<'a>(
            &'a self,
            _config: &'a AcmeConfig,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<CertBundle, AcmeError>> + Send + 'a>>
        {
            let call_count = self.call_count.clone();
            let delay = self.delay;
            Box::pin(async move {
                call_count.fetch_add(1, AtomicOrdering::Relaxed);
                tokio::time::sleep(delay).await;
                Err(AcmeError::Timeout)
            })
        }
    }

    #[test]
    fn runtime_alive_true_after_start() {
        let runtime = RuntimeBridge::new(test_config());
        assert!(wait_for(
            Instant::now() + Duration::from_millis(200),
            || runtime.is_alive()
        ));
        runtime.shutdown().expect("shutdown should send");
        runtime.join_for_test();
    }

    #[test]
    fn runtime_alive_false_after_shutdown() {
        let runtime = RuntimeBridge::new(test_config());
        assert!(wait_for(
            Instant::now() + Duration::from_millis(200),
            || runtime.is_alive()
        ));

        runtime.shutdown().expect("shutdown should send");
        runtime.join_for_test();

        assert!(!runtime.is_alive());
    }

    #[test]
    fn runtime_alive_false_after_panic() {
        let runtime = RuntimeBridge::new(test_config());
        assert!(wait_for(
            Instant::now() + Duration::from_millis(200),
            || runtime.is_alive()
        ));

        runtime.panic_for_test().expect("panic command should send");

        assert!(wait_for(
            Instant::now() + Duration::from_millis(500),
            || !runtime.is_alive()
        ));
        assert!(matches!(runtime.tick(), Err(RuntimeError::Stopped)));
        runtime.join_for_test();
    }

    /// Rapid `tick()` calls while a slow issuer is running are coalesced: at
    /// most one tick can queue behind the in-flight one, so the issuer is
    /// invoked at most twice regardless of how many `tick()` calls arrive.
    #[test]
    fn tick_coalesced_when_issuer_is_slow() {
        // Hold the metrics lock and clear any state left by other tests so
        // that `sm.tick()` metric calls don't trigger a stale mock scheduler.
        let _metrics_guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let call_count = Arc::new(AtomicUsize::new(0));
        let issuer = Box::new(SlowIssuer {
            call_count: call_count.clone(),
            // Each tick takes 400 ms — well over the 100 ms window in which
            // we fire 10 ticks.
            delay: Duration::from_millis(400),
        });

        let runtime = RuntimeBridge::new_with_issuer(test_config(), issuer);
        assert!(wait_for(
            Instant::now() + Duration::from_millis(200),
            || runtime.is_alive()
        ));

        // Fire 10 tick() calls within a ~100 ms window.
        for _ in 0..10 {
            runtime.tick().expect("tick send should not fail");
            std::thread::sleep(Duration::from_millis(10));
        }

        // Wait long enough for at most 2 issuer calls to complete
        // (1 in-flight + 1 queued = 2 × 400 ms = 800 ms, plus headroom).
        std::thread::sleep(Duration::from_millis(1200));

        let actual_calls = call_count.load(AtomicOrdering::Relaxed);
        assert!(
            actual_calls <= 2,
            "expected at most 2 issuer invocations, got {actual_calls}"
        );

        runtime.shutdown().expect("shutdown should send");
        runtime.join_for_test();
    }

    /// When a `Tick` is coalesced, `tick()` emits a `debug!` log with the
    /// field `dropped_since_last_run`.
    #[test]
    #[traced_test]
    fn tick_coalescing_emits_debug_log() {
        let _metrics_guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let issuer = Box::new(SlowIssuer {
            call_count: Arc::new(AtomicUsize::new(0)),
            delay: Duration::from_millis(500),
        });

        let runtime = RuntimeBridge::new_with_issuer(test_config(), issuer);
        assert!(wait_for(
            Instant::now() + Duration::from_millis(200),
            || runtime.is_alive()
        ));

        // First tick goes into the channel (no coalescing).
        runtime.tick().expect("first tick should send");

        // Second tick should be coalesced because the channel is now full
        // (first tick is either in-flight or queued).
        runtime.tick().expect("second tick should not error");

        // The debug log must have been emitted by now (it's synchronous with
        // the `tick()` call).
        assert!(logs_contain("tick coalesced"));
        // The structured field must reflect exactly one drop since the last run.
        assert!(logs_contain("dropped_since_last_run=1"));

        runtime.shutdown().expect("shutdown should send");
        runtime.join_for_test();
    }

    /// `shutdown()` is never dropped even when `tick()` calls are flooding the
    /// channel: the runtime must stop after processing any queued ticks.
    #[test]
    fn shutdown_not_dropped_during_tick_flood() {
        let _metrics_guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let issuer = Box::new(SlowIssuer {
            call_count: Arc::new(AtomicUsize::new(0)),
            delay: Duration::from_millis(50),
        });

        let runtime = RuntimeBridge::new_with_issuer(test_config(), issuer);
        assert!(wait_for(
            Instant::now() + Duration::from_millis(200),
            || runtime.is_alive()
        ));

        // Flood with ticks (most will be coalesced).
        for _ in 0..5 {
            runtime.tick().expect("tick should not error");
        }

        // Shutdown must succeed and the runtime must eventually stop.
        runtime.shutdown().expect("shutdown should succeed");
        runtime.join_for_test();

        assert!(!runtime.is_alive(), "runtime should be dead after shutdown");
    }
}
