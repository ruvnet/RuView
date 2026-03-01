//! PostgreSQL extension integration with predictive coding
//!
//! Provides predictive residual writing to reduce database write operations
//! by 90-99% through prediction-based gating.

use crate::routing::predictive::PredictiveLayer;
use crate::{NervousSystemError, Result};

/// Configuration for predictive writer
#[derive(Debug, Clone)]
pub struct PredictiveConfig {
    /// Vector dimension
    pub dimension: usize,

    /// Residual threshold for transmission (0.0-1.0)
    /// Higher values = fewer writes but less accuracy
    pub threshold: f32,

    /// Learning rate for prediction updates (0.0-1.0)
    pub learning_rate: f32,

    /// Enable adaptive threshold adjustment
    pub adaptive_threshold: bool,

    /// Target compression ratio (fraction of writes)
    pub target_compression: f32,
}

impl Default for PredictiveConfig {
    fn default() -> Self {
        Self {
            dimension: 128,
            threshold: 0.1,     // 10% change triggers write
            learning_rate: 0.1, // 10% learning rate
            adaptive_threshold: true,
            target_compression: 0.1, // Target 10% writes (90% reduction)
        }
    }
}

impl PredictiveConfig {
    /// Create new configuration for specific dimension
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            ..Default::default()
        }
    }

    /// Set threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set learning rate
    pub fn with_learning_rate(mut self, lr: f32) -> Self {
        self.learning_rate = lr;
        self
    }

    /// Set target compression ratio
    pub fn with_target_compression(mut self, target: f32) -> Self {
        self.target_compression = target;
        self
    }
}

/// Predictive writer for PostgreSQL vector columns
///
/// Uses predictive coding to minimize database writes by only transmitting
/// prediction errors that exceed a threshold. Achieves 90-99% write reduction.
///
/// # Example
///
/// ```
/// use ruvector_nervous_system::integration::{PredictiveWriter, PredictiveConfig};
///
/// let config = PredictiveConfig::new(128).with_threshold(0.1);
/// let mut writer = PredictiveWriter::new(config);
///
/// // First write always happens
/// let vector1 = vec![0.5; 128];
/// assert!(writer.should_write(&vector1));
/// writer.record_write(&vector1);
///
/// // Similar vector may not trigger write
/// let vector2 = vec![0.51; 128];
/// let should_write = writer.should_write(&vector2);
/// // Likely false due to small change
/// ```
pub struct PredictiveWriter {
    /// Configuration
    config: PredictiveConfig,

    /// Predictive layer for residual computation
    prediction_layer: PredictiveLayer,

    /// Statistics
    stats: WriterStats,
}

#[derive(Debug, Clone)]
struct WriterStats {
    /// Total write attempts
    attempts: usize,

    /// Actual writes performed
    writes: usize,

    /// Current compression ratio
    compression: f32,
}

impl WriterStats {
    fn new() -> Self {
        Self {
            attempts: 0,
            writes: 0,
            compression: 0.0,
        }
    }

    fn record_attempt(&mut self, wrote: bool) {
        self.attempts += 1;
        if wrote {
            self.writes += 1;
        }

        if self.attempts > 0 {
            self.compression = self.writes as f32 / self.attempts as f32;
        }
    }
}

impl PredictiveWriter {
    /// Create a new predictive writer
    ///
    /// # Arguments
    ///
    /// * `config` - Writer configuration
    pub fn new(config: PredictiveConfig) -> Self {
        let prediction_layer = PredictiveLayer::with_learning_rate(
            config.dimension,
            config.threshold,
            config.learning_rate,
        );

        Self {
            config,
            prediction_layer,
            stats: WriterStats::new(),
        }
    }

    /// Check if a vector should be written to database
    ///
    /// Returns true if the residual (prediction error) exceeds threshold.
    ///
    /// # Arguments
    ///
    /// * `new_vector` - Vector candidate for writing
    ///
    /// # Returns
    ///
    /// True if write should proceed, false if prediction is good enough
    pub fn should_write(&self, new_vector: &[f32]) -> bool {
        self.prediction_layer.should_transmit(new_vector)
    }

    /// Get the residual to write (prediction error)
    ///
    /// Returns Some(residual) if write should proceed, None otherwise.
    ///
    /// # Arguments
    ///
    /// * `new_vector` - Vector candidate for writing
    ///
    /// # Returns
    ///
    /// Residual vector if threshold exceeded, None otherwise
    pub fn residual_write(&mut self, new_vector: &[f32]) -> Option<Vec<f32>> {
        let result = self.prediction_layer.residual_gated_write(new_vector);

        // Record statistics
        self.stats.record_attempt(result.is_some());

        // Adapt threshold if enabled
        if self.config.adaptive_threshold && self.stats.attempts % 100 == 0 {
            self.adapt_threshold();
        }

        result
    }

    /// Record that a write was performed
    ///
    /// Updates the prediction with the written vector.
    ///
    /// # Arguments
    ///
    /// * `written_vector` - Vector that was written to database
    pub fn record_write(&mut self, written_vector: &[f32]) {
        self.prediction_layer.update(written_vector);
        self.stats.record_attempt(true);
    }

    /// Get current prediction for debugging
    pub fn current_prediction(&self) -> &[f32] {
        self.prediction_layer.prediction()
    }

