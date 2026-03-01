//! Identity Manager - Ed25519 + X25519 Key Management
//!
//! Security principles:
//! - Ed25519 for signing (identity keys)
//! - X25519 for key exchange (session keys)
//! - Never trust pubkeys from envelopes - only from signed registry
//! - Per-sender nonce tracking with timestamps for expiry
//! - Separate send/receive counters

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use x25519_dalek::{StaticSecret, PublicKey as X25519PublicKey};
use hkdf::Hkdf;
use sha2::Sha256;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// Ed25519 key pair for identity/signing
#[derive(Clone)]
pub struct KeyPair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    pub fn public_key_base64(&self) -> String {
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.public_key_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
        let Ok(verifying_key) = VerifyingKey::from_bytes(public_key) else {
            return false;
        };
        let sig = Signature::from_bytes(signature);
        verifying_key.verify(message, &sig).is_ok()
    }
}

/// Registered member in the swarm (from signed registry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredMember {
    pub agent_id: String,
    #[serde(with = "hex::serde")]
    pub ed25519_public_key: [u8; 32],
    #[serde(with = "hex::serde")]
    pub x25519_public_key: [u8; 32],
    pub capabilities: Vec<String>,
    pub joined_at: u64,
    pub last_heartbeat: u64,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

impl RegisteredMember {
    /// Verify the registration signature
    /// Signature covers: agent_id || ed25519_key || x25519_key || capabilities || joined_at
    pub fn verify(&self) -> bool {
        let canonical = self.canonical_data();
        KeyPair::verify(&self.ed25519_public_key, &canonical, &self.signature)
    }

    /// Create canonical data for signing
    pub fn canonical_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(self.agent_id.as_bytes());
        data.push(0); // separator
        data.extend_from_slice(&self.ed25519_public_key);
        data.extend_from_slice(&self.x25519_public_key);
        for cap in &self.capabilities {
            data.extend_from_slice(cap.as_bytes());
            data.push(0);
        }
        data.extend_from_slice(&self.joined_at.to_le_bytes());
        data
    }
}

/// Per-sender nonce tracking entry
struct NonceEntry {
    timestamp: u64,
}

/// Identity Manager with registry-based trust
pub struct IdentityManager {
    /// Our Ed25519 identity keypair
    identity_key: KeyPair,
    /// Our X25519 key for ECDH
    x25519_secret: StaticSecret,
    x25519_public: X25519PublicKey,

    /// Derived session keys per peer (cache)
    session_keys: Arc<RwLock<HashMap<String, [u8; 32]>>>,

    /// Registry of verified members - THE SOURCE OF TRUTH
    /// Never trust keys from envelopes, only from here
    member_registry: Arc<RwLock<HashMap<String, RegisteredMember>>>,

    /// Per-sender nonce tracking: senderId -> (nonce -> entry)
    seen_nonces: Arc<RwLock<HashMap<String, HashMap<String, NonceEntry>>>>,

    /// Local monotonic send counter
    send_counter: Arc<RwLock<u64>>,

    /// Per-peer receive counters
    recv_counters: Arc<RwLock<HashMap<String, u64>>>,

    /// Max nonce age (5 minutes)
    max_nonce_age_ms: u64,
}

impl IdentityManager {
    pub fn new() -> Self {
        let identity_key = KeyPair::generate();
        let x25519_secret = StaticSecret::random_from_rng(OsRng);
        let x25519_public = X25519PublicKey::from(&x25519_secret);

        Self {
            identity_key,
            x25519_secret,
            x25519_public,
            session_keys: Arc::new(RwLock::new(HashMap::new())),
            member_registry: Arc::new(RwLock::new(HashMap::new())),
            seen_nonces: Arc::new(RwLock::new(HashMap::new())),
            send_counter: Arc::new(RwLock::new(0)),
            recv_counters: Arc::new(RwLock::new(HashMap::new())),
            max_nonce_age_ms: 300_000, // 5 minutes
        }
    }

    /// Get our Ed25519 public key
    pub fn public_key(&self) -> [u8; 32] {
        self.identity_key.public_key_bytes()
    }

    /// Get our X25519 public key
    pub fn x25519_public_key(&self) -> [u8; 32] {
        self.x25519_public.to_bytes()
    }

