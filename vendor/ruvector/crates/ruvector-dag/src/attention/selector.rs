//! Attention Selector: UCB Bandit for mechanism selection
//!
//! Implements Upper Confidence Bound (UCB1) algorithm to dynamically select
//! the best attention mechanism based on observed performance.

use super::trait_def::{AttentionError, AttentionScores, DagAttentionMechanism};
use crate::dag::QueryDag;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SelectorConfig {
    /// UCB exploration constant (typically sqrt(2))
    pub exploration_factor: f32,
    /// Optimistic initialization value
    pub initial_value: f32,
    /// Minimum samples before exploitation
    pub min_samples: usize,
}

impl Default for SelectorConfig {
    fn default() -> Self {
        Self {
            exploration_factor: (2.0_f32).sqrt(),
            initial_value: 1.0,
            min_samples: 5,
        }
    }
}

pub struct AttentionSelector {
    config: SelectorConfig,
    mechanisms: Vec<Box<dyn DagAttentionMechanism>>,
    /// Cumulative rewards for each mechanism
    rewards: Vec<f32>,
    /// Number of times each mechanism was selected
    counts: Vec<usize>,
    /// Total number of selections
    total_count: usize,
}

impl AttentionSelector {
    pub fn new(mechanisms: Vec<Box<dyn DagAttentionMechanism>>, config: SelectorConfig) -> Self {
        let n = mechanisms.len();
        let initial_value = config.initial_value;
        Self {
            config,
            mechanisms,
            rewards: vec![initial_value; n],
            counts: vec![0; n],
            total_count: 0,
        }
    }

