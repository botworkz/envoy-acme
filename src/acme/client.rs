use async_trait::async_trait;
use instant_acme::{
    Account, Authorization, Challenge, NewOrder, Order, OrderState,
};

#[async_trait]
pub(crate) trait AcmeAccount: Send + Sync {
    async fn new_order(
        &self,
        req: &NewOrder<'_>,
    ) -> Result<Box<dyn AcmeOrder + Send>, instant_acme::Error>;
}

#[async_trait]
pub(crate) trait AcmeOrder: Send {
    async fn authorizations(&mut self) -> Result<Vec<Authorization>, instant_acme::Error>;
    /// Returns the key-authorization string for an HTTP-01 challenge.
    ///
    /// The trait returns `String` rather than `instant_acme::KeyAuthorization`
    /// so mock implementations can supply test values without going through
    /// the SDK type (which has no public constructor).
    fn key_authorization(&self, challenge: &Challenge) -> String;
    async fn set_challenge_ready(&mut self, url: &str) -> Result<(), instant_acme::Error>;
    async fn refresh(&mut self) -> Result<&OrderState, instant_acme::Error>;
    async fn finalize(&mut self, csr_der: &[u8]) -> Result<(), instant_acme::Error>;
    async fn certificate(&mut self) -> Result<Option<String>, instant_acme::Error>;
}

pub(crate) struct RealAcmeAccount<'a>(pub(crate) &'a Account);

pub(crate) struct RealAcmeOrder(Order);

#[async_trait]
impl AcmeAccount for RealAcmeAccount<'_> {
    async fn new_order(
        &self,
        req: &NewOrder<'_>,
    ) -> Result<Box<dyn AcmeOrder + Send>, instant_acme::Error> {
        Ok(Box::new(RealAcmeOrder(self.0.new_order(req).await?)))
    }
}

#[async_trait]
impl AcmeOrder for RealAcmeOrder {
    async fn authorizations(&mut self) -> Result<Vec<Authorization>, instant_acme::Error> {
        self.0.authorizations().await
    }

    fn key_authorization(&self, challenge: &Challenge) -> String {
        self.0.key_authorization(challenge).as_str().to_owned()
    }

    async fn set_challenge_ready(&mut self, url: &str) -> Result<(), instant_acme::Error> {
        self.0.set_challenge_ready(url).await
    }

    async fn refresh(&mut self) -> Result<&OrderState, instant_acme::Error> {
        self.0.refresh().await
    }

    async fn finalize(&mut self, csr_der: &[u8]) -> Result<(), instant_acme::Error> {
        self.0.finalize(csr_der).await
    }

    async fn certificate(&mut self) -> Result<Option<String>, instant_acme::Error> {
        self.0.certificate().await
    }
}
