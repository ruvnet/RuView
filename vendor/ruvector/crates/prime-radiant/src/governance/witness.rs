//! Witness Record Entity
//!
//! Implements immutable proof of every gate decision with content hashing.
//!
//! # Witness Chain
//!
//! Each witness record references its predecessor, forming a linked chain:
//!
//! ```text
//! Witness N-2 <-- Witness N-1 <-- Witness N
//!     ^               ^               ^
//!     |               |               |
//!  hash(N-2)      hash(N-1)       hash(N)
//! ```
//!
//! This provides:
//! - Temporal ordering guarantee
//! - Tamper detection (any modification breaks the chain)
//! - Deterministic replay capability
//!
//! # Core Invariant
//!
//! **No action without witness**: Every gate decision MUST produce a witness record.

use super::{Hash, PolicyBundleRef, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Unique identifier for a witness record
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WitnessId(pub Uuid);

impl WitnessId {
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

impl Default for WitnessId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WitnessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Compute lane levels (from ADR-014)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ComputeLane {
    /// Lane 0: Local residual updates, simple aggregates (<1ms)
    Reflex = 0,
    /// Lane 1: Evidence fetching, lightweight reasoning (~10ms)
    Retrieval = 1,
    /// Lane 2: Multi-step planning, spectral analysis (~100ms)
    Heavy = 2,
    /// Lane 3: Human escalation for sustained incoherence
    Human = 3,
}

impl ComputeLane {
    /// Get the numeric value
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Create from numeric value
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Reflex),
            1 => Some(Self::Retrieval),
            2 => Some(Self::Heavy),
            3 => Some(Self::Human),
            _ => None,
        }
    }

    /// Check if this lane requires human intervention
    #[must_use]
    pub const fn requires_human(&self) -> bool {
        matches!(self, Self::Human)
    }
}

impl std::fmt::Display for ComputeLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reflex => write!(f, "Reflex"),
            Self::Retrieval => write!(f, "Retrieval"),
            Self::Heavy => write!(f, "Heavy"),
            Self::Human => write!(f, "Human"),
        }
    }
}

/// Gate decision result
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GateDecision {
    /// Whether the action was allowed
    pub allow: bool,
    /// Required compute lane
    pub lane: ComputeLane,
    /// Reason for the decision (especially if denied)
    pub reason: Option<String>,
    /// Confidence in the decision (0.0 to 1.0)
    pub confidence: f32,
    /// Additional decision metadata
    pub metadata: HashMap<String, String>,
}

impl GateDecision {
    /// Create an allow decision
    #[must_use]
    pub fn allow(lane: ComputeLane) -> Self {
        Self {
            allow: true,
            lane,
            reason: None,
            confidence: 1.0,
            metadata: HashMap::new(),
        }
    }

    /// Create a deny decision
    #[must_use]
    pub fn deny(lane: ComputeLane, reason: impl Into<String>) -> Self {
        Self {
            allow: false,
            lane,
            reason: Some(reason.into()),
            confidence: 1.0,
            metadata: HashMap::new(),
        }
    }

    /// Set confidence level
    #[must_use]
    pub const fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Snapshot of coherence energy at decision time
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnergySnapshot {
    /// Total system energy (lower = more coherent)
    pub total_energy: f32,
    /// Energy for the specific scope being evaluated
    pub scope_energy: f32,
    /// Scope identifier
    pub scope: String,
    /// Number of edges contributing to this energy
    pub edge_count: u32,
    /// Timestamp when energy was computed
    pub computed_at: Timestamp,
    /// Fingerprint for change detection
    pub fingerprint: Hash,
    /// Per-scope breakdown (optional)
    pub scope_breakdown: Option<HashMap<String, f32>>,
}

impl EnergySnapshot {
    /// Create a new energy snapshot
    #[must_use]
    pub fn new(total_energy: f32, scope_energy: f32, scope: impl Into<String>) -> Self {
        Self {
            total_energy,
            scope_energy,
            scope: scope.into(),
            edge_count: 0,
            computed_at: Timestamp::now(),
            fingerprint: Hash::zero(),
            scope_breakdown: None,
        }
    }

    /// Set edge count
    #[must_use]
    pub const fn with_edge_count(mut self, count: u32) -> Self {
        self.edge_count = count;
        self
    }

    /// Set fingerprint
    #[must_use]
    pub const fn with_fingerprint(mut self, fingerprint: Hash) -> Self {
        self.fingerprint = fingerprint;
        self
    }

