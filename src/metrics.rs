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

impl MetricUpdate {
    /// Identity for gauge coalescing: two updates share an identity iff a
    /// later one fully supersedes an earlier one for dashboard purposes.
    ///
    /// Returns `None` for counters and histograms — those are append-only
    /// observations and must never be dropped from the pending queue.
    fn coalesce_key(&self) -> Option<(&'static str, &str)> {
        match self {
            MetricUpdate::ConsecutiveFailures { domain, .. } => {
                Some(("consecutive_failures", domain.as_str()))
            }
            MetricUpdate::NextRetryAt { domain, .. } => Some(("next_retry_at", domain.as_str())),
            MetricUpdate::CertNotAfter { domain, .. } => Some(("cert_not_after", domain.as_str())),
            MetricUpdate::IssuanceTotal { .. } | MetricUpdate::IssuanceDuration { .. } => None,
        }
    }
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
        for update in updates {
            // Coalesce gauges: a later set for the same (kind, domain) tuple
            // fully supersedes an earlier one for dashboard purposes, so we
            // drop the earlier one rather than letting the queue grow on
            // every tick. Counters and histograms have `coalesce_key() ==
            // None` and are always appended; we must never drop those.
            if let Some(key) = update.coalesce_key() {
                pending.retain(|existing| existing.coalesce_key() != Some(key));
            }
            pending.push_back(update);
        }
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

    // Envoy's histogram API accepts integer values here, so record whole seconds
    // and round fractional durations up into the next second bucket.
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

/// Serialization guard for tests that touch the shared in-process metrics state.
///
/// **Only safe in `current_thread` tokio test runtimes** — the returned
/// `MutexGuard` is `!Send` and would deadlock if held across an `.await`
/// boundary on a multi-threaded runtime where the post-await poll resumes on a
/// different worker. All our metrics tests use `#[tokio::test]` which is
/// current-thread by default, so this is fine; do not copy this pattern to a
/// multi-thread test without switching to `tokio::sync::Mutex`.
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

    #[test]
    fn gauge_updates_coalesce_for_same_domain() {
        // Three rapid sets for the same gauge+domain should collapse to one
        // entry in the pending queue — the latest one.
        let mut q: VecDeque<MetricUpdate> = VecDeque::new();
        let updates = [
            MetricUpdate::ConsecutiveFailures {
                domain: "a.example".into(),
                count: 1,
            },
            MetricUpdate::ConsecutiveFailures {
                domain: "a.example".into(),
                count: 2,
            },
            MetricUpdate::ConsecutiveFailures {
                domain: "a.example".into(),
                count: 3,
            },
        ];
        for u in updates {
            if let Some(key) = u.coalesce_key() {
                q.retain(|existing| existing.coalesce_key() != Some(key));
            }
            q.push_back(u);
        }
        assert_eq!(q.len(), 1);
        assert!(matches!(
            q.front().unwrap(),
            MetricUpdate::ConsecutiveFailures { count: 3, .. }
        ));
    }

    #[test]
    fn gauge_updates_do_not_coalesce_across_domains() {
        let mut q: VecDeque<MetricUpdate> = VecDeque::new();
        let updates = [
            MetricUpdate::ConsecutiveFailures {
                domain: "a.example".into(),
                count: 1,
            },
            MetricUpdate::ConsecutiveFailures {
                domain: "b.example".into(),
                count: 2,
            },
        ];
        for u in updates {
            if let Some(key) = u.coalesce_key() {
                q.retain(|existing| existing.coalesce_key() != Some(key));
            }
            q.push_back(u);
        }
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn counters_and_histograms_never_coalesce() {
        // Two counter increments and two histogram observations in a row
        // must all survive — dropping either would lose an observation.
        let mut q: VecDeque<MetricUpdate> = VecDeque::new();
        let updates = [
            MetricUpdate::IssuanceTotal { result: "success" },
            MetricUpdate::IssuanceTotal { result: "success" },
            MetricUpdate::IssuanceDuration { seconds: 42 },
            MetricUpdate::IssuanceDuration { seconds: 7 },
        ];
        for u in updates {
            if let Some(key) = u.coalesce_key() {
                q.retain(|existing| existing.coalesce_key() != Some(key));
            }
            q.push_back(u);
        }
        assert_eq!(q.len(), 4);
    }
}
