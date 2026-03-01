//! Time series processing and vectorization for RuVector

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use ndarray::Array1;
use serde::{Deserialize, Serialize};

use crate::ClimateObservation;

/// A vectorized time series for RuVector storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesVector {
    /// Series identifier
    pub id: String,

    /// Station/source ID
    pub station_id: String,

    /// Start time
    pub start_time: DateTime<Utc>,

    /// End time
    pub end_time: DateTime<Utc>,

    /// Temporal resolution (seconds)
    pub resolution_secs: i64,

    /// Feature vector for similarity search
    pub embedding: Vec<f32>,

    /// Statistical summary
    pub stats: SeriesStats,

    /// Raw values (optional, for debugging)
    pub raw_values: Option<Vec<f64>>,
}

/// Statistical summary of a time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesStats {
    /// Number of observations
    pub count: usize,

    /// Mean value
    pub mean: f64,

    /// Standard deviation
    pub std_dev: f64,

    /// Minimum value
    pub min: f64,

    /// Maximum value
    pub max: f64,

    /// Trend (linear slope)
    pub trend: f64,

    /// Variance ratio (for stationarity check)
    pub variance_ratio: f64,

    /// Autocorrelation at lag 1
    pub autocorr_lag1: f64,
}

/// Seasonal decomposition result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalDecomposition {
    /// Trend component
    pub trend: Vec<f64>,

    /// Seasonal component
    pub seasonal: Vec<f64>,

    /// Residual component
    pub residual: Vec<f64>,

    /// Period detected
    pub period: usize,

    /// Strength of seasonality (0-1)
    pub seasonal_strength: f64,

    /// Strength of trend (0-1)
    pub trend_strength: f64,
}

/// Time series processor
pub struct TimeSeriesProcessor {
    /// Configuration
    config: ProcessorConfig,
}

/// Processor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    /// Target embedding dimension
    pub embedding_dim: usize,

    /// Window size for rolling statistics
    pub window_size: usize,

    /// Enable seasonal decomposition
    pub decompose_seasonal: bool,

    /// Seasonal period (if known)
    pub seasonal_period: Option<usize>,

    /// Normalize embeddings
    pub normalize: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 128,
            window_size: 7,
            decompose_seasonal: true,
            seasonal_period: None,
            normalize: true,
        }
    }
}

impl TimeSeriesProcessor {
    /// Create a new processor
    pub fn new(config: ProcessorConfig) -> Self {
        Self { config }
    }

    /// Process observations into a time series vector
    pub fn process(&self, observations: &[ClimateObservation]) -> Option<TimeSeriesVector> {
        if observations.is_empty() {
            return None;
        }

        // Sort by time
        let mut sorted = observations.to_vec();
        sorted.sort_by_key(|o| o.timestamp);

        // Extract values and times
        let values: Vec<f64> = sorted.iter().map(|o| o.value).collect();
        let times: Vec<DateTime<Utc>> = sorted.iter().map(|o| o.timestamp).collect();

        let start_time = times.first().cloned()?;
        let end_time = times.last().cloned()?;
        let station_id = sorted.first()?.station_id.clone();

        // Compute resolution
        let resolution_secs = if times.len() >= 2 {
            let diffs: Vec<i64> = times
                .windows(2)
                .map(|w| (w[1] - w[0]).num_seconds())
                .collect();
            diffs.iter().sum::<i64>() / diffs.len() as i64
        } else {
            86400 // Default to daily
        };

        // Compute statistics
        let stats = self.compute_stats(&values);

        // Generate embedding
        let embedding = self.generate_embedding(&values, &stats);

        Some(TimeSeriesVector {
            id: format!("{}_{}", station_id, start_time.timestamp()),
            station_id,
            start_time,
            end_time,
            resolution_secs,
            embedding,
            stats,
            raw_values: Some(values),
        })
    }

    /// Compute statistical summary
    fn compute_stats(&self, values: &[f64]) -> SeriesStats {
        let n = values.len();
        if n == 0 {
            return SeriesStats {
                count: 0,
                mean: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
                trend: 0.0,
                variance_ratio: 1.0,
                autocorr_lag1: 0.0,
            };
        }

        let mean = values.iter().sum::<f64>() / n as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
        let std_dev = variance.sqrt();

        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Linear trend
        let trend = self.compute_trend(values);

        // Variance ratio (for stationarity)
        let variance_ratio = if n > 10 {
            let mid = n / 2;
            let var1: f64 =
                values[..mid].iter().map(|v| (v - mean).powi(2)).sum::<f64>() / mid as f64;
            let var2: f64 =
                values[mid..].iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - mid) as f64;
            if var1 > 0.0 {
                var2 / var1
            } else {
                1.0
            }
        } else {
            1.0
        };

