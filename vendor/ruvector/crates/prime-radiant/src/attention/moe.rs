//! Mixture of Experts Residual Processing
//!
//! Specialized expert routing for different residual characteristics.

use super::{AttentionCoherenceConfig, AttentionError, MoEProcessedResidual, Result};

/// Expert routing decision
#[derive(Debug, Clone)]
pub struct ExpertRouting {
    /// Selected expert indices
    pub expert_indices: Vec<usize>,
    /// Weights for each selected expert
    pub weights: Vec<f32>,
    /// Router logits (before top-k selection)
    pub router_logits: Vec<f32>,
}

impl ExpertRouting {
    /// Check if a specific expert was selected
    pub fn contains_expert(&self, idx: usize) -> bool {
        self.expert_indices.contains(&idx)
    }

    /// Get weight for a specific expert (0 if not selected)
    pub fn weight_for(&self, idx: usize) -> f32 {
        self.expert_indices
            .iter()
            .position(|&i| i == idx)
            .map(|pos| self.weights[pos])
            .unwrap_or(0.0)
    }
}

/// Mixture of Experts residual processor
///
/// Routes residuals to specialized experts based on their characteristics.
/// Each expert specializes in different types of residuals.
#[derive(Debug)]
pub struct MoEResidualProcessor {
    /// Configuration
    config: AttentionCoherenceConfig,
    /// Expert parameters (weights for each expert)
    experts: Vec<ExpertParams>,
    /// Router parameters
    router: RouterParams,
}

/// Parameters for a single expert
#[derive(Debug, Clone)]
struct ExpertParams {
    /// Linear transformation weights (dim x dim)
    weights: Vec<Vec<f32>>,
    /// Bias vector
    bias: Vec<f32>,
    /// Expert specialization (for interpretability)
    specialization: ExpertSpecialization,
}

/// Type of expert specialization
#[derive(Debug, Clone, Copy)]
enum ExpertSpecialization {
    /// High-magnitude residuals
    HighMagnitude,
    /// Low-magnitude residuals
    LowMagnitude,
    /// Sparse residuals
    Sparse,
    /// Dense residuals
    Dense,
}

/// Router parameters
#[derive(Debug, Clone)]
struct RouterParams {
    /// Router weights (num_experts x dim)
    weights: Vec<Vec<f32>>,
    /// Noise scale for exploration
    jitter_noise: f32,
}

impl MoEResidualProcessor {
    /// Create a new MoE processor
    pub fn new(config: AttentionCoherenceConfig) -> Self {
        let num_experts = config.num_experts;
        let dim = config.dimension;

        // Initialize experts with different specializations
        let specializations = [
            ExpertSpecialization::HighMagnitude,
            ExpertSpecialization::LowMagnitude,
            ExpertSpecialization::Sparse,
            ExpertSpecialization::Dense,
        ];

        let experts: Vec<ExpertParams> = (0..num_experts)
            .map(|i| {
                // Initialize with identity-like transformation
                let weights: Vec<Vec<f32>> = (0..dim)
                    .map(|j| {
                        let mut row = vec![0.0f32; dim];
                        row[j] = 1.0 + 0.1 * (i as f32 - num_experts as f32 / 2.0);
                        row
                    })
                    .collect();

                ExpertParams {
                    weights,
                    bias: vec![0.0; dim],
                    specialization: specializations[i % specializations.len()],
                }
            })
            .collect();

        // Initialize router
        let router_weights: Vec<Vec<f32>> = (0..num_experts)
            .map(|i| {
                // Different experts respond to different features
                let mut row = vec![0.1f32; dim];
                // Make each expert sensitive to different dimensions
                let start = (i * dim / num_experts).min(dim - 1);
                let end = ((i + 1) * dim / num_experts).min(dim);
                for j in start..end {
                    row[j] = 1.0;
                }
                row
            })
            .collect();

        let router = RouterParams {
            weights: router_weights,
            jitter_noise: 0.0,
        };

        Self {
            config,
            experts,
            router,
        }
    }

    /// Process a residual through MoE
    pub fn process(&self, residual: &[f32], context: &[f32]) -> Result<MoEProcessedResidual> {
        // Validate dimensions
        if residual.len() != self.config.dimension {
            return Err(AttentionError::DimensionMismatch {
                expected: self.config.dimension,
                actual: residual.len(),
            });
        }

        // Route to experts
        let routing = self.route(residual, context);

        // Process through selected experts
        let mut output = vec![0.0f32; self.config.dimension];

        for (&expert_idx, &weight) in routing.expert_indices.iter().zip(routing.weights.iter()) {
            let expert_output = self.apply_expert(expert_idx, residual);
            for (o, e) in output.iter_mut().zip(expert_output.iter()) {
                *o += weight * e;
            }
        }

        // Compute load balance loss
        let load_balance_loss = self.compute_load_balance_loss(&routing);

        Ok(MoEProcessedResidual {
            output,
            expert_indices: routing.expert_indices,
            expert_weights: routing.weights,
            load_balance_loss,
        })
    }

