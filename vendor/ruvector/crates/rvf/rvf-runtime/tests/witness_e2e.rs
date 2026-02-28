//! End-to-end integration tests for ADR-035 capability reports.
//!
//! Tests the full witness → scorecard → governance pipeline with real
//! HMAC-SHA256 signatures, policy enforcement, and deterministic replay.

use rvf_runtime::seed_crypto;
use rvf_runtime::witness::{
    GovernancePolicy, ParsedWitness, ScorecardBuilder, WitnessBuilder, WitnessError,
};
use rvf_types::witness::*;

const KEY: &[u8] = b"e2e-test-key-for-witness-bundle!";

fn make_entry(tool: &str, latency: u32, cost: u32, tokens: u32) -> ToolCallEntry {
    ToolCallEntry {
        action: tool.as_bytes().to_vec(),
        args_hash: seed_crypto::seed_content_hash(tool.as_bytes()),
        result_hash: [0x00; 8],
        latency_ms: latency,
        cost_microdollars: cost,
        tokens,
        policy_check: PolicyCheck::Allowed,
    }
}

#[test]
fn full_capability_report_pipeline() {
    let mut scorecard = ScorecardBuilder::new();
    let policy = GovernancePolicy::autonomous();

    // Simulate 5 tasks.
    let tasks: Vec<([u8; 16], TaskOutcome, bool)> = vec![
        ([0x01; 16], TaskOutcome::Solved, true),
        ([0x02; 16], TaskOutcome::Solved, true),
        ([0x03; 16], TaskOutcome::Solved, false), // solved but no full evidence
        ([0x04; 16], TaskOutcome::Failed, true),
        ([0x05; 16], TaskOutcome::Errored, false),
    ];

    for (task_id, outcome, full_evidence) in &tasks {
        let mut builder = WitnessBuilder::new(*task_id, policy.clone())
            .with_spec(b"fix issue #123")
            .with_outcome(*outcome);

        if *full_evidence {
            builder = builder
                .with_plan(b"1. read\n2. fix\n3. test")
                .with_diff(b"--- a/file.rs\n+++ b/file.rs")
                .with_test_log(b"test ... ok");
        }

        builder.record_tool_call(make_entry("Read", 50, 100, 500));
        builder.record_tool_call(make_entry("Edit", 100, 200, 1000));
        builder.record_tool_call(make_entry("Bash", 3000, 0, 0));

        let (payload, header) = builder.build_and_sign(KEY).unwrap();

        // Verify signature.
        let parsed = ParsedWitness::parse(&payload).unwrap();
        parsed.verify_all(KEY, &payload).unwrap();

        assert_eq!(header.tool_call_count, 3);
        assert_eq!(header.total_cost_microdollars, 300);

        scorecard.add_witness(&parsed, 0, 0);
    }

    let card = scorecard.finish();
    assert_eq!(card.total_tasks, 5);
    assert_eq!(card.solved, 3);
    assert_eq!(card.failed, 1);
    assert_eq!(card.errors, 1);
    assert!((card.solve_rate - 0.6).abs() < 0.01);
    // 2 out of 3 solved have full evidence.
    assert!((card.evidence_coverage - 0.6667).abs() < 0.01);
    // Total cost = 5 tasks * 300 each = 1500. Solved = 3. 1500/3 = 500.
    assert_eq!(card.cost_per_solve_microdollars, 500);
}