    /// Sign data with our identity key
    pub fn sign(&self, data: &[u8]) -> [u8; 64] {
        self.identity_key.sign(data).to_bytes()
    }

    /// Verify signature using ONLY the registry key
    /// Never use a key from the message itself
    pub fn verify_from_registry(&self, sender_id: &str, data: &[u8], signature: &[u8; 64]) -> bool {
        let registry = self.member_registry.read();
        let Some(member) = registry.get(sender_id) else {
            tracing::warn!("verify_from_registry: sender not in registry: {}", sender_id);
            return false;
        };
        KeyPair::verify(&member.ed25519_public_key, data, signature)
    }

    /// Register a member from a signed registration
    /// Verifies the signature before adding
    pub fn register_member(&self, member: RegisteredMember) -> bool {
        if !member.verify() {
            tracing::warn!("register_member: invalid signature for {}", member.agent_id);
            return false;
        }

        let mut registry = self.member_registry.write();

        // If already registered, only accept if from same key
        if let Some(existing) = registry.get(&member.agent_id) {
            if existing.ed25519_public_key != member.ed25519_public_key {
                tracing::warn!("register_member: key mismatch for {}", member.agent_id);
                return false;
            }
        }

        registry.insert(member.agent_id.clone(), member);
        true
    }

    /// Get registered member by ID
    pub fn get_member(&self, agent_id: &str) -> Option<RegisteredMember> {
        self.member_registry.read().get(agent_id).cloned()
    }

    /// Update member heartbeat
    pub fn update_heartbeat(&self, agent_id: &str) {
        let mut registry = self.member_registry.write();
        if let Some(member) = registry.get_mut(agent_id) {
            member.last_heartbeat = chrono::Utc::now().timestamp_millis() as u64;
        }
    }

    /// Get active members (heartbeat within threshold)
    pub fn get_active_members(&self, heartbeat_threshold_ms: u64) -> Vec<RegisteredMember> {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let registry = self.member_registry.read();
        registry.values()
            .filter(|m| now - m.last_heartbeat < heartbeat_threshold_ms)
            .cloned()
            .collect()
    }

    /// Create our registration (for publishing to registry)
    pub fn create_registration(&self, agent_id: &str, capabilities: Vec<String>) -> RegisteredMember {
        let joined_at = chrono::Utc::now().timestamp_millis() as u64;

        let mut member = RegisteredMember {
            agent_id: agent_id.to_string(),
            ed25519_public_key: self.public_key(),
            x25519_public_key: self.x25519_public_key(),
            capabilities,
            joined_at,
            last_heartbeat: joined_at,
            signature: [0u8; 64],
        };

        // Sign the canonical data
        let canonical = member.canonical_data();
        member.signature = self.sign(&canonical);

        member
    }

    /// Derive session key with peer using X25519 ECDH + HKDF
    /// Uses ONLY the X25519 key from the registry
    pub fn derive_session_key(&self, peer_id: &str, swarm_id: &str) -> Option<[u8; 32]> {
        let cache_key = format!("{}:{}", peer_id, swarm_id);

        // Check cache
        {
            let cache = self.session_keys.read();
            if let Some(key) = cache.get(&cache_key) {
                return Some(*key);
            }
        }

        // Get peer's X25519 key from registry ONLY
        let registry = self.member_registry.read();
        let peer = registry.get(peer_id)?;
        let peer_x25519 = X25519PublicKey::from(peer.x25519_public_key);

        // X25519 ECDH
        let shared_secret = self.x25519_secret.diffie_hellman(&peer_x25519);

        // HKDF with stable salt from both public keys
        let my_key = self.x25519_public.as_bytes();
        let peer_key = peer.x25519_public_key;

        // Salt = sha256(min(pubA, pubB) || max(pubA, pubB))
        use sha2::Digest;
        let salt = if my_key < &peer_key {
            let mut hasher = Sha256::new();
            hasher.update(my_key);
            hasher.update(&peer_key);
            hasher.finalize()
        } else {
            let mut hasher = Sha256::new();
            hasher.update(&peer_key);
            hasher.update(my_key);
            hasher.finalize()
        };

        // Info only includes swarm_id (salt already includes both parties' public keys for pair separation)
        // This ensures both parties derive the same key
        let info = format!("p2p-swarm-v2:{}", swarm_id);
        let hkdf = Hkdf::<Sha256>::new(Some(&salt), shared_secret.as_bytes());
        let mut session_key = [0u8; 32];
        hkdf.expand(info.as_bytes(), &mut session_key).ok()?;

        // Cache
        self.session_keys.write().insert(cache_key, session_key);

        Some(session_key)
    }

