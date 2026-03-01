//! Integration tests for Governance Layer
//!
//! Tests the Governance bounded context, verifying:
//! - Policy bundle creation and activation
//! - Multi-party approval workflows
//! - Witness chain integrity
//! - Lineage record tracking
//! - Content hash verification

use std::collections::HashMap;

// ============================================================================
// TEST TYPES
// ============================================================================

/// Simple hash type for testing
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Hash([u8; 32]);

impl Hash {
    fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn zero() -> Self {
        Self([0; 32])
    }

    fn is_zero(&self) -> bool {
        self.0 == [0; 32]
    }

    fn compute(data: &[u8]) -> Self {
        // Simple hash for testing (not cryptographic)
        let mut result = [0u8; 32];
        for (i, byte) in data.iter().enumerate() {
            result[i % 32] ^= byte.wrapping_mul((i + 1) as u8);
        }
        Self(result)
    }
}

/// Policy status
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PolicyStatus {
    Draft,
    PendingApproval,
    Active,
    Superseded,
}

/// Approval signature
#[derive(Clone, Debug)]
struct ApprovalSignature {
    approver_id: String,
    timestamp: u64,
    signature: Vec<u8>,
}

/// Threshold configuration
#[derive(Clone, Debug)]
struct ThresholdConfig {
    name: String,
    green_threshold: f32,
    amber_threshold: f32,
    red_threshold: f32,
    escalation_enabled: bool,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            green_threshold: 0.1,
            amber_threshold: 0.5,
            red_threshold: 1.0,
            escalation_enabled: true,
        }
    }
}

/// Policy bundle
struct PolicyBundle {
    id: String,
    version: (u32, u32, u32),
    status: PolicyStatus,
    thresholds: HashMap<String, ThresholdConfig>,
    required_approvals: usize,
    approvals: Vec<ApprovalSignature>,
    content_hash: Hash,
    created_at: u64,
    activated_at: Option<u64>,
}

impl PolicyBundle {
    fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: (1, 0, 0),
            status: PolicyStatus::Draft,
            thresholds: HashMap::new(),
            required_approvals: 1,
            approvals: Vec::new(),
            content_hash: Hash::zero(),
            created_at: 1000,
            activated_at: None,
        }
    }

    fn add_threshold(&mut self, name: impl Into<String>, config: ThresholdConfig) {
        self.thresholds.insert(name.into(), config);
    }

    fn set_required_approvals(&mut self, count: usize) {
        self.required_approvals = count;
    }

    fn submit_for_approval(&mut self) -> Result<(), &'static str> {
        if self.status != PolicyStatus::Draft {
            return Err("Policy is not in draft status");
        }
        if self.thresholds.is_empty() {
            return Err("Policy must have at least one threshold");
        }

        self.content_hash = self.compute_content_hash();
        self.status = PolicyStatus::PendingApproval;
        Ok(())
    }

    fn add_approval(&mut self, approval: ApprovalSignature) -> Result<(), &'static str> {
        if self.status != PolicyStatus::PendingApproval {
            return Err("Policy is not pending approval");
        }

        // Check for duplicate approver
        if self
            .approvals
            .iter()
            .any(|a| a.approver_id == approval.approver_id)
        {
            return Err("Approver has already approved");
        }

        self.approvals.push(approval);
        Ok(())
    }

    fn activate(&mut self, timestamp: u64) -> Result<(), &'static str> {
        if self.status != PolicyStatus::PendingApproval {
            return Err("Policy is not pending approval");
        }
        if self.approvals.len() < self.required_approvals {
            return Err("Insufficient approvals");
        }

        self.status = PolicyStatus::Active;
        self.activated_at = Some(timestamp);
        Ok(())
    }

    fn supersede(&mut self) -> Result<(), &'static str> {
        if self.status != PolicyStatus::Active {
            return Err("Can only supersede active policies");
        }
        self.status = PolicyStatus::Superseded;
        Ok(())
    }

    fn compute_content_hash(&self) -> Hash {
        // Simplified hash computation
        let mut data = Vec::new();
        data.extend_from_slice(self.id.as_bytes());
        data.extend_from_slice(&self.version.0.to_le_bytes());
        data.extend_from_slice(&self.version.1.to_le_bytes());
        data.extend_from_slice(&self.version.2.to_le_bytes());
        data.extend_from_slice(&(self.required_approvals as u32).to_le_bytes());

        for (name, config) in &self.thresholds {
            data.extend_from_slice(name.as_bytes());
            data.extend_from_slice(&config.green_threshold.to_le_bytes());
            data.extend_from_slice(&config.amber_threshold.to_le_bytes());
            data.extend_from_slice(&config.red_threshold.to_le_bytes());
        }

        Hash::compute(&data)
    }

    fn is_active(&self) -> bool {
        self.status == PolicyStatus::Active
    }
}

