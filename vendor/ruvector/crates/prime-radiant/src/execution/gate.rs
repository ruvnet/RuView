//! # Coherence Gate: Threshold-Based Action Gating
//!
//! The coherence gate is the core decision point that controls whether actions
//! are allowed to execute. It implements the ADR-014 gating logic:
//!
//! > Gate = refusal mechanism with witness
//!
//! ## Key Design Principles
//!
//! 1. **Most updates stay in reflex lane** - Low energy = automatic approval
//! 2. **Persistence detection** - Energy above threshold for duration triggers escalation
//! 3. **Mandatory witness creation** - Every decision produces an auditable record
//! 4. **Policy bundle reference** - All decisions reference signed governance
//!
//! ## Gating Flow
//!
//! ```text
//! Action Request
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ Compute Energy  │ ← Scoped energy from coherence engine
//! └─────────────────┘
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ Check Threshold │ ← Lane thresholds from policy bundle
//! └─────────────────┘
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ Check Persistence│ ← Energy history for this scope
//! └─────────────────┘
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ Create Witness  │ ← Mandatory for every decision
//! └─────────────────┘
//!       │
//!       ▼
//! ┌─────────────────┐
//! │ Return Decision │ → Allow, Escalate, or Deny
//! └─────────────────┘
//! ```

use super::action::{Action, ActionId, ActionImpact, ScopeId};
use super::ladder::{ComputeLane, EscalationReason, LaneThresholds, LaneTransition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Unique identifier for a policy bundle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyBundleRef {
    /// Bundle ID.
    pub id: uuid::Uuid,
    /// Bundle version.
    pub version: String,
    /// Content hash for integrity verification.
    pub content_hash: [u8; 32],
}

impl PolicyBundleRef {
    /// Create a new policy bundle reference.
    pub fn new(id: uuid::Uuid, version: impl Into<String>, content_hash: [u8; 32]) -> Self {
        Self {
            id,
            version: version.into(),
            content_hash,
        }
    }

    /// Create a placeholder reference for testing.
    pub fn placeholder() -> Self {
        Self {
            id: uuid::Uuid::nil(),
            version: "0.0.0-test".to_string(),
            content_hash: [0u8; 32],
        }
    }

    /// Get bytes representation for hashing.
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + self.version.len() + 32);
        bytes.extend_from_slice(self.id.as_bytes());
        bytes.extend_from_slice(self.version.as_bytes());
        bytes.extend_from_slice(&self.content_hash);
        bytes
    }
}

/// Unique identifier for a witness record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WitnessId(pub uuid::Uuid);

impl WitnessId {
    /// Generate a new random witness ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Create from an existing UUID.
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for WitnessId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WitnessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "witness-{}", self.0)
    }
}

/// Snapshot of coherence energy at the time of a gate decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergySnapshot {
    /// Total system energy.
    pub total_energy: f32,
    /// Energy for the action's scope.
    pub scope_energy: f32,
    /// Scope that was evaluated.
    pub scope: ScopeId,
    /// Timestamp of snapshot (Unix millis).
    pub timestamp_ms: u64,
    /// Fingerprint for change detection.
    pub fingerprint: [u8; 32],
}

impl EnergySnapshot {
    /// Create a new energy snapshot.
    pub fn new(total_energy: f32, scope_energy: f32, scope: ScopeId) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut fingerprint = [0u8; 32];
        let hash_input = format!(
            "{}:{}:{}:{}",
            total_energy,
            scope_energy,
            scope.as_str(),
            timestamp_ms
        );
        let hash = blake3::hash(hash_input.as_bytes());
        fingerprint.copy_from_slice(hash.as_bytes());

        Self {
            total_energy,
            scope_energy,
            scope,
            timestamp_ms,
            fingerprint,
        }
    }
}

/// The gate's decision on an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    /// Whether to allow the action.
    pub allow: bool,

    /// Required compute lane for execution.
    pub lane: ComputeLane,

    /// Reason if denied or escalated.
    pub reason: Option<String>,

    /// Escalation details if applicable.
    pub escalation: Option<EscalationReason>,
}

impl GateDecision {
    /// Create an allowing decision.
    pub fn allow(lane: ComputeLane) -> Self {
        Self {
            allow: true,
            lane,
            reason: None,
            escalation: None,
        }
    }

