//! Core trait definitions for the WiFi-DensePose system.
//!
//! This module defines the fundamental abstractions used throughout the system,
//! enabling a modular and testable architecture.
//!
//! # Traits
//!
//! - [`SignalProcessor`]: Process raw CSI frames into neural network-ready tensors
//! - [`NeuralInference`]: Run pose estimation inference on processed signals
//! - [`DataStore`]: Persist and retrieve CSI data and pose estimates
//!
//! # Design Philosophy
//!
//! These traits are designed with the following principles:
//!
//! 1. **Single Responsibility**: Each trait handles one concern
//! 2. **Testability**: All traits can be easily mocked for unit testing
//! 3. **Async-Ready**: Async versions available with the `async` feature
//! 4. **Error Handling**: Consistent use of `Result` types with domain errors

use crate::error::{CoreResult, InferenceError, SignalError, StorageError};
use crate::types::{CsiFrame, FrameId, PoseEstimate, ProcessedSignal, Timestamp};

/// Configuration for signal processing.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SignalProcessorConfig {
    /// Number of frames to buffer before processing
    pub buffer_size: usize,
    /// Sampling rate in Hz
    pub sample_rate_hz: f64,
    /// Whether to apply noise filtering
    pub apply_noise_filter: bool,
    /// Noise filter cutoff frequency in Hz
    pub filter_cutoff_hz: f64,
    /// Whether to normalize amplitudes
    pub normalize_amplitude: bool,
    /// Whether to unwrap phases
    pub unwrap_phase: bool,
    /// Window function for spectral analysis
    pub window_function: WindowFunction,
}

impl Default for SignalProcessorConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64,
            sample_rate_hz: 1000.0,
            apply_noise_filter: true,
            filter_cutoff_hz: 50.0,
            normalize_amplitude: true,
            unwrap_phase: true,
            window_function: WindowFunction::Hann,
        }
    }
}

/// Window functions for spectral analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum WindowFunction {
    /// Rectangular window (no windowing)
    Rectangular,
    /// Hann window
    #[default]
    Hann,
    /// Hamming window
    Hamming,
    /// Blackman window
    Blackman,
    /// Kaiser window
    Kaiser,
}

/// Signal processor for converting raw CSI frames into processed signals.
///
/// Implementations of this trait handle:
/// - Buffering and aggregating CSI frames
/// - Noise filtering and signal conditioning
/// - Phase unwrapping and amplitude normalization
/// - Feature extraction
///
/// # Example
///
/// ```ignore
/// use wifi_densepose_core::{SignalProcessor, CsiFrame};
///
/// fn process_frames(processor: &mut impl SignalProcessor, frames: Vec<CsiFrame>) {
///     for frame in frames {
///         if let Err(e) = processor.push_frame(frame) {
///             eprintln!("Failed to push frame: {}", e);
///         }
///     }
///
///     if let Some(signal) = processor.try_process() {
///         println!("Processed signal with {} time steps", signal.num_time_steps());
///     }
/// }
/// ```
pub trait SignalProcessor: Send + Sync {
    /// Returns the current configuration.
    fn config(&self) -> &SignalProcessorConfig;

    /// Updates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    fn set_config(&mut self, config: SignalProcessorConfig) -> Result<(), SignalError>;

    /// Pushes a new CSI frame into the processing buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame is invalid or the buffer is full.
    fn push_frame(&mut self, frame: CsiFrame) -> Result<(), SignalError>;

    /// Attempts to process the buffered frames.
    ///
    /// Returns `None` if insufficient frames are buffered.
    /// Returns `Some(ProcessedSignal)` on successful processing.
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    fn try_process(&mut self) -> Result<Option<ProcessedSignal>, SignalError>;

    /// Forces processing of whatever frames are buffered.
    ///
    /// # Errors
    ///
    /// Returns an error if no frames are buffered or processing fails.
    fn force_process(&mut self) -> Result<ProcessedSignal, SignalError>;

    /// Returns the number of frames currently buffered.
    fn buffered_frame_count(&self) -> usize;

    /// Clears the frame buffer.
    fn clear_buffer(&mut self);

    /// Resets the processor to its initial state.
    fn reset(&mut self);
}

