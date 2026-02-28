//! Integration tests for [`wifi_densepose_train::config`].
//!
//! All tests are deterministic: they use only fixed values and the
//! `TrainingConfig::default()` constructor.  No OS entropy or `rand` crate
//! is used.

use wifi_densepose_train::config::TrainingConfig;

// ---------------------------------------------------------------------------
// Default config invariants
// ---------------------------------------------------------------------------

/// The default configuration must pass its own validation.
#[test]
fn default_config_is_valid() {
    let cfg = TrainingConfig::default();
    cfg.validate()
        .expect("default TrainingConfig must be valid");
}

/// Every numeric field in the default config must be strictly positive where
/// the domain requires it.
#[test]
fn default_config_all_positive_fields() {
    let cfg = TrainingConfig::default();

    assert!(cfg.num_subcarriers > 0, "num_subcarriers must be > 0");
    assert!(cfg.native_subcarriers > 0, "native_subcarriers must be > 0");
    assert!(cfg.num_antennas_tx > 0, "num_antennas_tx must be > 0");
    assert!(cfg.num_antennas_rx > 0, "num_antennas_rx must be > 0");
    assert!(cfg.window_frames > 0, "window_frames must be > 0");
    assert!(cfg.heatmap_size > 0, "heatmap_size must be > 0");
    assert!(cfg.num_keypoints > 0, "num_keypoints must be > 0");
    assert!(cfg.num_body_parts > 0, "num_body_parts must be > 0");
    assert!(cfg.backbone_channels > 0, "backbone_channels must be > 0");
    assert!(cfg.batch_size > 0, "batch_size must be > 0");
    assert!(cfg.learning_rate > 0.0, "learning_rate must be > 0.0");
    assert!(cfg.weight_decay >= 0.0, "weight_decay must be >= 0.0");
    assert!(cfg.num_epochs > 0, "num_epochs must be > 0");
    assert!(cfg.grad_clip_norm > 0.0, "grad_clip_norm must be > 0.0");
}

/// The three loss weights in the default config must all be non-negative and
/// their sum must be positive (not all zero).
#[test]
fn default_config_loss_weights_sum_positive() {
    let cfg = TrainingConfig::default();

    assert!(cfg.lambda_kp >= 0.0, "lambda_kp must be >= 0.0");
    assert!(cfg.lambda_dp >= 0.0, "lambda_dp must be >= 0.0");
    assert!(cfg.lambda_tr >= 0.0, "lambda_tr must be >= 0.0");

    let total = cfg.lambda_kp + cfg.lambda_dp + cfg.lambda_tr;
    assert!(
        total > 0.0,
        "sum of loss weights must be > 0.0, got {total}"
    );
}

/// The default loss weights should sum to exactly 1.0 (within floating-point
/// tolerance).
#[test]
fn default_config_loss_weights_sum_to_one() {
    let cfg = TrainingConfig::default();
    let total = cfg.lambda_kp + cfg.lambda_dp + cfg.lambda_tr;
    let diff = (total - 1.0_f64).abs();
    assert!(
        diff < 1e-9,
        "expected loss weights to sum to 1.0, got {total} (diff={diff})"
    );
}

// ---------------------------------------------------------------------------
// Specific default values
// ---------------------------------------------------------------------------

/// The default number of subcarriers is 56 (MM-Fi target).
#[test]
fn default_num_subcarriers_is_56() {
    let cfg = TrainingConfig::default();
    assert_eq!(
        cfg.num_subcarriers, 56,
        "expected default num_subcarriers = 56, got {}",
        cfg.num_subcarriers
    );
}

/// The default number of native subcarriers is 114 (raw MM-Fi hardware output).
#[test]
fn default_native_subcarriers_is_114() {
    let cfg = TrainingConfig::default();
    assert_eq!(
        cfg.native_subcarriers, 114,
        "expected default native_subcarriers = 114, got {}",
        cfg.native_subcarriers
    );
}

