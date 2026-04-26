//! Training configuration for WiFi-DensePose.
//!
//! [`TrainingConfig`] is the single source of truth for all hyper-parameters,
//! dataset shapes, loss weights, and infrastructure settings used throughout
//! the training pipeline. It is serializable via [`serde`] so it can be stored
//! to / restored from JSON checkpoint files.
//!
//! # Example
//!
//! ```rust
//! use wifi_densepose_train::config::TrainingConfig;
//!
//! let cfg = TrainingConfig::default();
//! cfg.validate().expect("default config is valid");
//!
//! assert_eq!(cfg.num_subcarriers, 56);
//! assert_eq!(cfg.num_keypoints, 17);
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::ConfigError;

// ---------------------------------------------------------------------------
// TrainingConfig
// ---------------------------------------------------------------------------

/// Complete configuration for a WiFi-DensePose training run.
///
/// All fields have documented defaults that match the paper's experimental
/// setup. Use [`TrainingConfig::default()`] as a starting point, then override
/// individual fields as needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    // -----------------------------------------------------------------------
    // Data / Signal
    // -----------------------------------------------------------------------
    /// Number of subcarriers after interpolation (system target).
    ///
    /// The model always sees this many subcarriers regardless of the raw
    /// hardware output. Default: **56**.
    pub num_subcarriers: usize,

    /// Number of subcarriers in the raw dataset before interpolation.
    ///
    /// MM-Fi provides 114 subcarriers; set this to 56 when the dataset
    /// already matches the target count. Default: **114**.
    pub native_subcarriers: usize,

    /// Number of transmit antennas. Default: **3**.
    pub num_antennas_tx: usize,

    /// Number of receive antennas. Default: **3**.
    pub num_antennas_rx: usize,

    /// Temporal sliding-window length in frames. Default: **100**.
    pub window_frames: usize,

    /// Side length of the square keypoint heatmap output (H = W). Default: **56**.
    pub heatmap_size: usize,

    // -----------------------------------------------------------------------
    // Model
    // -----------------------------------------------------------------------
    /// Number of body keypoints (COCO 17-joint skeleton). Default: **17**.
    pub num_keypoints: usize,

    /// Number of DensePose body-part classes. Default: **24**.
    pub num_body_parts: usize,

    /// Number of feature-map channels in the backbone encoder. Default: **256**.
    pub backbone_channels: usize,

    // -----------------------------------------------------------------------
    // Optimisation
    // -----------------------------------------------------------------------
    /// Mini-batch size. Default: **8**.
    pub batch_size: usize,

    /// Initial learning rate for the Adam / AdamW optimiser. Default: **1e-3**.
    pub learning_rate: f64,

    /// L2 weight-decay regularisation coefficient. Default: **1e-4**.
    pub weight_decay: f64,

    /// Total number of training epochs. Default: **50**.
    pub num_epochs: usize,

    /// Number of linear-warmup epochs at the start. Default: **5**.
    pub warmup_epochs: usize,

    /// Epochs at which the learning rate is multiplied by `lr_gamma`.
    ///
    /// Default: **[30, 45]** (multi-step scheduler).
    pub lr_milestones: Vec<usize>,

    /// Multiplicative factor applied at each LR milestone. Default: **0.1**.
    pub lr_gamma: f64,

    /// Maximum gradient L2 norm for gradient clipping. Default: **1.0**.
    pub grad_clip_norm: f64,

    // -----------------------------------------------------------------------
    // Loss weights
    // -----------------------------------------------------------------------
    /// Weight for the keypoint heatmap loss term. Default: **0.3**.
    pub lambda_kp: f64,

    /// Weight for the DensePose body-part / UV-coordinate loss. Default: **0.6**.
    pub lambda_dp: f64,

    /// Weight for the cross-modal transfer / domain-alignment loss. Default: **0.1**.
    pub lambda_tr: f64,

    // -----------------------------------------------------------------------
    // Validation and checkpointing
    // -----------------------------------------------------------------------
    /// Run validation every N epochs. Default: **1**.
    pub val_every_epochs: usize,

    /// Stop training if validation loss does not improve for this many
    /// consecutive validation rounds. Default: **10**.
    pub early_stopping_patience: usize,

    /// Directory where model checkpoints are saved.
    pub checkpoint_dir: PathBuf,

    /// Directory where TensorBoard / CSV logs are written.
    pub log_dir: PathBuf,

    /// Keep only the top-K best checkpoints by validation metric. Default: **3**.
    pub save_top_k: usize,

    // -----------------------------------------------------------------------
    // Device
    // -----------------------------------------------------------------------
    /// Use a CUDA GPU for training when available. Default: **false**.
    pub use_gpu: bool,

    /// CUDA device index when `use_gpu` is `true`. Default: **0**.
    pub gpu_device_id: i64,

    /// Number of background data-loading threads. Default: **4**.
    pub num_workers: usize,

    // -----------------------------------------------------------------------
    // Reproducibility
    // -----------------------------------------------------------------------
    /// Global random seed for all RNG sources in the training pipeline.
    ///
    /// This seed is applied to the dataset shuffler, model parameter
    /// initialisation, and any stochastic augmentation. Default: **42**.
    pub seed: u64,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        TrainingConfig {
            // Data
            num_subcarriers: 56,
            native_subcarriers: 114,
            num_antennas_tx: 3,
            num_antennas_rx: 3,
            window_frames: 100,
            heatmap_size: 56,
            // Model
            num_keypoints: 17,
            num_body_parts: 24,
            backbone_channels: 256,
            // Optimisation
            batch_size: 8,
            learning_rate: 1e-3,
            weight_decay: 1e-4,
            num_epochs: 50,
            warmup_epochs: 5,
            lr_milestones: vec![30, 45],
            lr_gamma: 0.1,
            grad_clip_norm: 1.0,
            // Loss weights
            lambda_kp: 0.3,
            lambda_dp: 0.6,
            lambda_tr: 0.1,
            // Validation / checkpointing
            val_every_epochs: 1,
            early_stopping_patience: 10,
            checkpoint_dir: PathBuf::from("checkpoints"),
            log_dir: PathBuf::from("logs"),
            save_top_k: 3,
            // Device
            use_gpu: false,
            gpu_device_id: 0,
            num_workers: 4,
            // Reproducibility
            seed: 42,
        }
    }
}

