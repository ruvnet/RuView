//! Synthetic measured-path counterexamples; no hardware qualification claims.
use super::selection_tests::{feed, feed_sequence, fitted, phase_override};
use super::tests::{context, finished, runtime, START};
use super::*;
use crate::room_learner::selection::SelectedBinaryEvidence;

#[tokio::test]
async fn newer_completed_failed_assessment_withdraws_persistent_selection_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    let selected = rt
        .select_model(
            &rt.mutation_guard().unwrap(),
            &q.receipt.model_sha256,
            &q.receipt_sha256,
        )
        .unwrap();
    let model = rt.inner.lock().unwrap().model.as_ref().unwrap().clone();
    let original_assessment = rt.store.load_latest_assessment(&model).unwrap().unwrap();
    // Record deliberately mislabelled held-out empty evidence through actual commands.
    let now = phase_override(&rt, DataRole::HeldOut, RoomLabel::Empty, 8, 7.0);
    let error = rt.qualify(&rt.mutation_guard().unwrap()).unwrap_err();
    assert!(error.contains("false presence"), "{error}");
    let snapshot = rt.selection_snapshot(now, Some("synthetic-room"));
    assert_eq!(
        snapshot.selected.unwrap().selection_sha256,
        selected.receipt_sha256
    );
    assert!(snapshot.qualification.is_none());
    assert!(
        matches!(snapshot.current,SelectedBinaryEvidence::Unavailable{reason,..} if reason=="newer_qualification_failed")
    );
    assert!(rt
        .select_model(
            &rt.mutation_guard().unwrap(),
            &q.receipt.model_sha256,
            &q.receipt_sha256
        )
        .is_err());
    assert!(
        rt.store
            .save_assessment(&model, &original_assessment)
            .is_err(),
        "older pass must not erase a later completed failure"
    );
    let cold = runtime(dir.path());
    let now = feed(&cold, cold.receiver_now_ns(), &[1, 2], 2.0);
    let snapshot = cold.selection_snapshot(now, Some("synthetic-room"));
    assert_eq!(
        snapshot.selected.unwrap().selection_sha256,
        selected.receipt_sha256
    );
    assert!(
        matches!(snapshot.current,SelectedBinaryEvidence::Unavailable{reason,..} if reason=="newer_qualification_failed")
    );
    assert!(cold
        .store
        .load_qualification(
            cold.inner.lock().unwrap().model.as_ref().unwrap(),
            &q.receipt_sha256
        )
        .is_ok());
    // Restoring the explicit legacy control is permitted; history/model stay intact.
    cold.deselect_model(&cold.mutation_guard().unwrap())
        .unwrap();
    assert!(matches!(
        cold.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::None
    ));
}

