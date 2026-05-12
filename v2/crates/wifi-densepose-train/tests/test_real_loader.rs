//! Integration test for the *real* on-disk dataset loader ([`MmFiDataset`]).
//!
//! The deterministic training proof (`verify-training`) runs on the in-memory
//! `SyntheticCsiDataset`, which never touches `.npy` files — by design (a
//! reproducible source is the whole point of the proof). This test covers the
//! path the proof bypasses: it writes synthetic CSI to `.npy` files in the
//! directory layout [`MmFiDataset::discover`] expects, loads it back, and
//! checks the resulting [`CsiSample`] — including the subcarrier-interpolation
//! branch.

use ndarray::{Array3, Array4};
use ndarray_npy::write_npy;
use tempfile::TempDir;
use wifi_densepose_train::dataset::{CsiDataset, MmFiDataset};

/// Write one deterministic `S01/A01` recording (no RNG) under `root`, with
/// `n_t` frames, `[n_tx, n_rx]` antennas and `n_sc` subcarriers.
fn write_recording(root: &std::path::Path, n_t: usize, n_tx: usize, n_rx: usize, n_sc: usize) {
    let dir = root.join("S01").join("A01");
    std::fs::create_dir_all(&dir).expect("create S01/A01");

    let amplitude = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
        0.5 + 0.4 * (((t * 7 + tx * 3 + rx * 2 + sc) % 17) as f32 / 17.0)
    });
    let phase = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
        ((t + tx + rx + sc) as f32 * 0.05).sin()
    });
    let mut kp = Array3::<f32>::zeros((n_t, 17, 3));
    for t in 0..n_t {
        for j in 0..17 {
            kp[[t, j, 0]] = ((j as f32 + 1.0) / 18.0).clamp(0.0, 1.0); // x
            kp[[t, j, 1]] = (((j * 3 + t) % 18) as f32 / 18.0).clamp(0.0, 1.0); // y
            kp[[t, j, 2]] = 2.0; // COCO "visible"
        }
    }
    write_npy(dir.join("wifi_csi.npy"), &amplitude).expect("write wifi_csi.npy");
    write_npy(dir.join("wifi_csi_phase.npy"), &phase).expect("write wifi_csi_phase.npy");
    write_npy(dir.join("gt_keypoints.npy"), &kp).expect("write gt_keypoints.npy");
}

/// Round-trip: write `.npy`, discover, load — no interpolation (native == target).
#[test]
fn mmfi_loads_real_npy_without_interpolation() {
    let tmp = TempDir::new().expect("tempdir");
    write_recording(tmp.path(), 8, 3, 3, 56);

    let ds = MmFiDataset::discover(tmp.path(), 8, 56, 17).expect("discover the recording");
    assert!(ds.len() >= 1, "must discover at least one sample, got {}", ds.len());

    let sample = ds.get(0).expect("sample 0");
    assert_eq!(sample.amplitude.shape(), &[8, 3, 3, 56], "amplitude shape");
    assert_eq!(sample.phase.shape(), &[8, 3, 3, 56], "phase shape");
    assert_eq!(sample.keypoints.shape(), &[17, 2], "keypoints shape");
    assert_eq!(sample.keypoint_visibility.shape(), &[17], "visibility shape");
    assert!(sample.amplitude.iter().all(|v| v.is_finite()), "amplitude must be finite");
    assert!(sample.phase.iter().all(|v| v.is_finite()), "phase must be finite");
    assert!(sample.keypoints.iter().all(|v| v.is_finite()), "keypoints must be finite");
}

/// The loader resamples the subcarrier axis when the requested target differs
/// from the dataset's native count.
#[test]
fn mmfi_resamples_subcarriers_on_load() {
    let tmp = TempDir::new().expect("tempdir");
    write_recording(tmp.path(), 8, 3, 3, 56);

    // target (28) < native (56) — the loader must interpolate down.
    let ds = MmFiDataset::discover(tmp.path(), 8, 28, 17).expect("discover");
    let sample = ds.get(0).expect("sample 0");
    assert_eq!(
        sample.amplitude.shape(),
        &[8, 3, 3, 28],
        "amplitude must be resampled to the requested 28 subcarriers"
    );
    assert_eq!(sample.phase.shape(), &[8, 3, 3, 28], "phase must be resampled too");
    assert!(sample.amplitude.iter().all(|v| v.is_finite()), "resampled amplitude must be finite");
}

/// An empty root directory yields an empty dataset (no panic, no spurious
/// samples) — the same loader code path, just with nothing to discover.
#[test]
fn mmfi_empty_root_is_empty() {
    let tmp = TempDir::new().expect("tempdir");
    let ds = MmFiDataset::discover(tmp.path(), 8, 56, 17).expect("discover empty root");
    assert_eq!(ds.len(), 0, "empty root must produce an empty dataset");
}