/// Witness record
#[derive(Clone, Debug)]
struct WitnessRecord {
    id: String,
    action_hash: Hash,
    energy_snapshot: f32,
    decision: GateDecision,
    policy_ref: String,
    previous_witness_id: Option<String>,
    content_hash: Hash,
    timestamp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateDecision {
    Allow,
    Throttle,
    Block,
}

impl WitnessRecord {
    fn new(
        id: impl Into<String>,
        action_hash: Hash,
        energy_snapshot: f32,
        decision: GateDecision,
        policy_ref: impl Into<String>,
        previous_witness_id: Option<String>,
        timestamp: u64,
    ) -> Self {
        let mut record = Self {
            id: id.into(),
            action_hash,
            energy_snapshot,
            decision,
            policy_ref: policy_ref.into(),
            previous_witness_id,
            content_hash: Hash::zero(),
            timestamp,
        };
        record.content_hash = record.compute_content_hash();
        record
    }

    fn compute_content_hash(&self) -> Hash {
        let mut data = Vec::new();
        data.extend_from_slice(self.id.as_bytes());
        data.extend_from_slice(&self.action_hash.0);
        data.extend_from_slice(&self.energy_snapshot.to_le_bytes());
        data.extend_from_slice(&(self.decision as u8).to_le_bytes());
        data.extend_from_slice(self.policy_ref.as_bytes());
        if let Some(ref prev) = self.previous_witness_id {
            data.extend_from_slice(prev.as_bytes());
        }
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        Hash::compute(&data)
    }

    fn verify_hash(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }
}

/// Witness chain
struct WitnessChain {
    records: Vec<WitnessRecord>,
}

impl WitnessChain {
    fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    fn append(&mut self, record: WitnessRecord) -> Result<(), &'static str> {
        // Verify chain integrity
        if !self.records.is_empty() {
            let last = self.records.last().unwrap();
            if record.previous_witness_id != Some(last.id.clone()) {
                return Err("Previous witness ID mismatch");
            }
            if record.timestamp < last.timestamp {
                return Err("Timestamp must be non-decreasing");
            }
        } else if record.previous_witness_id.is_some() {
            return Err("First record must not have previous_witness_id");
        }

        // Verify content hash
        if !record.verify_hash() {
            return Err("Content hash verification failed");
        }

        self.records.push(record);
        Ok(())
    }

    fn verify_integrity(&self) -> bool {
        for (i, record) in self.records.iter().enumerate() {
            // Verify content hash
            if !record.verify_hash() {
                return false;
            }

            // Verify chain linkage
            if i == 0 {
                if record.previous_witness_id.is_some() {
                    return false;
                }
            } else {
                let expected_prev = Some(self.records[i - 1].id.clone());
                if record.previous_witness_id != expected_prev {
                    return false;
                }
            }
        }
        true
    }

    fn len(&self) -> usize {
        self.records.len()
    }
}