    /// Add scope breakdown
    #[must_use]
    pub fn with_breakdown(mut self, breakdown: HashMap<String, f32>) -> Self {
        self.scope_breakdown = Some(breakdown);
        self
    }

    /// Compute content hash for this snapshot
    #[must_use]
    pub fn content_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.total_energy.to_le_bytes());
        hasher.update(&self.scope_energy.to_le_bytes());
        hasher.update(self.scope.as_bytes());
        hasher.update(&self.edge_count.to_le_bytes());
        hasher.update(&self.computed_at.secs.to_le_bytes());
        hasher.update(&self.computed_at.nanos.to_le_bytes());
        hasher.update(self.fingerprint.as_bytes());
        Hash::from_blake3(hasher.finalize())
    }
}

/// Witness chain integrity errors
#[derive(Debug, Error)]
pub enum WitnessChainError {
    /// Previous witness not found
    #[error("Previous witness not found: {0}")]
    PreviousNotFound(WitnessId),

    /// Chain hash mismatch
    #[error("Chain hash mismatch at witness {0}")]
    HashMismatch(WitnessId),

    /// Temporal ordering violation
    #[error("Temporal ordering violation: {0} should be before {1}")]
    TemporalViolation(WitnessId, WitnessId),

    /// Gap in sequence
    #[error("Gap in witness sequence at {0}")]
    SequenceGap(u64),
}

/// Witness-related errors
#[derive(Debug, Error)]
pub enum WitnessError {
    /// Chain integrity error
    #[error("Chain integrity error: {0}")]
    ChainError(#[from] WitnessChainError),

    /// Invalid witness data
    #[error("Invalid witness data: {0}")]
    InvalidData(String),

    /// Witness not found
    #[error("Witness not found: {0}")]
    NotFound(WitnessId),

    /// Witness already exists
    #[error("Witness already exists: {0}")]
    AlreadyExists(WitnessId),
}

/// Immutable proof of a gate decision
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Unique witness identifier
    pub id: WitnessId,
    /// Sequence number within the chain
    pub sequence: u64,
    /// Hash of the action that was evaluated
    pub action_hash: Hash,
    /// Energy state at time of evaluation
    pub energy_snapshot: EnergySnapshot,
    /// Gate decision made
    pub decision: GateDecision,
    /// Policy bundle used for evaluation
    pub policy_bundle_ref: PolicyBundleRef,
    /// Creation timestamp
    pub timestamp: Timestamp,
    /// Reference to previous witness in chain (None for genesis)
    pub previous_witness: Option<WitnessId>,
    /// Hash of previous witness content (for chain integrity)
    pub previous_hash: Option<Hash>,
    /// Content hash of this witness (computed on creation)
    pub content_hash: Hash,
    /// Optional actor who triggered the action
    pub actor: Option<String>,
    /// Optional correlation ID for request tracing
    pub correlation_id: Option<String>,
}

impl WitnessRecord {
    /// Create a new witness record
    ///
    /// # Arguments
    ///
    /// * `action_hash` - Hash of the action being witnessed
    /// * `energy_snapshot` - Energy state at decision time
    /// * `decision` - The gate decision
    /// * `policy_bundle_ref` - Reference to the policy used
    /// * `previous` - Previous witness in chain (None for genesis)
    #[must_use]
    pub fn new(
        action_hash: Hash,
        energy_snapshot: EnergySnapshot,
        decision: GateDecision,
        policy_bundle_ref: PolicyBundleRef,
        previous: Option<&WitnessRecord>,
    ) -> Self {
        let id = WitnessId::new();
        let timestamp = Timestamp::now();

        let (previous_witness, previous_hash, sequence) = match previous {
            Some(prev) => (Some(prev.id), Some(prev.content_hash), prev.sequence + 1),
            None => (None, None, 0),
        };

        let mut witness = Self {
            id,
            sequence,
            action_hash,
            energy_snapshot,
            decision,
            policy_bundle_ref,
            timestamp,
            previous_witness,
            previous_hash,
            content_hash: Hash::zero(), // Placeholder, computed below
            actor: None,
            correlation_id: None,
        };

        // Compute and set content hash
        witness.content_hash = witness.compute_content_hash();
        witness
    }

    /// Create a genesis witness (first in chain)
    #[must_use]
    pub fn genesis(
        action_hash: Hash,
        energy_snapshot: EnergySnapshot,
        decision: GateDecision,
        policy_bundle_ref: PolicyBundleRef,
    ) -> Self {
        Self::new(
            action_hash,
            energy_snapshot,
            decision,
            policy_bundle_ref,
            None,
        )
    }

