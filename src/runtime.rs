use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
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
    tx: Arc<mpsc::UnboundedSender<Command>>,
    runtime_alive: Arc<AtomicBool>,
    #[cfg(test)]
    thread_handle: Arc<parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl RuntimeBridge {
    pub fn new(config: Config) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<Command>();
        let runtime_alive = Arc::new(AtomicBool::new(false));
        let runtime_alive_thread = runtime_alive.clone();

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

                    let mut sm = AcmeStateMachine::new(config.acme.clone(), sink);

                    while let Some(command) = rx.recv().await {
                        match command {
                            Command::Start | Command::Tick => {
                                if let Err(e) = sm.tick().await {
                                    envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                                        "envoy-acme: state-machine command failed: {e}"
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
            runtime_alive,
            #[cfg(test)]
            thread_handle: Arc::new(parking_lot::Mutex::new(Some(_handle))),
        }
    }

    pub fn start(&self) -> Result<(), RuntimeError> {
        self.tx
            .send(Command::Start)
            .map_err(|_| RuntimeError::Stopped)
    }

    pub fn tick(&self) -> Result<(), RuntimeError> {
        self.tx
            .send(Command::Tick)
            .map_err(|_| RuntimeError::Stopped)
    }

    pub fn shutdown(&self) -> Result<(), RuntimeError> {
        self.tx
            .send(Command::Shutdown)
            .map_err(|_| RuntimeError::Stopped)
    }

    pub fn is_alive(&self) -> bool {
        self.runtime_alive.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn panic_for_test(&self) -> Result<(), RuntimeError> {
        self.tx
            .send(Command::PanicForTest)
            .map_err(|_| RuntimeError::Stopped)
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
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    use super::*;
    use crate::config::{AcmeConfig, CertSinkConfig, Config, Layout, LogConfig};
    use crate::errors::RuntimeError;

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
}
