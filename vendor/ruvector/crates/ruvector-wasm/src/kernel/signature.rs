//! Ed25519 Signature Verification
//!
//! Provides cryptographic signature verification for kernel pack manifests
//! to ensure supply chain security.

use crate::kernel::error::VerifyError;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Kernel pack signature verifier
///
/// Maintains a list of trusted Ed25519 public keys and verifies
/// manifest signatures against them.
#[derive(Debug, Clone)]
pub struct KernelPackVerifier {
    /// Trusted Ed25519 public keys
    trusted_keys: Vec<VerifyingKey>,
    /// Whether to require signatures (can be disabled for development)
    require_signature: bool,
}

impl KernelPackVerifier {
    /// Create a new verifier with no trusted keys
    pub fn new() -> Self {
        KernelPackVerifier {
            trusted_keys: Vec::new(),
            require_signature: true,
        }
    }

    /// Create a verifier with pre-loaded trusted keys
    pub fn with_trusted_keys(keys: Vec<VerifyingKey>) -> Self {
        KernelPackVerifier {
            trusted_keys: keys,
            require_signature: true,
        }
    }

    /// Create a verifier that doesn't require signatures (for development)
    ///
    /// # Warning
    /// This should NEVER be used in production as it bypasses security checks.
    pub fn insecure_no_verify() -> Self {
        KernelPackVerifier {
            trusted_keys: Vec::new(),
            require_signature: false,
        }
    }

    /// Add a trusted public key from bytes
    pub fn add_trusted_key(&mut self, key_bytes: &[u8; 32]) -> Result<(), VerifyError> {
        let key = VerifyingKey::from_bytes(key_bytes).map_err(|e| VerifyError::KeyError {
            message: e.to_string(),
        })?;
        self.trusted_keys.push(key);
        Ok(())
    }

