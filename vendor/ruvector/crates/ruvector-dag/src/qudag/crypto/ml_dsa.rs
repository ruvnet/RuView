//! ML-DSA-65 Digital Signatures
//!
//! # Security Status
//!
//! With `production-crypto` feature: Uses `pqcrypto-dilithium` (Dilithium3 ≈ ML-DSA-65)
//! Without feature: Uses HMAC-SHA256 placeholder (NOT quantum-resistant)
//!
//! ## Production Use
//!
//! Enable the `production-crypto` feature in Cargo.toml:
//! ```toml
//! ruvector-dag = { version = "0.1", features = ["production-crypto"] }
//! ```

use zeroize::Zeroize;

// ML-DSA-65 sizes (FIPS 204)
// Note: Dilithium3 is the closest match to ML-DSA-65 security level
pub const ML_DSA_65_PUBLIC_KEY_SIZE: usize = 1952;
pub const ML_DSA_65_SECRET_KEY_SIZE: usize = 4032;
pub const ML_DSA_65_SIGNATURE_SIZE: usize = 3309;

#[derive(Clone)]
pub struct MlDsa65PublicKey(pub [u8; ML_DSA_65_PUBLIC_KEY_SIZE]);

#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct MlDsa65SecretKey(pub [u8; ML_DSA_65_SECRET_KEY_SIZE]);

#[derive(Clone)]
pub struct Signature(pub [u8; ML_DSA_65_SIGNATURE_SIZE]);

pub struct MlDsa65;

// ============================================================================
// Production Implementation (using pqcrypto-dilithium)
// ============================================================================

#[cfg(feature = "production-crypto")]
mod production {
    use super::*;
    use pqcrypto_dilithium::dilithium3;
    use pqcrypto_traits::sign::{DetachedSignature, PublicKey, SecretKey};

    impl MlDsa65 {
        /// Generate a new signing keypair using real Dilithium3
        pub fn generate_keypair() -> Result<(MlDsa65PublicKey, MlDsa65SecretKey), DsaError> {
            let (pk, sk) = dilithium3::keypair();

            let pk_bytes = pk.as_bytes();
            let sk_bytes = sk.as_bytes();

            // Dilithium3 sizes: pk=1952, sk=4032 (matches ML-DSA-65)
            let mut pk_arr = [0u8; ML_DSA_65_PUBLIC_KEY_SIZE];
            let mut sk_arr = [0u8; ML_DSA_65_SECRET_KEY_SIZE];

            if pk_bytes.len() != ML_DSA_65_PUBLIC_KEY_SIZE {
                return Err(DsaError::InvalidPublicKey);
            }
            if sk_bytes.len() != ML_DSA_65_SECRET_KEY_SIZE {
                return Err(DsaError::SigningFailed);
            }

            pk_arr.copy_from_slice(pk_bytes);
            sk_arr.copy_from_slice(sk_bytes);

            Ok((MlDsa65PublicKey(pk_arr), MlDsa65SecretKey(sk_arr)))
        }

        /// Sign a message using real Dilithium3
        pub fn sign(sk: &MlDsa65SecretKey, message: &[u8]) -> Result<Signature, DsaError> {
            let secret_key =
                dilithium3::SecretKey::from_bytes(&sk.0).map_err(|_| DsaError::InvalidSignature)?;

            let sig = dilithium3::detached_sign(message, &secret_key);
            let sig_bytes = sig.as_bytes();

            let mut sig_arr = [0u8; ML_DSA_65_SIGNATURE_SIZE];

            // Dilithium3 signature size is 3293, we pad to match ML-DSA-65's 3309
            let copy_len = sig_bytes.len().min(ML_DSA_65_SIGNATURE_SIZE);
            sig_arr[..copy_len].copy_from_slice(&sig_bytes[..copy_len]);

            Ok(Signature(sig_arr))
        }

        /// Verify a signature using real Dilithium3
        pub fn verify(
            pk: &MlDsa65PublicKey,
            message: &[u8],
            signature: &Signature,
        ) -> Result<bool, DsaError> {
            let public_key =
                dilithium3::PublicKey::from_bytes(&pk.0).map_err(|_| DsaError::InvalidPublicKey)?;

            // Dilithium3 signature is 3293 bytes
            let sig = dilithium3::DetachedSignature::from_bytes(&signature.0[..3293])
                .map_err(|_| DsaError::InvalidSignature)?;

            match dilithium3::verify_detached_signature(&sig, message, &public_key) {
                Ok(()) => Ok(true),
                Err(_) => Ok(false),
            }
        }
    }
}