/// Configuration for neural network inference.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InferenceConfig {
    /// Path to the model file
    pub model_path: String,
    /// Device to run inference on
    pub device: InferenceDevice,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Number of threads for CPU inference
    pub num_threads: usize,
    /// Confidence threshold for detections
    pub confidence_threshold: f32,
    /// Non-maximum suppression threshold
    pub nms_threshold: f32,
    /// Whether to use half precision (FP16)
    pub use_fp16: bool,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            device: InferenceDevice::Cpu,
            max_batch_size: 8,
            num_threads: 4,
            confidence_threshold: 0.5,
            nms_threshold: 0.45,
            use_fp16: false,
        }
    }
}

/// Device for running neural network inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum InferenceDevice {
    /// CPU inference
    #[default]
    Cpu,
    /// CUDA GPU inference
    Cuda {
        /// GPU device index
        device_id: usize,
    },
    /// TensorRT accelerated inference
    TensorRt {
        /// GPU device index
        device_id: usize,
    },
    /// CoreML (Apple Silicon)
    CoreMl,
    /// WebGPU for browser environments
    WebGpu,
}

/// Neural network inference engine for pose estimation.
///
/// Implementations of this trait handle:
/// - Loading and managing neural network models
/// - Running inference on processed signals
/// - Post-processing outputs into pose estimates
///
/// # Example
///
/// ```ignore
/// use wifi_densepose_core::{NeuralInference, ProcessedSignal};
///
/// async fn estimate_pose(
///     engine: &impl NeuralInference,
///     signal: ProcessedSignal,
/// ) -> Result<PoseEstimate, InferenceError> {
///     engine.infer(signal).await
/// }
/// ```
pub trait NeuralInference: Send + Sync {
    /// Returns the current configuration.
    fn config(&self) -> &InferenceConfig;

    /// Returns `true` if the model is loaded and ready.
    fn is_ready(&self) -> bool;

    /// Returns the model version string.
    fn model_version(&self) -> &str;

    /// Loads the model from the configured path.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded.
    fn load_model(&mut self) -> Result<(), InferenceError>;

    /// Unloads the current model to free resources.
    fn unload_model(&mut self);

    /// Runs inference on a single processed signal.
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
    fn infer(&self, signal: &ProcessedSignal) -> Result<PoseEstimate, InferenceError>;

    /// Runs inference on a batch of processed signals.
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
    fn infer_batch(&self, signals: &[ProcessedSignal])
        -> Result<Vec<PoseEstimate>, InferenceError>;

    /// Warms up the model by running a dummy inference.
    ///
    /// # Errors
    ///
    /// Returns an error if warmup fails.
    fn warmup(&mut self) -> Result<(), InferenceError>;

    /// Returns performance statistics.
    fn stats(&self) -> InferenceStats;
}

/// Performance statistics for neural network inference.
#[derive(Debug, Clone, Default)]
pub struct InferenceStats {
    /// Total number of inferences performed
    pub total_inferences: u64,
    /// Average inference latency in milliseconds
    pub avg_latency_ms: f64,
    /// 95th percentile latency in milliseconds
    pub p95_latency_ms: f64,
    /// Maximum latency in milliseconds
    pub max_latency_ms: f64,
    /// Inferences per second throughput
    pub throughput: f64,
    /// GPU memory usage in bytes (if applicable)
    pub gpu_memory_bytes: Option<u64>,
}

/// Query options for data store operations.
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Number of results to skip
    pub offset: Option<usize>,
    /// Start time filter (inclusive)
    pub start_time: Option<Timestamp>,
    /// End time filter (inclusive)
    pub end_time: Option<Timestamp>,
    /// Device ID filter
    pub device_id: Option<String>,
    /// Sort order
    pub sort_order: SortOrder,
}

/// Sort order for query results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Ascending order (oldest first)
    #[default]
    Ascending,
    /// Descending order (newest first)
    Descending,
}

/// Data storage trait for persisting and retrieving CSI data and pose estimates.
///
/// Implementations can use various backends:
/// - PostgreSQL/SQLite for relational storage
/// - Redis for caching
/// - Time-series databases for efficient temporal queries
///
/// # Example
///
/// ```ignore
/// use wifi_densepose_core::{DataStore, CsiFrame, PoseEstimate};
///
/// async fn save_and_query(
///     store: &impl DataStore,
///     frame: CsiFrame,
///     estimate: PoseEstimate,
/// ) {
///     store.store_csi_frame(&frame).await?;
///     store.store_pose_estimate(&estimate).await?;
///
///     let recent = store.get_recent_estimates(10).await?;
///     println!("Found {} recent estimates", recent.len());
/// }
/// ```
pub trait DataStore: Send + Sync {
    /// Returns `true` if the store is connected and ready.
    fn is_connected(&self) -> bool;

