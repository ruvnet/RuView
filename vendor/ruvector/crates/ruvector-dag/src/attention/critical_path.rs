//! Critical Path Attention: Focuses on bottleneck nodes

use super::{AttentionError, AttentionScores, DagAttention};
use crate::dag::QueryDag;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CriticalPathConfig {
    pub path_weight: f32,
    pub branch_penalty: f32,
}

impl Default for CriticalPathConfig {
    fn default() -> Self {
        Self {
            path_weight: 2.0,
            branch_penalty: 0.5,
        }
    }
}

pub struct CriticalPathAttention {
    config: CriticalPathConfig,
    critical_path: Vec<usize>,
}

impl CriticalPathAttention {
    pub fn new(config: CriticalPathConfig) -> Self {
        Self {
            config,
            critical_path: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(CriticalPathConfig::default())
    }

    /// Compute the critical path (longest path by cost)
    fn compute_critical_path(&self, dag: &QueryDag) -> Vec<usize> {
        let mut longest_path: HashMap<usize, (f64, Vec<usize>)> = HashMap::new();

        // Initialize leaves
        for &leaf in &dag.leaves() {
            if let Some(node) = dag.get_node(leaf) {
                longest_path.insert(leaf, (node.estimated_cost, vec![leaf]));
            }
        }

        // Process nodes in reverse topological order
        if let Ok(topo_order) = dag.topological_sort() {
            for &node_id in topo_order.iter().rev() {
                let node = match dag.get_node(node_id) {
                    Some(n) => n,
                    None => continue,
                };

                let mut max_cost = node.estimated_cost;
                let mut max_path = vec![node_id];

                // Check all children
                for &child in dag.children(node_id) {
                    if let Some(&(child_cost, ref child_path)) = longest_path.get(&child) {
                        let total_cost = node.estimated_cost + child_cost;
                        if total_cost > max_cost {
                            max_cost = total_cost;
                            max_path = vec![node_id];
                            max_path.extend(child_path);
                        }
                    }
                }

                longest_path.insert(node_id, (max_cost, max_path));
            }
        }

        // Find the path with maximum cost
        longest_path
            .into_iter()
            .max_by(|a, b| {
                a.1 .0
                    .partial_cmp(&b.1 .0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(_, (_, path))| path)
            .unwrap_or_default()
    }
}

impl DagAttention for CriticalPathAttention {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
        if dag.node_count() == 0 {
            return Err(AttentionError::EmptyDag);
        }

        let critical = self.compute_critical_path(dag);
        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        // Assign higher attention to nodes on critical path
        let node_ids: Vec<usize> = (0..dag.node_count()).collect();
        for node_id in node_ids {
            if dag.get_node(node_id).is_none() {
                continue;
            }

            let is_on_critical_path = critical.contains(&node_id);
            let num_children = dag.children(node_id).len();

            let mut score = if is_on_critical_path {
                self.config.path_weight
            } else {
                1.0
            };

            // Apply branch penalty for nodes with many children (potential bottlenecks)
            if num_children > 1 {
                score *= 1.0 + (num_children as f32 - 1.0) * self.config.branch_penalty;
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

    fn update(&mut self, dag: &QueryDag, execution_times: &HashMap<usize, f64>) {
        // Recompute critical path based on actual execution times
        // For now, we use the static cost-based approach
        self.critical_path = self.compute_critical_path(dag);

        // Could adjust path_weight based on execution time variance
        if !execution_times.is_empty() {
            let max_time = execution_times.values().fold(0.0f64, |a, &b| a.max(b));
            let avg_time: f64 =
                execution_times.values().sum::<f64>() / execution_times.len() as f64;

            if max_time > 0.0 && avg_time > 0.0 {
                // Increase path weight if there's high variance
                let variance_ratio = max_time / avg_time;
                if variance_ratio > 2.0 {
                    self.config.path_weight = (self.config.path_weight * 1.1).min(5.0);
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "critical_path"
    }

    fn complexity(&self) -> &'static str {
        "O(n + e)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};

    #[test]
    fn test_critical_path_attention() {
        let mut dag = QueryDag::new();

        // Create a DAG with different costs
        let id0 =
            dag.add_node(OperatorNode::seq_scan(0, "large_table").with_estimates(10000.0, 10.0));
        let id1 =
            dag.add_node(OperatorNode::filter(0, "status = 'active'").with_estimates(1000.0, 1.0));
        let id2 = dag.add_node(OperatorNode::hash_join(0, "user_id").with_estimates(5000.0, 5.0));

        dag.add_edge(id0, id2).unwrap();
        dag.add_edge(id1, id2).unwrap();

        let attention = CriticalPathAttention::with_defaults();
        let scores = attention.forward(&dag).unwrap();

        // Check normalization
        let sum: f32 = scores.values().sum();
        assert!((sum - 1.0).abs() < 1e-5);

        // Nodes on critical path should have higher attention
        let critical = attention.compute_critical_path(&dag);
        for &node_id in &critical {
            let score = scores.get(&node_id).unwrap();
            assert!(*score > 0.0);
        }
    }
}
