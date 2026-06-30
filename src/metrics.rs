//! Metrics module for envoy-acme.
//!
//! All internal locks use [`parking_lot::Mutex`], which is poison-immune: a
//! thread panic while holding the lock does not propagate `PoisonError` to
//! subsequent lock acquisitions on other threads.

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use parking_lot::Mutex;

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
    AccountState { domain: String, state: u64 },
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
            MetricUpdate::AccountState { domain, .. } => Some(("account_state", domain.as_str())),
            MetricUpdate::IssuanceTotal { .. } | MetricUpdate::IssuanceDuration { .. } => None,
        }
    }
}

struct MetricIds {
    issuance_total: EnvoyCounterVecId,
    consecutive_failures: EnvoyGaugeVecId,
    next_retry_at: EnvoyGaugeVecId,
    cert_not_after: EnvoyGaugeVecId,
    account_state: EnvoyGaugeVecId,
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
        account_state: envoy_config.define_gauge_vec("envoy_acme_account_state", &["domain"])?,
        issuance_duration: envoy_config.define_histogram("envoy_acme_issuance_duration_seconds")?,
    };

    let state = Arc::new(MetricsState {
        ids,
        scheduler: envoy_config.new_scheduler(),
        pending: Mutex::new(VecDeque::new()),
    });

    *metrics_state().lock() = Some(state);
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
        let mut pending = state.pending.lock();
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

pub(crate) fn record_issuance_permanent(_domain: &str, duration: Duration) {
    enqueue_many(vec![
        MetricUpdate::IssuanceTotal {
            result: "permanent",
        },
        MetricUpdate::IssuanceDuration {
            seconds: duration_to_seconds(duration),
        },
    ]);
}

