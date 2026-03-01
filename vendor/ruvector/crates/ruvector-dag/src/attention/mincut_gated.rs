//! MinCut Gated Attention: Gates attention by graph cut criticality

use super::trait_def::{AttentionError, AttentionScores, DagAttentionMechanism};
use crate::dag::QueryDag;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub enum FlowCapacity {
    UnitCapacity,
    CostBased,
    RowBased,
}

#[derive(Debug, Clone)]
pub struct MinCutConfig {
    pub gate_threshold: f32,
    pub flow_capacity: FlowCapacity,
}

impl Default for MinCutConfig {
    fn default() -> Self {
        Self {
            gate_threshold: 0.5,
            flow_capacity: FlowCapacity::UnitCapacity,
        }
    }
}

pub struct MinCutGatedAttention {
    config: MinCutConfig,
}

impl MinCutGatedAttention {
    pub fn new(config: MinCutConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(MinCutConfig::default())
    }

    /// Compute min-cut between leaves and root using Ford-Fulkerson
    fn compute_min_cut(&self, dag: &QueryDag) -> HashSet<usize> {
        let mut cut_nodes = HashSet::new();

        // Build capacity matrix from the DAG structure
        let mut capacity: HashMap<(usize, usize), f64> = HashMap::new();
        for node_id in 0..dag.node_count() {
            if dag.get_node(node_id).is_none() {
                continue;
            }
            for &child in dag.children(node_id) {
                let cap = match self.config.flow_capacity {
                    FlowCapacity::UnitCapacity => 1.0,
                    FlowCapacity::CostBased => dag
                        .get_node(node_id)
                        .map(|n| n.estimated_cost)
                        .unwrap_or(1.0),
                    FlowCapacity::RowBased => dag
                        .get_node(node_id)
                        .map(|n| n.estimated_rows)
                        .unwrap_or(1.0),
                };
                capacity.insert((node_id, child), cap);
            }
        }

        // Find source (root) and sink (any leaf)
        let source = match dag.root() {
            Some(root) => root,
            None => return cut_nodes,
        };

        let leaves = dag.leaves();
        if leaves.is_empty() {
            return cut_nodes;
        }

        // Use first leaf as sink
        let sink = leaves[0];

        // Ford-Fulkerson to find max flow
        let mut residual = capacity.clone();
        #[allow(unused_variables, unused_assignments)]
        let mut total_flow = 0.0;

        loop {
            // BFS to find augmenting path
            let mut parent: HashMap<usize, usize> = HashMap::new();
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();

            queue.push_back(source);
            visited.insert(source);

            while let Some(u) = queue.pop_front() {
                if u == sink {
                    break;
                }

                for v in dag.children(u) {
                    if !visited.contains(v) && residual.get(&(u, *v)).copied().unwrap_or(0.0) > 0.0
                    {
                        visited.insert(*v);
                        parent.insert(*v, u);
                        queue.push_back(*v);
                    }
                }
            }

            // No augmenting path found
            if !parent.contains_key(&sink) {
                break;
            }

            // Find minimum capacity along the path
            let mut path_flow = f64::INFINITY;
            let mut v = sink;
            while v != source {
                let u = parent[&v];
                path_flow = path_flow.min(residual.get(&(u, v)).copied().unwrap_or(0.0));
                v = u;
            }

            // Update residual capacities
            v = sink;
            while v != source {
                let u = parent[&v];
                *residual.entry((u, v)).or_insert(0.0) -= path_flow;
                *residual.entry((v, u)).or_insert(0.0) += path_flow;
                v = u;
            }

            total_flow += path_flow;
        }

        // Find nodes reachable from source in residual graph
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(source);
        reachable.insert(source);

        while let Some(u) = queue.pop_front() {
            for &v in dag.children(u) {
                if !reachable.contains(&v) && residual.get(&(u, v)).copied().unwrap_or(0.0) > 0.0 {
                    reachable.insert(v);
                    queue.push_back(v);
                }
            }
        }

        // Nodes in the cut are those with edges crossing from reachable to non-reachable
        for node_id in 0..dag.node_count() {
            if dag.get_node(node_id).is_none() {
                continue;
            }
            for &child in dag.children(node_id) {
                if reachable.contains(&node_id) && !reachable.contains(&child) {
                    cut_nodes.insert(node_id);
                    cut_nodes.insert(child);
                }
            }
        }

        cut_nodes
    }
}

impl DagAttentionMechanism for MinCutGatedAttention {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
        if dag.node_count() == 0 {
            return Err(AttentionError::InvalidDag("Empty DAG".to_string()));
        }

        let cut_nodes = self.compute_min_cut(dag);
        let n = dag.node_count();
        let mut score_vec = vec![0.0; n];
        let mut total = 0.0f32;

        // Gate attention based on whether node is in cut
        for node_id in 0..n {
            if dag.get_node(node_id).is_none() {
                continue;
            }

            let is_in_cut = cut_nodes.contains(&node_id);

            let score = if is_in_cut {
                // Nodes in the cut are critical bottlenecks
                1.0
            } else {
                // Other nodes get reduced attention
                self.config.gate_threshold
            };

            score_vec[node_id] = score;
            total += score;
        }

        // Normalize to sum to 1
        if total > 0.0 {
            for score in score_vec.iter_mut() {
                *score /= total;
            }
        }

        Ok(AttentionScores::new(score_vec))
    }

    fn name(&self) -> &'static str {
        "mincut_gated"
    }

    fn complexity(&self) -> &'static str {
        "O(n * e^2)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};

    #[test]
    fn test_mincut_gated_attention() {
        let mut dag = QueryDag::new();

        // Create a simple bottleneck DAG
        let id0 = dag.add_node(OperatorNode::seq_scan(0, "table1"));
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "table2"));
        let id2 = dag.add_node(OperatorNode::hash_join(0, "id"));
        let id3 = dag.add_node(OperatorNode::filter(0, "status = 'active'"));
        let id4 = dag.add_node(OperatorNode::project(0, vec!["name".to_string()]));

        // Create bottleneck at node id2
        dag.add_edge(id0, id2).unwrap();
        dag.add_edge(id1, id2).unwrap();
        dag.add_edge(id2, id3).unwrap();
        dag.add_edge(id2, id4).unwrap();

        let attention = MinCutGatedAttention::with_defaults();
        let scores = attention.forward(&dag).unwrap();

        // Check normalization
        let sum: f32 = scores.scores.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);

        // All scores should be in [0, 1]
        for &score in &scores.scores {
            assert!(score >= 0.0 && score <= 1.0);
        }
    }
}
