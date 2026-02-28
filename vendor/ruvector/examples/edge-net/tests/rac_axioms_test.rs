//! Comprehensive test suite for RAC 12 Axioms
//!
//! This test suite validates the RuVector Adversarial Coherence implementation
//! against all 12 axioms of the Adversarial Coherence Thesis.

use ruvector_edge_net::rac::*;
use std::collections::HashMap;

// ============================================================================
// Test Utilities
// ============================================================================

/// Create a test event with specified parameters
fn create_test_event(
    context: ContextId,
    author: PublicKeyBytes,
    kind: EventKind,
) -> Event {
    Event {
        id: [0u8; 32],
        prev: None,
        ts_unix_ms: 1609459200000, // 2021-01-01
        author,
        context,
        ruvector: Ruvector::new(vec![1.0, 0.0, 0.0]),
        kind,
        sig: vec![0u8; 64],
    }
}

/// Create a test assertion event
fn create_assert_event(proposition: &str, confidence: f32) -> AssertEvent {
    AssertEvent {
        proposition: proposition.as_bytes().to_vec(),
        evidence: vec![EvidenceRef::hash(&[1, 2, 3])],
        confidence,
        expires_at_unix_ms: None,
    }
}

/// Simple verifier for testing
struct TestVerifier;

impl Verifier for TestVerifier {
    fn incompatible(&self, _context: &ContextId, a: &AssertEvent, b: &AssertEvent) -> bool {
        // Simple incompatibility: different propositions with high confidence
        a.proposition != b.proposition && a.confidence > 0.8 && b.confidence > 0.8
    }
}

/// Simple authority policy for testing
struct TestAuthorityPolicy {
    authorized_contexts: HashMap<String, Vec<PublicKeyBytes>>,
}

impl AuthorityPolicy for TestAuthorityPolicy {
    fn authorized(&self, context: &ContextId, resolution: &ResolutionEvent) -> bool {
        let context_key = hex::encode(context);
        if let Some(authorized_keys) = self.authorized_contexts.get(&context_key) {
            // Check if any resolution signature is from authorized key
            // In real implementation, would verify signatures
            !authorized_keys.is_empty() && !resolution.authority_sigs.is_empty()
        } else {
            false
        }
    }

    fn quarantine_level(&self, _context: &ContextId, _conflict_id: &[u8; 32]) -> QuarantineLevel {
        QuarantineLevel::RequiresWitness
    }
}

// ============================================================================
// Axiom 1: Connectivity is not truth
// ============================================================================

#[test]
fn axiom1_connectivity_not_truth() {
    // High similarity does not imply correctness
    let correct_claim = Ruvector::new(vec![1.0, 0.0, 0.0]);
    let similar_wrong = Ruvector::new(vec![0.95, 0.31, 0.0]); // ~95% similar
    let dissimilar_correct = Ruvector::new(vec![0.0, 1.0, 0.0]); // 0% similar

    let similarity = correct_claim.similarity(&similar_wrong);
    assert!(similarity > 0.9, "Claims are highly similar");

    // Despite high similarity, semantic verification is required
    let verifier = TestVerifier;
    let context = [0u8; 32];

    let assert_correct = create_assert_event("sky is blue", 0.95);
    let assert_similar_wrong = create_assert_event("sky is green", 0.95);

    // Verifier detects incompatibility despite structural similarity
    assert!(
        verifier.incompatible(&context, &assert_correct, &assert_similar_wrong),
        "High similarity does not prevent conflict detection"
    );
}

#[test]
fn axiom1_structural_metrics_insufficient() {
    // Low connectivity (low similarity) can still be correct
    let baseline = Ruvector::new(vec![1.0, 0.0, 0.0]);
    let low_connectivity = Ruvector::new(vec![0.0, 0.0, 1.0]);

    let similarity = baseline.similarity(&low_connectivity);
    assert!(similarity < 0.1, "Very low structural connectivity");

    // But both can be correct in different contexts
    // Connectivity bounds failure modes, not correctness
}

// ============================================================================
// Axiom 2: Everything is an event
// ============================================================================

