//! ML-KEM-768 Key Encapsulation Mechanism
//!
//! # Security Status
//!
//! With `production-crypto` feature: Uses `pqcrypto-kyber` (Kyber768 ≈ ML-KEM-768)
//! Without feature: Uses HKDF-SHA256 placeholder (NOT quantum-resistant)
//!
//! ## Production Use
//!
//! Enable the `production-crypto` feature in Cargo.toml:
//! ```toml
//! ruvector-dag = { version = "0.1", features = ["production-crypto"] }
//! ```

use zeroize::Zeroize;

// ML-KEM-768 sizes (FIPS 203)
// Note: Kyber768 is the closest match to ML-KEM-768 security level
pub const ML_KEM_768_PUBLIC_KEY_SIZE: usize = 1184;
pub const ML_KEM_768_SECRET_KEY_SIZE: usize = 2400;
pub const ML_KEM_768_CIPHERTEXT_SIZE: usize = 1088;
pub const SHARED_SECRET_SIZE: usize = 32;

#[derive(Clone)]
pub struct MlKem768PublicKey(pub [u8; ML_KEM_768_PUBLIC_KEY_SIZE]);

#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct MlKem768SecretKey(pub [u8; ML_KEM_768_SECRET_KEY_SIZE]);

#[derive(Clone)]
pub struct EncapsulatedKey {
    pub ciphertext: [u8; ML_KEM_768_CIPHERTEXT_SIZE],
    pub shared_secret: [u8; SHARED_SECRET_SIZE],
}

pub struct MlKem768;

// ============================================================================
// Production Implementation (using pqcrypto-kyber)
// ============================================================================

#[cfg(feature = "production-crypto")]
mod production {
    use super::*;
    use pqcrypto_kyber::kyber768;
    use pqcrypto_traits::kem::{Ciphertext, PublicKey, SecretKey, SharedSecret};

    impl MlKem768 {
        /// Generate a new keypair using real Kyber768
        pub fn generate_keypair() -> Result<(MlKem768PublicKey, MlKem768SecretKey), KemError> {
            let (pk, sk) = kyber768::keypair();

            let pk_bytes = pk.as_bytes();
            let sk_bytes = sk.as_bytes();

            // Kyber768 sizes: pk=1184, sk=2400 (matches ML-KEM-768)
            let mut pk_arr = [0u8; ML_KEM_768_PUBLIC_KEY_SIZE];
            let mut sk_arr = [0u8; ML_KEM_768_SECRET_KEY_SIZE];

            if pk_bytes.len() != ML_KEM_768_PUBLIC_KEY_SIZE {
                return Err(KemError::InvalidPublicKey);
            }
            if sk_bytes.len() != ML_KEM_768_SECRET_KEY_SIZE {
                return Err(KemError::DecapsulationFailed);
            }

            pk_arr.copy_from_slice(pk_bytes);
            sk_arr.copy_from_slice(sk_bytes);

            Ok((MlKem768PublicKey(pk_arr), MlKem768SecretKey(sk_arr)))
        }

        /// Encapsulate a shared secret using real Kyber768
        pub fn encapsulate(pk: &MlKem768PublicKey) -> Result<EncapsulatedKey, KemError> {
            let public_key =
                kyber768::PublicKey::from_bytes(&pk.0).map_err(|_| KemError::InvalidPublicKey)?;

            let (ss, ct) = kyber768::encapsulate(&public_key);

            let ss_bytes = ss.as_bytes();
            let ct_bytes = ct.as_bytes();

            let mut shared_secret = [0u8; SHARED_SECRET_SIZE];
            let mut ciphertext = [0u8; ML_KEM_768_CIPHERTEXT_SIZE];

            if ss_bytes.len() != SHARED_SECRET_SIZE {
                return Err(KemError::DecapsulationFailed);
            }
            if ct_bytes.len() != ML_KEM_768_CIPHERTEXT_SIZE {
                return Err(KemError::InvalidCiphertext);
            }

            shared_secret.copy_from_slice(ss_bytes);
            ciphertext.copy_from_slice(ct_bytes);

            Ok(EncapsulatedKey {
                ciphertext,
                shared_secret,
            })
        }

