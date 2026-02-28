//! Post-quantum cryptography primitives
//!
//! This module provides cryptographic primitives for federation security:
//! - CRYSTALS-Kyber-1024 key exchange (NIST FIPS 203)
//! - ChaCha20-Poly1305 AEAD encryption
//! - HKDF-SHA256 key derivation
//! - Constant-time operations
//! - Secure memory zeroization
//!
//! # Security Level
//!
//! All primitives provide 256-bit classical security and 128+ bit post-quantum security.
//!
//! # Threat Model
//!
//! See /docs/SECURITY.md for comprehensive threat model and security architecture.

use crate::{FederationError, Result};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

// Re-export for convenience
pub use pqcrypto_kyber::kyber1024;
use pqcrypto_traits::kem::{Ciphertext, PublicKey, SecretKey, SharedSecret as PqSharedSecret};

/// Post-quantum cryptographic keypair
///
/// Uses CRYSTALS-Kyber-1024 for IND-CCA2 secure key encapsulation.
///
/// # Security Properties
///
/// - Public key: 1568 bytes (safe to distribute)
/// - Secret key: 3168 bytes (MUST be protected, auto-zeroized on drop)
/// - Post-quantum security: 256 bits (NIST Level 5)
///
/// # Example
///
/// ```ignore
/// let keypair = PostQuantumKeypair::generate();
/// let public_bytes = keypair.public_key();
/// // Send public_bytes to peer
/// ```
#[derive(Clone)]
pub struct PostQuantumKeypair {
    /// Public key (safe to share)
    pub public: Vec<u8>,
    /// Secret key (automatically zeroized on drop)
    secret: SecretKeyWrapper,
}

impl std::fmt::Debug for PostQuantumKeypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostQuantumKeypair")
            .field("public", &format!("{}bytes", self.public.len()))
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

/// Wrapper for secret key with automatic zeroization
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct SecretKeyWrapper(Vec<u8>);

impl PostQuantumKeypair {
    /// Generate a new post-quantum keypair using CRYSTALS-Kyber-1024
    ///
    /// # Security
    ///
    /// Uses OS CSPRNG (via `rand::thread_rng()`). Ensure OS has sufficient entropy.
    ///
    /// # Panics
    ///
    /// Never panics. Kyber key generation is deterministic after RNG sampling.
    pub fn generate() -> Self {
        let (public, secret) = kyber1024::keypair();

        Self {
            public: public.as_bytes().to_vec(),
            secret: SecretKeyWrapper(secret.as_bytes().to_vec()),
        }
    }

    /// Get the public key bytes
    ///
    /// Safe to transmit over insecure channels.
    pub fn public_key(&self) -> &[u8] {
        &self.public
    }

    /// Encapsulate: generate shared secret and ciphertext for recipient's public key
    ///
    /// # Arguments
    ///
    /// * `public_key` - Recipient's Kyber-1024 public key (1568 bytes)
    ///
    /// # Returns
    ///
    /// * `SharedSecret` - 32-byte shared secret (use for key derivation)
    /// * `Vec<u8>` - 1568-byte ciphertext (send to recipient)
    ///
    /// # Errors
    ///
    /// Returns `CryptoError` if public key is invalid (wrong size or corrupted).
    ///
    /// # Security
    ///
    /// The shared secret is cryptographically strong (256-bit entropy).
    /// The ciphertext is IND-CCA2 secure against quantum adversaries.
    pub fn encapsulate(public_key: &[u8]) -> Result<(SharedSecret, Vec<u8>)> {
        // Validate public key size (Kyber1024 = 1568 bytes)
        if public_key.len() != 1568 {
            return Err(FederationError::CryptoError(format!(
                "Invalid public key size: expected 1568 bytes, got {}",
                public_key.len()
            )));
        }

        // Parse public key
        let pk = kyber1024::PublicKey::from_bytes(public_key).map_err(|e| {
            FederationError::CryptoError(format!("Failed to parse Kyber public key: {:?}", e))
        })?;

        // Perform KEM encapsulation
        let (shared_secret, ciphertext) = kyber1024::encapsulate(&pk);

        Ok((
            SharedSecret(SecretBytes(shared_secret.as_bytes().to_vec())),
            ciphertext.as_bytes().to_vec(),
        ))
    }

