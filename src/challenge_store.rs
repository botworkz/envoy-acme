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

#[cfg(test)]
mod ttl_override {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    /// Sentinel meaning "use the production default".
    const UNSET: u64 = u64::MAX;

    static TEST_TTL_MILLIS: AtomicU64 = AtomicU64::new(UNSET);

    /// Override the challenge TTL for the duration of a single test.
    /// Call [`reset`] in the same test to restore the production default.
    pub fn set(d: Duration) {
        TEST_TTL_MILLIS.store(
            u64::try_from(d.as_millis()).expect("test TTL must fit in u64"),
            Ordering::Relaxed,
        );
    }

    /// Restore the TTL to the production default after a test override.
    pub fn reset() {
        TEST_TTL_MILLIS.store(UNSET, Ordering::Relaxed);
    }

    /// Return the currently active TTL (override if set, otherwise `None`).
    pub fn get() -> Option<Duration> {
        let v = TEST_TTL_MILLIS.load(Ordering::Relaxed);
        if v != UNSET {
            Some(Duration::from_millis(v))
        } else {
            None
        }
    }
}

/// Returns the effective TTL: the test override when set, otherwise
/// [`CHALLENGE_TTL`].
fn effective_ttl() -> Duration {
    #[cfg(test)]
    if let Some(d) = ttl_override::get() {
        return d;
    }
    CHALLENGE_TTL
}

pub fn init() -> ChallengeMap {
    STORE
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

pub fn get() -> ChallengeMap {
    init()
}

/// Insert a challenge token.  Stale entries (older than the effective TTL)
/// are swept from the map before the new entry is added, keeping the store
/// bounded.
pub fn insert(token: String, key_authorization: String) {
    let store = get();
    let ttl = effective_ttl();
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

pub fn remove(token: &str) {
    get().write().remove(token);
}

/// Look up a challenge token.
///
/// Returns `None` immediately if the entry is absent or has exceeded the TTL.
/// An expired entry is removed inline.  The common (non-expired) path holds
/// only a read lock; the write lock is acquired only when eviction is needed.
pub fn lookup(token: &str) -> Option<String> {
    let store = get();
    let ttl = effective_ttl();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_lookup_remove_roundtrip() {
        insert("token-a".to_string(), "key-auth".to_string());
        assert_eq!(lookup("token-a"), Some("key-auth".to_string()));
        remove("token-a");
        assert_eq!(lookup("token-a"), None);
    }

    #[test]
    fn entry_is_returned_before_ttl_expires() {
        ttl_override::set(Duration::from_millis(200));
        let token = "token-ttl-valid";
        insert(token.to_string(), "key-auth-valid".to_string());
        // Immediate lookup must succeed (well within TTL).
        assert_eq!(lookup(token), Some("key-auth-valid".to_string()));
        remove(token);
        ttl_override::reset();
    }

    #[test]
    fn entry_returns_none_after_ttl_expires() {
        ttl_override::set(Duration::from_millis(50));
        let token = "token-ttl-expired";
        insert(token.to_string(), "key-auth-expired".to_string());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(lookup(token), None);
        // Confirm the entry has been removed from the map.
        assert!(!get().read().contains_key(token));
        ttl_override::reset();
    }

    #[test]
    fn insert_sweeps_expired_entries() {
        ttl_override::set(Duration::from_millis(50));
        let old_token = "token-sweep-old";
        let new_token = "token-sweep-new";
        insert(old_token.to_string(), "key-auth-old".to_string());
        std::thread::sleep(Duration::from_millis(100));
        // Inserting a new entry must sweep the expired old entry.
        insert(new_token.to_string(), "key-auth-new".to_string());
        assert!(
            !get().read().contains_key(old_token),
            "expired entry should be swept on insert"
        );
        remove(new_token);
        ttl_override::reset();
    }
}
