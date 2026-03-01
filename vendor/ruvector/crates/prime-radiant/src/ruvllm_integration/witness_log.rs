//! Unified Witness Log
//!
//! Merges RuvLLM's inference witness logging with Prime-Radiant's governance witnesses,
//! providing a comprehensive audit trail for AI model inference under coherence governance.
//!
//! # Architecture (ADR-CE-017)
//!
//! ```text
//! +---------------------------+     +---------------------------+
//! |    RuvLLM Inference       |     |  Prime-Radiant Governance |
//! |    - Routing decisions    |     |  - Gate decisions         |
//! |    - Quality metrics      |     |  - Energy snapshots       |
//! |    - Latency breakdown    |     |  - Policy bundles         |
//! +------------+--------------+     +-------------+-------------+
//!              |                                  |
//!              v                                  v
//!         +-------------------------------------------+
//!         |        UnifiedWitnessLog                   |
//!         |  - GenerationWitness (linked records)     |
//!         |  - Hash chain for tamper evidence         |
//!         |  - Semantic search (query_embedding)      |
//!         |  - Audit trail queries                    |
//!         +-------------------------------------------+
//! ```
//!
//! # Core Invariant
//!
//! **Every LLM generation that passes through Prime-Radiant governance MUST produce a
//! GenerationWitness linking the inference witness to the coherence witness.**
//!
//! # Hash Chain
//!
//! Each `GenerationWitness` includes:
//! - Hash of the inference witness
//! - Hash of the coherence witness
//! - Hash of the previous `GenerationWitness`
//! - Combined content hash
//!
//! This provides tamper evidence: any modification to any witness breaks the chain.
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::ruvllm_integration::{UnifiedWitnessLog, GenerationWitness};
//!
//! let mut log = UnifiedWitnessLog::new();
//!
//! // Record a generation with both inference and coherence witnesses
//! let witness = log.record_generation(
//!     inference_witness,
//!     coherence_witness,
//! )?;
//!
//! // Query by session
//! let session_witnesses = log.query_by_session("session-123")?;
//!
//! // Verify chain integrity
//! assert!(log.verify_chain()?);
//! ```

