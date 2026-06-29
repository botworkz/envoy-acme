pub mod account;
pub mod backoff;
pub mod order;
pub mod renewal;

use std::path::Path;
use std::pin::Pin;

use tracing::{debug, error, info, instrument};

use crate::cert_sink::{CertBundle, CertSink};
use crate::challenge_store;
use crate::config::AcmeConfig;
use crate::errors::AcmeError;

use backoff::BackoffState;

const ACCOUNT_FILE: &str = "account.json";
const CERT_FILE: &str = "cert.pem";
const KEY_FILE: &str = "key.pem";
const BACKOFF_FILE: &str = "backoff.json";

// ---------------------------------------------------------------------------
// Issuer abstraction
// ---------------------------------------------------------------------------

/// Trait for the component that performs ACME certificate issuance.
///
/// The real implementation calls `account::load_or_create_account` followed
/// by `order::issue_certificate`.  Tests supply a `MockIssuer` that returns
/// pre-canned results without touching the network.
pub(crate) trait Issuer: Send + Sync {
    fn issue<'a>(
        &'a self,
        config: &'a AcmeConfig,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CertBundle, AcmeError>> + Send + 'a>>;
}

struct RealIssuer;

impl Issuer for RealIssuer {
    fn issue<'a>(
        &'a self,
        config: &'a AcmeConfig,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CertBundle, AcmeError>> + Send + 'a>> {
        Box::pin(async move {
            let account = account::load_or_create_account(
                &config.directory_uri,
                &config.contact,
                &config.state_dir.join(ACCOUNT_FILE),
                config.directory_ca_file.as_deref(),
            )
            .await?;
            order::issue_certificate(config, &account).await
        })
    }
}

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

pub struct AcmeStateMachine {
    config: AcmeConfig,
    sink: Box<dyn CertSink>,
    last_not_after_unix: Option<i64>,
    backoff: BackoffState,
    issuer: Box<dyn Issuer>,
}

impl AcmeStateMachine {
    /// Create a state machine backed by the real ACME issuer.
    pub fn new(config: AcmeConfig, sink: Box<dyn CertSink>) -> Self {
        Self::new_with_issuer(config, sink, Box::new(RealIssuer))
    }

    /// Create a state machine with an injected issuer.  Used in tests to
    /// supply a mock without network access.
    pub(crate) fn new_with_issuer(
        config: AcmeConfig,
        sink: Box<dyn CertSink>,
        issuer: Box<dyn Issuer>,
    ) -> Self {
        Self {
            config,
            sink,
            last_not_after_unix: None,
            backoff: BackoffState::default(),
            issuer,
        }
    }

    /// Drive one tick using the current system time.
    #[instrument(skip(self), fields(domain = %self.config.domains.first().cloned().unwrap_or_default()))]
    pub async fn tick(&mut self) -> Result<(), AcmeError> {
        self.tick_at(time::OffsetDateTime::now_utc().unix_timestamp())
            .await
    }

    /// Drive one tick at a given Unix timestamp.
    ///
    /// Accepts an explicit `now_unix` so that tests can exercise renewal-window
    /// and backoff logic with a fake clock without touching the network or real
    /// wall time.
    ///
    /// ## Tick flow
    ///
    /// ```text
    /// tick_at(now)
    ///   ├─ create state_dir if missing
    ///   ├─ load persisted backoff.json
    ///   ├─ if backoff.is_blocked(now)  →  log + return Ok(())     [no-op]
    ///   ├─ load cached cert from state_dir
    ///   ├─ if cert exists && !needs_renewal(now)  →  publish + return Ok(())
    ///   └─ issuer.issue(config)
    ///       ├─ Ok(bundle)  →  persist + publish + clear backoff + return Ok(())
    ///       └─ Err(e)
    ///           ├─ RateLimited  →  record_rate_limit (persist) + return Err(e)
    ///           └─ other        →  return Err(e)  [retry next tick]
    /// ```
    pub async fn tick_at(&mut self, now_unix: i64) -> Result<(), AcmeError> {
        tokio::fs::create_dir_all(&self.config.state_dir).await?;

        // Reload persisted backoff so that restarts respect next_retry_at.
        self.backoff = load_backoff(&self.config.state_dir).await;

        let domain = self
            .config
            .domains
            .first()
            .map(String::as_str)
            .unwrap_or("");

        // ── Rate-limit backoff guard ─────────────────────────────────────
        if self.backoff.is_blocked(now_unix) {
            let next_retry_at = self.backoff.next_retry_at.unwrap_or(0);
            debug!(
                domain,
                next_retry_at, "rate-limit backoff active, skipping issuance"
            );
            return Ok(());
        }

        // ── Renewal window check ─────────────────────────────────────────
        let cached = self.load_cached_bundle().await?;
        if let Some((bundle, not_after)) = cached {
            if !renewal::needs_renewal_at_with_domain_offset(
                not_after,
                now_unix,
                self.config.renewal_window_days,
                domain,
            ) {
                self.last_not_after_unix = Some(not_after);
                self.publish("cached", &bundle)?;
                return Ok(());
            }
        }

        // ── Certificate issuance ─────────────────────────────────────────
        match self.issuer.issue(&self.config).await {
            Ok(bundle) => {
                self.persist_bundle(&bundle).await?;
                self.publish("issued", &bundle)?;

                match renewal::cert_not_after_unix(&bundle.cert_pem) {
                    Ok(v) => self.last_not_after_unix = Some(v),
                    Err(e) => error!(%e, "unable to parse not_after from issued certificate"),
                }

                // Successful issuance clears any rate-limit back-off.
                self.backoff.clear();
                persist_backoff(&self.config.state_dir, &self.backoff).await?;

                Ok(())
            }
            Err(e) => {
                if matches!(
                    backoff::classify_acme_error(&e),
                    backoff::ErrorClass::RateLimited
                ) {
                    self.backoff.record_rate_limit(now_unix);
                    persist_backoff(&self.config.state_dir, &self.backoff).await?;
                    let next_retry_at = self.backoff.next_retry_at.unwrap_or(0);
                    info!(
                        domain,
                        next_retry_at,
                        consecutive_failures = self.backoff.consecutive_failures,
                        "rate-limited by ACME server, backing off"
                    );
                }
                // Non-rate-limit errors propagate as-is; the caller logs them
                // and retries on the next ordinary tick interval.
                Err(e)
            }
        }
    }

