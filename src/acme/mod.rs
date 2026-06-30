//! ACME state machine and supporting sub-modules for certificate issuance and renewal.
pub mod account;
pub mod backoff;
pub(crate) mod client;
pub mod order;
pub mod renewal;

use std::path::Path;
use std::pin::Pin;
use std::time::{Duration, Instant};

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
const SECONDS_PER_DAY: i64 = 86_400;
const DEFAULT_HEARTBEAT_EVERY_TICKS: u32 = 60;
const RECOVERY_REQUIRED_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StateSummary {
    NoCertCached,
    CertCached {
        not_after_unix: u64,
        days_until_renewal: i64,
    },
    CertCachedButInvalid {
        reason: String,
    },
}

pub(crate) fn inspect_state(
    state_dir: &Path,
    domain: &str,
    renewal_window_days: u64,
) -> StateSummary {
    let cert_path = state_dir.join(CERT_FILE);
    if !cert_path.exists() {
        return StateSummary::NoCertCached;
    }

    let key_path = state_dir.join(KEY_FILE);
    if !key_path.exists() {
        return StateSummary::CertCachedButInvalid {
            reason: format!(
                "missing key file for domain {domain}: {}",
                key_path.display()
            ),
        };
    }

    let cert_pem = match std::fs::read(&cert_path) {
        Ok(cert) => cert,
        Err(e) => {
            return StateSummary::CertCachedButInvalid {
                reason: format!("failed to read cert file for domain {domain}: {e}"),
            };
        }
    };

    if let Err(e) = std::fs::read(&key_path) {
        return StateSummary::CertCachedButInvalid {
            reason: format!("failed to read key file for domain {domain}: {e}"),
        };
    }

    let not_after = match renewal::cert_not_after_unix(&cert_pem) {
        Ok(v) => v,
        Err(e) => {
            return StateSummary::CertCachedButInvalid {
                reason: format!("cert parse failed for domain {domain}: {e}"),
            };
        }
    };

    let not_after_unix = match u64::try_from(not_after) {
        Ok(v) => v,
        Err(_) => {
            return StateSummary::CertCachedButInvalid {
                reason: "cert has invalid not_after timestamp".to_string(),
            };
        }
    };

    let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
    if not_after <= now_unix {
        let not_after_iso = time::OffsetDateTime::from_unix_timestamp(not_after)
            .map(|dt| dt.to_string())
            .unwrap_or_else(|_| format!("unix:{not_after}"));
        return StateSummary::CertCachedButInvalid {
            reason: format!("cert expired for domain {domain} at {not_after_iso}"),
        };
    }

    let days_until_expiry = (not_after - now_unix) / SECONDS_PER_DAY;
    let days_until_renewal = days_until_expiry - (renewal_window_days as i64);

    StateSummary::CertCached {
        not_after_unix,
        days_until_renewal,
    }
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
            let account_client = client::RealAcmeAccount(&account);
            order::issue_certificate(config, &account_client).await
        })
    }
}

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

/// Extract the ACME problem type URN from an error, if available.
///
/// Returns the `type` field of the problem document for `Protocol(Api(…))`
/// errors.  Returns `None` for all other error variants.
fn acme_problem_type(e: &AcmeError) -> Option<&str> {
    if let AcmeError::Protocol(instant_acme::Error::Api(problem)) = e {
        problem.r#type.as_deref()
    } else {
        None
    }
}

/// Returns `true` if a rate-limited log message should be emitted now, and
/// updates `last_log` to the current instant when it does.
///
/// Emits on the first call (`last_log == None`) and then at most once per
/// `interval`.  This mirrors the pattern used by `handle_runtime_tick_result`
/// for the "engine is dead" message.
fn should_emit_rate_limited_log(
    last_log: &mut Option<Instant>,
    interval: std::time::Duration,
) -> bool {
    let now = Instant::now();
    let emit = last_log
        .map(|t| now.duration_since(t) >= interval)
        .unwrap_or(true);
    if emit {
        *last_log = Some(now);
    }
    emit
}

