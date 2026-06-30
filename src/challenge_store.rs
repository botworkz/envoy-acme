use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Default TTL for challenge entries.  HTTP-01 authorizations are valid for
/// ~7 days at Let's Encrypt; 10 minutes is generous for a round-trip that
/// normally completes in seconds, while still bounding the store size.
const CHALLENGE_TTL: Duration = Duration::from_secs(600);

pub(crate) struct Entry {
    pub(crate) key_authorization: String,
    pub(crate) created_at: Instant,
}

pub type ChallengeMap = Arc<RwLock<HashMap<String, Entry>>>;

static STORE: OnceLock<ChallengeMap> = OnceLock::new();

pub fn init() -> ChallengeMap {
    STORE
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

pub fn get() -> ChallengeMap {
    init()
}

/// Insert a challenge token into `store` using the given `ttl`.  Stale entries
/// (older than `ttl`) are swept from the map before the new entry is added,
/// keeping the store bounded.
fn insert_into(store: &ChallengeMap, token: String, key_authorization: String, ttl: Duration) {
    let now = Instant::now();
    let mut guard = store.write();
    guard.retain(|_, e| now.saturating_duration_since(e.created_at) <= ttl);
    guard.insert(
        token,
        Entry {
            key_authorization,
            created_at: now,
        },
    );
}

/// Remove `token` from `store`.
fn remove_from(store: &ChallengeMap, token: &str) {
    store.write().remove(token);
}

/// Look up `token` in `store` with the given `ttl`.
///
/// Returns `None` immediately if the entry is absent or has exceeded `ttl`.
/// An expired entry is removed inline.  The common (non-expired) path holds
/// only a read lock; the write lock is acquired only when eviction is needed.
fn lookup_in(store: &ChallengeMap, token: &str, ttl: Duration) -> Option<String> {
    // Fast path: read lock only.
    {
        let guard = store.read();
        match guard.get(token) {
            None => return None,
            Some(e) if e.created_at.elapsed() <= ttl => {
                return Some(e.key_authorization.clone());
            }
            Some(_) => {} // expired — fall through to write path
        }
    }

    // Slow path: entry is stale.  Re-check under write lock before removing
    // so a concurrent re-insert of the same token is not accidentally evicted.
    let mut guard = store.write();
    if guard
        .get(token)
        .is_some_and(|e| e.created_at.elapsed() > ttl)
    {
        guard.remove(token);
    }
    None
}

/// Insert a challenge token.  Stale entries (older than the effective TTL)
/// are swept from the map before the new entry is added, keeping the store
/// bounded.
pub fn insert(token: String, key_authorization: String) {
    insert_into(&get(), token, key_authorization, CHALLENGE_TTL);
}

/// Remove a challenge token.
pub fn remove(token: &str) {
    remove_from(&get(), token);
}

/// Look up a challenge token.
///
/// Returns `None` immediately if the entry is absent or has exceeded the TTL.
/// An expired entry is removed inline.  The common (non-expired) path holds
/// only a read lock; the write lock is acquired only when eviction is needed.
pub fn lookup(token: &str) -> Option<String> {
    lookup_in(&get(), token, CHALLENGE_TTL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> ChallengeMap {
        Arc::new(RwLock::new(HashMap::new()))
    }

    #[test]
    fn insert_lookup_remove_roundtrip() {
        let store = fresh_store();
        let ttl = Duration::from_secs(60);
        insert_into(&store, "token-a".to_string(), "key-auth".to_string(), ttl);
        assert_eq!(
            lookup_in(&store, "token-a", ttl),
            Some("key-auth".to_string())
        );
        remove_from(&store, "token-a");
        assert_eq!(lookup_in(&store, "token-a", ttl), None);
    }

    #[test]
    fn entry_is_returned_before_ttl_expires() {
        let store = fresh_store();
        let ttl = Duration::from_millis(200);
        let token = "token-ttl-valid";
        insert_into(&store, token.to_string(), "key-auth-valid".to_string(), ttl);
        // Immediate lookup must succeed (well within TTL).
        assert_eq!(
            lookup_in(&store, token, ttl),
            Some("key-auth-valid".to_string())
        );
    }

    #[test]
    fn entry_returns_none_after_ttl_expires() {
        let store = fresh_store();
        let ttl = Duration::from_millis(50);
        let token = "token-ttl-expired";
        insert_into(
            &store,
            token.to_string(),
            "key-auth-expired".to_string(),
            ttl,
        );
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(lookup_in(&store, token, ttl), None);
        // Confirm the entry has been removed from the map.
        assert!(!store.read().contains_key(token));
    }

    #[test]
    fn insert_sweeps_expired_entries() {
        let store = fresh_store();
        let ttl = Duration::from_millis(50);
        let old_token = "token-sweep-old";
        let new_token = "token-sweep-new";
        insert_into(
            &store,
            old_token.to_string(),
            "key-auth-old".to_string(),
            ttl,
        );
        std::thread::sleep(Duration::from_millis(100));
        // Inserting a new entry must sweep the expired old entry.
        insert_into(
            &store,
            new_token.to_string(),
            "key-auth-new".to_string(),
            ttl,
        );
        assert!(
            !store.read().contains_key(old_token),
            "expired entry should be swept on insert"
        );
    }
}
