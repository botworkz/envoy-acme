use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use envoy_proxy_dynamic_modules_rust_sdk::abi::envoy_dynamic_module_type_metrics_result;
use envoy_proxy_dynamic_modules_rust_sdk::bootstrap::EnvoyBootstrapExtensionConfigScheduler;
use envoy_proxy_dynamic_modules_rust_sdk::{
    EnvoyBootstrapExtensionConfig, EnvoyCounterVecId, EnvoyGaugeVecId, EnvoyHistogramId,
};
use tracing::warn;

const METRICS_EVENT_ID: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
enum MetricUpdate {
    IssuanceTotal { result: &'static str },
    ConsecutiveFailures { domain: String, count: u32 },
    NextRetryAt { domain: String, unix_ts: u64 },
    CertNotAfter { domain: String, unix_ts: u64 },
    IssuanceDuration { seconds: u64 },
}

struct MetricIds {
    issuance_total: EnvoyCounterVecId,
    consecutive_failures: EnvoyGaugeVecId,
    next_retry_at: EnvoyGaugeVecId,
    cert_not_after: EnvoyGaugeVecId,
    issuance_duration: EnvoyHistogramId,
}

struct MetricsState {
    ids: MetricIds,
    scheduler: Box<dyn EnvoyBootstrapExtensionConfigScheduler>,
    pending: Mutex<VecDeque<MetricUpdate>>,
}

fn metrics_state() -> &'static Mutex<Option<Arc<MetricsState>>> {
    static STATE: OnceLock<Mutex<Option<Arc<MetricsState>>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn init(
    envoy_config: &mut dyn EnvoyBootstrapExtensionConfig,
) -> Result<(), envoy_dynamic_module_type_metrics_result> {
    let ids = MetricIds {
        issuance_total: envoy_config
            .define_counter_vec("envoy_acme_issuance_total", &["result"])?,
        consecutive_failures: envoy_config
            .define_gauge_vec("envoy_acme_consecutive_failures", &["domain"])?,
        next_retry_at: envoy_config
            .define_gauge_vec("envoy_acme_next_retry_at_seconds", &["domain"])?,
        cert_not_after: envoy_config
            .define_gauge_vec("envoy_acme_cert_not_after_seconds", &["domain"])?,
        issuance_duration: envoy_config.define_histogram("envoy_acme_issuance_duration_seconds")?,
    };

    let state = Arc::new(MetricsState {
        ids,
        scheduler: envoy_config.new_scheduler(),
        pending: Mutex::new(VecDeque::new()),
    });

    *metrics_state().lock().unwrap() = Some(state);
    Ok(())
}

pub(crate) fn on_scheduled(envoy_config: &mut dyn EnvoyBootstrapExtensionConfig, event_id: u64) {
    if event_id != METRICS_EVENT_ID {
        return;
    }

    let Some(state) = current_state() else {
        return;
    };

    let updates: Vec<_> = {
        let mut pending = state.pending.lock().unwrap();
        pending.drain(..).collect()
    };

    for update in updates {
        apply_update(envoy_config, &state.ids, update);
    }
}

pub(crate) fn record_issuance_success(_domain: &str, duration: Duration) {
    enqueue_many(vec![
        MetricUpdate::IssuanceTotal { result: "success" },
        MetricUpdate::IssuanceDuration {
            seconds: duration_to_seconds(duration),
        },
    ]);
}

pub(crate) fn record_issuance_failure(_domain: &str, duration: Duration) {
    enqueue_many(vec![
        MetricUpdate::IssuanceTotal { result: "failure" },
        MetricUpdate::IssuanceDuration {
            seconds: duration_to_seconds(duration),
        },
    ]);
}

pub(crate) fn set_consecutive_failures(domain: &str, count: u32) {
    enqueue(MetricUpdate::ConsecutiveFailures {
        domain: domain.to_string(),
        count,
    });
}

pub(crate) fn set_next_retry_at(domain: &str, unix_ts: u64) {
    enqueue(MetricUpdate::NextRetryAt {
        domain: domain.to_string(),
        unix_ts,
    });
}

pub(crate) fn set_cert_not_after(domain: &str, unix_ts: u64) {
    enqueue(MetricUpdate::CertNotAfter {
        domain: domain.to_string(),
        unix_ts,
    });
}

#[allow(dead_code)]
pub(crate) fn record_runtime_alive(_alive: bool) {}

fn enqueue(update: MetricUpdate) {
    enqueue_many(vec![update]);
}

fn enqueue_many(updates: Vec<MetricUpdate>) {
    #[cfg(test)]
    record_test_updates(&updates);

    let Some(state) = current_state() else {
        return;
    };

    {
        let mut pending = state.pending.lock().unwrap();
        pending.extend(updates);
    }

    state.scheduler.commit(METRICS_EVENT_ID);
}

fn current_state() -> Option<Arc<MetricsState>> {
    metrics_state().lock().unwrap().clone()
}

fn apply_update(
    envoy_config: &mut dyn EnvoyBootstrapExtensionConfig,
    ids: &MetricIds,
    update: MetricUpdate,
) {
    let result = match update {
        MetricUpdate::IssuanceTotal { result } => {
            envoy_config.increment_counter_vec(ids.issuance_total, &[result], 1)
        }
        MetricUpdate::ConsecutiveFailures { domain, count } => envoy_config.set_gauge_vec(
            ids.consecutive_failures,
            &[domain.as_str()],
            u64::from(count),
        ),
        MetricUpdate::NextRetryAt { domain, unix_ts } => {
            envoy_config.set_gauge_vec(ids.next_retry_at, &[domain.as_str()], unix_ts)
        }
        MetricUpdate::CertNotAfter { domain, unix_ts } => {
            envoy_config.set_gauge_vec(ids.cert_not_after, &[domain.as_str()], unix_ts)
        }
        MetricUpdate::IssuanceDuration { seconds } => {
            envoy_config.record_histogram_value(ids.issuance_duration, seconds)
        }
    };

    if let Err(err) = result {
        warn!(error = ?err, "failed to update Envoy metric");
    }
}

fn duration_to_seconds(duration: Duration) -> u64 {
    if duration.is_zero() {
        return 0;
    }

    duration
        .as_secs()
        .saturating_add(u64::from(duration.subsec_nanos() > 0))
}

#[cfg(test)]
fn test_updates() -> &'static Mutex<Vec<MetricUpdate>> {
    static TEST_UPDATES: OnceLock<Mutex<Vec<MetricUpdate>>> = OnceLock::new();
    TEST_UPDATES.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(test)]