pub(crate) fn record_issuance_recovery_required(_domain: &str, duration: Duration) {
    enqueue_many(vec![
        MetricUpdate::IssuanceTotal {
            result: "recovery_required",
        },
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

pub(crate) fn set_account_state(domain: &str, state: u64) {
    enqueue(MetricUpdate::AccountState {
        domain: domain.to_string(),
        state,
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
        let mut pending = state.pending.lock();
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
    metrics_state().lock().clone()
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
        MetricUpdate::AccountState { domain, state } => {
            envoy_config.set_gauge_vec(ids.account_state, &[domain.as_str()], state)
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

// ---------------------------------------------------------------------------
// Test-only recorder
//
// `cargo test` runs tests in parallel by default. The previous implementation
// kept observed metric updates in a process-global `Vec` and gated cross-test
// races behind a single opt-in `test_lock()` mutex. That left two failure
// modes:
//
//   1. Tests that do not hold `test_lock()` (e.g. those that only care about
//      tick side-effects, not metric contents) still call into `enqueue_many`
//      via `tick_at`, so they push entries into the shared `Vec`.
//   2. Negative assertions in metrics-aware tests (e.g.
//      `!metrics.contains("envoy_acme_issuance_total:failure")`) then see
//      those foreign entries and panic, which in turn poisons `test_lock()`
//      and cascades into ~18 secondary "failures" per CI run.
//
// Replacing the global `Vec` with a thread-local `RefCell` makes the recorder
// inherently per-test: each test thread sees only its own enqueues, so
// concurrent tests on other threads can never contaminate the assertion.
// `test_lock()` is still needed for tests that mutate `metrics_state()` (the
// `Option<Arc<MetricsState>>` shared across the whole process), but it is no
// longer load-bearing for the observation buffer.
// ---------------------------------------------------------------------------

#[cfg(test)]
thread_local! {
    static TEST_UPDATES: std::cell::RefCell<Vec<MetricUpdate>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn record_test_updates(updates: &[MetricUpdate]) {
    TEST_UPDATES.with(|cell| cell.borrow_mut().extend_from_slice(updates));
}

#[cfg(test)]
pub(crate) fn reset_test_state() {
    *metrics_state().lock() = None;
    TEST_UPDATES.with(|cell| cell.borrow_mut().clear());
}

#[cfg(test)]
pub(crate) fn take_test_updates() -> Vec<String> {
    TEST_UPDATES.with(|cell| {
        cell.borrow_mut()
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
                MetricUpdate::AccountState { domain, state } => {
                    format!("envoy_acme_account_state:{domain}:{state}")
                }
                MetricUpdate::IssuanceDuration { seconds } => {
                    format!("envoy_acme_issuance_duration_seconds:{seconds}")
                }
            })
            .collect()
    })
}

/// Serialization guard for tests that touch the shared in-process metrics state
/// (`metrics_state()` / `MetricsState`).
///
/// **Only safe in `current_thread` tokio test runtimes** — the returned
/// `MutexGuard` is `!Send` and would deadlock if held across an `.await`
/// boundary on a multi-threaded runtime where the post-await poll resumes on a
/// different worker. All our metrics tests use `#[tokio::test]` which is
/// current-thread by default, so this is fine; do not copy this pattern to a
/// multi-thread test without switching to `tokio::sync::Mutex`.
///
/// **This lock does NOT cover the test-update recorder.** That recorder is a
/// thread-local (`TEST_UPDATES`), so it is automatically isolated per test
/// thread — no lock is needed and contamination from concurrent tests on other
/// threads is structurally impossible.
#[cfg(test)]
pub(crate) fn test_lock() -> parking_lot::MutexGuard<'static, ()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(())).lock()
}

#[cfg(test)]
mod tests {
    use super::*;
    use envoy_proxy_dynamic_modules_rust_sdk::abi::envoy_dynamic_module_type_metrics_result;
    use envoy_proxy_dynamic_modules_rust_sdk::bootstrap::{
        MockEnvoyBootstrapExtensionConfig, MockEnvoyBootstrapExtensionConfigScheduler,
    };
    use envoy_proxy_dynamic_modules_rust_sdk::{
        EnvoyCounterVecId, EnvoyGaugeVecId, EnvoyHistogramId,
    };

    /// Build the standard init mock: registers all six metrics and wraps
    /// `scheduler` inside `new_scheduler`. `commit_times` is how many times
    /// `scheduler.commit` should be called (one per `enqueue_many` invocation).
    fn make_init_mock(commit_times: usize) -> MockEnvoyBootstrapExtensionConfig {
        let mut scheduler = MockEnvoyBootstrapExtensionConfigScheduler::new();
        scheduler
            .expect_commit()
            .times(commit_times)
            .returning(|_| ());

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
            .return_once(|_| Ok(EnvoyHistogramId(6)));
        envoy_config
            .expect_new_scheduler()
            .once()
            .return_once(move || Box::new(scheduler));
        envoy_config
    }

    #[test]
    fn registers_expected_metrics() {
        let _guard = test_lock();
        reset_test_state();
        let mut envoy_config = make_init_mock(0);
        init(&mut envoy_config).unwrap();
    }

    // ── apply_update dispatch ─────────────────────────────────────────────────

    #[test]
    fn apply_update_issuance_success() {
        let _guard = test_lock();
        reset_test_state();

        // commit is called once (enqueue_many batches both updates in one call)
        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        record_issuance_success("d.example", Duration::from_secs(2));

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_increment_counter_vec()
            .once()
            .withf(|id, labels, inc| {
                *id == EnvoyCounterVecId(1) && *labels == ["success"] && *inc == 1
            })
            .returning(|_, _, _| Ok(()));
        cfg2.expect_record_histogram_value()
            .once()
            .withf(|id, val| *id == EnvoyHistogramId(6) && *val == 2)
            .returning(|_, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);
    }

    #[test]
    fn apply_update_issuance_failure() {
        let _guard = test_lock();
        reset_test_state();

        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        record_issuance_failure("d.example", Duration::from_secs(3));

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_increment_counter_vec()
            .once()
            .withf(|id, labels, inc| {
                *id == EnvoyCounterVecId(1) && *labels == ["failure"] && *inc == 1
            })
            .returning(|_, _, _| Ok(()));
        cfg2.expect_record_histogram_value()
            .once()
            .withf(|id, val| *id == EnvoyHistogramId(6) && *val == 3)
            .returning(|_, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);
    }

    #[test]
    fn apply_update_consecutive_failures() {
        let _guard = test_lock();
        reset_test_state();

        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        set_consecutive_failures("d.example", 7);

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_set_gauge_vec()
            .once()
            .withf(|id, labels, val| {
                *id == EnvoyGaugeVecId(2) && *labels == ["d.example"] && *val == 7
            })
            .returning(|_, _, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);
    }

    #[test]
    fn apply_update_next_retry_at() {
        let _guard = test_lock();
        reset_test_state();

        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        set_next_retry_at("d.example", 1_700_000_000);

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_set_gauge_vec()
            .once()
            .withf(|id, labels, val| {
                *id == EnvoyGaugeVecId(3) && *labels == ["d.example"] && *val == 1_700_000_000
            })
            .returning(|_, _, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);
    }

    #[test]
    fn apply_update_cert_not_after() {
        let _guard = test_lock();
        reset_test_state();

        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        set_cert_not_after("d.example", 1_700_000_000);

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_set_gauge_vec()
            .once()
            .withf(|id, labels, val| {
                *id == EnvoyGaugeVecId(4) && *labels == ["d.example"] && *val == 1_700_000_000
            })
            .returning(|_, _, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);
    }

    #[test]
    fn apply_update_account_state() {
        let _guard = test_lock();
        reset_test_state();

        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        set_account_state("d.example", 1);

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_set_gauge_vec()
            .once()
            .withf(|id, labels, val| {
                *id == EnvoyGaugeVecId(5) && *labels == ["d.example"] && *val == 1
            })
            .returning(|_, _, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);
    }

    // ── apply_update warn-log path ────────────────────────────────────────────

    #[test]
    #[tracing_test::traced_test]
    fn apply_update_metric_error_logs_warning() {
        let _guard = test_lock();
        reset_test_state();

        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        record_issuance_success("d.example", Duration::from_secs(1));

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_increment_counter_vec()
            .once()
            .returning(|_, _, _| Err(envoy_dynamic_module_type_metrics_result::MetricNotFound));
        cfg2.expect_record_histogram_value()
            .once()
            .returning(|_, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);

        assert!(logs_contain("failed to update Envoy metric"));
    }

    // ── duration_to_seconds ───────────────────────────────────────────────────

    #[test]
    fn duration_to_seconds_cases() {
        // The function uses ceiling division: any non-zero subsecond nanos
        // count as a full extra second (matches the histogram bucket intent).
        assert_eq!(duration_to_seconds(Duration::ZERO), 0);
        assert_eq!(duration_to_seconds(Duration::from_nanos(1)), 1); // 0s + 1ns → ceil → 1
        assert_eq!(duration_to_seconds(Duration::from_secs(1)), 1); // exact, no subsec nanos
        assert_eq!(duration_to_seconds(Duration::from_millis(1500)), 2); // 1s + 500ms → ceil → 2
        assert_eq!(duration_to_seconds(Duration::new(u64::MAX, 0)), u64::MAX); // exact seconds, no overflow
        assert_eq!(duration_to_seconds(Duration::new(u64::MAX, 1)), u64::MAX); // saturating_add saturates on overflow
    }

    // ── on_scheduled wrong event_id ───────────────────────────────────────────

    #[test]
    fn on_scheduled_wrong_event_id_no_metrics_called() {
        let _guard = test_lock();
        reset_test_state();

        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        set_consecutive_failures("d.example", 1);

        // Second mock has NO expectations — passing any wrong event_id must be
        // a no-op; if any metric call is made the mock will panic.
        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        on_scheduled(&mut cfg2, 999);
    }

    // ── enqueue_many early-return when state is None ──────────────────────────

    #[test]
    fn enqueue_many_early_return_when_no_state() {
        let _guard = test_lock();
        reset_test_state(); // metrics_state() == None

        // Should not panic even though there is no metrics state.
        record_issuance_success("d.example", Duration::from_secs(1));

        // The test-only recorder still captured the updates despite the early
        // return from enqueue_many.
        let updates = take_test_updates();
        assert!(updates
            .iter()
            .any(|u| u.contains("envoy_acme_issuance_total")));
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

    // ── poison-immunity: runtime-thread panic must not cascade to caller ───────

    #[test]
    fn poisoned_lock_does_not_cascade_to_callback_thread() {
        let _guard = test_lock();
        reset_test_state();

        // Initialise metrics state so enqueue_many and on_scheduled have a
        // live state to work with (1 commit expected from record_issuance_success).
        let mut init_cfg = make_init_mock(1);
        init(&mut init_cfg).unwrap();

        // Spawn a thread that acquires the outer metrics-state lock and then
        // panics while holding it. With `std::sync::Mutex` this would poison
        // the lock; with `parking_lot::Mutex` it does not.
        let handle = std::thread::spawn(|| {
            let _lock = metrics_state().lock();
            panic!("intentional panic while holding metrics lock");
        });

        // The spawned thread should have panicked; its join handle returns Err.
        assert!(
            handle.join().is_err(),
            "expected the spawned thread to have panicked"
        );

        // After the panic the lock must be acquirable again. The following
        // calls must NOT panic — with parking_lot::Mutex poison is not
        // propagated to this thread.
        record_issuance_success("d.example", Duration::from_secs(1));

        let mut cfg2 = MockEnvoyBootstrapExtensionConfig::new();
        cfg2.expect_increment_counter_vec()
            .once()
            .returning(|_, _, _| Ok(()));
        cfg2.expect_record_histogram_value()
            .once()
            .returning(|_, _| Ok(()));

        on_scheduled(&mut cfg2, METRICS_EVENT_ID);
    }
}
