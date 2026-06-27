use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub type ChallengeMap = Arc<RwLock<HashMap<String, String>>>;

static STORE: OnceLock<ChallengeMap> = OnceLock::new();

pub fn init() -> ChallengeMap {
    STORE
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

pub fn get() -> ChallengeMap {
    init()
}

pub fn insert(token: String, key_authorization: String) {
    get().write().insert(token, key_authorization);
}

pub fn remove(token: &str) {
    get().write().remove(token);
}

pub fn lookup(token: &str) -> Option<String> {
    get().read().get(token).cloned()
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
}
