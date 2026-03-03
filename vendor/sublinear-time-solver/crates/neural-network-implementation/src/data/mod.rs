//! Data processing and management for temporal neural networks
//!
//! This module provides data loading, preprocessing, and batching functionality
//! specifically designed for temporal trajectory prediction tasks.

use crate::{
    config::Config,
    error::{Result, TemporalNeuralError},
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod loader;
pub mod preprocessing;
pub mod augmentation;

pub use loader::{CsvLoader, DataLoader};
pub use preprocessing::{Preprocessor, NormalizationStrategy};
pub use augmentation::{DataAugmentor, AugmentationConfig};

/// Time series data structure for temporal neural networks
#[derive(Debug, Clone)]
pub struct TimeSeriesData {
    /// Raw feature data (features x time)
    pub features: DMatrix<f64>,
    /// Feature names
    pub feature_names: Vec<String>,
    /// Timestamps (optional)
    pub timestamps: Option<Vec<f64>>,
    /// Sample rate in Hz
    pub sample_rate: f64,
    /// Data metadata
    pub metadata: DataMetadata,
}

/// Metadata about the dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMetadata {
    /// Dataset name
    pub name: String,
    /// Number of samples
    pub num_samples: usize,
    /// Number of features
    pub num_features: usize,
    /// Duration in seconds
    pub duration_seconds: f64,
    /// Data source
    pub source: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Preprocessing applied
    pub preprocessing_history: Vec<String>,
}

/// Windowed sample for training/evaluation
#[derive(Debug, Clone)]
pub struct WindowedSample {
    /// Input window (features x window_length)
    pub input: DMatrix<f64>,
    /// Target value (typically 2D position)
    pub target: DVector<f64>,
    /// Sample metadata
    pub metadata: SampleMetadata,
}

/// Metadata for individual samples
#[derive(Debug, Clone)]
pub struct SampleMetadata {
    /// Original sample index
    pub original_index: usize,
    /// Start time in seconds
    pub start_time: f64,
    /// End time in seconds
    pub end_time: f64,
    /// Target time (prediction horizon)
    pub target_time: f64,
    /// Data quality score (0.0 to 1.0)
    pub quality_score: f64,
}

/// Data splits for training, validation, and testing
#[derive(Debug, Clone)]
pub struct DataSplits {
    /// Training data
    pub train: Vec<WindowedSample>,
    /// Validation data
    pub val: Vec<WindowedSample>,
    /// Test data
    pub test: Vec<WindowedSample>,
    /// Split configuration
    pub config: SplitConfig,
}

/// Configuration for data splitting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitConfig {
    /// Training set fraction
    pub train_fraction: f64,
    /// Validation set fraction
    pub val_fraction: f64,
    /// Test set fraction
    pub test_fraction: f64,
    /// Whether to shuffle data
    pub shuffle: bool,
    /// Random seed for reproducibility
    pub random_seed: Option<u64>,
    /// Stratification strategy
    pub stratify_by: Option<String>,
}

impl TimeSeriesData {
    /// Create new time series data
    pub fn new(
        features: DMatrix<f64>,
        feature_names: Vec<String>,
        sample_rate: f64,
        name: String,
    ) -> Self {
        let num_samples = features.ncols();
        let num_features = features.nrows();
        let duration_seconds = num_samples as f64 / sample_rate;

        let metadata = DataMetadata {
            name: name.clone(),
            num_samples,
            num_features,
            duration_seconds,
            source: "unknown".to_string(),
            created_at: chrono::Utc::now(),
            preprocessing_history: Vec::new(),
        };

        Self {
            features,
            feature_names,
            timestamps: None,
            sample_rate,
            metadata,
        }
    }

    /// Load from CSV file
    pub fn from_csv<P: AsRef<Path>>(path: P) -> Result<Self> {
        CsvLoader::load(path)
    }