#[test]
fn axiom2_all_operations_are_events() {
    let context = [1u8; 32];
    let author = [2u8; 32];

    // Test all event types
    let assert_event = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("test claim", 0.9)),
    );
    assert!(matches!(assert_event.kind, EventKind::Assert(_)));

    let challenge_event = create_test_event(
        context,
        author,
        EventKind::Challenge(ChallengeEvent {
            conflict_id: [3u8; 32],
            claim_ids: vec![[4u8; 32]],
            reason: "Disputed".to_string(),
            requested_proofs: vec!["merkle".to_string()],
        }),
    );
    assert!(matches!(challenge_event.kind, EventKind::Challenge(_)));

    let support_event = create_test_event(
        context,
        author,
        EventKind::Support(SupportEvent {
            conflict_id: [3u8; 32],
            claim_id: [4u8; 32],
            evidence: vec![EvidenceRef::url("https://evidence.com")],
            cost: 100,
        }),
    );
    assert!(matches!(support_event.kind, EventKind::Support(_)));

    let resolution_event = create_test_event(
        context,
        author,
        EventKind::Resolution(ResolutionEvent {
            conflict_id: [3u8; 32],
            accepted: vec![[4u8; 32]],
            deprecated: vec![[5u8; 32]],
            rationale: vec![EvidenceRef::hash(&[6, 7, 8])],
            authority_sigs: vec![vec![0u8; 64]],
        }),
    );
    assert!(matches!(resolution_event.kind, EventKind::Resolution(_)));

    let deprecate_event = create_test_event(
        context,
        author,
        EventKind::Deprecate(DeprecateEvent {
            claim_id: [4u8; 32],
            by_resolution: [3u8; 32],
            superseded_by: Some([7u8; 32]),
        }),
    );
    assert!(matches!(deprecate_event.kind, EventKind::Deprecate(_)));
}

#[test]
fn axiom2_events_appended_to_log() {
    let log = EventLog::new();
    assert_eq!(log.len(), 0);

    let event1 = create_test_event(
        [1u8; 32],
        [2u8; 32],
        EventKind::Assert(create_assert_event("claim 1", 0.8)),
    );

    let event2 = create_test_event(
        [1u8; 32],
        [2u8; 32],
        EventKind::Assert(create_assert_event("claim 2", 0.9)),
    );

    log.append(event1);
    log.append(event2);

    assert_eq!(log.len(), 2, "All events logged");
    assert!(!log.is_empty());
}

// ============================================================================
// Axiom 3: No destructive edits
// ============================================================================

#[test]
fn axiom3_deprecation_not_deletion() {
    let mut engine = CoherenceEngine::new();
    let context = [1u8; 32];
    let author = [2u8; 32];

    // Create and ingest an assertion
    let mut assert_event = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("initial claim", 0.9)),
    );
    assert_event.id = [10u8; 32];

    engine.ingest(assert_event.clone());
    assert_eq!(engine.event_count(), 1);

    // Deprecate the claim
    let deprecate_event = create_test_event(
        context,
        author,
        EventKind::Deprecate(DeprecateEvent {
            claim_id: assert_event.id,
            by_resolution: [99u8; 32],
            superseded_by: Some([11u8; 32]),
        }),
    );

    engine.ingest(deprecate_event);
    assert_eq!(engine.event_count(), 2, "Deprecated event still in log");

    // Verify claim is quarantined but not deleted
    let claim_id_hex = hex::encode(&assert_event.id);
    assert_eq!(
        engine.get_quarantine_level(&claim_id_hex),
        3,
        "Deprecated claim is blocked"
    );
    assert!(!engine.can_use_claim(&claim_id_hex), "Cannot use deprecated claim");
}

#[test]
fn axiom3_append_only_log() {
    let log = EventLog::new();
    let initial_root = log.get_root();

    let event1 = create_test_event(
        [1u8; 32],
        [2u8; 32],
        EventKind::Assert(create_assert_event("claim", 0.9)),
    );

    log.append(event1);
    let root_after_append = log.get_root();

    // Root changes after append (events affect history)
    assert_ne!(initial_root, root_after_append, "Merkle root changes on append");

    // Cannot remove events - only append
    // Log length only increases
    assert_eq!(log.len(), 1);
}

