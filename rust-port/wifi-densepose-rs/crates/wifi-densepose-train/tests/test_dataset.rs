//! Integration tests for [`wifi_densepose_train::dataset`].
//!
//! All tests use [`SyntheticCsiDataset`] which is fully deterministic (no
//! random number generator, no OS entropy).  Tests that need a temporary
//! directory use [`tempfile::TempDir`].

use wifi_densepose_train::dataset::{
    CsiDataset, DatasetError, MmFiDataset, SyntheticCsiDataset, SyntheticConfig,
};

// ---------------------------------------------------------------------------
// Helper: default SyntheticConfig
// ---------------------------------------------------------------------------

fn default_cfg() -> SyntheticConfig {
    SyntheticConfig::default()
}

// ---------------------------------------------------------------------------
// SyntheticCsiDataset::len / is_empty
// ---------------------------------------------------------------------------

/// `len()` must return the exact count passed to the constructor.
#[test]
fn len_returns_constructor_count() {
    for &n in &[0_usize, 1, 10, 100, 200] {
        let ds = SyntheticCsiDataset::new(n, default_cfg());
        assert_eq!(
            ds.len(),
            n,
            "len() must return {n} for dataset of size {n}"
        );
    }
}

/// `is_empty()` must return `true` for a zero-length dataset.
#[test]
fn is_empty_true_for_zero_length() {
    let ds = SyntheticCsiDataset::new(0, default_cfg());
    assert!(
        ds.is_empty(),
        "is_empty() must be true for a dataset with 0 samples"
    );
}

/// `is_empty()` must return `false` for a non-empty dataset.
#[test]
fn is_empty_false_for_non_empty() {
    let ds = SyntheticCsiDataset::new(5, default_cfg());
    assert!(
        !ds.is_empty(),
        "is_empty() must be false for a dataset with 5 samples"
    );
}

// ---------------------------------------------------------------------------
// SyntheticCsiDataset::get — sample shapes
// ---------------------------------------------------------------------------

/// `get(0)` must return a [`CsiSample`] with the exact shapes expected by the
/// model's default configuration.
#[test]
fn get_sample_amplitude_shape() {
    let cfg = default_cfg();
    let ds = SyntheticCsiDataset::new(10, cfg.clone());
    let sample = ds.get(0).expect("get(0) must succeed");

    assert_eq!(
        sample.amplitude.shape(),
        &[cfg.window_frames, cfg.num_antennas_tx, cfg.num_antennas_rx, cfg.num_subcarriers],
        "amplitude shape must be [T, n_tx, n_rx, n_sc]"
    );
}

#[test]
fn get_sample_phase_shape() {
    let cfg = default_cfg();
    let ds = SyntheticCsiDataset::new(10, cfg.clone());
    let sample = ds.get(0).expect("get(0) must succeed");

    assert_eq!(
        sample.phase.shape(),
        &[cfg.window_frames, cfg.num_antennas_tx, cfg.num_antennas_rx, cfg.num_subcarriers],
        "phase shape must be [T, n_tx, n_rx, n_sc]"
    );
}

/// Keypoints shape must be [17, 2].
#[test]
fn get_sample_keypoints_shape() {
    let cfg = default_cfg();
    let ds = SyntheticCsiDataset::new(10, cfg.clone());
    let sample = ds.get(0).expect("get(0) must succeed");

    assert_eq!(
        sample.keypoints.shape(),
        &[cfg.num_keypoints, 2],
        "keypoints shape must be [17, 2], got {:?}",
        sample.keypoints.shape()
    );
}

/// Visibility shape must be [17].
#[test]
fn get_sample_visibility_shape() {
    let cfg = default_cfg();
    let ds = SyntheticCsiDataset::new(10, cfg.clone());
    let sample = ds.get(0).expect("get(0) must succeed");

    assert_eq!(
        sample.keypoint_visibility.shape(),
        &[cfg.num_keypoints],
        "keypoint_visibility shape must be [17], got {:?}",
        sample.keypoint_visibility.shape()
    );
}

