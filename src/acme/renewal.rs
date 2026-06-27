use crate::errors::AcmeError;

pub fn needs_renewal_at(not_after_unix: i64, now_unix: i64, window_days: u64) -> bool {
    let window_secs = (window_days as i64) * 24 * 60 * 60;
    now_unix + window_secs >= not_after_unix
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
}
