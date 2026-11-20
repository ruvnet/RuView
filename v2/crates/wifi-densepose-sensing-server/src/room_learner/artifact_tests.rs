//! Actual trained model + real private filesystem primitives; RF is SYNTHETIC.
use super::*;
use crate::room_learner::{observation::*, training::FitConfig};
use ruview_unified::RfModality;
use std::sync::OnceLock;

fn owner() -> String {
    "a".repeat(64)
}
fn fitted(room: &str, revision: u64, epochs: usize) -> RoomModel {
    let context = CaptureContext {
        owner_scope: owner(),
        installation_id: "synthetic-install".into(),
        room_digest: room.into(),
        revision,
        sources: (1..=2)
            .map(|id| SourceSpec {
                source_id: id,
                device_id: format!("synthetic-device-{id}"),
                grid_id: "synthetic-4-bin".into(),
                feature_count: 4,
                signal_kind: RfModality::Synthetic,
                link_id: format!("link-{id}"),
                link_index: 0,
                center_frequency_hz: 2_400_000_000,
                bandwidth_hz: 20_000_000,
                geometry: None,
            })
            .collect(),
    };
    let labels = [
        RoomLabel::Empty,
        RoomLabel::AdjacentOnly,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
    ];
    let rows = (1..=8u64)
        .map(|id| {
            let label = labels[(id as usize - 1) % 4];
            let views = context
                .sources
                .iter()
                .map(|s| {
                    let shift = match label {
                        RoomLabel::Empty => 0.0,
                        RoomLabel::AdjacentOnly => {
                            if s.source_id == 1 {
                                1.5
                            } else {
                                -1.5
                            }
                        }
                        _ => 1.5,
                    };
                    SourceView {
                        source_id: s.source_id,
                        device_id: s.device_id.clone(),
                        grid_id: s.grid_id.clone(),
                        signal_kind: s.signal_kind,
                        link_id: s.link_id.clone(),
                        link_index: 0,
                        center_frequency_hz: s.center_frequency_hz,
                        bandwidth_hz: s.bandwidth_hz,
                        geometry: None,
                        samples: (0..4)
                            .map(|t| {
                                (0..4)
                                    .map(|b| {
                                        10.0 + shift
                                            + t as f32 * 0.02
                                            + b as f32 * 0.01
                                            + id as f32 * 0.002
                                    })
                                    .collect()
                            })
                            .collect(),
                        frame_ids: (0..4).map(|t| u128::from(id) * 10 + t + 1).collect(),
                        sequence_ids: vec![1, 2, 3, 4],
                        first_time_ns: id * 1000 + 1,
                        last_time_ns: id * 1000 + 100,
                        clock: ClockInfo {
                            kind: ClockKind::Unknown,
                            uncertainty_ns: u64::MAX,
                        },
                        sample_age_ns: 0,
                        phase: PhaseAvailability::MagnitudeOnly,
                        provenance: CaptureProvenance::Synthetic,
                    }
                })
                .collect();
            CaptureCohort {
                id: format!("event-{id}"),
                context_digest: context.digest(),
                trial_id: format!("trial-{id}"),
                session_id: format!("session-{id}"),
                role: DataRole::Train,
                label,
                views,
                teacher: None,
                provenance: CaptureProvenance::Synthetic,
            }
        })
        .collect();
    RoomModel::fit(
        &ValidatedDataset::new(context, rows).unwrap(),
        FitConfig {
            epochs,
            ..FitConfig::default()
        },
    )
    .unwrap()
    .0
}
fn model() -> RoomModel {
    static MODEL: OnceLock<RoomModel> = OnceLock::new();
    MODEL
        .get_or_init(|| fitted("synthetic-room-a", 1, 2))
        .clone()
}
fn store(root: &Path) -> ArtifactStore {
    ArtifactStore::new(
        &root.join(owner()).join("SensingServer"),
        &owner(),
        "synthetic-install",
    )
    .unwrap()
}
fn immutable(store: &ArtifactStore, receipt: &CandidateReceipt) -> PathBuf {
    store
        .root
        .join(&receipt.context_digest)
        .join(format!("{}.json", receipt.model_sha256))
}
fn pointer(store: &ArtifactStore, model: &RoomModel) -> PathBuf {
    store.root.join(model.context().digest()).join(POINTER)
}
#[test]
fn trained_model_private_save_cold_reload_inference_round_trip_has_no_raw_data() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let model = model();
    assert!(store.load_candidate(model.context()).unwrap().is_none());
    let receipt = store.save_candidate(&model).unwrap();
    let envelope = private_read(&immutable(&store, &receipt)).unwrap();
    let text = String::from_utf8(envelope).unwrap();
    for raw in ["samples", "frame_ids", "sequence_ids", "trial_id"] {
        assert!(!text.contains(raw));
    }
    let cold = ArtifactStore::new(&store.data_dir, &owner(), "synthetic-install").unwrap();
    let restored = cold.load_candidate(model.context()).unwrap().unwrap();
    assert_eq!(model.parameters(), restored.parameters());
    assert_eq!(
        restored.training_manifest().training_provenance,
        [CaptureProvenance::Synthetic]
    );
    assert!(!restored.training_manifest().measurement_clock_available);
    assert_eq!(model.export().unwrap(), restored.export().unwrap());
    let context = model.context();
    let live = CaptureCohort {
        id: "live-input".into(),
        context_digest: context.digest(),
        trial_id: "live-trial".into(),
        session_id: "live-session".into(),
        role: DataRole::Inference,
        label: RoomLabel::Unknown,
        teacher: None,
        provenance: CaptureProvenance::Synthetic,
        views: context
            .sources
            .iter()
            .map(|s| SourceView {
                source_id: s.source_id,
                device_id: s.device_id.clone(),
                grid_id: s.grid_id.clone(),
                signal_kind: s.signal_kind,
                link_id: s.link_id.clone(),
                link_index: 0,
                center_frequency_hz: s.center_frequency_hz,
                bandwidth_hz: s.bandwidth_hz,
                geometry: None,
                samples: vec![vec![11.5; 4]; 4],
                frame_ids: vec![101, 102, 103, 104],
                sequence_ids: vec![1, 2, 3, 4],
                first_time_ns: 1000,
                last_time_ns: 1100,
                clock: ClockInfo {
                    kind: ClockKind::Unknown,
                    uncertainty_ns: u64::MAX,
                },
                sample_age_ns: 0,
                phase: PhaseAvailability::MagnitudeOnly,
                provenance: CaptureProvenance::Synthetic,
            })
            .collect(),
    };
    assert_eq!(
        model.infer(&live).occupied_probability,
        restored.infer(&live).occupied_probability
    );
    assert!(restored.infer(&live).occupied_probability.is_some());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(immutable(&store, &receipt))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(pointer(&store, &model))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&store.root).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