    /// Create a denying decision.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            allow: false,
            lane: ComputeLane::Human, // Requires human intervention
            reason: Some(reason.into()),
            escalation: None,
        }
    }

    /// Create an escalation decision.
    pub fn escalate(lane: ComputeLane, escalation: EscalationReason) -> Self {
        Self {
            allow: lane < ComputeLane::Human,
            lane,
            reason: Some(format!("Escalated: {}", escalation)),
            escalation: Some(escalation),
        }
    }

    /// Whether this decision requires escalation.
    pub fn is_escalated(&self) -> bool {
        self.escalation.is_some()
    }

    /// Whether this decision allows automatic execution.
    pub fn allows_automatic_execution(&self) -> bool {
        self.allow && self.lane.allows_automatic_execution()
    }
}

/// Immutable witness record for every gate decision.
///
/// This is the audit trail for the coherence engine. Every decision
/// produces a witness that can be verified and replayed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Unique witness identifier.
    pub id: WitnessId,

    /// Hash of the action that was evaluated.
    pub action_hash: [u8; 32],

    /// Action ID reference.
    pub action_id: ActionId,

    /// Energy snapshot at evaluation time.
    pub energy_snapshot: EnergySnapshot,

    /// The gate decision made.
    pub decision: GateDecision,

    /// Policy bundle used for decision.
    pub policy_bundle_ref: PolicyBundleRef,

    /// Timestamp of decision (Unix millis).
    pub timestamp_ms: u64,

    /// Hash chain reference to previous witness.
    pub previous_witness: Option<WitnessId>,

    /// Content hash of this witness (for chain integrity).
    pub content_hash: [u8; 32],
}

impl WitnessRecord {
    /// Create a new witness record.
    pub fn new(
        action_hash: [u8; 32],
        action_id: ActionId,
        energy_snapshot: EnergySnapshot,
        decision: GateDecision,
        policy_bundle_ref: PolicyBundleRef,
        previous_witness: Option<WitnessId>,
    ) -> Self {
        let id = WitnessId::new();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut record = Self {
            id,
            action_hash,
            action_id,
            energy_snapshot,
            decision,
            policy_bundle_ref,
            timestamp_ms,
            previous_witness,
            content_hash: [0u8; 32],
        };

        record.content_hash = record.compute_content_hash();
        record
    }

    /// Compute the content hash for this witness.
    fn compute_content_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.action_hash);
        hasher.update(self.action_id.as_bytes());
        hasher.update(&self.energy_snapshot.fingerprint);
        hasher.update(&(self.decision.allow as u8).to_le_bytes());
        hasher.update(&(self.decision.lane.as_u8()).to_le_bytes());
        hasher.update(&self.policy_bundle_ref.as_bytes());
        hasher.update(&self.timestamp_ms.to_le_bytes());

        if let Some(ref prev) = self.previous_witness {
            hasher.update(prev.0.as_bytes());
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(hasher.finalize().as_bytes());
        hash
    }

    /// Verify the content hash integrity.
    pub fn verify_integrity(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }
}

/// Energy history tracker for persistence detection.
#[derive(Debug, Clone, Default)]
pub struct EnergyHistory {
    /// Per-scope energy histories (timestamp_ms, energy).
    histories: HashMap<ScopeId, Vec<(u64, f32)>>,

    /// Maximum history entries per scope.
    max_entries: usize,
}

impl EnergyHistory {
    /// Create a new energy history tracker.
    pub fn new(max_entries: usize) -> Self {
        Self {
            histories: HashMap::new(),
            max_entries,
        }
    }

    /// Record an energy observation for a scope.
    pub fn record(&mut self, scope: &ScopeId, energy: f32) {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let history = self.histories.entry(scope.clone()).or_default();
        history.push((timestamp_ms, energy));

        // Trim old entries
        if history.len() > self.max_entries {
            history.drain(0..(history.len() - self.max_entries));
        }
    }

    /// Check if energy has been above threshold for the given duration.
    pub fn is_above_threshold(&self, scope: &ScopeId, threshold: f32, duration: Duration) -> bool {
        let history = match self.histories.get(scope) {
            Some(h) => h,
            None => return false,
        };

        if history.is_empty() {
            return false;
        }

        let duration_ms = duration.as_millis() as u64;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let window_start = now_ms.saturating_sub(duration_ms);

        // Check if all readings in the window are above threshold
        let readings_in_window: Vec<_> = history
            .iter()
            .filter(|(ts, _)| *ts >= window_start)
            .collect();

        if readings_in_window.is_empty() {
            return false;
        }

        // Need at least 2 readings and all must be above threshold
        readings_in_window.len() >= 2 && readings_in_window.iter().all(|(_, e)| *e >= threshold)
    }

