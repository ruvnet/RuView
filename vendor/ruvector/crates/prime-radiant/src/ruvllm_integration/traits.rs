//! Trait Definitions for Loose Coupling
//!
//! This module defines traits that allow loose coupling between Prime-Radiant
//! and RuvLLM. By depending on traits rather than concrete types, the integration
//! layer can work with different implementations and allows for easier testing.
//!
//! # Design Philosophy
//!
//! The traits follow the Dependency Inversion Principle:
//! - High-level integration logic depends on abstractions (traits)
//! - Low-level RuvLLM and Prime-Radiant types implement these traits
//! - This allows either side to evolve independently

use crate::coherence::CoherenceEnergy;
use crate::execution::GateDecision;
use crate::governance::WitnessRecord;
use crate::types::{Hash, NodeId, Timestamp, WitnessId};

use super::error::RuvllmIntegrationResult;

use std::collections::HashMap;

// ============================================================================
// COHERENCE VALIDATION TRAITS (ADR-CE-016)
// ============================================================================

/// Represents content that can be validated for coherence.
///
/// Implementations convert RuvLLM types (responses, contexts) into a form
/// that can be processed by Prime-Radiant's coherence engine.
pub trait CoherenceValidatable {
    /// Get the embedding representation for coherence checking.
    fn embedding(&self) -> &[f32];

    /// Get the dimension of the embedding.
    fn embedding_dim(&self) -> usize {
        self.embedding().len()
    }

    /// Extract claims/assertions from this content.
    ///
    /// Claims are individual statements that can be checked for consistency.
    fn extract_claims(&self) -> Vec<Claim>;

    /// Get metadata for node creation.
    fn metadata(&self) -> HashMap<String, String>;

    /// Get a unique identifier for this content.
    fn content_id(&self) -> String;
}

/// A claim/assertion extracted from content.
#[derive(Debug, Clone)]
pub struct Claim {
    /// Unique identifier for this claim
    pub id: String,
    /// Text of the claim
    pub text: String,
    /// Embedding of the claim
    pub embedding: Vec<f32>,
    /// Confidence in the claim extraction (0.0-1.0)
    pub extraction_confidence: f32,
    /// Type of claim
    pub claim_type: ClaimType,
}

/// Types of claims that can be extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimType {
    /// A factual assertion
    Factual,
    /// A causal relationship
    Causal,
    /// A temporal relationship
    Temporal,
    /// A comparison
    Comparison,
    /// An opinion or subjective statement
    Opinion,
    /// Unknown type
    Unknown,
}

/// Represents a context source (facts, previous messages, etc.).
pub trait ContextSource {
    /// Get facts from this context.
    fn facts(&self) -> Vec<Fact>;

    /// Get the overall context embedding.
    fn context_embedding(&self) -> &[f32];

    /// Get the context scope identifier.
    fn scope_id(&self) -> String;
}

/// A fact from the context.
#[derive(Debug, Clone)]
pub struct Fact {
    /// Unique identifier
    pub id: String,
    /// The node ID if already in the graph
    pub node_id: Option<NodeId>,
    /// Embedding
    pub embedding: Vec<f32>,
    /// Source of the fact
    pub source: String,
    /// Confidence in the fact (0.0-1.0)
    pub confidence: f32,
}

/// Semantic relation between a claim and a fact.
#[derive(Debug, Clone)]
pub struct SemanticRelation {
    /// Type of relation
    pub relation_type: RelationType,
    /// Strength of the relation (0.0-1.0)
    pub strength: f32,
}

/// Types of semantic relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
    /// Claim supports the fact
    Supports,
    /// Claim contradicts the fact
    Contradicts,
    /// Claim is unrelated
    Unrelated,
    /// Claim extends/elaborates the fact
    Extends,
    /// Claim cites the fact
    Cites,
}

impl Claim {
    /// Check if this claim relates to a fact.
    ///
    /// This is a placeholder - actual implementation would use
    /// semantic similarity and NLI models.
    pub fn relates_to(&self, fact: &Fact) -> Option<SemanticRelation> {
        // Compute cosine similarity
        let similarity = cosine_similarity(&self.embedding, &fact.embedding);

        if similarity > 0.7 {
            Some(SemanticRelation {
                relation_type: RelationType::Supports,
                strength: similarity,
            })
        } else if similarity < 0.3 {
            Some(SemanticRelation {
                relation_type: RelationType::Contradicts,
                strength: 1.0 - similarity,
            })
        } else {
            None
        }
    }
}

// ============================================================================
// UNIFIED WITNESS TRAITS (ADR-CE-017)
// ============================================================================

/// Provider of unified witness records across inference and coherence.
pub trait UnifiedWitnessProvider {
    /// Create a generation witness linking inference and coherence decisions.
    fn create_generation_witness(
        &mut self,
        prompt: &str,
        response: &str,
        coherence_decision: &GateDecision,
        coherence_witness: &WitnessRecord,
    ) -> RuvllmIntegrationResult<GenerationWitnessRef>;

    /// Get a witness by ID.
    fn get_witness(&self, id: &WitnessId) -> Option<&WitnessRecord>;

    /// Verify witness chain integrity.
    fn verify_chain_integrity(&self) -> RuvllmIntegrationResult<bool>;

