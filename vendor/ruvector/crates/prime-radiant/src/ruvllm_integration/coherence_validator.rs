//! # Sheaf Coherence Validator for RuvLLM Integration
//!
//! This module bridges RuvLLM's CoherenceValidator trait with Prime-Radiant's
//! sheaf-theoretic coherence energy computation.
//!
//! ## Design (ADR-CE-016)
//!
//! The `SheafCoherenceValidator` validates LLM responses by:
//! 1. Converting context and response into sheaf graph nodes
//! 2. Adding edges with semantic implication constraints
//! 3. Computing coherence energy via the sheaf Laplacian
//! 4. Producing a `ValidationResult` with allow/deny, energy, and witness
//!
//! ## Mathematical Foundation
//!
//! For an LLM response validation:
//! - **Nodes**: Context facts, response claims, semantic entities
//! - **Edges**: Logical implications, semantic consistency, factual support
//! - **Residual**: `r_e = rho_ctx(context) - rho_resp(response)` measures contradiction
//! - **Energy**: `E(S) = sum(w_e * ||r_e||^2)` quantifies total incoherence
//!
//! Low energy indicates the response is coherent with the context.
//! High energy triggers escalation or rejection.
//!
//! ## Example
//!
//! ```rust,ignore
//! use prime_radiant::ruvllm_integration::{
//!     SheafCoherenceValidator, ValidationContext, ValidationResult,
//! };
//! use prime_radiant::execution::CoherenceGate;
//! use prime_radiant::governance::PolicyBundleRef;
//!
//! let policy = PolicyBundleRef::placeholder();
//! let gate = CoherenceGate::with_defaults(policy);
//! let validator = SheafCoherenceValidator::new(gate);
//!
//! let ctx = ValidationContext::new()
//!     .with_context_embedding(context_vec)
//!     .with_response_embedding(response_vec);
//!
//! let result = validator.validate(&ctx)?;
//! if result.allowed {
//!     println!("Response is coherent (energy: {})", result.energy);
//! } else {
//!     println!("Response rejected: {}", result.reason.unwrap_or_default());
//! }
//! ```

use crate::coherence::CoherenceEngine;
use crate::error::CoherenceError;
use crate::execution::{
    Action, ActionImpact, ActionMetadata, CoherenceGate, EnergySnapshot, ExecutionContext,
    GateDecision, ScopeId as ExecScopeId, WitnessRecord as ExecWitnessRecord,
};
use crate::governance::{Hash, PolicyBundleRef, Timestamp, WitnessRecord as GovWitnessRecord};
use crate::substrate::{
    RestrictionMap, SheafEdge, SheafEdgeBuilder, SheafGraph, SheafNode, SheafNodeBuilder,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// VALIDATION CONTEXT
// ============================================================================

/// Context for validation containing embeddings and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationContext {
    /// Context embedding (e.g., from retrieval, conversation history)
    pub context_embedding: Vec<f32>,

    /// Response embedding (from the LLM output)
    pub response_embedding: Vec<f32>,

    /// Optional additional context embeddings (supporting evidence)
    pub supporting_embeddings: Vec<Vec<f32>>,

    /// Scope for policy lookup
    pub scope: String,

    /// Edge weights for different semantic relationships
    pub edge_weights: EdgeWeights,

    /// Metadata for audit trail
    pub metadata: HashMap<String, String>,

    /// Unique request ID for tracing
    pub request_id: Uuid,
}

impl ValidationContext {
    /// Create a new validation context with default settings
    pub fn new() -> Self {
        Self {
            context_embedding: Vec::new(),
            response_embedding: Vec::new(),
            supporting_embeddings: Vec::new(),
            scope: "default".to_string(),
            edge_weights: EdgeWeights::default(),
            metadata: HashMap::new(),
            request_id: Uuid::new_v4(),
        }
    }