impl TrainingConfig {
    /// Load a [`TrainingConfig`] from a JSON file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::FileRead`] if the file cannot be opened and
    /// [`ConfigError::InvalidValue`] if the JSON is malformed.
    pub fn from_json(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::FileRead {
            path: path.to_path_buf(),
            source,
        })?;
        let cfg: TrainingConfig = serde_json::from_str(&contents)
            .map_err(|e| ConfigError::invalid_value("(file)", e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Serialize this configuration to pretty-printed JSON and write it to
    /// `path`, creating parent directories if necessary.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::FileRead`] if the directory cannot be created or
    /// the file cannot be written.
    pub fn to_json(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::FileRead {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| ConfigError::invalid_value("(serialization)", e.to_string()))?;
        std::fs::write(path, json).map_err(|source| ConfigError::FileRead {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    /// Returns `true` when the native dataset subcarrier count differs from the
    /// model's target count and interpolation is therefore required.
    pub fn needs_subcarrier_interp(&self) -> bool {
        self.native_subcarriers != self.num_subcarriers
    }

    /// Validate all fields and return an error describing the first problem
    /// found, or `Ok(())` if the configuration is coherent.
    ///
    /// # Validated invariants
    ///
    /// - Subcarrier counts must be non-zero.
    /// - Antenna counts must be non-zero.
    /// - `window_frames` must be at least 1.
    /// - `batch_size` must be at least 1.
    /// - `learning_rate` must be strictly positive.
    /// - `weight_decay` must be non-negative.
    /// - Loss weights must be non-negative and sum to a positive value.
    /// - `num_epochs` must be greater than `warmup_epochs`.
    /// - All `lr_milestones` must be within `[1, num_epochs]` and strictly
    ///   increasing.
    /// - `save_top_k` must be at least 1.
    /// - `val_every_epochs` must be at least 1.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Subcarrier counts
        if self.num_subcarriers == 0 {
            return Err(ConfigError::invalid_value("num_subcarriers", "must be > 0"));
        }
        if self.native_subcarriers == 0 {
            return Err(ConfigError::invalid_value(
                "native_subcarriers",
                "must be > 0",
            ));
        }

        // Antenna counts
        if self.num_antennas_tx == 0 {
            return Err(ConfigError::invalid_value("num_antennas_tx", "must be > 0"));
        }
        if self.num_antennas_rx == 0 {
            return Err(ConfigError::invalid_value("num_antennas_rx", "must be > 0"));
        }

        // Temporal window
        if self.window_frames == 0 {
            return Err(ConfigError::invalid_value("window_frames", "must be > 0"));
        }

        // Heatmap
        if self.heatmap_size == 0 {
            return Err(ConfigError::invalid_value("heatmap_size", "must be > 0"));
        }

        // Model dims
        if self.num_keypoints == 0 {
            return Err(ConfigError::invalid_value("num_keypoints", "must be > 0"));
        }
        if self.num_body_parts == 0 {
            return Err(ConfigError::invalid_value("num_body_parts", "must be > 0"));
        }
        if self.backbone_channels == 0 {
            return Err(ConfigError::invalid_value(
                "backbone_channels",
                "must be > 0",
            ));
        }

        // Optimisation
        if self.batch_size == 0 {
            return Err(ConfigError::invalid_value("batch_size", "must be > 0"));
        }
        if self.learning_rate <= 0.0 {
            return Err(ConfigError::invalid_value(
                "learning_rate",
                "must be > 0.0",
            ));
        }
        if self.weight_decay < 0.0 {
            return Err(ConfigError::invalid_value(
                "weight_decay",
                "must be >= 0.0",
            ));
        }
        if self.grad_clip_norm <= 0.0 {
            return Err(ConfigError::invalid_value(
                "grad_clip_norm",
                "must be > 0.0",
            ));
        }

        // Epochs
        if self.num_epochs == 0 {
            return Err(ConfigError::invalid_value("num_epochs", "must be > 0"));
        }
        if self.warmup_epochs >= self.num_epochs {
            return Err(ConfigError::invalid_value(
                "warmup_epochs",
                "must be < num_epochs",
            ));
        }

        // LR milestones: must be strictly increasing and within bounds
        let mut prev = 0usize;
        for &m in &self.lr_milestones {
            if m == 0 || m > self.num_epochs {
                return Err(ConfigError::invalid_value(
                    "lr_milestones",
                    "each milestone must be in [1, num_epochs]",
                ));
            }
            if m <= prev {
                return Err(ConfigError::invalid_value(
                    "lr_milestones",
                    "milestones must be strictly increasing",
                ));
            }
            prev = m;
        }

        if self.lr_gamma <= 0.0 || self.lr_gamma >= 1.0 {
            return Err(ConfigError::invalid_value(
                "lr_gamma",
                "must be in (0.0, 1.0)",
            ));
        }

        // Loss weights
        if self.lambda_kp < 0.0 {
            return Err(ConfigError::invalid_value("lambda_kp", "must be >= 0.0"));
        }
        if self.lambda_dp < 0.0 {
            return Err(ConfigError::invalid_value("lambda_dp", "must be >= 0.0"));
        }
        if self.lambda_tr < 0.0 {
            return Err(ConfigError::invalid_value("lambda_tr", "must be >= 0.0"));
        }
        let total_weight = self.lambda_kp + self.lambda_dp + self.lambda_tr;
        if total_weight <= 0.0 {
            return Err(ConfigError::invalid_value(
                "lambda_kp / lambda_dp / lambda_tr",
                "at least one loss weight must be > 0.0",
            ));
        }

        // Validation / checkpoint
        if self.val_every_epochs == 0 {
            return Err(ConfigError::invalid_value(
                "val_every_epochs",
                "must be > 0",
            ));
        }
        if self.save_top_k == 0 {
            return Err(ConfigError::invalid_value("save_top_k", "must be > 0"));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_config_is_valid() {
        let cfg = TrainingConfig::default();
        cfg.validate().expect("default config should be valid");
    }

    #[test]
    fn json_round_trip() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.json");

        let original = TrainingConfig::default();
        original.to_json(&path).expect("serialization should succeed");

        let loaded = TrainingConfig::from_json(&path).expect("deserialization should succeed");
        assert_eq!(loaded.num_subcarriers, original.num_subcarriers);
        assert_eq!(loaded.batch_size, original.batch_size);
        assert_eq!(loaded.seed, original.seed);
        assert_eq!(loaded.lr_milestones, original.lr_milestones);
    }

    #[test]
    fn zero_subcarriers_is_invalid() {
        let mut cfg = TrainingConfig::default();
        cfg.num_subcarriers = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn negative_learning_rate_is_invalid() {
        let mut cfg = TrainingConfig::default();
        cfg.learning_rate = -0.001;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn warmup_equal_to_epochs_is_invalid() {
        let mut cfg = TrainingConfig::default();
        cfg.warmup_epochs = cfg.num_epochs;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn non_increasing_milestones_are_invalid() {
        let mut cfg = TrainingConfig::default();
        cfg.lr_milestones = vec![30, 20]; // wrong order
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn milestone_beyond_epochs_is_invalid() {
        let mut cfg = TrainingConfig::default();
        cfg.lr_milestones = vec![30, cfg.num_epochs + 1];
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn all_zero_loss_weights_are_invalid() {
        let mut cfg = TrainingConfig::default();
        cfg.lambda_kp = 0.0;
        cfg.lambda_dp = 0.0;
        cfg.lambda_tr = 0.0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn needs_subcarrier_interp_when_counts_differ() {
        let mut cfg = TrainingConfig::default();
        cfg.num_subcarriers = 56;
        cfg.native_subcarriers = 114;
        assert!(cfg.needs_subcarrier_interp());

        cfg.native_subcarriers = 56;
        assert!(!cfg.needs_subcarrier_interp());
    }

    #[test]
    fn config_fields_have_expected_defaults() {
        let cfg = TrainingConfig::default();
        assert_eq!(cfg.num_subcarriers, 56);
        assert_eq!(cfg.native_subcarriers, 114);
        assert_eq!(cfg.num_antennas_tx, 3);
        assert_eq!(cfg.num_antennas_rx, 3);
        assert_eq!(cfg.window_frames, 100);
        assert_eq!(cfg.heatmap_size, 56);
        assert_eq!(cfg.num_keypoints, 17);
        assert_eq!(cfg.num_body_parts, 24);
        assert_eq!(cfg.batch_size, 8);
        assert!((cfg.learning_rate - 1e-3).abs() < 1e-10);
        assert_eq!(cfg.num_epochs, 50);
        assert_eq!(cfg.warmup_epochs, 5);
        assert_eq!(cfg.lr_milestones, vec![30, 45]);
        assert!((cfg.lr_gamma - 0.1).abs() < 1e-10);
        assert!((cfg.lambda_kp - 0.3).abs() < 1e-10);
        assert!((cfg.lambda_dp - 0.6).abs() < 1e-10);
        assert!((cfg.lambda_tr - 0.1).abs() < 1e-10);
        assert_eq!(cfg.seed, 42);
    }
}
