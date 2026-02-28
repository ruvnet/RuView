//! Causal graph for tracking antecedent relationships

use crate::types::{PatternId, SubstrateTime};
use dashmap::DashMap;
use petgraph::algo::dijkstra;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Type of causal cone for queries
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CausalConeType {
    /// Past light cone (all events that could have influenced reference)
    Past,
    /// Future light cone (all events that reference could influence)
    Future,
    /// Relativistic light cone with velocity constraint
    LightCone {
        /// Velocity of causal influence (fraction of c)
        velocity: f32,
    },
}

/// Causal graph tracking antecedent relationships
pub struct CausalGraph {
    /// Forward edges: cause -> effects
    forward: DashMap<PatternId, Vec<PatternId>>,
    /// Backward edges: effect -> causes
    backward: DashMap<PatternId, Vec<PatternId>>,
    /// Pattern timestamps for light cone calculations
    timestamps: DashMap<PatternId, SubstrateTime>,
    /// Cached graph representation for path finding
    graph_cache:
        Arc<parking_lot::RwLock<Option<(DiGraph<PatternId, ()>, HashMap<PatternId, NodeIndex>)>>>,
}

impl CausalGraph {
    /// Create new causal graph
    pub fn new() -> Self {
        Self {
            forward: DashMap::new(),
            backward: DashMap::new(),
            timestamps: DashMap::new(),
            graph_cache: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Add causal edge: cause -> effect
    pub fn add_edge(&self, cause: PatternId, effect: PatternId) {
        // Add to forward edges
        self.forward
            .entry(cause)
            .or_insert_with(Vec::new)
            .push(effect);

        // Add to backward edges
        self.backward
            .entry(effect)
            .or_insert_with(Vec::new)
            .push(cause);

        // Invalidate cache
        *self.graph_cache.write() = None;
    }

    /// Add pattern with timestamp
    pub fn add_pattern(&self, id: PatternId, timestamp: SubstrateTime) {
        self.timestamps.insert(id, timestamp);
    }

    /// Get direct causes of a pattern
    pub fn causes(&self, pattern: PatternId) -> Vec<PatternId> {
        self.backward
            .get(&pattern)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Get direct effects of a pattern
    pub fn effects(&self, pattern: PatternId) -> Vec<PatternId> {
        self.forward
            .get(&pattern)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Get out-degree (number of effects)
    pub fn out_degree(&self, pattern: PatternId) -> usize {
        self.forward.get(&pattern).map(|v| v.len()).unwrap_or(0)
    }

    /// Get in-degree (number of causes)
    pub fn in_degree(&self, pattern: PatternId) -> usize {
        self.backward.get(&pattern).map(|v| v.len()).unwrap_or(0)
    }

    /// Compute shortest path distance between two patterns
    pub fn distance(&self, from: PatternId, to: PatternId) -> Option<usize> {
        if from == to {
            return Some(0);
        }

        // Build or retrieve cached graph
        let (graph, node_map) = {
            let cache = self.graph_cache.read();
            if let Some((g, m)) = cache.as_ref() {
                (g.clone(), m.clone())
            } else {
                drop(cache);
                let (g, m) = self.build_graph();
                *self.graph_cache.write() = Some((g.clone(), m.clone()));
                (g, m)
            }
        };

        // Get node indices
        let from_idx = *node_map.get(&from)?;
        let to_idx = *node_map.get(&to)?;

        // Run Dijkstra's algorithm
        let distances = dijkstra(&graph, from_idx, Some(to_idx), |_| 1);

        distances.get(&to_idx).copied()
    }

    /// Build petgraph representation for path finding
    fn build_graph(&self) -> (DiGraph<PatternId, ()>, HashMap<PatternId, NodeIndex>) {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        // Add all nodes
        for entry in self.forward.iter() {
            let id = *entry.key();
            if !node_map.contains_key(&id) {
                let idx = graph.add_node(id);
                node_map.insert(id, idx);
            }

            for &effect in entry.value() {
                if !node_map.contains_key(&effect) {
                    let idx = graph.add_node(effect);
                    node_map.insert(effect, idx);
                }
            }
        }

        // Add edges
        for entry in self.forward.iter() {
            let from = *entry.key();
            let from_idx = node_map[&from];

            for &to in entry.value() {
                let to_idx = node_map[&to];
                graph.add_edge(from_idx, to_idx, ());
            }
        }

        (graph, node_map)
    }

    /// Get all patterns in causal past
    pub fn causal_past(&self, pattern: PatternId) -> Vec<PatternId> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![pattern];

        while let Some(current) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            if let Some(causes) = self.backward.get(&current) {
                for &cause in causes.iter() {
                    if !visited.contains(&cause) {
                        stack.push(cause);
                        result.push(cause);
                    }
                }
            }
        }

        result
    }

    /// Get all patterns in causal future
    pub fn causal_future(&self, pattern: PatternId) -> Vec<PatternId> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![pattern];

        while let Some(current) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            if let Some(effects) = self.forward.get(&current) {
                for &effect in effects.iter() {
                    if !visited.contains(&effect) {
                        stack.push(effect);
                        result.push(effect);
                    }
                }
            }
        }

        result
    }