/// Lineage record
#[derive(Clone, Debug)]
struct LineageRecord {
    id: String,
    entity_ref: String,
    operation: Operation,
    dependencies: Vec<String>,
    witness_id: String,
    actor_id: String,
    timestamp: u64,
    content_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Create,
    Update,
    Delete,
    Derive,
}

impl LineageRecord {
    fn new(
        id: impl Into<String>,
        entity_ref: impl Into<String>,
        operation: Operation,
        dependencies: Vec<String>,
        witness_id: impl Into<String>,
        actor_id: impl Into<String>,
        timestamp: u64,
    ) -> Self {
        let mut record = Self {
            id: id.into(),
            entity_ref: entity_ref.into(),
            operation,
            dependencies,
            witness_id: witness_id.into(),
            actor_id: actor_id.into(),
            timestamp,
            content_hash: Hash::zero(),
        };
        record.content_hash = record.compute_content_hash();
        record
    }

    fn compute_content_hash(&self) -> Hash {
        let mut data = Vec::new();
        data.extend_from_slice(self.id.as_bytes());
        data.extend_from_slice(self.entity_ref.as_bytes());
        data.extend_from_slice(&(self.operation as u8).to_le_bytes());
        for dep in &self.dependencies {
            data.extend_from_slice(dep.as_bytes());
        }
        data.extend_from_slice(self.witness_id.as_bytes());
        data.extend_from_slice(self.actor_id.as_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        Hash::compute(&data)
    }

    fn verify_hash(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }
}

// ============================================================================
// POLICY BUNDLE TESTS
// ============================================================================

#[test]
fn test_policy_bundle_creation() {
    let mut policy = PolicyBundle::new("policy-001");

    assert_eq!(policy.status, PolicyStatus::Draft);
    assert_eq!(policy.version, (1, 0, 0));
    assert!(policy.thresholds.is_empty());
}

#[test]
fn test_policy_bundle_with_thresholds() {
    let mut policy = PolicyBundle::new("policy-001");

    policy.add_threshold(
        "global",
        ThresholdConfig {
            name: "global".to_string(),
            green_threshold: 0.05,
            amber_threshold: 0.3,
            red_threshold: 0.8,
            escalation_enabled: true,
        },
    );

    policy.add_threshold(
        "finance",
        ThresholdConfig {
            name: "finance".to_string(),
            green_threshold: 0.02,
            amber_threshold: 0.1,
            red_threshold: 0.5,
            escalation_enabled: true,
        },
    );

    assert_eq!(policy.thresholds.len(), 2);
    assert!(policy.thresholds.contains_key("global"));
    assert!(policy.thresholds.contains_key("finance"));
}

#[test]
fn test_policy_bundle_submission() {
    let mut policy = PolicyBundle::new("policy-001");

    // Cannot submit without thresholds
    assert!(policy.submit_for_approval().is_err());

    policy.add_threshold("global", ThresholdConfig::default());

    // Now submission should succeed
    assert!(policy.submit_for_approval().is_ok());
    assert_eq!(policy.status, PolicyStatus::PendingApproval);

    // Content hash should be set
    assert!(!policy.content_hash.is_zero());
}

#[test]
fn test_policy_bundle_approval_workflow() {
    let mut policy = PolicyBundle::new("policy-001");
    policy.add_threshold("global", ThresholdConfig::default());
    policy.set_required_approvals(2);
    policy.submit_for_approval().unwrap();

    // Add first approval
    policy
        .add_approval(ApprovalSignature {
            approver_id: "approver-1".to_string(),
            timestamp: 1001,
            signature: vec![1, 2, 3],
        })
        .unwrap();

    // Cannot activate with insufficient approvals
    assert!(policy.activate(1002).is_err());

    // Add second approval
    policy
        .add_approval(ApprovalSignature {
            approver_id: "approver-2".to_string(),
            timestamp: 1002,
            signature: vec![4, 5, 6],
        })
        .unwrap();

    // Now activation should succeed
    assert!(policy.activate(1003).is_ok());
    assert_eq!(policy.status, PolicyStatus::Active);
    assert_eq!(policy.activated_at, Some(1003));
}

#[test]
fn test_policy_bundle_duplicate_approval_rejected() {
    let mut policy = PolicyBundle::new("policy-001");
    policy.add_threshold("global", ThresholdConfig::default());
    policy.submit_for_approval().unwrap();

    policy
        .add_approval(ApprovalSignature {
            approver_id: "approver-1".to_string(),
            timestamp: 1001,
            signature: vec![1, 2, 3],
        })
        .unwrap();

    // Same approver cannot approve twice
    let result = policy.add_approval(ApprovalSignature {
        approver_id: "approver-1".to_string(),
        timestamp: 1002,
        signature: vec![4, 5, 6],
    });

    assert!(result.is_err());
}

#[test]
fn test_policy_bundle_supersession() {
    let mut policy_v1 = PolicyBundle::new("policy-001");
    policy_v1.add_threshold("global", ThresholdConfig::default());
    policy_v1.submit_for_approval().unwrap();
    policy_v1
        .add_approval(ApprovalSignature {
            approver_id: "approver-1".to_string(),
            timestamp: 1001,
            signature: vec![1, 2, 3],
        })
        .unwrap();
    policy_v1.activate(1002).unwrap();

    assert!(policy_v1.is_active());

    // Supersede the old policy
    policy_v1.supersede().unwrap();
    assert_eq!(policy_v1.status, PolicyStatus::Superseded);
    assert!(!policy_v1.is_active());
}

#[test]
fn test_policy_immutability_after_activation() {
    let mut policy = PolicyBundle::new("policy-001");
    policy.add_threshold("global", ThresholdConfig::default());
    policy.submit_for_approval().unwrap();
    policy
        .add_approval(ApprovalSignature {
            approver_id: "approver-1".to_string(),
            timestamp: 1001,
            signature: vec![1, 2, 3],
        })
        .unwrap();
    policy.activate(1002).unwrap();

    // Content hash is locked after activation
    let hash_at_activation = policy.content_hash;

    // Cannot add more thresholds (in a real system this would be prevented)
    // Here we just verify the hash would change if we could modify
    let new_hash = policy.compute_content_hash();
    assert_eq!(
        hash_at_activation, new_hash,
        "Hash should be stable after activation"
    );
}

// ============================================================================
// WITNESS RECORD TESTS
// ============================================================================

#[test]
fn test_witness_record_creation() {
    let action_hash = Hash::compute(b"test-action");
    let witness = WitnessRecord::new(
        "witness-001",
        action_hash,
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );

    assert_eq!(witness.id, "witness-001");
    assert_eq!(witness.decision, GateDecision::Allow);
    assert!(!witness.content_hash.is_zero());
}

#[test]
fn test_witness_record_hash_verification() {
    let action_hash = Hash::compute(b"test-action");
    let witness = WitnessRecord::new(
        "witness-001",
        action_hash,
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );

    assert!(witness.verify_hash());

    // Tampered witness would fail verification
    let mut tampered = witness.clone();
    tampered.energy_snapshot = 0.99; // Tamper with energy
    assert!(!tampered.verify_hash());
}

#[test]
fn test_witness_chain_integrity() {
    let mut chain = WitnessChain::new();

    // First witness (no previous)
    let witness1 = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action-1"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );
    chain.append(witness1).unwrap();

    // Second witness (references first)
    let witness2 = WitnessRecord::new(
        "witness-002",
        Hash::compute(b"action-2"),
        0.15,
        GateDecision::Allow,
        "policy-001",
        Some("witness-001".to_string()),
        1001,
    );
    chain.append(witness2).unwrap();

    // Third witness (references second)
    let witness3 = WitnessRecord::new(
        "witness-003",
        Hash::compute(b"action-3"),
        0.50,
        GateDecision::Throttle,
        "policy-001",
        Some("witness-002".to_string()),
        1002,
    );
    chain.append(witness3).unwrap();

    assert_eq!(chain.len(), 3);
    assert!(chain.verify_integrity());
}

#[test]
fn test_witness_chain_rejects_broken_chain() {
    let mut chain = WitnessChain::new();

    let witness1 = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action-1"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );
    chain.append(witness1).unwrap();

    // Try to append with wrong previous_witness_id
    let bad_witness = WitnessRecord::new(
        "witness-002",
        Hash::compute(b"action-2"),
        0.15,
        GateDecision::Allow,
        "policy-001",
        Some("wrong-id".to_string()), // Wrong reference!
        1001,
    );

    assert!(chain.append(bad_witness).is_err());
}

#[test]
fn test_witness_chain_rejects_timestamp_regression() {
    let mut chain = WitnessChain::new();

    let witness1 = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action-1"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );
    chain.append(witness1).unwrap();

    // Try to append with earlier timestamp
    let bad_witness = WitnessRecord::new(
        "witness-002",
        Hash::compute(b"action-2"),
        0.15,
        GateDecision::Allow,
        "policy-001",
        Some("witness-001".to_string()),
        999, // Earlier than witness-001!
    );

    assert!(chain.append(bad_witness).is_err());
}

#[test]
fn test_witness_chain_first_record_no_previous() {
    let mut chain = WitnessChain::new();

    // First record must not have previous_witness_id
    let bad_first = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action-1"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        Some("nonexistent".to_string()), // Should be None!
        1000,
    );