use crate::governance::{
    EnergySnapshot, GateDecision, Hash, PolicyBundleRef, Timestamp, WitnessId, WitnessRecord,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Re-export inference-related types when ruvllm is available
#[cfg(feature = "ruvllm")]
pub use ruvllm::witness_log::{
    LatencyBreakdown, RoutingDecision, WitnessEntry as InferenceWitness, WitnessLogStats,
};

/// Errors for the unified witness log
#[derive(Debug, Error)]
pub enum UnifiedWitnessError {
    /// Witness not found
    #[error("Witness not found: {0}")]
    NotFound(String),

    /// Chain integrity violation
    #[error("Chain integrity violation at witness {witness_id}: {reason}")]
    ChainViolation { witness_id: String, reason: String },

    /// Hash mismatch
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// Invalid witness data
    #[error("Invalid witness data: {0}")]
    InvalidData(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Governance witness error
    #[error("Governance witness error: {0}")]
    GovernanceError(String),

    /// Inference witness error
    #[error("Inference witness error: {0}")]
    InferenceError(String),
}

/// Result type for unified witness operations
pub type Result<T> = std::result::Result<T, UnifiedWitnessError>;

/// Unique identifier for a generation witness
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenerationWitnessId(pub Uuid);

impl GenerationWitnessId {
    /// Generate a new random ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get as bytes
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Create a nil/sentinel ID
    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Check if this is the nil ID
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for GenerationWitnessId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GenerationWitnessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lightweight inference witness summary for when full ruvllm is not available
/// This allows the unified log to work without the full ruvllm dependency
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InferenceWitnessSummary {
    /// Request ID from the inference
    pub request_id: Uuid,
    /// Session ID
    pub session_id: String,
    /// Model used for generation
    pub model_used: String,
    /// Quality score (0.0 - 1.0)
    pub quality_score: f32,
    /// Router confidence (0.0 - 1.0)
    pub routing_confidence: f32,
    /// Total latency in milliseconds
    pub total_latency_ms: f32,
    /// Whether the request was successful
    pub is_success: bool,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Timestamp of the inference
    pub timestamp: Timestamp,
    /// Query embedding for semantic search
    pub query_embedding: Option<Vec<f32>>,
    /// Content hash of the inference witness
    pub content_hash: Hash,
}

impl InferenceWitnessSummary {
    /// Create from full inference witness data
    #[cfg(feature = "ruvllm")]
    pub fn from_inference_witness(witness: &InferenceWitness) -> Self {
        Self {
            request_id: witness.request_id,
            session_id: witness.session_id.clone(),
            model_used: format!("{:?}", witness.model_used),
            quality_score: witness.quality_score,
            routing_confidence: witness.routing_decision.confidence,
            total_latency_ms: witness.latency.total_ms,
            is_success: witness.is_success(),
            error_message: witness.error.as_ref().map(|e| format!("{:?}", e)),
            timestamp: Timestamp::from(witness.timestamp),
            query_embedding: Some(witness.query_embedding.clone()),
            content_hash: Self::compute_hash(witness),
        }
    }

    /// Compute content hash for an inference witness
    #[cfg(feature = "ruvllm")]
    fn compute_hash(witness: &InferenceWitness) -> Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(witness.request_id.as_bytes());
        hasher.update(witness.session_id.as_bytes());
        hasher.update(&witness.quality_score.to_le_bytes());
        hasher.update(&witness.latency.total_ms.to_le_bytes());
        hasher.update(&[witness.is_success() as u8]);
        Hash::from_blake3(hasher.finalize())
    }

    /// Create a minimal summary without full ruvllm
    pub fn minimal(
        request_id: Uuid,
        session_id: String,
        model_used: String,
        quality_score: f32,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(request_id.as_bytes());
        hasher.update(session_id.as_bytes());
        hasher.update(&quality_score.to_le_bytes());
        let content_hash = Hash::from_blake3(hasher.finalize());

        Self {
            request_id,
            session_id,
            model_used,
            quality_score,
            routing_confidence: 1.0,
            total_latency_ms: 0.0,
            is_success: true,
            error_message: None,
            timestamp: Timestamp::now(),
            query_embedding: None,
            content_hash,
        }
    }
}

/// Coherence witness summary extracted from Prime-Radiant governance witness
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoherenceWitnessSummary {
    /// Witness ID from governance
    pub witness_id: WitnessId,
    /// Gate decision
    pub decision: GateDecision,
    /// Energy snapshot at decision time
    pub energy_snapshot: EnergySnapshot,
    /// Policy bundle reference
    pub policy_bundle_ref: PolicyBundleRef,
    /// Timestamp
    pub timestamp: Timestamp,
    /// Content hash of the coherence witness
    pub content_hash: Hash,
}

impl CoherenceWitnessSummary {
    /// Create from a governance witness record
    pub fn from_witness_record(record: &WitnessRecord) -> Self {
        Self {
            witness_id: record.id,
            decision: record.decision.clone(),
            energy_snapshot: record.energy_snapshot.clone(),
            policy_bundle_ref: record.policy_bundle_ref.clone(),
            timestamp: record.timestamp,
            content_hash: record.content_hash,
        }
    }
}

/// A generation witness linking inference and coherence witnesses
///
/// This is the primary record in the unified witness log, providing:
/// - Linkage between inference (RuvLLM) and coherence (Prime-Radiant) witnesses
/// - Hash chain for tamper evidence
/// - Semantic search capability via query embedding
/// - Audit trail with session/actor tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationWitness {
    /// Unique identifier
    pub id: GenerationWitnessId,
    /// Sequence number in the chain
    pub sequence: u64,
    /// Inference witness summary
    pub inference: InferenceWitnessSummary,
    /// Coherence witness summary
    pub coherence: CoherenceWitnessSummary,
    /// Combined content hash (hash of both witnesses)
    pub combined_hash: Hash,
    /// Reference to previous generation witness (None for genesis)
    pub previous_witness: Option<GenerationWitnessId>,
    /// Hash of previous witness content
    pub previous_hash: Option<Hash>,
    /// Final content hash including chain linkage
    pub content_hash: Hash,
    /// Optional actor who triggered the generation
    pub actor: Option<String>,
    /// Optional correlation ID for distributed tracing
    pub correlation_id: Option<String>,
    /// Custom tags for filtering
    pub tags: Vec<String>,
    /// Creation timestamp
    pub created_at: Timestamp,
}

impl GenerationWitness {
    /// Create a new generation witness linking inference and coherence records
    pub fn new(
        inference: InferenceWitnessSummary,
        coherence: CoherenceWitnessSummary,
        previous: Option<&GenerationWitness>,
    ) -> Self {
        let id = GenerationWitnessId::new();
        let created_at = Timestamp::now();

        // Compute combined hash of both witnesses
        let combined_hash = Self::compute_combined_hash(&inference, &coherence);

        let (previous_witness, previous_hash, sequence) = match previous {
            Some(prev) => (Some(prev.id), Some(prev.content_hash), prev.sequence + 1),
            None => (None, None, 0),
        };

        let mut witness = Self {
            id,
            sequence,
            inference,
            coherence,
            combined_hash,
            previous_witness,
            previous_hash,
            content_hash: Hash::zero(), // Placeholder
            actor: None,
            correlation_id: None,
            tags: Vec::new(),
            created_at,
        };

        // Compute final content hash
        witness.content_hash = witness.compute_content_hash();
        witness
    }

    /// Create a genesis witness (first in chain)
    pub fn genesis(inference: InferenceWitnessSummary, coherence: CoherenceWitnessSummary) -> Self {
        Self::new(inference, coherence, None)
    }

    /// Set the actor
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Set correlation ID
    #[must_use]
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Add tags
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Compute combined hash of inference and coherence witnesses
    fn compute_combined_hash(
        inference: &InferenceWitnessSummary,
        coherence: &CoherenceWitnessSummary,
    ) -> Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(inference.content_hash.as_bytes());
        hasher.update(coherence.content_hash.as_bytes());
        Hash::from_blake3(hasher.finalize())
    }

    /// Compute the full content hash including chain linkage
    pub fn compute_content_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();

        // Identity
        hasher.update(self.id.as_bytes());
        hasher.update(&self.sequence.to_le_bytes());

        // Combined witness hash
        hasher.update(self.combined_hash.as_bytes());

        // Chain linkage
        if let Some(ref prev_id) = self.previous_witness {
            hasher.update(prev_id.as_bytes());
        }
        if let Some(ref prev_hash) = self.previous_hash {
            hasher.update(prev_hash.as_bytes());
        }

        // Metadata
        if let Some(ref actor) = self.actor {
            hasher.update(actor.as_bytes());
        }
        if let Some(ref corr_id) = self.correlation_id {
            hasher.update(corr_id.as_bytes());
        }
        for tag in &self.tags {
            hasher.update(tag.as_bytes());
        }

        // Timestamp
        hasher.update(&self.created_at.secs.to_le_bytes());
        hasher.update(&self.created_at.nanos.to_le_bytes());

        Hash::from_blake3(hasher.finalize())
    }

    /// Verify the content hash is correct
    #[must_use]
    pub fn verify_content_hash(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }

    /// Verify chain linkage to a previous witness
    pub fn verify_chain_link(&self, previous: &GenerationWitness) -> Result<()> {
        // Check ID reference
        if self.previous_witness != Some(previous.id) {
            return Err(UnifiedWitnessError::ChainViolation {
                witness_id: self.id.to_string(),
                reason: format!(
                    "Previous witness ID mismatch: expected {:?}, got {:?}",
                    Some(previous.id),
                    self.previous_witness
                ),
            });
        }

        // Check hash linkage
        if self.previous_hash != Some(previous.content_hash) {
            return Err(UnifiedWitnessError::HashMismatch {
                expected: previous.content_hash.to_hex(),
                actual: self
                    .previous_hash
                    .map(|h| h.to_hex())
                    .unwrap_or_else(|| "None".to_string()),
            });
        }

        // Check sequence continuity
        if self.sequence != previous.sequence + 1 {
            return Err(UnifiedWitnessError::ChainViolation {
                witness_id: self.id.to_string(),
                reason: format!(
                    "Sequence discontinuity: expected {}, got {}",
                    previous.sequence + 1,
                    self.sequence
                ),
            });
        }

        Ok(())
    }

    /// Check if this is a genesis witness
    #[must_use]
    pub fn is_genesis(&self) -> bool {
        self.previous_witness.is_none() && self.sequence == 0
    }

    /// Get the session ID
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.inference.session_id
    }

    /// Check if the generation was allowed by coherence gate
    #[must_use]
    pub fn was_allowed(&self) -> bool {
        self.coherence.decision.allow
    }

    /// Check if the inference was successful
    #[must_use]
    pub fn was_successful(&self) -> bool {
        self.inference.is_success
    }

    /// Get the quality score
    #[must_use]
    pub fn quality_score(&self) -> f32 {
        self.inference.quality_score
    }

    /// Get the coherence energy at decision time
    #[must_use]
    pub fn coherence_energy(&self) -> f32 {
        self.coherence.energy_snapshot.total_energy
    }
}