// ---------------------------------------------------------------------------
// SyntheticCsiDataset::get — value ranges
// ---------------------------------------------------------------------------

/// All keypoint coordinates must lie in [0, 1].
#[test]
fn keypoints_in_unit_square() {
    let ds = SyntheticCsiDataset::new(5, default_cfg());
    for idx in 0..5 {
        let sample = ds.get(idx).expect("get must succeed");
        for joint in sample.keypoints.outer_iter() {
            let x = joint[0];
            let y = joint[1];
            assert!(
                x >= 0.0 && x <= 1.0,
                "keypoint x={x} at sample {idx} is outside [0, 1]"
            );
            assert!(
                y >= 0.0 && y <= 1.0,
                "keypoint y={y} at sample {idx} is outside [0, 1]"
            );
        }
    }
}

/// All visibility values in the synthetic dataset must be 2.0 (visible).
#[test]
fn visibility_all_visible_in_synthetic() {
    let ds = SyntheticCsiDataset::new(5, default_cfg());
    for idx in 0..5 {
        let sample = ds.get(idx).expect("get must succeed");
        for &v in sample.keypoint_visibility.iter() {
            assert!(
                (v - 2.0).abs() < 1e-6,
                "expected visibility = 2.0 (visible), got {v} at sample {idx}"
            );
        }
    }
}

