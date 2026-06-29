//! Per-domain rate-limit backoff for the ACME renewal loop.
//!
//! ## Error classification
//!
//! `classify_acme_error` inspects an [`AcmeError`] and returns an
//! [`ErrorClass`]:
//!
//! - [`ErrorClass::RateLimited`] — the ACME server returned a problem
//!   document whose `type` ends with `rateLimited`
//!   (`urn:ietf:params:acme:error:rateLimited`), or whose embedded HTTP
//!   status is 429.  These trigger exponential back-off.
//!
//! - [`ErrorClass::Transient`] — any other protocol error (network hiccup,
//!   bad-nonce retry exhausted, order timeout, etc.).  The caller can retry
//!   on the next ordinary tick without a long back-off.
//!
//! - [`ErrorClass::Permanent`] — errors that cannot be resolved by retrying
//!   (e.g. key-generation failure, sink I/O error).  Currently unused for
//!   backoff decisions; the tick returns the error to the caller as usual.
//!
//! **Limitation**: `instant-acme` wraps every non-2xx response body as a
//! `Problem` document.  If the ACME server returns HTTP 429 *without* a
//! valid JSON problem body, `instant-acme` will produce a JSON parse error
//! (`AcmeError::Protocol(Error::Json(…))`) rather than `Error::Api(…)`.  In
//! that uncommon case this classifier returns `Transient` rather than
//! `RateLimited`.  Real Let's Encrypt always sends a valid problem document
//! on 429, so this is not expected to matter in practice.
//!
//! ## Backoff schedule
//!
//! Base delay: 60 s.  Multiplier: 2× per consecutive failure.  Cap: 24 h.
//! ±20 % jitter applied to each computed delay (random, using
//! `rand::thread_rng`; across restarts is fine for a back-off).
//!
//! State is serialised to `backoff.json` in the domain's `state_dir` so
//! that `next_retry_at` is respected across process restarts.

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::errors::AcmeError;

/// How an [`AcmeError`] should be classified for retry scheduling.
#[derive(Debug, PartialEq, Eq)]
pub enum ErrorClass {
    /// ACME rate-limit; triggers exponential back-off.
    RateLimited,
    /// Transient error; retry on the next ordinary tick.
    Transient,
    /// Permanent error that won't be resolved by retrying.
    Permanent,
}

/// Inspect `e` and return the appropriate [`ErrorClass`].
pub fn classify_acme_error(e: &AcmeError) -> ErrorClass {
    match e {
        AcmeError::Protocol(inner) => classify_protocol_error(inner),
        // Sink / IO / cert-gen errors are permanent in the sense that
        // they are not ACME-server-side; don't trigger the long back-off.
        AcmeError::Sink(_) | AcmeError::Io(_) | AcmeError::CertGen(_) => ErrorClass::Permanent,
        // JSON parse, order failures, missing challenge: transient enough
        // to retry next tick without a long delay.
        _ => ErrorClass::Transient,
    }
}

fn classify_protocol_error(e: &instant_acme::Error) -> ErrorClass {
    match e {
        instant_acme::Error::Api(problem) => classify_problem(problem),
        // Hyper / HTTP transport errors → transient.
        _ => ErrorClass::Transient,
    }
}

fn classify_problem(p: &instant_acme::Problem) -> ErrorClass {
    // RFC 8555 §6.7 uses urn:ietf:params:acme:error:rateLimited.
    // Check both exact match and suffix to be forward-compatible.
    if let Some(t) = &p.r#type {
        if t.ends_with("rateLimited") {
            return ErrorClass::RateLimited;
        }
    }
    // Some servers embed the HTTP status directly in the problem document.
    if p.status == Some(429) {
        return ErrorClass::RateLimited;
    }
    ErrorClass::Transient
}

// ---------------------------------------------------------------------------
// Backoff state
// ---------------------------------------------------------------------------

const BASE_DELAY_SECS: i64 = 60;
const MAX_DELAY_SECS: i64 = 24 * 60 * 60; // 24 h
const JITTER_FRACTION: f64 = 0.20; // ±20 %