    /// Filter patterns by light cone constraint
    pub fn filter_by_light_cone(
        &self,
        reference: PatternId,
        reference_time: SubstrateTime,
        cone_type: CausalConeType,
        candidates: &[PatternId],
    ) -> Vec<PatternId> {
        candidates
            .iter()
            .filter(|&&id| self.is_in_light_cone(id, reference, reference_time, cone_type))
            .copied()
            .collect()
    }

    /// Check if pattern is within light cone
    fn is_in_light_cone(
        &self,
        pattern: PatternId,
        _reference: PatternId,
        reference_time: SubstrateTime,
        cone_type: CausalConeType,
    ) -> bool {
        let pattern_time = match self.timestamps.get(&pattern) {
            Some(t) => *t,
            None => return false,
        };

        match cone_type {
            CausalConeType::Past => pattern_time <= reference_time,
            CausalConeType::Future => pattern_time >= reference_time,
            CausalConeType::LightCone { velocity: _ } => {
                // Simplified relativistic constraint
                // In full implementation, would include spatial distance
                let time_diff = (reference_time - pattern_time).abs();
                let time_diff_secs = (time_diff.0 / 1_000_000_000).abs() as f32;

                // For now, just use temporal constraint
                // In full version: spatial_distance <= velocity * time_diff
                time_diff_secs >= 0.0 // Always true for temporal-only check
            }
        }
    }

    /// Get statistics about the causal graph
    pub fn stats(&self) -> CausalGraphStats {
        let num_nodes = self.timestamps.len();
        let num_edges: usize = self.forward.iter().map(|e| e.value().len()).sum();

        let avg_out_degree = if num_nodes > 0 {
            num_edges as f32 / num_nodes as f32
        } else {
            0.0
        };

        CausalGraphStats {
            num_nodes,
            num_edges,
            avg_out_degree,
        }
    }
}

impl Default for CausalGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about causal graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalGraphStats {
    /// Number of nodes
    pub num_nodes: usize,
    /// Number of edges
    pub num_edges: usize,
    /// Average out-degree
    pub avg_out_degree: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_graph_basic() {
        let graph = CausalGraph::new();

        let p1 = PatternId::new();
        let p2 = PatternId::new();
        let p3 = PatternId::new();

        let t1 = SubstrateTime::now();
        let t2 = SubstrateTime::now();
        let t3 = SubstrateTime::now();

        graph.add_pattern(p1, t1);
        graph.add_pattern(p2, t2);
        graph.add_pattern(p3, t3);

        // p1 -> p2 -> p3
        graph.add_edge(p1, p2);
        graph.add_edge(p2, p3);

        assert_eq!(graph.out_degree(p1), 1);
        assert_eq!(graph.in_degree(p2), 1);
        assert_eq!(graph.distance(p1, p3), Some(2));

        let past = graph.causal_past(p3);
        assert!(past.contains(&p1));
        assert!(past.contains(&p2));
    }
}