/// Per-domain ACME renewal state machine that checks, issues, and caches TLS certificates each tick.
pub struct AcmeStateMachine {
    config: AcmeConfig,
    sink: Box<dyn CertSink>,
    last_not_after_unix: Option<i64>,
    last_renewal_at_unix: Option<u64>,
    backoff: BackoffState,
    heartbeat_every_ticks: u32,
    ticks_since_last_heartbeat: u32,
    issuer: Box<dyn Issuer>,
    /// Timestamp of the last `RecoveryRequired` error log emission.
    /// Used to rate-limit log spam on repeated failures.
    last_recovery_required_log: Option<Instant>,
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
            last_recovery_required_log: None,
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
            if let Ok(not_after_unix) = u64::try_from(not_after) {
                metrics::set_cert_not_after(domain, not_after_unix);
            }
            if !renewal::needs_renewal_at_with_domain_offset(
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
        let timeout = Duration::from_secs(self.config.issuance_timeout_seconds);
        match tokio::time::timeout(timeout, self.issuer.issue(&self.config)).await {
            Ok(Ok(bundle)) => {
                let elapsed = issuance_started.elapsed();

                // Validate the issued bundle before persisting or publishing.
                // A CA returning wrong SANs or a mismatched key is treated as
                // an issuance failure so the bad bundle is never served.
                if let Err(e) = validate_bundle(&self.config, &bundle) {
                    let reason = match &e {
                        AcmeError::OrderFailed(msg) => msg.as_str(),
                        _ => "bundle validation failed",
                    };
                    metrics::record_issuance_failure(domain, elapsed);
                    error!(
                        domain,
                        reason, "issued bundle failed validation; treating as issuance failure"
                    );
                    metrics::set_consecutive_failures(domain, self.backoff.consecutive_failures);
                    if emit_heartbeat {
                        self.emit_heartbeat(now_unix, false);
                    }
                    return Err(e);
                }

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
                metrics::set_account_state(domain, 0);

                if emit_heartbeat {
                    self.emit_heartbeat(now_unix, false);
                }
                Ok(())
            }
            Ok(Err(e)) => {
                let elapsed = issuance_started.elapsed();
                let error_class = backoff::classify_acme_error(&e);
                let problem_type = acme_problem_type(&e).unwrap_or("unknown");
                match error_class {
                    backoff::ErrorClass::RateLimited => {
                        metrics::record_issuance_failure(domain, elapsed);
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
                        metrics::set_account_state(domain, 0);
                    }
                    backoff::ErrorClass::Permanent => {
                        metrics::record_issuance_permanent(domain, elapsed);
                        error!(
                            domain,
                            problem_type, %e,
                            "permanent ACME error (bug-class): will retry but operator should investigate"
                        );
                        metrics::set_account_state(domain, 0);
                    }
                    backoff::ErrorClass::RecoveryRequired => {
                        metrics::record_issuance_recovery_required(domain, elapsed);
                        if should_emit_rate_limited_log(
                            &mut self.last_recovery_required_log,
                            RECOVERY_REQUIRED_LOG_INTERVAL,
                        ) {
                            error!(
                                domain,
                                problem_type, %e,
                                "ACME account recovery required: operator action needed \
                                 (e.g. delete account.json and re-register, or check domain \
                                 authorisation); this message is rate-limited to once per 60 s"
                            );
                        }
                        metrics::set_account_state(domain, 1);
                    }
                    backoff::ErrorClass::Transient => {
                        metrics::record_issuance_failure(domain, elapsed);
                        metrics::set_account_state(domain, 0);
                    }
                }
                metrics::set_consecutive_failures(domain, self.backoff.consecutive_failures);
                if emit_heartbeat {
                    self.emit_heartbeat(now_unix, false);
                }
                // All error classes propagate the error; the caller retries on
                // the next ordinary tick interval.
                Err(e)
            }
            Err(_) => {
                let elapsed = issuance_started.elapsed();
                metrics::record_issuance_failure(domain, elapsed);
                metrics::set_consecutive_failures(domain, self.backoff.consecutive_failures);
                warn!(
                    domain,
                    timeout_seconds = self.config.issuance_timeout_seconds,
                    "issuance exceeded timeout; will retry next tick"
                );
                if emit_heartbeat {
                    self.emit_heartbeat(now_unix, false);
                }
                Err(AcmeError::Timeout)
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
        let bundle = CertBundle { cert_pem, key_pem };

        // Validate SANs and key↔cert consistency via the shared helper.
        if let Err(e) = validate_bundle(&self.config, &bundle) {
            let reason = match &e {
                AcmeError::OrderFailed(msg) => msg.as_str(),
                _ => "bundle validation failed",
            };
            warn!(domain = %domain, reason = %reason, "cached bundle invalid; will re-issue");
            return Ok(None);
        }

        let not_after = renewal::cert_not_after_unix(&bundle.cert_pem)?;
        Ok(Some((bundle, not_after)))
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
        Ok(bytes) => match serde_json::from_slice::<BackoffState>(&bytes) {
            Ok(state) => state,
            Err(parse_err) => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let nonce: u32 = rand::random();
                let quarantine =
                    path.with_file_name(format!("{BACKOFF_FILE}.corrupt.{ts}.{nonce:08x}"));
                if let Err(rename_err) = tokio::fs::rename(&path, &quarantine).await {
                    error!(
                        path = %path.display(),
                        error = %parse_err,
                        rename_error = %rename_err,
                        "backoff.json is corrupt (parse failed); rename for quarantine also failed — resetting to default"
                    );
                } else {
                    error!(
                        path = %path.display(),
                        quarantine = %quarantine.display(),
                        error = %parse_err,
                        "backoff.json is corrupt (parse failed); quarantined and resetting to default"
                    );
                }
                BackoffState::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BackoffState::default(),
        Err(e) => {
            error!(path = %path.display(), error = %e, "failed to read backoff.json; resetting to default");
            BackoffState::default()
        }
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
// Bundle validation
// ---------------------------------------------------------------------------

/// Returns `Ok(())` if `bundle.cert_pem` covers every domain in
/// `config.domains` and `bundle.key_pem` is mathematically paired with the
/// cert's public key (SPKI).
///
/// Returns `Err(AcmeError::OrderFailed(reason))` on the first check that
/// fails; the reason names the specific check so operators can distinguish a
/// CA-side problem from a config-side one.
fn validate_bundle(config: &AcmeConfig, bundle: &CertBundle) -> Result<(), AcmeError> {
    // Parse the certificate once and extract SPKI DER for the key-consistency
    // check below.  Both checks share a single parse.  The parsed cert borrows
    // from the PEM document, so everything is scoped together.
    let cert_spki_der: Vec<u8> = {
        let (_, pem_doc) = x509_parser::pem::parse_x509_pem(&bundle.cert_pem)
            .map_err(|e| AcmeError::OrderFailed(format!("cert.pem PEM parse failed: {e}")))?;
        let cert = pem_doc
            .parse_x509()
            .map_err(|e| AcmeError::OrderFailed(format!("cert.pem X.509 parse failed: {e}")))?;

        // ── SAN coverage ─────────────────────────────────────────────────────
        let cert_dns_names: std::collections::HashSet<String> = cert
            .subject_alternative_name()
            .ok()
            .flatten()
            .map(|ext| {
                ext.value
                    .general_names
                    .iter()
                    .filter_map(|gn| {
                        if let x509_parser::extensions::GeneralName::DNSName(name) = gn {
                            Some(name.to_lowercase())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let configured_lc: Vec<String> = config
            .domains
            .iter()
            .map(|d| d.to_ascii_lowercase())
            .collect();

        let missing: Vec<&str> = configured_lc
            .iter()
            .zip(config.domains.iter())
            .filter(|(lc, _)| !cert_dns_names.contains(lc.as_str()))
            .map(|(_, orig)| orig.as_str())
            .collect();

        if !missing.is_empty() {
            return Err(AcmeError::OrderFailed(format!(
                "cert SANs missing configured domain(s): {}",
                missing.join(", ")
            )));
        }

        // ── SPKI extraction (used for key-consistency check below) ────────────
        cert.subject_pki.raw.to_vec()
    };

    // ── Key↔cert consistency ──────────────────────────────────────────────────
    let key_str = std::str::from_utf8(&bundle.key_pem)
        .map_err(|_| AcmeError::OrderFailed("key.pem is not valid UTF-8".into()))?;
    let key_pair = rcgen::KeyPair::from_pem(key_str)
        .map_err(|e| AcmeError::OrderFailed(format!("key.pem parse failed: {e}")))?;
    if key_pair.public_key_der() != cert_spki_der {
        return Err(AcmeError::OrderFailed(
            "key.pem public key does not match cert.pem".into(),
        ));
    }

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
            issuance_timeout_seconds: 120,
        }
    }

    /// Generate a self-signed cert whose `not_after` is `not_after_unix`.
    ///
    /// `sans` is the list of DNS Subject Alternative Names to embed.  Pass the
    /// configured `domains` when the cert will be loaded through
    /// `load_cached_bundle` so that the SAN coverage check passes.
    fn generate_cert(not_after_unix: i64, sans: &[&str]) -> (Vec<u8>, Vec<u8>) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        let not_after = time::OffsetDateTime::from_unix_timestamp(not_after_unix).unwrap();
        params.not_before = not_after - Duration::from_secs(90 * 86_400);
        params.not_after = not_after;
        params.subject_alt_names = sans
            .iter()
            .map(|s| rcgen::SanType::DnsName((*s).try_into().unwrap()))
            .collect();
        let cert = params.self_signed(&key).unwrap();
        (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
    }

    #[test]
    fn inspect_state_returns_no_cert_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let summary = inspect_state(tmp.path(), "example.test", 10);
        assert_eq!(summary, StateSummary::NoCertCached);
    }

    #[test]
    fn inspect_state_returns_cached_when_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let renewal_window_days = 10u64;
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let not_after = now_unix + 30 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), key_pem).unwrap();

        let summary = inspect_state(tmp.path(), "example.test", renewal_window_days);
        match summary {
            StateSummary::CertCached {
                not_after_unix,
                days_until_renewal,
            } => {
                assert_eq!(not_after_unix as i64, not_after);
                assert!(
                    (19..=20).contains(&days_until_renewal),
                    "days_until_renewal={days_until_renewal} not in expected range",
                );
            }
            other => panic!("expected CertCached, got {other:?}"),
        }
    }

    #[test]
    fn inspect_state_returns_invalid_when_unparseable() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("cert.pem"), b"not a cert").unwrap();
        std::fs::write(tmp.path().join("key.pem"), b"not a key").unwrap();

        let summary = inspect_state(tmp.path(), "example.test", 10);
        match summary {
            StateSummary::CertCachedButInvalid { reason } => {
                assert!(
                    reason.to_ascii_lowercase().contains("parse"),
                    "reason should mention parse failure, got: {reason}"
                );
            }
            other => panic!("expected CertCachedButInvalid, got {other:?}"),
        }
    }

    #[test]
    fn inspect_state_returns_invalid_when_key_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, _) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), cert_pem).unwrap();

        let summary = inspect_state(tmp.path(), "example.test", 10);
        match summary {
            StateSummary::CertCachedButInvalid { reason } => {
                assert!(reason.contains("missing key file"));
            }
            other => panic!("expected CertCachedButInvalid, got {other:?}"),
        }
    }

    #[test]
    fn inspect_state_returns_invalid_when_cert_read_fails() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("cert.pem")).unwrap();
        std::fs::write(tmp.path().join("key.pem"), b"not a key").unwrap();

        let summary = inspect_state(tmp.path(), "example.test", 10);
        match summary {
            StateSummary::CertCachedButInvalid { reason } => {
                assert!(reason.contains("failed to read cert file"));
            }
            other => panic!("expected CertCachedButInvalid, got {other:?}"),
        }
    }

    #[test]
    fn inspect_state_returns_invalid_when_key_read_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, _) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), cert_pem).unwrap();
        std::fs::create_dir(tmp.path().join("key.pem")).unwrap();

        let summary = inspect_state(tmp.path(), "example.test", 10);
        match summary {
            StateSummary::CertCachedButInvalid { reason } => {
                assert!(reason.contains("failed to read key file"));
            }
            other => panic!("expected CertCachedButInvalid, got {other:?}"),
        }
    }

    #[test]
    fn inspect_state_returns_invalid_when_cert_expired() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, key_pem) = generate_cert(now_unix - 86_400, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), key_pem).unwrap();

        let summary = inspect_state(tmp.path(), "example.test", 10);
        match summary {
            StateSummary::CertCachedButInvalid { reason } => {
                assert!(reason.contains("cert expired for domain example.test"));
            }
            other => panic!("expected CertCachedButInvalid, got {other:?}"),
        }
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

        fn published_names(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
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
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);
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
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

        let new_not_after = now_unix + 90 * 86_400;
        let (new_cert, new_key) = generate_cert(new_not_after, &["a.example.test"]);

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
        let (new_cert, new_key) = generate_cert(new_not_after, &["a.example.test"]);

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
        assert!(metrics.contains("envoy_acme_account_state:a.example.test:0"));
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
        let (new_cert, new_key) = generate_cert(new_not_after, &["a.example.test"]);

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
        assert!(
            metrics.contains("envoy_acme_account_state:a.example.test:0"),
            "rate-limited error must set account_state=0 (not account-level)"
        );
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
        let (new_cert, new_key) = generate_cert(new_not_after, &["a.example.test"]);

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
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);
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
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);
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
    // SAN-coverage and key-consistency tests
    // ────────────────────────────────────────────────────────────────────────

    // Helper: returns a DevNull sink wrapped in a Box.
    fn dev_null_sink() -> Box<dyn crate::cert_sink::CertSink> {
        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }
        Box::new(DevNull)
    }

    /// A cert whose SANs cover MORE than the configured domains is still
    /// accepted (configured is a strict subset of cert SANs).
    #[tokio::test]
    async fn load_cached_bundle_ok_when_cert_sans_are_superset_of_configured() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        // Cert covers two names; test_config only requests "a.example.test".
        let (cert_pem, key_pem) = generate_cert(
            now_unix + 90 * 86_400,
            &["a.example.test", "b.example.test"],
        );
        let bundle = CertBundle { cert_pem, key_pem };
        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            dev_null_sink(),
            Box::new(MockIssuer::with_results(vec![])),
        );

        sm.persist_bundle(&bundle).await.unwrap();
        let loaded = sm.load_cached_bundle().await.unwrap();

        assert!(loaded.is_some(), "superset SANs should be accepted");
    }

    /// A cert whose SANs exactly match the two configured domains must be
    /// accepted — the exact-match multi-domain case.
    #[tokio::test]
    async fn load_cached_bundle_ok_with_exact_multi_domain_match() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();

        let mut cfg = test_config(tmp.path());
        cfg.domains = vec!["a.example.test".into(), "b.example.test".into()];

        // Cert covers exactly the two configured domains.
        let (cert_pem, key_pem) = generate_cert(
            now_unix + 90 * 86_400,
            &["a.example.test", "b.example.test"],
        );
        let bundle = CertBundle { cert_pem, key_pem };
        let sm = AcmeStateMachine::new_with_issuer(
            cfg,
            dev_null_sink(),
            Box::new(MockIssuer::with_results(vec![])),
        );

        sm.persist_bundle(&bundle).await.unwrap();
        let loaded = sm.load_cached_bundle().await.unwrap();

        assert!(
            loaded.is_some(),
            "cert with SANs exactly matching configured domains must be accepted"
        );
    }

    /// A cert whose SANs cover FEWER names than configured must be rejected.
    #[tokio::test]
    #[traced_test]
    async fn load_cached_bundle_none_when_cert_sans_missing_configured_domain() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();

        // Build a config that requires two domains.
        let mut cfg = test_config(tmp.path());
        cfg.domains = vec!["a.example.test".into(), "b.example.test".into()];

        // But the cert only covers one of them.
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);
        let bundle = CertBundle {
            cert_pem: cert_pem.clone(),
            key_pem,
        };

        // Write cert and key files manually with a sentinel matching the cert hash.
        std::fs::write(tmp.path().join(CERT_FILE), &cert_pem).unwrap();
        std::fs::write(tmp.path().join(KEY_FILE), bundle.key_pem.clone()).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

        let sm = AcmeStateMachine::new_with_issuer(
            cfg,
            dev_null_sink(),
            Box::new(MockIssuer::with_results(vec![])),
        );

        let loaded = sm.load_cached_bundle().await.unwrap();
        assert!(
            loaded.is_none(),
            "cert missing a configured SAN must be rejected"
        );
        assert!(
            logs_contain("cert SANs missing configured domain(s)"),
            "warn log must name the missing domain(s)"
        );
    }

    /// A cert whose public key does not match the stored private key must be
    /// rejected with an appropriate log message.
    #[tokio::test]
    #[traced_test]
    async fn load_cached_bundle_none_when_key_does_not_match_cert() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();

        let (cert_pem, _cert_key_pem) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);
        // Generate a *different* key pair — public key won't match cert.
        let (_other_cert_pem, mismatched_key_pem) =
            generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);

        // Write the cert with a mismatched key file; the sentinel only covers
        // the cert bytes, so it still passes the hash check.
        std::fs::write(tmp.path().join(CERT_FILE), &cert_pem).unwrap();
        std::fs::write(tmp.path().join(KEY_FILE), &mismatched_key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            dev_null_sink(),
            Box::new(MockIssuer::with_results(vec![])),
        );

        let loaded = sm.load_cached_bundle().await.unwrap();
        assert!(loaded.is_none(), "cert/key mismatch must cause rejection");
        assert!(
            logs_contain("key.pem public key does not match cert.pem"),
            "warn log must describe the key mismatch"
        );
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

    #[tokio::test]
    async fn force_renew_uses_tick_and_updates_not_after() {
        const STALE_NOT_AFTER_UNIX: i64 = 1;

        let tmp = tempfile::tempdir().unwrap();
        let not_after = time::OffsetDateTime::now_utc().unix_timestamp() + 90 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);

        let sink = std::sync::Arc::new(MockCertSink::default());

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let issuer = Box::new(MockIssuer::always_ok(cert_pem, key_pem));
        let mut sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(ArcSink(sink.clone())),
            issuer,
        );
        sm.last_not_after_unix = Some(STALE_NOT_AFTER_UNIX);

        sm.force_renew().await.unwrap();

        assert_eq!(sink.call_count(), 1);
        assert_ne!(sm.last_not_after_unix, Some(STALE_NOT_AFTER_UNIX));
        assert!(sm.last_not_after_unix.is_some());
    }

    #[traced_test]
    #[tokio::test]
    async fn issuance_with_unparseable_cert_fails_validation_and_returns_error() {
        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Ok(CertBundle {
            cert_pem: b"not a cert".to_vec(),
            key_pem: b"not a key".to_vec(),
        })]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        let err = sm.tick_at(1_000_000).await.unwrap_err();

        // Validation catches the bad bundle before persist/publish.
        assert!(
            matches!(err, AcmeError::OrderFailed(ref msg) if msg.contains("cert.pem")),
            "expected OrderFailed about cert parse, got: {err:?}"
        );
        assert!(
            logs_contain("issued bundle failed validation"),
            "error log must mention validation failure"
        );
        // Nothing persisted.
        assert!(
            !tmp.path().join(CERT_FILE).exists(),
            "cert.pem must not be written for an invalid bundle"
        );
    }

    /// A MockIssuer returning a bundle whose cert SANs are missing a configured
    /// domain must cause `tick_at` to return `Err(AcmeError::OrderFailed(_))`
    /// naming the missing domain, and must not persist or publish anything.
    #[traced_test]
    #[tokio::test]
    async fn tick_at_rejects_bundle_with_missing_san() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;

        // Config requires two domains; cert only covers one.
        let mut cfg = test_config(tmp.path());
        cfg.domains = vec!["a.example.test".into(), "b.example.test".into()];

        let not_after = now_unix + 90 * 86_400;
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);

        let sink = std::sync::Arc::new(MockCertSink::default());

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Ok(CertBundle {
            cert_pem,
            key_pem,
        })]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(cfg, Box::new(ArcSink(sink.clone())), issuer);

        let err = sm.tick_at(now_unix).await.unwrap_err();

        assert!(
            matches!(err, AcmeError::OrderFailed(ref msg) if msg.contains("b.example.test")),
            "error must name the missing domain, got: {err:?}"
        );
        assert!(
            logs_contain("issued bundle failed validation"),
            "error log must mention validation failure"
        );
        // Nothing persisted.
        assert!(
            !tmp.path().join(CERT_FILE).exists(),
            "cert.pem must not be written for a bundle with missing SANs"
        );
        assert!(
            !tmp.path().join(SENTINEL_FILE).exists(),
            "sentinel must not be written for a bundle with missing SANs"
        );
        // Sink not called.
        assert_eq!(
            sink.call_count(),
            0,
            "sink must not be called for a bundle with missing SANs"
        );
    }

    /// A MockIssuer returning a bundle whose `key_pem` does not match
    /// `cert_pem` must cause `tick_at` to return `Err(AcmeError::OrderFailed(_))`
    /// describing the key mismatch, and must not persist or publish anything.
    #[traced_test]
    #[tokio::test]
    async fn tick_at_rejects_bundle_with_mismatched_key() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;

        let not_after = now_unix + 90 * 86_400;
        let (cert_pem, _) = generate_cert(not_after, &["a.example.test"]);
        let (_, mismatched_key_pem) = generate_cert(not_after, &["a.example.test"]);

        let sink = std::sync::Arc::new(MockCertSink::default());

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Ok(CertBundle {
            cert_pem,
            key_pem: mismatched_key_pem,
        })]));
        let mut sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            Box::new(ArcSink(sink.clone())),
            issuer,
        );

        let err = sm.tick_at(now_unix).await.unwrap_err();

        assert!(
            matches!(err, AcmeError::OrderFailed(ref msg) if msg.contains("does not match")),
            "error must describe the key mismatch, got: {err:?}"
        );
        assert!(
            logs_contain("issued bundle failed validation"),
            "error log must mention validation failure"
        );
        // Nothing persisted.
        assert!(
            !tmp.path().join(CERT_FILE).exists(),
            "cert.pem must not be written for a bundle with a mismatched key"
        );
        assert!(
            !tmp.path().join(SENTINEL_FILE).exists(),
            "sentinel must not be written for a bundle with a mismatched key"
        );
        // Sink not called.
        assert_eq!(
            sink.call_count(),
            0,
            "sink must not be called for a bundle with a mismatched key"
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn non_rate_limited_error_does_not_escalate_backoff() {
        // Intentionally hold the global metrics test lock across the async tick
        // so metric updates remain isolated from other tests.
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Err(AcmeError::OrderFailed(
            "network down".into(),
        ))]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        let err = sm.tick_at(1_000_000).await.unwrap_err();

        assert!(matches!(err, AcmeError::OrderFailed(ref msg) if msg == "network down"));
        assert_eq!(sm.backoff, BackoffState::default());
        assert!(!tmp.path().join(BACKOFF_FILE).exists());

        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(metrics.contains("envoy_acme_issuance_total:failure"));
        assert!(metrics.contains("envoy_acme_consecutive_failures:a.example.test:0"));
        assert!(metrics.contains("envoy_acme_next_retry_at_seconds:a.example.test:0"));
    }

    fn account_does_not_exist_error() -> AcmeError {
        let problem: instant_acme::Problem = serde_json::from_value(serde_json::json!({
            "type": "urn:ietf:params:acme:error:accountDoesNotExist",
            "detail": "account not found on server"
        }))
        .unwrap();
        AcmeError::Protocol(instant_acme::Error::Api(problem))
    }

    fn malformed_error() -> AcmeError {
        let problem: instant_acme::Problem = serde_json::from_value(serde_json::json!({
            "type": "urn:ietf:params:acme:error:malformed",
            "detail": "request was malformed"
        }))
        .unwrap();
        AcmeError::Protocol(instant_acme::Error::Api(problem))
    }

    #[allow(clippy::await_holding_lock)]
    // The metrics test lock is a std::sync::MutexGuard, which is !Send.  It is
    // intentionally held across the async tick to serialise metric updates
    // across tests.  All our tests use #[tokio::test] which runs on a
    // current-thread scheduler, so the guard never crosses a thread boundary.
    #[tokio::test]
    async fn permanent_error_emits_permanent_metric_label() {
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Err(malformed_error())]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        assert!(sm.tick_at(1_000_000).await.is_err());

        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(
            metrics.contains("envoy_acme_issuance_total:permanent"),
            "expected 'permanent' label but got: {metrics:?}"
        );
        assert!(
            !metrics.contains("envoy_acme_issuance_total:failure"),
            "permanent error must not emit 'failure' label"
        );
    }

    #[allow(clippy::await_holding_lock)]
    // See permanent_error_emits_permanent_metric_label for why the suppression
    // is safe here.
    #[tokio::test]
    async fn recovery_required_error_emits_recovery_required_metric_label() {
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Err(
            account_does_not_exist_error(),
        )]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        assert!(sm.tick_at(1_000_000).await.is_err());

        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(
            metrics.contains("envoy_acme_issuance_total:recovery_required"),
            "expected 'recovery_required' label but got: {metrics:?}"
        );
        assert!(
            !metrics.contains("envoy_acme_issuance_total:failure"),
            "recovery_required error must not emit 'failure' label"
        );
        assert!(
            metrics.contains("envoy_acme_account_state:a.example.test:1"),
            "expected account_state=1 for recovery_required but got: {metrics:?}"
        );
    }

    #[allow(clippy::await_holding_lock)]
    // See permanent_error_emits_permanent_metric_label for why the suppression
    // is safe here.
    #[tokio::test]
    async fn successful_issuance_after_recovery_required_clears_account_state() {
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let new_not_after = now_unix + 90 * 86_400;
        let (new_cert, new_key) = generate_cert(new_not_after, &["a.example.test"]);

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![
            Err(account_does_not_exist_error()),
            Ok(CertBundle {
                cert_pem: new_cert,
                key_pem: new_key,
            }),
        ]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        // Tick 1: recovery_required → gauge = 1.
        assert!(sm.tick_at(now_unix).await.is_err());
        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(
            metrics.contains("envoy_acme_account_state:a.example.test:1"),
            "expected account_state=1 after first (recovery_required) tick but got: {metrics:?}"
        );

        // Tick 2: success → gauge drops back to 0.
        sm.tick_at(now_unix + 1).await.unwrap();
        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(
            metrics.contains("envoy_acme_account_state:a.example.test:0"),
            "expected account_state=0 after successful tick but got: {metrics:?}"
        );
    }

    #[allow(clippy::await_holding_lock)]
    // See permanent_error_emits_permanent_metric_label for why the suppression
    // is safe here.
    #[traced_test]
    #[tokio::test]
    async fn recovery_required_log_is_rate_limited_to_once_per_60s() {
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![
            Err(account_does_not_exist_error()),
            Err(account_does_not_exist_error()),
        ]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        // First tick: should log.
        assert!(sm.tick_at(1_000_000).await.is_err());
        // Second tick immediately after: should NOT log again (< 60 s elapsed).
        assert!(sm.tick_at(1_000_001).await.is_err());

        logs_assert(|lines: &[&str]| {
            let count = lines
                .iter()
                .filter(|line| line.contains("ACME account recovery required"))
                .count();
            match count {
                1 => Ok(()),
                n => Err(format!(
                    "expected exactly one recovery-required log within 60 s, got {n}"
                )),
            }
        });
    }

    #[traced_test]
    #[tokio::test]
    async fn invalid_backoff_json_quarantines_file_and_logs_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(BACKOFF_FILE), b"{not-json").unwrap();

        let state = load_backoff(tmp.path()).await;

        // Falls back to default.
        assert_eq!(state, BackoffState::default());
        // Original file is gone (renamed to quarantine file).
        assert!(
            !tmp.path().join(BACKOFF_FILE).exists(),
            "backoff.json should have been renamed away"
        );
        // A quarantine file matching the pattern was created.
        let corrupt_files: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("backoff.json.corrupt.")
            })
            .collect();
        assert_eq!(
            corrupt_files.len(),
            1,
            "expected exactly one quarantine file"
        );
        // Error was logged.
        assert!(
            logs_contain("backoff.json is corrupt"),
            "expected an error log about corrupt backoff.json"
        );
    }

    #[tokio::test]
    async fn missing_backoff_json_defaults_silently() {
        let tmp = tempfile::tempdir().unwrap();
        // No backoff.json written — first-run case.

        let state = load_backoff(tmp.path()).await;

        assert_eq!(state, BackoffState::default());
    }

    #[tokio::test]
    async fn publish_uses_default_domain_when_domains_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);

        let sink = std::sync::Arc::new(MockCertSink::default());

        struct ArcSink(std::sync::Arc<MockCertSink>);
        impl CertSink for ArcSink {
            fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
                self.0.publish(name, bundle)
            }
        }

        let mut config = test_config(tmp.path());
        config.domains.clear();

        let issuer = Box::new(MockIssuer::always_ok(cert_pem, key_pem));
        let mut sm =
            AcmeStateMachine::new_with_issuer(config, Box::new(ArcSink(sink.clone())), issuer);

        sm.tick_at(now_unix).await.unwrap();

        assert_eq!(sink.published_names(), vec!["default".to_string()]);
    }

    #[traced_test]
    #[tokio::test]
    async fn successful_issuance_emits_heartbeat_when_due() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::always_ok(cert_pem, key_pem));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);
        sm.heartbeat_every_ticks = 1;

        sm.tick_at(now_unix).await.unwrap();

        assert!(logs_contain("envoy-acme heartbeat"));
    }

    #[traced_test]
    #[tokio::test]
    async fn issuance_error_emits_heartbeat_when_due() {
        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![Err(AcmeError::OrderFailed(
            "network down".into(),
        ))]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);
        sm.heartbeat_every_ticks = 1;

        assert!(sm.tick_at(1_000_000).await.is_err());

        assert!(logs_contain("envoy-acme heartbeat"));
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
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

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
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

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
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

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

    #[traced_test]
    #[tokio::test]
    async fn heartbeat_blocked_by_backoff_reports_next_attempt() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let next_retry_at = now_unix + 3600;
        std::fs::write(
            tmp.path().join(BACKOFF_FILE),
            serde_json::to_vec(&BackoffState {
                consecutive_failures: 2,
                next_retry_at: Some(next_retry_at),
            })
            .unwrap(),
        )
        .unwrap();

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
        assert!(logs_contain(&format!(
            "next_attempt_at_unix=Some({next_retry_at})"
        )));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn cached_cert_emits_not_after_metric() {
        // Intentionally hold the global metrics test lock across the async tick
        // so metric updates remain isolated from other tests.
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();
        let now_unix = 1_000_000i64;
        let not_after = now_unix + 90 * 86_400; // outside the 30-day window
        let (cert_pem, key_pem) = generate_cert(not_after, &["a.example.test"]);
        std::fs::write(tmp.path().join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tmp.path().join("key.pem"), &key_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let issuer = Box::new(MockIssuer::with_results(vec![]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);

        sm.tick_at(now_unix).await.unwrap();

        let metrics: std::collections::HashSet<_> =
            crate::metrics::take_test_updates().into_iter().collect();
        assert!(metrics.contains(&format!(
            "envoy_acme_cert_not_after_seconds:a.example.test:{}",
            not_after
        )));
    }

    // ── Issuance timeout ────────────────────────────────────────────────────

    /// A `MockIssuer` whose `issue` future never resolves.
    struct HangingIssuer;

    impl Issuer for HangingIssuer {
        fn issue<'a>(
            &'a self,
            _config: &'a AcmeConfig,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<CertBundle, AcmeError>> + Send + 'a>>
        {
            Box::pin(std::future::pending())
        }
    }

    /// tick_at returns AcmeError::Timeout when the issuer hangs forever.
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn issuance_timeout_returns_timeout_error() {
        // Hold the metrics lock so metric updates stay isolated.
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let mut cfg = test_config(tmp.path());
        // Use a very short timeout so the test finishes quickly.
        cfg.issuance_timeout_seconds = 5;

        let mut sm =
            AcmeStateMachine::new_with_issuer(cfg, Box::new(DevNull), Box::new(HangingIssuer));

        let err = sm.tick_at(1_000_000).await.unwrap_err();
        assert!(
            matches!(err, AcmeError::Timeout),
            "expected AcmeError::Timeout, got {err:?}"
        );
    }

    /// A timeout records a failure metric but does not escalate backoff.
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn issuance_timeout_records_failure_metric_and_no_backoff_escalation() {
        let _guard = crate::metrics::test_lock();
        crate::metrics::reset_test_state();

        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        let mut cfg = test_config(tmp.path());
        cfg.issuance_timeout_seconds = 5;

        let mut sm =
            AcmeStateMachine::new_with_issuer(cfg, Box::new(DevNull), Box::new(HangingIssuer));

        let _ = sm.tick_at(1_000_000).await;

        // Backoff must not have escalated.
        assert_eq!(sm.backoff, BackoffState::default());
        assert!(!tmp.path().join(BACKOFF_FILE).exists());

        // A failure counter increment must have been recorded.
        let metrics: HashSet<_> = crate::metrics::take_test_updates().into_iter().collect();
        assert!(
            metrics.contains("envoy_acme_issuance_total:failure"),
            "expected failure metric; got {metrics:?}"
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // emit_heartbeat: checked_add overflow path
    //
    // When `now_unix` is close to `i64::MAX`, adding `tick_seconds` overflows
    // `i64::checked_add`, so `next_attempt_at_unix` is `None` in the log.
    // ────────────────────────────────────────────────────────────────────────

    #[traced_test]
    #[tokio::test]
    async fn emit_heartbeat_with_overflowing_now_unix() {
        let tmp = tempfile::tempdir().unwrap();

        struct DevNull;
        impl CertSink for DevNull {
            fn publish(&self, _: &str, _: &CertBundle) -> Result<(), SinkError> {
                Ok(())
            }
        }

        // Immediate error issuer so the tick completes quickly.
        let issuer = Box::new(MockIssuer::with_results(vec![Err(AcmeError::OrderFailed(
            "mock".into(),
        ))]));
        let mut sm =
            AcmeStateMachine::new_with_issuer(test_config(tmp.path()), Box::new(DevNull), issuer);
        // Force heartbeat on every tick.
        sm.heartbeat_every_ticks = 1;

        // i64::MAX - 1 + tick_seconds (60) overflows i64::checked_add → None.
        let _ = sm.tick_at(i64::MAX - 1).await;

        // The heartbeat log must show next_attempt_at_unix=None (overflow path).
        assert!(
            logs_contain("next_attempt_at_unix=None"),
            "heartbeat must log None when checked_add overflows"
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // load_cached_bundle: sentinel I/O error (non-NotFound)
    //
    // When the sentinel path exists but is not readable (e.g. it is a directory
    // rather than a regular file), tokio::fs::read returns an Err whose kind is
    // not NotFound, so load_cached_bundle must propagate it as Err(AcmeError).
    // ────────────────────────────────────────────────────────────────────────

    #[cfg(unix)]
    #[tokio::test]
    async fn load_cached_bundle_err_on_sentinel_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, key_pem) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);

        tokio::fs::write(tmp.path().join(CERT_FILE), &cert_pem)
            .await
            .unwrap();
        tokio::fs::write(tmp.path().join(KEY_FILE), &key_pem)
            .await
            .unwrap();
        // Create the sentinel path as a *directory* — reading it returns an I/O
        // error whose kind is IsADirectory (not NotFound).
        tokio::fs::create_dir(tmp.path().join(SENTINEL_FILE))
            .await
            .unwrap();

        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            dev_null_sink(),
            Box::new(MockIssuer::with_results(vec![])),
        );

        let result = sm.load_cached_bundle().await;
        assert!(
            result.is_err(),
            "non-NotFound sentinel I/O error must propagate as Err"
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // load_cached_bundle / validate_bundle: key.pem not valid UTF-8
    // ────────────────────────────────────────────────────────────────────────

    #[traced_test]
    #[tokio::test]
    async fn load_cached_bundle_none_when_key_not_utf8() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, _) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);

        std::fs::write(tmp.path().join(CERT_FILE), &cert_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();
        // Write a key that is not valid UTF-8.
        std::fs::write(tmp.path().join(KEY_FILE), b"\xff\xff\xff").unwrap();

        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            dev_null_sink(),
            Box::new(MockIssuer::with_results(vec![])),
        );

        let loaded = sm.load_cached_bundle().await.unwrap();
        assert!(
            loaded.is_none(),
            "non-UTF-8 key.pem must cause the bundle to be rejected"
        );
        assert!(
            logs_contain("not valid UTF-8"),
            "warn log must mention the UTF-8 error"
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // load_cached_bundle / validate_bundle: key.pem parse failed
    // ────────────────────────────────────────────────────────────────────────

    #[traced_test]
    #[tokio::test]
    async fn load_cached_bundle_none_when_key_parse_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let (cert_pem, _) = generate_cert(now_unix + 90 * 86_400, &["a.example.test"]);

        std::fs::write(tmp.path().join(CERT_FILE), &cert_pem).unwrap();
        std::fs::write(tmp.path().join(SENTINEL_FILE), sha256_hex(&cert_pem)).unwrap();
        // Valid UTF-8 but not valid PEM — rcgen::KeyPair::from_pem will fail.
        std::fs::write(tmp.path().join(KEY_FILE), b"not a valid pem key").unwrap();

        let sm = AcmeStateMachine::new_with_issuer(
            test_config(tmp.path()),
            dev_null_sink(),
            Box::new(MockIssuer::with_results(vec![])),
        );

        let loaded = sm.load_cached_bundle().await.unwrap();
        assert!(
            loaded.is_none(),
            "invalid PEM key must cause the bundle to be rejected"
        );
        assert!(
            logs_contain("key.pem parse failed"),
            "warn log must mention the parse failure"
        );
    }
}
