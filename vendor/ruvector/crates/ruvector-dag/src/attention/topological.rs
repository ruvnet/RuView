//! Topological Attention: Respects DAG ordering with depth-based decay

use super::{AttentionError, AttentionScores, DagAttention};
use crate::dag::QueryDag;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TopologicalConfig {
    pub decay_factor: f32, // 0.9 default
    pub max_depth: usize,  // 10 default
}

impl Default for TopologicalConfig {
    fn default() -> Self {
        Self {
            decay_factor: 0.9,
            max_depth: 10,
        }
    }
}

pub struct TopologicalAttention {
    config: TopologicalConfig,
}

impl TopologicalAttention {
    pub fn new(config: TopologicalConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(TopologicalConfig::default())
    }
}

impl DagAttention for TopologicalAttention {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
        if dag.node_count() == 0 {
            return Err(AttentionError::EmptyDag);
        }

        let depths = dag.compute_depths();
        let max_depth = depths.values().max().copied().unwrap_or(0);

        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        for (&node_id, &depth) in &depths {
            // Higher attention for nodes closer to root (higher depth from leaves)
            let normalized_depth = depth as f32 / (max_depth.max(1) as f32);
            let score = self.config.decay_factor.powf(1.0 - normalized_depth);
            scores.insert(node_id, score);
            total += score;
        }

        // Normalize to sum to 1
        if total > 0.0 {
            for score in scores.values_mut() {
                *score /= total;
            }
        }

        Ok(scores)
    }

    fn update(&mut self, _dag: &QueryDag, _times: &HashMap<usize, f64>) {
        // Topological attention is static, no updates needed
    }

    fn name(&self) -> &'static str {
        "topological"
    }

    fn complexity(&self) -> &'static str {
        "O(n)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};

    #[test]
    fn test_topological_attention() {
        let mut dag = QueryDag::new();

        // Create a simple DAG: 0 -> 1 -> 2
        let id0 = dag.add_node(OperatorNode::seq_scan(0, "users").with_estimates(100.0, 1.0));
        let id1 = dag.add_node(OperatorNode::filter(0, "age > 18").with_estimates(50.0, 1.0));
        let id2 = dag
            .add_node(OperatorNode::project(0, vec!["name".to_string()]).with_estimates(50.0, 1.0));

        dag.add_edge(id0, id1).unwrap();
        dag.add_edge(id1, id2).unwrap();

        let attention = TopologicalAttention::with_defaults();
        let scores = attention.forward(&dag).unwrap();

        // Check that scores sum to ~1.0
        let sum: f32 = scores.values().sum();
        assert!((sum - 1.0).abs() < 1e-5);

        // All scores should be in [0, 1]
        for &score in scores.values() {
            assert!(score >= 0.0 && score <= 1.0);
        }
    }
}