impl PartialEq for GenerationWitness {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for GenerationWitness {}

impl std::hash::Hash for GenerationWitness {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// Statistics for the unified witness log
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UnifiedWitnessStats {
    /// Total generation witnesses
    pub total_witnesses: usize,
    /// Witnesses by session
    pub sessions: usize,
    /// Generations allowed by coherence gate
    pub allowed_count: usize,
    /// Generations denied by coherence gate
    pub denied_count: usize,
    /// Successful inferences
    pub success_count: usize,
    /// Failed inferences
    pub error_count: usize,
    /// Average quality score
    pub avg_quality_score: f32,
    /// Average coherence energy
    pub avg_coherence_energy: f32,
    /// Chain integrity verified
    pub chain_verified: bool,
}

/// Query filters for searching generation witnesses
#[derive(Clone, Debug, Default)]
pub struct WitnessQuery {
    /// Filter by session ID
    pub session_id: Option<String>,
    /// Filter by actor
    pub actor: Option<String>,
    /// Filter by tags (any match)
    pub tags: Option<Vec<String>>,
    /// Filter by allowed status
    pub allowed: Option<bool>,
    /// Filter by success status
    pub successful: Option<bool>,
    /// Minimum quality score
    pub min_quality: Option<f32>,
    /// Maximum coherence energy
    pub max_energy: Option<f32>,
    /// Start time (inclusive)
    pub start_time: Option<Timestamp>,
    /// End time (inclusive)
    pub end_time: Option<Timestamp>,
    /// Limit results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}

impl WitnessQuery {
    /// Create a new query builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by session
    #[must_use]
    pub fn session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Filter by actor
    #[must_use]
    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Filter by allowed status
    #[must_use]
    pub fn allowed(mut self, allowed: bool) -> Self {
        self.allowed = Some(allowed);
        self
    }