    /// Route input to experts
    pub fn route(&self, input: &[f32], _context: &[f32]) -> ExpertRouting {
        // Compute router logits
        let logits: Vec<f32> = self
            .router
            .weights
            .iter()
            .map(|w| self.dot_product(input, w))
            .collect();

        // Top-k selection
        let k = self.config.moe_top_k.min(self.config.num_experts);

        let mut indexed_logits: Vec<(usize, f32)> =
            logits.iter().enumerate().map(|(i, &l)| (i, l)).collect();

        indexed_logits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_k: Vec<(usize, f32)> = indexed_logits.into_iter().take(k).collect();

        // Softmax over selected
        let max_logit = top_k
            .iter()
            .map(|(_, l)| *l)
            .fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = top_k.iter().map(|(_, l)| (l - max_logit).exp()).sum();

        let expert_indices: Vec<usize> = top_k.iter().map(|(i, _)| *i).collect();
        let weights: Vec<f32> = top_k
            .iter()
            .map(|(_, l)| (l - max_logit).exp() / exp_sum)
            .collect();

        ExpertRouting {
            expert_indices,
            weights,
            router_logits: logits,
        }
    }

    /// Apply a single expert
    fn apply_expert(&self, expert_idx: usize, input: &[f32]) -> Vec<f32> {
        let expert = &self.experts[expert_idx];
        let dim = input.len();

        let mut output = expert.bias.clone();

        // Matrix-vector multiply
        for (i, w_row) in expert.weights.iter().enumerate() {
            if i < dim {
                for (j, &x) in input.iter().enumerate() {
                    if j < w_row.len() {
                        output[i] += w_row[j] * x;
                    }
                }
            }
        }

        output
    }

    /// Compute load balance loss
    fn compute_load_balance_loss(&self, routing: &ExpertRouting) -> f32 {
        // Count how many times each expert is used
        let mut usage = vec![0.0f32; self.config.num_experts];
        for (&idx, &weight) in routing.expert_indices.iter().zip(routing.weights.iter()) {
            usage[idx] += weight;
        }

        // Ideal uniform distribution
        let ideal = 1.0 / self.config.num_experts as f32;

        // L2 deviation from uniform
        usage.iter().map(|&u| (u - ideal).powi(2)).sum::<f32>()
    }

    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Get expert statistics
    pub fn expert_usage(&self, routings: &[ExpertRouting]) -> Vec<f32> {
        let mut usage = vec![0.0f32; self.config.num_experts];

        for routing in routings {
            for (&idx, &weight) in routing.expert_indices.iter().zip(routing.weights.iter()) {
                usage[idx] += weight;
            }
        }

        // Normalize
        let total: f32 = usage.iter().sum();
        if total > 0.0 {
            for u in usage.iter_mut() {
                *u /= total;
            }
        }

        usage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moe_creation() {
        let config = AttentionCoherenceConfig {
            dimension: 16,
            num_experts: 4,
            moe_top_k: 2,
            ..Default::default()
        };
        let moe = MoEResidualProcessor::new(config);

        assert_eq!(moe.experts.len(), 4);
    }

    #[test]
    fn test_routing() {
        let config = AttentionCoherenceConfig {
            dimension: 8,
            num_experts: 4,
            moe_top_k: 2,
            ..Default::default()
        };
        let moe = MoEResidualProcessor::new(config);

        let input = vec![0.5f32; 8];
        let context = vec![0.1f32; 8];

        let routing = moe.route(&input, &context);

        assert_eq!(routing.expert_indices.len(), 2);
        assert_eq!(routing.weights.len(), 2);

        // Weights should sum to approximately 1
        let sum: f32 = routing.weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_process() {
        let config = AttentionCoherenceConfig {
            dimension: 8,
            num_experts: 4,
            moe_top_k: 2,
            ..Default::default()
        };
        let moe = MoEResidualProcessor::new(config);

        let residual = vec![0.1f32; 8];
        let context = vec![0.1f32; 8];

        let result = moe.process(&residual, &context).unwrap();

        assert_eq!(result.output.len(), 8);
        assert_eq!(result.expert_indices.len(), 2);
        assert!(result.load_balance_loss >= 0.0);
    }

    #[test]
    fn test_expert_usage() {
        let config = AttentionCoherenceConfig {
            dimension: 8,
            num_experts: 4,
            moe_top_k: 2,
            ..Default::default()
        };
        let moe = MoEResidualProcessor::new(config);

        let inputs: Vec<Vec<f32>> = (0..10).map(|i| vec![0.1 * (i + 1) as f32; 8]).collect();
        let context = vec![0.1f32; 8];

        let routings: Vec<ExpertRouting> =
            inputs.iter().map(|inp| moe.route(inp, &context)).collect();

        let usage = moe.expert_usage(&routings);

        assert_eq!(usage.len(), 4);
        // Should sum to approximately 1
        let sum: f32 = usage.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }
}
