//! Linear attention using random feature approximation (Performer-style)
//!
//! Complexity: O(n * k * d) where k = number of random features

use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;

/// Kernel type for linear attention
#[derive(Clone, Debug)]
pub enum KernelType {
    /// FAVOR+ softmax approximation
    Softmax,
    /// ReLU kernel
    ReLU,
    /// ELU kernel
    ELU,
}

/// Linear attention with random feature maps
///
/// Uses kernel trick to achieve O(n * k * d) complexity instead of O(n² * d).
pub struct LinearAttention {
    dim: usize,
    num_features: usize,
    kernel: KernelType,
    /// Random projection matrix [num_features x dim]
    random_features: Vec<f32>,
}

impl LinearAttention {
    /// Create new linear attention
    pub fn new(dim: usize, num_features: usize) -> Self {
        Self::with_kernel(dim, num_features, KernelType::Softmax)
    }

    /// Create with specific kernel type
    pub fn with_kernel(dim: usize, num_features: usize, kernel: KernelType) -> Self {
        // Initialize random features using Box-Muller for Gaussian
        let random_features = Self::generate_random_features(dim, num_features);

        Self {
            dim,
            num_features,
            kernel,
            random_features,
        }
    }

    fn generate_random_features(dim: usize, num_features: usize) -> Vec<f32> {
        use std::f32::consts::PI;

        let mut features = Vec::with_capacity(num_features * dim);
        let mut seed = 42u64;

        for _ in 0..((num_features * dim + 1) / 2) {
            // Simple LCG for reproducibility
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u1 = (seed as f32) / (u64::MAX as f32);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u2 = (seed as f32) / (u64::MAX as f32);

            // Box-Muller transform
            let r = (-2.0 * u1.max(1e-10).ln()).sqrt();
            let theta = 2.0 * PI * u2;

            features.push(r * theta.cos());
            if features.len() < num_features * dim {
                features.push(r * theta.sin());
            }
        }

        features.truncate(num_features * dim);

        // Normalize columns
        let scale = 1.0 / (dim as f32).sqrt();
        features.iter_mut().for_each(|x| *x *= scale);

        features
    }

    /// Apply feature map to input
    fn feature_map(&self, x: &[f32]) -> Vec<f32> {
        let mut phi = vec![0.0f32; self.num_features];

        for (i, phi_i) in phi.iter_mut().enumerate() {
            let projection: f32 = x
                .iter()
                .enumerate()
                .map(|(j, &xj)| xj * self.random_features[i * self.dim + j])
                .sum();

            *phi_i = match self.kernel {
                KernelType::Softmax => {
                    // FAVOR+: exp(projection - ||x||²/2) / sqrt(num_features)
                    let norm_sq: f32 = x.iter().map(|xi| xi * xi).sum();
                    (projection - norm_sq / 2.0).exp() / (self.num_features as f32).sqrt()
                }
                KernelType::ReLU => projection.max(0.0),
                KernelType::ELU => {
                    if projection >= 0.0 {
                        projection
                    } else {
                        projection.exp() - 1.0
                    }
                }
            };
        }

        phi
    }
}

impl Attention for LinearAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty keys".to_string()));
        }
        if keys.len() != values.len() {
            return Err(AttentionError::DimensionMismatch {
                expected: keys.len(),
                actual: values.len(),
            });
        }
        if query.len() != self.dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.dim,
                actual: query.len(),
            });
        }

        // Compute phi(Q)
        let phi_q = self.feature_map(query);

        // Compute sum_i phi(K_i)^T * V_i  and  sum_i phi(K_i)
        let value_dim = values[0].len();
        let mut kv_sum = vec![0.0f32; self.num_features * value_dim]; // [num_features x value_dim]
        let mut k_sum = vec![0.0f32; self.num_features];

        for (key, value) in keys.iter().zip(values.iter()) {
            let phi_k = self.feature_map(key);

            // Accumulate phi(K)^T * V (outer product contribution)
            for (i, &phi_ki) in phi_k.iter().enumerate() {
                for (j, &vj) in value.iter().enumerate() {
                    kv_sum[i * value_dim + j] += phi_ki * vj;
                }
                k_sum[i] += phi_ki;
            }
        }

        // Compute output: (phi(Q)^T * KV_sum) / (phi(Q)^T * K_sum)
        let mut output = vec![0.0f32; value_dim];
        let mut normalizer = 0.0f32;

        for (i, &phi_qi) in phi_q.iter().enumerate() {
            for (j, out_j) in output.iter_mut().enumerate() {
                *out_j += phi_qi * kv_sum[i * value_dim + j];
            }
            normalizer += phi_qi * k_sum[i];
        }

        // Normalize
        if normalizer.abs() > 1e-8 {
            output.iter_mut().for_each(|x| *x /= normalizer);
        }

        Ok(output)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        if let Some(m) = mask {
            let filtered: Vec<(usize, bool)> = m
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, keep)| *keep)
                .collect();
            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(i, _)| keys[*i]).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(i, _)| values[*i]).collect();
            self.compute(query, &filtered_keys, &filtered_values)
        } else {
            self.compute(query, keys, values)
        }
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_attention() {
        let attention = LinearAttention::new(64, 32);

        let query = vec![0.5; 64];
        let keys: Vec<Vec<f32>> = (0..100).map(|_| vec![0.3; 64]).collect();
        let values: Vec<Vec<f32>> = (0..100).map(|_| vec![1.0; 64]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_kernel_types() {
        for kernel in [KernelType::Softmax, KernelType::ReLU, KernelType::ELU] {
            let attention = LinearAttention::with_kernel(32, 16, kernel);

            let query = vec![1.0; 32];
            let keys = vec![vec![0.5; 32]; 10];
            let values = vec![vec![1.0; 32]; 10];

            let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

            let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
            assert_eq!(result.len(), 32);
        }
    }
}