        // Autocorrelation at lag 1
        let autocorr_lag1 = self.compute_autocorr(values, 1);

        SeriesStats {
            count: n,
            mean,
            std_dev,
            min,
            max,
            trend,
            variance_ratio,
            autocorr_lag1,
        }
    }

    /// Compute linear trend
    fn compute_trend(&self, values: &[f64]) -> f64 {
        let n = values.len();
        if n < 2 {
            return 0.0;
        }

        let x_mean = (n - 1) as f64 / 2.0;
        let y_mean = values.iter().sum::<f64>() / n as f64;

        let mut num = 0.0;
        let mut denom = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            num += (x - x_mean) * (y - y_mean);
            denom += (x - x_mean).powi(2);
        }

        if denom > 0.0 {
            num / denom
        } else {
            0.0
        }
    }

    /// Compute autocorrelation at given lag
    fn compute_autocorr(&self, values: &[f64], lag: usize) -> f64 {
        let n = values.len();
        if n <= lag {
            return 0.0;
        }

        let mean = values.iter().sum::<f64>() / n as f64;
        let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();

        if variance == 0.0 {
            return 0.0;
        }

        let mut cov = 0.0;
        for i in lag..n {
            cov += (values[i] - mean) * (values[i - lag] - mean);
        }

        cov / variance
    }

    /// Generate embedding vector for similarity search
    fn generate_embedding(&self, values: &[f64], stats: &SeriesStats) -> Vec<f32> {
        let mut embedding = Vec::with_capacity(self.config.embedding_dim);

        // Statistical features (first 16 dimensions)
        embedding.push(stats.mean as f32);
        embedding.push(stats.std_dev as f32);
        embedding.push(stats.min as f32);
        embedding.push(stats.max as f32);
        embedding.push(stats.trend as f32);
        embedding.push(stats.variance_ratio as f32);
        embedding.push(stats.autocorr_lag1 as f32);
        embedding.push((stats.max - stats.min) as f32); // Range

        // Quantile features
        let quantiles = self.compute_quantiles(values, &[0.1, 0.25, 0.5, 0.75, 0.9]);
        for q in quantiles {
            embedding.push(q as f32);
        }

        // Pad to reach target dimension
        while embedding.len() < 16 {
            embedding.push(0.0);
        }

        // Rolling window features (next 32 dimensions)
        if values.len() >= self.config.window_size {
            let rolling_means = self.rolling_mean(values, self.config.window_size);
            let rolling_stds = self.rolling_std(values, self.config.window_size);

            // Sample evenly from rolling stats
            let sample_count = 16;
            for i in 0..sample_count {
                let idx = i * rolling_means.len() / sample_count;
                if idx < rolling_means.len() {
                    embedding.push(rolling_means[idx] as f32);
                    embedding.push(rolling_stds[idx] as f32);
                }
            }
        }

        // Pad to target dimension
        while embedding.len() < self.config.embedding_dim {
            embedding.push(0.0);
        }

        // Truncate if needed
        embedding.truncate(self.config.embedding_dim);

        // Normalize
        if self.config.normalize {
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut embedding {
                    *x /= norm;
                }
            }
        }

        embedding
    }

    /// Compute quantiles
    fn compute_quantiles(&self, values: &[f64], quantiles: &[f64]) -> Vec<f64> {
        if values.is_empty() {
            return quantiles.iter().map(|_| 0.0).collect();
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        quantiles
            .iter()
            .map(|q| {
                let idx = (q * (sorted.len() - 1) as f64).round() as usize;
                sorted[idx.min(sorted.len() - 1)]
            })
            .collect()
    }

    /// Rolling mean
    fn rolling_mean(&self, values: &[f64], window: usize) -> Vec<f64> {
        if values.len() < window {
            return vec![];
        }

        let mut result = Vec::with_capacity(values.len() - window + 1);
        let mut sum: f64 = values[..window].iter().sum();

        result.push(sum / window as f64);

        for i in window..values.len() {
            sum += values[i] - values[i - window];
            result.push(sum / window as f64);
        }

        result
    }

    /// Rolling standard deviation
    fn rolling_std(&self, values: &[f64], window: usize) -> Vec<f64> {
        if values.len() < window {
            return vec![];
        }

        let means = self.rolling_mean(values, window);

        means
            .iter()
            .enumerate()
            .map(|(i, &mean)| {
                let variance: f64 = values[i..i + window]
                    .iter()
                    .map(|v| (v - mean).powi(2))
                    .sum::<f64>()
                    / window as f64;
                variance.sqrt()
            })
            .collect()
    }

    /// Decompose time series into trend, seasonal, and residual components
    pub fn decompose(&self, values: &[f64], period: usize) -> SeasonalDecomposition {
        let n = values.len();

        if n < period * 2 {
            return SeasonalDecomposition {
                trend: values.to_vec(),
                seasonal: vec![0.0; n],
                residual: vec![0.0; n],
                period,
                seasonal_strength: 0.0,
                trend_strength: 0.0,
            };
        }

        // Simple moving average for trend
        let mut trend = vec![0.0; n];
        let half_period = period / 2;

        for i in half_period..(n - half_period) {
            let window: f64 = values[(i - half_period)..(i + half_period + 1)]
                .iter()
                .sum();
            trend[i] = window / period as f64;
        }

        // Fill edges with nearest values
        for i in 0..half_period {
            trend[i] = trend[half_period];
        }
        for i in (n - half_period)..n {
            trend[i] = trend[n - half_period - 1];
        }

        // Detrended series
        let detrended: Vec<f64> = values.iter().zip(&trend).map(|(v, t)| v - t).collect();

        // Compute seasonal pattern
        let mut seasonal = vec![0.0; n];
        for i in 0..period {
            let indices: Vec<usize> = (i..n).step_by(period).collect();
            let seasonal_mean: f64 = indices.iter().map(|&j| detrended[j]).sum::<f64>()
                / indices.len() as f64;

            for &j in &indices {
                seasonal[j] = seasonal_mean;
            }
        }

        // Residual
        let residual: Vec<f64> = values
            .iter()
            .zip(&trend)
            .zip(&seasonal)
            .map(|((v, t), s)| v - t - s)
            .collect();

        // Compute strength measures
        let residual_var: f64 = residual.iter().map(|r| r * r).sum::<f64>() / n as f64;
        let detrended_var: f64 = detrended.iter().map(|d| d * d).sum::<f64>() / n as f64;
        let deseasoned: Vec<f64> = values.iter().zip(&seasonal).map(|(v, s)| v - s).collect();
        let deseasoned_var: f64 = deseasoned.iter().map(|d| d * d).sum::<f64>() / n as f64;

        let seasonal_strength = if detrended_var > 0.0 {
            (1.0 - residual_var / detrended_var).max(0.0)
        } else {
            0.0
        };

        let trend_strength = if deseasoned_var > 0.0 {
            (1.0 - residual_var / deseasoned_var).max(0.0)
        } else {
            0.0
        };

        SeasonalDecomposition {
            trend,
            seasonal,
            residual,
            period,
            seasonal_strength,
            trend_strength,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_creation() {
        let config = ProcessorConfig::default();
        let processor = TimeSeriesProcessor::new(config);
        assert_eq!(processor.config.embedding_dim, 128);
    }

    #[test]
    fn test_compute_stats() {
        let config = ProcessorConfig::default();
        let processor = TimeSeriesProcessor::new(config);

        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = processor.compute_stats(&values);

        assert_eq!(stats.count, 5);
        assert!((stats.mean - 3.0).abs() < 0.001);
        assert!((stats.min - 1.0).abs() < 0.001);
        assert!((stats.max - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_trend_calculation() {
        let config = ProcessorConfig::default();
        let processor = TimeSeriesProcessor::new(config);

        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let trend = processor.compute_trend(&values);

        assert!((trend - 1.0).abs() < 0.001); // Perfect linear trend
    }

    #[test]
    fn test_rolling_mean() {
        let config = ProcessorConfig::default();
        let processor = TimeSeriesProcessor::new(config);

        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let rolling = processor.rolling_mean(&values, 3);

        assert_eq!(rolling.len(), 3);
        assert!((rolling[0] - 2.0).abs() < 0.001);
        assert!((rolling[1] - 3.0).abs() < 0.001);
        assert!((rolling[2] - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_decomposition() {
        let config = ProcessorConfig::default();
        let processor = TimeSeriesProcessor::new(config);

        // Create synthetic data with trend and seasonality
        let n = 100;
        let period = 12;
        let mut values = Vec::with_capacity(n);

        for i in 0..n {
            let trend = 0.1 * i as f64;
            let seasonal = 5.0 * (2.0 * std::f64::consts::PI * i as f64 / period as f64).sin();
            values.push(trend + seasonal);
        }

        let decomp = processor.decompose(&values, period);

        assert_eq!(decomp.trend.len(), n);
        assert_eq!(decomp.seasonal.len(), n);
        assert_eq!(decomp.residual.len(), n);
        assert!(decomp.seasonal_strength > 0.5);
    }
}
