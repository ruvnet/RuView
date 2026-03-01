//! Router implementations for MoE expert selection

use crate::utils::stable_softmax;

/// Router trait for expert selection
pub trait Router: Send + Sync {
    /// Route input to experts, returning (expert_idx, weight) pairs
    fn route(&self, x: &[f32]) -> Vec<(usize, f32)>;

    /// Get number of experts
    fn num_experts(&self) -> usize;
}

/// Top-K routing decision
#[derive(Clone, Debug)]
pub struct TopKRouting {
    /// Selected experts with their normalized weights
    pub selections: Vec<(usize, f32)>,
}

/// Learned router with softmax gating
pub struct LearnedRouter {
    num_experts: usize,
    dim: usize,
    top_k: usize,
    /// Gate weights: [num_experts x dim]
    gate_weights: Vec<f32>,
}

impl LearnedRouter {
    /// Create new learned router
    pub fn new(num_experts: usize, dim: usize, top_k: usize) -> Self {
        // Initialize gate weights with Xavier initialization
        let scale = (2.0 / (dim + num_experts) as f32).sqrt();
        let mut seed = 42u64;

        let gate_weights: Vec<f32> = (0..num_experts * dim)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let u = (seed as f32) / (u64::MAX as f32);
                (u - 0.5) * 2.0 * scale
            })
            .collect();

        Self {
            num_experts,
            dim,
            top_k: top_k.min(num_experts),
            gate_weights,
        }
    }

    /// Compute raw gate logits
    fn compute_logits(&self, x: &[f32]) -> Vec<f32> {
        (0..self.num_experts)
            .map(|i| {
                x.iter()
                    .enumerate()
                    .map(|(j, &xj)| xj * self.gate_weights[i * self.dim + j])
                    .sum()
            })
            .collect()
    }

    /// Compute gate probabilities
    pub fn compute_gate(&self, x: &[f32]) -> Vec<f32> {
        let logits = self.compute_logits(x);
        stable_softmax(&logits)
    }

    /// Compute load balancing loss for batch
    pub fn load_balance_loss(&self, routing_decisions: &[TopKRouting]) -> f32 {
        if routing_decisions.is_empty() {
            return 0.0;
        }

        let batch_size = routing_decisions.len() as f32;

        // Count how many times each expert is used
        let mut expert_counts = vec![0.0f32; self.num_experts];
        let mut total_weight = vec![0.0f32; self.num_experts];

        for decision in routing_decisions {
            for &(expert_idx, weight) in &decision.selections {
                expert_counts[expert_idx] += 1.0;
                total_weight[expert_idx] += weight;
            }
        }

        // Compute auxiliary loss: encourage uniform distribution
        let _avg_count = expert_counts.iter().sum::<f32>() / self.num_experts as f32;
        let _avg_weight = total_weight.iter().sum::<f32>() / self.num_experts as f32;

        // CV-squared loss from Switch Transformer paper
        let count_var: f32 = expert_counts
            .iter()
            .map(|c| (c / batch_size - 1.0 / self.num_experts as f32).powi(2))
            .sum();

        self.num_experts as f32 * count_var
    }

    /// Update gate weights (for training)
    pub fn update_weights(&mut self, gradients: &[f32], learning_rate: f32) {
        for (w, g) in self.gate_weights.iter_mut().zip(gradients.iter()) {
            *w -= learning_rate * g;
        }
    }

    /// Get expert usage statistics
    pub fn expert_statistics(&self, routing_decisions: &[TopKRouting]) -> Vec<f32> {
        let mut counts = vec![0.0f32; self.num_experts];

        for decision in routing_decisions {
            for &(expert_idx, _) in &decision.selections {
                counts[expert_idx] += 1.0;
            }
        }

        let total: f32 = counts.iter().sum();
        if total > 0.0 {
            counts.iter_mut().for_each(|c| *c /= total);
        }

        counts
    }
}

impl Router for LearnedRouter {
    fn route(&self, x: &[f32]) -> Vec<(usize, f32)> {
        let probs = self.compute_gate(x);

        // Get top-k indices
        let mut indexed: Vec<(usize, f32)> = probs.into_iter().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k and renormalize
        let top_k: Vec<(usize, f32)> = indexed.into_iter().take(self.top_k).collect();
        let sum: f32 = top_k.iter().map(|(_, p)| p).sum();

        if sum > 1e-8 {
            top_k.into_iter().map(|(i, p)| (i, p / sum)).collect()
        } else {
            // Fallback: uniform over top-k
            top_k
                .into_iter()
                .map(|(i, _)| (i, 1.0 / self.top_k as f32))
                .collect()
        }
    }

    fn num_experts(&self) -> usize {
        self.num_experts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learned_router() {
        let router = LearnedRouter::new(4, 64, 2);

        let x = vec![0.5; 64];
        let routes = router.route(&x);

        assert_eq!(routes.len(), 2);

        // Weights should sum to 1
        let sum: f32 = routes.iter().map(|(_, w)| w).sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_load_balance_loss() {
        let router = LearnedRouter::new(4, 32, 2);

        // Simulate routing decisions
        let decisions: Vec<TopKRouting> = (0..100)
            .map(|i| TopKRouting {
                selections: vec![(i % 4, 0.6), ((i + 1) % 4, 0.4)],
            })
            .collect();

        let loss = router.load_balance_loss(&decisions);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_expert_statistics() {
        let router = LearnedRouter::new(4, 32, 2);

        let decisions: Vec<TopKRouting> = vec![
            TopKRouting {
                selections: vec![(0, 0.6), (1, 0.4)],
            },
            TopKRouting {
                selections: vec![(0, 0.5), (2, 0.5)],
            },
        ];

        let stats = router.expert_statistics(&decisions);
        assert_eq!(stats.len(), 4);

        // Should sum to 1
        let sum: f32 = stats.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }
}
