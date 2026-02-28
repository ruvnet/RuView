//! Mixture of Experts attention layer

use super::expert::{Expert, HyperbolicExpert, LinearExpert, StandardExpert};
use super::router::{LearnedRouter, Router, TopKRouting};
use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;

/// MoE configuration
#[derive(Clone, Debug)]
pub struct MoEConfig {
    pub dim: usize,
    pub num_experts: usize,
    pub top_k: usize,
    pub expert_capacity: f32,
    pub jitter_noise: f32,
}

impl Default for MoEConfig {
    fn default() -> Self {
        Self {
            dim: 256,
            num_experts: 4,
            top_k: 2,
            expert_capacity: 1.25,
            jitter_noise: 0.0,
        }
    }
}

impl MoEConfig {
    pub fn builder() -> MoEConfigBuilder {
        MoEConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct MoEConfigBuilder {
    config: MoEConfig,
}

impl MoEConfigBuilder {
    pub fn dim(mut self, dim: usize) -> Self {
        self.config.dim = dim;
        self
    }

    pub fn num_experts(mut self, n: usize) -> Self {
        self.config.num_experts = n;
        self
    }

    pub fn top_k(mut self, k: usize) -> Self {
        self.config.top_k = k;
        self
    }

    pub fn expert_capacity(mut self, c: f32) -> Self {
        self.config.expert_capacity = c;
        self
    }

    pub fn jitter_noise(mut self, j: f32) -> Self {
        self.config.jitter_noise = j;
        self
    }

    pub fn build(self) -> MoEConfig {
        self.config
    }
}

/// Mixture of Experts attention
pub struct MoEAttention {
    experts: Vec<Box<dyn Expert>>,
    router: LearnedRouter,
    config: MoEConfig,
}

impl MoEAttention {
    /// Create new MoE attention
    pub fn new(config: MoEConfig) -> Self {
        // Create diverse experts
        let mut experts: Vec<Box<dyn Expert>> = Vec::new();

        // Ensure we have at least num_experts
        let num_each = (config.num_experts + 2) / 3;

        for _ in 0..num_each {
            experts.push(Box::new(StandardExpert::new(config.dim)));
        }
        for _ in 0..num_each {
            experts.push(Box::new(HyperbolicExpert::new(config.dim, 1.0)));
        }
        for _ in 0..num_each {
            experts.push(Box::new(LinearExpert::new(config.dim, config.dim / 4)));
        }

        experts.truncate(config.num_experts);

        let router = LearnedRouter::new(config.num_experts, config.dim, config.top_k);

        Self {
            experts,
            router,
            config,
        }
    }

    /// Compute with auxiliary load balance loss
    pub fn compute_with_loss(
        &self,
        queries: &[&[f32]],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<(Vec<Vec<f32>>, f32)> {
        let mut outputs = Vec::with_capacity(queries.len());
        let mut routing_decisions = Vec::with_capacity(queries.len());

        for query in queries {
            let routes = self.router.route(query);
            routing_decisions.push(TopKRouting {
                selections: routes.clone(),
            });

            let mut output = vec![0.0f32; self.config.dim];
            for (expert_idx, weight) in routes {
                let expert_output = self.experts[expert_idx].compute(query, keys, values)?;
                for (o, e) in output.iter_mut().zip(expert_output.iter()) {
                    *o += weight * e;
                }
            }
            outputs.push(output);
        }

        let loss = self.router.load_balance_loss(&routing_decisions);
        Ok((outputs, loss))
    }

    /// Get expert usage statistics
    pub fn expert_statistics(&self, routing_decisions: &[TopKRouting]) -> Vec<f32> {
        self.router.expert_statistics(routing_decisions)
    }
}

impl Attention for MoEAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty keys".to_string()));
        }
        if query.len() != self.config.dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.config.dim,
                actual: query.len(),
            });
        }

        // Route query to experts
        let routes = self.router.route(query);

        // Compute weighted sum of expert outputs
        let mut output = vec![0.0f32; self.config.dim];

        for (expert_idx, weight) in routes {
            let expert_output = self.experts[expert_idx].compute(query, keys, values)?;
            for (o, e) in output.iter_mut().zip(expert_output.iter()) {
                *o += weight * e;
            }
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
        self.config.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moe_attention() {
        let config = MoEConfig::builder().dim(64).num_experts(4).top_k(2).build();

        let moe = MoEAttention::new(config);

        let query = vec![0.5; 64];
        let keys: Vec<Vec<f32>> = vec![vec![0.3; 64]; 10];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 64]; 10];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = moe.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_moe_with_loss() {
        let config = MoEConfig::builder().dim(32).num_experts(4).top_k(2).build();

        let moe = MoEAttention::new(config);

        let queries: Vec<Vec<f32>> = (0..10).map(|_| vec![0.5; 32]).collect();
        let keys: Vec<Vec<f32>> = vec![vec![0.3; 32]; 5];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 32]; 5];

        let query_refs: Vec<&[f32]> = queries.iter().map(|q| q.as_slice()).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let (outputs, loss) = moe
            .compute_with_loss(&query_refs, &keys_refs, &values_refs)
            .unwrap();

        assert_eq!(outputs.len(), 10);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_config_builder() {
        let config = MoEConfig::builder()
            .dim(128)
            .num_experts(8)
            .top_k(3)
            .expert_capacity(1.5)
            .jitter_noise(0.1)
            .build();

        assert_eq!(config.dim, 128);
        assert_eq!(config.num_experts, 8);
        assert_eq!(config.top_k, 3);
    }
}
