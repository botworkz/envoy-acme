use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, NewOrder, OrderStatus,
};
use rcgen::{CertificateParams, KeyPair};
use tokio::time::{sleep, Duration};

use crate::cert_sink::CertBundle;
use crate::challenge_store;
use crate::config::AcmeConfig;
use crate::errors::AcmeError;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_POLLS: usize = 30;

// TODO(sdk): envoy bootstrap send_http_callout exists, but instant-acme currently owns request
// formatting/signing and transport lifecycle. v0 keeps instant-acme's client on a dedicated runtime
// thread and uses the in-process HTTP filter for HTTP-01 challenge responses.
pub async fn issue_certificate(
    config: &AcmeConfig,
    account: &Account,
) -> Result<CertBundle, AcmeError> {
    let identifiers: Vec<Identifier> = config
        .domains
        .iter()
        .cloned()
        .map(Identifier::Dns)
        .collect();

    let mut order = account
        .new_order(&NewOrder {
            identifiers: &identifiers,
        })
        .await?;

    let mut challenge_tokens = Vec::new();
    let authorizations = order.authorizations().await?;

    for authz in &authorizations {
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
            continue;
        }

        let challenge = authz
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or_else(|| AcmeError::NoChallenge(domain.clone()))?;

        let key_auth = order.key_authorization(challenge).as_str().to_owned();
        challenge_store::insert(challenge.token.clone(), key_auth);
        challenge_tokens.push(challenge.token.clone());
        order.set_challenge_ready(&challenge.url).await?;
    }

    let mut ready = false;
    for _ in 0..MAX_POLLS {
        let state = order.refresh().await?;
        match state.status {
            OrderStatus::Ready | OrderStatus::Valid => {
                ready = true;
                break;
            }
            OrderStatus::Invalid => {
                break;
            }
            _ => sleep(POLL_INTERVAL).await,
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

    let key_pair = KeyPair::generate()?;
    let params = CertificateParams::new(config.domains.clone())?;
    let csr = params.serialize_request(&key_pair)?;
    order.finalize(csr.der()).await?;

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

    let cert_pem = cert_chain.ok_or_else(|| {
        AcmeError::OrderFailed("timed out waiting for certificate download".to_string())
    })?;

    Ok(CertBundle {
        cert_pem,
        key_pem: key_pair.serialize_pem().into_bytes(),
    })
}