    /// Set the context embedding
    pub fn with_context_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.context_embedding = embedding;
        self
    }

    /// Set the response embedding
    pub fn with_response_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.response_embedding = embedding;
        self
    }

    /// Add a supporting embedding (e.g., retrieved documents)
    pub fn with_supporting_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.supporting_embeddings.push(embedding);
        self
    }

    /// Set the scope for policy lookup
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = scope.into();
        self
    }

    /// Set custom edge weights
    pub fn with_edge_weights(mut self, weights: EdgeWeights) -> Self {
        self.edge_weights = weights;
        self
    }

    /// Add metadata for audit trail
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set the request ID (for correlation)
    pub fn with_request_id(mut self, id: Uuid) -> Self {
        self.request_id = id;
        self
    }

    /// Get the embedding dimension (assumes all embeddings have same dim)
    pub fn embedding_dim(&self) -> usize {
        if !self.context_embedding.is_empty() {
            self.context_embedding.len()
        } else if !self.response_embedding.is_empty() {
            self.response_embedding.len()
        } else {
            0
        }
    }

    /// Validate that the context is properly configured
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.context_embedding.is_empty() {
            return Err(ValidationError::MissingEmbedding("context".to_string()));
        }
        if self.response_embedding.is_empty() {
            return Err(ValidationError::MissingEmbedding("response".to_string()));
        }
        if self.context_embedding.len() != self.response_embedding.len() {
            return Err(ValidationError::DimensionMismatch {
                context_dim: self.context_embedding.len(),
                response_dim: self.response_embedding.len(),
            });
        }
        for emb in &self.supporting_embeddings {
            if emb.len() != self.context_embedding.len() {
                return Err(ValidationError::DimensionMismatch {
                    context_dim: self.context_embedding.len(),
                    response_dim: emb.len(),
                });
            }
        }
        Ok(())
    }
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// EDGE WEIGHTS
// ============================================================================

/// Weights for different types of semantic edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWeights {
    /// Weight for context-to-response consistency edges
    pub context_response: f32,

    /// Weight for response-to-supporting consistency edges
    pub response_support: f32,

    /// Weight for context-to-supporting consistency edges
    pub context_support: f32,

    /// Weight for intra-supporting consistency edges
    pub support_support: f32,
}

impl EdgeWeights {
    /// Create new edge weights
    pub fn new(
        context_response: f32,
        response_support: f32,
        context_support: f32,
        support_support: f32,
    ) -> Self {
        Self {
            context_response,
            response_support,
            context_support,
            support_support,
        }
    }

    /// Strict weights (higher penalties for inconsistency)
    pub fn strict() -> Self {
        Self {
            context_response: 2.0,
            response_support: 1.5,
            context_support: 1.0,
            support_support: 0.5,
        }
    }

    /// Permissive weights (lower penalties)
    pub fn permissive() -> Self {
        Self {
            context_response: 1.0,
            response_support: 0.5,
            context_support: 0.3,
            support_support: 0.2,
        }
    }
}

impl Default for EdgeWeights {
    fn default() -> Self {
        Self {
            context_response: 1.5,
            response_support: 1.0,
            context_support: 0.8,
            support_support: 0.3,
        }
    }
}

// ============================================================================
// VALIDATION RESULT
// ============================================================================

/// Result of coherence validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the response was allowed
    pub allowed: bool,

    /// Computed coherence energy (lower = more coherent)
    pub energy: f32,

    /// Reason for rejection (if not allowed)
    pub reason: Option<String>,

    /// Witness record for audit trail
    pub witness: ValidationWitness,

    /// Per-edge breakdown of energy contributions
    pub edge_breakdown: HashMap<String, f32>,

    /// Timestamp of validation
    pub timestamp: Timestamp,

    /// Request ID for correlation
    pub request_id: Uuid,
}

impl ValidationResult {
    /// Create an allowing result
    pub fn allow(energy: f32, witness: ValidationWitness, request_id: Uuid) -> Self {
        Self {
            allowed: true,
            energy,
            reason: None,
            witness,
            edge_breakdown: HashMap::new(),
            timestamp: Timestamp::now(),
            request_id,
        }
    }

