//! SYNTHETIC actual managed fitter -> combined SAME-store helper -> cold model
//! and actual aggregate report. No production issuer/runtime/paired API.
use super::*;
use crate::room_learner::{
    artifact::ArtifactStore,
    managed_schedule_contract::{LeadInPolicy, ManagedScheduleContract},
    native::{canonicalize_esp32, NativePacketInput},
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use wifi_densepose_hardware::{Adr018Flags, PpduType};
fn context() -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-fit-summary-v3".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=3)
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
fn actual_fit() -> (TemporalModel, TrainingReport, FitConfig) {
    let schedule =
        ManagedScheduleContract::build(context(), LeadInPolicy::Train5HeldOut15EngineeringV1)
            .unwrap();
    let configured = FitConfig {
        epochs: 2,
        ..Default::default()
    };
    let mut session = TemporalTrainingSession::managed(
        schedule.clone(),
        "actual-fit-summary".into(),
        configured.clone(),
    )
    .unwrap();
    let mut start = 10_000_000_000;
    let mut index = 0u32;
    for episode in schedule
        .episodes()
        .iter()
        .filter(|e| e.role == DataRole::Train)
    {
        let bound = schedule
            .bind_episode_start("actual-fit-summary", episode.index, 0, 0, None)
            .unwrap();
        for slot in 0..episode.slots.len() {
            let target = ManagedWindowTarget::from_runtime_episode(
                &schedule,
                &bound,
                "actual-fit-summary",
                slot,
                start,
            )
            .unwrap();
            session.begin_managed_window(&target).unwrap();
            'window: for row in 0..100 {
                for source in 1..=3 {
                    let gain = match episode.condition {
                        RoomLabel::Empty => 3,
                        RoomLabel::AdjacentOnly => 4,
                        RoomLabel::OccupiedStatic => 8,
                        RoomLabel::OccupiedMoving => 10 + (row % 2) as u8,
                        _ => unreachable!(),
                    };
                    let iq = (0..64)
                        .flat_map(|b| [gain + source + (b % 2) as u8, 4])
                        .collect::<Vec<_>>();
                    let at = target.target_ns()
                        + row as u64 * 1_000_000_000u64.div_ceil(33)
                        + u64::from(source) * 1000;
                    let packet = canonicalize_esp32(NativePacketInput {
                        node_id: source,
                        n_antennas: 1,
                        raw_bins: 64,
                        frequency_hz: 2_437_000_000,
                        ppdu_type: PpduType::HtLegacy,
                        sequence: index * 1000 + row as u32 + 1,
                        received_at_ns: at,
                        receiver_sample_rate_hz: 33.,
                        iq_bytes: &iq,
                        flags: Adr018Flags::default(),
                        provenance: CaptureProvenance::Synthetic,
                    })
                    .unwrap();
                    if session.ingest(&packet).unwrap() == WindowAdmission::Ready {
                        session.finish_window(at).unwrap();
                        break 'window;
                    }
                }
            }
            index += 1;
        }
        start += episode.target_active_recording_ms * 1_000_000 + 10_000_000_000;
    }
    let permit = session.synthetic_test_fit_admission().unwrap();
    let (model, report) = session
        .fit_managed_with_cancel(permit, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(report.training_cohorts, 64);
    assert!(report.backbone_delta_l2 > 0.);
    (model, report, configured)
}
fn fixture() -> (TemporalModel, TrainingReport, FitConfig) {
    static FIT: OnceLock<(Vec<u8>, TrainingReport, FitConfig)> = OnceLock::new();
    let (bytes, report, c) = FIT.get_or_init(|| {
        let (m, r, c) = actual_fit();
        (m.export().unwrap(), r, c)
    });
    (
        TemporalModel::import(bytes).unwrap(),
        report.clone(),
        c.clone(),
    )
}
fn store(root: &Path) -> ArtifactStore {
    ArtifactStore::new(
        &root.join("a".repeat(64)).join("SensingServer"),
        &"a".repeat(64),
        "synthetic-fit-summary-v3",
    )
    .unwrap()
}
fn sidecar(root: &Path, model: &TemporalModel) -> PathBuf {
    let b = model.fit_diagnostic_binding().unwrap();
    root.join("a".repeat(64))
        .join("SensingServer/room-learner/temporal-models")
        .join(b.context_digest)
        .join(format!("{}.fit-summary-v1.json", b.artifact_sha256))
}
fn json(v: &impl Serialize) -> Value {
    serde_json::to_value(v).unwrap()
}
#[test]
fn actual_managed_fit_combined_model_first_save_and_cold_summary_preserve_the_returned_report() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let (model, report, configured) = actual_fit();
    let before = model.export().unwrap();
    let result = storage
        .save_temporal_candidate_with_fit_summary(&model, &report, &configured)
        .unwrap();
    assert_eq!(json(&result.fit_diagnostics)["state"], "recorded");
    let summary = result.fit_diagnostics.summary.unwrap();
    assert_eq!(
        json(&summary.metrics.initial),
        json(&FitObjectiveValues(&report.initial))
    );
    assert_eq!(summary.metrics.completed_epochs, report.completed_epochs);
    assert_eq!(
        summary.metrics.objective_evaluations,
        report.objective_evaluations
    );
    assert_eq!(summary.metrics.training_cohorts, 64);
    assert_eq!(
        summary.metrics.contrastive_trials,
        report.contrastive_trials
    );
    assert_eq!(
        summary.metrics.parameter_delta_l2,
        report.parameter_delta_l2
    );
    assert_eq!(summary.metrics.backbone_delta_l2, report.backbone_delta_l2);
    assert!(summary.metrics.elapsed_ms.is_none());
    assert_eq!(
        summary.binding.artifact_sha256,
        result.candidate.artifact_sha256
    );
    assert_eq!(
        summary.binding.artifact_kind,
        "ruview.room-learner.managed-temporal-input.v3"
    );
    assert!(
        summary.binding.schedule_sha256.is_some()
            && summary.binding.execution_policy_sha256.is_some()
            && summary.binding.identity_evidence_sha256.is_some()
    );
    let cold = storage
        .load_exact_temporal_candidate(model.context(), &result.candidate.artifact_sha256)
        .unwrap();
    assert_eq!(cold.export().unwrap(), before);
    assert_eq!(model.export().unwrap(), before);
    let loaded = storage.temporal_fit_diagnostics(&cold);
    assert_eq!(json(&loaded)["state"], "recorded");
    assert_eq!(json(&loaded.summary.unwrap()), json(&summary));
    assert!(!summary.physically_validated && !summary.qualification_granted);
    assert!(sidecar(root.path(), &model).metadata().unwrap().len() < 16_384);
}
// Same names/types as ObjectiveValues serialization; no rounded/estimated loss.
struct FitObjectiveValues<'a>(&'a crate::room_learner::training::ObjectiveValues);
impl Serialize for FitObjectiveValues<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
#[test]
fn missing_history_is_not_recorded_and_optional_write_failure_keeps_whole_model_available() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let (model, report, c) = fixture();
    let receipt = storage.save_temporal_candidate(&model).unwrap();
    let before = model.export().unwrap();
    let missing = storage.temporal_fit_diagnostics(&model);
    assert_eq!(json(&missing)["state"], "not_recorded");
    assert!(missing.summary.is_none());
    crate::companion::atomic_private_write(
        &sidecar(root.path(), &model),
        b"existing malformed diagnostics",
    )
    .unwrap();
    let result = storage
        .save_temporal_candidate_with_fit_summary(&model, &report, &c)
        .unwrap();
    assert_eq!(json(&result.fit_diagnostics)["state"], "not_recorded");
    assert!(result.fit_diagnostics.warning.is_some());
    assert!(result.fit_diagnostics.summary.is_none());
    assert_eq!(
        storage
            .load_exact_temporal_candidate(model.context(), &receipt.artifact_sha256)
            .unwrap()
            .export()
            .unwrap(),
        before
    );
    assert_eq!(
        std::fs::read(sidecar(root.path(), &model)).unwrap(),
        b"existing malformed diagnostics"
    );
    assert_eq!(
        json(&storage.temporal_fit_diagnostics(&model))["state"],
        "unavailable"
    );
}
#[test]
fn exact_v3_bound_configuration_and_actual_numeric_report_are_required_without_invalidating_model()
{
    for mode in ["config", "objective", "delta", "count"] {
        let root = tempfile::tempdir().unwrap();
        let storage = store(root.path());
        let (model, mut report, mut c) = fixture();
        match mode {
            "config" => c.learning_rate = 0.3,
            "objective" => report.final_objective.total = f32::NAN,
            "delta" => report.parameter_delta_l2 = 0.,
            _ => report.training_cohorts = 4,
        };
        let result = storage
            .save_temporal_candidate_with_fit_summary(&model, &report, &c)
            .unwrap();
        assert_eq!(
            json(&result.fit_diagnostics)["state"],
            "not_recorded",
            "{mode}"
        );
        assert!(result.fit_diagnostics.warning.is_some());
        assert!(storage
            .load_exact_temporal_candidate(model.context(), &result.candidate.artifact_sha256)
            .is_ok());
    }
}
#[test]
fn whole_identity_and_policy_tampering_even_with_recomputed_summary_hash_is_unavailable() {
    for field in [
        "artifact_kind",
        "artifact_sha256",
        "context_digest",
        "training_data_sha256",
        "input_evidence_sha256",
        "input_policy_sha256",
        "execution_policy_sha256",
        "schedule_sha256",
        "identity_evidence_sha256",
    ] {
        let root = tempfile::tempdir().unwrap();
        let storage = store(root.path());
        let (model, r, c) = fixture();
        storage
            .save_temporal_candidate_with_fit_summary(&model, &r, &c)
            .unwrap();
        let path = sidecar(root.path(), &model);
        let mut e: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        e["summary"]["binding"][field] = json!("f".repeat(64));
        e["summary_sha256"] = json!(bytes_hash(&serde_json::to_vec(&e["summary"]).unwrap()));
        crate::companion::atomic_private_write(
            &path,
            &crate::room_learner::artifact::rehash_temporal_fit_fixture(
                &serde_json::to_vec(&e).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            storage.load_temporal_fit_summary(&model).unwrap_err(),
            "temporal_fit_summary_binding_changed",
            "{field}"
        );
        assert_eq!(
            json(&storage.temporal_fit_diagnostics(&model))["state"],
            "unavailable",
            "{field}"
        );
        assert!(storage.load_temporal_candidate(model.context()).is_ok());
    }
}
#[test]
fn aggregate_summary_contains_no_raw_rows_timestamps_credentials_or_authority() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let (m, r, c) = fixture();
    let result = storage
        .save_temporal_candidate_with_fit_summary(&m, &r, &c)
        .unwrap();
    let v = json(&result.fit_diagnostics);
    fn walk(v: &Value) {
        match v {
            Value::Object(o) => {
                for (k, v) in o {
                    assert!(
                        ![
                            "samples",
                            "frame_ids",
                            "sequence_ids",
                            "times_ns",
                            "captured_at",
                            "timestamp",
                            "secret",
                            "keys",
                            "raw_iq",
                            "teacher"
                        ]
                        .contains(&k.as_str()),
                        "{k}"
                    );
                    walk(v)
                }
            }
            Value::Array(a) => {
                for v in a {
                    walk(v)
                }
            }
            _ => {}
        }
    }
    walk(&v);
    assert_eq!(v["physically_validated"], false);
    assert_eq!(v["qualification_granted"], false);
}

