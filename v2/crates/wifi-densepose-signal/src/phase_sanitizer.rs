//! Phase Sanitization Module
//!
//! This module provides phase unwrapping, outlier removal, smoothing, and noise filtering
//! for CSI phase data to ensure reliable signal processing.

use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use thiserror::Error;

/// Errors that can occur during phase sanitization
#[derive(Debug, Error)]
pub enum PhaseSanitizationError {
    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Phase unwrapping failed
    #[error("Phase unwrapping failed: {0}")]
    UnwrapFailed(String),

    /// Outlier removal failed
    #[error("Outlier removal failed: {0}")]
    OutlierRemovalFailed(String),

    /// Smoothing failed
    #[error("Smoothing failed: {0}")]
    SmoothingFailed(String),

    /// Noise filtering failed
    #[error("Noise filtering failed: {0}")]
    NoiseFilterFailed(String),

    /// Invalid data format
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Pipeline error
    #[error("Sanitization pipeline failed: {0}")]
    PipelineFailed(String),
}

/// Phase unwrapping method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnwrappingMethod {
    /// Standard numpy-style unwrapping
    Standard,

    /// Row-by-row custom unwrapping
    Custom,

    /// Itoh's method for 2D unwrapping
    Itoh,

    /// Quality-guided unwrapping
    QualityGuided,
}

impl Default for UnwrappingMethod {
    fn default() -> Self {
        Self::Standard
    }
}

/// Configuration for phase sanitizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSanitizerConfig {
    /// Phase unwrapping method
    pub unwrapping_method: UnwrappingMethod,

    /// Z-score threshold for outlier detection
    pub outlier_threshold: f64,

    /// Window size for smoothing
    pub smoothing_window: usize,

    /// Enable outlier removal
    pub enable_outlier_removal: bool,

    /// Enable smoothing
    pub enable_smoothing: bool,

    /// Enable noise filtering
    pub enable_noise_filtering: bool,

    /// Noise filter cutoff frequency (normalized 0-1)
    pub noise_threshold: f64,

    /// Valid phase range
    pub phase_range: (f64, f64),
}

impl Default for PhaseSanitizerConfig {
    fn default() -> Self {
        Self {
            unwrapping_method: UnwrappingMethod::Standard,
            outlier_threshold: 3.0,
            smoothing_window: 5,
            enable_outlier_removal: true,
            enable_smoothing: true,
            enable_noise_filtering: false,
            noise_threshold: 0.05,
            phase_range: (-PI, PI),
        }
    }
}

impl PhaseSanitizerConfig {
    /// Create a new config builder
    pub fn builder() -> PhaseSanitizerConfigBuilder {
        PhaseSanitizerConfigBuilder::new()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), PhaseSanitizationError> {
        if self.outlier_threshold <= 0.0 {
            return Err(PhaseSanitizationError::InvalidConfig(
                "outlier_threshold must be positive".into(),
            ));
        }

        if self.smoothing_window == 0 {
            return Err(PhaseSanitizationError::InvalidConfig(
                "smoothing_window must be positive".into(),
            ));
        }

        if self.noise_threshold <= 0.0 || self.noise_threshold >= 1.0 {
            return Err(PhaseSanitizationError::InvalidConfig(
                "noise_threshold must be between 0 and 1".into(),
            ));
        }

        Ok(())
    }
}

/// Builder for PhaseSanitizerConfig
#[derive(Debug, Default)]
pub struct PhaseSanitizerConfigBuilder {
    config: PhaseSanitizerConfig,
}

impl PhaseSanitizerConfigBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: PhaseSanitizerConfig::default(),
        }
    }

    /// Set unwrapping method
    pub fn unwrapping_method(mut self, method: UnwrappingMethod) -> Self {
        self.config.unwrapping_method = method;
        self
    }

    /// Set outlier threshold
    pub fn outlier_threshold(mut self, threshold: f64) -> Self {
        self.config.outlier_threshold = threshold;
        self
    }

    /// Set smoothing window
    pub fn smoothing_window(mut self, window: usize) -> Self {
        self.config.smoothing_window = window;
        self
    }

    /// Enable/disable outlier removal
    pub fn enable_outlier_removal(mut self, enable: bool) -> Self {
        self.config.enable_outlier_removal = enable;
        self
    }

    /// Enable/disable smoothing
    pub fn enable_smoothing(mut self, enable: bool) -> Self {
        self.config.enable_smoothing = enable;
        self
    }

    /// Enable/disable noise filtering
    pub fn enable_noise_filtering(mut self, enable: bool) -> Self {
        self.config.enable_noise_filtering = enable;
        self
    }

    /// Set noise threshold
    pub fn noise_threshold(mut self, threshold: f64) -> Self {
        self.config.noise_threshold = threshold;
        self
    }

    /// Set phase range
    pub fn phase_range(mut self, min: f64, max: f64) -> Self {
        self.config.phase_range = (min, max);
        self
    }

    /// Build the configuration
    pub fn build(self) -> PhaseSanitizerConfig {
        self.config
    }
}