    /// Create a denying result
    pub fn deny(
        energy: f32,
        reason: impl Into<String>,
        witness: ValidationWitness,
        request_id: Uuid,
    ) -> Self {
        Self {
            allowed: false,
            energy,
            reason: Some(reason.into()),
            witness,
            edge_breakdown: HashMap::new(),
            timestamp: Timestamp::now(),
            request_id,
        }
    }

    /// Add edge breakdown information
    pub fn with_edge_breakdown(mut self, breakdown: HashMap<String, f32>) -> Self {
        self.edge_breakdown = breakdown;
        self
    }

    /// Check if the result indicates a coherent response
    pub fn is_coherent(&self, threshold: f32) -> bool {
        self.energy < threshold
    }
}

// ============================================================================
// VALIDATION WITNESS
// ============================================================================

/// Witness record for validation decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWitness {
    /// Unique witness ID
    pub id: Uuid,

    /// Hash of the validation context
    pub context_hash: Hash,

    /// Hash of the response embedding
    pub response_hash: Hash,

    /// Energy at validation time
    pub energy: f32,

    /// Scope used for policy lookup
    pub scope: String,

    /// Gate decision details
    pub decision: WitnessDecision,

    /// Policy bundle reference
    pub policy_ref: Option<PolicyBundleRef>,

    /// Timestamp
    pub timestamp: Timestamp,

    /// Fingerprint for integrity verification
    pub fingerprint: Hash,
}

impl ValidationWitness {
    /// Create a new validation witness
    pub fn new(
        context: &ValidationContext,
        energy: f32,
        decision: WitnessDecision,
        policy_ref: Option<PolicyBundleRef>,
    ) -> Self {
        let context_hash = Self::compute_embedding_hash(&context.context_embedding);
        let response_hash = Self::compute_embedding_hash(&context.response_embedding);

        let mut witness = Self {
            id: Uuid::new_v4(),
            context_hash,
            response_hash,
            energy,
            scope: context.scope.clone(),
            decision,
            policy_ref,
            timestamp: Timestamp::now(),
            fingerprint: Hash::zero(),
        };

        witness.fingerprint = witness.compute_fingerprint();
        witness
    }

    /// Compute hash of an embedding vector
    fn compute_embedding_hash(embedding: &[f32]) -> Hash {
        let mut hasher = blake3::Hasher::new();
        for &val in embedding {
            hasher.update(&val.to_le_bytes());
        }
        Hash::from_blake3(hasher.finalize())
    }

    /// Compute the fingerprint for integrity verification
    fn compute_fingerprint(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.id.as_bytes());
        hasher.update(self.context_hash.as_bytes());
        hasher.update(self.response_hash.as_bytes());
        hasher.update(&self.energy.to_le_bytes());
        hasher.update(self.scope.as_bytes());
        hasher.update(&[self.decision.allowed as u8]);
        hasher.update(&self.timestamp.secs.to_le_bytes());
        hasher.update(&self.timestamp.nanos.to_le_bytes());
        Hash::from_blake3(hasher.finalize())
    }

    /// Verify the witness integrity
    pub fn verify_integrity(&self) -> bool {
        self.fingerprint == self.compute_fingerprint()
    }
}

/// Decision details within a witness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessDecision {
    /// Whether allowed
    pub allowed: bool,

    /// Compute lane assigned
    pub lane: u8,

    /// Reason if denied
    pub reason: Option<String>,

    /// Confidence score
    pub confidence: f32,
}

impl WitnessDecision {
    /// Create an allow decision
    pub fn allow(lane: u8, confidence: f32) -> Self {
        Self {
            allowed: true,
            lane,
            reason: None,
            confidence,
        }
    }

    /// Create a deny decision
    pub fn deny(lane: u8, reason: impl Into<String>, confidence: f32) -> Self {
        Self {
            allowed: false,
            lane,
            reason: Some(reason.into()),
            confidence,
        }
    }
}

// ============================================================================
// VALIDATION ERROR
// ============================================================================