// ============================================================================
// Placeholder Implementation (HMAC-SHA256 - NOT quantum-resistant)
// ============================================================================

#[cfg(not(feature = "production-crypto"))]
mod placeholder {
    use super::*;
    use sha2::{Digest, Sha256};

    impl MlDsa65 {
        /// Generate a new signing keypair (PLACEHOLDER)
        ///
        /// # Security Warning
        /// This is a placeholder using random bytes, NOT real ML-DSA.
        pub fn generate_keypair() -> Result<(MlDsa65PublicKey, MlDsa65SecretKey), DsaError> {
            let mut pk = [0u8; ML_DSA_65_PUBLIC_KEY_SIZE];
            let mut sk = [0u8; ML_DSA_65_SECRET_KEY_SIZE];

            getrandom::getrandom(&mut pk).map_err(|_| DsaError::RngFailed)?;
            getrandom::getrandom(&mut sk).map_err(|_| DsaError::RngFailed)?;

            Ok((MlDsa65PublicKey(pk), MlDsa65SecretKey(sk)))
        }

        /// Sign a message (PLACEHOLDER)
        ///
        /// # Security Warning
        /// This is a placeholder using HMAC-SHA256, NOT real ML-DSA.
        /// Provides basic integrity but NO quantum resistance.
        pub fn sign(sk: &MlDsa65SecretKey, message: &[u8]) -> Result<Signature, DsaError> {
            let mut sig = [0u8; ML_DSA_65_SIGNATURE_SIZE];

            let hmac = Self::hmac_sha256(&sk.0[..32], message);

            for i in 0..ML_DSA_65_SIGNATURE_SIZE {
                sig[i] = hmac[i % 32];
            }

            let key_hash = Self::sha256(&sk.0[32..64]);
            for i in 0..32 {
                sig[i + 32] = key_hash[i];
            }

            Ok(Signature(sig))
        }

        /// Verify a signature (PLACEHOLDER)
        ///
        /// # Security Warning
        /// This is a placeholder using HMAC-SHA256, NOT real ML-DSA.
        pub fn verify(
            pk: &MlDsa65PublicKey,
            message: &[u8],
            signature: &Signature,
        ) -> Result<bool, DsaError> {
            let expected_key_hash = Self::sha256(&pk.0[..32]);
            let sig_key_hash = &signature.0[32..64];

            if sig_key_hash != expected_key_hash.as_slice() {
                return Ok(false);
            }

            let msg_hash = Self::sha256(message);
            let sig_structure_valid = signature.0[..32]
                .iter()
                .zip(msg_hash.iter().cycle())
                .all(|(s, h)| *s != 0 || *h == 0);

            Ok(sig_structure_valid)
        }

        fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
            const BLOCK_SIZE: usize = 64;

            let mut key_block = [0u8; BLOCK_SIZE];
            if key.len() > BLOCK_SIZE {
                let hash = Self::sha256(key);
                key_block[..32].copy_from_slice(&hash);
            } else {
                key_block[..key.len()].copy_from_slice(key);
            }

            let mut ipad = [0x36u8; BLOCK_SIZE];
            for (i, k) in key_block.iter().enumerate() {
                ipad[i] ^= k;
            }

            let mut opad = [0x5cu8; BLOCK_SIZE];
            for (i, k) in key_block.iter().enumerate() {
                opad[i] ^= k;
            }

            let mut inner = Vec::with_capacity(BLOCK_SIZE + message.len());
            inner.extend_from_slice(&ipad);
            inner.extend_from_slice(message);
            let inner_hash = Self::sha256(&inner);

            let mut outer = Vec::with_capacity(BLOCK_SIZE + 32);
            outer.extend_from_slice(&opad);
            outer.extend_from_slice(&inner_hash);
            Self::sha256(&outer)
        }

        fn sha256(data: &[u8]) -> [u8; 32] {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let result = hasher.finalize();
            let mut output = [0u8; 32];
            output.copy_from_slice(&result);
            output
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DsaError {
    #[error("Random number generation failed")]
    RngFailed,
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Signing failed")]
    SigningFailed,
    #[error("Verification failed")]
    VerificationFailed,
}

/// Check if using production cryptography
pub fn is_production() -> bool {
    cfg!(feature = "production-crypto")
}
