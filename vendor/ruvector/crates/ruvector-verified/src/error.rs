//! Verification error types.
//!
//! Maps lean-agentic kernel errors to RuVector verification errors.

use thiserror::Error;

/// Errors from the formal verification layer.
#[derive(Debug, Error)]
pub enum VerificationError {
    /// Vector dimension does not match the index dimension.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: u32, actual: u32 },

    /// The lean-agentic type checker rejected the proof term.
    #[error("type check failed: {0}")]
    TypeCheckFailed(String),

    /// Proof construction failed during term building.
    #[error("proof construction failed: {0}")]
    ProofConstructionFailed(String),

    /// The conversion engine exhausted its fuel budget.
    #[error("conversion timeout: exceeded {max_reductions} reduction steps")]
    ConversionTimeout { max_reductions: u32 },

    /// Unification of proof constraints failed.
    #[error("unification failed: {0}")]
    UnificationFailed(String),

    /// The arena ran out of term slots.
    #[error("arena exhausted: {allocated} terms allocated")]
    ArenaExhausted { allocated: u32 },

    /// A required declaration was not found in the proof environment.
    #[error("declaration not found: {name}")]
    DeclarationNotFound { name: String },

    /// Ed25519 proof signing or verification failed.
    #[error("attestation error: {0}")]
    AttestationError(String),
}

/// Convenience type alias.
pub type Result<T> = std::result::Result<T, VerificationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_dimension_mismatch() {
        let e = VerificationError::DimensionMismatch {
            expected: 128,
            actual: 256,
        };
        assert_eq!(e.to_string(), "dimension mismatch: expected 128, got 256");
    }

    #[test]
    fn error_display_type_check() {
        let e = VerificationError::TypeCheckFailed("bad term".into());
        assert_eq!(e.to_string(), "type check failed: bad term");
    }

    #[test]
    fn error_display_timeout() {
        let e = VerificationError::ConversionTimeout {
            max_reductions: 10000,
        };
        assert_eq!(
            e.to_string(),
            "conversion timeout: exceeded 10000 reduction steps"
        );
    }

    #[test]
    fn error_display_arena() {
        let e = VerificationError::ArenaExhausted { allocated: 42 };
        assert_eq!(e.to_string(), "arena exhausted: 42 terms allocated");
    }

    #[test]
    fn error_display_attestation() {
        let e = VerificationError::AttestationError("sig invalid".into());
        assert_eq!(e.to_string(), "attestation error: sig invalid");
    }
}
