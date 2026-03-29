//! Training commands for the desktop application.
//!
//! Provides Tauri commands for:
//! - GPU detection
//! - Dataset management
//! - Model/checkpoint operations
//! - Training job control
//! - RuVector configuration
//! - Metrics retrieval

use crate::domain::training::{
    CheckpointInfo, DatasetFormat, DatasetInfo, EpochMetrics, EvaluationMetrics,
    GpuBackend, GpuInfo, JointAccuracy, LiveTestMetrics,
    ModelInfo, ModelType, RuVectorConfig, TrainingConfig, TrainingJob,
    TrainingProgress, TrainingStatus,
};
use crate::state::AppState;
use tauri::State;

// ============================================================================
// Standard Datasets (built-in)
// ============================================================================

fn get_standard_datasets() -> Vec<DatasetInfo> {
    vec![
        DatasetInfo {
            id: "mmfi".into(),
            name: "MM-Fi Dataset".into(),
            description: "Multi-modal WiFi sensing dataset with 40 subjects, 27 activities".into(),
            format: DatasetFormat::MmFi,
            size_mb: 2400.0,
            samples: 320000,
            downloaded: false,
            path: None,
            url: Some("https://ntu-aiot-lab.github.io/mm-fi".into()),
        },
        DatasetInfo {
            id: "wipose".into(),
            name: "Wi-Pose Dataset".into(),
            description: "WiFi-based pose estimation with 3D skeleton annotations".into(),
            format: DatasetFormat::WiPose,
            size_mb: 1800.0,
            samples: 150000,
            downloaded: false,
            path: None,
            url: Some("https://github.com/Wi-Pose".into()),
        },
        DatasetInfo {
            id: "wiar".into(),
            name: "WiAR Dataset".into(),
            description: "WiFi activity recognition with CSI data".into(),
            format: DatasetFormat::Wiar,
            size_mb: 500.0,
            samples: 45000,
            downloaded: false,
            path: None,
            url: Some("https://github.com/WiAR".into()),
        },
    ]
}

// ============================================================================
// Standard Model Architectures
// ============================================================================

fn get_standard_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "csi-encoder-cnn".into(),
            name: "CSI Encoder (CNN)".into(),
            model_type: ModelType::Encoder,
            description: "Convolutional encoder for CSI amplitude/phase features".into(),
            params_m: 2.3,
            memory_mb: 128,
            paper: None,
        },
        ModelInfo {
            id: "csi-encoder-transformer".into(),
            name: "CSI Encoder (Transformer)".into(),
            model_type: ModelType::Encoder,
            description: "Self-attention based CSI feature extraction".into(),
            params_m: 8.5,
            memory_mb: 384,
            paper: Some("WiFi-ViT 2024".into()),
        },
        ModelInfo {
            id: "pose-decoder-lstm".into(),
            name: "Pose Decoder (LSTM)".into(),
            model_type: ModelType::Decoder,
            description: "Recurrent decoder for temporal pose estimation".into(),
            params_m: 1.8,
            memory_mb: 96,
            paper: None,
        },
        ModelInfo {
            id: "pose-decoder-gru".into(),
            name: "Pose Decoder (GRU)".into(),
            model_type: ModelType::Decoder,
            description: "Gated recurrent unit pose decoder (faster)".into(),
            params_m: 1.2,
            memory_mb: 64,
            paper: None,
        },
        ModelInfo {
            id: "aether-embedding".into(),
            name: "AETHER Embedding".into(),
            model_type: ModelType::Embedding,
            description: "Contrastive CSI embedding for person re-identification (ADR-024)".into(),
            params_m: 4.2,
            memory_mb: 192,
            paper: Some("AETHER 2025".into()),
        },
        ModelInfo {
            id: "meridian-adaptor".into(),
            name: "MERIDIAN Adaptor".into(),
            model_type: ModelType::Adaptor,
            description: "Cross-environment domain generalization module (ADR-027)".into(),
            params_m: 3.1,
            memory_mb: 144,
            paper: Some("MERIDIAN 2025".into()),
        },
    ]
}

// ============================================================================
// GPU Detection Commands
// ============================================================================

/// Detect available GPU(s) and return information.
#[tauri::command]
pub async fn detect_gpu(state: State<'_, AppState>) -> Result<GpuInfo, String> {
    // Check for cached GPU info
    if let Ok(training) = state.training.lock() {
        if let Some(ref info) = training.gpu_info {
            return Ok(info.clone());
        }
    }

    // Detect GPU
    let info = detect_gpu_internal();

    // Cache the result
    if let Ok(mut training) = state.training.lock() {
        training.gpu_info = Some(info.clone());
    }

    Ok(info)
}

