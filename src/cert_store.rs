//! In-memory store for the current TLS certificate/key bundle with watch for SDS push.
use tokio::sync::watch;

/// A PEM-encoded certificate chain + private key pair.
#[derive(Clone, Debug, Default)]
pub struct CertBundle {
    /// PEM-encoded certificate chain.
    pub cert_chain_pem: Vec<u8>,
    /// PEM-encoded private key.
    pub private_key_pem: Vec<u8>,
    /// Monotonically increasing version number.
    pub version: u64,
}

/// Thread-safe cert store backed by a `tokio::sync::watch` channel.
#[derive(Clone)]
pub struct CertStore {
    sender: watch::Sender<CertBundle>,
    receiver: watch::Receiver<CertBundle>,
}

impl CertStore {
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(CertBundle::default());
        Self { sender, receiver }
    }

    /// Update the cert bundle and notify all watchers.
    pub fn update(&self, cert_chain_pem: Vec<u8>, private_key_pem: Vec<u8>) {
        let version = self.sender.borrow().version + 1;
        let _ = self.sender.send(CertBundle {
            cert_chain_pem,
            private_key_pem,
            version,
        });
    }

    /// Get a receiver to watch for cert updates.
    pub fn subscribe(&self) -> watch::Receiver<CertBundle> {
        self.receiver.clone()
    }

    /// Get the current cert bundle.
    pub fn current(&self) -> CertBundle {
        self.sender.borrow().clone()
    }
}

impl Default for CertStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_increments_version() {
        let store = CertStore::new();
        assert_eq!(store.current().version, 0);
        store.update(b"cert".to_vec(), b"key".to_vec());
        let bundle = store.current();
        assert_eq!(bundle.version, 1);
        assert_eq!(bundle.cert_chain_pem, b"cert");
        assert_eq!(bundle.private_key_pem, b"key");
    }

    #[tokio::test]
    async fn test_subscribe_notifies() {
        let store = CertStore::new();
        let mut rx = store.subscribe();
        store.update(b"c".to_vec(), b"k".to_vec());
        rx.changed().await.unwrap();
        assert_eq!(rx.borrow().version, 1);
    }
}