/// Errors that can occur during validation
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    /// Missing required embedding
    #[error("Missing embedding: {0}")]
    MissingEmbedding(String),

    /// Dimension mismatch between embeddings
    #[error("Dimension mismatch: context={context_dim}, response={response_dim}")]
    DimensionMismatch {
        context_dim: usize,
        response_dim: usize,
    },

    /// Coherence computation failed
    #[error("Coherence computation failed: {0}")]
    CoherenceError(#[from] CoherenceError),

    /// Graph construction failed
    #[error("Graph construction failed: {0}")]
    GraphError(String),

    /// Policy not found
    #[error("Policy not found for scope: {0}")]
    PolicyNotFound(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

// ============================================================================
// VALIDATION ACTION (for gate integration)
// ============================================================================

/// Action implementation for validation requests
struct ValidationAction {
    scope: ExecScopeId,
    impact: ActionImpact,
    metadata: ActionMetadata,
    content_hash: [u8; 32],
}

impl ValidationAction {
    fn new(context: &ValidationContext) -> Self {
        // Compute content hash from context
        let mut hasher = blake3::Hasher::new();
        for &val in &context.context_embedding {
            hasher.update(&val.to_le_bytes());
        }
        for &val in &context.response_embedding {
            hasher.update(&val.to_le_bytes());
        }
        let hash = hasher.finalize();
        let mut content_hash = [0u8; 32];
        content_hash.copy_from_slice(hash.as_bytes());

        Self {
            scope: ExecScopeId::new(&context.scope),
            impact: ActionImpact::medium(),
            metadata: ActionMetadata::new(
                "LLMValidation",
                "Coherence validation for LLM response",
                &context.request_id.to_string(),
            ),
            content_hash,
        }
    }
}

impl Action for ValidationAction {
    type Output = ();
    type Error = ValidationError;

    fn scope(&self) -> &ExecScopeId {
        &self.scope
    }

    fn impact(&self) -> ActionImpact {
        self.impact
    }

    fn metadata(&self) -> &ActionMetadata {
        &self.metadata
    }

    fn execute(&self, _ctx: &ExecutionContext) -> Result<(), ValidationError> {
        // Validation action doesn't execute anything - it's just for gating
        Ok(())
    }

    fn content_hash(&self) -> [u8; 32] {
        self.content_hash
    }

    fn make_rollback_not_supported_error() -> ValidationError {
        ValidationError::Internal("Rollback not supported for validation".to_string())
    }
}

// ============================================================================
// SHEAF COHERENCE VALIDATOR
// ============================================================================

/// Configuration for the sheaf coherence validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    /// Default embedding dimension
    pub default_dim: usize,

    /// Energy threshold for automatic approval (reflex lane)
    pub reflex_threshold: f32,

    /// Energy threshold for retrieval lane
    pub retrieval_threshold: f32,

    /// Energy threshold for heavy lane
    pub heavy_threshold: f32,

    /// Whether to include supporting embeddings in the graph
    pub include_supporting: bool,

    /// Whether to create cross-support edges
    pub create_cross_support_edges: bool,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            default_dim: 384, // Common embedding dimension
            reflex_threshold: 0.3,
            retrieval_threshold: 0.6,
            heavy_threshold: 0.9,
            include_supporting: true,
            create_cross_support_edges: false,
        }
    }
}

/// Sheaf-based coherence validator for LLM responses
///
/// This validator uses Prime-Radiant's sheaf graph and coherence engine
/// to validate LLM responses against their context.
pub struct SheafCoherenceValidator {
    /// Coherence gate for threshold-based gating
    gate: CoherenceGate,

    /// Validator configuration
    config: ValidatorConfig,

    /// Policy bundle reference (optional)
    policy_ref: Option<PolicyBundleRef>,
}

impl SheafCoherenceValidator {
    /// Create a new validator with the given gate
    pub fn new(gate: CoherenceGate) -> Self {
        Self {
            gate,
            config: ValidatorConfig::default(),
            policy_ref: None,
        }
    }

