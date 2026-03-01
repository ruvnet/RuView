//! # Layer 2: Strange Loop Self-Modification Protocol
//!
//! Implements recursive self-observation for meta-cognitive graph optimization.
//!
//! ## Hierarchical Levels
//!
//! - **Level 0**: Object Graph - computational units and data flow
//! - **Level 1**: Meta-Graph - observes Level 0 statistics
//! - **Level 2**: Meta-Meta-Graph - observes learning dynamics
//!
//! The "strange loop" closes when Level 2 actions modify Level 0 structure,
//! which changes Level 1 observations, which triggers Level 2 re-evaluation.

use super::{
    network::{LayerConfig, NetworkConfig, SpikingNetwork},
    neuron::{LIFNeuron, NeuronConfig, NeuronPopulation},
    SimTime, Spike,
};
use crate::graph::{DynamicGraph, VertexId};
use std::collections::VecDeque;

/// Configuration for strange loop system
#[derive(Debug, Clone)]
pub struct StrangeLoopConfig {
    /// Number of Level 0 neurons (matches graph vertices)
    pub level0_size: usize,
    /// Number of Level 1 observer neurons
    pub level1_size: usize,
    /// Number of Level 2 meta-neurons
    pub level2_size: usize,
    /// Time step for simulation
    pub dt: f64,
    /// Threshold for strengthen action
    pub strengthen_threshold: f64,
    /// Threshold for prune action
    pub prune_threshold: f64,
    /// Minimum mincut contribution to keep edge
    pub prune_weight_threshold: f64,
    /// History window for observations
    pub observation_window: usize,
}

impl Default for StrangeLoopConfig {
    fn default() -> Self {
        Self {
            level0_size: 100,
            level1_size: 20,
            level2_size: 5,
            dt: 1.0,
            strengthen_threshold: 0.7,
            prune_threshold: 0.3,
            prune_weight_threshold: 0.1,
            observation_window: 100,
        }
    }
}

/// Meta-level in the hierarchy
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetaLevel {
    /// Level 0: Object graph being optimized
    Object,
    /// Level 1: Observer SNN watching Level 0
    Observer,
    /// Level 2: Meta-neuron modulating observer
    Meta,
}

/// Actions that Level 2 can take to modify Level 0
#[derive(Debug, Clone)]
pub enum MetaAction {
    /// Strengthen edges where observer activity is high
    Strengthen(f64),
    /// Remove edges below mincut contribution threshold
    Prune(f64),
    /// Radical reorganization using current mincut as seed
    Restructure,
    /// No action needed
    NoOp,
}

/// Cross-level influence matrix
#[derive(Debug, Clone)]
pub struct CrossLevelInfluence {
    /// Level 0 → Level 1 influence weights
    pub l0_to_l1: Vec<Vec<f64>>,
    /// Level 1 → Level 2 influence weights
    pub l1_to_l2: Vec<Vec<f64>>,
    /// Level 2 → Level 0 influence (the strange part)
    pub l2_to_l0: Vec<Vec<f64>>,
}

/// Meta-neuron for Level 2 decisions
#[derive(Debug, Clone)]
pub struct MetaNeuron {
    /// ID of this meta-neuron
    pub id: usize,
    /// Internal state
    pub state: f64,
    /// Decision threshold
    pub threshold: f64,
    /// History of observer summaries
    history: VecDeque<f64>,
    /// Window size for decisions
    window: usize,
}

impl MetaNeuron {
    /// Create a new meta-neuron
    pub fn new(id: usize, window: usize) -> Self {
        Self {
            id,
            state: 0.0,
            threshold: 0.5,
            history: VecDeque::with_capacity(window),
            window,
        }
    }

    /// Process observer summary and produce modulation signal
    pub fn modulate(&mut self, observer_summary: f64) -> MetaAction {
        // Update history
        self.history.push_back(observer_summary);
        if self.history.len() > self.window {
            self.history.pop_front();
        }

        // Compute trend
        let mean: f64 = self.history.iter().sum::<f64>() / self.history.len() as f64;
        let recent_mean: f64 = self.history.iter().rev().take(10).sum::<f64>()
            / 10.0f64.min(self.history.len() as f64);

        self.state = recent_mean - mean;

        // Decide action based on state
        if self.state > self.threshold {
            MetaAction::Strengthen(observer_summary)
        } else if self.state < -self.threshold {
            MetaAction::Prune(observer_summary.abs())
        } else if observer_summary.abs() > 2.0 * self.threshold {
            MetaAction::Restructure
        } else {
            MetaAction::NoOp
        }
    }

    /// Reset meta-neuron state
    pub fn reset(&mut self) {
        self.state = 0.0;
        self.history.clear();
    }
}

