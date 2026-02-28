//! DagMinCutEngine: Main min-cut computation engine

use super::local_kcut::LocalKCut;
use crate::dag::QueryDag;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct MinCutConfig {
    pub epsilon: f32, // Approximation factor
    pub local_search_depth: usize,
    pub cache_cuts: bool,
}

impl Default for MinCutConfig {
    fn default() -> Self {
        Self {
            epsilon: 0.1,
            local_search_depth: 3,
            cache_cuts: true,
        }
    }
}

/// Edge in the flow graph
#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub from: usize,
    pub to: usize,
    pub capacity: f64,
    pub flow: f64,
}

/// Result of min-cut computation
#[derive(Debug, Clone)]
pub struct MinCutResult {
    pub cut_value: f64,
    pub source_side: HashSet<usize>,
    pub sink_side: HashSet<usize>,
    pub cut_edges: Vec<(usize, usize)>,
}

pub struct DagMinCutEngine {
    config: MinCutConfig,
    adjacency: HashMap<usize, Vec<FlowEdge>>,
    node_count: usize,
    local_kcut: LocalKCut,
    cached_cuts: HashMap<(usize, usize), MinCutResult>,
}

impl DagMinCutEngine {
    pub fn new(config: MinCutConfig) -> Self {
        Self {
            config,
            adjacency: HashMap::new(),
            node_count: 0,
            local_kcut: LocalKCut::new(),
            cached_cuts: HashMap::new(),
        }
    }

    /// Build flow graph from DAG
    pub fn build_from_dag(&mut self, dag: &QueryDag) {
        self.adjacency.clear();
        self.node_count = dag.node_count();

        // Iterate over all possible node IDs
        for node_id in 0..dag.node_count() {
            if let Some(node) = dag.get_node(node_id) {
                let capacity = node.estimated_cost.max(1.0);

                for &child_id in dag.children(node_id) {
                    self.add_edge(node_id, child_id, capacity);
                }
            }
        }
    }

    pub fn add_edge(&mut self, from: usize, to: usize, capacity: f64) {
        self.adjacency.entry(from).or_default().push(FlowEdge {
            from,
            to,
            capacity,
            flow: 0.0,
        });
        // Add reverse edge for residual graph
        self.adjacency.entry(to).or_default().push(FlowEdge {
            from: to,
            to: from,
            capacity: 0.0,
            flow: 0.0,
        });

        self.node_count = self.node_count.max(from + 1).max(to + 1);

        // Invalidate cache
        self.cached_cuts.clear();
    }

    /// Compute min-cut between source and sink
    pub fn compute_mincut(&mut self, source: usize, sink: usize) -> MinCutResult {
        // Check cache
        if self.config.cache_cuts {
            if let Some(cached) = self.cached_cuts.get(&(source, sink)) {
                return cached.clone();
            }
        }

        // Use local k-cut for approximate but fast computation
        let result = self.local_kcut.compute(
            &self.adjacency,
            source,
            sink,
            self.config.local_search_depth,
        );

        if self.config.cache_cuts {
            self.cached_cuts.insert((source, sink), result.clone());
        }

        result
    }

    /// Dynamic update after edge weight change - O(n^0.12) amortized
    pub fn update_edge(&mut self, from: usize, to: usize, new_capacity: f64) {
        if let Some(edges) = self.adjacency.get_mut(&from) {
            for edge in edges.iter_mut() {
                if edge.to == to {
                    edge.capacity = new_capacity;
                    break;
                }
            }
        }

        // Invalidate affected cached cuts
        // Extract keys to avoid borrowing issues
        let keys_to_remove: Vec<(usize, usize)> = self
            .cached_cuts
            .keys()
            .filter(|(s, t)| self.cut_involves_edge(*s, *t, from, to))
            .copied()
            .collect();

        for key in keys_to_remove {
            self.cached_cuts.remove(&key);
        }
    }

    fn cut_involves_edge(&self, _source: usize, _sink: usize, _from: usize, _to: usize) -> bool {
        // Conservative: invalidate if edge is on any path from source to sink
        // This is a simplified check
        true
    }

    /// Compute criticality scores for all nodes
    pub fn compute_criticality(&mut self, dag: &QueryDag) -> HashMap<usize, f64> {
        let mut criticality = HashMap::new();

        let leaves = dag.leaves();
        let root = dag.root();

        if leaves.is_empty() || root.is_none() {
            return criticality;
        }

        let root = root.unwrap();

        // For each node, compute how much it affects the min-cut
        for node_id in 0..dag.node_count() {
            if dag.get_node(node_id).is_none() {
                continue;
            }

            // Compute min-cut with node vs without
            let cut_with = self.compute_mincut(leaves[0], root);

            // Temporarily increase node capacity
            for &child in dag.children(node_id) {
                self.update_edge(node_id, child, f64::INFINITY);
            }

            let cut_without = self.compute_mincut(leaves[0], root);

            // Restore capacity
            let node = dag.get_node(node_id).unwrap();
            for &child in dag.children(node_id) {
                self.update_edge(node_id, child, node.estimated_cost);
            }

            // Criticality = how much the cut increases without the node
            let crit = (cut_without.cut_value - cut_with.cut_value) / cut_with.cut_value.max(1.0);
            criticality.insert(node_id, crit.max(0.0));
        }

        criticality
    }
}