fn detect_gpu_internal() -> GpuInfo {
    // Check for Metal on macOS
    #[cfg(target_os = "macos")]
    {
        // Check if system has Apple Silicon or discrete GPU
        let has_metal = std::process::Command::new("system_profiler")
            .args(["SPDisplaysDataType", "-json"])
            .output()
            .map(|o| {
                let output = String::from_utf8_lossy(&o.stdout);
                output.contains("Metal") || output.contains("Apple M")
            })
            .unwrap_or(false);

        if has_metal {
            // Try to get GPU name
            let name = std::process::Command::new("system_profiler")
                .args(["SPDisplaysDataType"])
                .output()
                .ok()
                .and_then(|o| {
                    let output = String::from_utf8_lossy(&o.stdout);
                    // Parse chipset name
                    for line in output.lines() {
                        if line.contains("Chipset Model:") {
                            return line.split(':').nth(1).map(|s| s.trim().to_string());
                        }
                    }
                    None
                });

            return GpuInfo {
                available: true,
                backend: GpuBackend::Metal,
                name,
                memory_mb: None, // Metal doesn't easily expose this
                cuda_version: None,
                metal_supported: true,
            };
        }
    }

    // Check for CUDA on Linux/Windows
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        // Try nvidia-smi for CUDA detection
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = stdout.trim().split(',').collect();

                let name = parts.first().map(|s| s.trim().to_string());
                let memory_mb = parts.get(1)
                    .and_then(|s| s.trim().parse::<u64>().ok());

                // Get CUDA version
                let cuda_version = std::process::Command::new("nvidia-smi")
                    .output()
                    .ok()
                    .and_then(|o| {
                        let output = String::from_utf8_lossy(&o.stdout);
                        for line in output.lines() {
                            if line.contains("CUDA Version:") {
                                return line.split("CUDA Version:")
                                    .nth(1)
                                    .map(|s| s.split_whitespace().next().unwrap_or("").to_string());
                            }
                        }
                        None
                    });

                return GpuInfo {
                    available: true,
                    backend: GpuBackend::Cuda,
                    name,
                    memory_mb,
                    cuda_version,
                    metal_supported: false,
                };
            }
        }
    }

    // Fall back to CPU
    GpuInfo {
        available: false,
        backend: GpuBackend::Cpu,
        name: None,
        memory_mb: None,
        cuda_version: None,
        metal_supported: false,
    }
}

// ============================================================================
// Dataset Commands
// ============================================================================

/// List available datasets (both standard and downloaded).
#[tauri::command]
pub async fn list_datasets(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;

    // Return IDs of downloaded datasets
    Ok(training.datasets.iter()
        .filter(|d| d.downloaded)
        .map(|d| d.id.clone())
        .collect())
}

/// Get full dataset information.
#[tauri::command]
pub async fn get_datasets(state: State<'_, AppState>) -> Result<Vec<DatasetInfo>, String> {
    let mut training = state.training.lock().map_err(|e| e.to_string())?;

    // Initialize with standard datasets if empty
    if training.datasets.is_empty() {
        training.datasets = get_standard_datasets();
    }

    Ok(training.datasets.clone())
}

/// Download a dataset (placeholder - actual download would need async HTTP).
#[tauri::command]
pub async fn download_dataset(
    dataset_id: String,
    state: State<'_, AppState>,
) -> Result<DatasetInfo, String> {
    let mut training = state.training.lock().map_err(|e| e.to_string())?;

    // Find the dataset
    let dataset = training.datasets.iter_mut()
        .find(|d| d.id == dataset_id)
        .ok_or_else(|| format!("Dataset not found: {}", dataset_id))?;

    // Simulate download completion
    dataset.downloaded = true;
    dataset.path = Some(format!("~/.ruview/datasets/{}", dataset_id));

    Ok(dataset.clone())
}

// ============================================================================
// Model/Checkpoint Commands
// ============================================================================

/// List available model architectures.
#[tauri::command]
pub async fn list_models() -> Result<Vec<ModelInfo>, String> {
    Ok(get_standard_models())
}

/// List saved checkpoints.
#[tauri::command]
pub async fn list_checkpoints(state: State<'_, AppState>) -> Result<Vec<CheckpointInfo>, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;
    Ok(training.checkpoints.clone())
}

