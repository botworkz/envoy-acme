use instant_acme::{
    Authorization, AuthorizationStatus, Challenge, ChallengeType, Identifier, NewOrder, OrderStatus,
};
use rcgen::{CertificateParams, KeyPair};
use tokio::time::{sleep, Duration};

use crate::acme::client::AcmeAccount;
use crate::cert_sink::CertBundle;
use crate::challenge_store;
use crate::config::AcmeConfig;
use crate::errors::AcmeError;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_POLLS: usize = 30;

/// Result of inspecting a single authorization returned by the ACME server.
#[derive(Debug)]
enum AuthorizationAction<'a> {
    /// Authorization is already valid; nothing to do for this domain.
    Skip,
    /// Authorization is pending; the caller must register this HTTP-01 challenge.
    Register { challenge: &'a Challenge },
}

/// Decision returned by `classify_poll_status` for each iteration of the
/// readiness-polling loop.
#[derive(Debug)]
enum PollDecision {
    /// The order has reached a terminal success state; stop polling.
    Done,
    /// The order has failed irrecoverably; stop polling.
    Abort,
    /// The order is still in-progress; sleep and retry.
    Wait,
}

/// Map `config.domains` to the `Vec<Identifier>` required by `NewOrder`.
fn build_identifiers(domains: &[String]) -> Vec<Identifier> {
    domains.iter().cloned().map(Identifier::Dns).collect()
}

/// Inspect a single `Authorization` and decide what action `issue_certificate`
/// should take for the corresponding domain.
///
/// # Errors
/// - Returns `AcmeError::OrderFailed` when the authorization is in an
///   unexpected state (not `Pending` or `Valid`).
/// - Returns `AcmeError::NoChallenge` when the authorization is `Pending` but
///   contains no `http-01` challenge.
fn evaluate_authorization(authz: &Authorization) -> Result<AuthorizationAction<'_>, AcmeError> {
    let Identifier::Dns(domain) = &authz.identifier;

    if !matches!(
        authz.status,
        AuthorizationStatus::Pending | AuthorizationStatus::Valid
    ) {
        return Err(AcmeError::OrderFailed(format!(
            "authorization for {domain} in state {:?}",
            authz.status
        )));
    }

    if matches!(authz.status, AuthorizationStatus::Valid) {
        return Ok(AuthorizationAction::Skip);
    }

    // Status is Pending — find the http-01 challenge.
    let challenge = authz
        .challenges
        .iter()
        .find(|c| c.r#type == ChallengeType::Http01)
        .ok_or_else(|| AcmeError::NoChallenge(domain.clone()))?;

    Ok(AuthorizationAction::Register { challenge })
}

/// Classify an `OrderStatus` seen during the readiness-polling loop.
fn classify_poll_status(status: OrderStatus) -> PollDecision {
    match status {
        OrderStatus::Ready | OrderStatus::Valid => PollDecision::Done,
        OrderStatus::Invalid => PollDecision::Abort,
        _ => PollDecision::Wait,
    }
}

/// Generate a fresh key pair and serialize a CSR for the given domains.
///
/// Returns `(key_pair, csr_der)` where `csr_der` is the DER-encoded
/// certificate-signing request suitable for passing to `Order::finalize`.
fn build_csr(domains: &[String]) -> Result<(KeyPair, Vec<u8>), AcmeError> {
    let key_pair = KeyPair::generate()?;
    let params = CertificateParams::new(domains.to_vec())?;
    let csr = params.serialize_request(&key_pair)?;
    Ok((key_pair, csr.der().to_vec()))
}

/// Build the final `CertBundle` from the downloaded certificate chain and key
/// pair.
///
/// # Errors
/// Returns `AcmeError::OrderFailed` when `cert_chain` is `None`, meaning the
/// polling loop exhausted all retries without receiving a certificate.
fn assemble_bundle(
    cert_chain: Option<Vec<u8>>,
    key_pair: KeyPair,
) -> Result<CertBundle, AcmeError> {
    let cert_pem = cert_chain.ok_or_else(|| {
        AcmeError::OrderFailed("timed out waiting for certificate download".to_string())
    })?;
    Ok(CertBundle {
        cert_pem,
        key_pem: key_pair.serialize_pem().into_bytes(),
    })
}

