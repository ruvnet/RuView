//! Hyperbolic Attention Mechanism using Poincaré ball model

use super::poincare::{frechet_mean, poincare_distance, project_to_ball};
use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;

/// Configuration for hyperbolic attention
#[derive(Debug, Clone)]
pub struct HyperbolicAttentionConfig {
    pub dim: usize,
    pub curvature: f32,
    pub adaptive_curvature: bool,
    pub temperature: f32,
    pub frechet_max_iter: usize,
    pub frechet_tol: f32,
}

impl Default for HyperbolicAttentionConfig {
    fn default() -> Self {
        Self {
            dim: 128,
            curvature: -1.0,
            adaptive_curvature: false,
            temperature: 1.0,
            frechet_max_iter: 50,
            frechet_tol: 1e-5,
        }
    }
}

/// Hyperbolic Attention mechanism
pub struct HyperbolicAttention {
    config: HyperbolicAttentionConfig,
    current_curvature: f32,
}

impl HyperbolicAttention {
    pub fn new(config: HyperbolicAttentionConfig) -> Self {
        let current_curvature = config.curvature.abs();
        Self {
            config,
            current_curvature,
        }
    }

    pub fn compute_weights(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        if keys.is_empty() {
            return vec![];
        }

        let scores: Vec<f32> = keys
            .iter()
            .map(|k| -poincare_distance(query, k, self.current_curvature))
            .collect();

        self.softmax_with_temperature(&scores)
    }

    fn softmax_with_temperature(&self, scores: &[f32]) -> Vec<f32> {
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

    pub fn aggregate(&self, weights: &[f32], values: &[&[f32]]) -> Vec<f32> {
        if values.is_empty() {
            return vec![0.0; self.config.dim];
        }

        if values.len() == 1 {
            return values[0].to_vec();
        }

        frechet_mean(
            values,
            Some(weights),
            self.current_curvature,
            self.config.frechet_max_iter,
            self.config.frechet_tol,
        )
    }
}

impl Attention for HyperbolicAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() || values.is_empty() {
            return Err(AttentionError::EmptyInput(
                "Keys and values cannot be empty".to_string(),
            ));
        }

        let query_proj = project_to_ball(query, self.current_curvature, 1e-7);
        let keys_proj: Vec<Vec<f32>> = keys
            .iter()
            .map(|k| project_to_ball(k, self.current_curvature, 1e-7))
            .collect();
        let values_proj: Vec<Vec<f32>> = values
            .iter()
            .map(|v| project_to_ball(v, self.current_curvature, 1e-7))
            .collect();

        let keys_refs: Vec<&[f32]> = keys_proj.iter().map(|k| k.as_slice()).collect();
        let weights = self.compute_weights(&query_proj, &keys_refs);

        let values_refs: Vec<&[f32]> = values_proj.iter().map(|v| v.as_slice()).collect();
        let result = self.aggregate(&weights, &values_refs);

        Ok(result)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        let query_proj = project_to_ball(query, self.current_curvature, 1e-7);
        let keys_proj: Vec<Vec<f32>> = keys
            .iter()
            .map(|k| project_to_ball(k, self.current_curvature, 1e-7))
            .collect();
        let values_proj: Vec<Vec<f32>> = values
            .iter()
            .map(|v| project_to_ball(v, self.current_curvature, 1e-7))
            .collect();

        let keys_refs: Vec<&[f32]> = keys_proj.iter().map(|k| k.as_slice()).collect();
        let mut weights = self.compute_weights(&query_proj, &keys_refs);

        if let Some(mask_vec) = mask {
            for (i, &masked) in mask_vec.iter().enumerate() {
                if !masked && i < weights.len() {
                    weights[i] = 0.0;
                }
            }

            let sum: f32 = weights.iter().sum();
            if sum > 1e-10 {
                for w in &mut weights {
                    *w /= sum;
                }
            }
        }

        let values_refs: Vec<&[f32]> = values_proj.iter().map(|v| v.as_slice()).collect();
        Ok(self.aggregate(&weights, &values_refs))
    }

    fn dim(&self) -> usize {
        self.config.dim
    }
}