/// The default number of keypoints is 17 (COCO skeleton).
#[test]
fn default_num_keypoints_is_17() {
    let cfg = TrainingConfig::default();
    assert_eq!(
        cfg.num_keypoints, 17,
        "expected default num_keypoints = 17, got {}",
        cfg.num_keypoints
    );
}

/// The default antenna counts are 3×3.
#[test]
fn default_antenna_counts_are_3x3() {
    let cfg = TrainingConfig::default();
    assert_eq!(cfg.num_antennas_tx, 3, "expected num_antennas_tx = 3");
    assert_eq!(cfg.num_antennas_rx, 3, "expected num_antennas_rx = 3");
}

/// The default window length is 100 frames.
#[test]
fn default_window_frames_is_100() {
    let cfg = TrainingConfig::default();
    assert_eq!(
        cfg.window_frames, 100,
        "expected window_frames = 100, got {}",
        cfg.window_frames
    );
}

/// The default seed is 42.
#[test]
fn default_seed_is_42() {
    let cfg = TrainingConfig::default();
    assert_eq!(cfg.seed, 42, "expected seed = 42, got {}", cfg.seed);
}

// ---------------------------------------------------------------------------
// needs_subcarrier_interp equivalent property
// ---------------------------------------------------------------------------

/// When native_subcarriers differs from num_subcarriers, interpolation is
/// needed.  The default config has 114 != 56, so this property must hold.
#[test]
fn default_config_needs_interpolation() {
    let cfg = TrainingConfig::default();
    // 114 native → 56 target: interpolation is required.
    assert_ne!(
        cfg.native_subcarriers, cfg.num_subcarriers,
        "default config must require subcarrier interpolation (native={} != target={})",
        cfg.native_subcarriers, cfg.num_subcarriers
    );
}

/// When native_subcarriers equals num_subcarriers no interpolation is needed.
#[test]
fn equal_subcarrier_counts_means_no_interpolation_needed() {
    let mut cfg = TrainingConfig::default();
    cfg.native_subcarriers = cfg.num_subcarriers; // e.g., both = 56
    cfg.validate().expect("config with equal subcarrier counts must be valid");
    assert_eq!(
        cfg.native_subcarriers, cfg.num_subcarriers,
        "after setting equal counts, native ({}) must equal target ({})",
        cfg.native_subcarriers, cfg.num_subcarriers
    );
}

// ---------------------------------------------------------------------------
// csi_flat_size equivalent property
// ---------------------------------------------------------------------------

/// The flat input size of a single CSI window is
/// `window_frames × num_antennas_tx × num_antennas_rx × num_subcarriers`.
/// Verify the arithmetic matches the default config.
#[test]
fn csi_flat_size_matches_expected() {
    let cfg = TrainingConfig::default();
    let expected = cfg.window_frames
        * cfg.num_antennas_tx
        * cfg.num_antennas_rx
        * cfg.num_subcarriers;
    // Default: 100 * 3 * 3 * 56 = 50400
    assert_eq!(
        expected, 50_400,
        "CSI flat size must be 50400 for default config, got {expected}"
    );
}

/// The CSI flat size must be > 0 for any valid config.
#[test]
fn csi_flat_size_positive_for_valid_config() {
    let cfg = TrainingConfig::default();
    let flat_size = cfg.window_frames
        * cfg.num_antennas_tx
        * cfg.num_antennas_rx
        * cfg.num_subcarriers;
    assert!(
        flat_size > 0,
        "CSI flat size must be > 0, got {flat_size}"
    );
}

// ---------------------------------------------------------------------------
// JSON serialization round-trip
// ---------------------------------------------------------------------------

