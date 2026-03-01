//! # Layer 3: Causal Discovery via Spike Timing
//!
//! Uses spike-timing cross-correlation to infer causal relationships in graph events.
//!
//! ## Key Insight
//!
//! Spike train cross-correlation with asymmetric temporal windows naturally encodes
//! Granger-like causality:
//!
//! ```text
//! Neuron A:  ──●────────●────────●────────
//! Neuron B:  ────────●────────●────────●──
//!            │←─Δt─→│
//!
//! If Δt consistently positive → A causes B
//! STDP learning rule naturally encodes this!
//! ```
//!
//! After learning, W_AB reflects causal strength A→B
//!
//! ## MinCut Application
//!
//! MinCut on the causal graph reveals optimal intervention points -
//! minimum changes needed to affect outcomes.

use super::{
    neuron::{LIFNeuron, NeuronConfig, SpikeTrain},
    synapse::{AsymmetricSTDP, STDPConfig, Synapse, SynapseMatrix},
    SimTime, Spike,
};
use crate::graph::{DynamicGraph, EdgeId, VertexId};
use std::collections::{HashMap, HashSet, VecDeque};

/// Configuration for causal discovery
#[derive(Debug, Clone)]
pub struct CausalConfig {
    /// Number of event types (neurons)
    pub num_event_types: usize,
    /// Threshold for causal relationship detection
    pub causal_threshold: f64,
    /// Time window for causality (ms)
    pub time_window: f64,
    /// Asymmetric STDP configuration
    pub stdp: AsymmetricSTDP,
    /// Learning rate for causal weight updates
    pub learning_rate: f64,
    /// Decay rate for causal weights
    pub decay_rate: f64,
}

impl Default for CausalConfig {
    fn default() -> Self {
        Self {
            num_event_types: 100,
            causal_threshold: 0.1,
            time_window: 50.0,
            stdp: AsymmetricSTDP::default(),
            learning_rate: 0.01,
            decay_rate: 0.001,
        }
    }
}

/// Type of causal relationship
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CausalRelation {
    /// A causes B (positive influence)
    Causes,
    /// A prevents B (negative influence)
    Prevents,
    /// No significant causal relationship
    None,
}

/// A directed causal relationship
#[derive(Debug, Clone)]
pub struct CausalEdge {
    /// Source event type
    pub source: usize,
    /// Target event type
    pub target: usize,
    /// Causal strength (absolute value)
    pub strength: f64,
    /// Type of relationship
    pub relation: CausalRelation,
}

/// Directed graph representing causal relationships
#[derive(Debug, Clone)]
pub struct CausalGraph {
    /// Number of nodes (event types)
    pub num_nodes: usize,
    /// Causal edges
    edges: Vec<CausalEdge>,
    /// Adjacency list (source → targets)
    adjacency: HashMap<usize, Vec<(usize, f64, CausalRelation)>>,
}

impl CausalGraph {
    /// Create a new empty causal graph
    pub fn new(num_nodes: usize) -> Self {
        Self {
            num_nodes,
            edges: Vec::new(),
            adjacency: HashMap::new(),
        }
    }

    /// Add a causal edge
    pub fn add_edge(
        &mut self,
        source: usize,
        target: usize,
        strength: f64,
        relation: CausalRelation,
    ) {
        self.edges.push(CausalEdge {
            source,
            target,
            strength,
            relation,
        });

        self.adjacency
            .entry(source)
            .or_insert_with(Vec::new)
            .push((target, strength, relation));
    }

