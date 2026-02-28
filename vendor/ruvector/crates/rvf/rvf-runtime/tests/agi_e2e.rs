//! End-to-end integration tests and benchmarks for the AGI Cognitive Container
//! system (ADR-036). Covers the full build -> serialize -> parse -> validate
//! cycle, signed container tamper detection, execution mode validation matrix,
//! authority levels, resource budgets, coherence thresholds, and perf benchmarks.

use rvf_runtime::agi_container::{AgiContainerBuilder, ParsedAgiManifest};
use rvf_runtime::seed_crypto;
use rvf_types::agi_container::*;

const SIGNING_KEY: &[u8] = b"agi-e2e-test-signing-key-32bytes";
const ORCH_JSON: &[u8] = br#"{"model":"claude-opus-4-6","max_turns":100}"#;
const TASKS_JSON: &[u8] = br#"[{"id":1,"spec":"fix bug"}]"#;
const GRADERS_JSON: &[u8] = br#"[{"type":"test_pass"}]"#;
const TOOLS_JSON: &[u8] = br#"[{"name":"ruvector_query","type":"search"}]"#;
const COHER_JSON: &[u8] = br#"{"min_cut":0.7,"rollback":true}"#;

/// Build a fully-populated container with every optional section.
fn build_full_container() -> (Vec<u8>, AgiContainerHeader) {
    AgiContainerBuilder::new([0x01; 16], [0x02; 16])
        .with_model_id("claude-opus-4-6")
        .with_policy(b"autonomous", [0xAA; 8])
        .with_orchestrator(ORCH_JSON)
        .with_tool_registry(TOOLS_JSON)
        .with_agent_prompts(b"You are a coder agent.")
        .with_eval_tasks(TASKS_JSON)
        .with_eval_graders(GRADERS_JSON)
        .with_skill_library(b"[]")
        .with_replay_script(b"#!/bin/sh\nrvf replay $1")
        .with_kernel_config(b"console=ttyS0")
        .with_network_config(b"{\"port\":8080}")
        .with_coherence_config(COHER_JSON)
        .with_project_instructions(b"# CLAUDE.md")
        .with_dependency_snapshot(b"sha256:abc123")
        .with_authority_config(b"{\"level\":\"WriteMemory\"}")
        .with_domain_profile(b"coding")
        .offline_capable()
        .with_segments(ContainerSegments {
            kernel_present: true,
            kernel_size: 5_000_000,
            wasm_count: 2,
            wasm_total_size: 60_000,
            vec_segment_count: 4,
            index_segment_count: 2,
            witness_count: 100,
            crypto_present: false,
            manifest_present: true,
            orchestrator_present: true,
            world_model_present: true,
            domain_expansion_present: false,
            total_size: 0,
        })
        .build()
        .unwrap()
}

// -- 1. Full Container Lifecycle --

#[test]
fn full_container_lifecycle() {
    let (payload, header) = build_full_container();

    assert!(header.is_valid_magic());
    assert_eq!(header.version, 1);
    assert!(header.has_kernel());
    assert!(header.has_orchestrator());
    assert!(header.has_world_model());
    assert!(header.is_replay_capable());
    assert!(header.is_offline_capable());
    assert!(
        header.created_ns > 0,
        "created_ns should be a real timestamp"
    );

    // Header round-trip.
    let header_rt = AgiContainerHeader::from_bytes(&header.to_bytes()).unwrap();
    assert_eq!(header_rt, header);

    // Parse manifest and verify every section.
    let p = ParsedAgiManifest::parse(&payload).unwrap();
    assert_eq!(p.model_id_str(), Some("claude-opus-4-6"));
    assert_eq!(p.orchestrator_config.unwrap(), ORCH_JSON);
    assert_eq!(p.tool_registry.unwrap(), TOOLS_JSON);
    assert_eq!(p.eval_tasks.unwrap(), TASKS_JSON);
    assert_eq!(p.eval_graders.unwrap(), GRADERS_JSON);
    assert_eq!(p.coherence_config.unwrap(), COHER_JSON);
    assert!(p.policy.is_some());
    assert!(p.agent_prompts.is_some());
    assert!(p.skill_library.is_some());
    assert!(p.replay_script.is_some());
    assert!(p.kernel_config.is_some());
    assert!(p.network_config.is_some());
    assert!(p.project_instructions.is_some());
    assert!(p.dependency_snapshot.is_some());
    assert!(p.authority_config.is_some());
    assert!(p.domain_profile.is_some());
    assert!(p.is_autonomous_capable());

    // Segment-derived flags should all be present in the header.
    let seg_flags = ContainerSegments {
        kernel_present: true,
        wasm_count: 2,
        witness_count: 100,
        orchestrator_present: true,
        world_model_present: true,
        ..Default::default()
    }
    .to_flags();
    assert_eq!(header.flags & seg_flags, seg_flags);
}