#[test]
fn governance_restricted_mode_blocks_writes() {
    let policy = GovernancePolicy::restricted();
    let mut builder = WitnessBuilder::new([0x10; 16], policy)
        .with_spec(b"audit code")
        .with_outcome(TaskOutcome::Solved);

    // Read is allowed.
    let check = builder.record_tool_call(make_entry("Read", 50, 100, 500));
    assert_eq!(check, PolicyCheck::Allowed);

    // Write is denied.
    let check = builder.record_tool_call(make_entry("Write", 100, 200, 1000));
    assert_eq!(check, PolicyCheck::Denied);

    // Edit is denied.
    let check = builder.record_tool_call(make_entry("Edit", 100, 200, 1000));
    assert_eq!(check, PolicyCheck::Denied);

    // Bash is denied.
    let check = builder.record_tool_call(make_entry("Bash", 100, 0, 0));
    assert_eq!(check, PolicyCheck::Denied);

    assert_eq!(builder.policy_violations.len(), 3);

    let (payload, _) = builder.build_and_sign(KEY).unwrap();
    let parsed = ParsedWitness::parse(&payload).unwrap();
    let entries = parsed.parse_trace();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].policy_check, PolicyCheck::Allowed);
    assert_eq!(entries[1].policy_check, PolicyCheck::Denied);
    assert_eq!(entries[2].policy_check, PolicyCheck::Denied);
    assert_eq!(entries[3].policy_check, PolicyCheck::Denied);
}

#[test]
fn governance_approved_mode_gates_all() {
    let policy = GovernancePolicy::approved();
    let mut builder = WitnessBuilder::new([0x20; 16], policy).with_outcome(TaskOutcome::Solved);

    let check = builder.record_tool_call(make_entry("Read", 50, 100, 500));
    assert_eq!(check, PolicyCheck::Confirmed);

    let check = builder.record_tool_call(make_entry("Bash", 100, 0, 0));
    assert_eq!(check, PolicyCheck::Confirmed);

    // No policy violations — confirmed is not a violation.
    assert!(builder.policy_violations.is_empty());
}

#[test]
fn governance_autonomous_with_cost_cap() {
    let mut policy = GovernancePolicy::autonomous();
    policy.max_cost_microdollars = 500;

    let mut builder = WitnessBuilder::new([0x30; 16], policy).with_outcome(TaskOutcome::Solved);

    builder.record_tool_call(make_entry("Read", 50, 400, 500));
    assert!(builder.policy_violations.is_empty());

    builder.record_tool_call(make_entry("Edit", 50, 200, 500));
    assert_eq!(builder.policy_violations.len(), 1);
    assert!(builder.policy_violations[0].contains("cost budget"));
}

#[test]
fn deterministic_replay_same_bytes() {
    let policy = GovernancePolicy::autonomous();
    let mut builder = WitnessBuilder::new([0x40; 16], policy)
        .with_spec(b"fix bug #42")
        .with_plan(b"1. read auth.rs\n2. fix validation")
        .with_diff(b"@@ -10,3 +10,5 @@\n+    validate(input);")
        .with_test_log(b"test auth::validate ... ok\n3 passed")
        .with_outcome(TaskOutcome::Solved);

    builder.record_tool_call(make_entry("Read", 50, 100, 500));
    builder.record_tool_call(make_entry("Edit", 100, 200, 1000));
    builder.record_tool_call(make_entry("Bash", 2000, 0, 0));

    let (payload, _) = builder.build_and_sign(KEY).unwrap();

    // Parse and extract all sections.
    let parsed = ParsedWitness::parse(&payload).unwrap();
    parsed.verify_all(KEY, &payload).unwrap();

    assert_eq!(parsed.spec.unwrap(), b"fix bug #42");
    assert_eq!(parsed.plan.unwrap(), b"1. read auth.rs\n2. fix validation");
    assert_eq!(
        parsed.diff.unwrap(),
        b"@@ -10,3 +10,5 @@\n+    validate(input);"
    );
    assert_eq!(
        parsed.test_log.unwrap(),
        b"test auth::validate ... ok\n3 passed"
    );

    let entries = parsed.parse_trace();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].action, b"Read");
    assert_eq!(entries[1].action, b"Edit");
    assert_eq!(entries[2].action, b"Bash");
    assert_eq!(entries[0].latency_ms, 50);
    assert_eq!(entries[1].cost_microdollars, 200);
    assert_eq!(entries[2].tokens, 0);

    // The bundle is self-contained evidence.
    assert!(parsed.evidence_complete());
}