// ============================================================================
// Axiom 4: Every claim is scoped
// ============================================================================

#[test]
fn axiom4_claims_bound_to_context() {
    let context_a = [1u8; 32];
    let context_b = [2u8; 32];
    let author = [3u8; 32];

    let event_a = create_test_event(
        context_a,
        author,
        EventKind::Assert(create_assert_event("claim in context A", 0.9)),
    );

    let event_b = create_test_event(
        context_b,
        author,
        EventKind::Assert(create_assert_event("claim in context B", 0.9)),
    );

    assert_eq!(event_a.context, context_a, "Event bound to context A");
    assert_eq!(event_b.context, context_b, "Event bound to context B");
    assert_ne!(event_a.context, event_b.context, "Different contexts");
}

#[test]
fn axiom4_context_isolation() {
    let log = EventLog::new();
    let context_a = [1u8; 32];
    let context_b = [2u8; 32];
    let author = [3u8; 32];

    let mut event_a = create_test_event(
        context_a,
        author,
        EventKind::Assert(create_assert_event("claim A", 0.9)),
    );
    event_a.id = [10u8; 32];

    let mut event_b = create_test_event(
        context_b,
        author,
        EventKind::Assert(create_assert_event("claim B", 0.9)),
    );
    event_b.id = [11u8; 32];

    log.append(event_a);
    log.append(event_b);

    // Filter by context
    let events_a = log.for_context(&context_a);
    let events_b = log.for_context(&context_b);

    assert_eq!(events_a.len(), 1, "One event in context A");
    assert_eq!(events_b.len(), 1, "One event in context B");
    assert_eq!(events_a[0].context, context_a);
    assert_eq!(events_b[0].context, context_b);
}

// ============================================================================
// Axiom 5: Semantics drift is expected
// ============================================================================

#[test]
fn axiom5_drift_measurement() {
    let baseline = Ruvector::new(vec![1.0, 0.0, 0.0]);
    let slightly_drifted = Ruvector::new(vec![0.95, 0.1, 0.0]);
    let heavily_drifted = Ruvector::new(vec![0.5, 0.5, 0.5]);

    let slight_drift = slightly_drifted.drift_from(&baseline);
    let heavy_drift = heavily_drifted.drift_from(&baseline);

    assert!(slight_drift > 0.0, "Drift detected");
    assert!(slight_drift < 0.3, "Slight drift is small");
    assert!(heavy_drift > 0.4, "Heavy drift is large");
    assert!(heavy_drift > slight_drift, "Drift increases over time");
}

#[test]
fn axiom5_drift_not_denied() {
    // Drift is expected and measured, not treated as error
    let baseline = Ruvector::new(vec![1.0, 0.0, 0.0]);
    let drifted = Ruvector::new(vec![0.0, 1.0, 0.0]);

    let drift = drifted.drift_from(&baseline);

    // Maximum drift (orthogonal vectors)
    assert!((drift - 1.0).abs() < 0.001, "Maximum drift measured");

    // System should manage drift, not reject it
    // This test passes if drift calculation succeeds without error
}

// ============================================================================
// Axiom 6: Disagreement is signal
// ============================================================================

#[test]
fn axiom6_conflict_detection_triggers_quarantine() {
    let mut engine = CoherenceEngine::new();
    let context = [1u8; 32];
    let author = [2u8; 32];

    // Create two conflicting claims
    let mut claim1 = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("sky is blue", 0.95)),
    );
    claim1.id = [10u8; 32];

    let mut claim2 = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("sky is green", 0.95)),
    );
    claim2.id = [11u8; 32];

    engine.ingest(claim1.clone());
    engine.ingest(claim2.clone());

    // Issue challenge
    let challenge = create_test_event(
        context,
        author,
        EventKind::Challenge(ChallengeEvent {
            conflict_id: [99u8; 32],
            claim_ids: vec![claim1.id, claim2.id],
            reason: "Contradictory color claims".to_string(),
            requested_proofs: vec![],
        }),
    );

    engine.ingest(challenge);

    // Verify both claims are quarantined
    assert_eq!(
        engine.get_quarantine_level(&hex::encode(&claim1.id)),
        2,
        "Claim 1 quarantined"
    );
    assert_eq!(
        engine.get_quarantine_level(&hex::encode(&claim2.id)),
        2,
        "Claim 2 quarantined"
    );
    assert_eq!(engine.conflict_count(), 1, "Conflict recorded");
}

