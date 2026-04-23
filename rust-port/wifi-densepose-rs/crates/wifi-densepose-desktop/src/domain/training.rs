//! Training domain types for the desktop application.

use serde::{Deserialize, Serialize};

/// GPU backend type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum GpuBackend {
    Cuda,
    Metal,
    #[default]
    Cpu,
}

/// GPU information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpuInfo {
    pub available: bool,
    pub backend: GpuBackend,
    pub name: Option<String>,
    pub memory_mb: Option<u64>,
    pub cuda_version: Option<String>,
    pub metal_supported: bool,
}

/// Dataset format type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DatasetFormat {
    #[default]
    MmFi,
    WiPose,
    Wiar,
    Custom,
}

/// Dataset information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub format: DatasetFormat,
    pub size_mb: f64,
    pub samples: u64,
    pub downloaded: bool,
    pub path: Option<String>,
    pub url: Option<String>,
}

/// Model architecture type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    #[default]
    Encoder,
    Decoder,
    Embedding,
    Adaptor,
}

/// Model architecture information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub model_type: ModelType,
    pub description: String,
    pub params_m: f64,
    pub memory_mb: u64,
    pub paper: Option<String>,
}

/// Checkpoint information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub model_id: String,
    pub name: String,
    pub epoch: u32,
    pub val_loss: f64,
    pub created_at: String,
    pub path: String,
    pub size_mb: f64,
}

/// Training configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub dataset_id: String,
    pub model_id: String,
    pub epochs: u32,
    pub batch_size: u32,
    pub learning_rate: f64,
    pub optimizer: OptimizerType,
    pub weight_decay: f64,
    pub use_augmentation: bool,
    pub checkpoint_every: u32,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            dataset_id: "mmfi".into(),
            model_id: "csi-encoder-cnn".into(),
            epochs: 100,
            batch_size: 32,
            learning_rate: 0.001,
            optimizer: OptimizerType::Adam,
            weight_decay: 0.0001,
            use_augmentation: true,
            checkpoint_every: 10,
        }
    }
}

/// Optimizer type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OptimizerType {
    #[default]
    Adam,
    AdamW,
    Sgd,
}

/// Training job status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrainingStatus {
    #[default]
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
}

/// Training progress.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrainingProgress {
    pub epoch: u32,
    pub total_epochs: u32,
    pub batch: u32,
    pub total_batches: u32,
    pub train_loss: f64,
    pub val_loss: Option<f64>,
    pub learning_rate: f64,
    pub eta_secs: u64,
    pub gpu_memory_mb: Option<u64>,
}

/// Training job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingJob {
    pub id: String,
    pub config: TrainingConfig,
    pub status: TrainingStatus,
    pub started_at: Option<String>,
    pub progress: TrainingProgress,
    pub loss_history: Vec<EpochMetrics>,
}

/// Metrics for a single epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochMetrics {
    pub epoch: u32,
    pub train_loss: f64,
    pub val_loss: f64,
    pub train_acc: f64,
    pub val_acc: f64,
    pub learning_rate: f64,
    pub timestamp: String,
}

/// Evaluation metrics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvaluationMetrics {
    pub pck_05: f64,
    pub pck_10: f64,
    pub pck_20: f64,
    pub map_50: f64,
    pub map_75: f64,
    pub iou: f64,
}

/// Per-joint accuracy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointAccuracy {
    pub joint: String,
    pub accuracy: f64,
}

/// RuVector interpolation mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum InterpolationMode {
    Linear,
    Cubic,
    #[default]
    Sparse,
}

/// RuVector module configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuVectorConfig {
    // MinCut parameters
    pub mincut_enabled: bool,
    pub mincut_threshold: f64,
    pub mincut_max_persons: u32,

    // Attention parameters
    pub attention_enabled: bool,
    pub attention_heads: u32,
    pub attention_dropout: f64,

    // Temporal parameters
    pub temporal_enabled: bool,
    pub temporal_window_ms: u32,
    pub temporal_compression_ratio: u32,

    // Solver parameters
    pub solver_enabled: bool,
    pub solver_interpolation: InterpolationMode,
    pub solver_subcarrier_count: u32,

    // BVP parameters
    pub bvp_enabled: bool,
    pub bvp_filter_hz: (f64, f64),
}

impl Default for RuVectorConfig {
    fn default() -> Self {
        Self {
            mincut_enabled: true,
            mincut_threshold: 0.5,
            mincut_max_persons: 5,
            attention_enabled: true,
            attention_heads: 4,
            attention_dropout: 0.1,
            temporal_enabled: true,
            temporal_window_ms: 500,
            temporal_compression_ratio: 4,
            solver_enabled: true,
            solver_interpolation: InterpolationMode::Sparse,
            solver_subcarrier_count: 56,
            bvp_enabled: false,
            bvp_filter_hz: (0.7, 4.0),
        }
    }
}

/// Live test metrics from RuVector processing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LiveTestMetrics {
    pub fps: f64,
    pub latency_ms: f64,
    pub persons_detected: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_info_default() {
        let info = GpuInfo::default();
        assert!(!info.available);
        assert_eq!(info.backend, GpuBackend::Cpu);
    }

    #[test]
    fn test_training_config_default() {
        let config = TrainingConfig::default();
        assert_eq!(config.epochs, 100);
        assert_eq!(config.batch_size, 32);
        assert_eq!(config.optimizer, OptimizerType::Adam);
    }

    #[test]
    fn test_ruvector_config_default() {
        let config = RuVectorConfig::default();
        assert!(config.mincut_enabled);
        assert_eq!(config.mincut_threshold, 0.5);
        assert_eq!(config.attention_heads, 4);
    }

    #[test]
    fn test_serialization() {
        let config = TrainingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: TrainingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.epochs, config.epochs);
    }

    #[test]
    fn test_dataset_info() {
        let dataset = DatasetInfo {
            id: "mmfi".into(),
            name: "MM-Fi Dataset".into(),
            description: "Multi-modal WiFi sensing".into(),
            format: DatasetFormat::MmFi,
            size_mb: 2400.0,
            samples: 320000,
            downloaded: false,
            path: None,
            url: Some("https://example.com/mmfi.zip".into()),
        };
        assert_eq!(dataset.id, "mmfi");
        assert!(!dataset.downloaded);
    }
}
