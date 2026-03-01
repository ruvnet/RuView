//! Permit token issuance and verification

use crate::{ActionId, GateDecision};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier as Ed25519Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

/// Permit token: a signed capability that agents must present
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitToken {
    /// Gate decision
    pub decision: GateDecision,
    /// Action being permitted
    pub action_id: ActionId,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
    /// Time-to-live in nanoseconds
    pub ttl_ns: u64,
    /// Hash of the witness data
    #[serde(with = "hex::serde")]
    pub witness_hash: [u8; 32],
    /// Sequence number
    pub sequence: u64,
    /// Full Ed25519 signature (64 bytes)
    #[serde(with = "hex::serde")]
    pub signature: [u8; 64],
}

impl PermitToken {
    /// Check if token is still valid (not expired)
    pub fn is_valid_time(&self, now_ns: u64) -> bool {
        now_ns <= self.timestamp + self.ttl_ns
    }

    /// Encode token to base64 for transport
    pub fn encode_base64(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &json)
    }

    /// Decode token from base64
    pub fn decode_base64(encoded: &str) -> Result<Self, TokenDecodeError> {
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
            .map_err(|_| TokenDecodeError::InvalidBase64)?;
        serde_json::from_slice(&bytes).map_err(|_| TokenDecodeError::InvalidJson)
    }

    /// Get the content to be signed (excludes mac field)
    pub fn signable_content(&self) -> Vec<u8> {
        let mut content = Vec::with_capacity(128);
        content.extend_from_slice(&self.sequence.to_le_bytes());
        content.extend_from_slice(&self.timestamp.to_le_bytes());
        content.extend_from_slice(&self.ttl_ns.to_le_bytes());
        content.extend_from_slice(&self.witness_hash);
        content.extend_from_slice(self.action_id.as_bytes());
        content.push(self.decision as u8);
        content
    }
}

/// Error decoding a token
#[derive(Debug, thiserror::Error)]
pub enum TokenDecodeError {
    #[error("Invalid base64 encoding")]
    InvalidBase64,
    #[error("Invalid JSON structure")]
    InvalidJson,
}

/// Permit state: manages signing keys and token issuance
pub struct PermitState {
    /// Signing key for tokens
    signing_key: SigningKey,
    /// Next sequence number
    next_sequence: std::sync::atomic::AtomicU64,
}

impl PermitState {
    /// Create new permit state with fresh signing key
    pub fn new() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            signing_key,
            next_sequence: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Create permit state with a specific signing key
    pub fn with_key(signing_key: SigningKey) -> Self {
        Self {
            signing_key,
            next_sequence: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Get the next sequence number
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Sign a token with full Ed25519 signature
    pub fn sign_token(&self, mut token: PermitToken) -> PermitToken {
        let content = token.signable_content();
        let hash = blake3::hash(&content);
        let signature = self.signing_key.sign(hash.as_bytes());

        // Store full 64-byte Ed25519 signature
        token.signature.copy_from_slice(&signature.to_bytes());
        token
    }

    /// Get a verifier for this permit state
    pub fn verifier(&self) -> Verifier {
        Verifier {
            verifying_key: self.signing_key.verifying_key(),
        }
    }
}

impl Default for PermitState {
    fn default() -> Self {
        Self::new()
    }
}

/// Token verifier with actual Ed25519 signature verification
#[derive(Clone)]
pub struct Verifier {
    /// Ed25519 verifying key
    verifying_key: VerifyingKey,
}

impl Verifier {
    /// Create a new verifier from a verifying key
    pub fn new(verifying_key: VerifyingKey) -> Self {
        Self { verifying_key }
    }

    /// Verify a token's Ed25519 signature
    pub fn verify(&self, token: &PermitToken) -> Result<(), VerifyError> {
        // Compute hash of signable content
        let content = token.signable_content();
        let hash = blake3::hash(&content);

        // Reconstruct the Ed25519 signature from stored bytes
        let signature = Signature::from_bytes(&token.signature);

        // Actually verify the signature using Ed25519
        self.verifying_key
            .verify(hash.as_bytes(), &signature)
            .map_err(|_| VerifyError::SignatureFailed)
    }

    /// Verify token is valid (signature + time)
    pub fn verify_full(&self, token: &PermitToken) -> Result<(), VerifyError> {
        // Check signature first
        self.verify(token)?;

        // Check TTL - use saturating add to prevent overflow
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let expiry = token.timestamp.saturating_add(token.ttl_ns);
        if now > expiry {
            return Err(VerifyError::Expired);
        }

        Ok(())
    }
}

/// Verification error
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("Signature verification failed")]
    SignatureFailed,
    #[error("Hash mismatch")]
    HashMismatch,
    #[error("Token has expired")]
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_sign_verify() {
        let state = PermitState::new();
        let verifier = state.verifier();

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test-action".to_string(),
            timestamp: 1000000000,
            ttl_ns: 60_000_000_000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let signed = state.sign_token(token);
        assert!(verifier.verify(&signed).is_ok());
    }

    #[test]
    fn test_token_tamper_detection() {
        let state = PermitState::new();
        let verifier = state.verifier();

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test-action".to_string(),
            timestamp: 1000000000,
            ttl_ns: 60_000_000_000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let mut signed = state.sign_token(token);

        // Tamper with the action_id
        signed.action_id = "malicious-action".to_string();

        // Verification should fail
        assert!(verifier.verify(&signed).is_err());
    }

    #[test]
    fn test_token_wrong_key_rejection() {
        let state1 = PermitState::new();
        let state2 = PermitState::new();
        let verifier2 = state2.verifier();

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test-action".to_string(),
            timestamp: 1000000000,
            ttl_ns: 60_000_000_000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        // Sign with state1's key
        let signed = state1.sign_token(token);

        // Verify with state2's key should fail
        assert!(verifier2.verify(&signed).is_err());
    }

    #[test]
    fn test_token_base64_roundtrip() {
        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test-action".to_string(),
            timestamp: 1000000000,
            ttl_ns: 60_000_000_000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let encoded = token.encode_base64();
        let decoded = PermitToken::decode_base64(&encoded).unwrap();

        assert_eq!(token.action_id, decoded.action_id);
        assert_eq!(token.sequence, decoded.sequence);
    }

    #[test]
    fn test_token_expiry() {
        let state = PermitState::new();
        let verifier = state.verifier();

        // Create a token that expired in the past
        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test-action".to_string(),
            timestamp: 1000000000, // Long ago
            ttl_ns: 1,             // 1 nanosecond TTL
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let signed = state.sign_token(token);

        // Signature should be valid
        assert!(verifier.verify(&signed).is_ok());

        // But full verification (including TTL) should fail
        assert!(matches!(
            verifier.verify_full(&signed),
            Err(VerifyError::Expired)
        ));
    }
}
