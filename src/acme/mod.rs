pub mod account;
pub mod order;
pub mod renewal;

use tracing::{error, info, instrument};

use crate::cert_sink::{CertBundle, CertSink};
use crate::challenge_store;
use crate::config::AcmeConfig;
use crate::errors::AcmeError;

const ACCOUNT_FILE: &str = "account.json";
const CERT_FILE: &str = "cert.pem";
const KEY_FILE: &str = "key.pem";

pub struct AcmeStateMachine {
    config: AcmeConfig,
    sink: Box<dyn CertSink>,
    last_not_after_unix: Option<i64>,
}

impl AcmeStateMachine {
    pub fn new(config: AcmeConfig, sink: Box<dyn CertSink>) -> Self {
        Self {
            config,
            sink,
            last_not_after_unix: None,
        }
    }

    #[instrument(skip(self), fields(domain = %self.config.domains.first().cloned().unwrap_or_default()))]
    pub async fn tick(&mut self) -> Result<(), AcmeError> {
        tokio::fs::create_dir_all(&self.config.state_dir).await?;

        let cached = self.load_cached_bundle().await?;
        if let Some((bundle, not_after)) = cached {
            if !renewal::needs_renewal_at(
                not_after,
                time::OffsetDateTime::now_utc().unix_timestamp(),
                self.config.renewal_window_days,
            ) {
                self.last_not_after_unix = Some(not_after);
                self.publish("cached", &bundle)?;
                return Ok(());
            }
        }

        let account = account::load_or_create_account(
            &self.config.directory_uri,
            &self.config.contact,
            &self.config.state_dir.join(ACCOUNT_FILE),
        )
        .await?;

        let bundle = order::issue_certificate(&self.config, &account).await?;

        self.persist_bundle(&bundle).await?;
        self.publish("issued", &bundle)?;

        match renewal::cert_not_after_unix(&bundle.cert_pem) {
            Ok(v) => self.last_not_after_unix = Some(v),
            Err(e) => error!(%e, "unable to parse not_after from issued certificate"),
        }

        Ok(())
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