    /// Stores a CSI frame.
    ///
    /// # Errors
    ///
    /// Returns an error if the store operation fails.
    fn store_csi_frame(&self, frame: &CsiFrame) -> Result<(), StorageError>;

    /// Retrieves a CSI frame by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame is not found or retrieval fails.
    fn get_csi_frame(&self, id: &FrameId) -> Result<CsiFrame, StorageError>;

    /// Retrieves CSI frames matching the query options.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn query_csi_frames(&self, options: &QueryOptions) -> Result<Vec<CsiFrame>, StorageError>;

    /// Stores a pose estimate.
    ///
    /// # Errors
    ///
    /// Returns an error if the store operation fails.
    fn store_pose_estimate(&self, estimate: &PoseEstimate) -> Result<(), StorageError>;

    /// Retrieves a pose estimate by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the estimate is not found or retrieval fails.
    fn get_pose_estimate(&self, id: &FrameId) -> Result<PoseEstimate, StorageError>;

    /// Retrieves pose estimates matching the query options.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn query_pose_estimates(
        &self,
        options: &QueryOptions,
    ) -> Result<Vec<PoseEstimate>, StorageError>;

    /// Retrieves the N most recent pose estimates.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn get_recent_estimates(&self, count: usize) -> Result<Vec<PoseEstimate>, StorageError>;

    /// Deletes CSI frames older than the given timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    fn delete_csi_frames_before(&self, timestamp: &Timestamp) -> Result<u64, StorageError>;

    /// Deletes pose estimates older than the given timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    fn delete_pose_estimates_before(&self, timestamp: &Timestamp) -> Result<u64, StorageError>;

    /// Returns storage statistics.
    fn stats(&self) -> StorageStats;
}

/// Storage statistics.
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    /// Total number of CSI frames stored
    pub csi_frame_count: u64,
    /// Total number of pose estimates stored
    pub pose_estimate_count: u64,
    /// Total storage size in bytes
    pub total_size_bytes: u64,
    /// Oldest record timestamp
    pub oldest_record: Option<Timestamp>,
    /// Newest record timestamp
    pub newest_record: Option<Timestamp>,
}

// =============================================================================
// Async Trait Definitions (with `async` feature)
// =============================================================================

#[cfg(feature = "async")]
use async_trait::async_trait;

/// Async version of [`SignalProcessor`].
#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncSignalProcessor: Send + Sync {
    /// Returns the current configuration.
    fn config(&self) -> &SignalProcessorConfig;

    /// Updates the configuration.
    async fn set_config(&mut self, config: SignalProcessorConfig) -> Result<(), SignalError>;

    /// Pushes a new CSI frame into the processing buffer.
    async fn push_frame(&mut self, frame: CsiFrame) -> Result<(), SignalError>;

    /// Attempts to process the buffered frames.
    async fn try_process(&mut self) -> Result<Option<ProcessedSignal>, SignalError>;

    /// Forces processing of whatever frames are buffered.
    async fn force_process(&mut self) -> Result<ProcessedSignal, SignalError>;

    /// Returns the number of frames currently buffered.
    fn buffered_frame_count(&self) -> usize;

    /// Clears the frame buffer.
    async fn clear_buffer(&mut self);

    /// Resets the processor to its initial state.
    async fn reset(&mut self);
}

/// Async version of [`NeuralInference`].
#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncNeuralInference: Send + Sync {
    /// Returns the current configuration.
    fn config(&self) -> &InferenceConfig;

    /// Returns `true` if the model is loaded and ready.
    fn is_ready(&self) -> bool;

    /// Returns the model version string.
    fn model_version(&self) -> &str;

    /// Loads the model from the configured path.
    async fn load_model(&mut self) -> Result<(), InferenceError>;

    /// Unloads the current model to free resources.
    async fn unload_model(&mut self);

    /// Runs inference on a single processed signal.
    async fn infer(&self, signal: &ProcessedSignal) -> Result<PoseEstimate, InferenceError>;

    /// Runs inference on a batch of processed signals.
    async fn infer_batch(
        &self,
        signals: &[ProcessedSignal],
    ) -> Result<Vec<PoseEstimate>, InferenceError>;

    /// Warms up the model by running a dummy inference.
    async fn warmup(&mut self) -> Result<(), InferenceError>;

    /// Returns performance statistics.
    fn stats(&self) -> InferenceStats;
}

