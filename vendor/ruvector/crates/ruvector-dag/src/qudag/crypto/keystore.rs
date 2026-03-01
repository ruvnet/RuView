//! Secure Keystore with Zeroization

use super::identity::QuDagIdentity;
use std::collections::HashMap;
use zeroize::Zeroize;

pub struct SecureKeystore {
    identities: HashMap<String, QuDagIdentity>,
    master_key: Option<[u8; 32]>,
}

impl SecureKeystore {
    pub fn new() -> Self {
        Self {
            identities: HashMap::new(),
            master_key: None,
        }
    }

    pub fn with_master_key(key: [u8; 32]) -> Self {
        Self {
            identities: HashMap::new(),
            master_key: Some(key),
        }
    }

    pub fn add_identity(&mut self, identity: QuDagIdentity) {
        let id = identity.node_id.clone();
        self.identities.insert(id, identity);
    }

    pub fn get_identity(&self, node_id: &str) -> Option<&QuDagIdentity> {
        self.identities.get(node_id)
    }

    pub fn remove_identity(&mut self, node_id: &str) -> Option<QuDagIdentity> {
        self.identities.remove(node_id)
    }

    pub fn list_identities(&self) -> Vec<&str> {
        self.identities.keys().map(|s| s.as_str()).collect()
    }

    pub fn clear(&mut self) {
        self.identities.clear();
        if let Some(ref mut key) = self.master_key {
            key.zeroize();
        }
        self.master_key = None;
    }
}

impl Drop for SecureKeystore {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Default for SecureKeystore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    #[error("Identity not found")]
    IdentityNotFound,
    #[error("Keystore locked")]
    Locked,
    #[error("Storage error: {0}")]
    StorageError(String),
}
