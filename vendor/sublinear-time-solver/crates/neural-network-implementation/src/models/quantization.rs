//! INT8 quantization implementation for neural network optimization

use crate::error::{Result, TemporalNeuralError};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

/// Quantization scheme for model optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationScheme {
    /// 8-bit integer quantization
    Int8,
    /// 4-bit integer quantization
    Int4,
    /// Binary quantization
    Binary,
}

/// Quantized model wrapper
#[derive(Debug, Clone)]
pub struct QuantizedModel {
    /// Quantization scheme used
    pub scheme: QuantizationScheme,
    /// Quantized weights
    pub quantized_weights: Vec<i8>,
    /// Scale factors for dequantization
    pub scales: Vec<f32>,
    /// Zero points for symmetric quantization
    pub zero_points: Vec<i8>,
    /// Original model shape information
    pub shape_info: ModelShapeInfo,
}

/// Information about model structure for quantization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelShapeInfo {
    /// Layer dimensions
    pub layer_shapes: Vec<(usize, usize)>,
    /// Total parameter count
    pub total_params: usize,
    /// Memory savings achieved
    pub memory_savings_ratio: f64,
}

impl QuantizedModel {
    /// Create quantized model from floating point weights
    pub fn quantize_int8(weights: &[f64]) -> Result<Self> {
        if weights.is_empty() {
            return Err(TemporalNeuralError::QuantizationError {
                message: "Cannot quantize empty weight vector".to_string(),
                scheme: Some("int8".to_string()),
                accuracy_loss: None,
            });
        }

        let mut quantized_weights = Vec::with_capacity(weights.len());
        let mut scales = Vec::new();
        let mut zero_points = Vec::new();

        // For simplicity, quantize the entire weight vector with single scale/zero_point
        // In practice, you'd quantize per-layer or per-channel
        let min_val = weights.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_val = weights.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        let scale = (max_val - min_val) / 255.0; // 8-bit range
        let zero_point = (-min_val / scale).round() as i8;

        scales.push(scale as f32);
        zero_points.push(zero_point);

        for &weight in weights {
            let quantized = ((weight / scale) + zero_point as f64).round().clamp(-128.0, 127.0) as i8;
            quantized_weights.push(quantized);
        }

        let shape_info = ModelShapeInfo {
            layer_shapes: vec![(weights.len(), 1)], // Simplified
            total_params: weights.len(),
            memory_savings_ratio: 4.0, // 32-bit -> 8-bit
        };

        Ok(Self {
            scheme: QuantizationScheme::Int8,
            quantized_weights,
            scales,
            zero_points,
            shape_info,
        })
    }

    /// Dequantize weights back to floating point
    pub fn dequantize(&self) -> Result<Vec<f64>> {
        if self.quantized_weights.len() != self.shape_info.total_params {
            return Err(TemporalNeuralError::QuantizationError {
                message: "Weight count mismatch during dequantization".to_string(),
                scheme: Some(format!("{:?}", self.scheme)),
                accuracy_loss: None,
            });
        }

        let mut dequantized = Vec::with_capacity(self.quantized_weights.len());
        let scale = self.scales[0] as f64; // Using first scale for simplicity
        let zero_point = self.zero_points[0] as f64;

        for &quantized_weight in &self.quantized_weights {
            let dequantized_weight = (quantized_weight as f64 - zero_point) * scale;
            dequantized.push(dequantized_weight);
        }

        Ok(dequantized)
    }

    /// Get memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.quantized_weights.len() * std::mem::size_of::<i8>() +
        self.scales.len() * std::mem::size_of::<f32>() +
        self.zero_points.len() * std::mem::size_of::<i8>()
    }

    /// Estimate accuracy loss from quantization
    pub fn estimate_accuracy_loss(&self, original_weights: &[f64]) -> Result<f64> {
        let dequantized = self.dequantize()?;

        if dequantized.len() != original_weights.len() {
            return Err(TemporalNeuralError::QuantizationError {
                message: "Length mismatch for accuracy estimation".to_string(),
                scheme: Some(format!("{:?}", self.scheme)),
                accuracy_loss: None,
            });
        }

        // Compute MSE between original and dequantized weights
        let mse = original_weights.iter()
            .zip(dequantized.iter())
            .map(|(&orig, &deq)| (orig - deq).powi(2))
            .sum::<f64>() / original_weights.len() as f64;

        Ok(mse.sqrt()) // Return RMSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int8_quantization() {
        let weights = vec![1.0, 2.5, -1.5, 0.0, 3.2, -2.1];
        let quantized = QuantizedModel::quantize_int8(&weights).unwrap();

        assert_eq!(quantized.quantized_weights.len(), weights.len());
        assert_eq!(quantized.scales.len(), 1);
        assert_eq!(quantized.zero_points.len(), 1);
    }

    #[test]
    fn test_dequantization() {
        let weights = vec![1.0, 2.5, -1.5, 0.0, 3.2, -2.1];
        let quantized = QuantizedModel::quantize_int8(&weights).unwrap();
        let dequantized = quantized.dequantize().unwrap();

        assert_eq!(dequantized.len(), weights.len());

        // Check that dequantized values are reasonably close to originals
        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.1); // Should be within quantization error
        }
    }

    #[test]
    fn test_accuracy_loss_estimation() {
        let weights = vec![1.0, 2.5, -1.5, 0.0, 3.2, -2.1];
        let quantized = QuantizedModel::quantize_int8(&weights).unwrap();
        let accuracy_loss = quantized.estimate_accuracy_loss(&weights).unwrap();

        assert!(accuracy_loss >= 0.0);
        assert!(accuracy_loss < 1.0); // Should be reasonable for this simple case
    }

    #[test]
    fn test_memory_usage() {
        let weights = vec![1.0; 1000];
        let quantized = QuantizedModel::quantize_int8(&weights).unwrap();

        let memory_usage = quantized.memory_usage();
        assert!(memory_usage > 0);

        // Should use roughly 1/4 the memory (32-bit -> 8-bit)
        let original_memory = weights.len() * std::mem::size_of::<f64>();
        assert!(memory_usage < original_memory / 2);
    }
}