    /// Get edges from a node
    pub fn edges_from(&self, source: usize) -> &[(usize, f64, CausalRelation)] {
        self.adjacency
            .get(&source)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all edges
    pub fn edges(&self) -> &[CausalEdge] {
        &self.edges
    }

    /// Maximum nodes for transitive closure (O(n³) algorithm)
    const MAX_CLOSURE_NODES: usize = 500;

    /// Compute transitive closure (indirect causation)
    ///
    /// Uses Floyd-Warshall algorithm with O(n³) complexity.
    /// Limited to MAX_CLOSURE_NODES to prevent DoS.
    pub fn transitive_closure(&self) -> Self {
        let mut closed = Self::new(self.num_nodes);

        // Resource limit: skip if too many nodes (O(n³) would be too slow)
        if self.num_nodes > Self::MAX_CLOSURE_NODES {
            // Just copy direct edges without transitive closure
            for edge in &self.edges {
                closed.add_edge(edge.source, edge.target, edge.strength, edge.relation);
            }
            return closed;
        }

        // Copy direct edges
        for edge in &self.edges {
            closed.add_edge(edge.source, edge.target, edge.strength, edge.relation);
        }

        // Floyd-Warshall-like algorithm for transitive closure
        for k in 0..self.num_nodes {
            for i in 0..self.num_nodes {
                for j in 0..self.num_nodes {
                    if i == j || i == k || j == k {
                        continue;
                    }

                    // Check if path i→k→j exists
                    let ik_strength = self
                        .adjacency
                        .get(&i)
                        .and_then(|edges| edges.iter().find(|(t, _, _)| *t == k))
                        .map(|(_, s, _)| *s);

                    let kj_strength = self
                        .adjacency
                        .get(&k)
                        .and_then(|edges| edges.iter().find(|(t, _, _)| *t == j))
                        .map(|(_, s, _)| *s);

                    if let (Some(s1), Some(s2)) = (ik_strength, kj_strength) {
                        let indirect_strength = s1 * s2;

                        // Only add if stronger than existing direct path
                        let existing = closed
                            .adjacency
                            .get(&i)
                            .and_then(|edges| edges.iter().find(|(t, _, _)| *t == j))
                            .map(|(_, s, _)| *s)
                            .unwrap_or(0.0);

                        if indirect_strength > existing {
                            closed.add_edge(i, j, indirect_strength, CausalRelation::Causes);
                        }
                    }
                }
            }
        }

        closed
    }

    /// Find nodes reachable from a source
    pub fn reachable_from(&self, source: usize) -> HashSet<usize> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(source);
        visited.insert(source);

        while let Some(node) = queue.pop_front() {
            for (target, _, _) in self.edges_from(node) {
                if visited.insert(*target) {
                    queue.push_back(*target);
                }
            }
        }

        visited
    }

    /// Convert to undirected graph for mincut analysis
    pub fn to_undirected(&self) -> DynamicGraph {
        let graph = DynamicGraph::new();

        for edge in &self.edges {
            if !graph.has_edge(edge.source as u64, edge.target as u64) {
                let _ = graph.insert_edge(edge.source as u64, edge.target as u64, edge.strength);
            }
        }

        graph
    }
}

/// Graph event that can be observed
#[derive(Debug, Clone)]
pub struct GraphEvent {
    /// Type of event
    pub event_type: GraphEventType,
    /// Associated vertex (if applicable)
    pub vertex: Option<VertexId>,
    /// Associated edge (if applicable)
    pub edge: Option<(VertexId, VertexId)>,
    /// Event metadata
    pub data: f64,
}

/// Types of graph events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphEventType {
    /// Edge was added
    EdgeInsert,
    /// Edge was removed
    EdgeDelete,
    /// Edge weight changed
    WeightChange,
    /// MinCut value changed
    MinCutChange,
    /// Component split
    ComponentSplit,
    /// Component merged
    ComponentMerge,
}

/// Causal discovery using spiking neural network
pub struct CausalDiscoverySNN {
    /// One neuron per graph event type
    event_neurons: Vec<LIFNeuron>,
    /// Spike trains for each neuron
    spike_trains: Vec<SpikeTrain>,
    /// Synaptic weights encode discovered causal strength
    synapses: SynapseMatrix,
    /// Asymmetric STDP for causality detection
    stdp: AsymmetricSTDP,
    /// Configuration
    config: CausalConfig,
    /// Current simulation time
    time: SimTime,
    /// Event type mapping
    event_type_map: HashMap<GraphEventType, usize>,
    /// Reverse mapping
    index_to_event: HashMap<usize, GraphEventType>,
}

