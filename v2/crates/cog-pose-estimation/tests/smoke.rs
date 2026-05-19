//! Smoke tests for the cog-pose-estimation crate.
//!
//! These are deliberately tight — full inference integration tests
//! depend on a trained safetensors blob that doesn't live in-repo yet.

use cog_pose_estimation::{
    inference::{InferenceEngine, SyntheticInput, INPUT_SUBCARRIERS, INPUT_TIMESTEPS, OUTPUT_KEYPOINTS},
    manifest::ManifestSpec,
};

#[test]
fn synthetic_window_has_correct_shape() {
    let syn = SyntheticInput::default();
    let window = syn.as_window();
    assert_eq!(window.data.len(), INPUT_SUBCARRIERS * INPUT_TIMESTEPS);
}

#[test]
fn engine_produces_finite_output_for_synthetic_input() {
    let engine = InferenceEngine::new().expect("engine init");
    let out = engine
        .infer(&SyntheticInput::default().as_window())
        .expect("infer");
    assert!(out.is_finite(), "synthetic input must produce finite output");
    assert_eq!(out.keypoints.len(), OUTPUT_KEYPOINTS * 2);
}

#[test]
fn engine_rejects_wrong_shape_input() {
    let engine = InferenceEngine::new().expect("engine init");
    let bad = cog_pose_estimation::inference::CsiWindow { data: vec![0.0; 10] };
    assert!(engine.infer(&bad).is_err());
}

#[test]
fn manifest_roundtrips() {
    let spec = ManifestSpec::embedded("pose-estimation", "0.0.1");
    let s = serde_json::to_string(&spec).unwrap();
    let back: ManifestSpec = serde_json::from_str(&s).unwrap();
    assert_eq!(back.id, "pose-estimation");
    assert_eq!(back.version, "0.0.1");
}