// -- 2. Signed Container Tamper Detection --

#[test]
fn signed_container_tamper_detection() {
    let builder = AgiContainerBuilder::new([0x10; 16], [0x20; 16])
        .with_model_id("claude-opus-4-6")
        .with_orchestrator(ORCH_JSON)
        .with_eval_tasks(TASKS_JSON)
        .with_eval_graders(GRADERS_JSON)
        .with_segments(ContainerSegments {
            kernel_present: true,
            manifest_present: true,
            world_model_present: true,
            ..Default::default()
        });

    let (payload, header) = builder.build_and_sign(SIGNING_KEY).unwrap();
    assert!(header.is_signed());

    let unsigned_len = payload.len() - 32;
    let sig = &payload[unsigned_len..];
    assert!(seed_crypto::verify_seed(
        SIGNING_KEY,
        &payload[..unsigned_len],
        sig
    ));

    // Tamper with one byte in the TLV payload area.
    let mut tampered = payload.clone();
    tampered[AGI_HEADER_SIZE + 10] ^= 0xFF;
    assert!(
        !seed_crypto::verify_seed(SIGNING_KEY, &tampered[..unsigned_len], sig),
        "tampered payload must fail verification"
    );

    // Tamper with header byte.
    let mut tampered_hdr = payload.clone();
    tampered_hdr[7] ^= 0x01;
    assert!(
        !seed_crypto::verify_seed(SIGNING_KEY, &tampered_hdr[..unsigned_len], sig),
        "tampered header must fail verification"
    );
}

// -- 3. Execution Mode Validation Matrix --

#[test]
fn execution_mode_validation_matrix() {
    let m = |mp, kp, wc, wmc, vsc, isc, wnc| ContainerSegments {
        manifest_present: mp,
        kernel_present: kp,
        wasm_count: wc,
        world_model_present: wmc,
        vec_segment_count: vsc,
        index_segment_count: isc,
        witness_count: wnc,
        ..Default::default()
    };

    // Replay + no witness -> fail
    assert!(m(true, false, 0, false, 0, 0, 0)
        .validate(ExecutionMode::Replay)
        .is_err());
    // Replay + witness -> pass
    assert!(m(true, false, 0, false, 0, 0, 10)
        .validate(ExecutionMode::Replay)
        .is_ok());
    // Verify + no runtime -> fail
    assert!(m(true, false, 0, false, 0, 0, 0)
        .validate(ExecutionMode::Verify)
        .is_err());
    // Verify + kernel + world_model -> pass
    assert!(m(true, true, 0, true, 0, 0, 0)
        .validate(ExecutionMode::Verify)
        .is_ok());
    // Verify + wasm + vec -> pass
    assert!(m(true, false, 1, false, 2, 0, 0)
        .validate(ExecutionMode::Verify)
        .is_ok());
    // Live + kernel only (no world model) -> fail
    assert!(m(true, true, 0, false, 0, 0, 0)
        .validate(ExecutionMode::Live)
        .is_err());
    // Live + kernel + world model -> pass
    assert!(m(true, true, 0, true, 0, 0, 0)
        .validate(ExecutionMode::Live)
        .is_ok());
}

// -- 4. Authority Level Tests --

#[test]
fn authority_level_defaults_per_mode() {
    assert_eq!(
        AuthorityLevel::default_for_mode(ExecutionMode::Replay),
        AuthorityLevel::ReadOnly
    );
    assert_eq!(
        AuthorityLevel::default_for_mode(ExecutionMode::Verify),
        AuthorityLevel::ExecuteTools
    );
    assert_eq!(
        AuthorityLevel::default_for_mode(ExecutionMode::Live),
        AuthorityLevel::WriteMemory
    );
}