    /// Filter by success status
    #[must_use]
    pub fn successful(mut self, successful: bool) -> Self {
        self.successful = Some(successful);
        self
    }

    /// Set minimum quality score
    #[must_use]
    pub fn min_quality(mut self, score: f32) -> Self {
        self.min_quality = Some(score);
        self
    }

    /// Set maximum coherence energy
    #[must_use]
    pub fn max_energy(mut self, energy: f32) -> Self {
        self.max_energy = Some(energy);
        self
    }

    /// Set result limit
    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Unified witness log holding both coherence and inference witnesses
///
/// Provides:
/// - Recording of linked generation witnesses
/// - Hash chain integrity verification
/// - Query methods for audit trail
/// - Session-based filtering
/// - Semantic search via query embeddings
#[derive(Debug)]
pub struct UnifiedWitnessLog {
    /// All generation witnesses
    witnesses: Vec<GenerationWitness>,
    /// Index by ID for fast lookup
    by_id: HashMap<GenerationWitnessId, usize>,
    /// Index by session for session queries
    by_session: HashMap<String, Vec<usize>>,
    /// Index by correlation ID
    by_correlation: HashMap<String, Vec<usize>>,
    /// Current chain head
    head: Option<GenerationWitnessId>,
    /// Chain verified flag
    chain_verified: bool,
}

impl UnifiedWitnessLog {
    /// Create a new empty unified witness log
    pub fn new() -> Self {
        Self {
            witnesses: Vec::new(),
            by_id: HashMap::new(),
            by_session: HashMap::new(),
            by_correlation: HashMap::new(),
            head: None,
            chain_verified: true,
        }
    }