    /// Create a validator with default configuration and a placeholder policy
    pub fn with_defaults() -> Self {
        let policy = PolicyBundleRef {
            id: crate::governance::PolicyBundleId::new(),
            version: crate::governance::Version::initial(),
            content_hash: Hash::zero(),
        };
        let gate = CoherenceGate::with_defaults(policy.clone().into_execution_ref());
        Self {
            gate,
            config: ValidatorConfig::default(),
            policy_ref: Some(policy),
        }
    }

    /// Set the validator configuration
    pub fn with_config(mut self, config: ValidatorConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the policy bundle reference
    pub fn with_policy(mut self, policy: PolicyBundleRef) -> Self {
        self.policy_ref = Some(policy);
        self
    }

    /// Validate an LLM response against its context
    ///
    /// This method:
    /// 1. Validates the input context
    /// 2. Builds a sheaf graph from embeddings
    /// 3. Computes coherence energy
    /// 4. Evaluates against the gate
    /// 5. Returns a ValidationResult with witness
    pub fn validate(
        &mut self,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        // Validate the input
        context.validate()?;

        // Build the sheaf graph
        let graph = self.build_graph(context)?;

        // Compute coherence energy
        let energy = graph.compute_energy();

        // Create energy snapshot for the gate
        let energy_snapshot = EnergySnapshot::new(
            energy.total_energy,
            energy.scope_energy(&context.scope),
            ExecScopeId::new(&context.scope),
        );

        // Create action for gate evaluation
        let action = ValidationAction::new(context);

        // Evaluate with the gate
        let (decision, _exec_witness) = self.gate.evaluate_with_witness(&action, &energy_snapshot);

        // Determine confidence based on energy
        let confidence = self.compute_confidence(energy.total_energy);

        // Create witness decision
        let witness_decision = if decision.allow {
            WitnessDecision::allow(decision.lane.as_u8(), confidence)
        } else {
            WitnessDecision::deny(
                decision.lane.as_u8(),
                decision
                    .reason
                    .clone()
                    .unwrap_or_else(|| "Energy too high".to_string()),
                confidence,
            )
        };

        // Create validation witness
        let witness = ValidationWitness::new(
            context,
            energy.total_energy,
            witness_decision,
            self.policy_ref.clone(),
        );

        // Build edge breakdown
        let edge_breakdown = self.build_edge_breakdown(&graph, &energy);

        // Create result
        let result = if decision.allow {
            ValidationResult::allow(energy.total_energy, witness, context.request_id)
        } else {
            ValidationResult::deny(
                energy.total_energy,
                decision
                    .reason
                    .unwrap_or_else(|| "Coherence threshold exceeded".to_string()),
                witness,
                context.request_id,
            )
        };

        Ok(result.with_edge_breakdown(edge_breakdown))
    }

    /// Build a sheaf graph from the validation context
    fn build_graph(&self, context: &ValidationContext) -> Result<SheafGraph, ValidationError> {
        let graph = SheafGraph::new();
        let dim = context.embedding_dim();

        // Create context node
        let context_node = SheafNodeBuilder::new()
            .state_from_slice(&context.context_embedding)
            .label("context")
            .node_type("context")
            .namespace(&context.scope)
            .build();
        let context_id = graph.add_node(context_node);

        // Create response node
        let response_node = SheafNodeBuilder::new()
            .state_from_slice(&context.response_embedding)
            .label("response")
            .node_type("response")
            .namespace(&context.scope)
            .build();
        let response_id = graph.add_node(response_node);

        // Create context-response edge with identity restriction
        // This enforces that response should be semantically consistent with context
        let ctx_resp_edge = SheafEdgeBuilder::new(context_id, response_id)
            .identity_restrictions(dim)
            .weight(context.edge_weights.context_response)
            .edge_type("context_response")
            .namespace(&context.scope)
            .build();
        graph
            .add_edge(ctx_resp_edge)
            .map_err(|e| ValidationError::GraphError(e.to_string()))?;

        // Add supporting nodes and edges if configured
        if self.config.include_supporting {
            let mut support_ids = Vec::new();

            for (i, emb) in context.supporting_embeddings.iter().enumerate() {
                let support_node = SheafNodeBuilder::new()
                    .state_from_slice(emb)
                    .label(format!("support_{}", i))
                    .node_type("supporting")
                    .namespace(&context.scope)
                    .build();
                let support_id = graph.add_node(support_node);
                support_ids.push(support_id);

                // Edge from context to supporting
                let ctx_sup_edge = SheafEdgeBuilder::new(context_id, support_id)
                    .identity_restrictions(dim)
                    .weight(context.edge_weights.context_support)
                    .edge_type("context_support")
                    .namespace(&context.scope)
                    .build();
                graph
                    .add_edge(ctx_sup_edge)
                    .map_err(|e| ValidationError::GraphError(e.to_string()))?;

                // Edge from response to supporting
                let resp_sup_edge = SheafEdgeBuilder::new(response_id, support_id)
                    .identity_restrictions(dim)
                    .weight(context.edge_weights.response_support)
                    .edge_type("response_support")
                    .namespace(&context.scope)
                    .build();
                graph
                    .add_edge(resp_sup_edge)
                    .map_err(|e| ValidationError::GraphError(e.to_string()))?;
            }

            // Create cross-support edges if configured
            if self.config.create_cross_support_edges && support_ids.len() > 1 {
                for i in 0..support_ids.len() {
                    for j in (i + 1)..support_ids.len() {
                        let cross_edge = SheafEdgeBuilder::new(support_ids[i], support_ids[j])
                            .identity_restrictions(dim)
                            .weight(context.edge_weights.support_support)
                            .edge_type("support_support")
                            .namespace(&context.scope)
                            .build();
                        graph
                            .add_edge(cross_edge)
                            .map_err(|e| ValidationError::GraphError(e.to_string()))?;
                    }
                }
            }
        }

        Ok(graph)
    }

    /// Build a breakdown of energy by edge type
    fn build_edge_breakdown(
        &self,
        graph: &SheafGraph,
        energy: &crate::substrate::graph::CoherenceEnergy,
    ) -> HashMap<String, f32> {
        let mut breakdown: HashMap<String, f32> = HashMap::new();

        for edge_id in graph.edge_ids() {
            if let Some(edge) = graph.get_edge(edge_id) {
                let edge_type = edge.edge_type.as_deref().unwrap_or("unknown");
                if let Some(&edge_energy) = energy.edge_energies.get(&edge_id) {
                    *breakdown.entry(edge_type.to_string()).or_insert(0.0) += edge_energy;
                }
            }
        }

        breakdown
    }

    /// Compute confidence score based on energy
    fn compute_confidence(&self, energy: f32) -> f32 {
        // Higher energy = lower confidence
        // Map energy to [0, 1] confidence using sigmoid-like function
        let normalized = energy / self.config.heavy_threshold;
        1.0 / (1.0 + normalized.exp())
    }

    /// Get the current configuration
    pub fn config(&self) -> &ValidatorConfig {
        &self.config
    }

    /// Get a reference to the gate
    pub fn gate(&self) -> &CoherenceGate {
        &self.gate
    }

    /// Get a mutable reference to the gate
    pub fn gate_mut(&mut self) -> &mut CoherenceGate {
        &mut self.gate
    }

    /// Update the policy bundle reference
    pub fn update_policy(&mut self, policy: PolicyBundleRef) {
        self.policy_ref = Some(policy.clone());
        self.gate.update_policy_bundle(policy.into_execution_ref());
    }
}

// ============================================================================
// POLICY BUNDLE REF CONVERSION
// ============================================================================

impl PolicyBundleRef {
    /// Convert to execution layer's policy bundle ref
    fn into_execution_ref(self) -> crate::execution::PolicyBundleRef {
        crate::execution::PolicyBundleRef {
            id: self.id.0,
            version: format!(
                "{}.{}.{}",
                self.version.major, self.version.minor, self.version.patch
            ),
            content_hash: *self.content_hash.as_bytes(),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_embedding(dim: usize, base_value: f32) -> Vec<f32> {
        (0..dim).map(|i| base_value + (i as f32) * 0.01).collect()
    }

    #[test]
    fn test_validation_context_creation() {
        let ctx = ValidationContext::new()
            .with_context_embedding(vec![1.0, 2.0, 3.0])
            .with_response_embedding(vec![1.0, 2.0, 3.0])
            .with_scope("test")
            .with_metadata("test_key", "test_value");

        assert_eq!(ctx.embedding_dim(), 3);
        assert!(ctx.validate().is_ok());
    }

    #[test]
    fn test_validation_context_dimension_mismatch() {
        let ctx = ValidationContext::new()
            .with_context_embedding(vec![1.0, 2.0, 3.0])
            .with_response_embedding(vec![1.0, 2.0]);

        let result = ctx.validate();
        assert!(matches!(
            result,
            Err(ValidationError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_edge_weights() {
        let strict = EdgeWeights::strict();
        assert!(strict.context_response > 1.0);

        let permissive = EdgeWeights::permissive();
        assert!(permissive.context_response <= 1.0);
    }

    #[test]
    fn test_validation_witness_integrity() {
        let ctx = ValidationContext::new()
            .with_context_embedding(vec![1.0, 2.0, 3.0])
            .with_response_embedding(vec![1.0, 2.0, 3.0]);

        let witness = ValidationWitness::new(&ctx, 0.5, WitnessDecision::allow(0, 0.9), None);

        assert!(witness.verify_integrity());
    }

    #[test]
    fn test_validator_coherent_response() {
        let mut validator = SheafCoherenceValidator::with_defaults();

        // Similar embeddings should be coherent
        let ctx = ValidationContext::new()
            .with_context_embedding(create_test_embedding(64, 1.0))
            .with_response_embedding(create_test_embedding(64, 1.0));

        let result = validator.validate(&ctx).unwrap();
        assert!(result.allowed);
        assert!(result.energy < 0.01); // Very low energy for identical embeddings
    }

    #[test]
    fn test_validator_incoherent_response() {
        let mut validator = SheafCoherenceValidator::with_defaults().with_config(ValidatorConfig {
            reflex_threshold: 0.01, // Very strict
            ..Default::default()
        });

        // Very different embeddings should be incoherent
        let ctx = ValidationContext::new()
            .with_context_embedding(create_test_embedding(64, 1.0))
            .with_response_embedding(create_test_embedding(64, 100.0));

        let result = validator.validate(&ctx).unwrap();
        // With such different embeddings, energy should be high
        assert!(result.energy > 0.0);
    }

    #[test]
    fn test_validator_with_supporting() {
        let mut validator = SheafCoherenceValidator::with_defaults();

        let ctx = ValidationContext::new()
            .with_context_embedding(create_test_embedding(64, 1.0))
            .with_response_embedding(create_test_embedding(64, 1.0))
            .with_supporting_embedding(create_test_embedding(64, 1.0))
            .with_supporting_embedding(create_test_embedding(64, 1.0));

        let result = validator.validate(&ctx).unwrap();
        assert!(result.allowed);
        // Should have breakdown for multiple edge types
        assert!(!result.edge_breakdown.is_empty());
    }

    #[test]
    fn test_validation_result_serialization() {
        let ctx = ValidationContext::new()
            .with_context_embedding(vec![1.0, 2.0, 3.0])
            .with_response_embedding(vec![1.0, 2.0, 3.0]);

        let witness = ValidationWitness::new(&ctx, 0.1, WitnessDecision::allow(0, 0.95), None);

        let result = ValidationResult::allow(0.1, witness, ctx.request_id);

        // Should be serializable
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ValidationResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.energy, deserialized.energy);
        assert_eq!(result.allowed, deserialized.allowed);
    }
}