#[test]
fn authority_level_hierarchy() {
    // WriteExternal permits all.
    assert!(AuthorityLevel::WriteExternal.permits(AuthorityLevel::ReadOnly));
    assert!(AuthorityLevel::WriteExternal.permits(AuthorityLevel::WriteMemory));
    assert!(AuthorityLevel::WriteExternal.permits(AuthorityLevel::ExecuteTools));
    assert!(AuthorityLevel::WriteExternal.permits(AuthorityLevel::WriteExternal));
    // ExecuteTools permits itself and below.
    assert!(AuthorityLevel::ExecuteTools.permits(AuthorityLevel::ReadOnly));
    assert!(AuthorityLevel::ExecuteTools.permits(AuthorityLevel::WriteMemory));
    assert!(AuthorityLevel::ExecuteTools.permits(AuthorityLevel::ExecuteTools));
    assert!(!AuthorityLevel::ExecuteTools.permits(AuthorityLevel::WriteExternal));
    // ReadOnly permits nothing above itself.
    assert!(AuthorityLevel::ReadOnly.permits(AuthorityLevel::ReadOnly));
    assert!(!AuthorityLevel::ReadOnly.permits(AuthorityLevel::WriteMemory));
    assert!(!AuthorityLevel::ReadOnly.permits(AuthorityLevel::ExecuteTools));
    assert!(!AuthorityLevel::ReadOnly.permits(AuthorityLevel::WriteExternal));
}

// -- 5. Resource Budget Tests --

#[test]
fn resource_budget_clamping() {
    let clamped = ResourceBudget {
        max_time_secs: 99999,
        max_tokens: 99999999,
        max_cost_microdollars: 99999999,
        max_tool_calls: 65535,
        max_external_writes: 65535,
    }
    .clamped();
    assert_eq!(clamped.max_time_secs, 3600);
    assert_eq!(clamped.max_tokens, 1_000_000);
    assert_eq!(clamped.max_cost_microdollars, 10_000_000);
    assert_eq!(clamped.max_tool_calls, 500);
    assert_eq!(clamped.max_external_writes, 50);
}

#[test]
fn resource_budget_within_max_unchanged() {
    assert_eq!(ResourceBudget::DEFAULT.clamped(), ResourceBudget::DEFAULT);
}

// -- 6. Coherence Threshold Validation --

#[test]
fn coherence_threshold_validation() {
    assert!(CoherenceThresholds::DEFAULT.validate().is_ok());
    assert!(CoherenceThresholds::STRICT.validate().is_ok());

    // Invalid: score > 1.0
    let bad = CoherenceThresholds {
        min_coherence_score: 1.5,
        ..CoherenceThresholds::DEFAULT
    };
    assert!(bad.validate().is_err());
    // Invalid: negative rate
    let bad2 = CoherenceThresholds {
        max_contradiction_rate: -1.0,
        ..CoherenceThresholds::DEFAULT
    };
    assert!(bad2.validate().is_err());
    // Invalid: rollback ratio > 1.0
    let bad3 = CoherenceThresholds {
        max_rollback_ratio: 2.0,
        ..CoherenceThresholds::DEFAULT
    };
    assert!(bad3.validate().is_err());
    // Edge: zero values are valid
    let edge = CoherenceThresholds {
        min_coherence_score: 0.0,
        max_contradiction_rate: 0.0,
        max_rollback_ratio: 0.0,
    };
    assert!(edge.validate().is_ok());
}

// -- 7. Container Size Limit --

#[test]
fn container_size_limit_enforced() {
    let oversized = ContainerSegments {
        manifest_present: true,
        total_size: AGI_MAX_CONTAINER_SIZE + 1,
        ..Default::default()
    };
    assert_eq!(
        oversized.validate(ExecutionMode::Replay),
        Err(ContainerError::TooLarge {
            size: AGI_MAX_CONTAINER_SIZE + 1
        })
    );
}

// -- 8. Performance Benchmarks (using std::time) --

