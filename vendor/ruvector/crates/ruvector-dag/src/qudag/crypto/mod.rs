//! Quantum-Resistant Cryptography for QuDAG
//!
//! # Security Status
//!
//! | Component | With `production-crypto` | Without Feature |
//! |-----------|-------------------------|-----------------|
//! | ML-DSA-65 | ✓ Dilithium3 | ✗ HMAC-SHA256 placeholder |
//! | ML-KEM-768 | ✓ Kyber768 | ✗ HKDF-SHA256 placeholder |
//! | Differential Privacy | ✓ Production | ✓ Production |
//! | Keystore | ✓ Uses zeroize | ✓ Uses zeroize |
//!
//! ## Enabling Production Cryptography
//!
//! ```toml
//! ruvector-dag = { version = "0.1", features = ["production-crypto"] }
//! ```
//!
//! ## Startup Check
//!
//! Call [`check_crypto_security()`] at application startup to log security status.

mod differential_privacy;
mod identity;
mod keystore;
mod ml_dsa;
mod ml_kem;
mod security_notice;

pub use differential_privacy::{DifferentialPrivacy, DpConfig};
pub use identity::{IdentityError, QuDagIdentity};
pub use keystore::{KeystoreError, SecureKeystore};
pub use ml_dsa::{
    is_production as is_ml_dsa_production, DsaError, MlDsa65, MlDsa65PublicKey, MlDsa65SecretKey,
    Signature, ML_DSA_65_PUBLIC_KEY_SIZE, ML_DSA_65_SECRET_KEY_SIZE, ML_DSA_65_SIGNATURE_SIZE,
};
pub use ml_kem::{
    is_production as is_ml_kem_production, EncapsulatedKey, KemError, MlKem768, MlKem768PublicKey,
    MlKem768SecretKey, ML_KEM_768_CIPHERTEXT_SIZE, ML_KEM_768_PUBLIC_KEY_SIZE,
    ML_KEM_768_SECRET_KEY_SIZE, SHARED_SECRET_SIZE,
};
pub use security_notice::{
    check_crypto_security, is_production_ready, security_status, SecurityStatus,
};