    /// Set the actor
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        // Recompute hash since we changed content
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Set correlation ID
    #[must_use]
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        // Recompute hash since we changed content
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Compute the content hash using Blake3
    #[must_use]
    pub fn compute_content_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();

        // Core identifying fields
        hasher.update(self.id.as_bytes());
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(self.action_hash.as_bytes());

        // Energy snapshot hash
        hasher.update(self.energy_snapshot.content_hash().as_bytes());

        // Decision
        hasher.update(&[self.decision.allow as u8]);
        hasher.update(&[self.decision.lane.as_u8()]);
        hasher.update(&self.decision.confidence.to_le_bytes());
        if let Some(ref reason) = self.decision.reason {
            hasher.update(reason.as_bytes());
        }

        // Policy reference
        hasher.update(&self.policy_bundle_ref.as_bytes());

        // Timestamp
        hasher.update(&self.timestamp.secs.to_le_bytes());
        hasher.update(&self.timestamp.nanos.to_le_bytes());

        // Chain linkage
        if let Some(ref prev_id) = self.previous_witness {
            hasher.update(prev_id.as_bytes());
        }
        if let Some(ref prev_hash) = self.previous_hash {
            hasher.update(prev_hash.as_bytes());
        }

        // Optional fields
        if let Some(ref actor) = self.actor {
            hasher.update(actor.as_bytes());
        }
        if let Some(ref corr_id) = self.correlation_id {
            hasher.update(corr_id.as_bytes());
        }

        Hash::from_blake3(hasher.finalize())
    }

    /// Verify the content hash is correct
    #[must_use]
    pub fn verify_content_hash(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }

    /// Verify the chain linkage to a previous witness
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Previous witness hash doesn't match
    /// - Sequence numbers are not consecutive
    /// - Timestamp ordering is violated
    pub fn verify_chain_link(&self, previous: &WitnessRecord) -> Result<(), WitnessChainError> {
        // Check ID reference
        if self.previous_witness != Some(previous.id) {
            return Err(WitnessChainError::PreviousNotFound(previous.id));
        }

        // Check hash linkage
        if self.previous_hash != Some(previous.content_hash) {
            return Err(WitnessChainError::HashMismatch(self.id));
        }

        // Check sequence continuity
        if self.sequence != previous.sequence + 1 {
            return Err(WitnessChainError::SequenceGap(self.sequence));
        }

        // Check temporal ordering
        if self.timestamp < previous.timestamp {
            return Err(WitnessChainError::TemporalViolation(previous.id, self.id));
        }

        Ok(())
    }

    /// Check if this is a genesis witness
    #[must_use]
    pub fn is_genesis(&self) -> bool {
        self.previous_witness.is_none() && self.sequence == 0
    }

    /// Get the decision outcome
    #[must_use]
    pub const fn was_allowed(&self) -> bool {
        self.decision.allow
    }

    /// Get the compute lane
    #[must_use]
    pub const fn lane(&self) -> ComputeLane {
        self.decision.lane
    }
}

impl PartialEq for WitnessRecord {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for WitnessRecord {}

impl std::hash::Hash for WitnessRecord {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// Builder for creating witness chains
pub struct WitnessChainBuilder {
    head: Option<WitnessRecord>,
    policy_ref: PolicyBundleRef,
}

impl WitnessChainBuilder {
    /// Create a new chain builder
    #[must_use]
    pub fn new(policy_ref: PolicyBundleRef) -> Self {
        Self {
            head: None,
            policy_ref,
        }
    }

    /// Create a new chain builder starting from an existing witness
    #[must_use]
    pub fn from_head(head: WitnessRecord) -> Self {
        let policy_ref = head.policy_bundle_ref.clone();
        Self {
            head: Some(head),
            policy_ref,
        }
    }

    /// Add a witness to the chain
    pub fn add_witness(
        &mut self,
        action_hash: Hash,
        energy_snapshot: EnergySnapshot,
        decision: GateDecision,
    ) -> &WitnessRecord {
        let witness = WitnessRecord::new(
            action_hash,
            energy_snapshot,
            decision,
            self.policy_ref.clone(),
            self.head.as_ref(),
        );
        self.head = Some(witness);
        self.head.as_ref().unwrap()
    }

    /// Get the current head of the chain
    #[must_use]
    pub fn head(&self) -> Option<&WitnessRecord> {
        self.head.as_ref()
    }