#[test]
fn bench_header_serialize_deserialize() {
    use std::time::Instant;
    let header = AgiContainerHeader {
        magic: AGI_MAGIC,
        version: 1,
        flags: AGI_HAS_KERNEL | AGI_HAS_WASM | AGI_HAS_ORCHESTRATOR | AGI_SIGNED,
        container_id: [0x42; 16],
        build_id: [0x43; 16],
        created_ns: 1_700_000_000_000_000_000,
        model_id_hash: [0xAA; 8],
        policy_hash: [0xBB; 8],
    };
    let n: u128 = 100_000;

    let start = Instant::now();
    for _ in 0..n {
        let _ = std::hint::black_box(header.to_bytes());
    }
    let ser = start.elapsed();

    let bytes = header.to_bytes();
    let start = Instant::now();
    for _ in 0..n {
        let _ = std::hint::black_box(AgiContainerHeader::from_bytes(&bytes).unwrap());
    }
    let deser = start.elapsed();

    let ser_ns = ser.as_nanos() / n;
    let deser_ns = deser.as_nanos() / n;
    eprintln!("Header serialize:   {ser_ns:>8} ns/op ({n} iterations in {ser:?})");
    eprintln!("Header deserialize: {deser_ns:>8} ns/op ({n} iterations in {deser:?})");
    assert!(ser_ns < 1000, "serialize too slow: {ser_ns} ns/op");
    assert!(deser_ns < 1000, "deserialize too slow: {deser_ns} ns/op");
}

#[test]
fn bench_container_build_parse() {
    use std::time::Instant;
    let n: u128 = 10_000;
    let segs = || ContainerSegments {
        kernel_present: true,
        manifest_present: true,
        world_model_present: true,
        ..Default::default()
    };

    let start = Instant::now();
    for _ in 0..n {
        let b = AgiContainerBuilder::new([0x01; 16], [0x02; 16])
            .with_model_id("claude-opus-4-6")
            .with_policy(b"autonomous", [0xAA; 8])
            .with_orchestrator(ORCH_JSON)
            .with_eval_tasks(TASKS_JSON)
            .with_eval_graders(GRADERS_JSON)
            .with_segments(segs());
        let _ = std::hint::black_box(b.build().unwrap());
    }
    let build_elapsed = start.elapsed();

    let (payload, _) = AgiContainerBuilder::new([0x01; 16], [0x02; 16])
        .with_model_id("claude-opus-4-6")
        .with_orchestrator(ORCH_JSON)
        .with_eval_tasks(TASKS_JSON)
        .with_eval_graders(GRADERS_JSON)
        .with_segments(segs())
        .build()
        .unwrap();

    let start = Instant::now();
    for _ in 0..n {
        let _ = std::hint::black_box(ParsedAgiManifest::parse(&payload).unwrap());
    }
    let parse_elapsed = start.elapsed();

    let build_ns = build_elapsed.as_nanos() / n;
    let parse_ns = parse_elapsed.as_nanos() / n;
    eprintln!("Container build: {build_ns:>8} ns/op ({n} iterations in {build_elapsed:?})");
    eprintln!("Container parse: {parse_ns:>8} ns/op ({n} iterations in {parse_elapsed:?})");
    assert!(build_ns < 10_000, "build too slow: {build_ns} ns/op");
    assert!(parse_ns < 5_000, "parse too slow: {parse_ns} ns/op");
}

#[test]
fn bench_flags_computation() {
    use std::time::Instant;
    let n: u128 = 1_000_000;
    let segs = ContainerSegments {
        kernel_present: true,
        wasm_count: 2,
        witness_count: 100,
        crypto_present: true,
        orchestrator_present: true,
        world_model_present: true,
        vec_segment_count: 4,
        index_segment_count: 2,
        ..Default::default()
    };

    let start = Instant::now();
    for _ in 0..n {
        let _ = std::hint::black_box(segs.to_flags());
    }
    let elapsed = start.elapsed();

    let ns = elapsed.as_nanos() / n;
    eprintln!("Flags computation: {ns:>8} ns/op ({n} iterations in {elapsed:?})");
    assert!(ns < 100, "flags computation too slow: {ns} ns/op");
}
