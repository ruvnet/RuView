//! Mixed-Curvature Attention combining Euclidean and Hyperbolic spaces

use super::poincare::{frechet_mean, poincare_distance, project_to_ball};
use crate::error::AttentionResult;
use crate::traits::Attention;

#[derive(Debug, Clone)]
pub struct MixedCurvatureConfig {
    pub euclidean_dim: usize,
    pub hyperbolic_dim: usize,
    pub curvature: f32,
    pub mixing_weight: f32,
    pub temperature: f32,
    pub frechet_max_iter: usize,
    pub frechet_tol: f32,
}

impl Default for MixedCurvatureConfig {
    fn default() -> Self {
        Self {
            euclidean_dim: 64,
            hyperbolic_dim: 64,
            curvature: -1.0,
            mixing_weight: 0.5,
            temperature: 1.0,
            frechet_max_iter: 50,
            frechet_tol: 1e-5,
        }
    }
}

pub struct MixedCurvatureAttention {
    config: MixedCurvatureConfig,
}

impl MixedCurvatureAttention {
    pub fn new(config: MixedCurvatureConfig) -> Self {
        Self { config }
    }

    fn total_dim(&self) -> usize {
        self.config.euclidean_dim + self.config.hyperbolic_dim
    }

    fn split_embedding<'a>(&self, x: &'a [f32]) -> (&'a [f32], &'a [f32]) {
        let euclidean = &x[..self.config.euclidean_dim];
        let hyperbolic = &x[self.config.euclidean_dim..];
        (euclidean, hyperbolic)
    }

    fn softmax(&self, scores: &[f32]) -> Vec<f32> {
        if scores.is_empty() {
            return vec![];
        }

        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_scores: Vec<f32> = scores
            .iter()
            .map(|&s| ((s - max_score) / self.config.temperature).exp())
            .collect();

        let sum: f32 = exp_scores.iter().sum();
        if sum < 1e-10 {
            vec![1.0 / scores.len() as f32; scores.len()]
        } else {
            exp_scores.iter().map(|&e| e / sum).collect()
        }
    }

    fn compute_euclidean_weights(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        let scores: Vec<f32> = keys
            .iter()
            .map(|k| query.iter().zip(k.iter()).map(|(q, k)| q * k).sum())
            .collect();
        self.softmax(&scores)
    }

    fn compute_hyperbolic_weights(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        let c = self.config.curvature.abs();
        let query_proj = project_to_ball(query, c, 1e-7);
        let keys_proj: Vec<Vec<f32>> = keys.iter().map(|k| project_to_ball(k, c, 1e-7)).collect();

        let scores: Vec<f32> = keys_proj
            .iter()
            .map(|k| -poincare_distance(&query_proj, k, c))
            .collect();
        self.softmax(&scores)
    }

    fn aggregate_euclidean(&self, weights: &[f32], values: &[&[f32]]) -> Vec<f32> {
        let dim = values.get(0).map(|v| v.len()).unwrap_or(0);
        let mut result = vec![0.0; dim];

        for (weight, value) in weights.iter().zip(values.iter()) {
            for (i, &v) in value.iter().enumerate() {
                result[i] += weight * v;
            }
        }

        result
    }

    fn aggregate_hyperbolic(&self, weights: &[f32], values: &[&[f32]]) -> Vec<f32> {
        if values.is_empty() {
            return vec![0.0; self.config.hyperbolic_dim];
        }

        let c = self.config.curvature.abs();
        let values_proj: Vec<Vec<f32>> =
            values.iter().map(|v| project_to_ball(v, c, 1e-7)).collect();
        let values_refs: Vec<&[f32]> = values_proj.iter().map(|v| v.as_slice()).collect();

        frechet_mean(
            &values_refs,
            Some(weights),
            c,
            self.config.frechet_max_iter,
            self.config.frechet_tol,
        )
    }

    fn combine_components(&self, euclidean: Vec<f32>, hyperbolic: Vec<f32>) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.total_dim());
        result.extend(euclidean);
        result.extend(hyperbolic);
        result
    }
}

impl Attention for MixedCurvatureAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let (query_euc, query_hyp) = self.split_embedding(query);

        let keys_euc: Vec<&[f32]> = keys
            .iter()
            .map(|k| &k[..self.config.euclidean_dim])
            .collect();
        let keys_hyp: Vec<&[f32]> = keys
            .iter()
            .map(|k| &k[self.config.euclidean_dim..])
            .collect();

        let values_euc: Vec<&[f32]> = values
            .iter()
            .map(|v| &v[..self.config.euclidean_dim])
            .collect();
        let values_hyp: Vec<&[f32]> = values
            .iter()
            .map(|v| &v[self.config.euclidean_dim..])
            .collect();

        let weights_euc = self.compute_euclidean_weights(query_euc, &keys_euc);
        let weights_hyp = self.compute_hyperbolic_weights(query_hyp, &keys_hyp);

        let alpha = self.config.mixing_weight;
        let combined_weights: Vec<f32> = weights_euc
            .iter()
            .zip(&weights_hyp)
            .map(|(&w_e, &w_h)| (1.0 - alpha) * w_e + alpha * w_h)
            .collect();

        let sum: f32 = combined_weights.iter().sum();
        let normalized_weights: Vec<f32> = if sum > 1e-10 {
            combined_weights.iter().map(|&w| w / sum).collect()
        } else {
            vec![1.0 / combined_weights.len() as f32; combined_weights.len()]
        };

        let result_euc = self.aggregate_euclidean(&normalized_weights, &values_euc);
        let result_hyp = self.aggregate_hyperbolic(&normalized_weights, &values_hyp);

        Ok(self.combine_components(result_euc, result_hyp))
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        let (query_euc, query_hyp) = self.split_embedding(query);

        let keys_euc: Vec<&[f32]> = keys
            .iter()
            .map(|k| &k[..self.config.euclidean_dim])
            .collect();
        let keys_hyp: Vec<&[f32]> = keys
            .iter()
            .map(|k| &k[self.config.euclidean_dim..])
            .collect();
        let values_euc: Vec<&[f32]> = values
            .iter()
            .map(|v| &v[..self.config.euclidean_dim])
            .collect();
        let values_hyp: Vec<&[f32]> = values
            .iter()
            .map(|v| &v[self.config.euclidean_dim..])
            .collect();

        let weights_euc = self.compute_euclidean_weights(query_euc, &keys_euc);
        let weights_hyp = self.compute_hyperbolic_weights(query_hyp, &keys_hyp);

        let alpha = self.config.mixing_weight;
        let mut combined_weights: Vec<f32> = weights_euc
            .iter()
            .zip(&weights_hyp)
            .map(|(&w_e, &w_h)| (1.0 - alpha) * w_e + alpha * w_h)
            .collect();

        if let Some(mask_vec) = mask {
            for (i, &masked) in mask_vec.iter().enumerate() {
                if !masked && i < combined_weights.len() {
                    combined_weights[i] = 0.0;
                }
            }
        }

        let sum: f32 = combined_weights.iter().sum();
        let normalized_weights: Vec<f32> = if sum > 1e-10 {
            combined_weights.iter().map(|&w| w / sum).collect()
        } else {
            vec![1.0 / combined_weights.len() as f32; combined_weights.len()]
        };

        let result_euc = self.aggregate_euclidean(&normalized_weights, &values_euc);
        let result_hyp = self.aggregate_hyperbolic(&normalized_weights, &values_hyp);

        Ok(self.combine_components(result_euc, result_hyp))
    }

    fn dim(&self) -> usize {
        self.total_dim()
    }
}