/// Amplitude values must lie in the physics model range [0.2, 0.8].
///
/// The model computes: `0.5 + 0.3 * sin(...)`, so the range is [0.2, 0.8].
#[test]
fn amplitude_values_in_physics_range() {
    let ds = SyntheticCsiDataset::new(8, default_cfg());
    for idx in 0..8 {
        let sample = ds.get(idx).expect("get must succeed");
        for &v in sample.amplitude.iter() {
            assert!(
                v >= 0.19 && v <= 0.81,
                "amplitude value {v} at sample {idx} is outside [0.2, 0.8]"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// SyntheticCsiDataset — determinism
// ---------------------------------------------------------------------------

/// Calling `get(i)` multiple times must return bit-identical results.
#[test]
fn get_is_deterministic_same_index() {
    let ds = SyntheticCsiDataset::new(10, default_cfg());

    let s1 = ds.get(5).expect("first get must succeed");
    let s2 = ds.get(5).expect("second get must succeed");

    // Compare every element of amplitude.
    for ((t, tx, rx, k), v1) in s1.amplitude.indexed_iter() {
        let v2 = s2.amplitude[[t, tx, rx, k]];
        assert_eq!(
            v1.to_bits(),
            v2.to_bits(),
            "amplitude at [{t},{tx},{rx},{k}] must be bit-identical across calls"
        );
    }

    // Compare keypoints.
    for (j, v1) in s1.keypoints.indexed_iter() {
        let v2 = s2.keypoints[j];
        assert_eq!(
            v1.to_bits(),
            v2.to_bits(),
            "keypoint at {j:?} must be bit-identical across calls"
        );
    }
}

/// Different sample indices must produce different amplitude tensors (the
/// sinusoidal model ensures this for the default config).
#[test]
fn different_indices_produce_different_samples() {
    let ds = SyntheticCsiDataset::new(10, default_cfg());

    let s0 = ds.get(0).expect("get(0) must succeed");
    let s1 = ds.get(1).expect("get(1) must succeed");

    // At least some amplitude value must differ between index 0 and 1.
    let all_same = s0
        .amplitude
        .iter()
        .zip(s1.amplitude.iter())
        .all(|(a, b)| (a - b).abs() < 1e-7);

    assert!(
        !all_same,
        "samples at different indices must not be identical in amplitude"
    );
}

/// Two datasets with the same configuration produce identical samples at the
/// same index (seed is implicit in the analytical formula).
#[test]
fn two_datasets_same_config_same_samples() {
    let cfg = default_cfg();
    let ds1 = SyntheticCsiDataset::new(20, cfg.clone());
    let ds2 = SyntheticCsiDataset::new(20, cfg);

    for idx in [0_usize, 7, 19] {
        let s1 = ds1.get(idx).expect("ds1.get must succeed");
        let s2 = ds2.get(idx).expect("ds2.get must succeed");

        for ((t, tx, rx, k), v1) in s1.amplitude.indexed_iter() {
            let v2 = s2.amplitude[[t, tx, rx, k]];
            assert_eq!(
                v1.to_bits(),
                v2.to_bits(),
                "amplitude at [{t},{tx},{rx},{k}] must match across two equivalent datasets \
                 (sample {idx})"
            );
        }
    }
}

/// Two datasets with different num_subcarriers must produce different output
/// shapes (and thus different data).
#[test]
fn different_config_produces_different_data() {
    let mut cfg1 = default_cfg();
    let mut cfg2 = default_cfg();
    cfg2.num_subcarriers = 28; // different subcarrier count

    let ds1 = SyntheticCsiDataset::new(5, cfg1);
    let ds2 = SyntheticCsiDataset::new(5, cfg2);

    let s1 = ds1.get(0).expect("get(0) from ds1 must succeed");
    let s2 = ds2.get(0).expect("get(0) from ds2 must succeed");

    assert_ne!(
        s1.amplitude.shape(),
        s2.amplitude.shape(),
        "datasets with different configs must produce different-shaped samples"
    );
}

// ---------------------------------------------------------------------------
// SyntheticCsiDataset — out-of-bounds error
// ---------------------------------------------------------------------------

/// Requesting an index equal to `len()` must return an error.
#[test]
fn get_out_of_bounds_returns_error() {
    let ds = SyntheticCsiDataset::new(5, default_cfg());
    let result = ds.get(5); // index == len → out of bounds
    assert!(
        result.is_err(),
        "get(5) on a 5-element dataset must return Err"
    );
}

/// Requesting a large index must also return an error.
#[test]
fn get_large_index_returns_error() {
    let ds = SyntheticCsiDataset::new(3, default_cfg());
    let result = ds.get(1_000_000);
    assert!(
        result.is_err(),
        "get(1_000_000) on a 3-element dataset must return Err"
    );
}

// ---------------------------------------------------------------------------
// MmFiDataset — directory not found
// ---------------------------------------------------------------------------

/// [`MmFiDataset::discover`] must return a [`DatasetError::DirectoryNotFound`]
/// when the root directory does not exist.
#[test]
fn mmfi_dataset_nonexistent_directory_returns_error() {
    let nonexistent = std::path::PathBuf::from(
        "/tmp/wifi_densepose_test_nonexistent_path_that_cannot_exist_at_all",
    );
    // Ensure it really doesn't exist before the test.
    assert!(
        !nonexistent.exists(),
        "test precondition: path must not exist"
    );

    let result = MmFiDataset::discover(&nonexistent, 100, 56, 17);

    assert!(
        result.is_err(),
        "MmFiDataset::discover must return Err for a non-existent directory"
    );

    // The error must specifically be DirectoryNotFound.
    match result.unwrap_err() {
        DatasetError::DirectoryNotFound { .. } => { /* expected */ }
        other => panic!(
            "expected DatasetError::DirectoryNotFound, got {:?}",
            other
        ),
    }
}

/// An empty temporary directory that exists must not panic — it simply has
/// no entries and produces an empty dataset.
#[test]
fn mmfi_dataset_empty_directory_produces_empty_dataset() {
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("tempdir must be created");
    let ds = MmFiDataset::discover(tmp.path(), 100, 56, 17)
        .expect("discover on an empty directory must succeed");

    assert_eq!(
        ds.len(),
        0,
        "dataset discovered from an empty directory must have 0 samples"
    );
    assert!(
        ds.is_empty(),
        "is_empty() must be true for an empty dataset"
    );
}

// ---------------------------------------------------------------------------
// DataLoader integration
// ---------------------------------------------------------------------------

/// The DataLoader must yield exactly `len` samples when iterating without
/// shuffling over a SyntheticCsiDataset.
#[test]
fn dataloader_yields_all_samples_no_shuffle() {
    use wifi_densepose_train::dataset::DataLoader;

    let n = 17_usize;
    let ds = SyntheticCsiDataset::new(n, default_cfg());
    let dl = DataLoader::new(&ds, 4, false, 42);

    let total: usize = dl.iter().map(|batch| batch.len()).sum();
    assert_eq!(
        total, n,
        "DataLoader must yield exactly {n} samples, got {total}"
    );
}

/// The DataLoader with shuffling must still yield all samples.
#[test]
fn dataloader_yields_all_samples_with_shuffle() {
    use wifi_densepose_train::dataset::DataLoader;

    let n = 20_usize;
    let ds = SyntheticCsiDataset::new(n, default_cfg());
    let dl = DataLoader::new(&ds, 6, true, 99);

    let total: usize = dl.iter().map(|batch| batch.len()).sum();
    assert_eq!(
        total, n,
        "shuffled DataLoader must yield exactly {n} samples, got {total}"
    );
}

/// Shuffled iteration with the same seed must produce the same order twice.
#[test]
fn dataloader_shuffle_is_deterministic_same_seed() {
    use wifi_densepose_train::dataset::DataLoader;

    let ds = SyntheticCsiDataset::new(20, default_cfg());
    let dl1 = DataLoader::new(&ds, 5, true, 77);
    let dl2 = DataLoader::new(&ds, 5, true, 77);

    let ids1: Vec<u64> = dl1.iter().flatten().map(|s| s.frame_id).collect();
    let ids2: Vec<u64> = dl2.iter().flatten().map(|s| s.frame_id).collect();

    assert_eq!(
        ids1, ids2,
        "same seed must produce identical shuffle order"
    );
}

/// Different seeds must produce different iteration orders.
#[test]
fn dataloader_shuffle_different_seeds_differ() {
    use wifi_densepose_train::dataset::DataLoader;

    let ds = SyntheticCsiDataset::new(20, default_cfg());
    let dl1 = DataLoader::new(&ds, 20, true, 1);
    let dl2 = DataLoader::new(&ds, 20, true, 2);

    let ids1: Vec<u64> = dl1.iter().flatten().map(|s| s.frame_id).collect();
    let ids2: Vec<u64> = dl2.iter().flatten().map(|s| s.frame_id).collect();

    assert_ne!(ids1, ids2, "different seeds must produce different orders");
}

/// `num_batches()` must equal `ceil(n / batch_size)`.
#[test]
fn dataloader_num_batches_ceiling_division() {
    use wifi_densepose_train::dataset::DataLoader;

    let ds = SyntheticCsiDataset::new(10, default_cfg());
    let dl = DataLoader::new(&ds, 3, false, 0);
    // ceil(10 / 3) = 4
    assert_eq!(
        dl.num_batches(),
        4,
        "num_batches must be ceil(10 / 3) = 4, got {}",
        dl.num_batches()
    );
}

/// An empty dataset produces zero batches.
#[test]
fn dataloader_empty_dataset_zero_batches() {
    use wifi_densepose_train::dataset::DataLoader;

    let ds = SyntheticCsiDataset::new(0, default_cfg());
    let dl = DataLoader::new(&ds, 4, false, 42);
    assert_eq!(
        dl.num_batches(),
        0,
        "empty dataset must produce 0 batches"
    );
    assert_eq!(
        dl.iter().count(),
        0,
        "iterator over empty dataset must yield 0 items"
    );
}