fn record_test_updates(updates: &[MetricUpdate]) {
    test_updates().lock().unwrap().extend_from_slice(updates);
}

#[cfg(test)]
pub(crate) fn reset_test_state() {
    *metrics_state().lock().unwrap() = None;
    test_updates().lock().unwrap().clear();
}

#[cfg(test)]
pub(crate) fn take_test_updates() -> Vec<String> {
    test_updates()
        .lock()
        .unwrap()
        .drain(..)
        .map(|update| match update {
            MetricUpdate::IssuanceTotal { result } => {
                format!("envoy_acme_issuance_total:{result}")
            }
            MetricUpdate::ConsecutiveFailures { domain, count } => {
                format!("envoy_acme_consecutive_failures:{domain}:{count}")
            }
            MetricUpdate::NextRetryAt { domain, unix_ts } => {
                format!("envoy_acme_next_retry_at_seconds:{domain}:{unix_ts}")
            }
            MetricUpdate::CertNotAfter { domain, unix_ts } => {
                format!("envoy_acme_cert_not_after_seconds:{domain}:{unix_ts}")
            }
            MetricUpdate::IssuanceDuration { seconds } => {
                format!("envoy_acme_issuance_duration_seconds:{seconds}")
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use envoy_proxy_dynamic_modules_rust_sdk::bootstrap::{
        MockEnvoyBootstrapExtensionConfig, MockEnvoyBootstrapExtensionConfigScheduler,
    };
    use envoy_proxy_dynamic_modules_rust_sdk::{
        EnvoyCounterVecId, EnvoyGaugeVecId, EnvoyHistogramId,
    };

    #[test]
    fn registers_expected_metrics() {
        let _guard = test_lock();
        reset_test_state();

        let mut scheduler = MockEnvoyBootstrapExtensionConfigScheduler::new();
        scheduler.expect_commit().times(0);

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
            .expect_define_histogram()
            .once()
            .withf(|name| name == "envoy_acme_issuance_duration_seconds")
            .return_once(|_| Ok(EnvoyHistogramId(5)));
        envoy_config
            .expect_new_scheduler()
            .once()
            .return_once(move || Box::new(scheduler));

        init(&mut envoy_config).unwrap();
    }
}
