//! SYNTHETIC native inputs + actual fitter/filesystem; no runtime or user data.
use super::*;
use crate::room_learner::{
    capture::{CaptureSession, PhaseRequest, RawSourceFrame},
    native::{canonicalize_esp32, CanonicalPacket, NativePacketInput},
    observation::*,
    selection::{SelectionEnvelope, SelectionReceipt},
    temporal_acquisition::{ExperimentalPlanId, ReceiverCadenceBand, WindowAdmission},
    temporal_model_input::TemporalTrainingSession,
    training::FitConfig,
};
use std::{collections::BTreeMap, sync::OnceLock};
use wifi_densepose_hardware::{Adr018Flags, PpduType};
fn context() -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-temporal-store".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=2)
            .map(|source_id| SourceSpec {
                source_id,
                device_id: format!("esp32-node-{source_id}"),
                grid_id: "ant1-bins64-ppdu0-usable-drop2-linear-v2".into(),
                feature_count: 56,
                signal_kind: ruview_unified::RfModality::WifiCsi,
                center_frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                link_id: format!("native-{source_id}"),
                link_index: 0,
                geometry: None,
            })
            .collect(),
    }
}
fn packet(source: u8, sequence: u32, time: u64, label: RoomLabel, row: usize) -> CanonicalPacket {
    let gain = match label {
        RoomLabel::Empty => 3,
        RoomLabel::AdjacentOnly => 4,
        RoomLabel::OccupiedStatic => 8,
        RoomLabel::OccupiedMoving => 10 + (row % 2) as u8,
        RoomLabel::Unknown => 8,
    };
    let bytes = (0..64)
        .flat_map(|bin| [gain + source + (bin % 2) as u8, 4])
        .collect::<Vec<_>>();
    canonicalize_esp32(NativePacketInput {
        node_id: source,
        n_antennas: 1,
        raw_bins: 64,
        frequency_hz: 2_437_000_000,
        ppdu_type: PpduType::HtLegacy,
        sequence,
        received_at_ns: time,
        receiver_sample_rate_hz: 33.,
        iq_bytes: &bytes,
        flags: Adr018Flags::default(),
        provenance: CaptureProvenance::Synthetic,
    })
    .unwrap()
}
fn fitted() -> TemporalModel {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    TemporalModel::import(BYTES.get_or_init(|| {
        let mut session = TemporalTrainingSession::new(
            context(),
            ExperimentalPlanId::Native2000msSlots2,
            &[
                (1, ReceiverCadenceBand::Hz30To36),
                (2, ReceiverCadenceBand::Hz30To36),
            ],
            "synthetic-store-fit".into(),
            FitConfig {
                epochs: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let labels = [
            RoomLabel::Empty,
            RoomLabel::OccupiedStatic,
            RoomLabel::OccupiedMoving,
            RoomLabel::AdjacentOnly,
        ];
        let mut sequence = 0;
        for window in 0..16usize {
            let target = 10_000_000_000 + window as u64 * 3_000_000_000;
            session
                .begin_window(target, target + 3_000_000_000)
                .unwrap();
            let label = labels[window / 4];
            let mut complete = false;
            'rows: for row in 0..256usize {
                sequence += 1;
                for source in 1..=2 {
                    let at = target
                        + row as u64 * 1_000_000_000u64.div_ceil(33)
                        + u64::from(source) * 1000;
                    if session
                        .ingest(&packet(source, sequence, at, label, row))
                        .unwrap()
                        == WindowAdmission::Ready
                    {
                        session.finish_window(at).unwrap();
                        complete = true;
                        break 'rows;
                    }
                }
            }
            assert!(complete);
        }
        let (model, report) = session.fit().unwrap();
        assert!(report.backbone_delta_l2 > 0.);
        model.export().unwrap()
    }))
    .unwrap()
}
fn store(root: &Path) -> ArtifactStore {
    let c = context();
    ArtifactStore::new(
        &root.join(&c.owner_scope).join("SensingServer"),
        &c.owner_scope,
        &c.installation_id,
    )
    .unwrap()
}
fn live(model: &TemporalModel) -> serde_json::Value {
    let mut input = model.live_input("synthetic-fresh-live".into()).unwrap();
    let target = 1_000_000_000_000;
    input.begin_window(target, target + 3_000_000_000).unwrap();
    for row in 0..256usize {
        for source in 1..=2 {
            let at = target + row as u64 * 1_000_000_000u64.div_ceil(33) + u64::from(source) * 1000;
            if input
                .ingest(&packet(
                    source,
                    100_000 + row as u32,
                    at,
                    RoomLabel::Unknown,
                    row,
                ))
                .unwrap()
                == WindowAdmission::Ready
            {
                return serde_json::to_value(model.predict(input.finish(at).unwrap(), at).unwrap())
                    .unwrap();
            }
        }
    }
    panic!("missing complete native horizon")
}
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}
fn blob_path(store: &ArtifactStore, receipt: &TemporalCandidateReceipt) -> PathBuf {
    store
        .root
        .join(DIRECTORY)
        .join(&receipt.context_digest)
        .join(format!("{}.json", receipt.artifact_sha256))
}
#[test]
fn actual_native_fit_whole_envelope_private_persist_coldload_same_inference() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let model = fitted();
    let bytes = model.export().unwrap();
    assert!(bytes.len() > LIMIT);
    assert!(bytes.len() < BLOB_LIMIT);
    let before = serde_json::to_vec(&storage.catalog().unwrap()).unwrap();
    let receipt = storage.save_temporal_candidate(&model).unwrap();
    assert_eq!(receipt.artifact_sha256, digest(&bytes));
    assert_eq!(receipt.input_policy_sha256, model.plan_sha256());
    assert!(receipt.diagnostic_only);
    assert!(!receipt.qualification_granted && !receipt.physically_validated);
    let path = blob_path(&storage, &receipt);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(
        private_read(&path).is_err(),
        "global64KiB reader was broadened"
    );
    assert_eq!(
        serde_json::to_vec(&storage.catalog().unwrap()).unwrap(),
        before
    );
    assert!(storage.load_candidate(&context()).unwrap().is_none());
    assert!(storage.load_selection().unwrap().is_none());
    let cold = store(root.path());
    let loaded = cold.load_temporal_candidate(&context()).unwrap().unwrap();
    assert_eq!(loaded.export().unwrap(), bytes);
    assert_eq!(live(&model), live(&loaded));
    assert_eq!(
        cold.temporal_candidate_receipt(&context()).unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(
        cold.load_exact_temporal_candidate(&context(), &receipt.artifact_sha256)
            .unwrap()
            .export()
            .unwrap(),
        bytes
    );
    // Whole codec includes private weights/policy AND per-row receive/frame/sequence
    // lineage. It is NOT aggregate-only; raw IQ/CSI sample arrays are absent.
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["payload"]["model_json"].is_string());
    assert!(value["payload"]["evidence"][0]["sources"][0]["times_ns"].is_array());
    assert!(value["payload"]["evidence"][0]["sources"][0]["frame_ids"].is_array());
    assert!(value["payload"]["evidence"][0]["sources"][0]["sequence_ids"].is_array());
    assert!(crate::room_learner::model::RoomModel::import(&bytes).is_err());
}
fn altered(bytes: &[u8], case: &str) -> Vec<u8> {
    let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let mut text = String::from_utf8(bytes.to_vec()).unwrap();
    match case {
        "policy" => {
            text = text.replacen("\"target_span_ms\":2000", "\"target_span_ms\":1000", 1);
        }
        "source" => {
            text = text.replacen("\"bandwidth_hz\":20000000", "\"bandwidth_hz\":40000000", 1);
        }
        "frame" => {
            let old = value["payload"]["evidence"][0]["sources"][0]["frame_ids"][0]
                .as_str()
                .unwrap();
            text = text.replacen(
                &format!("\"frame_ids\":[\"{old}\""),
                "\"frame_ids\":[\"0\"",
                1,
            );
            let a = text.find("\"evidence\":").unwrap() + "\"evidence\":".len();
            let b = text.find(",\"model_sha256\"").unwrap();
            let hash = digest(text[a..b].as_bytes());
            text = text.replacen(
                value["payload"]["input_evidence_sha256"].as_str().unwrap(),
                &hash,
                1,
            );
        }
        _ => unreachable!(),
    }
    let start = text.find("\"payload\":").unwrap() + "\"payload\":".len();
    let hash = digest(text[start..text.len() - 1].as_bytes());
    text = text.replacen(value["payload_sha256"].as_str().unwrap(), &hash, 1);
    text.into_bytes()
}
#[test]
fn policy_source_frame_and_expected_context_tampering_cannot_load() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let model = fitted();
    let receipt = storage.save_temporal_candidate(&model).unwrap();
    let bytes = model.export().unwrap();
    let path = blob_path(&storage, &receipt);
    let parent = path.parent().unwrap();
    for case in ["policy", "source", "frame"] {
        let bad = altered(&bytes, case);
        let hash = digest(&bad);
        let target = parent.join(format!("{hash}.json"));
        atomic_private_write(&target, &bad).unwrap();
        let error = storage
            .load_exact_temporal_candidate(&context(), &hash)
            .err()
            .expect("tampered policy/source/frame accepted");
        assert!(
            !error.contains("temporal_input_envelope_integrity"),
            "negative was only noncanonical JSON: {error}"
        );
        assert!(
            !error.contains("temporal_artifact_hash_mismatch"),
            "negative did not reach codec: {error}"
        );
    }
    let wrong_hash = "2".repeat(64);
    atomic_private_write(&parent.join(format!("{wrong_hash}.json")), &bytes).unwrap();
    assert_eq!(
        storage
            .load_exact_temporal_candidate(&context(), &wrong_hash)
            .err()
            .unwrap(),
        "temporal_artifact_hash_mismatch"
    );
    let mut wrong = context();
    wrong.revision += 1;
    assert!(storage
        .load_exact_temporal_candidate(&wrong, &receipt.artifact_sha256)
        .is_err());
    assert!(storage.load_temporal_candidate(&wrong).unwrap().is_none());
    assert_eq!(
        storage.temporal_candidate_receipt(&context()).unwrap(),
        Some(receipt)
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}
#[test]
fn immutable_corruption_and_pointer_privilege_changes_are_preserved_and_rejected() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let model = fitted();
    let receipt = storage.save_temporal_candidate(&model).unwrap();
    let path = blob_path(&storage, &receipt);
    let original = fs::read(&path).unwrap();
    atomic_private_write(&path, b"synthetic corrupted immutable record").unwrap();
    let corrupt = fs::read(&path).unwrap();
    assert!(storage.save_temporal_candidate(&model).is_err());
    assert_eq!(fs::read(&path).unwrap(), corrupt);
    assert!(storage.load_temporal_candidate(&context()).is_err());
    atomic_private_write(&path, &original).unwrap();
    let pointer = path.parent().unwrap().join(POINTER);
    let mut forged = receipt.clone();
    forged.qualification_granted = true;
    atomic_private_write(&pointer, &encode(&forged).unwrap()).unwrap();
    let pointer_bytes = fs::read(&pointer).unwrap();
    assert!(storage.load_temporal_candidate(&context()).is_err());
    assert!(storage.save_temporal_candidate(&model).is_err());
    assert_eq!(fs::read(&pointer).unwrap(), pointer_bytes);
}
#[test]
fn owner_installation_and_private_size_permissions_are_fail_closed() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let model = fitted();
    let receipt = storage.save_temporal_candidate(&model).unwrap();
    let wrong = ArtifactStore::new(
        &root.path().join("b".repeat(64)).join("SensingServer"),
        &"b".repeat(64),
        &context().installation_id,
    )
    .unwrap();
    assert!(wrong.save_temporal_candidate(&model).is_err());
    assert!(wrong
        .load_exact_temporal_candidate(&context(), &receipt.artifact_sha256)
        .is_err());
    let install = ArtifactStore::new(
        &storage.data_dir,
        &context().owner_scope,
        "foreign-installation",
    )
    .unwrap();
    assert!(install.save_temporal_candidate(&model).is_err());
    let path = blob_path(&storage, &receipt);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(storage.load_temporal_candidate(&context()).is_err());
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o644
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let oversized = path
        .parent()
        .unwrap()
        .join(format!("{}.json", "1".repeat(64)));
    let file = fs::File::create(&oversized).unwrap();
    fs::set_permissions(&oversized, fs::Permissions::from_mode(0o600)).unwrap();
    file.set_len((BLOB_LIMIT + 1) as u64).unwrap();
    assert_eq!(
        storage
            .load_exact_temporal_candidate(&context(), &"1".repeat(64))
            .err()
            .unwrap(),
        "temporal_artifact_size_limit"
    );
    let pointer = path.parent().unwrap().join(POINTER);
    let file = fs::File::create(&pointer).unwrap();
    fs::set_permissions(&pointer, fs::Permissions::from_mode(0o600)).unwrap();
    file.set_len((LIMIT + 1) as u64).unwrap();
    assert_eq!(
        storage.load_temporal_candidate(&context()).err().unwrap(),
        "temporal_artifact_size_limit"
    );
}
#[test]
fn blob_and_namespace_symlinks_never_follow_outside_owned_kind() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let model = fitted();
    let receipt = storage.save_temporal_candidate(&model).unwrap();
    let path = blob_path(&storage, &receipt);
    let outside = root.path().join("external-synthetic-blob");
    fs::rename(&path, &outside).unwrap();
    symlink(&outside, &path).unwrap();
    assert!(storage.load_temporal_candidate(&context()).is_err());
    assert!(storage.save_temporal_candidate(&model).is_err());
    assert_eq!(fs::read(&outside).unwrap(), model.export().unwrap());
    fs::remove_file(&path).unwrap();
    fs::rename(&outside, &path).unwrap();
    let namespace = storage.root.join(DIRECTORY);
    let moved = storage.root.join("outside-kind-test");
    fs::rename(&namespace, &moved).unwrap();
    symlink(&moved, &namespace).unwrap();
    assert!(storage.load_temporal_candidate(&context()).is_err());
    assert!(storage.save_temporal_candidate(&model).is_err());
}
fn v1_model_and_seed() -> (
    crate::room_learner::model::RoomModel,
    crate::room_learner::managed_training::TrainingProofSeed,
) {
    let mut capture = CaptureSession::new(context(), "synthetic-v1-preserved".into()).unwrap();
    for (index, label) in [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ]
    .into_iter()
    .enumerate()
    {
        let start = 1_000_000_000 + index as u64 * 13_000_000_000;
        capture
            .begin_phase(
                PhaseRequest {
                    trial_id: format!("v1-trial-{index}"),
                    session_id: format!("v1-session-{index}"),
                    role: DataRole::Train,
                    label,
                    duration_ms: 12000,
                    lead_in_ms: 0,
                    conditions_confirmed: true,
                },
                start,
            )
            .unwrap();
        for quarter in 0..4u64 {
            for sample in 0..8u32 {
                for source in 1..=2u8 {
                    let values = vec![
                        match label {
                            RoomLabel::Empty => 2.,
                            RoomLabel::AdjacentOnly => 2.3,
                            _ => 6.,
                        };
                        56
                    ];
                    capture
                        .ingest(
                            RawSourceFrame {
                                source_id: source,
                                grid_id: "ant1-bins64-ppdu0-usable-drop2-linear-v2",
                                frequency_hz: 2_437_000_000,
                                bandwidth_hz: 20_000_000,
                                sequence: index as u32 * 100 + quarter as u32 * 8 + sample + 1,
                                amplitudes: &values,
                            },
                            start
                                + quarter * 3_000_000_000
                                + 1_000_000
                                + u64::from(sample) * 10_000_000
                                + u64::from(source),
                        )
                        .unwrap();
                }
            }
        }
        capture.finish_phase(start + 12_000_000_000).unwrap();
    }
    let seed = capture.managed_training_seed().unwrap();
    let (model, _) = crate::room_learner::model::RoomModel::fit(
        &capture.validated_dataset().unwrap(),
        FitConfig {
            epochs: 2,
            ..Default::default()
        },
    )
    .unwrap();
    (model, seed)
}
#[test]
fn existing_v1_model_proof_fence_pointer_selection_and_catalog_bytes_do_not_change() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let (v1, seed) = v1_model_and_seed();
    storage.save_managed_candidate(&v1, seed).unwrap();
    let receipt = SelectionReceipt {
        schema: "ruview.room-learner.model-selection.v1".into(),
        owner_scope: context().owner_scope,
        installation_id: context().installation_id,
        revision: 1,
        selected_at_unix_ms: 1,
        selected: None,
        previous: None,
    };
    let envelope = SelectionEnvelope {
        receipt_sha256: crate::room_learner::qualification::hash(&receipt).unwrap(),
        receipt,
    };
    storage.save_selection(&envelope, 0).unwrap();
    let before = files(&storage.data_dir);
    assert!(before
        .keys()
        .any(|p| p.to_string_lossy().contains("managed-training")));
    assert!(before.keys().any(|p| p.to_string_lossy().contains("fence")));
    let catalog = serde_json::to_vec(&storage.catalog().unwrap()).unwrap();
    let v1bytes = v1.export().unwrap();
    storage.save_temporal_candidate(&fitted()).unwrap();
    for (path, bytes) in before {
        assert_eq!(fs::read(storage.data_dir.join(path)).unwrap(), bytes);
    }
    assert_eq!(
        storage
            .load_candidate(&context())
            .unwrap()
            .unwrap()
            .export()
            .unwrap(),
        v1bytes
    );
    assert_eq!(storage.load_selection().unwrap(), Some(envelope));
    assert_eq!(
        serde_json::to_vec(&storage.catalog().unwrap()).unwrap(),
        catalog
    );
}
#[test]
fn reserved_temporal_kind_does_not_consume_v1_catalog_context_scan_budget() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    for index in 0..256u32 {
        let path = storage.root.join(format!("{index:064x}"));
        private_dir(&path).unwrap();
    }
    let before = serde_json::to_vec(&storage.catalog().unwrap()).unwrap();
    directory(&storage.root.join(DIRECTORY), true).unwrap();
    assert_eq!(
        serde_json::to_vec(&storage.catalog().unwrap()).unwrap(),
        before
    );
}