    #[allow(dead_code)]
    pub async fn force_renew(&mut self) -> Result<(), AcmeError> {
        self.last_not_after_unix = None;
        self.tick().await
    }

    async fn load_cached_bundle(&self) -> Result<Option<(CertBundle, i64)>, AcmeError> {
        let cert_path = self.config.state_dir.join(CERT_FILE);
        let key_path = self.config.state_dir.join(KEY_FILE);
        if !cert_path.exists() || !key_path.exists() {
            return Ok(None);
        }

        let cert_pem = tokio::fs::read(&cert_path).await?;
        let key_pem = tokio::fs::read(&key_path).await?;
        let not_after = renewal::cert_not_after_unix(&cert_pem)?;
        Ok(Some((CertBundle { cert_pem, key_pem }, not_after)))
    }

    async fn persist_bundle(&self, bundle: &CertBundle) -> Result<(), AcmeError> {
        let cert_path = self.config.state_dir.join(CERT_FILE);
        let key_path = self.config.state_dir.join(KEY_FILE);
        tokio::fs::write(cert_path, &bundle.cert_pem).await?;
        tokio::fs::write(key_path, &bundle.key_pem).await?;
        Ok(())
    }

    fn publish(&self, marker: &str, bundle: &CertBundle) -> Result<(), AcmeError> {
        let domain = self
            .config
            .domains
            .first()
            .map(std::string::String::as_str)
            .unwrap_or("default");
        self.sink.publish(domain, bundle)?;
        info!(domain, marker, "published certificate bundle to sink");
        Ok(())
    }

    pub fn clear_challenges(&self) {
        let map = challenge_store::get();
        map.write().clear();
    }
}

// ---------------------------------------------------------------------------
// Backoff persistence helpers
// ---------------------------------------------------------------------------

async fn load_backoff(state_dir: &Path) -> BackoffState {
    let path = state_dir.join(BACKOFF_FILE);
    match tokio::fs::read(&path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => BackoffState::default(),
    }
}