/// Meta-Cognitive MinCut with Strange Loop
pub struct MetaCognitiveMinCut {
    /// Level 0: Object graph being optimized
    object_graph: DynamicGraph,
    /// Level 1: SNN observing object graph statistics
    observer_snn: SpikingNetwork,
    /// Level 2: Meta-neurons modulating observer behavior
    meta_neurons: Vec<MetaNeuron>,
    /// Cross-level influence matrix
    influence: CrossLevelInfluence,
    /// Configuration
    config: StrangeLoopConfig,
    /// Current simulation time
    time: SimTime,
    /// History of mincut values
    mincut_history: VecDeque<f64>,
    /// History of actions taken
    action_history: Vec<MetaAction>,
}

impl MetaCognitiveMinCut {
    /// Create a new meta-cognitive mincut system
    pub fn new(graph: DynamicGraph, config: StrangeLoopConfig) -> Self {
        let n = graph.num_vertices();

        // Level 1: Observer SNN
        let observer_config = NetworkConfig {
            layers: vec![LayerConfig::new(config.level1_size)],
            ..NetworkConfig::default()
        };
        let observer_snn = SpikingNetwork::new(observer_config);

        // Level 2: Meta-neurons
        let meta_neurons: Vec<_> = (0..config.level2_size)
            .map(|i| MetaNeuron::new(i, config.observation_window))
            .collect();

        // Initialize cross-level influence
        let influence = CrossLevelInfluence {
            l0_to_l1: vec![vec![0.1; config.level1_size]; n],
            l1_to_l2: vec![vec![0.1; config.level2_size]; config.level1_size],
            l2_to_l0: vec![vec![0.1; n]; config.level2_size],
        };

        let observation_window = config.observation_window;

        Self {
            object_graph: graph,
            observer_snn,
            meta_neurons,
            influence,
            config,
            time: 0.0,
            mincut_history: VecDeque::with_capacity(observation_window),
            action_history: Vec::new(),
        }
    }

    /// Encode graph state as spike pattern for Level 1
    fn encode_graph_state(&self) -> Vec<f64> {
        let vertices = self.object_graph.vertices();
        let mut encoding = vec![0.0; self.config.level1_size];

        for (i, v) in vertices.iter().enumerate() {
            let degree = self.object_graph.degree(*v) as f64;
            let weight_sum: f64 = self
                .object_graph
                .neighbors(*v)
                .iter()
                .filter_map(|(_, _)| Some(1.0))
                .sum();

            // Project to observer neurons
            for j in 0..encoding.len() {
                if i < self.influence.l0_to_l1.len() && j < self.influence.l0_to_l1[i].len() {
                    encoding[j] += self.influence.l0_to_l1[i][j] * (degree + weight_sum);
                }
            }
        }

        encoding
    }

    /// Get population rate as observer summary
    fn observer_summary(&self) -> f64 {
        self.observer_snn.layer_rate(0, 100.0)
    }

    /// Find high-correlation pairs in observer SNN
    fn high_correlation_pairs(&self, threshold: f64) -> Vec<(VertexId, VertexId)> {
        let sync_matrix = self.observer_snn.synchrony_matrix();
        let vertices = self.object_graph.vertices();
        let mut pairs = Vec::new();

        for i in 0..sync_matrix.len().min(vertices.len()) {
            for j in (i + 1)..sync_matrix[i].len().min(vertices.len()) {
                if sync_matrix[i][j] > threshold {
                    pairs.push((vertices[i], vertices[j]));
                }
            }
        }

        pairs
    }

    /// Compute mincut contribution for each edge (simplified)
    fn mincut_contribution(&self, edge: &crate::graph::Edge) -> f64 {
        // Simplified: degree-based contribution
        let src_degree = self.object_graph.degree(edge.source) as f64;
        let tgt_degree = self.object_graph.degree(edge.target) as f64;

        edge.weight / (src_degree + tgt_degree).max(1.0)
    }

    /// Rebuild graph from partition (simplified)
    fn rebuild_from_partition(&mut self, vertices: &[VertexId]) {
        // Keep only edges within the partition
        let vertex_set: std::collections::HashSet<_> = vertices.iter().collect();

        let edges_to_remove: Vec<_> = self
            .object_graph
            .edges()
            .iter()
            .filter(|e| !vertex_set.contains(&e.source) || !vertex_set.contains(&e.target))
            .map(|e| (e.source, e.target))
            .collect();

        for (u, v) in edges_to_remove {
            let _ = self.object_graph.delete_edge(u, v);
        }
    }