    /// Create windowed samples for training
    pub fn create_windowed_samples(
        &self,
        window_length: usize,
        horizon_length: usize,
        stride: usize,
    ) -> Result<Vec<WindowedSample>> {
        if window_length == 0 || horizon_length == 0 {
            return Err(TemporalNeuralError::DataError {
                message: "Window length and horizon length must be positive".to_string(),
                context: None,
            });
        }

        if window_length + horizon_length > self.features.ncols() {
            return Err(TemporalNeuralError::DataError {
                message: "Window + horizon exceeds data length".to_string(),
                context: Some(format!(
                    "window: {}, horizon: {}, data: {}",
                    window_length, horizon_length, self.features.ncols()
                )),
            });
        }

        let mut samples = Vec::new();
        let num_samples = self.features.ncols();
        let max_start = num_samples - window_length - horizon_length + 1;

        for start_idx in (0..max_start).step_by(stride) {
            let end_idx = start_idx + window_length;
            let target_idx = end_idx + horizon_length - 1;

            // Extract input window
            let input = self.features.view((0, start_idx), (self.features.nrows(), window_length)).into();

            // Extract target (assuming we predict position)
            let target = if self.features.nrows() >= 2 {
                DVector::from_vec(vec![
                    self.features[(0, target_idx)], // x
                    self.features[(1, target_idx)], // y
                ])
            } else {
                return Err(TemporalNeuralError::DataError {
                    message: "Need at least 2 features for position prediction".to_string(),
                    context: None,
                });
            };

            // Create sample metadata
            let start_time = start_idx as f64 / self.sample_rate;
            let end_time = end_idx as f64 / self.sample_rate;
            let target_time = target_idx as f64 / self.sample_rate;

            let metadata = SampleMetadata {
                original_index: start_idx,
                start_time,
                end_time,
                target_time,
                quality_score: self.compute_sample_quality(&input),
            };

            samples.push(WindowedSample {
                input,
                target,
                metadata,
            });
        }

        Ok(samples)
    }

    /// Perform temporal split (no shuffling across time)
    pub fn temporal_split(
        &self,
        train_fraction: f64,
        val_fraction: f64,
        test_fraction: f64,
    ) -> Result<DataSplits> {
        self.temporal_split_with_config(&SplitConfig {
            train_fraction,
            val_fraction,
            test_fraction,
            shuffle: false, // Never shuffle temporal data
            random_seed: None,
            stratify_by: None,
        })
    }

    /// Perform temporal split with configuration
    pub fn temporal_split_with_config(&self, config: &SplitConfig) -> Result<DataSplits> {
        // Validate fractions
        let total_fraction = config.train_fraction + config.val_fraction + config.test_fraction;
        if (total_fraction - 1.0).abs() > 1e-6 {
            return Err(TemporalNeuralError::DataError {
                message: format!("Split fractions must sum to 1.0, got {}", total_fraction),
                context: None,
            });
        }

        // Create windowed samples (using default parameters)
        let window_length = 256; // Default: 128ms at 2kHz
        let horizon_length = 1000; // Default: 500ms at 2kHz
        let stride = 10; // Overlap samples

        let all_samples = self.create_windowed_samples(window_length, horizon_length, stride)?;

        if all_samples.is_empty() {
            return Err(TemporalNeuralError::DataError {
                message: "No samples created from data".to_string(),
                context: None,
            });
        }

        // Temporal split (no shuffling)
        let n_total = all_samples.len();
        let n_train = (n_total as f64 * config.train_fraction) as usize;
        let n_val = (n_total as f64 * config.val_fraction) as usize;

        let train = all_samples[0..n_train].to_vec();
        let val = all_samples[n_train..n_train + n_val].to_vec();
        let test = all_samples[n_train + n_val..].to_vec();

        Ok(DataSplits {
            train,
            val,
            test,
            config: config.clone(),
        })
    }

    /// Compute quality score for a sample
    fn compute_sample_quality(&self, input: &DMatrix<f64>) -> f64 {
        // Simple quality metrics:
        // 1. No NaN or infinite values
        // 2. Reasonable variance (not flat line)
        // 3. No extreme outliers

        let mut quality = 1.0;

        // Check for invalid values
        for &val in input.iter() {
            if !val.is_finite() {
                quality *= 0.1; // Heavy penalty for invalid data
            }
        }

        // Check variance
        for row in 0..input.nrows() {
            let row_data: Vec<f64> = input.row(row).iter().cloned().collect();
            let mean = row_data.iter().sum::<f64>() / row_data.len() as f64;
            let variance = row_data.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / row_data.len() as f64;

            if variance < 1e-6 {
                quality *= 0.5; // Penalty for flat data
            }
        }

        quality.clamp(0.0, 1.0)
    }