#[test]
fn tampered_bundle_detected() {
    let mut builder = WitnessBuilder::new([0x50; 16], GovernancePolicy::autonomous())
        .with_spec(b"original spec")
        .with_outcome(TaskOutcome::Solved);
    builder.record_tool_call(make_entry("Bash", 100, 0, 0));

    let (mut payload, _) = builder.build_and_sign(KEY).unwrap();

    // Tamper.
    payload[WITNESS_HEADER_SIZE + 10] ^= 0xFF;

    let parsed = ParsedWitness::parse(&payload).unwrap();
    assert!(parsed.verify_signature(KEY, &payload).is_err());
}

#[test]
fn postmortem_on_failure() {
    let builder = WitnessBuilder::new([0x60; 16], GovernancePolicy::autonomous())
        .with_spec(b"implement feature X")
        .with_diff(b"partial diff")
        .with_test_log(b"test feature_x ... FAILED\n0 passed, 1 failed")
        .with_postmortem(b"Root cause: missing null check in parser")
        .with_outcome(TaskOutcome::Failed);

    let (payload, header) = builder.build_and_sign(KEY).unwrap();
    assert_eq!(header.outcome, TaskOutcome::Failed as u8);

    let parsed = ParsedWitness::parse(&payload).unwrap();
    parsed.verify_all(KEY, &payload).unwrap();
    assert_eq!(
        parsed.postmortem.unwrap(),
        b"Root cause: missing null check in parser"
    );
}

#[test]
fn scorecard_percentiles() {
    let policy = GovernancePolicy::autonomous();
    let mut sc = ScorecardBuilder::new();

    // Create 20 tasks with varying latencies.
    for i in 0..20u8 {
        let mut builder = WitnessBuilder::new([i; 16], policy.clone())
            .with_spec(b"task")
            .with_diff(b"diff")
            .with_test_log(b"ok")
            .with_outcome(TaskOutcome::Solved);
        // Latency: 100, 200, ..., 2000 ms.
        builder.record_tool_call(make_entry("Bash", (i as u32 + 1) * 100, 100, 100));
        let (payload, _) = builder.build().unwrap();
        let parsed = ParsedWitness::parse(&payload).unwrap();
        sc.add_witness(&parsed, 0, 0);
    }

    let card = sc.finish();
    assert_eq!(card.total_tasks, 20);
    assert_eq!(card.solved, 20);
    assert!((card.solve_rate - 1.0).abs() < 0.01);
    assert!((card.evidence_coverage - 1.0).abs() < 0.01);
    // Median of 100..2000 (step 100) = ~1000
    assert!(card.median_latency_ms >= 900 && card.median_latency_ms <= 1100);
    // p95 should be near 1900-2000.
    assert!(card.p95_latency_ms >= 1800);
}

#[test]
fn rollback_tracking() {
    let mut builder = WitnessBuilder::new([0x70; 16], GovernancePolicy::autonomous())
        .with_outcome(TaskOutcome::Solved);

    builder.record_rollback();
    builder.record_rollback();
    assert_eq!(builder.rollback_count, 2);

    let (payload, _) = builder.build().unwrap();
    let parsed = ParsedWitness::parse(&payload).unwrap();

    let mut sc = ScorecardBuilder::new();
    sc.add_witness(&parsed, 0, 2);
    let card = sc.finish();
    assert_eq!(card.rollback_count, 2);
}

#[test]
fn zero_policy_violations_in_autonomous() {
    let policy = GovernancePolicy::autonomous();
    let mut total_violations = 0u32;

    for i in 0..100u8 {
        let mut builder =
            WitnessBuilder::new([i; 16], policy.clone()).with_outcome(TaskOutcome::Solved);
        builder.record_tool_call(make_entry("Read", 10, 10, 10));
        builder.record_tool_call(make_entry("Edit", 10, 10, 10));
        builder.record_tool_call(make_entry("Bash", 10, 10, 10));
        total_violations += builder.policy_violations.len() as u32;
    }

    assert_eq!(total_violations, 0, "zero policy violations in 100 runs");
}