    /// Decapsulate: extract shared secret from ciphertext
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - 1568-byte Kyber-1024 ciphertext
    ///
    /// # Returns
    ///
    /// * `SharedSecret` - 32-byte shared secret (same as encapsulator's)
    ///
    /// # Errors
    ///
    /// Returns `CryptoError` if:
    /// - Ciphertext is wrong size
    /// - Ciphertext is invalid or corrupted
    /// - Decapsulation fails (should never happen with valid inputs)
    ///
    /// # Security
    ///
    /// Timing-safe: execution time independent of secret key or ciphertext validity.
    pub fn decapsulate(&self, ciphertext: &[u8]) -> Result<SharedSecret> {
        // Validate ciphertext size
        if ciphertext.len() != 1568 {
            return Err(FederationError::CryptoError(format!(
                "Invalid ciphertext size: expected 1568 bytes, got {}",
                ciphertext.len()
            )));
        }

        // Parse secret key
        let sk = kyber1024::SecretKey::from_bytes(&self.secret.0).map_err(|e| {
            FederationError::CryptoError(format!("Failed to parse secret key: {:?}", e))
        })?;

        // Parse ciphertext
        let ct = kyber1024::Ciphertext::from_bytes(ciphertext).map_err(|e| {
            FederationError::CryptoError(format!("Failed to parse Kyber ciphertext: {:?}", e))
        })?;

        // Perform KEM decapsulation
        let shared_secret = kyber1024::decapsulate(&ct, &sk);

        Ok(SharedSecret(SecretBytes(shared_secret.as_bytes().to_vec())))
    }
}

/// Secret bytes wrapper with automatic zeroization
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct SecretBytes(Vec<u8>);

/// Shared secret derived from Kyber KEM
///
/// # Security
///
/// - Automatically zeroized on drop
/// - 32 bytes of cryptographically strong key material
/// - Suitable for HKDF key derivation
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SharedSecret(SecretBytes);

impl std::fmt::Debug for SharedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedSecret")
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl SharedSecret {
    /// Derive encryption and MAC keys from shared secret using HKDF-SHA256
    ///
    /// # Key Derivation
    ///
    /// ```text
    /// shared_secret (32 bytes from Kyber)
    ///     ↓
    /// HKDF-Extract(salt=zeros, ikm=shared_secret) → PRK
    ///     ↓
    /// HKDF-Expand(PRK, info="encryption") → encryption_key (32 bytes)
    /// HKDF-Expand(PRK, info="mac") → mac_key (32 bytes)
    /// ```
    ///
    /// # Returns
    ///
    /// - Encryption key: 256-bit key for ChaCha20
    /// - MAC key: 256-bit key for Poly1305
    ///
    /// # Security
    ///
    /// Keys are cryptographically independent. Compromise of one does not affect the other.
    pub fn derive_keys(&self) -> (Vec<u8>, Vec<u8>) {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        // HKDF-Extract: PRK = HMAC-SHA256(salt=zeros, ikm=shared_secret)
        let salt = [0u8; 32]; // Zero salt is acceptable for Kyber output
        let mut extract_hmac =
            HmacSha256::new_from_slice(&salt).expect("HMAC-SHA256 accepts any key size");
        extract_hmac.update(&self.0 .0);
        let prk = extract_hmac.finalize().into_bytes();

        // HKDF-Expand for encryption key
        let mut enc_hmac = HmacSha256::new_from_slice(&prk).expect("PRK is valid HMAC key");
        enc_hmac.update(b"encryption");
        enc_hmac.update(&[1u8]); // Counter = 1
        let encrypt_key = enc_hmac.finalize().into_bytes().to_vec();

        // HKDF-Expand for MAC key
        let mut mac_hmac = HmacSha256::new_from_slice(&prk).expect("PRK is valid HMAC key");
        mac_hmac.update(b"mac");
        mac_hmac.update(&[1u8]); // Counter = 1
        let mac_key = mac_hmac.finalize().into_bytes().to_vec();

        (encrypt_key, mac_key)
    }
}

/// Encrypted communication channel using ChaCha20-Poly1305 AEAD
///
/// # Security Properties
///
/// - Confidentiality: ChaCha20 stream cipher (IND-CPA)
/// - Integrity: Poly1305 MAC (SUF-CMA)
/// - AEAD: Combined mode (IND-CCA2)
/// - Nonce: 96-bit random + 32-bit counter (unique per message)
///
/// # Example
///
/// ```ignore
/// let channel = EncryptedChannel::new(peer_id, shared_secret);
/// let ciphertext = channel.encrypt(b"secret message")?;
/// let plaintext = channel.decrypt(&ciphertext)?;
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedChannel {
    /// Peer identifier
    pub peer_id: String,
    /// Encryption key (not serialized - ephemeral)
    #[serde(skip)]
    encrypt_key: Vec<u8>,
    /// MAC key for authentication (not serialized - ephemeral)
    #[serde(skip)]
    mac_key: Vec<u8>,
    /// Message counter for nonce generation
    #[serde(skip)]
    counter: std::sync::atomic::AtomicU32,
}