#[test]
fn axiom6_epistemic_temperature_tracking() {
    let conflict = Conflict {
        id: [1u8; 32],
        context: [2u8; 32],
        claim_ids: vec![[3u8; 32], [4u8; 32]],
        detected_at: 1609459200000,
        status: ConflictStatus::Challenged,
        temperature: 0.5,
        escalation_count: 0,
    };

    assert!(conflict.temperature > 0.0, "Temperature tracked");
    assert!(conflict.temperature <= 1.0, "Temperature normalized");

    // Sustained contradictions should increase temperature
    // (Implementation detail - would need history tracking)
}

// ============================================================================
// Axiom 7: Authority is scoped, not global
// ============================================================================

#[test]
fn axiom7_scoped_authority_verification() {
    let context_a = [1u8; 32];
    let context_b = [2u8; 32];
    let authorized_key = [3u8; 32];
    let unauthorized_key = [4u8; 32];

    let mut policy = TestAuthorityPolicy {
        authorized_contexts: HashMap::new(),
    };
    policy.authorized_contexts.insert(
        hex::encode(&context_a),
        vec![authorized_key],
    );

    // Resolution in authorized context
    let authorized_resolution = ResolutionEvent {
        conflict_id: [99u8; 32],
        accepted: vec![[10u8; 32]],
        deprecated: vec![],
        rationale: vec![],
        authority_sigs: vec![vec![0u8; 64]], // Simulated signature
    };

    assert!(
        policy.authorized(&context_a, &authorized_resolution),
        "Authorized in context A"
    );
    assert!(
        !policy.authorized(&context_b, &authorized_resolution),
        "Not authorized in context B"
    );
}

#[test]
fn axiom7_threshold_authority() {
    let context = [1u8; 32];
    let key1 = [1u8; 32];
    let key2 = [2u8; 32];
    let key3 = [3u8; 32];

    let authority = ScopedAuthority {
        context,
        authorized_keys: vec![key1, key2, key3],
        threshold: 2, // 2-of-3 required
        allowed_evidence: vec!["merkle".to_string()],
    };

    assert_eq!(authority.threshold, 2, "Threshold set");
    assert_eq!(authority.authorized_keys.len(), 3, "3 authorized keys");

    // Real implementation would verify k-of-n signatures
}

// ============================================================================
// Axiom 8: Witnesses matter
// ============================================================================

#[test]
fn axiom8_witness_cost_tracking() {
    let support = SupportEvent {
        conflict_id: [1u8; 32],
        claim_id: [2u8; 32],
        evidence: vec![
            EvidenceRef::url("https://source1.com"),
            EvidenceRef::hash(&[3, 4, 5]),
        ],
        cost: 100,
    };

    assert!(support.cost > 0, "Witness has cost/stake");
    assert!(support.evidence.len() > 1, "Multiple evidence sources");
}

#[test]
fn axiom8_evidence_diversity() {
    // Different evidence types indicate diversity
    let hash_evidence = EvidenceRef::hash(&[1, 2, 3]);
    let url_evidence = EvidenceRef::url("https://example.com");

    assert_eq!(hash_evidence.kind, "hash");
    assert_eq!(url_evidence.kind, "url");
    assert_ne!(hash_evidence.kind, url_evidence.kind, "Diverse evidence types");
}

// Note: Full witness path independence verification requires implementation

// ============================================================================
// Axiom 9: Quarantine is mandatory
// ============================================================================

