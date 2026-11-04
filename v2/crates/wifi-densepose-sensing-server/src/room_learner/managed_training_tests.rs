//! SYNTHETIC native packet fixtures; no hardware or general accuracy claim.
use super::tests::{context, phase, runtime, START};
use super::*;
use crate::room_learner::{
    capture::CaptureSession,
    managed_training::ManagedTrainingProof,
    observation::ValidatedDataset,
    qualification::{hash, managed_evidence_with_proof},
};
use std::path::PathBuf;
fn proof_path(root: &Path, model: &RoomModel) -> PathBuf {
    use sha2::{Digest, Sha256};
    root.join("a".repeat(64))
        .join("SensingServer/room-learner")
        .join(model.context().digest())
        .join(format!(
            "managed-training-{:x}.json",
            Sha256::digest(model.export().unwrap())
        ))
}
fn current(rt: &RoomLearningRuntime) -> (Arc<RoomModel>, ManagedTrainingProof) {
    let state = rt.inner.lock().unwrap();
    let model = state.model.as_ref().unwrap().clone();
    let proof = rt
        .store
        .load_managed_training_proof(&model)
        .unwrap()
        .unwrap();
    (model, proof)
}
fn restore(root: &Path) -> Arc<RoomLearningRuntime> {
    let cold = runtime(root);
    cold.configure(
        &cold.mutation_guard().unwrap(),
        context(),
        "caller-new-run-must-not-replace-original".into(),
        false,
    )
    .unwrap();
    cold
}
#[tokio::test]
async fn metadata_proof_cold_resume_fresh_all_conditions_qualify_v2_without_raw_train() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (model, proof) = current(&rt);
    let before = std::fs::read(proof_path(dir.path(), &model)).unwrap();
    let text = String::from_utf8(before.clone()).unwrap();
    for forbidden in [
        "\"samples\"",
        "\"frame_ids\"",
        "\"values\"",
        "\"amplitudes\"",
    ] {
        assert!(!text.contains(forbidden));
    }
    let legacy = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    assert_eq!(
        legacy.receipt.schema,
        "ruview.local-operational-qualification.v1"
    );
    assert!(!serde_json::to_value(&legacy.receipt)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("training_lineage_basis"));
    let cold = restore(dir.path());
    let ready = cold.status(START + 200_000_000_000)["qualification_readiness"].clone();
    assert_eq!(ready["eligible_for_held_out"], true, "{ready}");
    assert_eq!(ready["raw_train_rows_retained"], false);
    assert_eq!(ready["training_lineage_basis"], "managed_training_proof_v1");
    assert_eq!(ready["train_cohorts_per_condition"], json!([4, 4, 4, 4]));
    assert_eq!(ready["managed_training_proof_sha256"], proof.proof_sha256);
    for (i, label) in crate::room_learner::qualification::CONDITIONS
        .into_iter()
        .enumerate()
    {
        phase(&cold, DataRole::HeldOut, label, i as u32 + 8);
    }
    let q = cold.qualify(&cold.mutation_guard().unwrap()).unwrap();
    assert_eq!(
        q.receipt.schema,
        "ruview.local-operational-qualification.v2"
    );
    assert_eq!(
        q.receipt.managed_training_proof_sha256.as_deref(),
        Some(proof.proof_sha256.as_str())
    );
    assert_eq!(
        q.receipt.training_lineage_basis.as_deref(),
        Some("managed_training_proof_v1")
    );
    assert!(!q.receipt.report.independence_verified && !q.receipt.physically_validated);
    assert_eq!(
        cold.status(START + 200_000_000_000)["last_evaluation"]["train_lineage_checked"],
        false
    );
    assert_eq!(
        std::fs::read(proof_path(dir.path(), &model)).unwrap(),
        before
    );
    let summary = crate::room_learner::selection::QualificationSummary::from(&q);
    let public = serde_json::to_value(&summary).unwrap();
    assert_eq!(
        public["schema"],
        "ruview.local-operational-qualification.v2"
    );
    assert!(public.get("first_observed_at_ns").is_none());
    cold.select_model(
        &cold.mutation_guard().unwrap(),
        &q.receipt.model_sha256,
        &q.receipt_sha256,
    )
    .unwrap();
    let selected = runtime(dir.path());
    let s = selected.status(START + 200_000_000_000);
    assert_eq!(
        s["model_selection"]["selected"]["qualification_sha256"],
        q.receipt_sha256
    );
    assert_eq!(s["qualification_readiness"]["eligible_for_held_out"], true);
}
#[tokio::test]
async fn old_diagnostic_model_without_proof_loads_but_stops_selection_capture_before_effort() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (model, _) = current(&rt);
    std::fs::remove_file(proof_path(dir.path(), &model)).unwrap();
    let cold = restore(dir.path());
    let status = cold.status(START + 200_000_000_000);
    assert_eq!(status["candidate_loaded"], true);
    assert_eq!(
        status["qualification_readiness"]["reason"],
        "missing_managed_training_proof"
    );
    let result = cold.begin_phase(
        &cold.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: "fresh-empty".into(),
            session_id: "fresh-empty-session".into(),
            role: DataRole::HeldOut,
            label: RoomLabel::Empty,
            duration_ms: 12000,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        START + 200_000_000_000,
    );
    assert!(result.unwrap_err().contains("cannot qualify"));
    assert!(!cold.capture_active().unwrap());
    assert!(cold.qualify(&cold.mutation_guard().unwrap()).is_err());
}
#[tokio::test]
async fn proof_resume_preserves_counter_namespace_roles_and_rejects_regressed_receiver_clock() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (_, proof) = current(&rt);
    let mut capture = CaptureSession::resume_managed_proof(&proof).unwrap();
    let w = &proof.seed.sources[0];
    let values = vec![2.0; 8];
    for sequence in [w.sequence, w.sequence.wrapping_sub(1), 0] {
        assert!(capture
            .ingest(
                RawSourceFrame {
                    source_id: w.source_id,
                    grid_id: "ht64-canonical8",
                    frequency_hz: 2_437_000_000,
                    bandwidth_hz: 20_000_000,
                    sequence,
                    amplitudes: &values
                },
                proof.seed.last_seen_ns + 1
            )
            .is_err());
    }
    assert!(capture
        .ingest(
            RawSourceFrame {
                source_id: w.source_id,
                grid_id: "ht64-canonical8",
                frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                sequence: w.sequence.wrapping_add(1),
                amplitudes: &values
            },
            proof.seed.last_seen_ns - 1
        )
        .is_err());
    capture
        .ingest(
            RawSourceFrame {
                source_id: w.source_id,
                grid_id: "ht64-canonical8",
                frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                sequence: w.sequence.wrapping_add(1),
                amplitudes: &values,
            },
            proof.seed.last_seen_ns + 1,
        )
        .unwrap();
    assert_eq!(
        capture.progress(proof.seed.last_seen_ns + 2).sources[0].last_accepted_sequence,
        Some(w.sequence + 1)
    );
    let crossed = PhaseRequest {
        trial_id: "synthetic-trial-0".into(),
        session_id: "unused-session".into(),
        role: DataRole::HeldOut,
        label: RoomLabel::Empty,
        duration_ms: 12000,
        lead_in_ms: 0,
        conditions_confirmed: true,
    };
    assert!(capture
        .begin_phase(crossed, proof.seed.last_seen_ns + 3)
        .unwrap_err()
        .contains("original managed training roles"));
    let cold = restore(dir.path());
    assert_eq!(
        cold.status(proof.seed.last_seen_ns - 1)["qualification_readiness"]["reason"],
        "receiver_clock_incomparable"
    );
}
#[tokio::test]
async fn v2_capture_checkpoint_retains_metadata_fences_and_no_dummy_training_rows() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (_, proof) = current(&rt);
    let capture = CaptureSession::resume_managed_proof(&proof).unwrap();
    let at = proof.seed.last_seen_ns + 10;
    let snapshot = capture.checkpoint(at).unwrap();
    let json = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(json["schema"], "ruview.room-learner.capture-checkpoint.v2");
    assert!(json["cohorts"].as_array().unwrap().is_empty());
    let restored =
        CaptureSession::restore_checkpoint(snapshot, &proof.seed.context, at + 1).unwrap();
    assert_eq!(restored.session.metadata_lineage(), Some(&proof.seed));
    assert_eq!(
        restored.session.progress(at + 2).sources[0].last_accepted_sequence,
        Some(proof.seed.sources[0].sequence)
    );
    let mut missing = json;
    missing.as_object_mut().unwrap().remove("metadata_lineage");
    let snapshot = serde_json::from_value(missing).unwrap();
    assert!(CaptureSession::restore_checkpoint(snapshot, &proof.seed.context, at + 1).is_err());
}
#[tokio::test]
async fn corrupt_foreign_and_torn_metadata_never_replace_exact_train_checks() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (model, proof) = current(&rt);
    for mutate in [
        "owner",
        "installation",
        "context",
        "model",
        "train",
        "source",
        "count",
        "epoch",
        "clock",
        "floor",
        "ack",
        "hash",
    ] {
        let mut p = proof.clone();
        match mutate {
            "owner" => p.seed.context.owner_scope = "b".repeat(64),
            "installation" => p.seed.context.installation_id = "foreign-installation".into(),
            "context" => p.seed.context.room_digest = "foreign-room".into(),
            "model" => p.model_sha256 = "b".repeat(64),
            "train" => p.seed.training_data_sha256 = "b".repeat(64),
            "source" => {
                p.seed.context.sources[1].source_id = 99;
                p.seed.sources[1].source_id = 99;
            }
            "count" => p.seed.train_counts[1] = 0,
            "epoch" => p.seed.source_epoch_verified = true,
            "clock" => p.seed.receiver_clock_domain = "unknown".into(),
            "floor" => {
                p.seed.sources[0].ordinal = p.seed.sources[0].train_range.last_ordinal - 1;
                p.seed.sources[0].admitted_packets = p.seed.sources[0].ordinal;
            }
            "ack" => p.seed.phase_claims[0].cohort_count = 0,
            _ => p.proof_sha256 = "b".repeat(64),
        }
        if mutate != "hash" {
            p.proof_sha256 = hash(&(&p.schema, &p.model_sha256, &p.seed)).unwrap();
        }
        assert!(p.validate(&model).is_err(), "{mutate}");
    }
    let evaluation = rt
        .store
        .evaluate_candidate(
            &model,
            &rt.inner
                .lock()
                .unwrap()
                .capture
                .as_ref()
                .unwrap()
                .validated_dataset()
                .unwrap(),
            DataRole::HeldOut,
        )
        .unwrap();
    let state = rt.inner.lock().unwrap();
    let capture = state.capture.as_ref().unwrap();
    let mut rows = capture.cohorts().to_vec();
    rows.iter_mut()
        .find(|c| c.role == DataRole::Train)
        .unwrap()
        .views[0]
        .samples[0][0] += 0.25;
    let altered = ValidatedDataset::new(model.context().clone(), rows).unwrap();
    assert!(managed_evidence_with_proof(
        &model,
        &altered,
        capture.history(),
        &evaluation,
        Some(&proof)
    )
    .unwrap_err()
    .contains("retained exact Train"));
    drop(state);
    crate::companion::atomic_private_write(&proof_path(dir.path(), &model), b"{").unwrap();
    assert!(rt.store.load_managed_training_proof(&model).is_err());
    let cold = runtime(dir.path());
    assert!(cold
        .configure(
            &cold.mutation_guard().unwrap(),
            context(),
            "torn".into(),
            false
        )
        .is_err());
}
fn phase_at(
    rt: &RoomLearningRuntime,
    label: RoomLabel,
    index: u32,
    base: u64,
    gain_override: Option<f64>,
) -> u64 {
    phase_at_group(rt, label, index, base, gain_override, None)
}
fn phase_at_group(
    rt: &RoomLearningRuntime,
    label: RoomLabel,
    index: u32,
    base: u64,
    gain_override: Option<f64>,
    group: Option<&str>,
) -> u64 {
    let end = prepare_phase_at(rt, label, index, base, gain_override, group);
    rt.finish_phase(&rt.mutation_guard().unwrap(), end, true)
        .unwrap();
    end
}
fn prepare_phase_at(
    rt: &RoomLearningRuntime,
    label: RoomLabel,
    index: u32,
    base: u64,
    gain_override: Option<f64>,
    group: Option<&str>,
) -> u64 {
    let at = base + u64::from(index) * 13_000_000_000;
    let role = if index < 4 {
        DataRole::Train
    } else {
        DataRole::HeldOut
    };
    rt.begin_phase(
        &rt.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: format!("epoch-trial-{index}"),
            session_id: group
                .map(str::to_owned)
                .unwrap_or_else(|| format!("epoch-session-{index}")),
            role,
            label,
            duration_ms: 12000,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        at,
    )
    .unwrap();
    let gain = gain_override.unwrap_or(match label {
        RoomLabel::Empty => 2.0,
        RoomLabel::OccupiedStatic => 6.0,
        RoomLabel::OccupiedMoving => 7.0,
        RoomLabel::AdjacentOnly => 2.3,
        RoomLabel::Unknown => panic!("unlabelled"),
    });
    let context = rt.context().unwrap();
    for quarter in 0..4u32 {
        for sample in 0..8u32 {
            for source in &context.sources {
                let values: Vec<_> = (0..source.feature_count)
                    .map(|bin| {
                        gain + f64::from(source.source_id) * 0.3
                            + bin as f64 * 0.04
                            + f64::from(sample) * 0.003
                    })
                    .collect();
                rt.ingest(
                    RawSourceFrame {
                        source_id: source.source_id,
                        grid_id: &source.grid_id,
                        frequency_hz: source.center_frequency_hz,
                        bandwidth_hz: source.bandwidth_hz,
                        sequence: index * 32 + quarter * 8 + sample + 1,
                        amplitudes: &values,
                    },
                    at + u64::from(quarter) * 3_000_000_000
                        + 1_000_000
                        + u64::from(sample) * 10_000_000
                        + u64::from(source.source_id),
                )
                .unwrap();
            }
        }
    }
    at + 12_000_000_000
}
#[tokio::test]
async fn failed_assessment_cold_resume_keeps_all_sealed_phase_fences_not_only_train() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (model, proof) = current(&rt);
    rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    // Deliberately wrong synthetic Empty observations produce a failed policy.
    phase_at(&rt, RoomLabel::Empty, 8, START, Some(7.0));
    assert!(rt.qualify(&rt.mutation_guard().unwrap()).is_err());
    let fence = rt.store.load_managed_capture_fence(&model, &proof).unwrap();
    assert_eq!(fence.revision, 5);
    let cold = restore(dir.path());
    let status = cold.status(START + 200_000_000_000);
    assert!(status["model_selection"]["qualification_error"]
        .as_str()
        .is_some());
    let state = cold.inner.lock().unwrap();
    let capture = state.capture.as_ref().unwrap();
    assert_eq!(capture.metadata_lineage(), Some(&fence.seed));
    let last = &fence.seed.sources[0];
    drop(state);
    let values = vec![2.0; 8];
    assert!(cold
        .ingest(
            RawSourceFrame {
                source_id: last.source_id,
                grid_id: "ht64-canonical8",
                frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                sequence: last.sequence,
                amplitudes: &values
            },
            START + 200_000_000_000
        )
        .is_err());
    assert!(cold
        .begin_phase(
            &cold.mutation_guard().unwrap(),
            PhaseRequest {
                trial_id: "epoch-trial-8".into(),
                session_id: "new-session".into(),
                role: DataRole::HeldOut,
                label: RoomLabel::Empty,
                duration_ms: 12000,
                lead_in_ms: 0,
                conditions_confirmed: true
            },
            START + 200_000_000_000
        )
        .is_err());
    // New real conditions require new IDs AND advancing original source counters.
    phase_at(&cold, RoomLabel::Empty, 9, START, None);
    let next = cold
        .store
        .load_managed_capture_fence(&model, &proof)
        .unwrap();
    assert_eq!(next.revision, 6);
    assert!(next.seed.sources[0].ordinal > last.ordinal);
}
#[tokio::test]
async fn missing_torn_and_unpublished_sealed_ledger_never_reset_to_train_floor() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (model, proof) = current(&rt);
    let directory = proof_path(dir.path(), &model)
        .parent()
        .unwrap()
        .join(format!("managed-fences-{}", proof.model_sha256));
    let head = directory.join("managed-sealed-phase-head-v1.json");
    let retained = std::fs::read(&head).unwrap();
    std::fs::remove_file(&head).unwrap();
    assert!(rt.store.load_managed_capture_fence(&model, &proof).is_err());
    let cold = runtime(dir.path());
    assert!(cold
        .configure(
            &cold.mutation_guard().unwrap(),
            context(),
            "missing-ledger".into(),
            false
        )
        .is_err());
    crate::companion::atomic_private_write(&head, b"{").unwrap();
    assert!(rt.store.load_managed_capture_fence(&model, &proof).is_err());
    crate::companion::atomic_private_write(&head, &retained).unwrap();
    // A pending immutable child without published head blocks model resume.
    let original = rt.store.load_managed_capture_fence(&model, &proof).unwrap();
    let mut next_seed = original.seed.clone();
    let mut claim = next_seed.phase_claims.last().unwrap().clone();
    claim.trial_sha256 = "c".repeat(64);
    claim.session_sha256 = "d".repeat(64);
    next_seed.phase_claims.push(claim);
    let pending = crate::room_learner::managed_capture_fences::ManagedCaptureFence::issue(
        &proof,
        Some(&original),
        next_seed.clone(),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&pending).unwrap();
    crate::companion::atomic_private_write(
        &directory.join(format!("{}.json", pending.fence_sha256)),
        &bytes,
    )
    .unwrap();
    assert!(rt
        .store
        .load_managed_capture_fence(&model, &proof)
        .unwrap_err()
        .contains("pending"));
    // The exact finishing caller can complete this pending publication, keeping
    // its original identity instead of inventing a newer observation timestamp.
    let saved = rt
        .store
        .save_managed_capture_fence(&model, &proof, next_seed)
        .unwrap();
    assert_eq!(saved, pending);
    assert_eq!(
        rt.store.load_managed_capture_fence(&model, &proof).unwrap(),
        pending
    );
}
#[tokio::test]
async fn unix_scale_v2_wire_fixture_uses_decimal_ns_and_bounded_summary_only() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let mut context = context();
    context.room_digest = "c".repeat(64);
    for source in &mut context.sources {
        source.device_id = format!("esp32-node-{}", source.source_id);
        source.link_id = format!("receiver-{}-transmitter-unverified", source.source_id);
        source.grid_id = crate::room_learner::native::grid_id(
            1,
            64,
            wifi_densepose_hardware::PpduType::HtLegacy,
        )
        .unwrap();
        source.feature_count = 56;
    }
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context.clone(),
        "synthetic-unix-time-fixture".into(),
        true,
    )
    .unwrap();
    const BASE: u64 = 1_793_600_000_000_000_000;
    for (i, label) in crate::room_learner::qualification::CONDITIONS
        .into_iter()
        .enumerate()
    {
        phase_at(&rt, label, i as u32, BASE, None);
    }
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 8,
            learning_rate: 0.5,
            hidden_dim: 2,
            embedding_dim: 2,
            temporal_tokens: 2,
            ..FitConfig::default()
        },
    )
    .unwrap();
    assert_eq!(super::tests::finished(&rt).await["state"], "ready");
    for (i, label) in crate::room_learner::qualification::CONDITIONS
        .into_iter()
        .enumerate()
    {
        phase_at(&rt, label, i as u32 + 4, BASE, None);
    }
    let v1 = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    let cold = runtime(dir.path());
    cold.configure(
        &cold.mutation_guard().unwrap(),
        context.clone(),
        "synthetic-fresh-runtime".into(),
        false,
    )
    .unwrap();
    let owner_status = cold.status(BASE + 300_000_000_000);
    let readiness = owner_status["qualification_readiness"].clone();
    let clock_ineligible_status = cold.status(BASE - 1);
    let diagnostic_dir = tempfile::tempdir().unwrap();
    let diagnostic = runtime(diagnostic_dir.path());
    let (diagnostic_model, _) = current(&rt);
    diagnostic.store.save_candidate(&diagnostic_model).unwrap();
    diagnostic
        .configure(
            &diagnostic.mutation_guard().unwrap(),
            context.clone(),
            "v1-diagnostic-import".into(),
            false,
        )
        .unwrap();
    let legacy_ineligible_status = diagnostic.status(BASE + 300_000_000_000);
    for (i, label) in crate::room_learner::qualification::CONDITIONS
        .into_iter()
        .enumerate()
    {
        phase_at(&cold, label, i as u32 + 8, BASE, None);
    }
    let q = cold.qualify(&cold.mutation_guard().unwrap()).unwrap();
    let receipt = serde_json::to_value(&q).unwrap();
    assert_eq!(
        receipt["receipt"]["first_observed_at_ns"],
        BASE.saturating_add(1_000_001).to_string()
    );
    assert!(receipt["receipt"]["last_observed_at_ns"].is_string());
    let summary = serde_json::to_value(crate::room_learner::selection::QualificationSummary::from(
        &q,
    ))
    .unwrap();
    assert!(summary["evaluated_at_unix_ms"].as_u64().unwrap() < (1u64 << 53));
    for key in [
        "first_observed_at_ns",
        "last_observed_at_ns",
        "phase_claims",
        "sources",
        "frame_ids",
        "sequence_ids",
    ] {
        assert!(summary.get(key).is_none());
    }
    let fixture = json!({"fixture_kind":"synthetic_native_producer_2026_utc","readiness":readiness,"owner_status":owner_status,"legacy_ineligible_status":legacy_ineligible_status,"clock_ineligible_status":clock_ineligible_status,"v1_summary":crate::room_learner::selection::QualificationSummary::from(&v1),"v2_summary":summary,"v2_envelope":receipt,"raw_train_lineage_checked":cold.status(BASE+300_000_000_000)["last_evaluation"]["train_lineage_checked"]});
    if let Ok(path) = std::env::var("RUVIEW_MANAGED_PROOF_FIXTURE_OUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    }
}
#[tokio::test]
async fn shared_current_role_session_is_allowed_but_sealed_resume_group_cannot_be_reused() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context(),
        "shared-role-fixture".into(),
        true,
    )
    .unwrap();
    for (i, label) in crate::room_learner::qualification::CONDITIONS
        .into_iter()
        .enumerate()
    {
        phase_at_group(
            &rt,
            label,
            i as u32,
            START,
            None,
            Some("one-declared-train-session"),
        );
    }
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 8,
            learning_rate: 0.5,
            hidden_dim: 2,
            embedding_dim: 2,
            temporal_tokens: 2,
            ..FitConfig::default()
        },
    )
    .unwrap();
    assert_eq!(super::tests::finished(&rt).await["state"], "ready");
    for (i, label) in crate::room_learner::qualification::CONDITIONS
        .into_iter()
        .enumerate()
    {
        phase_at_group(
            &rt,
            label,
            i as u32 + 4,
            START,
            None,
            Some("one-declared-heldout-session"),
        );
    }
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    assert_eq!(q.receipt.report.declared_session_count, 1);
    assert!(!q.receipt.independence_verified);
    let cold = restore(dir.path());
    assert!(cold
        .begin_phase(
            &cold.mutation_guard().unwrap(),
            PhaseRequest {
                trial_id: "actually-new-trial".into(),
                session_id: "one-declared-heldout-session".into(),
                role: DataRole::HeldOut,
                label: RoomLabel::Empty,
                duration_ms: 12000,
                lead_in_ms: 0,
                conditions_confirmed: true
            },
            START + 200_000_000_000
        )
        .is_err());
    assert!(!cold.capture_active().unwrap());
}
#[tokio::test]
async fn phase_seal_waits_for_durable_fence_and_pending_retry_keeps_original_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (model, proof) = current(&rt);
    let directory = proof_path(dir.path(), &model)
        .parent()
        .unwrap()
        .join(format!("managed-fences-{}", proof.model_sha256));
    let head = directory.join("managed-sealed-phase-head-v1.json");
    let old_head = std::fs::read(&head).unwrap();
    let old = rt.store.load_managed_capture_fence(&model, &proof).unwrap();
    let end = prepare_phase_at(&rt, RoomLabel::Empty, 8, START, None, None);
    crate::companion::atomic_private_write(&head, b"{").unwrap();
    assert!(rt
        .finish_phase(&rt.mutation_guard().unwrap(), end, true)
        .is_err());
    assert!(rt.capture_active().unwrap());
    assert_eq!(
        rt.inner
            .lock()
            .unwrap()
            .capture
            .as_ref()
            .unwrap()
            .history()
            .len(),
        8
    );
    crate::companion::atomic_private_write(&head, &old_head).unwrap();
    let mut completed = rt.inner.lock().unwrap().capture.as_ref().unwrap().clone();
    completed.finish_phase(end).unwrap();
    let pending = crate::room_learner::managed_capture_fences::ManagedCaptureFence::issue(
        &proof,
        Some(&old),
        completed.managed_fence_seed(&proof).unwrap(),
    )
    .unwrap();
    crate::companion::atomic_private_write(
        &directory.join(format!("{}.json", pending.fence_sha256)),
        &serde_json::to_vec(&pending).unwrap(),
    )
    .unwrap();
    // Unlabelled packets after a completed phase don't replace the first sealed
    // phase record's timestamps when the finish acknowledgement is retried.
    let values = vec![2.0; 8];
    let last = &pending.seed.sources[0];
    rt.ingest(
        RawSourceFrame {
            source_id: last.source_id,
            grid_id: "ht64-canonical8",
            frequency_hz: 2_437_000_000,
            bandwidth_hz: 20_000_000,
            sequence: last.sequence + 1,
            amplitudes: &values,
        },
        end + 1,
    )
    .unwrap();
    rt.finish_phase(&rt.mutation_guard().unwrap(), end + 2, true)
        .unwrap();
    assert!(!rt.capture_active().unwrap());
    assert_eq!(
        rt.store.load_managed_capture_fence(&model, &proof).unwrap(),
        pending
    );
}
#[tokio::test]
async fn managed_proof_and_initial_fence_must_commit_before_candidate_pointer_and_retry_is_immutable(
) {
    let source_dir = tempfile::tempdir().unwrap();
    let source = super::selection_tests::fitted(source_dir.path()).await;
    let (model, original) = current(&source);
    let target_dir = tempfile::tempdir().unwrap();
    let target = runtime(target_dir.path());
    let proof_file = proof_path(target_dir.path(), &model);
    crate::companion::private_dir(proof_file.parent().unwrap()).unwrap();
    let fence_directory = proof_file
        .parent()
        .unwrap()
        .join(format!("managed-fences-{}", original.model_sha256));
    crate::companion::atomic_private_write(&fence_directory, b"not a directory").unwrap();
    assert!(target
        .store
        .save_managed_candidate(&model, original.seed.clone())
        .is_err());
    assert!(target
        .store
        .load_candidate(model.context())
        .unwrap()
        .is_none());
    assert!(!proof_file
        .parent()
        .unwrap()
        .join("candidate-v1.json")
        .exists());
    let first = std::fs::read(&proof_file).unwrap();
    std::fs::remove_file(&fence_directory).unwrap();
    let mut later = original.seed.clone();
    for source in &mut later.sources {
        source.ordinal += 1;
        source.sequence = source.sequence.wrapping_add(1);
        source.last_time_ns += 1;
        source.admitted_packets += 1;
    }
    later.last_seen_ns += 1;
    target.store.save_managed_candidate(&model, later).unwrap();
    assert_eq!(std::fs::read(&proof_file).unwrap(), first);
    assert_eq!(
        target
            .store
            .load_managed_training_proof(&model)
            .unwrap()
            .unwrap(),
        original
    );
    let fence = target
        .store
        .load_managed_capture_fence(&model, &original)
        .unwrap();
    assert_eq!(fence.revision, 0);
    assert_eq!(fence.seed, original.seed);
    assert!(target
        .store
        .load_candidate(model.context())
        .unwrap()
        .is_some());
}
#[tokio::test]
async fn malformed_metadata_checkpoint_fails_closed_without_integer_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    let (_, proof) = current(&rt);
    let capture = CaptureSession::resume_managed_proof(&proof).unwrap();
    let at = proof.seed.last_seen_ns + 10;
    let mut value = serde_json::to_value(capture.checkpoint(at).unwrap()).unwrap();
    value["metadata_lineage"]["phase_claims"][0]["cohort_count"] = json!(u64::MAX);
    let snapshot = serde_json::from_value(value).unwrap();
    let result = std::panic::catch_unwind(|| {
        CaptureSession::restore_checkpoint(snapshot, &proof.seed.context, at + 1)
    });
    assert!(
        result.is_ok(),
        "malformed metadata panicked before bounded validation"
    );
    assert!(result.unwrap().is_err());
}
#[tokio::test]
async fn cold_resume_preserves_total_cohort_budget_and_refuses_before_operator_capture() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::selection_tests::fitted(dir.path()).await;
    for index in 8..32 {
        phase_at(&rt, RoomLabel::Empty, index, START, None);
    }
    let cold = restore(dir.path());
    let result = cold.begin_phase(
        &cold.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: "fresh-but-over-budget".into(),
            session_id: "new-group".into(),
            role: DataRole::HeldOut,
            label: RoomLabel::Empty,
            duration_ms: 12000,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        START + 500_000_000_000,
    );
    assert!(result.unwrap_err().contains("capacity"));
    assert!(!cold.capture_active().unwrap());
}