/// Async version of [`DataStore`].
#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncDataStore: Send + Sync {
    /// Returns `true` if the store is connected and ready.
    fn is_connected(&self) -> bool;

    /// Stores a CSI frame.
    async fn store_csi_frame(&self, frame: &CsiFrame) -> Result<(), StorageError>;

    /// Retrieves a CSI frame by ID.
    async fn get_csi_frame(&self, id: &FrameId) -> Result<CsiFrame, StorageError>;

    /// Retrieves CSI frames matching the query options.
    async fn query_csi_frames(&self, options: &QueryOptions) -> Result<Vec<CsiFrame>, StorageError>;

    /// Stores a pose estimate.
    async fn store_pose_estimate(&self, estimate: &PoseEstimate) -> Result<(), StorageError>;

    /// Retrieves a pose estimate by ID.
    async fn get_pose_estimate(&self, id: &FrameId) -> Result<PoseEstimate, StorageError>;

    /// Retrieves pose estimates matching the query options.
    async fn query_pose_estimates(
        &self,
        options: &QueryOptions,
    ) -> Result<Vec<PoseEstimate>, StorageError>;

    /// Retrieves the N most recent pose estimates.
    async fn get_recent_estimates(&self, count: usize) -> Result<Vec<PoseEstimate>, StorageError>;

    /// Deletes CSI frames older than the given timestamp.
    async fn delete_csi_frames_before(&self, timestamp: &Timestamp) -> Result<u64, StorageError>;

    /// Deletes pose estimates older than the given timestamp.
    async fn delete_pose_estimates_before(
        &self,
        timestamp: &Timestamp,
    ) -> Result<u64, StorageError>;

    /// Returns storage statistics.
    fn stats(&self) -> StorageStats;
}

// =============================================================================
// Extension Traits
// =============================================================================

/// Extension trait for pipeline composition.
pub trait Pipeline: Send + Sync {
    /// The input type for this pipeline stage.
    type Input;
    /// The output type for this pipeline stage.
    type Output;
    /// The error type for this pipeline stage.
    type Error;

    /// Processes input and produces output.
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

/// Trait for types that can validate themselves.
pub trait Validate {
    /// Validates the instance.
    ///
    /// # Errors
    ///
    /// Returns an error describing validation failures.
    fn validate(&self) -> CoreResult<()>;
}

/// Trait for types that can be reset to a default state.
pub trait Resettable {
    /// Resets the instance to its initial state.
    fn reset(&mut self);
}

/// Trait for types that track health status.
pub trait HealthCheck {
    /// Health status of the component.
    type Status;

    /// Performs a health check and returns the current status.
    fn health_check(&self) -> Self::Status;

    /// Returns `true` if the component is healthy.
    fn is_healthy(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_processor_config_default() {
        let config = SignalProcessorConfig::default();
        assert_eq!(config.buffer_size, 64);
        assert!(config.apply_noise_filter);
        assert!(config.sample_rate_hz > 0.0);
    }

    #[test]
    fn test_inference_config_default() {
        let config = InferenceConfig::default();
        assert_eq!(config.device, InferenceDevice::Cpu);
        assert!(config.confidence_threshold > 0.0);
        assert!(config.max_batch_size > 0);
    }

    #[test]
    fn test_query_options_default() {
        let options = QueryOptions::default();
        assert!(options.limit.is_none());
        assert!(options.offset.is_none());
        assert_eq!(options.sort_order, SortOrder::Ascending);
    }

    #[test]
    fn test_inference_device_variants() {
        let cpu = InferenceDevice::Cpu;
        let cuda = InferenceDevice::Cuda { device_id: 0 };
        let tensorrt = InferenceDevice::TensorRt { device_id: 1 };

        assert_eq!(cpu, InferenceDevice::Cpu);
        assert!(matches!(cuda, InferenceDevice::Cuda { device_id: 0 }));
        assert!(matches!(tensorrt, InferenceDevice::TensorRt { device_id: 1 }));
    }
}