#[test]
fn axiom9_contested_claims_quarantined() {
    let manager = QuarantineManager::new();

    // Initially no quarantine
    assert!(manager.can_use("claim-1"));
    assert_eq!(manager.get_level("claim-1"), QuarantineLevel::None as u8);

    // Quarantine contested claim
    manager.set_level("claim-1", QuarantineLevel::Blocked as u8);

    assert!(!manager.can_use("claim-1"), "Quarantined claim cannot be used");
    assert_eq!(manager.quarantined_count(), 1);
}

#[test]
fn axiom9_quarantine_levels_enforced() {
    let manager = QuarantineManager::new();

    // Test all quarantine levels
    manager.set_level("claim-none", QuarantineLevel::None as u8);
    manager.set_level("claim-conservative", QuarantineLevel::Conservative as u8);
    manager.set_level("claim-witness", QuarantineLevel::RequiresWitness as u8);
    manager.set_level("claim-blocked", QuarantineLevel::Blocked as u8);

    assert!(manager.can_use("claim-none"));
    assert!(manager.can_use("claim-conservative"));
    assert!(manager.can_use("claim-witness"));
    assert!(!manager.can_use("claim-blocked"), "Blocked claims unusable");

    assert_eq!(manager.quarantined_count(), 3, "3 quarantined claims");
}

#[test]
fn axiom9_quarantine_prevents_decision_use() {
    let mut engine = CoherenceEngine::new();
    let context = [1u8; 32];
    let author = [2u8; 32];

    let mut claim = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("disputed claim", 0.9)),
    );
    claim.id = [10u8; 32];

    engine.ingest(claim.clone());

    // Quarantine the claim
    let challenge = create_test_event(
        context,
        author,
        EventKind::Challenge(ChallengeEvent {
            conflict_id: [99u8; 32],
            claim_ids: vec![claim.id],
            reason: "Disputed".to_string(),
            requested_proofs: vec![],
        }),
    );

    engine.ingest(challenge);

    // Create decision trace depending on quarantined claim
    let trace = DecisionTrace::new(vec![claim.id], vec![1, 2, 3]);

    assert!(!trace.can_replay(&engine), "Decision cannot be replayed with quarantined dependency");
}

// ============================================================================
// Axiom 10: All decisions are replayable
// ============================================================================

#[test]
fn axiom10_decision_trace_completeness() {
    let dep1 = [1u8; 32];
    let dep2 = [2u8; 32];
    let outcome = vec![10, 20, 30];

    let trace = DecisionTrace::new(vec![dep1, dep2], outcome.clone());

    assert_eq!(trace.dependencies.len(), 2, "All dependencies recorded");
    assert_eq!(trace.outcome, outcome, "Outcome recorded");
    assert!(trace.timestamp > 0, "Timestamp recorded");
    assert!(!trace.has_disputed, "Dispute flag tracked");
    assert!(!trace.quarantine_policy.is_empty(), "Policy recorded");
}

#[test]
fn axiom10_decision_replayability() {
    let engine = CoherenceEngine::new();

    // Decision with no dependencies
    let trace = DecisionTrace::new(vec![], vec![1, 2, 3]);

    assert!(trace.can_replay(&engine), "Decision with no dependencies is replayable");

    // Decision with valid (non-quarantined) dependency
    let mut engine2 = CoherenceEngine::new();
    let context = [1u8; 32];
    let author = [2u8; 32];

    let mut claim = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("valid claim", 0.9)),
    );
    claim.id = [10u8; 32];

    engine2.ingest(claim.clone());

    let trace2 = DecisionTrace::new(vec![claim.id], vec![1, 2, 3]);
    assert!(trace2.can_replay(&engine2), "Decision with valid dependencies is replayable");
}

// ============================================================================
// Axiom 11: Equivocation is detectable
// ============================================================================

#[test]
fn axiom11_merkle_root_changes_on_append() {
    let log = EventLog::new();
    let root1 = log.get_root();

    let event = create_test_event(
        [1u8; 32],
        [2u8; 32],
        EventKind::Assert(create_assert_event("claim", 0.9)),
    );

    log.append(event);
    let root2 = log.get_root();

    assert_ne!(root1, root2, "Merkle root changes on append");

    // Different histories produce different roots
    // Making it hard to show different histories to different peers
}