/// Compute the Unix timestamp at which the next retry is allowed.
///
/// - `now_unix`: current time (Unix seconds).
/// - `consecutive`: 0-based failure index (0 → first failure, 1 → second, …).
///
/// Returns `now_unix + delay` where `delay = clamp(60 * 2^consecutive, 60, 86400)`,
/// with ±20 % random jitter.
pub fn compute_next_retry_at(now_unix: i64, consecutive: u32) -> i64 {
    // Saturate the exponent so we don't overflow.
    let exp = consecutive.min(20);
    let delay = BASE_DELAY_SECS
        .saturating_mul(1i64.checked_shl(exp).unwrap_or(i64::MAX))
        .min(MAX_DELAY_SECS);

    let jitter_max = ((delay as f64) * JITTER_FRACTION) as i64;
    let jitter: i64 = if jitter_max > 0 {
        rand::thread_rng().gen_range(-jitter_max..=jitter_max)
    } else {
        0
    };

    now_unix + (delay + jitter).max(1)
}

/// Per-domain rate-limit backoff state, persisted to `backoff.json`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BackoffState {
    /// How many consecutive rate-limit failures have occurred.
    pub consecutive_failures: u32,
    /// Unix timestamp before which issuance must not be attempted again.
    /// `None` means no active backoff.
    pub next_retry_at: Option<i64>,
}

impl BackoffState {
    /// Returns `true` if the domain is currently in its back-off window.
    pub fn is_blocked(&self, now_unix: i64) -> bool {
        self.next_retry_at.is_some_and(|t| now_unix < t)
    }

    /// Record one rate-limit failure and update `next_retry_at`.
    pub fn record_rate_limit(&mut self, now_unix: i64) {
        // The exponent is 0-based (first failure → 60 s, second → 120 s, …).
        let exp = self.consecutive_failures; // current count before increment
        self.consecutive_failures += 1;
        self.next_retry_at = Some(compute_next_retry_at(now_unix, exp));
    }

    /// Clear the back-off state after a successful issuance.
    pub fn clear(&mut self) {
        self.consecutive_failures = 0;
        self.next_retry_at = None;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rate_limited_error() -> AcmeError {
        // Build a Problem document with the rate-limit type URN.
        let problem: instant_acme::Problem = serde_json::from_value(serde_json::json!({
            "type": "urn:ietf:params:acme:error:rateLimited",
            "detail": "too many requests",
            "status": 429
        }))
        .unwrap();
        AcmeError::Protocol(instant_acme::Error::Api(problem))
    }

    fn make_bad_nonce_error() -> AcmeError {
        let problem: instant_acme::Problem = serde_json::from_value(serde_json::json!({
            "type": "urn:ietf:params:acme:error:badNonce",
            "detail": "nonce stale"
        }))
        .unwrap();
        AcmeError::Protocol(instant_acme::Error::Api(problem))
    }

    #[test]
    fn classify_rate_limited_urn() {
        assert_eq!(
            classify_acme_error(&make_rate_limited_error()),
            ErrorClass::RateLimited,
        );
    }

    #[test]
    fn classify_rate_limited_via_status_only() {
        let problem: instant_acme::Problem = serde_json::from_value(serde_json::json!({
            "status": 429
        }))
        .unwrap();
        let e = AcmeError::Protocol(instant_acme::Error::Api(problem));
        assert_eq!(classify_acme_error(&e), ErrorClass::RateLimited);
    }

    #[test]
    fn classify_other_problem_is_transient() {
        assert_eq!(
            classify_acme_error(&make_bad_nonce_error()),
            ErrorClass::Transient,
        );
    }

    #[test]
    fn classify_io_error_is_permanent() {
        let e = AcmeError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        assert_eq!(classify_acme_error(&e), ErrorClass::Permanent);
    }

    #[test]
    fn backoff_state_is_blocked() {
        let mut s = BackoffState::default();
        assert!(!s.is_blocked(1000));
        s.record_rate_limit(1000);
        // next_retry_at must be > 1000
        assert!(s.next_retry_at.unwrap() > 1000);
        assert!(s.is_blocked(1001));
        assert!(!s.is_blocked(s.next_retry_at.unwrap() + 1));
    }

    #[test]
    fn backoff_clear_resets_state() {
        let mut s = BackoffState::default();
        s.record_rate_limit(1000);
        s.clear();
        assert_eq!(s, BackoffState::default());
    }
}
