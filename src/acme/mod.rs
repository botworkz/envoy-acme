pub mod account;
pub mod backoff;
pub mod order;
pub mod renewal;

use std::path::Path;
use std::pin::Pin;
use std::time::Instant;

use sha2::{Digest, Sha256};
use tracing::{debug, error, info, instrument, warn};

use crate::cert_sink::{CertBundle, CertSink};
use crate::challenge_store;
use crate::config::AcmeConfig;
use crate::errors::AcmeError;
use crate::metrics;

use backoff::BackoffState;

const ACCOUNT_FILE: &str = "account.json";
const CERT_FILE: &str = "cert.pem";
const KEY_FILE: &str = "key.pem";
const SENTINEL_FILE: &str = "bundle.ok";
const BACKOFF_FILE: &str = "backoff.json";
const DEFAULT_HEARTBEAT_EVERY_TICKS: u32 = 60;

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

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
    last_renewal_at_unix: Option<u64>,
    backoff: BackoffState,
    heartbeat_every_ticks: u32,
    ticks_since_last_heartbeat: u32,
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
            last_renewal_at_unix: None,
            backoff: BackoffState::default(),
            // TODO: make heartbeat interval configurable via `acme.heartbeat_every_ticks`.
            heartbeat_every_ticks: DEFAULT_HEARTBEAT_EVERY_TICKS,
            ticks_since_last_heartbeat: 0,
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
        let emit_heartbeat = self.should_emit_heartbeat();
        tokio::fs::create_dir_all(&self.config.state_dir).await?;

        // Reload persisted backoff so that restarts respect next_retry_at.
        self.backoff = load_backoff(&self.config.state_dir).await;

        let domain = self
            .config
            .domains
            .first()
            .map(String::as_str)
            .unwrap_or("");

        metrics::set_consecutive_failures(domain, self.backoff.consecutive_failures);
        metrics::set_next_retry_at(
            domain,
            self.backoff
                .next_retry_at
                .and_then(|ts| u64::try_from(ts).ok())
                .unwrap_or(0),
        );

        // ── Rate-limit backoff guard ─────────────────────────────────────
        if self.backoff.is_blocked(now_unix) {
            let next_retry_at = self.backoff.next_retry_at.unwrap_or(0);
            debug!(
                domain,
                next_retry_at, "rate-limit backoff active, skipping issuance"
            );
            if emit_heartbeat {
                self.emit_heartbeat(now_unix, true);
            }
            return Ok(());
        }

        // ── Renewal window check ─────────────────────────────────────────
        let cached = self.load_cached_bundle().await?;
        if let Some((bundle, not_after)) = cached {
            // Keep the most recently observed cert expiry in memory for heartbeat logs.
            self.last_not_after_unix = Some(not_after);
            if !renewal::needs_renewaxl_at_with_domain_offset(
                not_after,
                now_unix,
                self.config.renewal_window_days,
                domain,
            ) {
                self.publish("cached", &bundle)?;
                if emit_heartbeat {
                    self.emit_heartbeat(now_unix, false);
                }
                return Ok(());
            }
        }

        // ── Certificate issuance ─────────────────────────────────────────
        let issuance_started = Instant::now();
        match self.issuer.issue(&self.config).await {
            Ok(bundle) => {
                let elapsed = issuance_started.elapsed();
                self.persist_bundle(&bundle).await?;
                self.publish("issued", &bundle)?;

                match renewal::cert_not_after_unix(&bundle.cert_pem) {
                    Ok(v) => {
                        self.last_not_after_unix = Some(v);
                        if let Ok(not_after_unix) = u64::try_from(v) {
                            metrics::set_cert_not_after(domain, not_after_unix);
                        }
                    }
                    Err(e) => error!(%e, "unable to parse not_after from issued certificate"),
                }
                self.last_renewal_at_unix = u64::try_from(now_unix).ok();

                // Successful issuance clears any rate-limit back-off.
                self.backoff.clear();
                persist_backoff(&self.config.state_dir, &self.backoff).await?;
                metrics::record_issuance_success(domain, elapsed);
                metrics::set_consecutive_failures(domain, 0);
                metrics::set_next_retry_at(domain, 0);

                if emit_heartbeat {
                    self.emit_heartbeat(now_unix, false);
                }
                Ok(())
            }
            Err(e) => {
                let elapsed = issuance_started.elapsed();
                metrics::record_issuance_failure(domain, elapsed);
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
                    if let Ok(next_retry_at_unix) = u64::try_from(next_retry_at) {
                        metrics::set_next_retry_at(domain, next_retry_at_unix);
                    }
                }
<<<<<<< HEAD
                metrics::set_consecutive_failures(domain, self.backoff.consecutive_failures);
=======
                if emit_heartbeat {
                    self.emit_heartbeat(now_unix, false);
                }
>>>>>>> bc04f96 (acme: Add periodic heartbeat logs and tests)
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

    fn should_emit_heartbeat(&mut self) -> bool {
        self.ticks_since_last_heartbeat = self.ticks_since_last_heartbeat.saturating_add(1);
        if self
            .ticks_since_last_heartbeat
            .ge(&self.heartbeat_every_ticks.max(1))
        {
            self.ticks_since_last_heartbeat = 0;
            return true;
        }
        false
    }

    fn emit_heartbeat(&self, now_unix: i64, backoff_blocked: bool) {
        let cert_not_after_unix = self.last_not_after_unix.and_then(|v| u64::try_from(v).ok());
        let next_attempt_at_unix = if backoff_blocked {
            self.backoff
                .next_retry_at
                .and_then(|v| u64::try_from(v).ok())
        } else {
            let tick_seconds = i64::try_from(self.config.tick_seconds).ok();
            tick_seconds
                .and_then(|delta| now_unix.checked_add(delta))
                .and_then(|v| u64::try_from(v).ok())
        };

        for domain in &self.config.domains {
            info!(
                domain = %domain,
                state = "heartbeat",
                cert_not_after_unix = ?cert_not_after_unix,
                last_renewal_at_unix = ?self.last_renewal_at_unix,
                next_attempt_at_unix = ?next_attempt_at_unix,
                consecutive_failures = self.backoff.consecutive_failures,
                "envoy-acme heartbeat"
            );
        }
    }

    async fn load_cached_bundle(&self) -> Result<Option<(CertBundle, i64)>, AcmeError> {
        let cert_path = self.config.state_dir.join(CERT_FILE);
        let key_path = self.config.state_dir.join(KEY_FILE);
        if !cert_path.exists() || !key_path.exists() {
            return Ok(None);
        }

        // Check the sentinel first; on a missing or stale sentinel the cert/key
        // bytes are not trustworthy regardless of how parseable they are, and
        // there is no point reading them. The sentinel is also the cheapest of
        // the three reads, so the broken-cache path stays fast.
        let sentinel_path = self.config.state_dir.join(SENTINEL_FILE);
        let domain = self
            .config
            .domains
            .first()
            .map(String::as_str)
            .unwrap_or("");
        let sentinel = match tokio::fs::read(&sentinel_path).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                warn!(
                    domain = %domain,
                    reason = "sentinel missing",
                    "cached bundle invalid; will re-issue"
                );
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };

        let cert_pem = tokio::fs::read(&cert_path).await?;
        let expected = sha256_hex(&cert_pem);
        if sentinel != expected.as_bytes() {
            warn!(
                domain = %domain,
                reason = "sentinel hash mismatch",
                "cached bundle invalid; will re-issue"
            );
            return Ok(None);
        }

        let key_pem = tokio::fs::read(&key_path).await?;
        let not_after = renewal::cert_not_after_unix(&cert_pem)?;
        Ok(Some((CertBundle { cert_pem, key_pem }, not_after)))
    }

    async fn persist_bundle(&self, bundle: &CertBundle) -> Result<(), AcmeError> {
        let cert_path = self.config.state_dir.join(CERT_FILE);
        let key_path = self.config.state_dir.join(KEY_FILE);
        let cert_pem = bundle.cert_pem.clone();
        tokio::task::spawn_blocking(move || {
            crate::atomic_write::write_atomic(&cert_path, &cert_pem, false)
        })
        .await
        .map_err(std::io::Error::other)??;
        let key_pem = bundle.key_pem.clone();
        tokio::task::spawn_blocking(move || {
            crate::atomic_write::write_atomic(&key_path, &key_pem, true)
        })
        .await
        .map_err(std::io::Error::other)??;
        let sentinel_path = self.config.state_dir.join(SENTINEL_FILE);
        let sentinel = sha256_hex(&bundle.cert_pem).into_bytes();
        tokio::task::spawn_blocking(move || {
            crate::atomic_write::write_atomic(&sentinel_path, &sentinel, false)
        })
        .await
        .map_err(std::io::Error::other)??;
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
    tokio::task::spawn_blocking(move || crate::atomic_write::write_atomic(&path, &bytes, false))
        .await
        .map_err(std::io::Error::other)??;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Mutex;
    use std::time::Duration;

    use rcgen::{CertificateParams, KeyPair};
    use tracing_test::traced_test;

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

    // ── MockCertSink ────────────────────────────────────────────────────────

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

    // ── MockIssuer ──────────────────────────────────────────────────────────

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

    // ────────────────────────────────────────────────────────────────────────
    // Test 1 — cert outside renewal window: tick → no issuer call, one sink
    //           call (re-publish of cached cert)
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn cert_outside_renewal_window_no_issuance() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        // Cert expires 90 days from now; 30-day window → not in window yet.
        let not_after = now_unix + 90 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

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

    // ────────────────────────────────────────────────────────────────────────
    // Test 2 — cert inside renewal window: tick → exactly one sink call
    //           via the mock issuer
    // ────────────────────────────────────────────────────────────────────────
    #[tokio::test]
    async fn cert_inside_renewal_window_triggers_issuance() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        // Cert expires 10 days from now; 30-day window → inside window.
        let not_after = now_unix + 10 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

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

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn successful_issuance_emits_success_metrics() {
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let new_not_after = now_unix + 90 * 86_400;
        let (new_cert, new_key) = generate_cert(new_not_after);

        let sink = std::sync::Arc::new(MockCertSink::default());

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let issuer = Box::new(MockIssuer::always_ok(new_cert, new_key));
        let mut sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(ArcSink(sink)),
            issuer,
        );

        sm.tick_at(now_unix).await.unwrap();

        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(metrics.contains("envoy_acme_issuance_total:success"));
        assert!(metrics
            .iter()
            .any(|metric| metric.starts_with("envoy_acme_issuance_duration_seconds:")));
        assert!(metrics.contains("envoy_acme_consecutive_failures:a.example.test:0"));
        assert!(metrics.contains("envoy_acme_next_retry_at_seconds:a.example.test:0"));
        assert!(metrics.contains(&format!(
            "envoy_acme_cert_not_after_seconds:a.example.test:{}",
            new_not_after
        )));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 3 — rate-limited error: backoff persisted; next tick is a no-op;
    //           tick after backoff window retries
    // ────────────────────────────────────────────────────────────────────────
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

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn rate_limited_failure_emits_failure_metrics() {
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;

        let sink = std::sync::Arc::new(MockCertSink::default());

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Err(rate_limit_error())]));
        let mut sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(ArcSink(sink)),
            issuer,
        );

        assert!(sm.tick_at(now_unix).await.is_err());

        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(metrics.contains("envoy_acme_issuance_total:failure"));
        assert!(metrics
            .iter()
            .any(|metric| metric.starts_with("envoy_acme_issuance_duration_seconds:")));
        assert!(metrics.contains("envoy_acme_consecutive_failures:a.example.test:1"));

        let next_retry_metric = metrics
            .iter()
            .filter_map(|metric| {
                metric
                    .strip_prefix("envoy_acme_next_retry_at_seconds:a.example.test:")
                    .and_then(|value| value.parse::<i64>().ok())
            })
            .max()
            .expect("next_retry_at metric should be recorded");
        assert!(next_retry_metric > now_unix);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 4 — backoff escalates: second delay is roughly 2× the first
    // ────────────────────────────────────────────────────────────────────────
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

    // ────────────────────────────────────────────────────────────────────────
    // Test 5 — backoff cap: many consecutive failures → delay ≤ 24 h + 20 %
    // ────────────────────────────────────────────────────────────────────────
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

    // ────────────────────────────────────────────────────────────────────────
    // Test 6 — success clears backoff
    // ────────────────────────────────────────────────────────────────────────
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

    #[tokio::test]
    async fn load_cached_bundle_ok_when_sentinel_matches() {
        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400);
        let bundle = CertBundle {
            cert_pem: cert_pem.clone(),
            key_pem: key_pem.clone(),
        };
        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(DevNull),
            Box::new(MockIssuer::with_results(vec![])),
        );

        sm.persist_bundle(&bundle).await.unwrap();
        let loaded = sm.load_cached_bundle().await.unwrap();

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn load_cached_bundle_none_when_sentinel_missing() {
        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(CERT_FILE), b"not a cert").unwrap();
        std::fs::write(tmp.path().join(KEY_FILE), b"not a key").unwrap();
        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(DevNull),
            Box::new(MockIssuer::with_results(vec![])),
        );

        let loaded = sm.load_cached_bundle().await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn load_cached_bundle_none_when_sentinel_stale() {
        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400);
        let bundle = CertBundle { cert_pem, key_pem };
        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(DevNull),
            Box::new(MockIssuer::with_results(vec![])),
        );

        sm.persist_bundle(&bundle).await.unwrap();
        tokio::fs::write(tmp.path().join(CERT_FILE), b"stale cert bytes")
            .await
            .unwrap();

        let loaded = sm.load_cached_bundle().await.unwrap();
        assert!(loaded.is_none());
    }

    // ────────────────────────────────────────────────────────────────────────
    // Test 7 — jitter offset is deterministic
    // ────────────────────────────────────────────────────────────────────────
    #[test]
    fn jitter_offset_is_stable() {
        let not_after = 1_000_000 + 90 * 86_400;
        let now = 1_000_000i64;
        let r1 = renewal::needs_renewal_at_with_domain_offset(not_after, now, 30, "a.example");
        let r2 = renewal::needs_renewal_at_with_domain_offset(not_after, now, 30, "a.example");
        assert_eq!(r1, r2);
    }

    // ────────────────────────────────────────────────────────────────────────
    // Test 8 — jitter offset spreads: 100 distinct domains span > half window
    // ────────────────────────────────────────────────────────────────────────
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

    // ────────────────────────────────────────────────────────────────────────
    // Heartbeat tests
    //
    // NOTE: `#[traced_test]` MUST be the outer attribute and `#[tokio::test]`
    // the inner one. `traced_test` installs its thread-local subscriber by
    // wrapping the test fn; if `tokio::test` is outer, the runtime is built
    // first and `info!` calls from inside the async body run on a worker
    // thread where the subscriber is not in scope, so `logs_contain` and
    // `logs_assert` will return nothing and the tests will fail spuriously.
    // ────────────────────────────────────────────────────────────────────────
    #[traced_test]
    #[tokio::test]
    async fn heartbeat_fires_on_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let not_after = now_unix + 90 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);
        sm.heartbeat_every_ticks = 3;

        sm.tick_at(now_unix).await.unwrap();
        sm.tick_at(now_unix + 60).await.unwrap();
        sm.tick_at(now_unix + 120).await.unwrap();

        logs_assert(|lines: &[&str]| {
            let count = lines
                .iter()
                .filter(|line| line.contains("envoy-acme heartbeat"))
                .count();
            match count {
                1 => Ok(()),
                n => Err(format!("expected exactly one heartbeat event, got {n}")),
            }
        });
    }

    #[traced_test]
    #[tokio::test]
    async fn heartbeat_includes_expected_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let not_after = now_unix + 90 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);
        sm.heartbeat_every_ticks = 1;

        sm.tick_at(now_unix).await.unwrap();

        assert!(logs_contain("envoy-acme heartbeat"));
        assert!(logs_contain("domain=a.example.test"));
        assert!(logs_contain("state=\"heartbeat\""));
        assert!(logs_contain("consecutive_failures=0"));
        assert!(logs_contain("cert_not_after_unix=Some("));
    }

    #[traced_test]
    #[tokio::test]
    async fn heartbeat_resets_counter() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let not_after = now_unix + 90 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);
        sm.heartbeat_every_ticks = 3;

        sm.tick_at(now_unix).await.unwrap();
        sm.tick_at(now_unix + 60).await.unwrap();
        sm.tick_at(now_unix + 120).await.unwrap();
        sm.tick_at(now_unix + 180).await.unwrap();
        sm.tick_at(now_unix + 240).await.unwrap();
        sm.tick_at(now_unix + 300).await.unwrap();

        logs_assert(|lines: &[&str]| {
            let count = lines
                .iter()
                .filter(|line| line.contains("envoy-acme heartbeat"))
                .count();
            match count {
                2 => Ok(()),
                n => Err(format!("expected exactly two heartbeat events, got {n}")),
            }
        });
    }
}
