use crate::errors::AcmeError;

/// Returns `true` if the certificate should be renewed.
///
/// This is a thin wrapper around [`needs_renewal_at_with_domain_offset`] that
/// passes an empty domain string, resulting in zero offset (equivalent to the
/// original behaviour).  Prefer [`needs_renewal_at_with_domain_offset`] when a
/// domain name is available so that multiple certs spread their renewals across
/// the window.
#[allow(dead_code)]
pub fn needs_renewal_at(not_after_unix: i64, now_unix: i64, window_days: u64) -> bool {
    needs_renewal_at_with_domain_offset(not_after_unix, now_unix, window_days, "")
}

/// Returns `true` if the certificate for `domain` should be renewed.
///
/// The nominal renewal window starts `window_days * 86400` seconds before
/// `not_after_unix`.  A deterministic per-domain offset is applied *inside*
/// that window so that certs for different domains do not all fire at the very
/// leading edge simultaneously:
///
/// ```text
/// offset  = FNV-1a(domain) mod (window_days * 86400)
/// renew_at = not_after_unix - window_secs + offset
/// ```
///
/// The offset shifts the renewal trigger *later* into the window, so each
/// domain renews at a stable point spread across the full window length.
/// Because the offset is derived from the domain name hash (not randomness),
/// the decision is identical across restarts — no flap.
///
/// For a cert expiring at `E` with a 30-day window:
/// - Without offset every domain triggers renewal at `E − 30d`.
/// - With offset domain `"a.example"` might trigger at `E − 30d + 4h`,
///   `"b.example"` at `E − 30d + 11h`, etc.
pub fn needs_renewal_at_with_domain_offset(
    not_after_unix: i64,
    now_unix: i64,
    window_days: u64,
    domain: &str,
) -> bool {
    let window_secs = (window_days as i64).saturating_mul(86_400);
    let offset = if domain.is_empty() || window_secs == 0 {
        0i64
    } else {
        (fnv1a(domain) % (window_secs as u64)) as i64
    };
    // renew when: now >= not_after - window_secs + offset
    // i.e.:       now + window_secs - offset >= not_after
    now_unix + window_secs - offset >= not_after_unix
}

/// FNV-1a 64-bit hash of a UTF-8 string.
fn fnv1a(s: &str) -> u64 {
    const OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    s.bytes()
        .fold(OFFSET_BASIS, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME))
}

pub fn cert_not_after_unix(cert_pem: &[u8]) -> Result<i64, AcmeError> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem)
        .map_err(|e| AcmeError::OrderFailed(format!("failed parsing PEM certificate: {e}")))?;
    let cert = pem
        .parse_x509()
        .map_err(|e| AcmeError::OrderFailed(format!("failed parsing X.509 certificate: {e}")))?;
    Ok(cert.validity().not_after.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renewal_window_logic() {
        let now = 1_000_000;
        assert!(needs_renewal_at(now + 5 * 86_400, now, 30));
        assert!(!needs_renewal_at(now + 100 * 86_400, now, 30));
    }

    #[test]
    fn domain_offset_is_deterministic() {
        let not_after = 1_000_000 + 90 * 86_400;
        let window_days = 30u64;
        // Calling twice with the same inputs must return the same result.
        let r1 =
            needs_renewal_at_with_domain_offset(not_after, 1_000_000, window_days, "a.example");
        let r2 =
            needs_renewal_at_with_domain_offset(not_after, 1_000_000, window_days, "a.example");
        assert_eq!(r1, r2);
    }

    #[test]
    fn domain_offsets_span_window() {
        // Generate 100 distinct domains and collect their offsets.
        // More than half the window length must be covered (sanity check).
        let window_days = 30u64;
        let window_secs = window_days * 86_400;
        let offsets: Vec<u64> = (0..100u32)
            .map(|i| fnv1a(&format!("domain-{i}.example")) % window_secs)
            .collect();
        let min = offsets.iter().min().copied().unwrap();
        let max = offsets.iter().max().copied().unwrap();
        assert!(
            max - min > window_secs / 2,
            "offsets span only {span}s of a {window_secs}s window",
            span = max - min,
        );
    }
}