    /// Get statistics about the data
    pub fn get_statistics(&self) -> DataStatistics {
        let n_samples = self.features.ncols();
        let n_features = self.features.nrows();

        let mut feature_stats = Vec::new();
        for i in 0..n_features {
            let row_data: Vec<f64> = self.features.row(i).iter().cloned().collect();
            let mean = row_data.iter().sum::<f64>() / row_data.len() as f64;
            let variance = row_data.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / row_data.len() as f64;

            let min_val = row_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_val = row_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            feature_stats.push(FeatureStatistics {
                name: self.feature_names.get(i).cloned().unwrap_or_else(|| format!("feature_{}", i)),
                mean,
                std_dev: variance.sqrt(),
                min_value: min_val,
                max_value: max_val,
                num_samples: n_samples,
            });
        }

        DataStatistics {
            num_samples: n_samples,
            num_features: n_features,
            duration_seconds: self.metadata.duration_seconds,
            sample_rate: self.sample_rate,
            feature_stats,
            has_missing_values: self.has_missing_values(),
            data_quality_score: self.compute_overall_quality(),
        }
    }

    /// Check if data has missing values
    fn has_missing_values(&self) -> bool {
        self.features.iter().any(|&x| !x.is_finite())
    }

    /// Compute overall data quality score
    fn compute_overall_quality(&self) -> f64 {
        if self.has_missing_values() {
            return 0.5;
        }

        // Check for reasonable data ranges and variance
        let mut quality_score = 1.0;
        for i in 0..self.features.nrows() {
            let row_data: Vec<f64> = self.features.row(i).iter().cloned().collect();
            let variance = {
                let mean = row_data.iter().sum::<f64>() / row_data.len() as f64;
                row_data.iter()
                    .map(|x| (x - mean).powi(2))
                    .sum::<f64>() / row_data.len() as f64
            };

            if variance < 1e-6 {
                quality_score *= 0.8; // Penalty for low variance
            }
        }

        quality_score.clamp(0.0, 1.0)
    }
}

/// Statistics about the dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStatistics {
    /// Number of samples
    pub num_samples: usize,
    /// Number of features
    pub num_features: usize,
    /// Duration in seconds
    pub duration_seconds: f64,
    /// Sample rate in Hz
    pub sample_rate: f64,
    /// Per-feature statistics
    pub feature_stats: Vec<FeatureStatistics>,
    /// Whether data has missing values
    pub has_missing_values: bool,
    /// Overall data quality score
    pub data_quality_score: f64,
}

/// Statistics for individual features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStatistics {
    /// Feature name
    pub name: String,
    /// Mean value
    pub mean: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Minimum value
    pub min_value: f64,
    /// Maximum value
    pub max_value: f64,
    /// Number of samples
    pub num_samples: usize,
}

impl DataSplits {
    /// Get split sizes
    pub fn get_sizes(&self) -> (usize, usize, usize) {
        (self.train.len(), self.val.len(), self.test.len())
    }