#[tokio::test]
async fn rejected_selected_input_withdraws_old_window_until_all_sources_are_fresh_again() {
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
    let now = feed(&cold, at, &[1, 2], 2.0);
    assert!(matches!(
        cold.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Ready { .. }
    ));
    cold.record_native_rejection(99, "ht64-canonical8", "foreign source");
    cold.record_native_rejection(1, "other-grid", "unselected PHY");
    assert!(matches!(
        cold.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Ready { .. }
    ));
    cold.record_native_rejection(1, "ht64-canonical8", "invalid native frame");
    assert!(matches!(
        cold.selected_binary_evidence(now, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    let partial = feed_sequence(&cold, at + 200_000_000, &[1], 2.0, 20_000);
    assert!(matches!(
        cold.selected_binary_evidence(partial, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    let fresh = feed_sequence(&cold, at + 400_000_000, &[1, 2], 2.0, 30_000);
    assert!(matches!(
        cold.selected_binary_evidence(fresh, Some("synthetic-room")),
        SelectedBinaryEvidence::Ready { .. }
    ));
    assert!(cold
        .ingest(
            RawSourceFrame {
                source_id: 1,
                grid_id: "ht64-canonical8",
                frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                sequence: 30_007,
                amplitudes: &[2.3; 8]
            },
            fresh + 1
        )
        .is_err());
    assert!(matches!(
        cold.selected_binary_evidence(fresh + 1, Some("synthetic-room")),
        SelectedBinaryEvidence::Unavailable { .. }
    ));
}

#[tokio::test]
async fn fresh_selection_snapshot_rechecks_durable_artifacts_not_only_cached_model() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    rt.select_model(
        &rt.mutation_guard().unwrap(),
        &q.receipt.model_sha256,
        &q.receipt_sha256,
    )
    .unwrap();
    let now = START + 103_000_000_000;
    assert!(matches!(
        rt.selection_snapshot(now, Some("synthetic-room")).current,
        SelectedBinaryEvidence::Ready { .. }
    ));
    let path = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner")
        .join(context().digest())
        .join(format!("{}.json", q.receipt.model_sha256));
    let root = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner");
    let qualification = root
        .join(context().digest())
        .join("qualifications")
        .join(format!("{}.json", q.receipt_sha256));
    let pointer = root.join("model-selection-v1.json");
    let assessment = root
        .join(context().digest())
        .join("assessments")
        .join(format!("{}.latest.json", q.receipt.model_sha256));
    for damaged in [path, qualification, pointer, assessment] {
        let original = std::fs::read(&damaged).unwrap();
        std::fs::write(&damaged, b"corrupt durable evidence").unwrap();
        assert!(
            matches!(
                rt.selection_snapshot(now, Some("synthetic-room")).current,
                SelectedBinaryEvidence::Unavailable { .. }
            ),
            "{}",
            damaged.display()
        );
        assert!(rt
            .selection_snapshot(now, Some("synthetic-room"))
            .integrity_error
            .is_some());
        std::fs::write(&damaged, original).unwrap();
        assert!(matches!(
            rt.selection_snapshot(now, Some("synthetic-room")).current,
            SelectedBinaryEvidence::Ready { .. }
        ));
    }
}

#[tokio::test]
async fn canonical_56_feature_context_qualifies_through_actual_managed_training_commands() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let mut c = context();
    for s in &mut c.sources {
        s.feature_count = 56;
        s.grid_id = "ht64-canonical56".into();
    }
    rt.configure(
        &rt.mutation_guard().unwrap(),
        c,
        "canonical-width-fixture".into(),
        true,
    )
    .unwrap();
    let cases = [
        (RoomLabel::Empty, 2.0),
        (RoomLabel::OccupiedStatic, 6.0),
        (RoomLabel::OccupiedMoving, 7.0),
        (RoomLabel::AdjacentOnly, 2.3),
    ];
    for (i, (label, gain)) in cases.into_iter().enumerate() {
        phase_override(&rt, DataRole::Train, label, i as u32, gain);
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
    let training = finished(&rt).await;
    assert_eq!(training["state"], "ready", "{training}");
    for (i, (label, gain)) in cases.into_iter().enumerate() {
        phase_override(&rt, DataRole::HeldOut, label, i as u32 + 4, gain);
    }
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    rt.select_model(
        &rt.mutation_guard().unwrap(),
        &q.receipt.model_sha256,
        &q.receipt_sha256,
    )
    .unwrap();
    assert!(matches!(
        rt.selected_binary_evidence(START + 103_000_000_000, Some("synthetic-room")),
        SelectedBinaryEvidence::Ready { .. }
    ));
}

#[tokio::test]
async fn missing_authoritative_pointer_with_history_is_unknown_not_legacy_after_cold_load() {
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    let selected = rt
        .select_model(
            &rt.mutation_guard().unwrap(),
            &q.receipt.model_sha256,
            &q.receipt_sha256,
        )
        .unwrap();
    let root = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner");
    let pointer = root.join("model-selection-v1.json");
    let original = std::fs::read(&pointer).unwrap();
    let history = root
        .join("selections")
        .join(format!("{}.json", selected.receipt_sha256));
    let history_bytes = std::fs::read(&history).unwrap();
    std::fs::rename(&pointer, root.join("selection-pointer-backup.json")).unwrap();
    let cold = runtime(dir.path());
    let status = cold.selection_snapshot(cold.receiver_now_ns(), Some("synthetic-room"));
    assert!(status
        .integrity_error
        .as_deref()
        .is_some_and(|s| s.contains("history")));
    assert!(matches!(
        status.current,
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    assert!(!pointer.exists());
    assert_eq!(std::fs::read(&history).unwrap(), history_bytes);
    assert_eq!(
        std::fs::read(root.join("selection-pointer-backup.json")).unwrap(),
        original
    );
    // An actual explicit deselection pointer, unlike a missing pointer, proves legacy mode.
    std::fs::rename(root.join("selection-pointer-backup.json"), &pointer).unwrap();
    let restored = runtime(dir.path());
    restored
        .deselect_model(&restored.mutation_guard().unwrap())
        .unwrap();
    let deselected = runtime(dir.path());
    let status =
        deselected.selection_snapshot(deselected.receiver_now_ns(), Some("synthetic-room"));
    assert!(status.integrity_error.is_none());
    assert!(matches!(status.current, SelectedBinaryEvidence::None));
}

#[tokio::test]
async fn orphan_first_write_history_does_not_invent_a_selected_model_or_legacy_fallback() {
    use crate::room_learner::{
        qualification::hash,
        selection::{SelectionEnvelope, SelectionReceipt, SelectionTarget},
    };
    let dir = tempfile::tempdir().unwrap();
    let rt = fitted(dir.path()).await;
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    let root = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner");
    let history = root.join("selections");
    crate::companion::private_dir(&history).unwrap();
    // Simulate the persisted boundary after immutable history but before the
    // first authoritative pointer. No successful selection is asserted.
    let receipt = SelectionReceipt {
        schema: "ruview.room-learner.model-selection.v1".into(),
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-install".into(),
        revision: 1,
        selected_at_unix_ms: rt.receiver_now_ns() / 1_000_000,
        selected: Some(SelectionTarget {
            context: context(),
            model_sha256: q.receipt.model_sha256.clone(),
            model_version: q.receipt.model_version,
            qualification_sha256: q.receipt_sha256.clone(),
        }),
        previous: None,
    };
    let envelope = SelectionEnvelope {
        receipt_sha256: hash(&receipt).unwrap(),
        receipt,
    };
    let bytes = serde_json::to_vec(&envelope).unwrap();
    let record = history.join(format!("{}.json", envelope.receipt_sha256));
    crate::companion::atomic_private_write(&record, &bytes).unwrap();
    let pointer = root.join("model-selection-v1.json");
    std::fs::create_dir(&pointer).unwrap();
    assert!(crate::companion::atomic_private_write(&pointer, &bytes).is_err());
    std::fs::remove_dir(&pointer).unwrap();
    let cold = runtime(dir.path());
    let status = cold.selection_snapshot(cold.receiver_now_ns(), Some("synthetic-room"));
    assert!(status.selected.is_none());
    assert!(status.integrity_error.is_some());
    assert!(matches!(
        status.current,
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    assert!(!pointer.exists());
    assert_eq!(std::fs::read(record).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn never_selected_empty_history_is_legacy_but_unsafe_history_is_unknown() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    assert!(matches!(
        rt.selection_snapshot(START, Some("synthetic-room")).current,
        SelectedBinaryEvidence::None
    ));
    let root = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner");
    let history = root.join("selections");
    crate::companion::private_dir(&history).unwrap();
    assert!(matches!(
        runtime(dir.path())
            .selection_snapshot(START, Some("synthetic-room"))
            .current,
        SelectedBinaryEvidence::None
    ));
    std::fs::set_permissions(&history, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert!(matches!(
        runtime(dir.path())
            .selection_snapshot(START, Some("synthetic-room"))
            .current,
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    std::fs::set_permissions(&history, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir(&history).unwrap();
    let external = dir.path().join("empty-history-target");
    crate::companion::private_dir(&external).unwrap();
    symlink(&external, &history).unwrap();
    assert!(matches!(
        runtime(dir.path())
            .selection_snapshot(START, Some("synthetic-room"))
            .current,
        SelectedBinaryEvidence::Unavailable { .. }
    ));
    assert!(std::fs::symlink_metadata(&history)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(std::fs::read_dir(external).unwrap().count(), 0);
}
