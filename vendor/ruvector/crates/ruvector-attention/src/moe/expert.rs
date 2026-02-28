//! Expert implementations for MoE attention

use crate::error::AttentionResult;
use crate::utils::stable_softmax;

/// Type of expert
#[derive(Clone, Debug, PartialEq)]
pub enum ExpertType {
    /// Standard scaled dot-product
    Standard,
    /// Hyperbolic attention
    Hyperbolic,
    /// Linear attention
    Linear,
}

/// Expert trait for attention computation
pub trait Expert: Send + Sync {
    /// Compute attention for this expert
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>>;

    /// Get expert type
    fn expert_type(&self) -> ExpertType;

    /// Get dimension
    fn dim(&self) -> usize;
}

/// Standard scaled dot-product expert
pub struct StandardExpert {
    dim: usize,
    scale: f32,
}

impl StandardExpert {
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            scale: 1.0 / (dim as f32).sqrt(),
        }
    }
}

impl Expert for StandardExpert {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        // Compute attention scores
        let scores: Vec<f32> = keys
            .iter()
            .map(|k| {
                query
                    .iter()
                    .zip(k.iter())
                    .map(|(q, ki)| q * ki)
                    .sum::<f32>()
                    * self.scale
            })
            .collect();

        // Softmax
        let weights = stable_softmax(&scores);

        // Weighted sum
        let mut output = vec![0.0f32; self.dim];
        for (weight, value) in weights.iter().zip(values.iter()) {
            for (o, v) in output.iter_mut().zip(value.iter()) {
                *o += weight * v;
            }
        }

        Ok(output)
    }

    fn expert_type(&self) -> ExpertType {
        ExpertType::Standard
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

/// Hyperbolic expert using Poincaré distance
pub struct HyperbolicExpert {
    dim: usize,
    curvature: f32,
}

impl HyperbolicExpert {
    pub fn new(dim: usize, curvature: f32) -> Self {
        Self { dim, curvature }
    }

    fn poincare_distance(&self, u: &[f32], v: &[f32]) -> f32 {
        let c = self.curvature.abs();
        let sqrt_c = c.sqrt();

        let diff_sq: f32 = u.iter().zip(v.iter()).map(|(a, b)| (a - b).powi(2)).sum();
        let norm_u_sq: f32 = u.iter().map(|x| x * x).sum();
        let norm_v_sq: f32 = v.iter().map(|x| x * x).sum();

        let denom = (1.0 - c * norm_u_sq).max(1e-7) * (1.0 - c * norm_v_sq).max(1e-7);
        let arg = 1.0 + 2.0 * c * diff_sq / denom;

        (1.0 / sqrt_c) * arg.max(1.0).acosh()
    }
}

impl Expert for HyperbolicExpert {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        // Use negative Poincaré distance as similarity
        let scores: Vec<f32> = keys
            .iter()
            .map(|k| -self.poincare_distance(query, k))
            .collect();

        let weights = stable_softmax(&scores);

        let mut output = vec![0.0f32; self.dim];
        for (weight, value) in weights.iter().zip(values.iter()) {
            for (o, v) in output.iter_mut().zip(value.iter()) {
                *o += weight * v;
            }
        }

        Ok(output)
    }

    fn expert_type(&self) -> ExpertType {
        ExpertType::Hyperbolic
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

/// Linear attention expert with random features
pub struct LinearExpert {
    dim: usize,
    num_features: usize,
    random_features: Vec<f32>,
}

impl LinearExpert {
    pub fn new(dim: usize, num_features: usize) -> Self {
        use std::f32::consts::PI;

        // Generate random features
        let mut features = Vec::with_capacity(num_features * dim);
        let mut seed = 123u64;

        for _ in 0..((num_features * dim + 1) / 2) {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u1 = (seed as f32) / (u64::MAX as f32);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u2 = (seed as f32) / (u64::MAX as f32);

            let r = (-2.0 * u1.max(1e-10).ln()).sqrt();
            let theta = 2.0 * PI * u2;

            features.push(r * theta.cos() / (dim as f32).sqrt());
            if features.len() < num_features * dim {
                features.push(r * theta.sin() / (dim as f32).sqrt());
            }
        }
        features.truncate(num_features * dim);

        Self {
            dim,
            num_features,
            random_features: features,
        }
    }

    fn feature_map(&self, x: &[f32]) -> Vec<f32> {
        (0..self.num_features)
            .map(|i| {
                let proj: f32 = x
                    .iter()
                    .enumerate()
                    .map(|(j, &xj)| xj * self.random_features[i * self.dim + j])
                    .sum();
                let norm_sq: f32 = x.iter().map(|xi| xi * xi).sum();
                (proj - norm_sq / 2.0).exp() / (self.num_features as f32).sqrt()
            })
            .collect()
    }
}

impl Expert for LinearExpert {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let phi_q = self.feature_map(query);
        let value_dim = values.get(0).map(|v| v.len()).unwrap_or(self.dim);

        let mut kv_sum = vec![0.0f32; self.num_features * value_dim];
        let mut k_sum = vec![0.0f32; self.num_features];

        for (key, value) in keys.iter().zip(values.iter()) {
            let phi_k = self.feature_map(key);
            for (i, &phi_ki) in phi_k.iter().enumerate() {
                for (j, &vj) in value.iter().enumerate() {
                    kv_sum[i * value_dim + j] += phi_ki * vj;
                }
                k_sum[i] += phi_ki;
            }
        }

        let mut output = vec![0.0f32; value_dim];
        let mut normalizer = 0.0f32;

        for (i, &phi_qi) in phi_q.iter().enumerate() {
            for (j, out_j) in output.iter_mut().enumerate() {
                *out_j += phi_qi * kv_sum[i * value_dim + j];
            }
            normalizer += phi_qi * k_sum[i];
        }

        if normalizer.abs() > 1e-8 {
            output.iter_mut().for_each(|x| *x /= normalizer);
        }

        Ok(output)
    }

    fn expert_type(&self) -> ExpertType {
        ExpertType::Linear
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_expert() {
        let expert = StandardExpert::new(64);
        let query = vec![0.5; 64];
        let keys: Vec<Vec<f32>> = vec![vec![0.3; 64]; 10];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 64]; 10];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = expert.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_hyperbolic_expert() {
        let expert = HyperbolicExpert::new(32, 1.0);
        let query = vec![0.1; 32]; // Small values to stay in ball
        let keys: Vec<Vec<f32>> = vec![vec![0.1; 32]; 5];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 32]; 5];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = expert.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_linear_expert() {
        let expert = LinearExpert::new(64, 32);
        let query = vec![0.5; 64];
        let keys: Vec<Vec<f32>> = vec![vec![0.3; 64]; 10];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 64]; 10];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = expert.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 64);
    }
}
