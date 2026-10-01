//! End-to-end slice over tiny MM-Fi-layout fixtures: discover → manifest →
//! subject-disjoint split → preprocessing pipeline → provenance checks.
//!
//! Fixtures are deterministic synthetic arrays (unit-test scale, never real
//! CSI): they exercise the *plumbing* — directory scan, shape validation,
//! conversion, metadata — while real data stays in the operator's ignored
//! `data/` directory (see `docs/cpu-inference-real-csi.md`).

use std::collections::BTreeSet;

use ndarray::{Array3, Array4};
use ndarray_npy::write_npy;
use tempfile::TempDir;
use wifi_densepose_train::dataset::{CsiDataset, MmFiDataset};
use wifi_densepose_train::public_dataset::{
    DatasetManifest, PreparedSplit, ProcessedSampleMeta, SplitPolicy,
};
use wifi_densepose_train::real_csi::{RealCsiConfig, RealCsiPipeline, RealCsiSourceInfo};

/// Write one deterministic `Sxx/Axx` recording (no RNG): 114 native
/// subcarriers, 1 TX x 3 RX, COCO-17 keypoints.
fn write_recording(root: &std::path::Path, subj: u32, action: u32) {
    let dir = root
        .join(format!("S{subj:02}"))
        .join(format!("A{action:02}"));
    std::fs::create_dir_all(&dir).expect("create recording dir");
    let (n_t, n_tx, n_rx, n_sc) = (10, 1, 3, 114);

    let amplitude = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
        0.5 + 0.4 * (((t * 7 + tx * 3 + rx * 2 + sc) % 17) as f32 / 17.0)
    });
    let phase = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
        ((t * 5 + tx + rx * 2 + sc) as f32 * 0.05).sin()
    });
    let mut kp = Array3::<f32>::zeros((n_t, 17, 3));
    for t in 0..n_t {
        for j in 0..17 {
            kp[[t, j, 0]] = ((j as f32 + 1.0) / 18.0).clamp(0.0, 1.0);
            kp[[t, j, 1]] = (((j * 3 + t) % 18) as f32 / 18.0).clamp(0.0, 1.0);
            kp[[t, j, 2]] = 2.0;
        }
    }
    write_npy(dir.join("wifi_csi.npy"), &amplitude).expect("write amplitude");
    write_npy(dir.join("wifi_csi_phase.npy"), &phase).expect("write phase");
    write_npy(dir.join("gt_keypoints.npy"), &kp).expect("write keypoints");
}

fn fixture_root() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    write_recording(tmp.path(), 1, 1);
    write_recording(tmp.path(), 1, 2);
    write_recording(tmp.path(), 2, 1);
    tmp
}

#[test]
fn discover_finds_subjects_and_windows() {
    let tmp = fixture_root();
    let ds = MmFiDataset::discover(tmp.path(), 10, 56, 17).expect("discover");
    assert_eq!(ds.subjects(), vec![1, 2]);
    assert_eq!(ds.len(), 3, "one 10-frame window per recording");
}

#[test]
fn discover_subset_restricts_subjects() {
    let tmp = fixture_root();
    let keep: BTreeSet<u32> = [1].into_iter().collect();
    let ds = MmFiDataset::discover_subset(tmp.path(), 10, 56, 17, &keep).expect("subset");
    assert_eq!(ds.subjects(), vec![1]);
    assert_eq!(ds.len(), 2, "subject 1 owns two recordings");
}

#[test]
fn split_manifest_round_trip_is_leak_free() {
    let tmp = fixture_root();
    let ds = MmFiDataset::discover(tmp.path(), 10, 56, 17).expect("discover");
    let (train, test) = ds.subject_disjoint_split(0.5, 42).expect("split");

    let split = PreparedSplit {
        dataset: "mmfi-fixture".to_string(),
        policy: SplitPolicy::SubjectDisjoint,
        seed: 42,
        train_subjects: train.subjects().iter().copied().collect(),
        test_subjects: test.subjects().iter().copied().collect(),
        train_windows: train.len(),
        test_windows: test.len(),
    };
    split.verify().expect("split must verify");
    assert_eq!(
        split.train_windows + split.test_windows,
        ds.len(),
        "split must cover every window exactly once"
    );

    let path = split.save(tmp.path(), "split.json").expect("save split");
    PreparedSplit::load(&path)
        .expect("load split")
        .verify()
        .expect("reloaded split verifies");
}

#[test]
fn manifest_template_needs_preparation() {
    let template = DatasetManifest::mmfi_template();
    assert!(!template.characteristics_verified);
    assert!(template.validate().is_err());

    let tmp = fixture_root();
    let ds = MmFiDataset::discover(tmp.path(), 10, 56, 17).expect("discover");
    let mut manifest = DatasetManifest::mmfi_template();
    manifest.local_path = Some(tmp.path().to_path_buf());
    manifest.file_count = 3;
    manifest.sample_count = ds.len();
    manifest.subject_count = ds.subjects().len();
    manifest.activity_count = 2;
    manifest.original_subcarrier_count = 114;
    manifest.characteristics_verified = true;
    manifest.validate().expect("filled manifest validates");

    let path = manifest.save().expect("save manifest");
    let loaded = DatasetManifest::load(&path).expect("load manifest");
    loaded.validate().expect("reloaded manifest validates");
    assert_eq!(loaded.sample_count, 3);
}

#[test]
fn pipeline_bridge_preserves_loader_provenance() {
    let tmp = fixture_root();
    // Discover at the native width so the pipeline (not the loader) performs
    // the 114 -> 56 conversion and records it.
    let ds = MmFiDataset::discover(tmp.path(), 10, 114, 17).expect("discover");
    let sample = ds.get(0).expect("sample 0");
    assert_eq!(sample.amplitude.shape(), &[10, 1, 3, 114]);

    let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
    let source = RealCsiSourceInfo {
        source_dataset: "mmfi-fixture".to_string(),
        antenna_configuration: "1TXx3RX".to_string(),
        sampling_rate_hz: Some(100.0),
        environment_id: None,
    };
    let out = pipe
        .process_loader_sample(&sample, &source)
        .expect("process");

    assert_eq!(out.amplitude.shape(), &[10, 1, 3, 56]);
    assert_eq!(out.phase.shape(), &[10, 1, 3, 56]);
    let meta: &ProcessedSampleMeta = &out.meta;
    assert_eq!(meta.source_dataset, "mmfi-fixture");
    assert_eq!(meta.subject_id, sample.subject_id);
    assert_eq!(meta.activity_id, sample.action_id);
    assert_eq!(meta.frame_index, sample.frame_id);
    assert_eq!(meta.original_subcarrier_count, 114);
    assert_eq!(meta.processed_subcarrier_count, 56);
    assert!(!meta.preprocessing_version.is_empty());
    assert!(out.amplitude.iter().all(|v| v.is_finite()));
    assert!(out.phase.iter().all(|v| v.is_finite()));
}