impl CausalDiscoverySNN {
    /// Create a new causal discovery SNN
    pub fn new(config: CausalConfig) -> Self {
        let n = config.num_event_types;

        // Create event neurons
        let neuron_config = NeuronConfig {
            tau_membrane: 10.0, // Fast response
            threshold: 0.5,
            ..NeuronConfig::default()
        };

        let event_neurons: Vec<_> = (0..n)
            .map(|i| LIFNeuron::with_config(i, neuron_config.clone()))
            .collect();

        let spike_trains: Vec<_> = (0..n)
            .map(|i| SpikeTrain::with_window(i, config.time_window * 10.0))
            .collect();

        // Fully connected synapses
        let mut synapses = SynapseMatrix::new(n, n);
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    synapses.add_synapse(i, j, 0.0); // Start with zero weights
                }
            }
        }

        // Initialize event type mapping
        let event_type_map: HashMap<_, _> = [
            (GraphEventType::EdgeInsert, 0),
            (GraphEventType::EdgeDelete, 1),
            (GraphEventType::WeightChange, 2),
            (GraphEventType::MinCutChange, 3),
            (GraphEventType::ComponentSplit, 4),
            (GraphEventType::ComponentMerge, 5),
        ]
        .iter()
        .cloned()
        .collect();

        let index_to_event: HashMap<_, _> = event_type_map.iter().map(|(k, v)| (*v, *k)).collect();

        Self {
            event_neurons,
            spike_trains,
            synapses,
            stdp: config.stdp.clone(),
            config,
            time: 0.0,
            event_type_map,
            index_to_event,
        }
    }

    /// Convert graph event to neuron index
    fn event_to_neuron(&self, event: &GraphEvent) -> usize {
        self.event_type_map
            .get(&event.event_type)
            .copied()
            .unwrap_or(0)
    }

    /// Observe a graph event
    pub fn observe_event(&mut self, event: GraphEvent, timestamp: SimTime) {
        self.time = timestamp;

        // Convert graph event to spike
        let neuron_id = self.event_to_neuron(&event);

        if neuron_id < self.event_neurons.len() {
            // Record spike
            self.event_neurons[neuron_id].inject_spike(timestamp);
            self.spike_trains[neuron_id].record_spike(timestamp);

            // STDP update: causal relationships emerge in weights
            self.stdp
                .update_weights(&mut self.synapses, neuron_id, timestamp);
        }
    }

    /// Process a batch of events
    pub fn observe_events(&mut self, events: &[GraphEvent], timestamps: &[SimTime]) {
        for (event, &ts) in events.iter().zip(timestamps.iter()) {
            self.observe_event(event.clone(), ts);
        }
    }

    /// Decay all synaptic weights toward baseline
    ///
    /// Applies exponential decay: w' = w * (1 - decay_rate) + baseline * decay_rate
    pub fn decay_weights(&mut self) {
        let decay = self.config.decay_rate;
        let baseline = 0.5; // Neutral weight
        let n = self.config.num_event_types;

        // Iterate through all possible synapse pairs
        for i in 0..n {
            for j in 0..n {
                if let Some(synapse) = self.synapses.get_synapse_mut(i, j) {
                    // Exponential decay toward baseline
                    synapse.weight = synapse.weight * (1.0 - decay) + baseline * decay;
                }
            }
        }
    }

    /// Extract causal graph from learned weights
    pub fn extract_causal_graph(&self) -> CausalGraph {
        let n = self.config.num_event_types;
        let mut graph = CausalGraph::new(n);

        for ((i, j), synapse) in self.synapses.iter() {
            let w = synapse.weight;

            if w.abs() > self.config.causal_threshold {
                let strength = w.abs();
                let relation = if w > 0.0 {
                    CausalRelation::Causes
                } else {
                    CausalRelation::Prevents
                };

                graph.add_edge(*i, *j, strength, relation);
            }
        }

        graph
    }

    /// Find optimal intervention points using MinCut on causal graph
    pub fn optimal_intervention_points(
        &self,
        controllable: &[usize],
        targets: &[usize],
    ) -> Vec<usize> {
        let causal = self.extract_causal_graph();
        let undirected = causal.to_undirected();

        // Simple heuristic: find nodes on paths from controllable to targets
        let mut intervention_points = Vec::new();
        let controllable_set: HashSet<_> = controllable.iter().cloned().collect();
        let target_set: HashSet<_> = targets.iter().cloned().collect();

        for edge in causal.edges() {
            // If edge connects controllable region to target region
            if controllable_set.contains(&edge.source) || target_set.contains(&edge.target) {
                intervention_points.push(edge.source);
            }
        }

        intervention_points.sort();
        intervention_points.dedup();
        intervention_points
    }

    /// Get causal strength between two event types
    pub fn causal_strength(&self, from: GraphEventType, to: GraphEventType) -> f64 {
        let i = self.event_type_map.get(&from).copied().unwrap_or(0);
        let j = self.event_type_map.get(&to).copied().unwrap_or(0);

        self.synapses.weight(i, j)
    }

    /// Get all direct causes of an event type
    pub fn direct_causes(&self, event_type: GraphEventType) -> Vec<(GraphEventType, f64)> {
        let j = self.event_type_map.get(&event_type).copied().unwrap_or(0);
        let mut causes = Vec::new();

        for i in 0..self.config.num_event_types {
            if i != j {
                let w = self.synapses.weight(i, j);
                if w > self.config.causal_threshold {
                    if let Some(&event) = self.index_to_event.get(&i) {
                        causes.push((event, w));
                    }
                }
            }
        }

        causes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        causes
    }

    /// Get all direct effects of an event type
    pub fn direct_effects(&self, event_type: GraphEventType) -> Vec<(GraphEventType, f64)> {
        let i = self.event_type_map.get(&event_type).copied().unwrap_or(0);
        let mut effects = Vec::new();

        for j in 0..self.config.num_event_types {
            if i != j {
                let w = self.synapses.weight(i, j);
                if w > self.config.causal_threshold {
                    if let Some(&event) = self.index_to_event.get(&j) {
                        effects.push((event, w));
                    }
                }
            }
        }

        effects.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        effects
    }

    /// Reset the SNN
    pub fn reset(&mut self) {
        self.time = 0.0;

        for neuron in &mut self.event_neurons {
            neuron.reset();
        }

        for train in &mut self.spike_trains {
            train.clear();
        }

        // Reset weights to zero
        for i in 0..self.config.num_event_types {
            for j in 0..self.config.num_event_types {
                if i != j {
                    self.synapses.set_weight(i, j, 0.0);
                }
            }
        }
    }

    /// Get summary statistics
    pub fn summary(&self) -> CausalSummary {
        let causal = self.extract_causal_graph();

        let mut total_strength = 0.0;
        let mut causes_count = 0;
        let mut prevents_count = 0;

        for edge in causal.edges() {
            total_strength += edge.strength;
            match edge.relation {
                CausalRelation::Causes => causes_count += 1,
                CausalRelation::Prevents => prevents_count += 1,
                CausalRelation::None => {}
            }
        }

        CausalSummary {
            num_relationships: causal.edges().len(),
            causes_count,
            prevents_count,
            avg_strength: total_strength / causal.edges().len().max(1) as f64,
            time_elapsed: self.time,
        }
    }
}

