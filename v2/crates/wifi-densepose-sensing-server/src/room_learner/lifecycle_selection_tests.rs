//! SYNTHETIC managed packets exercise the actual qualification/selection lifecycle.
//! No hardware, physical accuracy or operator-ground-truth validation is claimed.
use super::tests::{context, finished, phase, runtime, train_phases, START};
use super::*;
use crate::room_learner::{
    qualification::QualificationEnvelope, selection::SelectedBinaryEvidence,
};

pub(super) async fn fitted(root: &Path) -> Arc<RoomLearningRuntime> {
    let rt = runtime(root);
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context(),
        "selection-fixture".into(),
        true,
    )
    .unwrap();
    train_phases(&rt);
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
    let status = finished(&rt).await;
    assert_eq!(status["state"], "ready", "{status}");
    for (i, label) in [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ]
    .into_iter()
    .enumerate()
    {
        phase(&rt, DataRole::HeldOut, label, i as u32 + 4);
    }
    rt
}
pub(super) fn feed(rt: &RoomLearningRuntime, at: u64, sources: &[u8], gain: f64) -> u64 {
    feed_sequence(rt, at, sources, gain, 10_000)
}
pub(super) fn feed_sequence(
    rt: &RoomLearningRuntime,
    at: u64,
    sources: &[u8],
    gain: f64,
    sequence_base: u32,
) -> u64 {
    for i in 0..8u32 {
        for source in sources {
            let values: Vec<f64> = (0..8)
                .map(|bin| {
                    gain + f64::from(*source) * 0.3 + bin as f64 * 0.04 + f64::from(i) * 0.003
                })
                .collect();
            rt.ingest(
                RawSourceFrame {
                    source_id: *source,
                    grid_id: "ht64-canonical8",
                    frequency_hz: 2_437_000_000,
                    bandwidth_hz: 20_000_000,
                    sequence: sequence_base + i,
                    amplitudes: &values,
                },
                at + u64::from(i) * 10_000_000 + u64::from(*source),
            )
            .unwrap();
        }
    }
    at + 80_000_000
}
fn ready(value: SelectedBinaryEvidence) -> crate::room_learner::selection::SelectedModelIdentity {
    match value {
        SelectedBinaryEvidence::Ready { identity, .. } => identity,
        other => panic!("expected ready: {other:?}"),
    }
}
#[tokio::test]
async fn qualification_selection_cold_restore_deselect_and_exact_rollback_compose() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    assert!(matches!(
        rt.selected_binary_evidence(START + 104_000_000_000, Some("synthetic-room")),
        SelectedBinaryEvidence::None
    ));
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    assert_eq!(q.receipt.label_provenance, "managed-sealed-history-v1");
    assert!(!q.receipt.physically_validated && !q.receipt.independence_verified);
    assert_eq!(q.receipt.criteria.len(), 4);
    let evaltime = q.receipt.evaluated_at_unix_ms;
    let oldguard = rt.mutation_guard().unwrap();
    let selected = rt
        .select_model(&oldguard, &q.receipt.model_sha256, &q.receipt_sha256)
        .unwrap();
    assert!(rt.deselect_model(&oldguard).is_err());
    let first = ready(rt.selected_binary_evidence(START + 103_000_000_000, Some("synthetic-room")));
    assert_eq!(first.selection_sha256, selected.receipt_sha256);
    assert!(matches!(
        rt.selected_binary_evidence(START + 103_000_000_000, Some("wrong-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    assert!(matches!(
        rt.selected_binary_evidence(START + 200_000_000_000, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));

    let cold = runtime(dir.path());
    let snapshot = cold.selection_snapshot(cold.receiver_now_ns(), Some("synthetic-room"));
    assert_eq!(
        snapshot.selected.unwrap().selection_sha256,
        selected.receipt_sha256
    );
    assert_eq!(
        snapshot.qualification.unwrap().evaluated_at_unix_ms,
        evaltime
    );
    assert!(matches!(
        snapshot.current,
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    // Stored selection survives cold load, but historical capture buffers do not.
    let at = cold.receiver_now_ns();
    let now = feed(&cold, at, &[1, 2], 2.0);
    let a = cold.selected_binary_evidence(now, Some("synthetic-room"));
    let b = cold.selected_binary_evidence(now + 10_000_000, Some("synthetic-room"));
    match (&a, &b) {
        (
            SelectedBinaryEvidence::Ready {
                observation_digest: x,
                observed_at_unix_ms: t,
                ..
            },
            SelectedBinaryEvidence::Ready {
                observation_digest: y,
                observed_at_unix_ms: u,
                ..
            },
        ) => {
            assert_eq!(x, y);
            assert_eq!(t, u);
            assert!(*t < now / 1_000_000)
        }
        _ => panic!("fresh repeated proof unavailable: {a:?}, {b:?}"),
    }
    assert_eq!(ready(a).model_sha256, q.receipt.model_sha256);
    let deselected = cold
        .deselect_model(&cold.mutation_guard().unwrap())
        .unwrap();
    assert!(matches!(
        cold.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::None
    ));
    let repeated = cold
        .deselect_model(&cold.mutation_guard().unwrap())
        .unwrap();
    assert_eq!(repeated, deselected);
    let rolled = cold
        .rollback_model(&cold.mutation_guard().unwrap())
        .unwrap();
    assert_eq!(
        rolled.receipt.selected.unwrap().model_sha256,
        q.receipt.model_sha256
    );
    assert_eq!(
        ready(cold.selected_binary_evidence(now, Some("synthetic-room"))).evaluated_at_unix_ms,
        evaltime
    );
    // Selection/model are not coupled to the 24-hour provisional-capture TTL.
    let tomorrow = feed_sequence(
        &cold,
        now + 25 * 60 * 60 * 1_000_000_000,
        &[1, 2],
        2.0,
        20_000,
    );
    assert_eq!(
        ready(cold.selected_binary_evidence(tomorrow, Some("synthetic-room"))).evaluated_at_unix_ms,
        evaltime
    );
    // Starting another candidate must never automatically replace the selected model.
    cold.configure(
        &cold.mutation_guard().unwrap(),
        context(),
        "new-candidate".into(),
        true,
    )
    .unwrap();
    assert_eq!(
        cold.selection_snapshot(now, Some("synthetic-room"))
            .selected
            .unwrap()
            .model_sha256,
        q.receipt.model_sha256
    );
    assert!(cold.status(now)["candidate_identity"].is_null());
    assert!(matches!(
        cold.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    let fresh = feed(&cold, tomorrow + 1_000_000_000, &[1, 2], 2.0);
    assert!(cold.qualify(&cold.mutation_guard().unwrap()).is_err());
    assert!(
        matches!(
            cold.selected_binary_evidence(fresh, Some("synthetic-room")),
            SelectedBinaryEvidence::Ready { .. }
        ),
        "incomplete candidate must not revoke the existing selected model"
    );
}

#[tokio::test]
async fn qualification_requires_real_lineage_all_conditions_and_matching_sealed_history() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    let state = rt.inner.lock().unwrap();
    let model = state.model.as_ref().unwrap();
    let capture = state.capture.as_ref().unwrap();
    let dataset = capture.validated_dataset().unwrap();
    let evaluation = rt
        .store
        .evaluate_candidate(model, &dataset, DataRole::HeldOut)
        .unwrap();
    let good =
        QualificationEnvelope::issue(model, &dataset, capture.history(), &evaluation).unwrap();
    // Actual inference reports from deliberately wrong/abstaining model heads,
    // not hand-edited success metrics, must fail the per-condition policy.
    for (bias, reason) in [
        (-100.0, "occupied sensitivity"),
        (100.0, "false presence"),
        (0.0, "coverage"),
    ] {
        let mut wrong = (**model).clone();
        wrong.head_weights.fill(0.0);
        *wrong.head_weights.last_mut().unwrap() = bias;
        let report = wrong.evaluate(dataset.held_out()).unwrap();
        let error = crate::room_learner::qualification::criteria(&report, &[4; 4]).unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
    let incomplete: Vec<_> = dataset
        .held_out()
        .iter()
        .filter(|c| c.label != RoomLabel::AdjacentOnly)
        .cloned()
        .collect();
    let report = model.evaluate(&incomplete).unwrap();
    assert!(
        crate::room_learner::qualification::criteria(&report, &[4; 4])
            .unwrap_err()
            .contains("AdjacentOnly")
    );
    let mut missing = capture.history().to_vec();
    missing.retain(|h| h.label != RoomLabel::AdjacentOnly);
    assert!(QualificationEnvelope::issue(model, &dataset, &missing, &evaluation).is_err());
    let no_train = crate::room_learner::observation::ValidatedDataset::new(
        context(),
        dataset.held_out().to_vec(),
    )
    .unwrap();
    assert!(
        QualificationEnvelope::issue(model, &no_train, capture.history(), &evaluation).is_err()
    );
    let mut tampered = good.clone();
    tampered.receipt.policy.minimum_evaluated_cohort_coverage = 0.0;
    tampered.receipt.policy_sha256 =
        crate::room_learner::qualification::hash(&tampered.receipt.policy).unwrap();
    tampered.receipt_sha256 = crate::room_learner::qualification::hash(&tampered.receipt).unwrap();
    assert!(tampered.validate(model, &evaluation).is_err());
    let mut overflow = good.clone();
    overflow.receipt.criteria[0].train_cohorts = usize::MAX;
    overflow.receipt_sha256 = crate::room_learner::qualification::hash(&overflow.receipt).unwrap();
    assert!(overflow.validate(model, &evaluation).is_err());
    let mut other = good;
    other.receipt.model_sha256 = "b".repeat(64);
    other.receipt_sha256 = crate::room_learner::qualification::hash(&other.receipt).unwrap();
    assert!(other.validate(model, &evaluation).is_err());
}

#[tokio::test]
async fn selection_corruption_owner_mismatch_missing_sources_and_ood_never_fall_back() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    rt.select_model(
        &rt.mutation_guard().unwrap(),
        &q.receipt.model_sha256,
        &q.receipt_sha256,
    )
    .unwrap();
    let cold = runtime(dir.path());
    let at = cold.receiver_now_ns();
    let now = feed(&cold, at, &[1], 2.0);
    assert!(matches!(
        cold.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    let ood = runtime(dir.path());
    let now = feed(&ood, ood.receiver_now_ns(), &[1, 2], 1_000_000.0);
    assert!(matches!(
        ood.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    let path = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner/model-selection-v1.json");
    let original = std::fs::read(&path).unwrap();
    let foreign_dir = dir.path().join("b".repeat(64)).join("SensingServer");
    let foreign =
        RoomLearningRuntime::new(&foreign_dir, &"b".repeat(64), "synthetic-install").unwrap();
    drop(foreign);
    std::fs::write(
        foreign_dir.join("room-learner/model-selection-v1.json"),
        &original,
    )
    .unwrap();
    let foreign =
        RoomLearningRuntime::new(&foreign_dir, &"b".repeat(64), "synthetic-install").unwrap();
    assert!(matches!(
        foreign.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    std::fs::write(&path, b"broken").unwrap();
    let broken = runtime(dir.path());
    assert!(matches!(
        broken.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    assert!(rt
        .store
        .load_exact_candidate(&context(), &q.receipt.model_sha256)
        .is_ok());
}

/// Genuine managed capture path with synthetic, deliberately selectable signal gain.
pub(super) fn phase_override(
    rt: &RoomLearningRuntime,
    role: DataRole,
    label: RoomLabel,
    index: u32,
    gain: f64,
) -> u64 {
    let at = START + u64::from(index) * 13_000_000_000;
    rt.begin_phase(
        &rt.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: format!("selection-trial-{index}"),
            session_id: format!("selection-session-{index}"),
            role,
            label,
            duration_ms: 12_000,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        at,
    )
    .unwrap();
    let context = rt.context().unwrap();
    for quarter in 0..4u32 {
        for sample in 0..8u32 {
            for source in &context.sources {
                let values: Vec<f64> = (0..source.feature_count)
                    .map(|bin| {
                        gain + f64::from(source.source_id) * 0.3
                            + bin as f64 * 0.04
                            + f64::from(sample) * 0.003
                    })
                    .collect();
                let t = at
                    + u64::from(quarter) * 3_000_000_000
                    + 1_000_000
                    + u64::from(sample) * 10_000_000
                    + u64::from(source.source_id);
                rt.ingest(
                    RawSourceFrame {
                        source_id: source.source_id,
                        grid_id: &source.grid_id,
                        frequency_hz: source.center_frequency_hz,
                        bandwidth_hz: source.bandwidth_hz,
                        sequence: index * 32 + quarter * 8 + sample + 1,
                        amplitudes: &values,
                    },
                    t,
                )
                .unwrap();
            }
        }
    }
    rt.finish_phase(&rt.mutation_guard().unwrap(), at + 12_000_000_000, true)
        .unwrap();
    at + 12_000_000_000
}