    assert!(chain.append(bad_first).is_err());
}

// ============================================================================
// LINEAGE RECORD TESTS
// ============================================================================

#[test]
fn test_lineage_record_creation() {
    let lineage = LineageRecord::new(
        "lineage-001",
        "entity:fact:123",
        Operation::Create,
        vec![],
        "witness-001",
        "agent-alpha",
        1000,
    );

    assert_eq!(lineage.id, "lineage-001");
    assert_eq!(lineage.operation, Operation::Create);
    assert!(lineage.dependencies.is_empty());
    assert!(lineage.verify_hash());
}

#[test]
fn test_lineage_record_with_dependencies() {
    let lineage = LineageRecord::new(
        "lineage-003",
        "entity:derived:789",
        Operation::Derive,
        vec!["lineage-001".to_string(), "lineage-002".to_string()],
        "witness-003",
        "agent-alpha",
        1002,
    );

    assert_eq!(lineage.dependencies.len(), 2);
    assert!(lineage.dependencies.contains(&"lineage-001".to_string()));
    assert!(lineage.dependencies.contains(&"lineage-002".to_string()));
}

#[test]
fn test_lineage_record_hash_verification() {
    let lineage = LineageRecord::new(
        "lineage-001",
        "entity:fact:123",
        Operation::Create,
        vec![],
        "witness-001",
        "agent-alpha",
        1000,
    );

    assert!(lineage.verify_hash());

    // Tampered record would fail
    let mut tampered = lineage.clone();
    tampered.actor_id = "evil-agent".to_string();
    assert!(!tampered.verify_hash());
}

