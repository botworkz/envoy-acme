//! ACME (RFC 8555) certificate manager built on [`instant_acme`].
//!
//! Responsibilities:
//! - Load or create an ACME account, persisting credentials under `state_dir`.
//! - Issue a single certificate covering all configured domains using the
//!   HTTP-01 challenge, coordinated with the [`ChallengeStore`] and the
//!   `ext_proc` service.
//! - Persist the issued chain/key to disk and publish them to the
//!   [`CertStore`] so the SDS service can push them to Envoy.
//! - Periodically renew certificates before they expire.
use std::path::{Path, PathBuf};
use std::time::Duration;

use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, NewAccount,
    NewOrder, OrderStatus,
};
use rcgen::{CertificateParams, KeyPair};
use tokio::time::sleep;
use tracing::{error, info, instrument, warn};

use crate::cert_store::CertStore;
use crate::challenge_store::ChallengeStore;
use crate::config::AcmeConfig;
use crate::errors::AcmeError;

const ACCOUNT_FILE: &str = "account.json";
const CERT_FILE: &str = "cert.pem";
const KEY_FILE: &str = "key.pem";

/// Interval between renewal checks.
const CHECK_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
/// How long to wait between polls of the ACME order/challenge state machine.
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// Maximum number of polls before giving up on a state transition.
const MAX_POLLS: usize = 20;

/// Decide whether a certificate expiring at `not_after_unix` should be renewed
/// given the current time `now_unix` and a renewal `window_days`.
fn needs_renewal_at(not_after_unix: i64, now_unix: i64, window_days: u64) -> bool {
    let window_secs = window_days as i64 * 24 * 60 * 60;
    now_unix + window_secs >= not_after_unix
}

/// Parse the leaf certificate's `notAfter` (as a Unix timestamp) from a PEM chain.
fn cert_not_after_unix(cert_pem: &[u8]) -> Result<i64, AcmeError> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem)
        .map_err(|e| AcmeError::Tls(format!("failed to parse certificate PEM: {e}")))?;
    let cert = pem
        .parse_x509()
        .map_err(|e| AcmeError::Tls(format!("failed to parse X.509 certificate: {e}")))?;
    Ok(cert.validity().not_after.timestamp())
}

/// The ACME manager.
pub struct AcmeManager {
    config: AcmeConfig,
    challenge_store: ChallengeStore,
    cert_store: CertStore,
}

impl AcmeManager {
    pub fn new(config: AcmeConfig, challenge_store: ChallengeStore, cert_store: CertStore) -> Self {
        Self {
            config,
            challenge_store,
            cert_store,
        }
    }

    fn account_path(&self) -> PathBuf {
        self.config.state_dir.join(ACCOUNT_FILE)
    }
    fn cert_path(&self) -> PathBuf {
        self.config.state_dir.join(CERT_FILE)
    }
    fn key_path(&self) -> PathBuf {
        self.config.state_dir.join(KEY_FILE)
    }

    /// Run the manager forever: ensure a valid certificate, then loop checking
    /// for renewal at a fixed interval.
    #[instrument(skip(self), name = "acme_manager")]
    pub async fn run(self) -> Result<(), AcmeError> {
        tokio::fs::create_dir_all(&self.config.state_dir).await?;

        // Publish any previously issued certificate immediately so Envoy can
        // start serving TLS before the (possibly slow) ACME flow completes.
        if let Some((cert, key)) = self.load_existing_bundle().await? {
            info!("loaded existing certificate from state directory");
            self.cert_store.update(cert, key);
        }

        let account = self.load_or_create_account().await?;

        loop {
            match self.ensure_certificate(&account).await {
                Ok(true) => info!("certificate issued/renewed"),
                Ok(false) => info!("certificate still valid; no action needed"),
                Err(err) => error!(%err, "certificate issuance failed; will retry"),
            }
            sleep(CHECK_INTERVAL).await;
        }
    }

    /// Load the persisted cert/key bundle from disk, if present.
    async fn load_existing_bundle(&self) -> Result<Option<(Vec<u8>, Vec<u8>)>, AcmeError> {
        let (cert_path, key_path) = (self.cert_path(), self.key_path());
        if !cert_path.exists() || !key_path.exists() {
            return Ok(None);
        }
        let cert = tokio::fs::read(&cert_path).await?;
        let key = tokio::fs::read(&key_path).await?;
        Ok(Some((cert, key)))
    }

    /// Load the ACME account from persisted credentials, or create a new one.
    #[instrument(skip(self), name = "load_account")]
    async fn load_or_create_account(&self) -> Result<Account, AcmeError> {
        let path = self.account_path();
        if path.exists() {
            let data = tokio::fs::read(&path).await?;
            let credentials: AccountCredentials = serde_json::from_slice(&data)?;
            let account = Account::from_credentials(credentials).await?;
            info!("restored existing ACME account");
            return Ok(account);
        }

        info!("creating new ACME account");
        let (account, credentials) = Account::create(
            &NewAccount {
                contact: &[self.config.contact.as_str()],
                terms_of_service_agreed: true,
                only_return_existing: false,
            },
            &self.config.directory_url,
            None,
        )
        .await?;

        let serialized = serde_json::to_vec_pretty(&credentials)?;
        write_private(&path, &serialized).await?;
        info!("persisted ACME account credentials");
        Ok(account)
    }

    /// Determine whether issuance/renewal is needed, and perform it if so.
    /// Returns `true` if a new certificate was issued.
    async fn ensure_certificate(&self, account: &Account) -> Result<bool, AcmeError> {
        if !self.should_issue().await? {
            return Ok(false);
        }
        self.issue_certificate(account).await?;
        Ok(true)
    }