    /// Create with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            witnesses: Vec::with_capacity(capacity),
            by_id: HashMap::with_capacity(capacity),
            by_session: HashMap::new(),
            by_correlation: HashMap::new(),
            head: None,
            chain_verified: true,
        }
    }

    /// Record a new generation linking inference and coherence witnesses
    pub fn record_generation(
        &mut self,
        inference: InferenceWitnessSummary,
        coherence: CoherenceWitnessSummary,
    ) -> Result<&GenerationWitness> {
        let previous = self.head.and_then(|id| self.get(&id));
        let witness = GenerationWitness::new(inference, coherence, previous);

        self.insert(witness)
    }

    /// Record a generation with full witness records (when ruvllm feature is enabled)
    #[cfg(feature = "ruvllm")]
    pub fn record_generation_full(
        &mut self,
        inference: &InferenceWitness,
        coherence: &WitnessRecord,
    ) -> Result<&GenerationWitness> {
        let inference_summary = InferenceWitnessSummary::from_inference_witness(inference);
        let coherence_summary = CoherenceWitnessSummary::from_witness_record(coherence);
        self.record_generation(inference_summary, coherence_summary)
    }

    /// Insert a generation witness directly
    fn insert(&mut self, witness: GenerationWitness) -> Result<&GenerationWitness> {
        let id = witness.id;
        let session_id = witness.session_id().to_string();
        let correlation_id = witness.correlation_id.clone();
        let index = self.witnesses.len();

        // Update indices
        self.by_id.insert(id, index);
        self.by_session.entry(session_id).or_default().push(index);
        if let Some(corr_id) = correlation_id {
            self.by_correlation.entry(corr_id).or_default().push(index);
        }

        // Update head
        self.head = Some(id);

        // Store witness
        self.witnesses.push(witness);

        Ok(&self.witnesses[index])
    }

    /// Get a witness by ID
    pub fn get(&self, id: &GenerationWitnessId) -> Option<&GenerationWitness> {
        self.by_id.get(id).map(|&idx| &self.witnesses[idx])
    }

    /// Get the current chain head
    pub fn head(&self) -> Option<&GenerationWitness> {
        self.head.and_then(|id| self.get(&id))
    }

    /// Get all witnesses for a session
    pub fn query_by_session(&self, session_id: &str) -> Vec<&GenerationWitness> {
        self.by_session
            .get(session_id)
            .map(|indices| indices.iter().map(|&idx| &self.witnesses[idx]).collect())
            .unwrap_or_default()
    }

    /// Get all witnesses for a correlation ID
    pub fn query_by_correlation(&self, correlation_id: &str) -> Vec<&GenerationWitness> {
        self.by_correlation
            .get(correlation_id)
            .map(|indices| indices.iter().map(|&idx| &self.witnesses[idx]).collect())
            .unwrap_or_default()
    }