async fn persist_backoff(state_dir: &Path, state: &BackoffState) -> Result<(), AcmeError> {
    let path = state_dir.join(BACKOFF_FILE);
    let bytes = serde_json::to_vec(state)?;
    tokio::fs::write(path, bytes).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use rcgen::{CertificateParams, KeyPair};

    use super::*;
    use crate::cert_sink::{CertBundle, CertSink};
    use crate::config::{AcmeConfig, CertSinkConfig, Layout};
    use crate::errors::SinkError;

    // ── Helpers ─────────────────────────────────────────────────────────────

    fn test_config(state_dir: &std::path::Path) -> AcmeConfig {
        AcmeConfig {
            directory_profile: None,
            directory_uri: "https://acme.invalid/directory".into(),
            directory_ca_file: None,
            contact: "mailto:test@example.test".into(),
            domains: vec!["a.example.test".into()],
            renewal_window_days: 30,
            state_dir: state_dir.to_path_buf(),
            cert_sink: CertSinkConfig {
                sink_type: "filesystem".into(),
                cert_dir: state_dir.to_path_buf(),
                layout: Layout::PerDomain,
            },
            tick_seconds: 60,
        }
    }

    /// Generate a self-signed cert whose `not_after` is `not_after_unix`.
    fn generate_cert(not_after_unix: i64) -> (Vec<u8>, Vec<u8>) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        let not_after = time::OffsetDateTime::from_unix_timestamp(not_after_unix).unwrap();
        params.not_before = not_after - Duration::from_secs(90 * 86_400);
        params.not_after = not_after;
        let cert = params.self_signed(&key).unwrap();
        (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
    }

    // ── MockCertSink ─────────────────────────────────────────────────────────

    struct MockCertSink {
        calls: Mutex<Vec<String>>,
    }

    impl Default for MockCertSink {
        fn default() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl CertSink for MockCertSink {
        fn publish(&self, name: &str, _bundle: &CertBundle) -> Result<(), SinkError> {
            self.calls.lock().unwrap().push(name.to_string());
            Ok(())
        }
    }

    impl MockCertSink {
        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    // ── MockIssuer ───────────────────────────────────────────────────────────

    struct MockIssuer {
        results: Mutex<Vec<Result<CertBundle, AcmeError>>>,
    }

    impl MockIssuer {
        fn with_results(results: Vec<Result<CertBundle, AcmeError>>) -> Self {
            Self {
                results: Mutex::new(results),
            }
        }

        fn always_ok(cert_pem: Vec<u8>, key_pem: Vec<u8>) -> Self {
            Self::with_results(vec![Ok(CertBundle { cert_pem, key_pem })])
        }
    }

    impl Issuer for MockIssuer {
        fn issue<'a>(
            &'a self,
            _config: &'a AcmeConfig,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<CertBundle, AcmeError>> + Send + 'a>>
        {
            let result = self
                .results
                .lock()
                .unwrap()
                .drain(..1)
                .next()
                .unwrap_or_else(|| Err(AcmeError::OrderFailed("mock exhausted".into())));
            Box::pin(async move { result })
        }
    }

    fn rate_limit_error() -> AcmeError {
        let problem: instant_acme::Problem = serde_json::from_value(serde_json::json!({
            "type": "urn:ietf:params:acme:error:rateLimited",
            "detail": "too many certificates",
            "status": 429
        }))
        .unwrap();
        AcmeError::Protocol(instant_acme::Error::Api(problem))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 1 — cert outside renewal window: tick → no issuer call, one sink
    //           call (re-publish of cached cert)
    // ─────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn cert_outside_renewal_window_no_issuance() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        // Cert expires 90 days from now; 30-day window → not in window yet.
        let not_after = now_unix + 90 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();

        let sink = std::sync::Arc::new(MockCertSink::default());
        let sink_clone = sink.clone();

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        // MockIssuer with empty results — must not be called.
        let issuer = Box::new(MockIssuer::with_results(vec![]));
        let mut sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(ArcSink(sink.clone())),
            issuer,
        );
        sm.tick_at(now_unix).await.unwrap();

        // One "cached" publish, no issuer call.
        assert_eq!(sink_clone.call_count(), 1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 2 — cert inside renewal window: tick → exactly one sink call
    //           via the mock issuer
    // ─────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn cert_inside_renewal_window_triggers_issuance() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        // Cert expires 10 days from now; 30-day window → inside window.
        let not_after = now_unix + 10 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();

        let new_not_after = now_unix + 90 * 86_400;
        let (new_cert, new_key) = generate_cert(new_not_after);

        let sink = std::sync::Arc::new(MockCertSink::default());
        let sink_clone = sink.clone();

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let issuer = Box::new(MockIssuer::always_ok(new_cert, new_key));
        let mut sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(ArcSink(sink.clone())),
            issuer,
        );
        sm.tick_at(now_unix).await.unwrap();

        assert_eq!(sink_clone.call_count(), 1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 3 — rate-limited error: backoff persisted; next tick is a no-op;
    //           tick after backoff window retries
    // ─────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn rate_limited_sets_backoff_and_blocks_retry() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let new_not_after = now_unix + 90 * 86_400;
        let (new_cert, new_key) = generate_cert(new_not_after);

        let sink = std::sync::Arc::new(MockCertSink::default());
        let sink_clone = sink.clone();

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![
            Err(rate_limit_error()),
            Ok(CertBundle {
                cert_pem: new_cert,
                key_pem: new_key,
            }),
        ]));
        let mut sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(ArcSink(sink.clone())),
            issuer,
        );

        // Tick 1 → rate-limit → error, backoff.json written.
        assert!(sm.tick_at(now_unix).await.is_err());
        assert_eq!(sink_clone.call_count(), 0, "no publish on rate-limit");

        let bp = tmp.path().join("backoff.json");
        assert!(bp.exists(), "backoff.json must be written");
        let state: BackoffState = serde_json::from_slice(&std::fs::read(&bp).unwrap()).unwrap();
        assert_eq!(state.consecutive_failures, 1);
        let next_retry_at = state.next_retry_at.unwrap();
        assert!(next_retry_at > now_unix);

        // Tick 2 — still within backoff → no-op.
        sm.tick_at(next_retry_at - 1).await.unwrap();
        assert_eq!(sink_clone.call_count(), 0, "still blocked during backoff");

        // Tick 3 — after backoff → issuance succeeds.
        sm.tick_at(next_retry_at + 1).await.unwrap();
        assert_eq!(sink_clone.call_count(), 1, "cert published after backoff");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 4 — backoff escalates: second delay is roughly 2× the first
    // ─────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn backoff_escalates_on_consecutive_failures() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![
            Err(rate_limit_error()),
            Err(rate_limit_error()),
        ]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        let _ = sm.tick_at(now_unix).await;
        let first_retry = sm.backoff.next_retry_at.unwrap();
        let first_delay = first_retry - now_unix;

        let t2 = first_retry + 1;
        let _ = sm.tick_at(t2).await;
        let second_retry = sm.backoff.next_retry_at.unwrap();
        let second_delay = second_retry - t2;

        // With ±20 % jitter, second delay should be ~2× first.
        // Accept [1.2×, 3.0×] to cover full jitter range.
        assert!(
            second_delay >= first_delay * 12 / 10,
            "second_delay={second_delay} should be ≥ 1.2× first_delay={first_delay}"
        );
        assert!(
            second_delay <= first_delay * 30 / 10,
            "second_delay={second_delay} should be ≤ 3.0× first_delay={first_delay}"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 5 — backoff cap: many consecutive failures → delay ≤ 24 h + 20 %
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn backoff_cap_never_exceeds_24h_plus_jitter() {
        const MAX_DELAY: i64 = 24 * 60 * 60;
        const JITTER: i64 = (MAX_DELAY as f64 * 0.20) as i64;

        for consecutive in [0u32, 5, 10, 20, 50, 100] {
            let next = backoff::compute_next_retry_at(0, consecutive);
            let delay = next;
            assert!(
                delay <= MAX_DELAY + JITTER,
                "delay {delay}s exceeds cap for consecutive={consecutive}"
            );
            assert!(delay >= 1);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 6 — success clears backoff
    // ─────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn success_clears_backoff() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let new_not_after = now_unix + 90 * 86_400;
        let (new_cert, new_key) = generate_cert(new_not_after);

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![
            Err(rate_limit_error()),
            Ok(CertBundle {
                cert_pem: new_cert,
                key_pem: new_key,
            }),
        ]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        let _ = sm.tick_at(now_unix).await;
        assert_eq!(sm.backoff.consecutive_failures, 1);
        let retry_at = sm.backoff.next_retry_at.unwrap();

        sm.tick_at(retry_at + 1).await.unwrap();
        assert_eq!(sm.backoff.consecutive_failures, 0);
        assert!(sm.backoff.next_retry_at.is_none());

        let stored: BackoffState =
            serde_json::from_slice(&std::fs::read(tmp.path().join("backoff.json")).unwrap())
                .unwrap();
        assert_eq!(stored, BackoffState::default());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 7 — jitter offset is deterministic
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn jitter_offset_is_stable() {
        let not_after = 1_000_000 + 90 * 86_400;
        let now = 1_000_000i64;
        let r1 = renewal::needs_renewal_at_with_domain_offset(not_after, now, 30, "a.example");
        let r2 = renewal::needs_renewal_at_with_domain_offset(not_after, now, 30, "a.example");
        assert_eq!(r1, r2);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 8 — jitter offset spreads: 100 distinct domains span > half window
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn jitter_offset_spreads_across_window() {
        let window_secs = 30u64 * 86_400;
        // The offset for a domain is: fnv1a(domain) % window_secs.
        // Replicate the formula from renewal.rs here.
        fn offset(domain: &str, window: u64) -> u64 {
            const O: u64 = 14_695_981_039_346_656_037;
            const P: u64 = 1_099_511_628_211;
            let h = domain
                .bytes()
                .fold(O, |h, b| (h ^ u64::from(b)).wrapping_mul(P));
            h % window
        }
        let offsets: Vec<u64> = (0..100u32)
            .map(|i| offset(&format!("domain-{i}.example"), window_secs))
            .collect();
        let min = *offsets.iter().min().unwrap();
        let max = *offsets.iter().max().unwrap();
        assert!(
            max - min > window_secs / 2,
            "span {}s ≤ half of {}s window",
            max - min,
            window_secs
        );
    }
}
