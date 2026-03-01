//! Error Types for RuvLLM Integration
//!
//! Defines error types specific to the Prime-Radiant + RuvLLM integration layer.
//! These errors wrap both Prime-Radiant coherence errors and RuvLLM-specific failures.

use crate::error::{CoherenceError, ComputationError, GovernanceError, SubstrateError};
use crate::types::{EdgeId, NodeId, WitnessId};
use thiserror::Error;

/// Top-level error for RuvLLM integration operations
#[derive(Debug, Error)]
pub enum RuvllmIntegrationError {
    // =========================================================================
    // Coherence Validation Errors (ADR-CE-016)
    // =========================================================================
    /// Failed to convert context to sheaf nodes
    #[error("Context conversion failed: {0}")]
    ContextConversionFailed(String),

    /// Failed to convert response to sheaf nodes
    #[error("Response conversion failed: {0}")]
    ResponseConversionFailed(String),

    /// Embedding dimension mismatch
    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    EmbeddingDimensionMismatch {
        /// Expected embedding dimension
        expected: usize,
        /// Actual embedding dimension
        actual: usize,
    },

    /// Claim extraction failed
    #[error("Failed to extract claims from response: {0}")]
    ClaimExtractionFailed(String),

    /// Semantic relation detection failed
    #[error("Failed to detect semantic relations: {0}")]
    SemanticRelationFailed(String),

    /// Coherence validation timed out
    #[error("Coherence validation timed out after {timeout_ms}ms")]
    ValidationTimeout {
        /// Timeout in milliseconds
        timeout_ms: u64,
    },

    // =========================================================================
    // Witness Log Errors (ADR-CE-017)
    // =========================================================================
    /// Failed to create generation witness
    #[error("Failed to create generation witness: {0}")]
    WitnessCreationFailed(String),

    /// Witness chain integrity violation
    #[error("Witness chain integrity violation: {0}")]
    WitnessChainIntegrity(String),

    /// Failed to link inference and coherence witnesses
    #[error("Failed to link witnesses: inference={inference_id}, coherence={coherence_id}")]
    WitnessLinkFailed {
        /// Inference witness ID
        inference_id: String,
        /// Coherence witness ID
        coherence_id: WitnessId,
    },

    /// Hash chain computation failed
    #[error("Hash chain computation failed: {0}")]
    HashChainFailed(String),

    // =========================================================================
    // Pattern Bridge Errors (ADR-CE-018)
    // =========================================================================
    /// Pattern not found in ReasoningBank
    #[error("Pattern not found: {0}")]
    PatternNotFound(String),

    /// Failed to extract embeddings from pattern
    #[error("Failed to extract embeddings from pattern: {0}")]
    EmbeddingExtractionFailed(String),

    /// Restriction map training failed
    #[error("Restriction map training failed: {0}")]
    RestrictionMapTrainingFailed(String),

    /// Verdict processing failed
    #[error("Failed to process verdict: {0}")]
    VerdictProcessingFailed(String),

    /// Pattern consolidation failed
    #[error("Pattern consolidation failed: {0}")]
    ConsolidationFailed(String),

    // =========================================================================
    // Memory Layer Errors (ADR-CE-019)
    // =========================================================================
    /// Memory entry conversion failed
    #[error("Memory entry conversion to node failed: {0}")]
    MemoryConversionFailed(String),

    /// Memory type not supported
    #[error("Memory type not supported for coherence tracking: {0}")]
    UnsupportedMemoryType(String),

    /// Failed to find related memories
    #[error("Failed to find related memories: {0}")]
    RelatedMemorySearchFailed(String),

    /// Memory coherence check failed
    #[error("Memory coherence check failed: node={node_id}")]
    MemoryCoherenceCheckFailed {
        /// Node ID of the memory entry
        node_id: NodeId,
    },

    /// Circular memory reference detected
    #[error("Circular memory reference detected: {0}")]
    CircularMemoryReference(String),

    // =========================================================================
    // Confidence Errors (ADR-CE-020)
    // =========================================================================
    /// Confidence computation failed
    #[error("Confidence computation failed: {0}")]
    ConfidenceComputationFailed(String),

    /// Invalid energy scale parameter
    #[error("Invalid energy scale: {scale} (must be positive)")]
    InvalidEnergyScale {
        /// The invalid scale value
        scale: f32,
    },

    /// Confidence threshold out of range
    #[error("Confidence threshold out of range: {threshold} (must be 0.0-1.0)")]
    InvalidConfidenceThreshold {
        /// The invalid threshold value
        threshold: f32,
    },

    /// Energy breakdown unavailable
    #[error("Energy breakdown unavailable for confidence explanation")]
    EnergyBreakdownUnavailable,