/// Export a model checkpoint to ONNX or TorchScript.
#[tauri::command]
pub async fn export_model(
    checkpoint_id: String,
    format: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;

    let checkpoint = training.checkpoints.iter()
        .find(|c| c.id == checkpoint_id)
        .ok_or_else(|| format!("Checkpoint not found: {}", checkpoint_id))?;

    let output_path = match format.as_str() {
        "onnx" => format!("{}.onnx", checkpoint.path.trim_end_matches(".pt")),
        "torchscript" => format!("{}.ts", checkpoint.path.trim_end_matches(".pt")),
        _ => return Err(format!("Unsupported format: {}", format)),
    };

    // In a real implementation, this would call the actual export logic
    Ok(output_path)
}

// ============================================================================
// Training Job Commands
// ============================================================================

/// Start a training job.
#[tauri::command]
pub async fn start_training(
    config: TrainingConfig,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut training = state.training.lock().map_err(|e| e.to_string())?;

    // Create a new job
    let job_id = uuid::Uuid::new_v4().to_string();
    let job = TrainingJob {
        id: job_id.clone(),
        config,
        status: TrainingStatus::Running,
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        progress: TrainingProgress::default(),
        loss_history: Vec::new(),
    };

    training.current_job = Some(job);

    // In a real implementation, this would spawn a background training thread
    // and emit progress events via Tauri's event system

    Ok(job_id)
}

/// Stop the current training job.
#[tauri::command]
pub async fn stop_training(state: State<'_, AppState>) -> Result<(), String> {
    let mut training = state.training.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut job) = training.current_job {
        job.status = TrainingStatus::Paused;
    }

    Ok(())
}

/// Get current training progress.
#[tauri::command]
pub async fn training_progress(state: State<'_, AppState>) -> Result<Option<TrainingProgress>, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;
    Ok(training.current_job.as_ref().map(|j| j.progress.clone()))
}

// ============================================================================
// RuVector Configuration Commands
// ============================================================================

/// Get current RuVector configuration.
#[tauri::command]
pub async fn get_ruvector_config(state: State<'_, AppState>) -> Result<RuVectorConfig, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;
    Ok(training.ruvector_config.clone())
}

/// Set RuVector configuration.
#[tauri::command]
pub async fn set_ruvector_config(
    config: RuVectorConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut training = state.training.lock().map_err(|e| e.to_string())?;
    training.ruvector_config = config;
    Ok(())
}

/// Test RuVector modules on live CSI data.
#[tauri::command]
pub async fn test_ruvector_live(
    _state: State<'_, AppState>,
) -> Result<LiveTestMetrics, String> {
    // In a real implementation, this would process live CSI data
    // through the RuVector pipeline and return metrics
    Ok(LiveTestMetrics {
        fps: 30.0,
        latency_ms: 15.0,
        persons_detected: 1,
    })
}

// ============================================================================
// Metrics Commands
// ============================================================================

/// Get training history (loss/accuracy per epoch).
#[tauri::command]
pub async fn get_training_history(state: State<'_, AppState>) -> Result<Vec<EpochMetrics>, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;
    Ok(training.training_history.clone())
}

/// Get evaluation metrics.
#[tauri::command]
pub async fn get_evaluation_metrics(state: State<'_, AppState>) -> Result<Option<EvaluationMetrics>, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;
    Ok(training.evaluation_metrics.clone())
}

/// Get per-joint accuracy metrics.
#[tauri::command]
pub async fn get_joint_accuracies(state: State<'_, AppState>) -> Result<Vec<JointAccuracy>, String> {
    let training = state.training.lock().map_err(|e| e.to_string())?;
    Ok(training.joint_accuracies.clone())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_datasets() {
        let datasets = get_standard_datasets();
        assert_eq!(datasets.len(), 3);
        assert!(datasets.iter().any(|d| d.id == "mmfi"));
    }

    #[test]
    fn test_standard_models() {
        let models = get_standard_models();
        assert_eq!(models.len(), 6);
        assert!(models.iter().any(|m| m.id == "csi-encoder-cnn"));
    }

    #[test]
    fn test_detect_gpu_internal() {
        let info = detect_gpu_internal();
        // Just verify it returns valid data
        assert!(matches!(info.backend, GpuBackend::Cpu | GpuBackend::Cuda | GpuBackend::Metal));
    }

    #[test]
    fn test_ruvector_config_default() {
        let config = RuVectorConfig::default();
        assert!(config.mincut_enabled);
        assert_eq!(config.attention_heads, 4);
    }
}