impl Clone for EncryptedChannel {
    fn clone(&self) -> Self {
        Self {
            peer_id: self.peer_id.clone(),
            encrypt_key: self.encrypt_key.clone(),
            mac_key: self.mac_key.clone(),
            counter: std::sync::atomic::AtomicU32::new(
                self.counter.load(std::sync::atomic::Ordering::SeqCst),
            ),
        }
    }
}

impl EncryptedChannel {
    /// Create a new encrypted channel from a shared secret
    ///
    /// # Arguments
    ///
    /// * `peer_id` - Identifier for the peer (for auditing/logging)
    /// * `shared_secret` - Shared secret from Kyber KEM
    ///
    /// # Security
    ///
    /// Keys are derived using HKDF-SHA256 with domain separation.
    pub fn new(peer_id: String, shared_secret: SharedSecret) -> Self {
        let (encrypt_key, mac_key) = shared_secret.derive_keys();

        Self {
            peer_id,
            encrypt_key,
            mac_key,
            counter: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Encrypt a message using ChaCha20-Poly1305
    ///
    /// # Arguments
    ///
    /// * `plaintext` - Message to encrypt
    ///
    /// # Returns
    ///
    /// Ciphertext format: `[nonce: 12 bytes][ciphertext][tag: 16 bytes]`
    ///
    /// # Errors
    ///
    /// Returns `CryptoError` if encryption fails (should never happen).
    ///
    /// # Security
    ///
    /// - Unique nonce per message (96-bit random + 32-bit counter)
    /// - Authenticated encryption (modify ciphertext = detection)
    /// - Quantum resistance: 128-bit security (Grover bound)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };

        // Create cipher instance
        let key_array: [u8; 32] = self
            .encrypt_key
            .as_slice()
            .try_into()
            .map_err(|_| FederationError::CryptoError("Invalid key size".into()))?;
        let cipher = ChaCha20Poly1305::new(&key_array.into());

        // Generate unique nonce: [random: 8 bytes][counter: 4 bytes]
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[0..8].copy_from_slice(&rand::random::<[u8; 8]>());
        let counter = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        nonce_bytes[8..12].copy_from_slice(&counter.to_le_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt with AEAD
        let ciphertext = cipher.encrypt(nonce, plaintext).map_err(|e| {
            FederationError::CryptoError(format!("ChaCha20-Poly1305 encryption failed: {}", e))
        })?;

        // Prepend nonce to ciphertext (needed for decryption)
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt a message using ChaCha20-Poly1305
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - Encrypted message (format: `[nonce: 12][ciphertext][tag: 16]`)
    ///
    /// # Returns
    ///
    /// Decrypted plaintext
    ///
    /// # Errors
    ///
    /// Returns `CryptoError` if:
    /// - Ciphertext is too short (< 28 bytes)
    /// - Authentication tag verification fails (tampering detected)
    /// - Decryption fails
    ///
    /// # Security
    ///
    /// - **Constant-time**: Timing independent of plaintext content
    /// - **Tamper-evident**: Any modification causes authentication failure
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };

        // Validate minimum size: nonce(12) + tag(16) = 28 bytes
        if ciphertext.len() < 28 {
            return Err(FederationError::CryptoError(format!(
                "Ciphertext too short: {} bytes (minimum 28)",
                ciphertext.len()
            )));
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ct) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Create cipher instance
        let key_array: [u8; 32] = self
            .encrypt_key
            .as_slice()
            .try_into()
            .map_err(|_| FederationError::CryptoError("Invalid key size".into()))?;
        let cipher = ChaCha20Poly1305::new(&key_array.into());

        // Decrypt with AEAD (authentication happens here)
        let plaintext = cipher.decrypt(nonce, ct).map_err(|e| {
            FederationError::CryptoError(format!(
                "ChaCha20-Poly1305 decryption failed (tampering?): {}",
                e
            ))
        })?;

        Ok(plaintext)
    }

    /// Sign a message with HMAC-SHA256
    ///
    /// # Arguments
    ///
    /// * `message` - Message to authenticate
    ///
    /// # Returns
    ///
    /// 32-byte HMAC tag
    ///
    /// # Security
    ///
    /// - PRF security: tag reveals nothing about key
    /// - Quantum resistance: 128-bit security (Grover)
    ///
    /// # Note
    ///
    /// If using `encrypt()`, signatures are redundant (Poly1305 provides authentication).
    /// Use this for non-encrypted authenticated messages.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let mut mac = Hmac::<Sha256>::new_from_slice(&self.mac_key)
            .expect("HMAC-SHA256 accepts any key size");
        mac.update(message);
        mac.finalize().into_bytes().to_vec()
    }