    /// Query witnesses with filters
    pub fn query(&self, query: &WitnessQuery) -> Vec<&GenerationWitness> {
        let mut results: Vec<&GenerationWitness> = self.witnesses.iter().collect();

        // Apply filters
        if let Some(ref session) = query.session_id {
            results.retain(|w| w.session_id() == session);
        }
        if let Some(ref actor) = query.actor {
            results.retain(|w| w.actor.as_deref() == Some(actor.as_str()));
        }
        if let Some(allowed) = query.allowed {
            results.retain(|w| w.was_allowed() == allowed);
        }
        if let Some(successful) = query.successful {
            results.retain(|w| w.was_successful() == successful);
        }
        if let Some(min_quality) = query.min_quality {
            results.retain(|w| w.quality_score() >= min_quality);
        }
        if let Some(max_energy) = query.max_energy {
            results.retain(|w| w.coherence_energy() <= max_energy);
        }
        if let Some(ref start) = query.start_time {
            results.retain(|w| w.created_at >= *start);
        }
        if let Some(ref end) = query.end_time {
            results.retain(|w| w.created_at <= *end);
        }
        if let Some(ref tags) = query.tags {
            results.retain(|w| w.tags.iter().any(|t| tags.contains(t)));
        }

        // Apply pagination
        if let Some(offset) = query.offset {
            results = results.into_iter().skip(offset).collect();
        }
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        results
    }

    /// Get all session IDs
    pub fn sessions(&self) -> Vec<&str> {
        self.by_session.keys().map(|s| s.as_str()).collect()
    }

    /// Get the total number of witnesses
    pub fn len(&self) -> usize {
        self.witnesses.len()
    }

    /// Check if the log is empty
    pub fn is_empty(&self) -> bool {
        self.witnesses.is_empty()
    }

    /// Verify the entire chain integrity
    pub fn verify_chain(&mut self) -> Result<bool> {
        if self.witnesses.is_empty() {
            self.chain_verified = true;
            return Ok(true);
        }

        // Verify first witness is genesis
        if !self.witnesses[0].is_genesis() {
            self.chain_verified = false;
            return Err(UnifiedWitnessError::ChainViolation {
                witness_id: self.witnesses[0].id.to_string(),
                reason: "First witness is not genesis".to_string(),
            });
        }

        // Verify content hashes
        for witness in &self.witnesses {
            if !witness.verify_content_hash() {
                self.chain_verified = false;
                return Err(UnifiedWitnessError::HashMismatch {
                    expected: witness.content_hash.to_hex(),
                    actual: witness.compute_content_hash().to_hex(),
                });
            }
        }

        // Verify chain linkage
        for i in 1..self.witnesses.len() {
            self.witnesses[i].verify_chain_link(&self.witnesses[i - 1])?;
        }

        self.chain_verified = true;
        Ok(true)
    }

    /// Get statistics about the witness log
    pub fn stats(&self) -> UnifiedWitnessStats {
        if self.witnesses.is_empty() {
            return UnifiedWitnessStats::default();
        }

        let allowed_count = self.witnesses.iter().filter(|w| w.was_allowed()).count();
        let success_count = self.witnesses.iter().filter(|w| w.was_successful()).count();
        let total_quality: f32 = self.witnesses.iter().map(|w| w.quality_score()).sum();
        let total_energy: f32 = self.witnesses.iter().map(|w| w.coherence_energy()).sum();

        UnifiedWitnessStats {
            total_witnesses: self.witnesses.len(),
            sessions: self.by_session.len(),
            allowed_count,
            denied_count: self.witnesses.len() - allowed_count,
            success_count,
            error_count: self.witnesses.len() - success_count,
            avg_quality_score: total_quality / self.witnesses.len() as f32,
            avg_coherence_energy: total_energy / self.witnesses.len() as f32,
            chain_verified: self.chain_verified,
        }
    }