    /// Get the current sequence number
    #[must_use]
    pub fn current_sequence(&self) -> u64 {
        self.head.as_ref().map_or(0, |w| w.sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{PolicyBundleId, Version};

    fn test_policy_ref() -> PolicyBundleRef {
        PolicyBundleRef {
            id: PolicyBundleId::new(),
            version: Version::initial(),
            content_hash: Hash::zero(),
        }
    }

    fn test_energy_snapshot() -> EnergySnapshot {
        EnergySnapshot::new(0.5, 0.3, "test-scope")
    }

    #[test]
    fn test_witness_creation() {
        let action_hash = Hash::from_bytes([1u8; 32]);
        let energy = test_energy_snapshot();
        let decision = GateDecision::allow(ComputeLane::Reflex);
        let policy_ref = test_policy_ref();

        let witness = WitnessRecord::genesis(action_hash, energy, decision, policy_ref);

        assert!(witness.is_genesis());
        assert!(witness.was_allowed());
        assert_eq!(witness.lane(), ComputeLane::Reflex);
        assert_eq!(witness.sequence, 0);
        assert!(witness.verify_content_hash());
    }

    #[test]
    fn test_witness_chain() {
        let policy_ref = test_policy_ref();
        let mut builder = WitnessChainBuilder::new(policy_ref);

        // Genesis
        let action1 = Hash::from_bytes([1u8; 32]);
        let witness1 = builder.add_witness(
            action1,
            test_energy_snapshot(),
            GateDecision::allow(ComputeLane::Reflex),
        );
        assert!(witness1.is_genesis());
        let witness1_id = witness1.id.clone();

        // Second witness
        let action2 = Hash::from_bytes([2u8; 32]);
        let witness2 = builder.add_witness(
            action2,
            test_energy_snapshot(),
            GateDecision::deny(ComputeLane::Heavy, "High energy"),
        );
        assert!(!witness2.is_genesis());
        assert_eq!(witness2.sequence, 1);
        assert_eq!(witness2.previous_witness, Some(witness1_id));
    }

    #[test]
    fn test_chain_verification() {
        let policy_ref = test_policy_ref();

        // Create genesis
        let genesis = WitnessRecord::genesis(
            Hash::from_bytes([1u8; 32]),
            test_energy_snapshot(),
            GateDecision::allow(ComputeLane::Reflex),
            policy_ref.clone(),
        );

        // Create next witness
        let next = WitnessRecord::new(
            Hash::from_bytes([2u8; 32]),
            test_energy_snapshot(),
            GateDecision::allow(ComputeLane::Retrieval),
            policy_ref,
            Some(&genesis),
        );

        // Verify chain link
        assert!(next.verify_chain_link(&genesis).is_ok());
    }

    #[test]
    fn test_content_hash_determinism() {
        let action = Hash::from_bytes([1u8; 32]);
        let energy = test_energy_snapshot();
        let decision = GateDecision::allow(ComputeLane::Reflex);
        let policy_ref = test_policy_ref();

        let witness =
            WitnessRecord::genesis(action, energy.clone(), decision.clone(), policy_ref.clone());

        // Verify hash is consistent
        let hash1 = witness.compute_content_hash();
        let hash2 = witness.compute_content_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_tamper_detection() {
        let action = Hash::from_bytes([1u8; 32]);
        let energy = test_energy_snapshot();
        let decision = GateDecision::allow(ComputeLane::Reflex);
        let policy_ref = test_policy_ref();

        let mut witness = WitnessRecord::genesis(action, energy, decision, policy_ref);

        // Tamper with the witness
        witness.decision.confidence = 0.5;

        // Content hash should no longer match
        assert!(!witness.verify_content_hash());
    }

    #[test]
    fn test_gate_decision() {
        let allow = GateDecision::allow(ComputeLane::Reflex)
            .with_confidence(0.95)
            .with_metadata("source", "test");

        assert!(allow.allow);
        assert_eq!(allow.lane, ComputeLane::Reflex);
        assert!((allow.confidence - 0.95).abs() < f32::EPSILON);
        assert_eq!(allow.metadata.get("source"), Some(&"test".to_string()));

        let deny = GateDecision::deny(ComputeLane::Human, "High energy detected");
        assert!(!deny.allow);
        assert_eq!(deny.reason, Some("High energy detected".to_string()));
    }

    #[test]
    fn test_compute_lane() {
        assert_eq!(ComputeLane::from_u8(0), Some(ComputeLane::Reflex));
        assert_eq!(ComputeLane::from_u8(3), Some(ComputeLane::Human));
        assert_eq!(ComputeLane::from_u8(4), None);

        assert!(!ComputeLane::Reflex.requires_human());
        assert!(ComputeLane::Human.requires_human());
    }
}