#[test]
fn oversized_or_symlink_summary_is_optional_and_never_replaces_the_retained_bytes() {
    for mode in ["oversize", "symlink"] {
        let root = tempfile::tempdir().unwrap();
        let storage = store(root.path());
        let (m, r, c) = fixture();
        let receipt = storage.save_temporal_candidate(&m).unwrap();
        let path = sidecar(root.path(), &m);
        if mode == "oversize" {
            crate::companion::atomic_private_write(&path, &vec![b'x'; 16_385]).unwrap();
        } else {
            #[cfg(unix)]
            {
                let target = root.path().join("unrelated.txt");
                std::fs::write(&target, b"keep unrelated").unwrap();
                std::os::unix::fs::symlink(&target, &path).unwrap();
            }
            #[cfg(not(unix))]
            {
                continue;
            }
        }
        let result = storage
            .save_temporal_candidate_with_fit_summary(&m, &r, &c)
            .unwrap();
        assert_eq!(json(&result.fit_diagnostics)["state"], "not_recorded");
        assert!(storage
            .load_exact_temporal_candidate(m.context(), &receipt.artifact_sha256)
            .is_ok());
        assert_eq!(
            json(&storage.temporal_fit_diagnostics(&m))["state"],
            "unavailable"
        );
        if mode == "oversize" {
            assert_eq!(std::fs::read(&path).unwrap().len(), 16_385);
        } else {
            assert_eq!(
                std::fs::read(root.path().join("unrelated.txt")).unwrap(),
                b"keep unrelated"
            );
            assert!(std::fs::symlink_metadata(path)
                .unwrap()
                .file_type()
                .is_symlink());
        }
    }
}
#[test]
#[cfg(unix)]
fn insecure_summary_and_foreign_store_cannot_expose_actual_private_metrics() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let (m, r, c) = fixture();
    let result = storage
        .save_temporal_candidate_with_fit_summary(&m, &r, &c)
        .unwrap();
    let path = sidecar(root.path(), &m);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        json(&storage.temporal_fit_diagnostics(&m))["state"],
        "unavailable"
    );
    let wrong = ArtifactStore::new(
        &root.path().join("b".repeat(64)).join("SensingServer"),
        &"b".repeat(64),
        "synthetic-fit-summary-v3",
    )
    .unwrap();
    assert!(wrong
        .save_temporal_candidate_with_fit_summary(&m, &r, &c)
        .is_err());
    assert!(wrong.load_temporal_fit_summary(&m).is_err());
    assert!(storage
        .load_exact_temporal_candidate(m.context(), &result.candidate.artifact_sha256)
        .is_ok());
}
#[test]
fn rehashed_false_objectives_cannot_be_reported_as_actual_fit_diagnostics() {
    let root = tempfile::tempdir().unwrap();
    let storage = store(root.path());
    let (m, r, c) = fixture();
    storage
        .save_temporal_candidate_with_fit_summary(&m, &r, &c)
        .unwrap();
    let path = sidecar(root.path(), &m);
    let mut envelope: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    envelope["summary"]["metrics"]["final_objective"]["total"] = json!(r.initial.total + 10.);
    envelope["summary_sha256"] = json!(bytes_hash(
        &serde_json::to_vec(&envelope["summary"]).unwrap()
    ));
    crate::companion::atomic_private_write(
        &path,
        &crate::room_learner::artifact::rehash_temporal_fit_fixture(
            &serde_json::to_vec(&envelope).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        storage.load_temporal_fit_summary(&m).unwrap_err(),
        "temporal_fit_report_invalid"
    );
    assert_eq!(
        json(&storage.temporal_fit_diagnostics(&m))["state"],
        "unavailable"
    );
    assert!(storage.load_temporal_candidate(m.context()).is_ok());
}
