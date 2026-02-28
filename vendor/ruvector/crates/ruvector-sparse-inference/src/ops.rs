//! Basic neural network operations

use std::f32;

/// Linear layer (fully connected)
#[derive(Debug, Clone)]
pub struct Linear {
    pub weight: Vec<Vec<f32>>, // [out_features, in_features]
    pub bias: Option<Vec<f32>>,
    pub in_features: usize,
    pub out_features: usize,
}

impl Linear {
    pub fn new(in_features: usize, out_features: usize, use_bias: bool) -> Self {
        Self {
            weight: vec![vec![0.0; in_features]; out_features],
            bias: if use_bias {
                Some(vec![0.0; out_features])
            } else {
                None
            },
            in_features,
            out_features,
        }
    }

    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mut output = vec![0.0; self.out_features];

        for i in 0..self.out_features {
            let mut sum = 0.0;
            for j in 0..self.in_features.min(input.len()) {
                sum += self.weight[i][j] * input[j];
            }
            if let Some(ref bias) = self.bias {
                sum += bias[i];
            }
            output[i] = sum;
        }

        output
    }
}

/// Embedding layer
#[derive(Debug, Clone)]
pub struct Embedding {
    pub weight: Vec<Vec<f32>>, // [vocab_size, embedding_dim]
    pub vocab_size: usize,
    pub embedding_dim: usize,
}

impl Embedding {
    pub fn new(vocab_size: usize, embedding_dim: usize) -> Self {
        Self {
            weight: vec![vec![0.0; embedding_dim]; vocab_size],
            vocab_size,
            embedding_dim,
        }
    }

    pub fn forward(&self, input_ids: &[u64]) -> Vec<f32> {
        let mut output = Vec::new();

        for &id in input_ids {
            let idx = id as usize;
            if idx < self.vocab_size {
                output.extend_from_slice(&self.weight[idx]);
            } else {
                output.extend_from_slice(&vec![0.0; self.embedding_dim]);
            }
        }

        output
    }
}

/// RMSNorm (Root Mean Square Layer Normalization)
#[derive(Debug, Clone)]
pub struct RMSNorm {
    pub weight: Vec<f32>,
    pub eps: f32,
}

impl RMSNorm {
    pub fn new(dim: usize, eps: f32) -> Self {
        Self {
            weight: vec![1.0; dim],
            eps,
        }
    }

    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mean_square = input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32;
        let rms = (mean_square + self.eps).sqrt();

        input
            .iter()
            .zip(self.weight.iter())
            .map(|(x, w)| (x / rms) * w)
            .collect()
    }
}

/// LayerNorm
#[derive(Debug, Clone)]
pub struct LayerNorm {
    pub weight: Vec<f32>,
    pub bias: Vec<f32>,
    pub eps: f32,
}

impl LayerNorm {
    pub fn new(dim: usize, eps: f32) -> Self {
        Self {
            weight: vec![1.0; dim],
            bias: vec![0.0; dim],
            eps,
        }
    }

    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mean = input.iter().sum::<f32>() / input.len() as f32;
        let variance = input.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / input.len() as f32;
        let std = (variance + self.eps).sqrt();

        input
            .iter()
            .zip(self.weight.iter().zip(self.bias.iter()))
            .map(|(x, (w, b))| ((x - mean) / std) * w + b)
            .collect()
    }
}

/// SiLU (Swish) activation function
pub fn silu(x: f32) -> f32 {
    x / (1.0 + (-x).exp())
}

/// GELU activation
pub fn gelu(x: f32) -> f32 {
    0.5 * x * (1.0 + ((2.0 / f32::consts::PI).sqrt() * (x + 0.044715 * x.powi(3))).tanh())
}

/// ReLU activation
pub fn relu(x: f32) -> f32 {
    x.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        let mut linear = Linear::new(3, 2, true);
        linear.weight = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        linear.bias = Some(vec![0.1, 0.2]);

        let input = vec![1.0, 2.0, 3.0];
        let output = linear.forward(&input);

        assert_eq!(output.len(), 2);
        assert!((output[0] - 14.1).abs() < 1e-5);
        assert!((output[1] - 32.2).abs() < 1e-5);
    }

    #[test]
    fn test_silu() {
        assert!((silu(0.0) - 0.0).abs() < 1e-5);
        assert!(silu(1.0) > 0.0);
        assert!(silu(-1.0) < 0.0);
    }

    #[test]
    fn test_rms_norm() {
        let norm = RMSNorm::new(4, 1e-6);
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = norm.forward(&input);
        assert_eq!(output.len(), 4);
    }
}