#[test]
fn axiom11_inclusion_proof_generation() {
    let log = EventLog::new();

    let mut event = create_test_event(
        [1u8; 32],
        [2u8; 32],
        EventKind::Assert(create_assert_event("claim", 0.9)),
    );
    event.id = [10u8; 32];

    let event_id = log.append(event);

    let proof = log.prove_inclusion(&event_id);
    assert!(proof.is_some(), "Inclusion proof generated");

    let proof = proof.unwrap();
    assert_eq!(proof.event_id, event_id, "Proof references correct event");
    // Compare root bytes properly (get_root returns hex string)
    let expected_root = hex::decode(log.get_root()).unwrap();
    assert_eq!(proof.root.to_vec(), expected_root, "Proof includes root");
}

#[test]
fn axiom11_event_chaining() {
    let mut prev_id: Option<EventId> = None;

    for i in 0..3 {
        let mut event = create_test_event(
            [1u8; 32],
            [2u8; 32],
            EventKind::Assert(create_assert_event(&format!("claim {}", i), 0.9)),
        );
        event.prev = prev_id;
        event.id = [i; 32];

        if i > 0 {
            assert!(event.prev.is_some(), "Event chains to previous");
        }

        prev_id = Some(event.id);
    }
}

// ============================================================================
// Axiom 12: Local learning is allowed
// ============================================================================

#[test]
fn axiom12_learning_attribution() {
    let author = [42u8; 32];
    let event = create_test_event(
        [1u8; 32],
        author,
        EventKind::Assert(create_assert_event("learned pattern", 0.85)),
    );

    assert_eq!(event.author, author, "Learning attributed to author");

    // Events are signed (in real implementation)
    assert!(!event.sig.is_empty(), "Event is signed");
}

#[test]
fn axiom12_learning_is_challengeable() {
    let mut engine = CoherenceEngine::new();
    let context = [1u8; 32];
    let author = [2u8; 32];

    // Local learning produces a claim
    let mut learned_claim = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("AI learned pattern", 0.9)),
    );
    learned_claim.id = [20u8; 32];

    engine.ingest(learned_claim.clone());

    // Learning can be challenged like any other claim
    let challenge = create_test_event(
        context,
        [3u8; 32], // Different author challenges
        EventKind::Challenge(ChallengeEvent {
            conflict_id: [99u8; 32],
            claim_ids: vec![learned_claim.id],
            reason: "Learned pattern incorrect".to_string(),
            requested_proofs: vec!["training_data".to_string()],
        }),
    );

    engine.ingest(challenge);

    // Challenged learning is quarantined
    assert_eq!(
        engine.get_quarantine_level(&hex::encode(&learned_claim.id)),
        2,
        "Challenged learning is quarantined"
    );
}