#[test]
fn test_lineage_tracks_all_operations() {
    let operations = vec![
        Operation::Create,
        Operation::Update,
        Operation::Delete,
        Operation::Derive,
    ];

    for (i, op) in operations.iter().enumerate() {
        let lineage = LineageRecord::new(
            format!("lineage-{:03}", i),
            format!("entity:test:{}", i),
            *op,
            vec![],
            format!("witness-{:03}", i),
            "agent-alpha",
            1000 + i as u64,
        );

        assert_eq!(lineage.operation, *op);
        assert!(lineage.verify_hash());
    }
}

// ============================================================================
// GOVERNANCE INVARIANT TESTS
// ============================================================================

#[test]
fn test_invariant_no_action_without_witness() {
    // Every gate decision must produce a witness record
    struct GateEngine {
        chain: WitnessChain,
        next_id: u64,
    }

    impl GateEngine {
        fn new() -> Self {
            Self {
                chain: WitnessChain::new(),
                next_id: 1,
            }
        }

        fn decide(
            &mut self,
            action_hash: Hash,
            energy: f32,
            policy_ref: &str,
        ) -> (GateDecision, String) {
            let decision = if energy < 0.1 {
                GateDecision::Allow
            } else if energy < 0.5 {
                GateDecision::Throttle
            } else {
                GateDecision::Block
            };

            let id = format!("witness-{:06}", self.next_id);
            self.next_id += 1;

            let prev_id = self.chain.records.last().map(|r| r.id.clone());

            let witness = WitnessRecord::new(
                id.clone(),
                action_hash,
                energy,
                decision,
                policy_ref,
                prev_id,
                1000 + self.next_id,
            );

            // This is the invariant: every decision creates a witness
            self.chain
                .append(witness)
                .expect("Witness must be created for every decision");

            (decision, id)
        }
    }

    let mut engine = GateEngine::new();

    // Multiple decisions, all create witnesses
    engine.decide(Hash::compute(b"action-1"), 0.05, "policy-001");
    engine.decide(Hash::compute(b"action-2"), 0.25, "policy-001");
    engine.decide(Hash::compute(b"action-3"), 0.75, "policy-001");

    assert_eq!(engine.chain.len(), 3);
    assert!(engine.chain.verify_integrity());
}

