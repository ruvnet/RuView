//! Calibration data for quantization

use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Calibration data for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationData {
    /// Layer-wise activation statistics
    pub layers: Vec<LayerCalibration>,
    /// Global input statistics
    pub input_stats: ActivationStats,
    /// Number of calibration samples used
    pub num_samples: usize,
    /// Calibration method used
    pub method: CalibrationMethod,
}

/// Per-layer calibration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerCalibration {
    /// Layer index
    pub layer_idx: usize,
    /// Layer name
    pub name: String,
    /// Activation statistics after this layer
    pub activation_stats: ActivationStats,
    /// Weight statistics for this layer
    pub weight_stats: WeightStats,
    /// Optimal scale for activations (Q16.16)
    pub act_scale: i32,
    /// Optimal scale for weights (Q16.16)
    pub weight_scale: i32,
}

/// Activation statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivationStats {
    /// Minimum value seen
    pub min: f32,
    /// Maximum value seen
    pub max: f32,
    /// Mean value
    pub mean: f32,
    /// Standard deviation
    pub std: f32,
    /// Histogram bins (for entropy calibration)
    #[serde(default)]
    pub histogram: Vec<u32>,
    /// Histogram bin edges
    #[serde(default)]
    pub bin_edges: Vec<f32>,
}

impl ActivationStats {
    /// Create empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Update stats with a batch of values
    pub fn update(&mut self, values: &[f32]) {
        if values.is_empty() {
            return;
        }

        // Update min/max
        for &v in values {
            if v < self.min || self.min == 0.0 {
                self.min = v;
            }
            if v > self.max {
                self.max = v;
            }
        }

        // Update running mean and std
        let n = values.len() as f32;
        let batch_mean = values.iter().sum::<f32>() / n;
        let batch_var = values.iter().map(|v| (v - batch_mean).powi(2)).sum::<f32>() / n;

        // Simple update (not online algorithm)
        self.mean = batch_mean;
        self.std = batch_var.sqrt();
    }

    /// Compute optimal scale for symmetric quantization to n bits
    pub fn optimal_scale(&self, bits: u8) -> f32 {
        let max_range = self.max.abs().max(self.min.abs());
        let qmax = (1 << (bits - 1)) as f32 - 1.0;
        max_range / qmax
    }
}

/// Weight statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeightStats {
    /// Min weight value
    pub min: f32,
    /// Max weight value
    pub max: f32,
    /// Sparsity (fraction of zeros)
    pub sparsity: f32,
}

impl WeightStats {
    /// Compute from weight tensor
    pub fn from_weights(weights: &[f32]) -> Self {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut zeros = 0usize;

        for &w in weights {
            if w < min {
                min = w;
            }
            if w > max {
                max = w;
            }
            if w.abs() < 1e-6 {
                zeros += 1;
            }
        }

        Self {
            min,
            max,
            sparsity: zeros as f32 / weights.len() as f32,
        }
    }
}

/// Calibration method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalibrationMethod {
    /// Use min/max of observed values
    MinMax,
    /// Use percentile clipping (e.g., 99.9%)
    Percentile(u32), // 999 = 99.9%
    /// Entropy-based calibration (KL divergence)
    Entropy,
    /// Mean-squared error minimization
    Mse,
}

impl Default for CalibrationMethod {
    fn default() -> Self {
        Self::MinMax
    }
}

impl CalibrationData {
    /// Create empty calibration data
    pub fn new(method: CalibrationMethod) -> Self {
        Self {
            layers: Vec::new(),
            input_stats: ActivationStats::new(),
            num_samples: 0,
            method,
        }
    }

    /// Add layer calibration
    pub fn add_layer(&mut self, calib: LayerCalibration) {
        self.layers.push(calib);
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

/// Calibrate a model by collecting activation statistics
pub fn calibrate_model<F>(
    run_inference: F,
    calibration_inputs: &[Vec<u16>],
    num_layers: usize,
    method: CalibrationMethod,
) -> Result<CalibrationData>
where
    F: Fn(&[u16]) -> Result<Vec<Vec<f32>>>, // Returns activations per layer
{
    let mut calibration = CalibrationData::new(method);

    // Initialize layer stats
    let mut layer_stats: Vec<ActivationStats> =
        (0..num_layers).map(|_| ActivationStats::new()).collect();

    // Run calibration passes
    for input in calibration_inputs {
        // Run inference and collect activations
        let activations = run_inference(input)?;

        // Update statistics
        for (layer_idx, layer_act) in activations.iter().enumerate() {
            if layer_idx < num_layers {
                layer_stats[layer_idx].update(layer_act);
            }
        }

        calibration.num_samples += 1;
    }

    // Create layer calibrations
    for (layer_idx, stats) in layer_stats.into_iter().enumerate() {
        let act_scale = match method {
            CalibrationMethod::MinMax => stats.optimal_scale(8),
            CalibrationMethod::Percentile(_) => stats.optimal_scale(8) * 0.99,
            CalibrationMethod::Entropy => stats.optimal_scale(8),
            CalibrationMethod::Mse => stats.optimal_scale(8),
        };

        calibration.add_layer(LayerCalibration {
            layer_idx,
            name: format!("layer_{}", layer_idx),
            activation_stats: stats,
            weight_stats: WeightStats::default(),
            act_scale: (act_scale * 65536.0) as i32,
            weight_scale: 65536, // Default 1.0
        });
    }

    Ok(calibration)
}

/// Apply percentile clipping to calibration
pub fn apply_percentile(stats: &ActivationStats, percentile: f32) -> (f32, f32) {
    if stats.histogram.is_empty() || stats.bin_edges.len() < 2 {
        return (stats.min, stats.max);
    }

    let total: u32 = stats.histogram.iter().sum();
    let target_low = (total as f32 * (1.0 - percentile) / 2.0) as u32;
    let target_high = (total as f32 * (1.0 + percentile) / 2.0) as u32;

    let mut cumsum = 0u32;
    let mut low_idx = 0;
    let mut high_idx = stats.histogram.len() - 1;

    for (i, &count) in stats.histogram.iter().enumerate() {
        cumsum += count;
        if cumsum >= target_low && low_idx == 0 {
            low_idx = i;
        }
        if cumsum >= target_high {
            high_idx = i;
            break;
        }
    }

    (stats.bin_edges[low_idx], stats.bin_edges[high_idx + 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activation_stats_update() {
        let mut stats = ActivationStats::new();
        stats.update(&[1.0, 2.0, 3.0, 4.0, 5.0]);

        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert!((stats.mean - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_optimal_scale() {
        let mut stats = ActivationStats::new();
        stats.min = -1.0;
        stats.max = 1.0;

        let scale = stats.optimal_scale(8);
        // For 8-bit, qmax = 127, so scale should be 1.0/127 ≈ 0.00787
        assert!((scale - 1.0 / 127.0).abs() < 0.001);
    }

    #[test]
    fn test_weight_stats() {
        let weights = vec![0.0, 0.1, -0.1, 0.5, -0.5, 0.0];
        let stats = WeightStats::from_weights(&weights);

        assert_eq!(stats.min, -0.5);
        assert_eq!(stats.max, 0.5);
        assert!((stats.sparsity - 2.0 / 6.0).abs() < 0.01);
    }

    #[test]
    fn test_calibration_serialization() {
        let calib = CalibrationData::new(CalibrationMethod::MinMax);
        let bytes = calib.to_bytes().unwrap();
        let restored = CalibrationData::from_bytes(&bytes).unwrap();

        assert_eq!(calib.method, restored.method);
    }
}