/// Serializing a config to JSON and deserializing it must yield an identical
/// config (all fields must match).
#[test]
fn config_json_roundtrip_identical() {
    use std::path::PathBuf;
    use tempfile::tempdir;

    let tmp = tempdir().expect("tempdir must be created");
    let path = tmp.path().join("config.json");

    let original = TrainingConfig::default();
    original
        .to_json(&path)
        .expect("to_json must succeed for default config");

    let loaded = TrainingConfig::from_json(&path)
        .expect("from_json must succeed for previously serialized config");

    // Verify all fields are equal.
    assert_eq!(
        loaded.num_subcarriers, original.num_subcarriers,
        "num_subcarriers must survive round-trip"
    );
    assert_eq!(
        loaded.native_subcarriers, original.native_subcarriers,
        "native_subcarriers must survive round-trip"
    );
    assert_eq!(
        loaded.num_antennas_tx, original.num_antennas_tx,
        "num_antennas_tx must survive round-trip"
    );
    assert_eq!(
        loaded.num_antennas_rx, original.num_antennas_rx,
        "num_antennas_rx must survive round-trip"
    );
    assert_eq!(
        loaded.window_frames, original.window_frames,
        "window_frames must survive round-trip"
    );
    assert_eq!(
        loaded.heatmap_size, original.heatmap_size,
        "heatmap_size must survive round-trip"
    );
    assert_eq!(
        loaded.num_keypoints, original.num_keypoints,
        "num_keypoints must survive round-trip"
    );
    assert_eq!(
        loaded.num_body_parts, original.num_body_parts,
        "num_body_parts must survive round-trip"
    );
    assert_eq!(
        loaded.backbone_channels, original.backbone_channels,
        "backbone_channels must survive round-trip"
    );
    assert_eq!(
        loaded.batch_size, original.batch_size,
        "batch_size must survive round-trip"
    );
    assert!(
        (loaded.learning_rate - original.learning_rate).abs() < 1e-12,
        "learning_rate must survive round-trip: got {}",
        loaded.learning_rate
    );
    assert!(
        (loaded.weight_decay - original.weight_decay).abs() < 1e-12,
        "weight_decay must survive round-trip"
    );
    assert_eq!(
        loaded.num_epochs, original.num_epochs,
        "num_epochs must survive round-trip"
    );
    assert_eq!(
        loaded.warmup_epochs, original.warmup_epochs,
        "warmup_epochs must survive round-trip"
    );
    assert_eq!(
        loaded.lr_milestones, original.lr_milestones,
        "lr_milestones must survive round-trip"
    );
    assert!(
        (loaded.lr_gamma - original.lr_gamma).abs() < 1e-12,
        "lr_gamma must survive round-trip"
    );
    assert!(
        (loaded.grad_clip_norm - original.grad_clip_norm).abs() < 1e-12,
        "grad_clip_norm must survive round-trip"
    );
    assert!(
        (loaded.lambda_kp - original.lambda_kp).abs() < 1e-12,
        "lambda_kp must survive round-trip"
    );
    assert!(
        (loaded.lambda_dp - original.lambda_dp).abs() < 1e-12,
        "lambda_dp must survive round-trip"
    );
    assert!(
        (loaded.lambda_tr - original.lambda_tr).abs() < 1e-12,
        "lambda_tr must survive round-trip"
    );
    assert_eq!(
        loaded.val_every_epochs, original.val_every_epochs,
        "val_every_epochs must survive round-trip"
    );
    assert_eq!(
        loaded.early_stopping_patience, original.early_stopping_patience,
        "early_stopping_patience must survive round-trip"
    );
    assert_eq!(
        loaded.save_top_k, original.save_top_k,
        "save_top_k must survive round-trip"
    );
    assert_eq!(loaded.use_gpu, original.use_gpu, "use_gpu must survive round-trip");
    assert_eq!(
        loaded.gpu_device_id, original.gpu_device_id,
        "gpu_device_id must survive round-trip"
    );
    assert_eq!(
        loaded.num_workers, original.num_workers,
        "num_workers must survive round-trip"
    );
    assert_eq!(loaded.seed, original.seed, "seed must survive round-trip");
}