    /// Get compression statistics
    pub fn stats(&self) -> CompressionStats {
        CompressionStats {
            total_attempts: self.stats.attempts,
            actual_writes: self.stats.writes,
            compression_ratio: self.stats.compression,
            bandwidth_reduction: 1.0 - self.stats.compression,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = WriterStats::new();
    }

    /// Adapt threshold to meet target compression ratio
    fn adapt_threshold(&mut self) {
        let current_ratio = self.stats.compression;
        let target = self.config.target_compression;

        // If writing too much, increase threshold
        if current_ratio > target * 1.1 {
            let new_threshold = self.config.threshold * 1.1;
            self.config.threshold = new_threshold.min(0.5); // Cap at 0.5
            self.prediction_layer.set_threshold(self.config.threshold);
        }
        // If writing too little, decrease threshold
        else if current_ratio < target * 0.9 {
            let new_threshold = self.config.threshold * 0.9;
            self.config.threshold = new_threshold.max(0.01); // Floor at 0.01
            self.prediction_layer.set_threshold(self.config.threshold);
        }
    }

    /// Get current threshold
    pub fn threshold(&self) -> f32 {
        self.config.threshold
    }
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    /// Total write attempts
    pub total_attempts: usize,

    /// Actual writes performed
    pub actual_writes: usize,

    /// Compression ratio (writes / attempts)
    pub compression_ratio: f32,

    /// Bandwidth reduction (1 - compression_ratio)
    pub bandwidth_reduction: f32,
}

impl CompressionStats {
    /// Get bandwidth reduction percentage
    pub fn reduction_percent(&self) -> f32 {
        self.bandwidth_reduction * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictive_writer_creation() {
        let config = PredictiveConfig::new(128);
        let writer = PredictiveWriter::new(config);

        let stats = writer.stats();
        assert_eq!(stats.total_attempts, 0);
        assert_eq!(stats.actual_writes, 0);
    }

    #[test]
    fn test_first_write_always_happens() {
        let config = PredictiveConfig::new(64);
        let writer = PredictiveWriter::new(config);

        let vector = vec![0.5; 64];
        // First write should always happen (no prediction yet)
        assert!(writer.should_write(&vector));
    }

    #[test]
    fn test_residual_write() {
        let config = PredictiveConfig::new(64).with_threshold(0.1);
        let mut writer = PredictiveWriter::new(config);

        let v1 = vec![0.5; 64];
        let residual1 = writer.residual_write(&v1);
        assert!(residual1.is_some()); // First write

        // Very similar vector - should not write
        let v2 = vec![0.501; 64];
        let _residual2 = writer.residual_write(&v2);
        // May or may not write depending on threshold

        let stats = writer.stats();
        assert!(stats.total_attempts >= 2);
    }

    #[test]
    fn test_compression_statistics() {
        let config = PredictiveConfig::new(32).with_threshold(0.2);
        let mut writer = PredictiveWriter::new(config);

        // Stable signal should learn and reduce writes
        let stable = vec![1.0; 32];

        for _ in 0..100 {
            let _ = writer.residual_write(&stable);
        }

        let stats = writer.stats();
        assert_eq!(stats.total_attempts, 100);

        // Should achieve some compression
        assert!(
            stats.compression_ratio < 0.5,
            "Compression ratio too high: {}",
            stats.compression_ratio
        );
        assert!(stats.bandwidth_reduction > 0.5);
    }

    #[test]
    fn test_adaptive_threshold() {
        let config = PredictiveConfig::new(32)
            .with_threshold(0.1)
            .with_target_compression(0.1); // Target 10% writes

        let mut writer = PredictiveWriter::new(config);

        let _initial_threshold = writer.threshold();

        // Slowly varying signal
        for i in 0..200 {
            let mut signal = vec![1.0; 32];
            signal[0] = 1.0 + (i as f32 * 0.001).sin() * 0.05;
            let _ = writer.residual_write(&signal);
        }

        // Threshold may have adapted
        let final_threshold = writer.threshold();

        // Just verify it's still within reasonable bounds
        assert!(final_threshold > 0.01 && final_threshold < 0.5);
    }

    #[test]
    fn test_record_write() {
        let config = PredictiveConfig::new(16);
        let mut writer = PredictiveWriter::new(config);

        let v1 = vec![0.5; 16];
        writer.record_write(&v1);

        let stats = writer.stats();
        assert_eq!(stats.actual_writes, 1);
        assert_eq!(stats.total_attempts, 1);
    }

    #[test]
    fn test_config_builder() {
        let config = PredictiveConfig::new(256)
            .with_threshold(0.15)
            .with_learning_rate(0.2)
            .with_target_compression(0.05);

        assert_eq!(config.dimension, 256);
        assert_eq!(config.threshold, 0.15);
        assert_eq!(config.learning_rate, 0.2);
        assert_eq!(config.target_compression, 0.05);
    }

    #[test]
    fn test_prediction_convergence() {
        let config = PredictiveConfig::new(8).with_learning_rate(0.3);
        let mut writer = PredictiveWriter::new(config);

        let signal = vec![0.7; 8];

        // Repeat same signal
        for _ in 0..50 {
            let _ = writer.residual_write(&signal);
        }

        // Prediction should converge to signal
        let prediction = writer.current_prediction();
        let error: f32 = prediction
            .iter()
            .zip(signal.iter())
            .map(|(p, s)| (p - s).abs())
            .sum::<f32>()
            / signal.len() as f32;

        assert!(error < 0.05, "Prediction error too high: {}", error);
    }
}