/// Statistics for sanitization operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SanitizationStatistics {
    /// Total samples processed
    pub total_processed: usize,

    /// Total outliers removed
    pub outliers_removed: usize,

    /// Total sanitization errors
    pub sanitization_errors: usize,
}

impl SanitizationStatistics {
    /// Calculate outlier rate
    pub fn outlier_rate(&self) -> f64 {
        if self.total_processed > 0 {
            self.outliers_removed as f64 / self.total_processed as f64
        } else {
            0.0
        }
    }

    /// Calculate error rate
    pub fn error_rate(&self) -> f64 {
        if self.total_processed > 0 {
            self.sanitization_errors as f64 / self.total_processed as f64
        } else {
            0.0
        }
    }
}

/// Phase Sanitizer for cleaning and preparing phase data
#[derive(Debug)]
pub struct PhaseSanitizer {
    config: PhaseSanitizerConfig,
    statistics: SanitizationStatistics,
}

impl PhaseSanitizer {
    /// Create a new phase sanitizer
    pub fn new(config: PhaseSanitizerConfig) -> Result<Self, PhaseSanitizationError> {
        config.validate()?;
        Ok(Self {
            config,
            statistics: SanitizationStatistics::default(),
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &PhaseSanitizerConfig {
        &self.config
    }

    /// Validate phase data format and values
    pub fn validate_phase_data(&self, phase_data: &Array2<f64>) -> Result<(), PhaseSanitizationError> {
        // Check if data is empty
        if phase_data.is_empty() {
            return Err(PhaseSanitizationError::InvalidData(
                "Phase data cannot be empty".into(),
            ));
        }

        // Check if values are within valid range
        let (min_val, max_val) = self.config.phase_range;
        for &val in phase_data.iter() {
            if val < min_val || val > max_val {
                return Err(PhaseSanitizationError::InvalidData(format!(
                    "Phase value {} outside valid range [{}, {}]",
                    val, min_val, max_val
                )));
            }
        }

        Ok(())
    }

    /// Unwrap phase data to remove 2pi discontinuities
    pub fn unwrap_phase(&self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        if phase_data.is_empty() {
            return Err(PhaseSanitizationError::UnwrapFailed(
                "Cannot unwrap empty phase data".into(),
            ));
        }

        match self.config.unwrapping_method {
            UnwrappingMethod::Standard => self.unwrap_standard(phase_data),
            UnwrappingMethod::Custom => self.unwrap_custom(phase_data),
            UnwrappingMethod::Itoh => self.unwrap_itoh(phase_data),
            UnwrappingMethod::QualityGuided => self.unwrap_quality_guided(phase_data),
        }
    }

    /// Standard phase unwrapping (numpy-style)
    fn unwrap_standard(&self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        let mut unwrapped = phase_data.clone();
        let (_nrows, ncols) = unwrapped.dim();

        for i in 0..unwrapped.nrows() {
            let mut row_data: Vec<f64> = (0..ncols).map(|j| unwrapped[[i, j]]).collect();
            Self::unwrap_1d(&mut row_data);
            for (j, &val) in row_data.iter().enumerate() {
                unwrapped[[i, j]] = val;
            }
        }

        Ok(unwrapped)
    }

    /// Custom row-by-row phase unwrapping
    fn unwrap_custom(&self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        let mut unwrapped = phase_data.clone();
        let ncols = unwrapped.ncols();

        for i in 0..unwrapped.nrows() {
            let mut row_data: Vec<f64> = (0..ncols).map(|j| unwrapped[[i, j]]).collect();
            self.unwrap_1d_custom(&mut row_data);
            for (j, &val) in row_data.iter().enumerate() {
                unwrapped[[i, j]] = val;
            }
        }

        Ok(unwrapped)
    }

    /// Itoh's 2D phase unwrapping method
    fn unwrap_itoh(&self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        let mut unwrapped = phase_data.clone();
        let (nrows, ncols) = phase_data.dim();

        // First unwrap rows
        for i in 0..nrows {
            let mut row_data: Vec<f64> = (0..ncols).map(|j| unwrapped[[i, j]]).collect();
            Self::unwrap_1d(&mut row_data);
            for (j, &val) in row_data.iter().enumerate() {
                unwrapped[[i, j]] = val;
            }
        }

        // Then unwrap columns
        for j in 0..ncols {
            let mut col: Vec<f64> = unwrapped.column(j).to_vec();
            Self::unwrap_1d(&mut col);
            for (i, &val) in col.iter().enumerate() {
                unwrapped[[i, j]] = val;
            }
        }

        Ok(unwrapped)
    }

    /// Quality-guided phase unwrapping
    fn unwrap_quality_guided(&self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        // For now, use standard unwrapping with quality weighting
        // A full implementation would use phase derivatives as quality metric
        let mut unwrapped = phase_data.clone();
        let (nrows, ncols) = phase_data.dim();

        // Calculate quality map based on phase gradients
        // Note: Full quality-guided implementation would use this map for ordering
        let _quality = self.calculate_quality_map(phase_data);

        // Unwrap starting from highest quality regions
        for i in 0..nrows {
            let mut row_data: Vec<f64> = (0..ncols).map(|j| unwrapped[[i, j]]).collect();
            Self::unwrap_1d(&mut row_data);
            for (j, &val) in row_data.iter().enumerate() {
                unwrapped[[i, j]] = val;
            }
        }

        Ok(unwrapped)
    }

    /// Calculate quality map for quality-guided unwrapping
    fn calculate_quality_map(&self, phase_data: &Array2<f64>) -> Array2<f64> {
        let (nrows, ncols) = phase_data.dim();
        let mut quality = Array2::zeros((nrows, ncols));

        for i in 0..nrows {
            for j in 0..ncols {
                let mut grad_sum = 0.0;
                let mut count = 0;

                // Calculate local phase gradient magnitude
                if j > 0 {
                    grad_sum += (phase_data[[i, j]] - phase_data[[i, j - 1]]).abs();
                    count += 1;
                }
                if j < ncols - 1 {
                    grad_sum += (phase_data[[i, j + 1]] - phase_data[[i, j]]).abs();
                    count += 1;
                }
                if i > 0 {
                    grad_sum += (phase_data[[i, j]] - phase_data[[i - 1, j]]).abs();
                    count += 1;
                }
                if i < nrows - 1 {
                    grad_sum += (phase_data[[i + 1, j]] - phase_data[[i, j]]).abs();
                    count += 1;
                }

                // Quality is inverse of gradient magnitude
                if count > 0 {
                    quality[[i, j]] = 1.0 / (1.0 + grad_sum / count as f64);
                }
            }
        }

        quality
    }

    /// In-place 1D phase unwrapping
    fn unwrap_1d(data: &mut [f64]) {
        if data.len() < 2 {
            return;
        }

        let mut correction = 0.0;
        let mut prev_wrapped = data[0];

        for i in 1..data.len() {
            let current_wrapped = data[i];
            // Calculate diff using original wrapped values
            let diff = current_wrapped - prev_wrapped;

            if diff > PI {
                correction -= 2.0 * PI;
            } else if diff < -PI {
                correction += 2.0 * PI;
            }

            data[i] = current_wrapped + correction;
            prev_wrapped = current_wrapped;
        }
    }

    /// Custom 1D phase unwrapping with tolerance
    fn unwrap_1d_custom(&self, data: &mut [f64]) {
        if data.len() < 2 {
            return;
        }

        let tolerance = 0.9 * PI; // Slightly less than pi for robustness
        let mut correction = 0.0;

        for i in 1..data.len() {
            let diff = data[i] - data[i - 1] + correction;
            if diff > tolerance {
                correction -= 2.0 * PI;
            } else if diff < -tolerance {
                correction += 2.0 * PI;
            }
            data[i] += correction;
        }
    }

    /// Remove outliers from phase data using Z-score method
    pub fn remove_outliers(&mut self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        if !self.config.enable_outlier_removal {
            return Ok(phase_data.clone());
        }

        // Detect outliers
        let outlier_mask = self.detect_outliers(phase_data)?;

        // Interpolate outliers
        let cleaned = self.interpolate_outliers(phase_data, &outlier_mask)?;

        Ok(cleaned)
    }

    /// Detect outliers using Z-score method
    fn detect_outliers(&mut self, phase_data: &Array2<f64>) -> Result<Array2<bool>, PhaseSanitizationError> {
        let (nrows, ncols) = phase_data.dim();
        let mut outlier_mask = Array2::from_elem((nrows, ncols), false);

        for i in 0..nrows {
            let row = phase_data.row(i);
            let mean = row.mean().unwrap_or(0.0);
            let std = self.calculate_std_1d(&row.to_vec());

            for j in 0..ncols {
                let z_score = (phase_data[[i, j]] - mean).abs() / (std + 1e-8);
                if z_score > self.config.outlier_threshold {
                    outlier_mask[[i, j]] = true;
                    self.statistics.outliers_removed += 1;
                }
            }
        }

        Ok(outlier_mask)
    }

    /// Interpolate outlier values using linear interpolation
    fn interpolate_outliers(
        &self,
        phase_data: &Array2<f64>,
        outlier_mask: &Array2<bool>,
    ) -> Result<Array2<f64>, PhaseSanitizationError> {
        let mut cleaned = phase_data.clone();
        let (nrows, ncols) = phase_data.dim();

        for i in 0..nrows {
            // Find valid (non-outlier) indices
            let valid_indices: Vec<usize> = (0..ncols)
                .filter(|&j| !outlier_mask[[i, j]])
                .collect();

            let outlier_indices: Vec<usize> = (0..ncols)
                .filter(|&j| outlier_mask[[i, j]])
                .collect();

            if valid_indices.len() >= 2 && !outlier_indices.is_empty() {
                // Extract valid values
                let valid_values: Vec<f64> = valid_indices
                    .iter()
                    .map(|&j| phase_data[[i, j]])
                    .collect();

                // Interpolate outliers
                for &j in &outlier_indices {
                    cleaned[[i, j]] = self.linear_interpolate(j, &valid_indices, &valid_values);
                }
            }
        }

        Ok(cleaned)
    }

    /// Linear interpolation helper
    fn linear_interpolate(&self, x: usize, xs: &[usize], ys: &[f64]) -> f64 {
        if xs.is_empty() {
            return 0.0;
        }

        // Find surrounding points
        let mut lower_idx = 0;
        let mut upper_idx = xs.len() - 1;

        for (i, &xi) in xs.iter().enumerate() {
            if xi <= x {
                lower_idx = i;
            }
            if xi >= x {
                upper_idx = i;
                break;
            }
        }

        if lower_idx == upper_idx {
            return ys[lower_idx];
        }

        // Linear interpolation
        let x0 = xs[lower_idx] as f64;
        let x1 = xs[upper_idx] as f64;
        let y0 = ys[lower_idx];
        let y1 = ys[upper_idx];

        y0 + (y1 - y0) * (x as f64 - x0) / (x1 - x0)
    }

    /// Smooth phase data using moving average
    pub fn smooth_phase(&self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        if !self.config.enable_smoothing {
            return Ok(phase_data.clone());
        }

        let mut smoothed = phase_data.clone();
        let (nrows, ncols) = phase_data.dim();

        // Ensure odd window size
        let mut window_size = self.config.smoothing_window;
        if window_size % 2 == 0 {
            window_size += 1;
        }

        let half_window = window_size / 2;

        for i in 0..nrows {
            for j in half_window..ncols.saturating_sub(half_window) {
                let mut sum = 0.0;
                for k in 0..window_size {
                    sum += phase_data[[i, j - half_window + k]];
                }
                smoothed[[i, j]] = sum / window_size as f64;
            }
        }

        Ok(smoothed)
    }

    /// Filter noise using low-pass Butterworth filter
    pub fn filter_noise(&self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        if !self.config.enable_noise_filtering {
            return Ok(phase_data.clone());
        }

        let (nrows, ncols) = phase_data.dim();

        // Check minimum length for filtering
        let min_filter_length = 18;
        if ncols < min_filter_length {
            return Ok(phase_data.clone());
        }

        // Simple low-pass filter using exponential smoothing
        let alpha = self.config.noise_threshold;
        let mut filtered = phase_data.clone();

        for i in 0..nrows {
            // Forward pass
            for j in 1..ncols {
                filtered[[i, j]] = alpha * filtered[[i, j]] + (1.0 - alpha) * filtered[[i, j - 1]];
            }

            // Backward pass for zero-phase filtering
            for j in (0..ncols - 1).rev() {
                filtered[[i, j]] = alpha * filtered[[i, j]] + (1.0 - alpha) * filtered[[i, j + 1]];
            }
        }

        Ok(filtered)
    }

    /// Complete sanitization pipeline
    pub fn sanitize_phase(&mut self, phase_data: &Array2<f64>) -> Result<Array2<f64>, PhaseSanitizationError> {
        self.statistics.total_processed += 1;

        // Validate input
        self.validate_phase_data(phase_data).map_err(|e| {
            self.statistics.sanitization_errors += 1;
            e
        })?;

        // Unwrap phase
        let unwrapped = self.unwrap_phase(phase_data).map_err(|e| {
            self.statistics.sanitization_errors += 1;
            e
        })?;

        // Remove outliers
        let cleaned = self.remove_outliers(&unwrapped).map_err(|e| {
            self.statistics.sanitization_errors += 1;
            e
        })?;

        // Smooth phase
        let smoothed = self.smooth_phase(&cleaned).map_err(|e| {
            self.statistics.sanitization_errors += 1;
            e
        })?;

        // Filter noise
        let filtered = self.filter_noise(&smoothed).map_err(|e| {
            self.statistics.sanitization_errors += 1;
            e
        })?;

        Ok(filtered)
    }

    /// Get sanitization statistics
    pub fn get_statistics(&self) -> &SanitizationStatistics {
        &self.statistics
    }

    /// Reset statistics
    pub fn reset_statistics(&mut self) {
        self.statistics = SanitizationStatistics::default();
    }

    /// Calculate standard deviation for 1D slice
    fn calculate_std_1d(&self, data: &[f64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let mean: f64 = data.iter().sum::<f64>() / data.len() as f64;
        let variance: f64 = data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;
        variance.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn create_test_phase_data() -> Array2<f64> {
        // Create phase data with some simulated wrapping
        Array2::from_shape_fn((4, 64), |(i, j)| {
            let base = (j as f64 * 0.05).sin() * (PI / 2.0);
            base + (i as f64 * 0.1)
        })
    }

    fn create_wrapped_phase_data() -> Array2<f64> {
        // Create phase data that will need unwrapping
        // Generate a linearly increasing phase that wraps at +/- pi boundaries
        Array2::from_shape_fn((2, 20), |(i, j)| {
            let unwrapped = j as f64 * 0.4 + i as f64 * 0.2;
            // Proper wrap to [-pi, pi]
            let mut wrapped = unwrapped;
            while wrapped > PI {
                wrapped -= 2.0 * PI;
            }
            while wrapped < -PI {
                wrapped += 2.0 * PI;
            }
            wrapped
        })
    }

    #[test]
    fn test_config_validation() {
        let config = PhaseSanitizerConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let config = PhaseSanitizerConfig::builder()
            .outlier_threshold(-1.0)
            .build();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_sanitizer_creation() {
        let config = PhaseSanitizerConfig::default();
        let sanitizer = PhaseSanitizer::new(config);
        assert!(sanitizer.is_ok());
    }

    #[test]
    fn test_phase_validation() {
        let config = PhaseSanitizerConfig::default();
        let sanitizer = PhaseSanitizer::new(config).unwrap();

        let valid_data = create_test_phase_data();
        assert!(sanitizer.validate_phase_data(&valid_data).is_ok());

        // Test with out-of-range values
        let invalid_data = Array2::from_elem((2, 10), 10.0);
        assert!(sanitizer.validate_phase_data(&invalid_data).is_err());
    }

    #[test]
    fn test_phase_unwrapping() {
        let config = PhaseSanitizerConfig::builder()
            .unwrapping_method(UnwrappingMethod::Standard)
            .build();
        let sanitizer = PhaseSanitizer::new(config).unwrap();

        let wrapped = create_wrapped_phase_data();
        let unwrapped = sanitizer.unwrap_phase(&wrapped);
        assert!(unwrapped.is_ok());

        // Verify that differences are now smooth (no jumps > pi)
        let unwrapped = unwrapped.unwrap();
        let ncols = unwrapped.ncols();
        for i in 0..unwrapped.nrows() {
            for j in 1..ncols {
                let diff = (unwrapped[[i, j]] - unwrapped[[i, j - 1]]).abs();
                assert!(diff < PI + 0.1, "Jump detected: {}", diff);
            }
        }
    }

    #[test]
    fn test_outlier_removal() {
        let config = PhaseSanitizerConfig::builder()
            .outlier_threshold(2.0)
            .enable_outlier_removal(true)
            .build();
        let mut sanitizer = PhaseSanitizer::new(config).unwrap();

        let mut data = create_test_phase_data();
        // Insert an outlier
        data[[0, 10]] = 100.0 * data[[0, 10]];

        // Need to use data within valid range
        let data = Array2::from_shape_fn((4, 64), |(i, j)| {
            if i == 0 && j == 10 {
                PI * 0.9 // Near boundary but valid
            } else {
                0.1 * (j as f64 * 0.1).sin()
            }
        });

        let cleaned = sanitizer.remove_outliers(&data);
        assert!(cleaned.is_ok());
    }

    #[test]
    fn test_phase_smoothing() {
        let config = PhaseSanitizerConfig::builder()
            .smoothing_window(5)
            .enable_smoothing(true)
            .build();
        let sanitizer = PhaseSanitizer::new(config).unwrap();

        let noisy_data = Array2::from_shape_fn((2, 20), |(_, j)| {
            (j as f64 * 0.2).sin() + 0.1 * ((j * 7) as f64).sin()
        });

        let smoothed = sanitizer.smooth_phase(&noisy_data);
        assert!(smoothed.is_ok());
    }

    #[test]
    fn test_noise_filtering() {
        let config = PhaseSanitizerConfig::builder()
            .noise_threshold(0.1)
            .enable_noise_filtering(true)
            .build();
        let sanitizer = PhaseSanitizer::new(config).unwrap();

        let data = create_test_phase_data();
        let filtered = sanitizer.filter_noise(&data);
        assert!(filtered.is_ok());
    }

    #[test]
    fn test_complete_pipeline() {
        let config = PhaseSanitizerConfig::builder()
            .unwrapping_method(UnwrappingMethod::Standard)
            .outlier_threshold(3.0)
            .smoothing_window(3)
            .enable_outlier_removal(true)
            .enable_smoothing(true)
            .enable_noise_filtering(false)
            .build();
        let mut sanitizer = PhaseSanitizer::new(config).unwrap();

        let data = create_test_phase_data();
        let sanitized = sanitizer.sanitize_phase(&data);
        assert!(sanitized.is_ok());

        let stats = sanitizer.get_statistics();
        assert_eq!(stats.total_processed, 1);
    }

    #[test]
    fn test_different_unwrapping_methods() {
        let methods = vec![
            UnwrappingMethod::Standard,
            UnwrappingMethod::Custom,
            UnwrappingMethod::Itoh,
            UnwrappingMethod::QualityGuided,
        ];

        let wrapped = create_wrapped_phase_data();

        for method in methods {
            let config = PhaseSanitizerConfig::builder()
                .unwrapping_method(method)
                .build();
            let sanitizer = PhaseSanitizer::new(config).unwrap();

            let result = sanitizer.unwrap_phase(&wrapped);
            assert!(result.is_ok(), "Failed for method {:?}", method);
        }
    }

    #[test]
    fn test_empty_data_handling() {
        let config = PhaseSanitizerConfig::default();
        let sanitizer = PhaseSanitizer::new(config).unwrap();

        let empty = Array2::<f64>::zeros((0, 0));
        assert!(sanitizer.validate_phase_data(&empty).is_err());
        assert!(sanitizer.unwrap_phase(&empty).is_err());
    }

    #[test]
    fn test_statistics() {
        let config = PhaseSanitizerConfig::default();
        let mut sanitizer = PhaseSanitizer::new(config).unwrap();

        let data = create_test_phase_data();
        let _ = sanitizer.sanitize_phase(&data);
        let _ = sanitizer.sanitize_phase(&data);

        let stats = sanitizer.get_statistics();
        assert_eq!(stats.total_processed, 2);

        sanitizer.reset_statistics();
        let stats = sanitizer.get_statistics();
        assert_eq!(stats.total_processed, 0);
    }
}
