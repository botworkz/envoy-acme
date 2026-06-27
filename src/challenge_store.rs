//! In-memory store mapping ACME HTTP-01 tokens to key authorizations.
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe in-memory store for ACME HTTP-01 challenge tokens.
#[derive(Clone, Default)]
pub struct ChallengeStore {
    inner: Arc<RwLock<HashMap<String, String>>>,
}

impl ChallengeStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a token → key_authorization mapping.
    pub async fn insert(&self, token: String, key_authorization: String) {
        self.inner.write().await.insert(token, key_authorization);
    }

    /// Look up a key authorization by token.
    pub async fn get(&self, token: &str) -> Option<String> {
        self.inner.read().await.get(token).cloned()
    }

    /// Remove a token after the challenge is complete.
    pub async fn remove(&self, token: &str) {
        self.inner.write().await.remove(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_get() {
        let store = ChallengeStore::new();
        store
            .insert("token1".to_string(), "keyauth1".to_string())
            .await;
        assert_eq!(store.get("token1").await, Some("keyauth1".to_string()));
        assert_eq!(store.get("missing").await, None);
    }

    #[tokio::test]
    async fn test_remove() {
        let store = ChallengeStore::new();
        store.insert("tok".to_string(), "auth".to_string()).await;
        store.remove("tok").await;
        assert_eq!(store.get("tok").await, None);
    }
}