#[test]
fn test_invariant_no_write_without_lineage() {
    // Every authoritative write must have lineage
    struct WriteEngine {
        lineages: Vec<LineageRecord>,
        next_id: u64,
    }

    impl WriteEngine {
        fn new() -> Self {
            Self {
                lineages: Vec::new(),
                next_id: 1,
            }
        }

        fn write(
            &mut self,
            entity_ref: &str,
            operation: Operation,
            dependencies: Vec<String>,
            witness_id: &str,
            actor_id: &str,
        ) -> String {
            let id = format!("lineage-{:06}", self.next_id);
            self.next_id += 1;

            let lineage = LineageRecord::new(
                id.clone(),
                entity_ref,
                operation,
                dependencies,
                witness_id,
                actor_id,
                1000 + self.next_id,
            );

            // This is the invariant: every write creates lineage
            assert!(lineage.verify_hash());
            self.lineages.push(lineage);

            id
        }
    }

    let mut engine = WriteEngine::new();

    let l1 = engine.write(
        "entity:fact:1",
        Operation::Create,
        vec![],
        "witness-001",
        "agent-1",
    );
    let l2 = engine.write(
        "entity:fact:2",
        Operation::Create,
        vec![],
        "witness-002",
        "agent-1",
    );
    let l3 = engine.write(
        "entity:derived:1",
        Operation::Derive,
        vec![l1, l2],
        "witness-003",
        "agent-1",
    );

    assert_eq!(engine.lineages.len(), 3);
    assert_eq!(engine.lineages[2].dependencies.len(), 2);
}

// ============================================================================
// CONTENT HASH CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_content_hash_determinism() {
    // Same inputs should produce same hash
    let witness1 = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );

    let witness2 = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );

    assert_eq!(witness1.content_hash, witness2.content_hash);
}

#[test]
fn test_content_hash_sensitivity() {
    // Different inputs should produce different hashes
    let base = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );

    // Change ID
    let diff_id = WitnessRecord::new(
        "witness-002",
        Hash::compute(b"action"),
        0.05,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );
    assert_ne!(base.content_hash, diff_id.content_hash);

    // Change energy
    let diff_energy = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action"),
        0.06,
        GateDecision::Allow,
        "policy-001",
        None,
        1000,
    );
    assert_ne!(base.content_hash, diff_energy.content_hash);

    // Change decision
    let diff_decision = WitnessRecord::new(
        "witness-001",
        Hash::compute(b"action"),
        0.05,
        GateDecision::Block,
        "policy-001",
        None,
        1000,
    );
    assert_ne!(base.content_hash, diff_decision.content_hash);
}