#[test]
fn independent_rooms_revisions_and_replaced_candidates_retain_all_prior_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let a = model();
    let b = fitted("synthetic-room-b", 1, 2);
    let a2 = fitted("synthetic-room-a", 2, 2);
    let ar = store.save_candidate(&a).unwrap();
    let before = private_read(&immutable(&store, &ar)).unwrap();
    store.save_candidate(&b).unwrap();
    store.save_candidate(&a2).unwrap();
    for expected in [&a, &b, &a2] {
        assert_eq!(
            store
                .load_candidate(expected.context())
                .unwrap()
                .unwrap()
                .parameters(),
            expected.parameters()
        );
    }
    let replacement = fitted("synthetic-room-a", 1, 3);
    let next = store.save_candidate(&replacement).unwrap();
    assert_ne!(ar.model_sha256, next.model_sha256);
    assert_eq!(private_read(&immutable(&store, &ar)).unwrap(), before);
    assert_eq!(
        store
            .load_candidate(a.context())
            .unwrap()
            .unwrap()
            .parameters(),
        replacement.parameters()
    );
    assert!(immutable(&store, &next).is_file());
}
#[test]
fn corrupt_pointer_or_immutable_bytes_fail_closed_and_preserve_existing_candidate() {
    for pointer_corruption in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        let model = model();
        let receipt = store.save_candidate(&model).unwrap();
        let path = if pointer_corruption {
            pointer(&store, &model)
        } else {
            immutable(&store, &receipt)
        };
        atomic_private_write(&path, b"corrupt candidate bytes").unwrap();
        let before = private_read(&path).unwrap();
        assert!(store.load_candidate(model.context()).is_err());
        assert!(store.save_candidate(&model).is_err());
        assert_eq!(private_read(&path).unwrap(), before);
    }
}
#[test]
fn model_hash_tampering_wrong_context_and_installation_cannot_be_adopted() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let model = model();
    let receipt = store.save_candidate(&model).unwrap();
    let mut envelope: serde_json::Value =
        serde_json::from_slice(&private_read(&immutable(&store, &receipt)).unwrap()).unwrap();
    envelope["learned_model_json"] = serde_json::Value::String("{}".into());
    atomic_private_write(
        &immutable(&store, &receipt),
        &serde_json::to_vec(&envelope).unwrap(),
    )
    .unwrap();
    assert!(store
        .load_candidate(model.context())
        .unwrap_err()
        .contains("integrity"));
    let mut other = model.context().clone();
    other.installation_id = "other-install".into();
    assert!(store.load_candidate(&other).is_err());
    let wrong = ArtifactStore::new(&store.data_dir, &owner(), "other-install").unwrap();
    assert!(wrong.save_candidate(&model).is_err());
    other = model.context().clone();
    other.owner_scope = "b".repeat(64);
    assert!(store.load_candidate(&other).is_err());
    assert!(ArtifactStore::new(&store.data_dir, &"b".repeat(64), "synthetic-install").is_err());
}
#[test]
fn existing_account_witness_and_accepted_control_models_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let witness = store.data_dir.join("account-owner-v1.json");
    let before = private_read(&witness).unwrap();
    let accepted = store.data_dir.join("accepted-model.json");
    let control = store.data_dir.join("control-model.json");
    atomic_private_write(&accepted, b"accepted bytes").unwrap();
    atomic_private_write(&control, b"control bytes").unwrap();
    store.save_candidate(&model()).unwrap();
    assert_eq!(private_read(&witness).unwrap(), before);
    assert_eq!(private_read(&accepted).unwrap(), b"accepted bytes");
    assert_eq!(private_read(&control).unwrap(), b"control bytes");
    atomic_private_write(&witness, b"changed owner witness").unwrap();
    assert!(store.save_candidate(&model()).is_err());
    assert_eq!(private_read(&witness).unwrap(), b"changed owner witness");
}
#[cfg(unix)]
#[test]
fn symlink_candidate_pointer_context_and_root_are_rejected_without_touching_victims() {
    use std::os::unix::fs::symlink;
    for level in ["pointer", "immutable", "context", "root"] {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path());
        let model = model();
        let receipt = store.save_candidate(&model).unwrap();
        let victim = temp.path().join("victim");
        atomic_private_write(&victim, b"victim private bytes").unwrap();
        let path = match level {
            "pointer" => pointer(&store, &model),
            "immutable" => immutable(&store, &receipt),
            "context" => store.root.join(model.context().digest()),
            _ => store.root.clone(),
        };
        let preserved = path.with_extension("preserved");
        std::fs::rename(&path, &preserved).unwrap();
        symlink(&victim, &path).unwrap();
        assert!(
            store.load_candidate(model.context()).is_err(),
            "accepted symlink {level}"
        );
        assert!(
            store.save_candidate(&model).is_err(),
            "overwrote symlink {level}"
        );
        assert_eq!(private_read(&victim).unwrap(), b"victim private bytes");
        assert!(std::fs::symlink_metadata(path)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
#[cfg(unix)]
#[test]
fn insecure_permissions_and_disk_path_failure_leave_previous_model_unchanged() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let model = model();
    let receipt = store.save_candidate(&model).unwrap();
    let path = pointer(&store, &model);
    let before = private_read(&path).unwrap();
    let immutable_before = private_read(&immutable(&store, &receipt)).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.load_candidate(model.context()).is_err());
    assert!(store.save_candidate(&model).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(private_read(&path).unwrap(), before);
    // A filesystem obstacle at the destination refuses publication and retains
    // the old pointer (moved aside by the fixture) and immutable learned bytes.
    let old = path.with_extension("old");
    std::fs::rename(&path, &old).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(store.save_candidate(&model).is_err());
    assert_eq!(private_read(&old).unwrap(), before);
    assert_eq!(
        private_read(&immutable(&store, &receipt)).unwrap(),
        immutable_before
    );
}
#[test]
fn claimed_qualification_and_unbounded_or_traversing_pointers_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(temp.path());
    let model = model();
    let receipt = store.save_candidate(&model).unwrap();
    let path = pointer(&store, &model);
    let original = private_read(&path).unwrap();
    let artifact_before = private_read(&immutable(&store, &receipt)).unwrap();
    let mut forged: serde_json::Value = serde_json::from_slice(&original).unwrap();
    forged["physically_validated"] = serde_json::json!(true);
    let mut traversing: serde_json::Value = serde_json::from_slice(&original).unwrap();
    traversing["model_sha256"] = serde_json::json!("../../accepted-model");
    for bytes in [
        serde_json::to_vec(&forged).unwrap(),
        serde_json::to_vec(&traversing).unwrap(),
        vec![b' '; LIMIT + 1],
    ] {
        atomic_private_write(&path, &bytes).unwrap();
        assert!(store.load_candidate(model.context()).is_err());
        assert!(store.save_candidate(&model).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            private_read(&immutable(&store, &receipt)).unwrap(),
            artifact_before
        );
    }
}
#[test]
fn unowned_legacy_data_and_blocked_context_paths_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let legacy = temp.path().join(owner()).join("SensingServer");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("legacy-model"), b"unowned bytes").unwrap();
    assert!(ArtifactStore::new(&legacy, &owner(), "synthetic-install").is_err());
    assert_eq!(
        std::fs::read(legacy.join("legacy-model")).unwrap(),
        b"unowned bytes"
    );
    assert!(!legacy.join("account-owner-v1.json").exists());
    let other = tempfile::tempdir().unwrap();
    let store = store(other.path());
    let model = model();
    let blocked = store.root.join(model.context().digest());
    atomic_private_write(&blocked, b"existing obstacle").unwrap();
    assert!(store.save_candidate(&model).is_err());
    assert_eq!(private_read(&blocked).unwrap(), b"existing obstacle");
}

#[test]
fn catalog_retains_exact_saved_source_contexts_and_quarantines_other_corrupt_entries() {
    let temp=tempfile::tempdir().unwrap();let store=store(temp.path());let a=model();
    let receipt=store.save_candidate(&a).unwrap();
    let unknown=store.root.join("f".repeat(64));std::fs::create_dir(&unknown).unwrap();
    atomic_private_write(&unknown.join(POINTER),b"bad pointer").unwrap();
    let listed=store.catalog().unwrap();
    assert_eq!(listed.candidates.len(),1);assert_eq!(listed.unreadable_contexts,1);
    assert_eq!(listed.candidates[0].context,*a.context());
    assert_eq!(listed.candidates[0].receipt,receipt);
    assert!(store.load_candidate(a.context()).unwrap().is_some());
    assert_eq!(private_read(&unknown.join(POINTER)).unwrap(),b"bad pointer");
    let encoded=serde_json::to_string(&listed).unwrap();
    assert!(!encoded.contains("encoder_weights") && !encoded.contains("samples"));
}