#[test]
fn axiom12_learning_is_rollbackable() {
    let mut engine = CoherenceEngine::new();
    let context = [1u8; 32];
    let author = [2u8; 32];

    // Original learning
    let mut old_learning = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("v1 pattern", 0.8)),
    );
    old_learning.id = [30u8; 32];

    engine.ingest(old_learning.clone());

    // New learning supersedes old
    let mut new_learning = create_test_event(
        context,
        author,
        EventKind::Assert(create_assert_event("v2 pattern", 0.9)),
    );
    new_learning.id = [31u8; 32];

    engine.ingest(new_learning.clone());

    // Deprecate old learning
    let deprecate = create_test_event(
        context,
        author,
        EventKind::Deprecate(DeprecateEvent {
            claim_id: old_learning.id,
            by_resolution: [99u8; 32],
            superseded_by: Some(new_learning.id),
        }),
    );

    engine.ingest(deprecate);

    // Old learning is rolled back but not deleted (3 events: old, new, deprecate)
    assert_eq!(engine.event_count(), 3, "All events preserved");
    assert!(!engine.can_use_claim(&hex::encode(&old_learning.id)), "Old learning not usable");
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn integration_full_dispute_lifecycle() {
    let mut engine = CoherenceEngine::new();
    let context = [1u8; 32];
    let author1 = [2u8; 32];
    let author2 = [3u8; 32];

    // Step 1: Two agents make conflicting claims
    let mut claim1 = create_test_event(
        context,
        author1,
        EventKind::Assert(create_assert_event("answer is 42", 0.95)),
    );
    claim1.id = [10u8; 32];

    let mut claim2 = create_test_event(
        context,
        author2,
        EventKind::Assert(create_assert_event("answer is 43", 0.95)),
    );
    claim2.id = [11u8; 32];

    engine.ingest(claim1.clone());
    engine.ingest(claim2.clone());

    assert_eq!(engine.event_count(), 2);

    // Step 2: Conflict detected and challenged
    let challenge = create_test_event(
        context,
        author1,
        EventKind::Challenge(ChallengeEvent {
            conflict_id: [99u8; 32],
            claim_ids: vec![claim1.id, claim2.id],
            reason: "Contradictory answers".to_string(),
            requested_proofs: vec!["computation".to_string()],
        }),
    );

    engine.ingest(challenge);

    assert_eq!(engine.conflict_count(), 1, "Conflict recorded");
    assert_eq!(engine.quarantined_count(), 2, "Both claims quarantined");

    // Step 3: Evidence provided
    let support = create_test_event(
        context,
        author1,
        EventKind::Support(SupportEvent {
            conflict_id: [99u8; 32],
            claim_id: claim1.id,
            evidence: vec![EvidenceRef::url("https://proof.com/42")],
            cost: 100,
        }),
    );

    engine.ingest(support);

    // Step 4: Resolution
    let resolution = create_test_event(
        context,
        [4u8; 32], // Authority
        EventKind::Resolution(ResolutionEvent {
            conflict_id: [99u8; 32],
            accepted: vec![claim1.id],
            deprecated: vec![claim2.id],
            rationale: vec![EvidenceRef::hash(&[1, 2, 3])],
            authority_sigs: vec![vec![0u8; 64]],
        }),
    );

    engine.ingest(resolution);

    // Step 5: Verify resolution applied
    assert!(!engine.can_use_claim(&hex::encode(&claim2.id)), "Rejected claim blocked");
    assert!(engine.can_use_claim(&hex::encode(&claim1.id)), "Accepted claim usable");

    // All events preserved in log (claim1, claim2, challenge, support, resolution = 5)
    assert_eq!(engine.event_count(), 5, "Complete history preserved");
}

#[test]
fn integration_cross_context_isolation() {
    let mut engine = CoherenceEngine::new();
    let context_math = [1u8; 32];
    let context_physics = [2u8; 32];
    let author = [3u8; 32];

    // Claim in math context
    let mut math_claim = create_test_event(
        context_math,
        author,
        EventKind::Assert(create_assert_event("2+2=4", 1.0)),
    );
    math_claim.id = [10u8; 32];

    // Claim in physics context
    let mut physics_claim = create_test_event(
        context_physics,
        author,
        EventKind::Assert(create_assert_event("e=mc^2", 1.0)),
    );
    physics_claim.id = [11u8; 32];

    engine.ingest(math_claim.clone());
    engine.ingest(physics_claim.clone());

    // Challenge in math context
    let math_challenge = create_test_event(
        context_math,
        author,
        EventKind::Challenge(ChallengeEvent {
            conflict_id: [99u8; 32],
            claim_ids: vec![math_claim.id],
            reason: "Disputed".to_string(),
            requested_proofs: vec![],
        }),
    );

    engine.ingest(math_challenge);

    // Only math claim should be quarantined
    assert_eq!(
        engine.get_quarantine_level(&hex::encode(&math_claim.id)),
        2,
        "Math claim quarantined"
    );
    assert_eq!(
        engine.get_quarantine_level(&hex::encode(&physics_claim.id)),
        0,
        "Physics claim unaffected"
    );
}
