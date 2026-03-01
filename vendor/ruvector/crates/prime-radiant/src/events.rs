//! Domain events for the Prime-Radiant coherence engine.
//!
//! All domain events are persisted to the event log for deterministic replay.
//! This enables:
//! - Temporal ordering of all decisions
//! - Tamper detection via content hashes
//! - Deterministic replay capability

use crate::types::{
    EdgeId, Hash, LineageId, NodeId, PolicyBundleId, ScopeId, Timestamp, WitnessId,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// DOMAIN EVENT ENUM
// ============================================================================

/// All domain events in the coherence engine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    // -------------------------------------------------------------------------
    // Substrate Events
    // -------------------------------------------------------------------------
    /// A new node was created in the sheaf graph
    NodeCreated {
        /// Node ID
        node_id: NodeId,
        /// Namespace
        namespace: String,
        /// State dimension
        dimension: usize,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// A node's state was updated
    NodeUpdated {
        /// Node ID
        node_id: NodeId,
        /// Previous state hash
        previous_hash: Hash,
        /// New state hash
        new_hash: Hash,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// A node was removed from the graph
    NodeRemoved {
        /// Node ID
        node_id: NodeId,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// A new edge was created with restriction maps
    EdgeCreated {
        /// Edge ID
        edge_id: EdgeId,
        /// Source node
        source: NodeId,
        /// Target node
        target: NodeId,
        /// Edge weight
        weight: f32,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// An edge was removed
    EdgeRemoved {
        /// Edge ID
        edge_id: EdgeId,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Edge weight was updated
    EdgeWeightUpdated {
        /// Edge ID
        edge_id: EdgeId,
        /// Previous weight
        previous_weight: f32,
        /// New weight
        new_weight: f32,
        /// Event timestamp
        timestamp: Timestamp,
    },

    // -------------------------------------------------------------------------
    // Coherence Computation Events
    // -------------------------------------------------------------------------
    /// Full coherence energy was computed
    EnergyComputed {
        /// Total energy value
        total_energy: f32,
        /// Number of edges computed
        edge_count: usize,
        /// Graph fingerprint
        fingerprint: Hash,
        /// Computation duration in microseconds
        duration_us: u64,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Incremental energy update was computed
    EnergyUpdated {
        /// Node that triggered update
        trigger_node: NodeId,
        /// Number of affected edges
        affected_edges: usize,
        /// New total energy
        new_energy: f32,
        /// Delta from previous energy
        energy_delta: f32,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Spectral drift was detected
    DriftDetected {
        /// Drift magnitude
        magnitude: f32,
        /// Affected eigenvalue modes
        affected_modes: Vec<usize>,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// High-energy edge identified (hotspot)
    HotspotIdentified {
        /// Edge ID
        edge_id: EdgeId,
        /// Edge energy
        energy: f32,
        /// Energy rank (1 = highest)
        rank: usize,
        /// Event timestamp
        timestamp: Timestamp,
    },

    // -------------------------------------------------------------------------
    // Governance Events
    // -------------------------------------------------------------------------
    /// New policy bundle was created
    PolicyCreated {
        /// Policy bundle ID
        bundle_id: PolicyBundleId,
        /// Version
        version: String,
        /// Required approvals
        required_approvals: usize,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Policy bundle was signed by an approver
    PolicySigned {
        /// Policy bundle ID
        bundle_id: PolicyBundleId,
        /// Approver ID (as string for serialization)
        approver: String,
        /// Current signature count
        signature_count: usize,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Policy bundle reached required approvals
    PolicyApproved {
        /// Policy bundle ID
        bundle_id: PolicyBundleId,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Policy bundle was activated
    PolicyActivated {
        /// Policy bundle ID
        bundle_id: PolicyBundleId,
        /// Previous active policy (if any)
        previous_policy: Option<PolicyBundleId>,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Policy bundle was deprecated
    PolicyDeprecated {
        /// Policy bundle ID
        bundle_id: PolicyBundleId,
        /// Replacement policy
        replacement: PolicyBundleId,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Witness record was created
    WitnessCreated {
        /// Witness ID
        witness_id: WitnessId,
        /// Action hash
        action_hash: Hash,
        /// Energy at decision time
        energy: f32,
        /// Decision (allowed/denied)
        allowed: bool,
        /// Compute lane assigned
        lane: u8,
        /// Previous witness in chain
        previous_witness: Option<WitnessId>,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Lineage record was created
    LineageCreated {
        /// Lineage ID
        lineage_id: LineageId,
        /// Entity reference
        entity_ref: String,
        /// Operation type
        operation: String,
        /// Authorizing witness
        witness_id: WitnessId,
        /// Event timestamp
        timestamp: Timestamp,
    },

    // -------------------------------------------------------------------------
    // Execution Events
    // -------------------------------------------------------------------------
    /// Action was allowed by the coherence gate
    ActionAllowed {
        /// Action hash
        action_hash: Hash,
        /// Scope
        scope: ScopeId,
        /// Compute lane used
        lane: u8,
        /// Energy at decision
        energy: f32,
        /// Witness ID
        witness_id: WitnessId,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Action was denied by the coherence gate
    ActionDenied {
        /// Action hash
        action_hash: Hash,
        /// Scope
        scope: ScopeId,
        /// Reason for denial
        reason: String,
        /// Energy at decision
        energy: f32,
        /// Witness ID
        witness_id: WitnessId,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Escalation was triggered
    EscalationTriggered {
        /// Action hash
        action_hash: Hash,
        /// From lane
        from_lane: u8,
        /// To lane
        to_lane: u8,
        /// Reason
        reason: String,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Human review was requested
    HumanReviewRequested {
        /// Action hash
        action_hash: Hash,
        /// Scope
        scope: ScopeId,
        /// Energy at request
        energy: f32,
        /// Persistence duration in seconds
        persistence_secs: u64,
        /// Event timestamp
        timestamp: Timestamp,
    },

    // -------------------------------------------------------------------------
    // Threshold Tuning Events (SONA)
    // -------------------------------------------------------------------------
    /// Regime started for threshold learning
    RegimeStarted {
        /// Regime ID
        regime_id: String,
        /// Initial energy
        initial_energy: f32,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Regime ended with outcome
    RegimeEnded {
        /// Regime ID
        regime_id: String,
        /// Final quality score
        quality: f32,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Successful pattern was learned
    PatternLearned {
        /// Pattern type
        pattern_type: String,
        /// Quality score
        quality: f32,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Threshold was adapted via Micro-LoRA
    ThresholdAdapted {
        /// Scope affected
        scope: ScopeId,
        /// Previous threshold
        previous_reflex: f32,
        /// New threshold
        new_reflex: f32,
        /// Trigger (energy spike magnitude)
        trigger: f32,
        /// Event timestamp
        timestamp: Timestamp,
    },

    // -------------------------------------------------------------------------
    // Tile Fabric Events
    // -------------------------------------------------------------------------
    /// Fabric tick completed
    FabricTickCompleted {
        /// Tick number
        tick: u32,
        /// Global energy
        global_energy: f32,
        /// Active tiles
        active_tiles: usize,
        /// Duration in microseconds
        duration_us: u64,
        /// Event timestamp
        timestamp: Timestamp,
    },

    /// Evidence threshold crossed in tile
    EvidenceThresholdCrossed {
        /// Tile ID
        tile_id: u8,
        /// E-value
        e_value: f64,
        /// Event timestamp
        timestamp: Timestamp,
    },
}

impl DomainEvent {
    /// Get the event type as a string
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::NodeCreated { .. } => "NodeCreated",
            Self::NodeUpdated { .. } => "NodeUpdated",
            Self::NodeRemoved { .. } => "NodeRemoved",
            Self::EdgeCreated { .. } => "EdgeCreated",
            Self::EdgeRemoved { .. } => "EdgeRemoved",
            Self::EdgeWeightUpdated { .. } => "EdgeWeightUpdated",
            Self::EnergyComputed { .. } => "EnergyComputed",
            Self::EnergyUpdated { .. } => "EnergyUpdated",
            Self::DriftDetected { .. } => "DriftDetected",
            Self::HotspotIdentified { .. } => "HotspotIdentified",
            Self::PolicyCreated { .. } => "PolicyCreated",
            Self::PolicySigned { .. } => "PolicySigned",
            Self::PolicyApproved { .. } => "PolicyApproved",
            Self::PolicyActivated { .. } => "PolicyActivated",
            Self::PolicyDeprecated { .. } => "PolicyDeprecated",
            Self::WitnessCreated { .. } => "WitnessCreated",
            Self::LineageCreated { .. } => "LineageCreated",
            Self::ActionAllowed { .. } => "ActionAllowed",
            Self::ActionDenied { .. } => "ActionDenied",
            Self::EscalationTriggered { .. } => "EscalationTriggered",
            Self::HumanReviewRequested { .. } => "HumanReviewRequested",
            Self::RegimeStarted { .. } => "RegimeStarted",
            Self::RegimeEnded { .. } => "RegimeEnded",
            Self::PatternLearned { .. } => "PatternLearned",
            Self::ThresholdAdapted { .. } => "ThresholdAdapted",
            Self::FabricTickCompleted { .. } => "FabricTickCompleted",
            Self::EvidenceThresholdCrossed { .. } => "EvidenceThresholdCrossed",
        }
    }

    /// Get the timestamp of the event
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Self::NodeCreated { timestamp, .. }
            | Self::NodeUpdated { timestamp, .. }
            | Self::NodeRemoved { timestamp, .. }
            | Self::EdgeCreated { timestamp, .. }
            | Self::EdgeRemoved { timestamp, .. }
            | Self::EdgeWeightUpdated { timestamp, .. }
            | Self::EnergyComputed { timestamp, .. }
            | Self::EnergyUpdated { timestamp, .. }
            | Self::DriftDetected { timestamp, .. }
            | Self::HotspotIdentified { timestamp, .. }
            | Self::PolicyCreated { timestamp, .. }
            | Self::PolicySigned { timestamp, .. }
            | Self::PolicyApproved { timestamp, .. }
            | Self::PolicyActivated { timestamp, .. }
            | Self::PolicyDeprecated { timestamp, .. }
            | Self::WitnessCreated { timestamp, .. }
            | Self::LineageCreated { timestamp, .. }
            | Self::ActionAllowed { timestamp, .. }
            | Self::ActionDenied { timestamp, .. }
            | Self::EscalationTriggered { timestamp, .. }
            | Self::HumanReviewRequested { timestamp, .. }
            | Self::RegimeStarted { timestamp, .. }
            | Self::RegimeEnded { timestamp, .. }
            | Self::PatternLearned { timestamp, .. }
            | Self::ThresholdAdapted { timestamp, .. }
            | Self::FabricTickCompleted { timestamp, .. }
            | Self::EvidenceThresholdCrossed { timestamp, .. } => *timestamp,
        }
    }

    /// Compute content hash for integrity
    pub fn content_hash(&self) -> Hash {
        let serialized = serde_json::to_vec(self).unwrap_or_default();
        Hash::digest(&serialized)
    }
}

// ============================================================================
// EVENT METADATA
// ============================================================================

/// Metadata for an event in the event log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Sequence number in the log
    pub sequence: u64,
    /// Content hash for integrity
    pub content_hash: Hash,
    /// Signature (if signed)
    pub signature: Option<Vec<u8>>,
}

/// A complete event record with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    /// The domain event
    pub event: DomainEvent,
    /// Event metadata
    pub metadata: EventMetadata,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = DomainEvent::NodeCreated {
            node_id: NodeId::new(),
            namespace: "test".to_string(),
            dimension: 64,
            timestamp: Timestamp::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: DomainEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.event_type(), decoded.event_type());
    }

    #[test]
    fn test_event_content_hash() {
        let event = DomainEvent::EnergyComputed {
            total_energy: 0.5,
            edge_count: 100,
            fingerprint: Hash::zero(),
            duration_us: 1000,
            timestamp: Timestamp::now(),
        };

        let h1 = event.content_hash();
        let h2 = event.content_hash();
        assert_eq!(h1, h2);
    }
}