    // =========================================================================
    // Wrapped Errors from Other Layers
    // =========================================================================
    /// Error from Prime-Radiant coherence computation
    #[error("Coherence error: {0}")]
    Coherence(#[from] CoherenceError),

    /// Error from Prime-Radiant substrate
    #[error("Substrate error: {0}")]
    Substrate(#[from] SubstrateError),

    /// Error from Prime-Radiant governance
    #[error("Governance error: {0}")]
    Governance(#[from] GovernanceError),

    /// Error from Prime-Radiant computation
    #[error("Computation error: {0}")]
    Computation(#[from] ComputationError),

    /// Generic internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Result type for RuvLLM integration operations
pub type RuvllmIntegrationResult<T> = std::result::Result<T, RuvllmIntegrationError>;

/// Alias for backward compatibility with alternate naming convention
pub type RuvLlmIntegrationError = RuvllmIntegrationError;

/// Alias for Result type
pub type Result<T> = RuvllmIntegrationResult<T>;

// ============================================================================
// Error Conversion Utilities
// ============================================================================

impl RuvllmIntegrationError {
    /// Create a context conversion error
    pub fn context_conversion(msg: impl Into<String>) -> Self {
        Self::ContextConversionFailed(msg.into())
    }

    /// Create a response conversion error
    pub fn response_conversion(msg: impl Into<String>) -> Self {
        Self::ResponseConversionFailed(msg.into())
    }

    /// Create a witness creation error
    pub fn witness_creation(msg: impl Into<String>) -> Self {
        Self::WitnessCreationFailed(msg.into())
    }

    /// Create a pattern not found error
    pub fn pattern_not_found(pattern_id: impl Into<String>) -> Self {
        Self::PatternNotFound(pattern_id.into())
    }

    /// Create a restriction map training error
    pub fn restriction_training(msg: impl Into<String>) -> Self {
        Self::RestrictionMapTrainingFailed(msg.into())
    }

    /// Create a memory conversion error
    pub fn memory_conversion(msg: impl Into<String>) -> Self {
        Self::MemoryConversionFailed(msg.into())
    }

    /// Create a confidence computation error
    pub fn confidence(msg: impl Into<String>) -> Self {
        Self::ConfidenceComputationFailed(msg.into())
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Create a config error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Check if this is a validation-related error
    pub fn is_validation_error(&self) -> bool {
        matches!(
            self,
            Self::ContextConversionFailed(_)
                | Self::ResponseConversionFailed(_)
                | Self::EmbeddingDimensionMismatch { .. }
                | Self::ClaimExtractionFailed(_)
                | Self::SemanticRelationFailed(_)
                | Self::ValidationTimeout { .. }
        )
    }

    /// Check if this is a witness-related error
    pub fn is_witness_error(&self) -> bool {
        matches!(
            self,
            Self::WitnessCreationFailed(_)
                | Self::WitnessChainIntegrity(_)
                | Self::WitnessLinkFailed { .. }
                | Self::HashChainFailed(_)
        )
    }

    /// Check if this is a pattern bridge error
    pub fn is_pattern_error(&self) -> bool {
        matches!(
            self,
            Self::PatternNotFound(_)
                | Self::EmbeddingExtractionFailed(_)
                | Self::RestrictionMapTrainingFailed(_)
                | Self::VerdictProcessingFailed(_)
                | Self::ConsolidationFailed(_)
        )
    }

    /// Check if this is a memory layer error
    pub fn is_memory_error(&self) -> bool {
        matches!(
            self,
            Self::MemoryConversionFailed(_)
                | Self::UnsupportedMemoryType(_)
                | Self::RelatedMemorySearchFailed(_)
                | Self::MemoryCoherenceCheckFailed { .. }
                | Self::CircularMemoryReference(_)
        )
    }

    /// Check if this is a confidence error
    pub fn is_confidence_error(&self) -> bool {
        matches!(
            self,
            Self::ConfidenceComputationFailed(_)
                | Self::InvalidEnergyScale { .. }
                | Self::InvalidConfidenceThreshold { .. }
                | Self::EnergyBreakdownUnavailable
        )
    }

    /// Get the ADR reference for this error category
    pub fn adr_reference(&self) -> Option<&'static str> {
        if self.is_validation_error() {
            Some(super::adr_references::COHERENCE_VALIDATOR)
        } else if self.is_witness_error() {
            Some(super::adr_references::UNIFIED_WITNESS)
        } else if self.is_pattern_error() {
            Some(super::adr_references::PATTERN_BRIDGE)
        } else if self.is_memory_error() {
            Some(super::adr_references::MEMORY_AS_NODES)
        } else if self.is_confidence_error() {
            Some(super::adr_references::CONFIDENCE_FROM_ENERGY)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = RuvllmIntegrationError::context_conversion("invalid format");
        assert!(err.is_validation_error());
        assert_eq!(
            err.adr_reference(),
            Some(super::super::adr_references::COHERENCE_VALIDATOR)
        );
    }

    #[test]
    fn test_witness_error() {
        let err = RuvllmIntegrationError::witness_creation("chain broken");
        assert!(err.is_witness_error());
        assert!(!err.is_validation_error());
    }

    #[test]
    fn test_pattern_error() {
        let err = RuvllmIntegrationError::pattern_not_found("pattern-123");
        assert!(err.is_pattern_error());
    }

    #[test]
    fn test_memory_error() {
        let err = RuvllmIntegrationError::memory_conversion("embedding missing");
        assert!(err.is_memory_error());
    }

    #[test]
    fn test_confidence_error() {
        let err = RuvllmIntegrationError::InvalidEnergyScale { scale: -1.0 };
        assert!(err.is_confidence_error());
    }

    #[test]
    fn test_error_display() {
        let err = RuvllmIntegrationError::EmbeddingDimensionMismatch {
            expected: 768,
            actual: 512,
        };
        let msg = err.to_string();
        assert!(msg.contains("768"));
        assert!(msg.contains("512"));
    }
}