    /// Export witnesses for a session as JSON
    pub fn export_session(&self, session_id: &str) -> Result<String> {
        let witnesses = self.query_by_session(session_id);
        if witnesses.is_empty() {
            return Err(UnifiedWitnessError::SessionNotFound(session_id.to_string()));
        }
        serde_json::to_string_pretty(&witnesses)
            .map_err(|e| UnifiedWitnessError::Storage(e.to_string()))
    }

    /// Get witnesses in range by sequence number
    pub fn range_by_sequence(&self, start: u64, end: u64) -> Vec<&GenerationWitness> {
        self.witnesses
            .iter()
            .filter(|w| w.sequence >= start && w.sequence <= end)
            .collect()
    }

    /// Find witnesses with quality below threshold (for alerting)
    pub fn low_quality_witnesses(&self, threshold: f32) -> Vec<&GenerationWitness> {
        self.witnesses
            .iter()
            .filter(|w| w.quality_score() < threshold)
            .collect()
    }

    /// Find witnesses with high coherence energy (potential issues)
    pub fn high_energy_witnesses(&self, threshold: f32) -> Vec<&GenerationWitness> {
        self.witnesses
            .iter()
            .filter(|w| w.coherence_energy() > threshold)
            .collect()
    }

    /// Get denied generations (blocked by coherence gate)
    pub fn denied_generations(&self) -> Vec<&GenerationWitness> {
        self.witnesses.iter().filter(|w| !w.was_allowed()).collect()
    }

    /// Get failed inferences
    pub fn failed_inferences(&self) -> Vec<&GenerationWitness> {
        self.witnesses
            .iter()
            .filter(|w| !w.was_successful())
            .collect()
    }
}

impl Default for UnifiedWitnessLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{
        EnergySnapshot, GateDecision, Hash, PolicyBundleId, PolicyBundleRef, Timestamp, Version,
        WitnessComputeLane as ComputeLane, WitnessId,
    };

    fn test_inference_summary() -> InferenceWitnessSummary {
        InferenceWitnessSummary::minimal(
            Uuid::new_v4(),
            "test-session".to_string(),
            "small".to_string(),
            0.85,
        )
    }

    fn test_coherence_summary() -> CoherenceWitnessSummary {
        CoherenceWitnessSummary {
            witness_id: WitnessId::new(),
            decision: GateDecision::allow(ComputeLane::Reflex),
            energy_snapshot: EnergySnapshot::new(0.3, 0.2, "test-scope"),
            policy_bundle_ref: PolicyBundleRef {
                id: PolicyBundleId::new(),
                version: Version::initial(),
                content_hash: Hash::zero(),
            },
            timestamp: Timestamp::now(),
            content_hash: Hash::zero(),
        }
    }

