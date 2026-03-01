//! # Security Notice for QuDAG Cryptography
//!
//! ## Security Status
//!
//! | Component | With `production-crypto` | Without Feature |
//! |-----------|-------------------------|-----------------|
//! | ML-DSA-65 | ✓ Dilithium3 (NIST PQC) | ✗ HMAC-SHA256 placeholder |
//! | ML-KEM-768 | ✓ Kyber768 (NIST PQC) | ✗ HKDF-SHA256 placeholder |
//! | Differential Privacy | ✓ Production-ready | ✓ Production-ready |
//! | Keystore | ✓ Uses zeroize | ✓ Uses zeroize |
//!
//! ## Enabling Production Cryptography
//!
//! Add to your Cargo.toml:
//! ```toml
//! ruvector-dag = { version = "0.1", features = ["production-crypto"] }
//! ```
//!
//! ## NIST Post-Quantum Cryptography Standards
//!
//! - **FIPS 203**: ML-KEM (Module-Lattice Key Encapsulation Mechanism)
//! - **FIPS 204**: ML-DSA (Module-Lattice Digital Signature Algorithm)
//!
//! The `production-crypto` feature uses:
//! - `pqcrypto-dilithium` (Dilithium3 ≈ ML-DSA-65 security level)
//! - `pqcrypto-kyber` (Kyber768 ≈ ML-KEM-768 security level)
//!
//! ## Security Contact
//!
//! Report security issues to: security@ruvector.io

use super::{ml_dsa, ml_kem};

/// Check cryptographic security at startup
///
/// Call this function during application initialization to log
/// warnings about placeholder crypto usage.
///
/// # Example
///
/// ```rust,ignore
/// fn main() {
///     ruvector_dag::qudag::crypto::check_crypto_security();
///     // ... rest of application
/// }
/// ```
#[cold]
pub fn check_crypto_security() {
    let status = security_status();

    if status.production_ready {
        tracing::info!("✓ QuDAG cryptography: Production mode enabled (Dilithium3 + Kyber768)");
    } else {
        tracing::warn!(
            "⚠️ SECURITY WARNING: Using placeholder cryptography. \
             NOT suitable for production. Enable 'production-crypto' feature."
        );
        tracing::warn!(
            "   ML-DSA: {} | ML-KEM: {}",
            if status.ml_dsa_ready {
                "Ready"
            } else {
                "PLACEHOLDER"
            },
            if status.ml_kem_ready {
                "Ready"
            } else {
                "PLACEHOLDER"
            }
        );
    }
}

/// Runtime check for production readiness
pub fn is_production_ready() -> bool {
    ml_dsa::is_production() && ml_kem::is_production()
}

/// Get detailed security status report
pub fn security_status() -> SecurityStatus {
    let ml_dsa_ready = ml_dsa::is_production();
    let ml_kem_ready = ml_kem::is_production();

    SecurityStatus {
        ml_dsa_ready,
        ml_kem_ready,
        dp_ready: true,
        keystore_ready: true,
        production_ready: ml_dsa_ready && ml_kem_ready,
    }
}

/// Security status of cryptographic components
#[derive(Debug, Clone)]
pub struct SecurityStatus {
    /// ML-DSA-65 uses real implementation (Dilithium3)
    pub ml_dsa_ready: bool,
    /// ML-KEM-768 uses real implementation (Kyber768)
    pub ml_kem_ready: bool,
    /// Differential privacy is properly implemented
    pub dp_ready: bool,
    /// Keystore uses proper zeroization
    pub keystore_ready: bool,
    /// Overall production readiness
    pub production_ready: bool,
}

impl std::fmt::Display for SecurityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "QuDAG Cryptography Security Status:")?;
        writeln!(
            f,
            "  ML-DSA-65:  {} ({})",
            if self.ml_dsa_ready { "✓" } else { "✗" },
            if self.ml_dsa_ready {
                "Dilithium3"
            } else {
                "PLACEHOLDER"
            }
        )?;
        writeln!(
            f,
            "  ML-KEM-768: {} ({})",
            if self.ml_kem_ready { "✓" } else { "✗" },
            if self.ml_kem_ready {
                "Kyber768"
            } else {
                "PLACEHOLDER"
            }
        )?;
        writeln!(
            f,
            "  DP:         {} ({})",
            if self.dp_ready { "✓" } else { "✗" },
            if self.dp_ready { "Ready" } else { "Not Ready" }
        )?;
        writeln!(
            f,
            "  Keystore:   {} ({})",
            if self.keystore_ready { "✓" } else { "✗" },
            if self.keystore_ready {
                "Ready"
            } else {
                "Not Ready"
            }
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "  OVERALL:    {}",
            if self.production_ready {
                "✓ PRODUCTION READY (Post-Quantum Secure)"
            } else {
                "✗ NOT PRODUCTION READY - Enable 'production-crypto' feature"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_status() {
        let status = security_status();
        // These should always be ready
        assert!(status.dp_ready);
        assert!(status.keystore_ready);

        // ML-DSA and ML-KEM depend on feature flag
        #[cfg(feature = "production-crypto")]
        {
            assert!(status.ml_dsa_ready);
            assert!(status.ml_kem_ready);
            assert!(status.production_ready);
        }

        #[cfg(not(feature = "production-crypto"))]
        {
            assert!(!status.ml_dsa_ready);
            assert!(!status.ml_kem_ready);
            assert!(!status.production_ready);
        }
    }

    #[test]
    fn test_is_production_ready() {
        #[cfg(feature = "production-crypto")]
        assert!(is_production_ready());

        #[cfg(not(feature = "production-crypto"))]
        assert!(!is_production_ready());
    }

    #[test]
    fn test_display() {
        let status = security_status();
        let display = format!("{}", status);
        assert!(display.contains("QuDAG Cryptography Security Status"));
        assert!(display.contains("ML-DSA-65"));
        assert!(display.contains("ML-KEM-768"));
    }
}