// TODO(sdk): envoy bootstrap send_http_callout exists, but instant-acme currently owns request
// formatting/signing and transport lifecycle. v0 keeps instant-acme's client on a dedicated runtime
// thread and uses the in-process HTTP filter for HTTP-01 challenge responses.
pub async fn issue_certificate(
    config: &AcmeConfig,
    account: &dyn AcmeAccount,
) -> Result<CertBundle, AcmeError> {
    let identifiers = build_identifiers(&config.domains);

    let mut order = account
        .new_order(&NewOrder {
            identifiers: &identifiers,
        })
        .await?;

    let mut challenge_tokens = Vec::new();
    let authorizations = order.authorizations().await?;

    for authz in &authorizations {
        match evaluate_authorization(authz)? {
            AuthorizationAction::Skip => continue,
            AuthorizationAction::Register { challenge } => {
                let key_auth = order.key_authorization(challenge).as_str().to_owned();
                challenge_store::insert(challenge.token.clone(), key_auth);
                challenge_tokens.push(challenge.token.clone());
                order.set_challenge_ready(&challenge.url).await?;
            }
        }
    }

    let mut ready = false;
    for _ in 0..MAX_POLLS {
        let state = order.refresh().await?;
        match classify_poll_status(state.status) {
            PollDecision::Done => {
                ready = true;
                break;
            }
            PollDecision::Abort => break,
            PollDecision::Wait => sleep(POLL_INTERVAL).await,
        }
    }

    if !ready {
        for token in &challenge_tokens {
            challenge_store::remove(token);
        }
        return Err(AcmeError::OrderFailed(
            "timed out waiting for ready authorization".to_string(),
        ));
    }

    let (key_pair, csr_der) = build_csr(&config.domains)?;
    order.finalize(&csr_der).await?;

    let mut cert_chain = None;
    for _ in 0..MAX_POLLS {
        if let Some(chain) = order.certificate().await? {
            cert_chain = Some(chain.into_bytes());
            break;
        }
        sleep(POLL_INTERVAL).await;
    }

    for token in &challenge_tokens {
        challenge_store::remove(token);
    }

    assemble_bundle(cert_chain, key_pair)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use instant_acme::{Authorization, ChallengeType, Identifier, OrderState};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    // ── helpers for constructing Authorization fixtures via serde ──────────

    fn make_authz(status: &str, domain: &str, challenge_types: &[&str]) -> Authorization {
        let challenges: Vec<serde_json::Value> = challenge_types
            .iter()
            .enumerate()
            .map(|(i, t)| {
                serde_json::json!({
                    "type": t,
                    "url": format!("https://acme.test/chal/{i}"),
                    "token": format!("token-{i}"),
                    "status": "pending"
                })
            })
            .collect();

        let raw = serde_json::json!({
            "status": status,
            "expires": "2099-01-01T00:00:00Z",
            "identifier": {"type": "dns", "value": domain},
            "challenges": challenges
        });
        serde_json::from_value(raw).expect("fixture must deserialise")
    }

    // ── build_identifiers ─────────────────────────────────────────────────

    #[test]
    fn build_identifiers_empty() {
        let ids = build_identifiers(&[]);
        assert!(ids.is_empty());
    }

    #[test]
    fn build_identifiers_single() {
        let ids = build_identifiers(&["example.com".to_owned()]);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], Identifier::Dns("example.com".to_owned()));
    }

    #[test]
    fn build_identifiers_multiple() {
        let domains = ["a.test", "b.test", "c.test"].map(str::to_owned).to_vec();
        let ids = build_identifiers(&domains);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[2], Identifier::Dns("c.test".to_owned()));
    }

    // ── evaluate_authorization ────────────────────────────────────────────

    #[test]
    fn evaluate_authorization_valid_returns_skip() {
        let authz = make_authz("valid", "example.test", &[]);
        let action = evaluate_authorization(&authz).expect("should succeed");
        assert!(matches!(action, AuthorizationAction::Skip));
    }

    #[test]
    fn evaluate_authorization_pending_with_http01_returns_register() {
        let authz = make_authz("pending", "example.test", &["http-01", "dns-01"]);
        let action = evaluate_authorization(&authz).expect("should succeed");
        match action {
            AuthorizationAction::Register { challenge } => {
                assert_eq!(challenge.r#type, ChallengeType::Http01);
            }
            AuthorizationAction::Skip => panic!("expected Register, got Skip"),
        }
    }

    #[test]
    fn evaluate_authorization_pending_without_http01_returns_no_challenge() {
        let authz = make_authz("pending", "no-http.test", &["dns-01", "tls-alpn-01"]);
        let err = evaluate_authorization(&authz).expect_err("should fail");
        assert!(
            matches!(err, AcmeError::NoChallenge(ref d) if d == "no-http.test"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn evaluate_authorization_bad_status_returns_order_failed() {
        let authz = make_authz("revoked", "bad.test", &["http-01"]);
        let err = evaluate_authorization(&authz).expect_err("should fail");
        match err {
            AcmeError::OrderFailed(msg) => {
                assert!(
                    msg.contains("authorization for bad.test in state"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected OrderFailed, got {other:?}"),
        }
    }

    #[test]
    fn evaluate_authorization_error_message_exact_format() {
        // Verify the error string contains exactly the expected substring.
        let authz = make_authz("expired", "expired.test", &[]);
        let err = evaluate_authorization(&authz).expect_err("should fail");
        let AcmeError::OrderFailed(msg) = err else {
            panic!("expected OrderFailed");
        };
        assert!(
            msg.starts_with("authorization for expired.test in state "),
            "message was: {msg}"
        );
    }

    // ── classify_poll_status ──────────────────────────────────────────────

    #[test]
    fn classify_poll_status_ready_is_done() {
        assert!(matches!(
            classify_poll_status(OrderStatus::Ready),
            PollDecision::Done
        ));
    }

    #[test]
    fn classify_poll_status_valid_is_done() {
        assert!(matches!(
            classify_poll_status(OrderStatus::Valid),
            PollDecision::Done
        ));
    }

    #[test]
    fn classify_poll_status_invalid_is_abort() {
        assert!(matches!(
            classify_poll_status(OrderStatus::Invalid),
            PollDecision::Abort
        ));
    }

    #[test]
    fn classify_poll_status_pending_is_wait() {
        assert!(matches!(
            classify_poll_status(OrderStatus::Pending),
            PollDecision::Wait
        ));
    }

    #[test]
    fn classify_poll_status_processing_is_wait() {
        assert!(matches!(
            classify_poll_status(OrderStatus::Processing),
            PollDecision::Wait
        ));
    }

    // ── build_csr ─────────────────────────────────────────────────────────

    #[test]
    fn build_csr_returns_non_empty_der() {
        let (key_pair, csr_der) = build_csr(&["example.com".to_owned()]).expect("build_csr failed");
        assert!(!csr_der.is_empty(), "CSR DER must not be empty");
        // Sanity-check: key pair serialises to PEM without error.
        let pem = key_pair.serialize_pem();
        assert!(pem.contains("PRIVATE KEY"), "key PEM looks wrong: {pem}");
    }

    #[test]
    fn build_csr_multiple_domains() {
        let domains = ["a.test", "b.test"].map(str::to_owned).to_vec();
        let (_, csr_der) = build_csr(&domains).expect("build_csr failed");
        assert!(!csr_der.is_empty());
    }

    // ── assemble_bundle ───────────────────────────────────────────────────

    #[test]
    fn assemble_bundle_some_chain_produces_bundle() {
        let (key_pair, _) = build_csr(&["example.com".to_owned()]).unwrap();
        let cert_bytes = b"-----BEGIN CERTIFICATE-----\nfake\n-----END CERTIFICATE-----\n".to_vec();
        let bundle = assemble_bundle(Some(cert_bytes.clone()), key_pair)
            .expect("assemble_bundle should succeed");
        assert_eq!(bundle.cert_pem, cert_bytes);
        assert!(!bundle.key_pem.is_empty());
    }

    #[test]
    fn assemble_bundle_none_chain_returns_order_failed() {
        let (key_pair, _) = build_csr(&["example.com".to_owned()]).unwrap();
        let err = assemble_bundle(None, key_pair).expect_err("should fail");
        match err {
            AcmeError::OrderFailed(msg) => {
                assert!(
                    msg.contains("timed out waiting for certificate download"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected OrderFailed, got {other:?}"),
        }
    }

    struct MockAcmeAccount {
        new_order_result: Mutex<
            Option<Result<Box<dyn crate::acme::client::AcmeOrder + Send>, instant_acme::Error>>,
        >,
    }

    #[async_trait]
    impl crate::acme::client::AcmeAccount for MockAcmeAccount {
        async fn new_order(
            &self,
            _req: &NewOrder<'_>,
        ) -> Result<Box<dyn crate::acme::client::AcmeOrder + Send>, instant_acme::Error> {
            self.new_order_result
                .lock()
                .expect("mutex lock")
                .take()
                .expect("new_order called once")
        }
    }

    struct MockAcmeOrder {
        authorizations_response: Option<Result<Vec<Authorization>, instant_acme::Error>>,
        refresh_statuses: VecDeque<OrderStatus>,
        refresh_state: OrderState,
        finalize_response: Option<Result<(), instant_acme::Error>>,
        certificate_responses: VecDeque<Result<Option<String>, instant_acme::Error>>,
    }

    impl MockAcmeOrder {
        fn with_statuses(statuses: Vec<OrderStatus>) -> Self {
            Self {
                authorizations_response: Some(Ok(vec![make_authz("valid", "example.test", &[])])),
                refresh_statuses: statuses.into(),
                refresh_state: OrderState {
                    status: OrderStatus::Pending,
                    authorizations: Vec::new(),
                    error: None,
                    finalize: "https://acme.test/finalize".to_string(),
                    certificate: Some("https://acme.test/cert".to_string()),
                },
                finalize_response: Some(Ok(())),
                certificate_responses: VecDeque::from([Ok(Some("CERT-CHAIN".to_string()))]),
            }
        }
    }

    #[async_trait]
    impl crate::acme::client::AcmeOrder for MockAcmeOrder {
        async fn authorizations(&mut self) -> Result<Vec<Authorization>, instant_acme::Error> {
            self.authorizations_response
                .take()
                .expect("authorizations called once")
        }

        fn key_authorization(&self, _challenge: &Challenge) -> instant_acme::KeyAuthorization {
            panic!("key_authorization should not be called in these tests")
        }

        async fn set_challenge_ready(&mut self, _url: &str) -> Result<(), instant_acme::Error> {
            Ok(())
        }

        async fn refresh(&mut self) -> Result<&OrderState, instant_acme::Error> {
            if let Some(status) = self.refresh_statuses.front().copied() {
                self.refresh_state.status = status;
                self.refresh_statuses.pop_front();
            }
            Ok(&self.refresh_state)
        }

        async fn finalize(&mut self, _csr_der: &[u8]) -> Result<(), instant_acme::Error> {
            self.finalize_response.take().expect("finalize called once")
        }

        async fn certificate(&mut self) -> Result<Option<String>, instant_acme::Error> {
            self.certificate_responses.pop_front().unwrap_or(Ok(None))
        }
    }

    fn make_config() -> AcmeConfig {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        AcmeConfig {
            directory_profile: None,
            directory_uri: "https://acme.test/directory".to_string(),
            directory_ca_file: None,
            contact: "mailto:test@example.test".to_string(),
            domains: vec!["example.test".to_string()],
            renewal_window_days: 30,
            state_dir: temp_dir.path().to_path_buf(),
            cert_sink: crate::config::CertSinkConfig {
                sink_type: "files".to_string(),
                cert_dir: temp_dir.path().to_path_buf(),
                layout: crate::config::Layout::PerDomain,
            },
            tick_seconds: 60,
            issuance_timeout_seconds: 120,
        }
    }

    #[tokio::test]
    async fn issue_certificate_success_returns_bundle() {
        let account = MockAcmeAccount {
            new_order_result: Mutex::new(Some(Ok(Box::new(MockAcmeOrder::with_statuses(vec![
                OrderStatus::Ready,
            ]))))),
        };
        let bundle = issue_certificate(&make_config(), &account)
            .await
            .expect("issue_certificate should succeed");
        assert_eq!(bundle.cert_pem, b"CERT-CHAIN".to_vec());
        assert!(!bundle.key_pem.is_empty());
    }

    #[tokio::test]
    async fn issue_certificate_new_order_error_is_propagated() {
        let account = MockAcmeAccount {
            new_order_result: Mutex::new(Some(Err(instant_acme::Error::Str("new-order-error")))),
        };
        let err = issue_certificate(&make_config(), &account)
            .await
            .expect_err("expected failure");
        assert!(matches!(
            err,
            AcmeError::Protocol(instant_acme::Error::Str("new-order-error"))
        ));
    }

    #[tokio::test]
    async fn issue_certificate_refresh_invalid_returns_not_ready_message() {
        let account = MockAcmeAccount {
            new_order_result: Mutex::new(Some(Ok(Box::new(MockAcmeOrder::with_statuses(vec![
                OrderStatus::Invalid,
            ]))))),
        };
        let err = issue_certificate(&make_config(), &account)
            .await
            .expect_err("expected failure");
        assert!(matches!(
            err,
            AcmeError::OrderFailed(ref m) if m == "timed out waiting for ready authorization"
        ));
    }

    #[tokio::test]
    async fn issue_certificate_finalize_error_is_propagated() {
        let mut order = MockAcmeOrder::with_statuses(vec![OrderStatus::Ready]);
        order.finalize_response = Some(Err(instant_acme::Error::Str("finalize-error")));
        let account = MockAcmeAccount {
            new_order_result: Mutex::new(Some(Ok(Box::new(order)))),
        };
        let err = issue_certificate(&make_config(), &account)
            .await
            .expect_err("expected failure");
        assert!(matches!(
            err,
            AcmeError::Protocol(instant_acme::Error::Str("finalize-error"))
        ));
    }

    #[tokio::test]
    async fn issue_certificate_certificate_error_is_propagated() {
        let mut order = MockAcmeOrder::with_statuses(vec![OrderStatus::Ready]);
        order.certificate_responses =
            VecDeque::from([Err(instant_acme::Error::Str("certificate-error"))]);
        let account = MockAcmeAccount {
            new_order_result: Mutex::new(Some(Ok(Box::new(order)))),
        };
        let err = issue_certificate(&make_config(), &account)
            .await
            .expect_err("expected failure");
        assert!(matches!(
            err,
            AcmeError::Protocol(instant_acme::Error::Str("certificate-error"))
        ));
    }

    /// Order goes through Processing → Ready, requiring one sleep in the
    /// readiness-polling loop.
    #[tokio::test]
    async fn issue_certificate_polling_eventually_ready() {
        let account = MockAcmeAccount {
            new_order_result: Mutex::new(Some(Ok(Box::new(MockAcmeOrder::with_statuses(vec![
                OrderStatus::Processing,
                OrderStatus::Ready,
            ]))))),
        };
        let bundle = issue_certificate(&make_config(), &account)
            .await
            .expect("should succeed after polling");
        assert_eq!(bundle.cert_pem, b"CERT-CHAIN".to_vec());
    }

    /// Certificate download returns None on the first poll and Some on the
    /// second, so the sleep inside the certificate-download loop is exercised.
    #[tokio::test]
    async fn issue_certificate_certificate_polling_eventually_some() {
        let mut order = MockAcmeOrder::with_statuses(vec![OrderStatus::Ready]);
        order.certificate_responses = VecDeque::from([
            Ok(None),                              // first poll: not yet available
            Ok(Some("CERT-CHAIN".to_string())),    // second poll: ready
        ]);
        let account = MockAcmeAccount {
            new_order_result: Mutex::new(Some(Ok(Box::new(order)))),
        };
        let bundle = issue_certificate(&make_config(), &account)
            .await
            .expect("should succeed on second certificate poll");
        assert_eq!(bundle.cert_pem, b"CERT-CHAIN".to_vec());
    }
}
