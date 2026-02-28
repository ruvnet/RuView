//! Causal Cone Attention: Focuses on ancestors with temporal discount

use super::{AttentionError, AttentionScores, DagAttention};
use crate::dag::QueryDag;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CausalConeConfig {
    pub time_window_ms: u64,
    pub future_discount: f32,
    pub ancestor_weight: f32,
}

impl Default for CausalConeConfig {
    fn default() -> Self {
        Self {
            time_window_ms: 1000,
            future_discount: 0.8,
            ancestor_weight: 0.9,
        }
    }
}

pub struct CausalConeAttention {
    config: CausalConeConfig,
}

impl CausalConeAttention {
    pub fn new(config: CausalConeConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(CausalConeConfig::default())
    }
}

impl DagAttention for CausalConeAttention {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
        if dag.node_count() == 0 {
            return Err(AttentionError::EmptyDag);
        }

        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        // For each node, compute attention based on:
        // 1. Number of ancestors (causal influence)
        // 2. Distance from node (temporal decay)
        let node_ids: Vec<usize> = (0..dag.node_count()).collect();
        for node_id in node_ids {
            if dag.get_node(node_id).is_none() {
                continue;
            }

            let ancestors = dag.ancestors(node_id);
            let ancestor_count = ancestors.len();

            // Base score is proportional to causal influence (number of ancestors)
            let mut score = 1.0 + (ancestor_count as f32 * self.config.ancestor_weight);

            // Apply temporal discount based on depth
            let depths = dag.compute_depths();
            if let Some(&depth) = depths.get(&node_id) {
                score *= self.config.future_discount.powi(depth as i32);
            }

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
        // Could update temporal discount based on actual execution times
        // For now, static configuration
    }

    fn name(&self) -> &'static str {
        "causal_cone"
    }

    fn complexity(&self) -> &'static str {
        "O(n^2)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};

    #[test]
    fn test_causal_cone_attention() {
        let mut dag = QueryDag::new();

        // Create a DAG with multiple paths
        let id0 = dag.add_node(OperatorNode::seq_scan(0, "table1"));
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "table2"));
        let id2 = dag.add_node(OperatorNode::hash_join(0, "id"));
        let id3 = dag.add_node(OperatorNode::project(0, vec!["name".to_string()]));

        dag.add_edge(id0, id2).unwrap();
        dag.add_edge(id1, id2).unwrap();
        dag.add_edge(id2, id3).unwrap();

        let attention = CausalConeAttention::with_defaults();
        let scores = attention.forward(&dag).unwrap();

        // Check normalization
        let sum: f32 = scores.values().sum();
        assert!((sum - 1.0).abs() < 1e-5);

        // All scores should be in [0, 1]
        for &score in scores.values() {
            assert!(score >= 0.0 && score <= 1.0);
        }
    }
}