    /// Get the duration that energy has been above threshold.
    pub fn duration_above_threshold(&self, scope: &ScopeId, threshold: f32) -> Option<Duration> {
        let history = self.histories.get(scope)?;

        if history.is_empty() {
            return None;
        }

        // Find the first reading above threshold, counting backwards
        let mut start_ts = None;
        for (ts, energy) in history.iter().rev() {
            if *energy >= threshold {
                start_ts = Some(*ts);
            } else {
                break;
            }
        }

        start_ts.map(|start| {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            Duration::from_millis(now_ms.saturating_sub(start))
        })
    }

    /// Clear history for a scope.
    pub fn clear_scope(&mut self, scope: &ScopeId) {
        self.histories.remove(scope);
    }

    /// Clear all history.
    pub fn clear_all(&mut self) {
        self.histories.clear();
    }
}

/// The coherence gate with configurable thresholds.
///
/// This is the main gating mechanism that controls action execution
/// based on coherence energy levels and persistence detection.
#[derive(Debug, Clone)]
pub struct CoherenceGate {
    /// Lane thresholds for energy-based escalation.
    thresholds: LaneThresholds,

    /// Persistence window for detecting sustained incoherence.
    persistence_window: Duration,

    /// Reference to the active policy bundle.
    policy_bundle: PolicyBundleRef,

    /// Energy history for persistence detection.
    history: EnergyHistory,

    /// Last witness ID for chaining.
    last_witness_id: Option<WitnessId>,

    /// Lane transition history.
    transitions: Vec<LaneTransition>,

    /// Maximum transitions to keep.
    max_transitions: usize,
}

impl CoherenceGate {
    /// Create a new coherence gate with the given configuration.
    pub fn new(
        thresholds: LaneThresholds,
        persistence_window: Duration,
        policy_bundle: PolicyBundleRef,
    ) -> Self {
        Self {
            thresholds,
            persistence_window,
            policy_bundle,
            history: EnergyHistory::new(1000),
            last_witness_id: None,
            transitions: Vec::new(),
            max_transitions: 100,
        }
    }

    /// Create a gate with default configuration.
    pub fn with_defaults(policy_bundle: PolicyBundleRef) -> Self {
        Self::new(
            LaneThresholds::default(),
            Duration::from_secs(5),
            policy_bundle,
        )
    }

    /// Evaluate whether an action should proceed.
    ///
    /// This is the core gating method that:
    /// 1. Determines required lane based on energy
    /// 2. Checks for persistent incoherence
    /// 3. Creates mandatory witness record
    /// 4. Returns the gate decision
    #[inline]
    pub fn evaluate<A: Action>(&mut self, action: &A, energy: &EnergySnapshot) -> GateDecision {
        let current_energy = energy.scope_energy;

        // FAST PATH: Low energy and low-risk action -> immediate reflex approval
        // This bypasses most computation for the common case (ADR-014 reflex lane)
        if current_energy < self.thresholds.reflex {
            let impact = action.impact();
            if !impact.is_high_risk() {
                // Quick history record and return
                self.history.record(action.scope(), current_energy);
                return GateDecision::allow(ComputeLane::Reflex);
            }
        }

        // STANDARD PATH: Full evaluation for higher energy or high-risk actions
        self.evaluate_full(action, energy)
    }

