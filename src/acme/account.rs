use std::path::Path;

use instant_acme::{Account, AccountCredentials, NewAccount};

use crate::errors::AcmeError;

pub async fn load_or_create_account(
    directory_uri: &str,
    contact: &str,
    path: &Path,
) -> Result<Account, AcmeError> {
    if path.exists() {
        let data = tokio::fs::read(path).await?;
        let credentials: AccountCredentials = serde_json::from_slice(&data)?;
        return Ok(Account::from_credentials(credentials).await?);
    }

    let (account, credentials) = Account::create(
        &NewAccount {
            contact: &[contact],
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        directory_uri,
        None,
    )
    .await?;

    let bytes = serde_json::to_vec_pretty(&credentials)?;
    tokio::fs::write(path, bytes).await?;
    Ok(account)
}
