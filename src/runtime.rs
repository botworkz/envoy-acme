use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::thread;

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
}

#[derive(Clone)]
pub struct RuntimeBridge {
    tx: Arc<mpsc::UnboundedSender<Command>>,
}

impl RuntimeBridge {
    pub fn new(config: Config) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<Command>();

        thread::spawn(move || {
            // enable_all so the IO driver is present for the rustls/hyper
            // HTTPS transport used by instant-acme. Without IO, the first
            // connect attempt would fail and the runtime would drop.
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                        "envoy-acme: failed to create tokio runtime: {e}"
                    );
                    return;
                }
            };

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
                        }
                    }
                });
            }));

            if let Err(panic) = result {
                let msg = panic_message(panic.as_ref());
                envoy_proxy_dynamic_modules_rust_sdk::envoy_log_error!(
                    "envoy-acme: runtime thread panicked: {msg}"
                );
            }
        });

        Self { tx: Arc::new(tx) }
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