/// A modified config with non-default values must also survive a JSON
/// round-trip.
#[test]
fn config_json_roundtrip_modified_values() {
    use tempfile::tempdir;

    let tmp = tempdir().expect("tempdir must be created");
    let path = tmp.path().join("modified.json");

    let mut cfg = TrainingConfig::default();
    cfg.batch_size = 16;
    cfg.learning_rate = 5e-4;
    cfg.num_epochs = 100;
    cfg.warmup_epochs = 10;
    cfg.lr_milestones = vec![50, 80];
    cfg.seed = 99;

    cfg.validate().expect("modified config must be valid before serialization");
    cfg.to_json(&path).expect("to_json must succeed");

    let loaded = TrainingConfig::from_json(&path).expect("from_json must succeed");

    assert_eq!(loaded.batch_size, 16, "batch_size must match after round-trip");
    assert!(
        (loaded.learning_rate - 5e-4_f64).abs() < 1e-12,
        "learning_rate must match after round-trip"
    );
    assert_eq!(loaded.num_epochs, 100, "num_epochs must match after round-trip");
    assert_eq!(loaded.warmup_epochs, 10, "warmup_epochs must match after round-trip");
    assert_eq!(
        loaded.lr_milestones,
        vec![50, 80],
        "lr_milestones must match after round-trip"
    );
    assert_eq!(loaded.seed, 99, "seed must match after round-trip");
}

// ---------------------------------------------------------------------------
// Validation: invalid configurations are rejected
// ---------------------------------------------------------------------------

/// Setting num_subcarriers to 0 must produce a validation error.
#[test]
fn zero_num_subcarriers_is_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.num_subcarriers = 0;
    assert!(
        cfg.validate().is_err(),
        "num_subcarriers = 0 must be rejected by validate()"
    );
}

/// Setting native_subcarriers to 0 must produce a validation error.
#[test]
fn zero_native_subcarriers_is_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.native_subcarriers = 0;
    assert!(
        cfg.validate().is_err(),
        "native_subcarriers = 0 must be rejected by validate()"
    );
}

/// Setting batch_size to 0 must produce a validation error.
#[test]
fn zero_batch_size_is_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.batch_size = 0;
    assert!(
        cfg.validate().is_err(),
        "batch_size = 0 must be rejected by validate()"
    );
}

/// A negative learning rate must produce a validation error.
#[test]
fn negative_learning_rate_is_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.learning_rate = -0.001;
    assert!(
        cfg.validate().is_err(),
        "learning_rate < 0 must be rejected by validate()"
    );
}

/// warmup_epochs >= num_epochs must produce a validation error.
#[test]
fn warmup_exceeding_epochs_is_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.warmup_epochs = cfg.num_epochs; // equal, which is still invalid
    assert!(
        cfg.validate().is_err(),
        "warmup_epochs >= num_epochs must be rejected by validate()"
    );
}

/// All loss weights set to 0.0 must produce a validation error.
#[test]
fn all_zero_loss_weights_are_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.lambda_kp = 0.0;
    cfg.lambda_dp = 0.0;
    cfg.lambda_tr = 0.0;
    assert!(
        cfg.validate().is_err(),
        "all-zero loss weights must be rejected by validate()"
    );
}

/// Non-increasing lr_milestones must produce a validation error.
#[test]
fn non_increasing_milestones_are_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.lr_milestones = vec![40, 30]; // wrong order
    assert!(
        cfg.validate().is_err(),
        "non-increasing lr_milestones must be rejected by validate()"
    );
}

/// An lr_milestone beyond num_epochs must produce a validation error.
#[test]
fn milestone_beyond_num_epochs_is_invalid() {
    let mut cfg = TrainingConfig::default();
    cfg.lr_milestones = vec![30, cfg.num_epochs + 1];
    assert!(
        cfg.validate().is_err(),
        "lr_milestone > num_epochs must be rejected by validate()"
    );
}