    /// Select mechanism using UCB1 algorithm
    pub fn select(&self) -> usize {
        if self.mechanisms.is_empty() {
            return 0;
        }

        // If any mechanism hasn't been tried min_samples times, try it
        for (i, &count) in self.counts.iter().enumerate() {
            if count < self.config.min_samples {
                return i;
            }
        }

        // UCB1 selection: exploitation + exploration
        let ln_total = (self.total_count as f32).ln().max(1.0);

        let ucb_values: Vec<f32> = self
            .mechanisms
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let count = self.counts[i] as f32;
                if count == 0.0 {
                    return f32::INFINITY;
                }

                let exploitation = self.rewards[i] / count;
                let exploration = self.config.exploration_factor * (ln_total / count).sqrt();

                exploitation + exploration
            })
            .collect();

        ucb_values
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Update rewards after execution
    pub fn update(&mut self, mechanism_idx: usize, reward: f32) {
        if mechanism_idx < self.rewards.len() {
            self.rewards[mechanism_idx] += reward;
            self.counts[mechanism_idx] += 1;
            self.total_count += 1;
        }
    }

    /// Get the selected mechanism
    pub fn get_mechanism(&self, idx: usize) -> Option<&dyn DagAttentionMechanism> {
        self.mechanisms.get(idx).map(|m| m.as_ref())
    }

    /// Get mutable reference to mechanism for updates
    pub fn get_mechanism_mut(&mut self, idx: usize) -> Option<&mut Box<dyn DagAttentionMechanism>> {
        self.mechanisms.get_mut(idx)
    }

    /// Get statistics for all mechanisms
    pub fn stats(&self) -> HashMap<&'static str, MechanismStats> {
        self.mechanisms
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let stats = MechanismStats {
                    total_reward: self.rewards[i],
                    count: self.counts[i],
                    avg_reward: if self.counts[i] > 0 {
                        self.rewards[i] / self.counts[i] as f32
                    } else {
                        0.0
                    },
                };
                (m.name(), stats)
            })
            .collect()
    }

    /// Get the best performing mechanism based on average reward
    pub fn best_mechanism(&self) -> Option<usize> {
        self.mechanisms
            .iter()
            .enumerate()
            .filter(|(i, _)| self.counts[*i] >= self.config.min_samples)
            .max_by(|(i, _), (j, _)| {
                let avg_i = self.rewards[*i] / self.counts[*i] as f32;
                let avg_j = self.rewards[*j] / self.counts[*j] as f32;
                avg_i
                    .partial_cmp(&avg_j)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        for i in 0..self.rewards.len() {
            self.rewards[i] = self.config.initial_value;
            self.counts[i] = 0;
        }
        self.total_count = 0;
    }

    /// Forward pass using selected mechanism
    pub fn forward(&mut self, dag: &QueryDag) -> Result<(AttentionScores, usize), AttentionError> {
        let selected = self.select();
        let mechanism = self
            .get_mechanism(selected)
            .ok_or_else(|| AttentionError::ConfigError("No mechanisms available".to_string()))?;

        let scores = mechanism.forward(dag)?;
        Ok((scores, selected))
    }
}

#[derive(Debug, Clone)]
pub struct MechanismStats {
    pub total_reward: f32,
    pub count: usize,
    pub avg_reward: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType, QueryDag};

    // Mock mechanism for testing
    struct MockMechanism {
        name: &'static str,
        score_value: f32,
    }

    impl DagAttentionMechanism for MockMechanism {
        fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
            let scores = vec![self.score_value; dag.nodes.len()];
            Ok(AttentionScores::new(scores))
        }

        fn name(&self) -> &'static str {
            self.name
        }

        fn complexity(&self) -> &'static str {
            "O(1)"
        }
    }

    #[test]
    fn test_ucb_selection() {
        let mechanisms: Vec<Box<dyn DagAttentionMechanism>> = vec![
            Box::new(MockMechanism {
                name: "mech1",
                score_value: 0.5,
            }),
            Box::new(MockMechanism {
                name: "mech2",
                score_value: 0.7,
            }),
            Box::new(MockMechanism {
                name: "mech3",
                score_value: 0.3,
            }),
        ];

        let mut selector = AttentionSelector::new(mechanisms, SelectorConfig::default());

        // First selections should explore all mechanisms
        for _ in 0..15 {
            let selected = selector.select();
            selector.update(selected, 0.5);
        }

        assert!(selector.total_count > 0);
        assert!(selector.counts.iter().all(|&c| c > 0));
    }

    #[test]
    fn test_best_mechanism() {
        let mechanisms: Vec<Box<dyn DagAttentionMechanism>> = vec![
            Box::new(MockMechanism {
                name: "poor",
                score_value: 0.3,
            }),
            Box::new(MockMechanism {
                name: "good",
                score_value: 0.8,
            }),
        ];

        let mut selector = AttentionSelector::new(
            mechanisms,
            SelectorConfig {
                min_samples: 2,
                ..Default::default()
            },
        );

        // Simulate different rewards
        selector.update(0, 0.3);
        selector.update(0, 0.4);
        selector.update(1, 0.8);
        selector.update(1, 0.9);

        let best = selector.best_mechanism().unwrap();
        assert_eq!(best, 1);
    }

    #[test]
    fn test_selector_forward() {
        let mechanisms: Vec<Box<dyn DagAttentionMechanism>> = vec![Box::new(MockMechanism {
            name: "test",
            score_value: 0.5,
        })];

        let mut selector = AttentionSelector::new(mechanisms, SelectorConfig::default());

        let mut dag = QueryDag::new();
        let node = OperatorNode::new(0, OperatorType::Scan);
        dag.add_node(node);

        let (scores, idx) = selector.forward(&dag).unwrap();
        assert_eq!(scores.scores.len(), 1);
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_stats() {
        let mechanisms: Vec<Box<dyn DagAttentionMechanism>> = vec![Box::new(MockMechanism {
            name: "mech1",
            score_value: 0.5,
        })];

        // Use initial_value = 0 so we can test pure update accumulation
        let config = SelectorConfig {
            initial_value: 0.0,
            ..Default::default()
        };
        let mut selector = AttentionSelector::new(mechanisms, config);
        selector.update(0, 1.0);
        selector.update(0, 2.0);

        let stats = selector.stats();
        let mech1_stats = stats.get("mech1").unwrap();

        assert_eq!(mech1_stats.count, 2);
        assert_eq!(mech1_stats.total_reward, 3.0);
        assert_eq!(mech1_stats.avg_reward, 1.5);
    }
}