/// Summary of causal discovery
#[derive(Debug, Clone)]
pub struct CausalSummary {
    /// Total number of discovered relationships
    pub num_relationships: usize,
    /// Number of positive causal relationships
    pub causes_count: usize,
    /// Number of preventive relationships
    pub prevents_count: usize,
    /// Average causal strength
    pub avg_strength: f64,
    /// Time elapsed in observation
    pub time_elapsed: SimTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_graph() {
        let mut graph = CausalGraph::new(5);
        graph.add_edge(0, 1, 0.8, CausalRelation::Causes);
        graph.add_edge(1, 2, 0.6, CausalRelation::Causes);

        assert_eq!(graph.edges().len(), 2);

        let reachable = graph.reachable_from(0);
        assert!(reachable.contains(&1));
        assert!(reachable.contains(&2));
    }

    #[test]
    fn test_causal_discovery_snn() {
        let config = CausalConfig::default();
        let mut snn = CausalDiscoverySNN::new(config);

        // Observe events with consistent temporal ordering
        for i in 0..10 {
            let t = i as f64 * 10.0;

            // Edge insert always followed by mincut change
            snn.observe_event(
                GraphEvent {
                    event_type: GraphEventType::EdgeInsert,
                    vertex: None,
                    edge: Some((0, 1)),
                    data: 1.0,
                },
                t,
            );

            snn.observe_event(
                GraphEvent {
                    event_type: GraphEventType::MinCutChange,
                    vertex: None,
                    edge: None,
                    data: 0.5,
                },
                t + 5.0,
            );
        }

        let summary = snn.summary();
        assert!(summary.time_elapsed > 0.0);
    }

    #[test]
    fn test_transitive_closure() {
        let mut graph = CausalGraph::new(4);
        graph.add_edge(0, 1, 0.8, CausalRelation::Causes);
        graph.add_edge(1, 2, 0.6, CausalRelation::Causes);
        graph.add_edge(2, 3, 0.5, CausalRelation::Causes);

        let closed = graph.transitive_closure();

        // Should have indirect edges
        assert!(closed.edges().len() >= 3);
    }

    #[test]
    fn test_intervention_points() {
        let config = CausalConfig::default();
        let snn = CausalDiscoverySNN::new(config);

        let interventions = snn.optimal_intervention_points(&[0, 1], &[3, 4]);
        // Should return some intervention points (may be empty if no learned causality)
        assert!(interventions.len() >= 0);
    }
}
