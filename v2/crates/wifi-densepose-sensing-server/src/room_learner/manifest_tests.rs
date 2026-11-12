//! SYNTHETIC manifest integrity; provenance remains an input claim, not a receipt.
use super::*;
use crate::room_learner::{
    model::RoomModel,
    training::{learning_tests::dataset, FitConfig},
};
use std::sync::OnceLock;
fn model() -> RoomModel {
    static MODEL: OnceLock<RoomModel> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            RoomModel::fit(
                &dataset(8),
                FitConfig {
                    epochs: 2,
                    ..FitConfig::default()
                },
            )
            .unwrap()
            .0
        })
        .clone()
}
#[test]
fn streamed_digest_covers_actual_train_bytes_and_consumed_channels_are_explicit() {
    let ds = dataset(8);
    let manifest = TrainingManifest::from_training(ds.train()).unwrap();
    assert_eq!(
        manifest.training_data_sha256,
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(ds.train()).unwrap())
        )
    );
    assert_eq!(manifest.training_provenance, [CaptureProvenance::Synthetic]);
    assert_eq!(manifest.consumed_channels, [ConsumedChannel::Magnitude]);
    assert_eq!(manifest.clock_claims, [ClockKind::Unknown]);
    assert!(!manifest.measurement_clock_available);
    assert_eq!(manifest.training_cohorts, 8);
    let mut altered = ds.train().to_vec();
    altered[0].views[0].samples[0][0] += 0.01;
    assert_ne!(
        manifest.training_data_sha256,
        TrainingManifest::from_training(&altered)
            .unwrap()
            .training_data_sha256
    );
    assert!(TrainingManifest::from_training(ds.held_out()).is_err());
    assert!(TrainingManifest::from_training(ds.validation()).is_err());
}
#[test]
fn model_round_trip_retains_synthetic_origin_without_raw_rows_or_phone_channels() {
    let trained = model();
    let bytes = trained.export().unwrap();
    let imported = RoomModel::import(&bytes).unwrap();
    assert_eq!(trained.training_manifest(), imported.training_manifest());
    assert_eq!(
        imported.training_manifest().training_provenance,
        [CaptureProvenance::Synthetic]
    );
    assert!(!imported.training_manifest().measurement_clock_available);
    let json = String::from_utf8(bytes).unwrap();
    for raw in [
        "samples",
        "frame_ids",
        "trial_id",
        "phase",
        "camera",
        "phone",
    ] {
        assert!(!json.contains(raw));
    }
}
#[test]
fn missing_unknown_duplicated_forged_live_and_nonfinite_manifest_fields_are_rejected() {
    let bytes = model().export().unwrap();
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("training_manifest");
    assert!(RoomModel::import(&serde_json::to_vec(&missing).unwrap()).is_err());
    for (field, value) in [
        ("training_provenance", serde_json::json!([])),
        (
            "training_provenance",
            serde_json::json!(["UnknownPhysicalSource"]),
        ),
        (
            "training_provenance",
            serde_json::json!(["Synthetic", "Synthetic"]),
        ),
        (
            "training_provenance",
            serde_json::json!(["LiveReceiverObserved"]),
        ),
        (
            "training_data_sha256",
            serde_json::json!("not-a-training-data-hash"),
        ),
        ("training_data_sha256", serde_json::json!("A".repeat(64))),
        ("consumed_channels", serde_json::json!(["phase"])),
        ("consumed_channels", serde_json::json!([])),
        ("clock_claims", serde_json::json!(["Unknown", "Unknown"])),
        ("measurement_clock_available", serde_json::json!(true)),
        ("training_cohorts", serde_json::json!(1.5)),
        ("training_cohorts", serde_json::Value::Null),
    ] {
        let mut candidate = original.clone();
        candidate["training_manifest"][field] = value;
        assert!(
            RoomModel::import(&serde_json::to_vec(&candidate).unwrap()).is_err(),
            "accepted malformed {field}"
        );
    }
    let nonfinite = String::from_utf8(bytes).unwrap().replace(
        "\"measurement_clock_available\":false",
        "\"measurement_clock_available\":NaN",
    );
    assert!(RoomModel::import(nonfinite.as_bytes()).is_err());
}
#[test]
fn source_clock_claims_never_promote_synthetic_to_measured_or_live() {
    let ds = dataset(8);
    let mut rows = ds.train().to_vec();
    for c in &mut rows {
        for v in &mut c.views {
            v.clock.kind = ClockKind::SourceMeasurement;
            v.clock.uncertainty_ns = 0;
        }
    }
    let manifest = TrainingManifest::from_training(&rows).unwrap();
    assert_eq!(manifest.clock_claims, [ClockKind::SourceMeasurement]);
    assert!(!manifest.measurement_clock_available);
    for c in &mut rows {
        c.provenance = CaptureProvenance::LiveReceiverObserved;
        for v in &mut c.views {
            v.provenance = CaptureProvenance::LiveReceiverObserved;
        }
    }
    assert!(TrainingManifest::from_training(&rows)
        .unwrap_err()
        .contains("synthetic RF"));
}