    /// Get the current chain hash.
    fn chain_hash(&self) -> Hash;
}

/// Reference to a generation witness.
#[derive(Debug, Clone)]
pub struct GenerationWitnessRef {
    /// Inference witness ID (from RuvLLM)
    pub inference_id: String,
    /// Coherence witness ID (from Prime-Radiant)
    pub coherence_id: WitnessId,
    /// Combined hash
    pub combined_hash: Hash,
    /// Timestamp
    pub timestamp: Timestamp,
}

// ============================================================================
// PATTERN BRIDGE TRAITS (ADR-CE-018)
// ============================================================================

/// Bridge between RuvLLM patterns and Prime-Radiant restriction maps.
pub trait PatternBridge {
    /// Learn from a successful pattern.
    fn learn_success(
        &mut self,
        pattern_id: &str,
        source_embedding: &[f32],
        target_embedding: &[f32],
    ) -> RuvllmIntegrationResult<()>;

    /// Learn from a failed pattern.
    fn learn_failure(
        &mut self,
        pattern_id: &str,
        source_embedding: &[f32],
        target_embedding: &[f32],
        failure_residual: &[f32],
    ) -> RuvllmIntegrationResult<()>;

    /// Get the restriction map for a pattern.
    fn get_restriction_map(&self, pattern_id: &str) -> Option<RestrictionMapRef>;

    /// Export learned maps for use in Prime-Radiant graph.
    fn export_to_graph(&self) -> Vec<(String, RestrictionMapRef)>;
}

/// Reference to a restriction map.
#[derive(Debug, Clone)]
pub struct RestrictionMapRef {
    /// Input dimension
    pub input_dim: usize,
    /// Output dimension
    pub output_dim: usize,
    /// Pattern ID this was learned from
    pub source_pattern: String,
    /// Number of training examples
    pub training_count: usize,
}

// ============================================================================
// MEMORY COHERENCE TRAITS (ADR-CE-019)
// ============================================================================

/// Memory type enumeration for coherence tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    /// Long-term agentic patterns
    Agentic,
    /// Current working context
    Working,
    /// Conversation history
    Episodic,
}

/// A memory entry for coherence tracking.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    /// Unique identifier
    pub id: String,
    /// Memory type
    pub memory_type: MemoryType,
    /// Embedding vector
    pub embedding: Vec<f32>,
    /// Metadata
    pub metadata: HashMap<String, String>,
    /// Timestamp
    pub timestamp: Timestamp,
}

/// Provider of memory coherence checks.
pub trait MemoryCoherenceProvider {
    /// Add a memory entry with coherence checking.
    fn add_with_coherence(
        &mut self,
        entry: MemoryEntry,
    ) -> RuvllmIntegrationResult<MemoryAddResult>;

    /// Check if adding an entry would cause incoherence.
    fn check_coherence(&self, entry: &MemoryEntry) -> RuvllmIntegrationResult<f32>;

    /// Get related memories for an entry.
    fn find_related(&self, entry: &MemoryEntry, limit: usize) -> Vec<String>;

    /// Get the current coherence state of all memories.
    fn memory_coherence_energy(&self) -> f32;
}

/// Result of adding a memory with coherence tracking.
#[derive(Debug, Clone)]
pub struct MemoryAddResult {
    /// Memory ID assigned
    pub memory_id: String,
    /// Node ID in sheaf graph
    pub node_id: NodeId,
    /// Coherence energy after adding
    pub energy: f32,
    /// Whether the memory is coherent with existing
    pub coherent: bool,
    /// IDs of conflicting memories (if any)
    pub conflicts: Vec<String>,
}

// ============================================================================
// CONFIDENCE TRAITS (ADR-CE-020)
// ============================================================================

/// Source of confidence derived from coherence energy.
pub trait ConfidenceSource {
    /// Compute confidence from coherence energy.
    ///
    /// Low energy = high confidence (coherent)
    /// High energy = low confidence (incoherent)
    fn confidence_from_energy(&self, energy: &CoherenceEnergy) -> ConfidenceResult;

    /// Get the energy threshold for 50% confidence.
    fn threshold(&self) -> f32;

    /// Get the energy scale parameter.
    fn scale(&self) -> f32;
}

/// Result of confidence computation.
#[derive(Debug, Clone)]
pub struct ConfidenceResult {
    /// Confidence value (0.0-1.0)
    pub value: f32,
    /// Human-readable explanation
    pub explanation: String,
    /// Whether this confidence is backed by a witness
    pub witness_backed: bool,
    /// Top contributing edges to uncertainty (if available)
    pub uncertainty_sources: Vec<UncertaintySource>,
}

/// Source of uncertainty in confidence calculation.
#[derive(Debug, Clone)]
pub struct UncertaintySource {
    /// Description of the source
    pub description: String,
    /// Energy contribution
    pub energy_contribution: f32,
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_claim_type() {
        assert_ne!(ClaimType::Factual, ClaimType::Opinion);
    }

    #[test]
    fn test_relation_type() {
        assert_ne!(RelationType::Supports, RelationType::Contradicts);
    }

    #[test]
    fn test_memory_type() {
        assert_ne!(MemoryType::Agentic, MemoryType::Working);
        assert_ne!(MemoryType::Working, MemoryType::Episodic);
    }
}