    /// Generate cryptographic nonce
    pub fn generate_nonce() -> String {
        let mut bytes = [0u8; 16];
        rand::RngCore::fill_bytes(&mut OsRng, &mut bytes);
        hex::encode(bytes)
    }

    /// Check nonce validity with per-sender tracking
    pub fn check_nonce(&self, nonce: &str, timestamp: u64, sender_id: &str) -> bool {
        let now = chrono::Utc::now().timestamp_millis() as u64;

        // Reject old messages
        if now.saturating_sub(timestamp) > self.max_nonce_age_ms {
            return false;
        }

        // Reject future timestamps (1 minute tolerance for clock skew)
        if timestamp > now + 60_000 {
            return false;
        }

        let mut nonces = self.seen_nonces.write();
        let sender_nonces = nonces.entry(sender_id.to_string()).or_insert_with(HashMap::new);

        // Reject replayed nonces
        if sender_nonces.contains_key(nonce) {
            return false;
        }

        // Record nonce
        sender_nonces.insert(nonce.to_string(), NonceEntry { timestamp });

        true
    }

    /// Cleanup expired nonces
    pub fn cleanup_nonces(&self) {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let mut nonces = self.seen_nonces.write();

        for sender_nonces in nonces.values_mut() {
            sender_nonces.retain(|_, entry| now - entry.timestamp < self.max_nonce_age_ms);
        }
        nonces.retain(|_, v| !v.is_empty());
    }

    /// Get next send counter
    pub fn next_send_counter(&self) -> u64 {
        let mut counter = self.send_counter.write();
        *counter += 1;
        *counter
    }

    /// Validate receive counter (must be > last seen from peer)
    pub fn validate_recv_counter(&self, peer_id: &str, counter: u64) -> bool {
        let mut counters = self.recv_counters.write();
        let last_seen = counters.get(peer_id).copied().unwrap_or(0);

        if counter <= last_seen {
            return false;
        }

        counters.insert(peer_id.to_string(), counter);
        true
    }

    /// Rotate session key for peer
    pub fn rotate_session_key(&self, peer_id: &str) {
        self.session_keys.write().retain(|k, _| !k.starts_with(peer_id));
    }
}

impl Default for IdentityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_sign_verify() {
        let keypair = KeyPair::generate();
        let message = b"test message";
        let signature = keypair.sign(message);

        assert!(KeyPair::verify(
            &keypair.public_key_bytes(),
            message,
            &signature.to_bytes()
        ));
    }

    #[test]
    fn test_member_registration() {
        let identity = IdentityManager::new();
        let registration = identity.create_registration("test-agent", vec!["executor".to_string()]);

        assert!(registration.verify());
        assert!(identity.register_member(registration));
    }

    #[test]
    fn test_session_key_derivation() {
        let alice = IdentityManager::new();
        let bob = IdentityManager::new();

        // Register each other with valid capabilities
        let alice_reg = alice.create_registration("alice", vec!["worker".to_string()]);
        let bob_reg = bob.create_registration("bob", vec!["worker".to_string()]);

        // Register in each other's registry
        alice.register_member(bob_reg.clone());
        bob.register_member(alice_reg.clone());

        // Derive session keys - both should derive the same key
        let alice_key = alice.derive_session_key("bob", "test-swarm").unwrap();
        let bob_key = bob.derive_session_key("alice", "test-swarm").unwrap();

        // Keys should match (symmetric ECDH)
        assert_eq!(alice_key, bob_key, "Session keys should be symmetric");
    }

    #[test]
    fn test_nonce_replay_protection() {
        let identity = IdentityManager::new();
        let nonce = IdentityManager::generate_nonce();
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        // First use should succeed
        assert!(identity.check_nonce(&nonce, timestamp, "sender-1"));

        // Replay should fail
        assert!(!identity.check_nonce(&nonce, timestamp, "sender-1"));

        // Different sender can use same nonce (per-sender tracking)
        assert!(identity.check_nonce(&nonce, timestamp, "sender-2"));
    }
}