    /// Execute one strange loop iteration
    pub fn strange_loop_step(&mut self) -> MetaAction {
        // Level 0 → Level 1: Encode graph state as spike patterns
        let graph_state = self.encode_graph_state();
        self.observer_snn.inject_current(&graph_state);

        // Level 1 dynamics: Observer SNN processes graph state
        let _observer_spikes = self.observer_snn.step();

        // Level 1 → Level 2: Meta-neuron receives observer output
        let observer_summary = self.observer_summary();

        // Level 2 decision: Aggregate meta-neuron decisions
        let mut actions = Vec::new();
        for meta_neuron in &mut self.meta_neurons {
            actions.push(meta_neuron.modulate(observer_summary));
        }

        // Select dominant action (simplified: first non-NoOp)
        let action = actions
            .into_iter()
            .find(|a| !matches!(a, MetaAction::NoOp))
            .unwrap_or(MetaAction::NoOp);

        // Level 2 → Level 0: Close the strange loop
        match &action {
            MetaAction::Strengthen(threshold) => {
                // Add edges where observer activity is high
                let hot_pairs = self.high_correlation_pairs(*threshold);
                for (u, v) in hot_pairs {
                    if !self.object_graph.has_edge(u, v) {
                        let _ = self.object_graph.insert_edge(u, v, 1.0);
                    } else {
                        // Strengthen existing edge
                        if let Some(edge) = self.object_graph.get_edge(u, v) {
                            let _ = self
                                .object_graph
                                .update_edge_weight(u, v, edge.weight * 1.1);
                        }
                    }
                }
            }
            MetaAction::Prune(threshold) => {
                // Remove edges below mincut contribution threshold
                let weak_edges: Vec<_> = self
                    .object_graph
                    .edges()
                    .iter()
                    .filter(|e| self.mincut_contribution(e) < *threshold)
                    .map(|e| (e.source, e.target))
                    .collect();

                for (u, v) in weak_edges {
                    let _ = self.object_graph.delete_edge(u, v);
                }
            }
            MetaAction::Restructure => {
                // Use largest connected component
                let components = self.object_graph.connected_components();
                if let Some(largest) = components.iter().max_by_key(|c| c.len()) {
                    if largest.len() < self.object_graph.num_vertices() {
                        self.rebuild_from_partition(largest);
                    }
                }
            }
            MetaAction::NoOp => {}
        }

        self.time += self.config.dt;
        self.action_history.push(action.clone());

        action
    }

    /// Get object graph
    pub fn graph(&self) -> &DynamicGraph {
        &self.object_graph
    }

    /// Get mutable object graph
    pub fn graph_mut(&mut self) -> &mut DynamicGraph {
        &mut self.object_graph
    }

    /// Get observer SNN
    pub fn observer(&self) -> &SpikingNetwork {
        &self.observer_snn
    }

    /// Get action history
    pub fn action_history(&self) -> &[MetaAction] {
        &self.action_history
    }

    /// Get meta-level state summary
    pub fn level_summary(&self) -> (f64, f64, f64) {
        let l0 = self.object_graph.num_edges() as f64;
        let l1 = self.observer_summary();
        let l2 =
            self.meta_neurons.iter().map(|m| m.state).sum::<f64>() / self.meta_neurons.len() as f64;

        (l0, l1, l2)
    }

    /// Reset the system
    pub fn reset(&mut self) {
        self.observer_snn.reset();
        for meta in &mut self.meta_neurons {
            meta.reset();
        }
        self.time = 0.0;
        self.mincut_history.clear();
        self.action_history.clear();
    }

    /// Run multiple strange loop iterations
    pub fn run(&mut self, steps: usize) -> Vec<MetaAction> {
        let mut actions = Vec::new();
        for _ in 0..steps {
            actions.push(self.strange_loop_step());
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_neuron() {
        let mut neuron = MetaNeuron::new(0, 10);

        // Feed increasing summaries
        for i in 0..15 {
            let _ = neuron.modulate(0.1 * i as f64);
        }

        // Should have accumulated state
        assert!(neuron.history.len() == 10);
    }

    #[test]
    fn test_strange_loop_creation() {
        let graph = DynamicGraph::new();
        for i in 0..10 {
            graph.insert_edge(i, (i + 1) % 10, 1.0).unwrap();
        }

        let config = StrangeLoopConfig::default();
        let system = MetaCognitiveMinCut::new(graph, config);

        let (l0, l1, l2) = system.level_summary();
        assert!(l0 > 0.0);
    }

    #[test]
    fn test_strange_loop_step() {
        let graph = DynamicGraph::new();
        for i in 0..10 {
            for j in (i + 1)..10 {
                graph.insert_edge(i, j, 1.0).unwrap();
            }
        }

        let config = StrangeLoopConfig::default();
        let mut system = MetaCognitiveMinCut::new(graph, config);

        // Run a few steps
        let actions = system.run(5);
        assert_eq!(actions.len(), 5);
    }
}