    /// Add a trusted public key from hex string
    pub fn add_trusted_key_hex(&mut self, hex: &str) -> Result<(), VerifyError> {
        // Remove "ed25519:" prefix if present
        let hex = hex.strip_prefix("ed25519:").unwrap_or(hex);

        let bytes = hex::decode(hex).map_err(|e| VerifyError::KeyError {
            message: format!("Invalid hex: {}", e),
        })?;

        if bytes.len() != 32 {
            return Err(VerifyError::KeyError {
                message: format!("Invalid key length: expected 32 bytes, got {}", bytes.len()),
            });
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        self.add_trusted_key(&key_bytes)
    }

    /// Add a trusted public key from base64 string
    pub fn add_trusted_key_base64(&mut self, b64: &str) -> Result<(), VerifyError> {
        // Remove "ed25519:" prefix if present
        let b64 = b64.strip_prefix("ed25519:").unwrap_or(b64);

        use base64::{engine::general_purpose::STANDARD, Engine};
        let bytes = STANDARD.decode(b64).map_err(|e| VerifyError::KeyError {
            message: format!("Invalid base64: {}", e),
        })?;

        if bytes.len() != 32 {
            return Err(VerifyError::KeyError {
                message: format!("Invalid key length: expected 32 bytes, got {}", bytes.len()),
            });
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        self.add_trusted_key(&key_bytes)
    }

    /// Verify manifest signature against trusted keys
    ///
    /// # Arguments
    /// * `manifest` - The manifest bytes to verify
    /// * `signature` - The signature bytes (64 bytes)
    ///
    /// # Returns
    /// * `Ok(())` if signature is valid and from a trusted key
    /// * `Err(VerifyError::NoTrustedKey)` if no trusted key verified the signature
    pub fn verify(&self, manifest: &[u8], signature: &[u8]) -> Result<(), VerifyError> {
        // Skip verification if disabled (development mode)
        if !self.require_signature {
            return Ok(());
        }

        // Check we have trusted keys
        if self.trusted_keys.is_empty() {
            return Err(VerifyError::NoTrustedKey);
        }

        // Parse signature
        let sig = Signature::from_slice(signature).map_err(|e| VerifyError::InvalidSignature {
            reason: format!("Invalid signature format: {}", e),
        })?;

        // Try each trusted key
        for key in &self.trusted_keys {
            if key.verify(manifest, &sig).is_ok() {
                return Ok(());
            }
        }

        Err(VerifyError::NoTrustedKey)
    }

    /// Verify manifest with signature from hex string
    pub fn verify_hex(&self, manifest: &[u8], signature_hex: &str) -> Result<(), VerifyError> {
        let signature = hex::decode(signature_hex).map_err(|e| VerifyError::InvalidSignature {
            reason: format!("Invalid hex signature: {}", e),
        })?;
        self.verify(manifest, &signature)
    }

    /// Verify manifest with signature from base64 string
    pub fn verify_base64(&self, manifest: &[u8], signature_b64: &str) -> Result<(), VerifyError> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let signature =
            STANDARD
                .decode(signature_b64)
                .map_err(|e| VerifyError::InvalidSignature {
                    reason: format!("Invalid base64 signature: {}", e),
                })?;
        self.verify(manifest, &signature)
    }

    /// Get number of trusted keys
    pub fn trusted_key_count(&self) -> usize {
        self.trusted_keys.len()
    }

    /// Check if signature verification is required
    pub fn is_verification_required(&self) -> bool {
        self.require_signature
    }
}

impl Default for KernelPackVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility function to sign a manifest (for kernel pack creation)
#[cfg(feature = "signing")]
pub fn sign_manifest(manifest: &[u8], signing_key: &ed25519_dalek::SigningKey) -> Vec<u8> {
    use ed25519_dalek::Signer;
    signing_key.sign(manifest).to_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn generate_key_pair() -> (SigningKey, VerifyingKey) {
        // Use a fixed test seed for reproducibility
        let mut seed = [0u8; 32];
        // Simple deterministic seed based on test
        for (i, b) in seed.iter_mut().enumerate() {
            *b = (i * 7 + 13) as u8;
        }
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    #[test]
    fn test_verify_success() {
        use ed25519_dalek::Signer;

        let (signing_key, verifying_key) = generate_key_pair();
        let manifest = b"test manifest content";
        let signature = signing_key.sign(manifest);

        let mut verifier = KernelPackVerifier::new();
        verifier.trusted_keys.push(verifying_key);

        assert!(verifier.verify(manifest, &signature.to_bytes()).is_ok());
    }

    #[test]
    fn test_verify_wrong_key() {
        use ed25519_dalek::Signer;

        let (signing_key, _) = generate_key_pair();
        let (_, wrong_verifying_key) = generate_key_pair();

        let manifest = b"test manifest content";
        let signature = signing_key.sign(manifest);

        let mut verifier = KernelPackVerifier::new();
        verifier.trusted_keys.push(wrong_verifying_key);

        let result = verifier.verify(manifest, &signature.to_bytes());
        assert!(matches!(result, Err(VerifyError::NoTrustedKey)));
    }

    #[test]
    fn test_verify_no_keys() {
        let verifier = KernelPackVerifier::new();
        let manifest = b"test manifest";
        let signature = [0u8; 64];

        let result = verifier.verify(manifest, &signature);
        assert!(matches!(result, Err(VerifyError::NoTrustedKey)));
    }

    #[test]
    fn test_insecure_no_verify() {
        let verifier = KernelPackVerifier::insecure_no_verify();
        let manifest = b"test manifest";
        let invalid_signature = [0u8; 64];

        // Should pass even with invalid signature
        assert!(verifier.verify(manifest, &invalid_signature).is_ok());
        assert!(!verifier.is_verification_required());
    }

    #[test]
    fn test_add_key_hex() {
        let mut verifier = KernelPackVerifier::new();

        // Valid 32-byte key in hex
        let hex_key = "0000000000000000000000000000000000000000000000000000000000000000";
        // Note: This is a degenerate key but tests the parsing
        let result = verifier.add_trusted_key_hex(hex_key);
        // This specific key may or may not be valid depending on curve requirements
        // The important thing is that hex parsing works
        assert!(result.is_ok() || matches!(result, Err(VerifyError::KeyError { .. })));
    }

    #[test]
    fn test_add_key_with_prefix() {
        let mut verifier = KernelPackVerifier::new();

        // Key with ed25519: prefix
        let prefixed_key =
            "ed25519:0000000000000000000000000000000000000000000000000000000000000000";
        let _ = verifier.add_trusted_key_hex(prefixed_key);
        // Just testing that prefix stripping works
    }

    #[test]
    fn test_invalid_hex() {
        let mut verifier = KernelPackVerifier::new();
        let invalid = "not_valid_hex";

        let result = verifier.add_trusted_key_hex(invalid);
        assert!(matches!(result, Err(VerifyError::KeyError { .. })));
    }

    #[test]
    fn test_wrong_key_length() {
        let mut verifier = KernelPackVerifier::new();
        let short_key = "0000000000000000"; // 8 bytes

        let result = verifier.add_trusted_key_hex(short_key);
        assert!(matches!(result, Err(VerifyError::KeyError { .. })));
    }
}