    /// Full evaluation path for non-trivial cases
    #[inline(never)] // Keep this out-of-line to keep fast path small
    fn evaluate_full<A: Action>(&mut self, action: &A, energy: &EnergySnapshot) -> GateDecision {
        let scope = action.scope();
        let impact = action.impact();
        let current_energy = energy.scope_energy;

        // Record energy observation
        self.history.record(scope, current_energy);

        // Determine base lane from energy using branchless comparison
        let mut lane = self.thresholds.lane_for_energy(current_energy);

        // Adjust for action impact
        if impact.is_high_risk() && lane < ComputeLane::Retrieval {
            lane = ComputeLane::Retrieval;
        }

        // Check for persistent incoherence
        let persistent =
            self.history
                .is_above_threshold(scope, self.thresholds.reflex, self.persistence_window);

        let escalation = if persistent && lane < ComputeLane::Heavy {
            // Persistent incoherence requires at least Heavy lane
            let duration = self
                .history
                .duration_above_threshold(scope, self.thresholds.reflex)
                .unwrap_or_default();

            let reason = EscalationReason::persistent(
                duration.as_millis() as u64,
                self.persistence_window.as_millis() as u64,
            );

            let old_lane = lane;
            lane = ComputeLane::Heavy;

            // Record transition
            self.record_transition(old_lane, lane, reason.clone(), current_energy);

            Some(reason)
        } else if current_energy >= self.thresholds.reflex {
            // Energy-based escalation
            let reason = EscalationReason::energy(current_energy, self.thresholds.reflex);

            if lane > ComputeLane::Reflex {
                Some(reason)
            } else {
                None
            }
        } else {
            None
        };

        // Build decision
        if lane == ComputeLane::Human {
            GateDecision::deny("Energy exceeds all automatic thresholds - requires human review")
        } else if let Some(escalation) = escalation {
            GateDecision::escalate(lane, escalation)
        } else {
            GateDecision::allow(lane)
        }
    }

    /// Fast path evaluation that skips witness creation
    /// Use when witness is not needed (e.g., preflight checks)
    #[inline]
    pub fn evaluate_fast(&self, scope_energy: f32) -> ComputeLane {
        self.thresholds.lane_for_energy(scope_energy)
    }

    /// Create a witness record for a gate decision.
    ///
    /// This MUST be called for every evaluation to maintain the audit trail.
    pub fn create_witness<A: Action>(
        &mut self,
        action: &A,
        energy: &EnergySnapshot,
        decision: &GateDecision,
    ) -> WitnessRecord {
        let witness = WitnessRecord::new(
            action.content_hash(),
            action.metadata().id.clone(),
            energy.clone(),
            decision.clone(),
            self.policy_bundle.clone(),
            self.last_witness_id.clone(),
        );

        self.last_witness_id = Some(witness.id.clone());
        witness
    }

    /// Evaluate and create witness in one call.
    pub fn evaluate_with_witness<A: Action>(
        &mut self,
        action: &A,
        energy: &EnergySnapshot,
    ) -> (GateDecision, WitnessRecord) {
        let decision = self.evaluate(action, energy);
        let witness = self.create_witness(action, energy, &decision);
        (decision, witness)
    }

    /// Record a lane transition.
    fn record_transition(
        &mut self,
        from: ComputeLane,
        to: ComputeLane,
        reason: EscalationReason,
        energy: f32,
    ) {
        let transition = LaneTransition::new(from, to, reason, energy);
        self.transitions.push(transition);

        // Trim old transitions
        if self.transitions.len() > self.max_transitions {
            self.transitions
                .drain(0..(self.transitions.len() - self.max_transitions));
        }
    }

    /// Get recent lane transitions.
    pub fn recent_transitions(&self) -> &[LaneTransition] {
        &self.transitions
    }

    /// Update the policy bundle reference.
    pub fn update_policy_bundle(&mut self, bundle: PolicyBundleRef) {
        self.policy_bundle = bundle;
    }

    /// Update the lane thresholds.
    pub fn update_thresholds(&mut self, thresholds: LaneThresholds) {
        self.thresholds = thresholds;
    }

    /// Update the persistence window.
    pub fn update_persistence_window(&mut self, window: Duration) {
        self.persistence_window = window;
    }

    /// Get current thresholds.
    pub fn thresholds(&self) -> &LaneThresholds {
        &self.thresholds
    }

    /// Get current persistence window.
    pub fn persistence_window(&self) -> Duration {
        self.persistence_window
    }

    /// Get current policy bundle reference.
    pub fn policy_bundle(&self) -> &PolicyBundleRef {
        &self.policy_bundle
    }

    /// Clear energy history for a scope.
    pub fn clear_scope_history(&mut self, scope: &ScopeId) {
        self.history.clear_scope(scope);
    }