    /// Validate splits
    pub fn validate(&self) -> Result<()> {
        if self.train.is_empty() {
            return Err(TemporalNeuralError::DataError {
                message: "Training set is empty".to_string(),
                context: None,
            });
        }

        if self.val.is_empty() {
            return Err(TemporalNeuralError::DataError {
                message: "Validation set is empty".to_string(),
                context: None,
            });
        }

        if self.test.is_empty() {
            return Err(TemporalNeuralError::DataError {
                message: "Test set is empty".to_string(),
                context: None,
            });
        }

        // Check input/output consistency
        let first_train = &self.train[0];
        let input_shape = (first_train.input.nrows(), first_train.input.ncols());
        let output_dim = first_train.target.len();

        for split_name in ["train", "val", "test"] {
            let samples = match split_name {
                "train" => &self.train,
                "val" => &self.val,
                "test" => &self.test,
                _ => unreachable!(),
            };

            for (i, sample) in samples.iter().enumerate() {
                let sample_input_shape = (sample.input.nrows(), sample.input.ncols());
                if sample_input_shape != input_shape {
                    return Err(TemporalNeuralError::DataError {
                        message: format!(
                            "Input shape mismatch in {} set, sample {}: expected {:?}, got {:?}",
                            split_name, i, input_shape, sample_input_shape
                        ),
                        context: None,
                    });
                }

                if sample.target.len() != output_dim {
                    return Err(TemporalNeuralError::DataError {
                        message: format!(
                            "Output dimension mismatch in {} set, sample {}: expected {}, got {}",
                            split_name, i, output_dim, sample.target.len()
                        ),
                        context: None,
                    });
                }
            }
        }

        Ok(())
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> SplitSummary {
        SplitSummary {
            train_size: self.train.len(),
            val_size: self.val.len(),
            test_size: self.test.len(),
            total_size: self.train.len() + self.val.len() + self.test.len(),
            input_shape: self.train.first().map(|s| (s.input.nrows(), s.input.ncols())),
            output_dim: self.train.first().map(|s| s.target.len()),
            train_duration: self.get_duration(&self.train),
            val_duration: self.get_duration(&self.val),
            test_duration: self.get_duration(&self.test),
        }
    }

    fn get_duration(&self, samples: &[WindowedSample]) -> f64 {
        if samples.is_empty() {
            0.0
        } else {
            let first = &samples[0].metadata;
            let last = &samples[samples.len() - 1].metadata;
            last.target_time - first.start_time
        }
    }
}

/// Summary of data splits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitSummary {
    /// Training set size
    pub train_size: usize,
    /// Validation set size
    pub val_size: usize,
    /// Test set size
    pub test_size: usize,
    /// Total size
    pub total_size: usize,
    /// Input shape (features, time_steps)
    pub input_shape: Option<(usize, usize)>,
    /// Output dimension
    pub output_dim: Option<usize>,
    /// Training duration in seconds
    pub train_duration: f64,
    /// Validation duration in seconds
    pub val_duration: f64,
    /// Test duration in seconds
    pub test_duration: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> TimeSeriesData {
        // Create synthetic trajectory data: spiral motion
        let n_samples = 1000;
        let sample_rate = 100.0;
        let mut features = DMatrix::zeros(4, n_samples); // [x, y, vx, vy]

        for i in 0..n_samples {
            let t = i as f64 / sample_rate;
            let radius = 1.0 + 0.1 * t;
            let angle = 2.0 * std::f64::consts::PI * t;

            features[(0, i)] = radius * angle.cos(); // x
            features[(1, i)] = radius * angle.sin(); // y
            features[(2, i)] = -radius * angle.sin() * 2.0 * std::f64::consts::PI; // vx
            features[(3, i)] = radius * angle.cos() * 2.0 * std::f64::consts::PI; // vy
        }

        TimeSeriesData::new(
            features,
            vec!["x".to_string(), "y".to_string(), "vx".to_string(), "vy".to_string()],
            sample_rate,
            "test_spiral".to_string(),
        )
    }

    #[test]
    fn test_data_creation() {
        let data = create_test_data();
        assert_eq!(data.features.nrows(), 4);
        assert_eq!(data.features.ncols(), 1000);
        assert_eq!(data.sample_rate, 100.0);
        assert_eq!(data.feature_names.len(), 4);
    }

    #[test]
    fn test_windowed_samples() {
        let data = create_test_data();
        let samples = data.create_windowed_samples(50, 10, 5).unwrap();

        assert!(!samples.is_empty());

        let first_sample = &samples[0];
        assert_eq!(first_sample.input.shape(), (4, 50));
        assert_eq!(first_sample.target.len(), 2);
        assert!(first_sample.metadata.quality_score > 0.0);
    }

    #[test]
    fn test_temporal_split() {
        let data = create_test_data();
        let splits = data.temporal_split(0.7, 0.15, 0.15).unwrap();

        let (train_size, val_size, test_size) = splits.get_sizes();
        assert!(train_size > 0);
        assert!(val_size > 0);
        assert!(test_size > 0);

        // Check temporal ordering
        let train_start_time = splits.train.first().unwrap().metadata.start_time;
        let val_start_time = splits.val.first().unwrap().metadata.start_time;
        let test_start_time = splits.test.first().unwrap().metadata.start_time;

        assert!(train_start_time <= val_start_time);
        assert!(val_start_time <= test_start_time);

        splits.validate().unwrap();
    }

    #[test]
    fn test_data_statistics() {
        let data = create_test_data();
        let stats = data.get_statistics();

        assert_eq!(stats.num_features, 4);
        assert_eq!(stats.num_samples, 1000);
        assert!(!stats.has_missing_values);
        assert!(stats.data_quality_score > 0.8);
        assert_eq!(stats.feature_stats.len(), 4);
    }

    #[test]
    fn test_split_validation() {
        let data = create_test_data();
        let splits = data.temporal_split(0.8, 0.1, 0.1).unwrap();

        // Should pass validation
        assert!(splits.validate().is_ok());

        let summary = splits.get_summary();
        assert!(summary.total_size > 0);
        assert_eq!(summary.input_shape, Some((4, 256))); // Default window size
        assert_eq!(summary.output_dim, Some(2)); // x, y position
    }
}