    /// Whether a certificate should be issued now (missing or near expiry).
    async fn should_issue(&self) -> Result<bool, AcmeError> {
        let cert_path = self.cert_path();
        if !cert_path.exists() {
            return Ok(true);
        }
        let cert_pem = tokio::fs::read(&cert_path).await?;
        let not_after = cert_not_after_unix(&cert_pem)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        Ok(needs_renewal_at(
            not_after,
            now,
            self.config.renewal_window_days,
        ))
    }

    /// Run the full ACME order flow for all configured domains.
    #[instrument(skip(self, account), fields(domains = ?self.config.domains), name = "issue_certificate")]
    async fn issue_certificate(&self, account: &Account) -> Result<(), AcmeError> {
        let identifiers: Vec<Identifier> = self
            .config
            .domains
            .iter()
            .map(|d| Identifier::Dns(d.clone()))
            .collect();

        let mut order = account
            .new_order(&NewOrder {
                identifiers: &identifiers,
            })
            .await?;
        info!("created ACME order");

        let authorizations = order.authorizations().await?;
        let mut pending_tokens: Vec<(String, String)> = Vec::new();

        for authz in &authorizations {
            let Identifier::Dns(domain) = &authz.identifier;
            match authz.status {
                AuthorizationStatus::Valid => {
                    info!(%domain, "authorization already valid");
                    continue;
                }
                AuthorizationStatus::Pending => {}
                other => {
                    return Err(AcmeError::OrderFailed(format!(
                        "authorization for {domain} in unexpected state {other:?}"
                    )));
                }
            }

            let challenge = authz
                .challenges
                .iter()
                .find(|c| c.r#type == ChallengeType::Http01)
                .ok_or_else(|| AcmeError::NoChallenge(domain.clone()))?;

            let key_auth = order.key_authorization(challenge);
            self.challenge_store
                .insert(challenge.token.clone(), key_auth.as_str().to_string())
                .await;
            info!(%domain, token = %challenge.token, "registered HTTP-01 challenge");
            pending_tokens.push((challenge.url.clone(), challenge.token.clone()));
        }

        for (challenge_url, _) in &pending_tokens {
            order.set_challenge_ready(challenge_url).await?;
        }

        // Poll until the order is ready (all authorizations validated).
        self.wait_for_order_ready(&mut order).await?;

        // Generate a fresh key pair and CSR for the configured domains.
        let key_pair = KeyPair::generate()?;
        let params = CertificateParams::new(self.config.domains.clone())?;
        let csr = params.serialize_request(&key_pair)?;
        order.finalize(csr.der()).await?;
        info!("order finalized; awaiting certificate");

        let cert_chain_pem = self.wait_for_certificate(&mut order).await?;
        let private_key_pem = key_pair.serialize_pem();

        // Persist and publish.
        tokio::fs::write(self.cert_path(), cert_chain_pem.as_bytes()).await?;
        write_private(&self.key_path(), private_key_pem.as_bytes()).await?;
        self.cert_store
            .update(cert_chain_pem.into_bytes(), private_key_pem.into_bytes());

        // Clean up challenge tokens.
        for (_, token) in &pending_tokens {
            self.challenge_store.remove(token).await;
        }

        info!("certificate issued and published");
        Ok(())
    }

    async fn wait_for_order_ready(&self, order: &mut instant_acme::Order) -> Result<(), AcmeError> {
        for _ in 0..MAX_POLLS {
            let state = order.refresh().await?;
            match state.status {
                OrderStatus::Ready | OrderStatus::Valid => return Ok(()),
                OrderStatus::Invalid => {
                    return Err(AcmeError::OrderFailed("order became invalid".to_string()));
                }
                status => {
                    warn!(?status, "order not ready yet; polling");
                    sleep(POLL_INTERVAL).await;
                }
            }
        }
        Err(AcmeError::OrderFailed(
            "timed out waiting for order to become ready".to_string(),
        ))
    }

    async fn wait_for_certificate(
        &self,
        order: &mut instant_acme::Order,
    ) -> Result<String, AcmeError> {
        for _ in 0..MAX_POLLS {
            if let Some(cert) = order.certificate().await? {
                return Ok(cert);
            }
            sleep(POLL_INTERVAL).await;
        }
        Err(AcmeError::OrderFailed(
            "timed out waiting for certificate".to_string(),
        ))
    }
}

/// Write a file with owner-only permissions (best effort on Unix).
async fn write_private(path: &Path, data: &[u8]) -> Result<(), AcmeError> {
    tokio::fs::write(path, data).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(path, perms).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renews_when_inside_window() {
        let now = 1_000_000_000;
        let window_days = 30;
        // Expires in 10 days -> inside the 30 day window -> renew.
        let not_after = now + 10 * 24 * 60 * 60;
        assert!(needs_renewal_at(not_after, now, window_days));
    }

    #[test]
    fn does_not_renew_when_outside_window() {
        let now = 1_000_000_000;
        let window_days = 30;
        // Expires in 60 days -> outside the 30 day window -> keep.
        let not_after = now + 60 * 24 * 60 * 60;
        assert!(!needs_renewal_at(not_after, now, window_days));
    }

    #[test]
    fn renews_when_already_expired() {
        let now = 1_000_000_000;
        let not_after = now - 1;
        assert!(needs_renewal_at(not_after, now, 30));
    }
}