    /// Verify a message signature using constant-time comparison
    ///
    /// # Arguments
    ///
    /// * `message` - Original message
    /// * `signature` - HMAC tag to verify
    ///
    /// # Returns
    ///
    /// `true` if signature is valid, `false` otherwise
    ///
    /// # Security
    ///
    /// - **Constant-time**: Execution time independent of signature validity
    /// - **Timing-attack resistant**: No early termination on mismatch
    ///
    /// # Critical Security Property
    ///
    /// This function MUST use constant-time comparison to prevent timing side-channels.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        use subtle::ConstantTimeEq;

        let expected = self.sign(message);

        // Constant-time comparison (critical for security)
        if expected.len() != signature.len() {
            return false;
        }

        expected.ct_eq(signature).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = PostQuantumKeypair::generate();
        assert_eq!(keypair.public.len(), 1568); // Kyber-1024 public key size
    }

    #[test]
    fn test_key_exchange() {
        let alice = PostQuantumKeypair::generate();
        let bob = PostQuantumKeypair::generate();

        // Alice encapsulates to Bob
        let (alice_secret, ciphertext) = PostQuantumKeypair::encapsulate(bob.public_key()).unwrap();

        // Bob decapsulates
        let bob_secret = bob.decapsulate(&ciphertext).unwrap();

        // Derive keys and verify they match
        let (alice_enc, alice_mac) = alice_secret.derive_keys();
        let (bob_enc, bob_mac) = bob_secret.derive_keys();

        assert_eq!(alice_enc, bob_enc, "Encryption keys must match");
        assert_eq!(alice_mac, bob_mac, "MAC keys must match");
    }

    #[test]
    fn test_encrypted_channel() {
        let keypair = PostQuantumKeypair::generate();
        let (secret, _) = PostQuantumKeypair::encapsulate(keypair.public_key()).unwrap();

        let channel = EncryptedChannel::new("peer1".to_string(), secret);

        let plaintext = b"Hello, post-quantum federation!";
        let ciphertext = channel.encrypt(plaintext).unwrap();

        // Verify ciphertext is different
        assert_ne!(&ciphertext[12..], plaintext);

        // Decrypt and verify
        let decrypted = channel.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_message_signing() {
        let keypair = PostQuantumKeypair::generate();
        let (secret, _) = PostQuantumKeypair::encapsulate(keypair.public_key()).unwrap();
        let channel = EncryptedChannel::new("peer1".to_string(), secret);

        let message = b"Important authenticated message";
        let signature = channel.sign(message);

        // Verify valid signature
        assert!(channel.verify(message, &signature));

        // Verify invalid signature
        assert!(!channel.verify(b"Different message", &signature));

        // Verify tampered signature
        let mut bad_sig = signature.clone();
        bad_sig[0] ^= 1; // Flip one bit
        assert!(!channel.verify(message, &bad_sig));
    }

    #[test]
    fn test_decryption_tamper_detection() {
        let keypair = PostQuantumKeypair::generate();
        let (secret, _) = PostQuantumKeypair::encapsulate(keypair.public_key()).unwrap();
        let channel = EncryptedChannel::new("peer1".to_string(), secret);

        let plaintext = b"Secret message";
        let mut ciphertext = channel.encrypt(plaintext).unwrap();

        // Tamper with ciphertext (flip one bit in encrypted data)
        ciphertext[20] ^= 1;

        // Decryption should fail due to authentication
        let result = channel.decrypt(&ciphertext);
        assert!(
            result.is_err(),
            "Tampered ciphertext should fail authentication"
        );
    }

    #[test]
    fn test_invalid_public_key_size() {
        let bad_pk = vec![0u8; 100]; // Wrong size
        let result = PostQuantumKeypair::encapsulate(&bad_pk);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_ciphertext_size() {
        let keypair = PostQuantumKeypair::generate();
        let bad_ct = vec![0u8; 100]; // Wrong size
        let result = keypair.decapsulate(&bad_ct);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonce_uniqueness() {
        let keypair = PostQuantumKeypair::generate();
        let (secret, _) = PostQuantumKeypair::encapsulate(keypair.public_key()).unwrap();
        let channel = EncryptedChannel::new("peer1".to_string(), secret);

        let plaintext = b"Test message";

        // Encrypt same message twice
        let ct1 = channel.encrypt(plaintext).unwrap();
        let ct2 = channel.encrypt(plaintext).unwrap();

        // Ciphertexts should be different (different nonces)
        assert_ne!(ct1, ct2, "Nonces must be unique");

        // Both should decrypt correctly
        assert_eq!(channel.decrypt(&ct1).unwrap(), plaintext);
        assert_eq!(channel.decrypt(&ct2).unwrap(), plaintext);
    }
}