        /// Decapsulate to recover the shared secret using real Kyber768
        pub fn decapsulate(
            sk: &MlKem768SecretKey,
            ciphertext: &[u8; ML_KEM_768_CIPHERTEXT_SIZE],
        ) -> Result<[u8; SHARED_SECRET_SIZE], KemError> {
            let secret_key = kyber768::SecretKey::from_bytes(&sk.0)
                .map_err(|_| KemError::DecapsulationFailed)?;

            let ct = kyber768::Ciphertext::from_bytes(ciphertext)
                .map_err(|_| KemError::InvalidCiphertext)?;

            let ss = kyber768::decapsulate(&ct, &secret_key);
            let ss_bytes = ss.as_bytes();

            let mut shared_secret = [0u8; SHARED_SECRET_SIZE];
            if ss_bytes.len() != SHARED_SECRET_SIZE {
                return Err(KemError::DecapsulationFailed);
            }

            shared_secret.copy_from_slice(ss_bytes);
            Ok(shared_secret)
        }
    }
}

// ============================================================================
// Placeholder Implementation (HKDF-SHA256 - NOT quantum-resistant)
// ============================================================================

#[cfg(not(feature = "production-crypto"))]
mod placeholder {
    use super::*;
    use sha2::{Digest, Sha256};

    impl MlKem768 {
        /// Generate a new keypair (PLACEHOLDER)
        ///
        /// # Security Warning
        /// This is a placeholder using random bytes, NOT real ML-KEM.
        pub fn generate_keypair() -> Result<(MlKem768PublicKey, MlKem768SecretKey), KemError> {
            let mut pk = [0u8; ML_KEM_768_PUBLIC_KEY_SIZE];
            let mut sk = [0u8; ML_KEM_768_SECRET_KEY_SIZE];

            getrandom::getrandom(&mut pk).map_err(|_| KemError::RngFailed)?;
            getrandom::getrandom(&mut sk).map_err(|_| KemError::RngFailed)?;

            Ok((MlKem768PublicKey(pk), MlKem768SecretKey(sk)))
        }

        /// Encapsulate a shared secret (PLACEHOLDER)
        ///
        /// # Security Warning
        /// This is a placeholder using HKDF-SHA256, NOT real ML-KEM.
        pub fn encapsulate(pk: &MlKem768PublicKey) -> Result<EncapsulatedKey, KemError> {
            let mut ephemeral = [0u8; 32];
            getrandom::getrandom(&mut ephemeral).map_err(|_| KemError::RngFailed)?;

            let mut ciphertext = [0u8; ML_KEM_768_CIPHERTEXT_SIZE];

            let pk_hash = Self::sha256(&pk.0[..64]);
            for i in 0..32 {
                ciphertext[i] = ephemeral[i] ^ pk_hash[i];
            }

            let padding = Self::sha256(&ephemeral);
            for i in 32..ML_KEM_768_CIPHERTEXT_SIZE {
                ciphertext[i] = padding[i % 32];
            }

            let shared_secret = Self::hkdf_sha256(&ephemeral, &pk.0[..32], b"ml-kem-768-shared");

            Ok(EncapsulatedKey {
                ciphertext,
                shared_secret,
            })
        }

        /// Decapsulate to recover the shared secret (PLACEHOLDER)
        ///
        /// # Security Warning
        /// This is a placeholder using HKDF-SHA256, NOT real ML-KEM.
        pub fn decapsulate(
            sk: &MlKem768SecretKey,
            ciphertext: &[u8; ML_KEM_768_CIPHERTEXT_SIZE],
        ) -> Result<[u8; SHARED_SECRET_SIZE], KemError> {
            let sk_hash = Self::sha256(&sk.0[..64]);
            let mut ephemeral = [0u8; 32];
            for i in 0..32 {
                ephemeral[i] = ciphertext[i] ^ sk_hash[i];
            }

            let expected_padding = Self::sha256(&ephemeral);
            for i in 32..64.min(ML_KEM_768_CIPHERTEXT_SIZE) {
                if ciphertext[i] != expected_padding[i % 32] {
                    return Err(KemError::InvalidCiphertext);
                }
            }

            let shared_secret = Self::hkdf_sha256(&ephemeral, &sk.0[..32], b"ml-kem-768-shared");
            Ok(shared_secret)
        }

        fn hkdf_sha256(ikm: &[u8], salt: &[u8], info: &[u8]) -> [u8; SHARED_SECRET_SIZE] {
            let prk = Self::hmac_sha256(salt, ikm);
            let mut okm_input = Vec::with_capacity(info.len() + 1);
            okm_input.extend_from_slice(info);
            okm_input.push(1);
            Self::hmac_sha256(&prk, &okm_input)
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
            let mut opad = [0x5cu8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                ipad[i] ^= key_block[i];
                opad[i] ^= key_block[i];
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
pub enum KemError {
    #[error("Random number generation failed")]
    RngFailed,
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid ciphertext")]
    InvalidCiphertext,
    #[error("Decapsulation failed")]
    DecapsulationFailed,
}

/// Check if using production cryptography
pub fn is_production() -> bool {
    cfg!(feature = "production-crypto")
}