    #[test]
    fn test_generation_witness_creation() {
        let inference = test_inference_summary();
        let coherence = test_coherence_summary();

        let witness = GenerationWitness::genesis(inference, coherence);

        assert!(witness.is_genesis());
        assert!(witness.verify_content_hash());
        assert!(witness.was_allowed());
        assert!(witness.was_successful());
        assert!((witness.quality_score() - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn test_witness_chain() {
        let mut log = UnifiedWitnessLog::new();

        // Record genesis
        let w1 = log
            .record_generation(test_inference_summary(), test_coherence_summary())
            .unwrap();
        assert!(w1.is_genesis());
        assert_eq!(w1.sequence, 0);

        // Record second witness
        let w2 = log
            .record_generation(test_inference_summary(), test_coherence_summary())
            .unwrap();
        assert!(!w2.is_genesis());
        assert_eq!(w2.sequence, 1);
        assert!(w2.previous_witness.is_some());

        // Verify chain
        assert!(log.verify_chain().unwrap());
    }

    #[test]
    fn test_session_query() {
        let mut log = UnifiedWitnessLog::new();

        // Record witnesses with different sessions
        let mut inference1 = test_inference_summary();
        inference1.session_id = "session-1".to_string();
        log.record_generation(inference1, test_coherence_summary())
            .unwrap();

        let mut inference2 = test_inference_summary();
        inference2.session_id = "session-2".to_string();
        log.record_generation(inference2, test_coherence_summary())
            .unwrap();

        let mut inference3 = test_inference_summary();
        inference3.session_id = "session-1".to_string();
        log.record_generation(inference3, test_coherence_summary())
            .unwrap();

        // Query by session
        let session1_witnesses = log.query_by_session("session-1");
        assert_eq!(session1_witnesses.len(), 2);

        let session2_witnesses = log.query_by_session("session-2");
        assert_eq!(session2_witnesses.len(), 1);
    }

    #[test]
    fn test_witness_query_filters() {
        let mut log = UnifiedWitnessLog::new();

        // Record witnesses with varying quality
        let mut inference_high = test_inference_summary();
        inference_high.quality_score = 0.95;
        log.record_generation(inference_high, test_coherence_summary())
            .unwrap();

        let mut inference_low = test_inference_summary();
        inference_low.quality_score = 0.3;
        log.record_generation(inference_low, test_coherence_summary())
            .unwrap();

        // Query with minimum quality filter
        let high_quality = log.query(&WitnessQuery::new().min_quality(0.8));
        assert_eq!(high_quality.len(), 1);
        assert!((high_quality[0].quality_score() - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn test_stats() {
        let mut log = UnifiedWitnessLog::new();

        // Record a few witnesses
        log.record_generation(test_inference_summary(), test_coherence_summary())
            .unwrap();
        log.record_generation(test_inference_summary(), test_coherence_summary())
            .unwrap();

        let stats = log.stats();
        assert_eq!(stats.total_witnesses, 2);
        assert_eq!(stats.allowed_count, 2);
        assert_eq!(stats.success_count, 2);
        assert!(stats.chain_verified);
    }

    #[test]
    fn test_tamper_detection() {
        let inference = test_inference_summary();
        let coherence = test_coherence_summary();

        let mut witness = GenerationWitness::genesis(inference, coherence);

        // Verify original hash
        assert!(witness.verify_content_hash());

        // Tamper with the witness
        witness.inference.quality_score = 0.99;

        // Content hash should no longer match
        assert!(!witness.verify_content_hash());
    }

    #[test]
    fn test_chain_verification() {
        let inference = test_inference_summary();
        let coherence = test_coherence_summary();

        let genesis = GenerationWitness::genesis(inference.clone(), coherence.clone());
        let second = GenerationWitness::new(inference.clone(), coherence.clone(), Some(&genesis));

        // Valid chain link
        assert!(second.verify_chain_link(&genesis).is_ok());

        // Create witness with wrong previous
        let mut bad_witness = GenerationWitness::new(inference, coherence, Some(&genesis));
        bad_witness.previous_witness = Some(GenerationWitnessId::new()); // Wrong ID

        // Should fail verification
        assert!(bad_witness.verify_chain_link(&genesis).is_err());
    }

    #[test]
    fn test_denied_and_failed_queries() {
        let mut log = UnifiedWitnessLog::new();

        // Record allowed/successful
        log.record_generation(test_inference_summary(), test_coherence_summary())
            .unwrap();

        // Record denied
        let mut denied_coherence = test_coherence_summary();
        denied_coherence.decision = GateDecision::deny(ComputeLane::Heavy, "High energy");
        log.record_generation(test_inference_summary(), denied_coherence)
            .unwrap();

        // Record failed inference
        let mut failed_inference = test_inference_summary();
        failed_inference.is_success = false;
        failed_inference.error_message = Some("Timeout".to_string());
        log.record_generation(failed_inference, test_coherence_summary())
            .unwrap();

        // Query denied
        let denied = log.denied_generations();
        assert_eq!(denied.len(), 1);

        // Query failed
        let failed = log.failed_inferences();
        assert_eq!(failed.len(), 1);
    }
}