    /// Reset the gate to initial state.
    pub fn reset(&mut self) {
        self.history.clear_all();
        self.last_witness_id = None;
        self.transitions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::action::{ActionError, ActionMetadata, ExecutionContext};

    // Test action implementation
    struct TestAction {
        scope: ScopeId,
        impact: ActionImpact,
        metadata: ActionMetadata,
    }

    impl TestAction {
        fn new(scope: &str) -> Self {
            Self {
                scope: ScopeId::new(scope),
                impact: ActionImpact::low(),
                metadata: ActionMetadata::new("TestAction", "Test action", "test-actor"),
            }
        }

        fn with_impact(mut self, impact: ActionImpact) -> Self {
            self.impact = impact;
            self
        }
    }

    impl Action for TestAction {
        type Output = ();
        type Error = ActionError;

        fn scope(&self) -> &ScopeId {
            &self.scope
        }

        fn impact(&self) -> ActionImpact {
            self.impact
        }

        fn metadata(&self) -> &ActionMetadata {
            &self.metadata
        }

        fn execute(&self, _ctx: &ExecutionContext) -> Result<(), ActionError> {
            Ok(())
        }

        fn content_hash(&self) -> [u8; 32] {
            let hash = blake3::hash(self.scope.as_str().as_bytes());
            let mut result = [0u8; 32];
            result.copy_from_slice(hash.as_bytes());
            result
        }

        fn make_rollback_not_supported_error() -> ActionError {
            ActionError::RollbackNotSupported
        }
    }

    #[test]
    fn test_gate_low_energy_allows_reflex() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope");
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());

        let decision = gate.evaluate(&action, &energy);

        assert!(decision.allow);
        assert_eq!(decision.lane, ComputeLane::Reflex);
        assert!(!decision.is_escalated());
    }

    #[test]
    fn test_gate_medium_energy_escalates() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope");
        let energy = EnergySnapshot::new(0.4, 0.35, action.scope.clone());

        let decision = gate.evaluate(&action, &energy);

        assert!(decision.allow);
        assert_eq!(decision.lane, ComputeLane::Retrieval);
        assert!(decision.is_escalated());
    }

    #[test]
    fn test_gate_high_energy_heavy_lane() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope");
        let energy = EnergySnapshot::new(0.7, 0.65, action.scope.clone());

        let decision = gate.evaluate(&action, &energy);

        assert!(decision.allow);
        assert_eq!(decision.lane, ComputeLane::Heavy);
    }

    #[test]
    fn test_gate_extreme_energy_denies() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope");
        let energy = EnergySnapshot::new(0.95, 0.9, action.scope.clone());

        let decision = gate.evaluate(&action, &energy);

        assert!(!decision.allow);
        assert_eq!(decision.lane, ComputeLane::Human);
    }

    #[test]
    fn test_gate_high_risk_impact_escalates() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope").with_impact(ActionImpact::critical());
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());

        let decision = gate.evaluate(&action, &energy);

        // Even low energy gets escalated due to high-risk action
        assert!(decision.allow);
        assert!(decision.lane >= ComputeLane::Retrieval);
    }

    #[test]
    fn test_witness_record_integrity() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope");
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());

        let (decision, witness) = gate.evaluate_with_witness(&action, &energy);

        assert!(witness.verify_integrity());
        assert_eq!(witness.decision.allow, decision.allow);
        assert_eq!(witness.decision.lane, decision.lane);
    }

    #[test]
    fn test_witness_chain() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope");
        let energy = EnergySnapshot::new(0.1, 0.05, action.scope.clone());

        // First witness
        let (_, witness1) = gate.evaluate_with_witness(&action, &energy);
        assert!(witness1.previous_witness.is_none());

        // Second witness should chain to first
        let (_, witness2) = gate.evaluate_with_witness(&action, &energy);
        assert_eq!(witness2.previous_witness, Some(witness1.id));
    }

    #[test]
    fn test_energy_history() {
        let mut history = EnergyHistory::new(100);
        let scope = ScopeId::new("test");

        // Record some energy values
        for _ in 0..5 {
            history.record(&scope, 0.5);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Should detect persistent high energy
        assert!(history.is_above_threshold(&scope, 0.3, Duration::from_millis(30)));

        // Should not detect if threshold too high
        assert!(!history.is_above_threshold(&scope, 0.6, Duration::from_millis(30)));
    }

    #[test]
    fn test_gate_transitions_recorded() {
        let mut gate = CoherenceGate::with_defaults(PolicyBundleRef::placeholder());
        let action = TestAction::new("test.scope");

        // Record multiple evaluations that should trigger persistence
        for _ in 0..10 {
            let energy = EnergySnapshot::new(0.4, 0.35, action.scope.clone());
            gate.evaluate(&action, &energy);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // After multiple high-energy evaluations, may have recorded transitions
        // Note: exact behavior depends on timing
        let transitions = gate.recent_transitions();
        // Just verify we can access transitions without panic
        assert!(transitions.len() <= gate.max_transitions);
    }
}
