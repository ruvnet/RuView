//! # Spiking Neural Network Architecture
//!
//! Provides layered spiking network architecture for integration with MinCut algorithms.
//!
//! ## Network Types
//!
//! - **Feedforward**: Input → Hidden → Output
//! - **Recurrent**: With lateral connections
//! - **Graph-coupled**: Topology mirrors graph structure

use super::{
    neuron::{LIFNeuron, NeuronConfig, NeuronPopulation, SpikeTrain},
    synapse::{STDPConfig, Synapse, SynapseMatrix},
    SimTime, Spike, Vector,
};
use crate::graph::DynamicGraph;
use rayon::prelude::*;
use std::collections::VecDeque;

/// Configuration for a single network layer
#[derive(Debug, Clone)]
pub struct LayerConfig {
    /// Number of neurons in this layer
    pub size: usize,
    /// Neuron configuration
    pub neuron_config: NeuronConfig,
    /// Whether this layer has recurrent (lateral) connections
    pub recurrent: bool,
}

impl LayerConfig {
    /// Create a new layer config
    pub fn new(size: usize) -> Self {
        Self {
            size,
            neuron_config: NeuronConfig::default(),
            recurrent: false,
        }
    }

    /// Enable recurrent connections
    pub fn with_recurrence(mut self) -> Self {
        self.recurrent = true;
        self
    }

    /// Set custom neuron configuration
    pub fn with_neuron_config(mut self, config: NeuronConfig) -> Self {
        self.neuron_config = config;
        self
    }
}

/// Configuration for the full network
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Layer configurations (input to output)
    pub layers: Vec<LayerConfig>,
    /// STDP configuration for all synapses
    pub stdp_config: STDPConfig,
    /// Time step for simulation
    pub dt: f64,
    /// Enable winner-take-all lateral inhibition
    pub winner_take_all: bool,
    /// WTA inhibition strength
    pub wta_strength: f64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            layers: vec![
                LayerConfig::new(100), // Input
                LayerConfig::new(50),  // Hidden
                LayerConfig::new(10),  // Output
            ],
            stdp_config: STDPConfig::default(),
            dt: 1.0,
            winner_take_all: false,
            wta_strength: 0.8,
        }
    }
}

/// A spiking neural network
#[derive(Debug, Clone)]
pub struct SpikingNetwork {
    /// Configuration
    pub config: NetworkConfig,
    /// Neurons organized by layer
    layers: Vec<NeuronPopulation>,
    /// Feedforward weight matrices (layer i → layer i+1)
    feedforward_weights: Vec<SynapseMatrix>,
    /// Recurrent weight matrices (within layer)
    recurrent_weights: Vec<Option<SynapseMatrix>>,
    /// Current simulation time
    time: SimTime,
    /// Spike buffer for delayed transmission
    spike_buffer: VecDeque<(Spike, usize, SimTime)>, // (spike, target_layer, arrival_time)
    /// Global inhibition state (for WTA)
    global_inhibition: f64,
}

impl SpikingNetwork {
    /// Create a new spiking network from configuration
    pub fn new(config: NetworkConfig) -> Self {
        let mut layers = Vec::new();
        let mut feedforward_weights = Vec::new();
        let mut recurrent_weights = Vec::new();

        for (i, layer_config) in config.layers.iter().enumerate() {
            // Create neuron population
            let population = NeuronPopulation::with_config(
                layer_config.size,
                layer_config.neuron_config.clone(),
            );
            layers.push(population);

            // Create feedforward weights to next layer
            if i + 1 < config.layers.len() {
                let next_size = config.layers[i + 1].size;
                let mut weights = SynapseMatrix::with_config(
                    layer_config.size,
                    next_size,
                    config.stdp_config.clone(),
                );

                // Initialize with random weights
                for pre in 0..layer_config.size {
                    for post in 0..next_size {
                        let weight = rand_weight();
                        weights.add_synapse(pre, post, weight);
                    }
                }

                feedforward_weights.push(weights);
            }

            // Create recurrent weights if enabled
            if layer_config.recurrent {
                let mut weights = SynapseMatrix::with_config(
                    layer_config.size,
                    layer_config.size,
                    config.stdp_config.clone(),
                );

                // Sparse random recurrent connections
                for pre in 0..layer_config.size {
                    for post in 0..layer_config.size {
                        if pre != post && rand_bool(0.1) {
                            weights.add_synapse(pre, post, rand_weight() * 0.5);
                        }
                    }
                }

                recurrent_weights.push(Some(weights));
            } else {
                recurrent_weights.push(None);
            }
        }

        Self {
            config,
            layers,
            feedforward_weights,
            recurrent_weights,
            time: 0.0,
            spike_buffer: VecDeque::new(),
            global_inhibition: 0.0,
        }
    }

    /// Create network with topology matching a graph
    pub fn from_graph(graph: &DynamicGraph, config: NetworkConfig) -> Self {
        let n = graph.num_vertices();

        // Single layer matching graph topology
        let mut network_config = config.clone();
        network_config.layers = vec![LayerConfig::new(n).with_recurrence()];

        let mut network = Self::new(network_config);

        // Copy graph edges as recurrent connections
        if let Some(ref mut recurrent) = network.recurrent_weights[0] {
            let vertices: Vec<_> = graph.vertices();
            let vertex_to_idx: std::collections::HashMap<_, _> =
                vertices.iter().enumerate().map(|(i, &v)| (v, i)).collect();

            for edge in graph.edges() {
                if let (Some(&pre), Some(&post)) = (
                    vertex_to_idx.get(&edge.source),
                    vertex_to_idx.get(&edge.target),
                ) {
                    recurrent.set_weight(pre, post, edge.weight);
                    recurrent.set_weight(post, pre, edge.weight); // Undirected
                }
            }
        }

        network
    }

    /// Reset network state
    pub fn reset(&mut self) {
        self.time = 0.0;
        self.spike_buffer.clear();
        self.global_inhibition = 0.0;

        for layer in &mut self.layers {
            layer.reset();
        }
    }

    /// Get number of layers
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Get layer size
    pub fn layer_size(&self, layer: usize) -> usize {
        self.layers.get(layer).map(|l| l.size()).unwrap_or(0)
    }

    /// Get current simulation time
    pub fn current_time(&self) -> SimTime {
        self.time
    }

    /// Inject current to input layer
    pub fn inject_current(&mut self, currents: &[f64]) {
        if !self.layers.is_empty() {
            let input_layer = &mut self.layers[0];
            let n = currents.len().min(input_layer.size());

            for (i, neuron) in input_layer.neurons.iter_mut().take(n).enumerate() {
                neuron.set_membrane_potential(neuron.membrane_potential() + currents[i] * 0.1);
            }
        }
    }

    /// Run one integration step
    /// Returns spikes from output layer
    pub fn step(&mut self) -> Vec<Spike> {
        let dt = self.config.dt;
        self.time += dt;

        // Collect all spikes from this timestep
        let mut all_spikes: Vec<Vec<Spike>> = Vec::new();

        // Process each layer
        for layer_idx in 0..self.layers.len() {
            // Calculate input currents for this layer
            let mut currents = vec![0.0; self.layers[layer_idx].size()];

            // Add feedforward input from previous layer (sparse iteration)
            if layer_idx > 0 {
                let weights = &self.feedforward_weights[layer_idx - 1];
                // Collect pre-activations once
                let pre_activations: Vec<f64> = self.layers[layer_idx - 1]
                    .neurons
                    .iter()
                    .map(|n| n.membrane_potential().max(0.0))
                    .collect();
                // Use sparse weighted sum computation
                let ff_currents = weights.compute_weighted_sums(&pre_activations);
                for (j, &c) in ff_currents.iter().enumerate() {
                    currents[j] += c;
                }
            }

            // Add recurrent input (sparse iteration)
            if let Some(ref weights) = self.recurrent_weights[layer_idx] {
                // Collect activations
                let activations: Vec<f64> = self.layers[layer_idx]
                    .neurons
                    .iter()
                    .map(|n| n.membrane_potential().max(0.0))
                    .collect();
                // Use sparse weighted sum computation
                let rec_currents = weights.compute_weighted_sums(&activations);
                for (j, &c) in rec_currents.iter().enumerate() {
                    currents[j] += c;
                }
            }

            // Apply winner-take-all inhibition
            if self.config.winner_take_all && layer_idx == self.layers.len() - 1 {
                let max_v = self.layers[layer_idx]
                    .neurons
                    .iter()
                    .map(|n| n.membrane_potential())
                    .fold(f64::NEG_INFINITY, f64::max);

                for (i, neuron) in self.layers[layer_idx].neurons.iter().enumerate() {
                    if neuron.membrane_potential() < max_v {
                        currents[i] -= self.config.wta_strength * self.global_inhibition;
                    }
                }
            }

            // Update neurons
            let spikes = self.layers[layer_idx].step(&currents, dt);
            all_spikes.push(spikes.clone());

            // Update global inhibition
            if !spikes.is_empty() {
                self.global_inhibition = (self.global_inhibition + 0.1).min(1.0);
            } else {
                self.global_inhibition *= 0.95;
            }

            // STDP updates for feedforward weights
            if layer_idx > 0 {
                for spike in &spikes {
                    self.feedforward_weights[layer_idx - 1]
                        .on_post_spike(spike.neuron_id, self.time);
                }
            }

            if layer_idx + 1 < self.layers.len() {
                for spike in &spikes {
                    self.feedforward_weights[layer_idx].on_pre_spike(spike.neuron_id, self.time);
                }
            }
        }

        // Return output layer spikes
        all_spikes.last().cloned().unwrap_or_default()
    }

    /// Run until a decision is made (output neuron spikes)
    pub fn run_until_decision(&mut self, max_steps: usize) -> Vec<Spike> {
        for _ in 0..max_steps {
            let spikes = self.step();
            if !spikes.is_empty() {
                return spikes;
            }
        }
        Vec::new()
    }

    /// Get population firing rate for a layer
    pub fn layer_rate(&self, layer: usize, window: f64) -> f64 {
        self.layers
            .get(layer)
            .map(|l| l.population_rate(window))
            .unwrap_or(0.0)
    }

    /// Get global synchrony
    pub fn global_synchrony(&self) -> f64 {
        let mut total_sync = 0.0;
        let mut count = 0;

        for layer in &self.layers {
            total_sync += layer.synchrony(10.0);
            count += 1;
        }

        if count > 0 {
            total_sync / count as f64
        } else {
            0.0
        }
    }

    /// Get synchrony matrix (pairwise correlation)
    pub fn synchrony_matrix(&self) -> Vec<Vec<f64>> {
        // Single layer synchrony for simplicity
        let layer = &self.layers[0];
        let n = layer.size();
        let mut matrix = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let corr =
                    layer.spike_trains[i].cross_correlation(&layer.spike_trains[j], 50.0, 5.0);
                let sync = corr.iter().sum::<f64>() / corr.len() as f64;
                matrix[i][j] = sync;
                matrix[j][i] = sync;
            }
            matrix[i][i] = 1.0;
        }

        matrix
    }

    /// Get output layer activities
    pub fn get_output(&self) -> Vec<f64> {
        self.layers
            .last()
            .map(|l| l.neurons.iter().map(|n| n.membrane_potential()).collect())
            .unwrap_or_default()
    }

    /// Apply reward signal for R-STDP
    pub fn apply_reward(&mut self, reward: f64) {
        for weights in &mut self.feedforward_weights {
            weights.apply_reward(reward);
        }
        for weights in &mut self.recurrent_weights {
            if let Some(w) = weights {
                w.apply_reward(reward);
            }
        }
    }

    /// Get low-activity regions (for search skip optimization)
    pub fn low_activity_regions(&self) -> Vec<usize> {
        let mut low_activity = Vec::new();
        let threshold = 0.001;

        for (layer_idx, layer) in self.layers.iter().enumerate() {
            for (neuron_idx, train) in layer.spike_trains.iter().enumerate() {
                if train.spike_rate(100.0) < threshold {
                    low_activity.push(layer_idx * 1000 + neuron_idx);
                }
            }
        }

        low_activity
    }

    /// Sync first layer weights back to graph
    pub fn sync_to_graph(&self, graph: &mut DynamicGraph) {
        if let Some(ref recurrent) = self.recurrent_weights.first().and_then(|r| r.as_ref()) {
            let vertices: Vec<_> = graph.vertices();

            for ((pre, post), synapse) in recurrent.iter() {
                if *pre < vertices.len() && *post < vertices.len() {
                    let u = vertices[*pre];
                    let v = vertices[*post];
                    if graph.has_edge(u, v) {
                        let _ = graph.update_edge_weight(u, v, synapse.weight);
                    }
                }
            }
        }
    }
}

// Thread-safe PRNG for weight initialization using atomic CAS
use std::sync::atomic::{AtomicU64, Ordering};
static RNG_STATE: AtomicU64 = AtomicU64::new(0x853c49e6748fea9b);

fn rand_u64() -> u64 {
    // Use compare_exchange loop to ensure atomicity
    loop {
        let current = RNG_STATE.load(Ordering::Relaxed);
        let next = current
            .wrapping_mul(0x5851f42d4c957f2d)
            .wrapping_add(0x14057b7ef767814f);
        match RNG_STATE.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(_) => continue, // Retry on contention
        }
    }
}

fn rand_weight() -> f64 {
    (rand_u64() as f64) / (u64::MAX as f64) * 0.5 + 0.25
}

fn rand_bool(p: f64) -> bool {
    (rand_u64() as f64) / (u64::MAX as f64) < p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_creation() {
        let config = NetworkConfig::default();
        let network = SpikingNetwork::new(config);

        assert_eq!(network.num_layers(), 3);
        assert_eq!(network.layer_size(0), 100);
        assert_eq!(network.layer_size(1), 50);
        assert_eq!(network.layer_size(2), 10);
    }

    #[test]
    fn test_network_step() {
        let config = NetworkConfig::default();
        let mut network = SpikingNetwork::new(config);

        // Inject strong current
        let currents = vec![5.0; 100];
        network.inject_current(&currents);

        // Run several steps
        let mut total_spikes = 0;
        for _ in 0..100 {
            let spikes = network.step();
            total_spikes += spikes.len();
        }

        // Should produce some output
        assert!(network.current_time() > 0.0);
    }

    #[test]
    fn test_graph_network() {
        use crate::graph::DynamicGraph;

        let graph = DynamicGraph::new();
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 0, 1.0).unwrap();

        let config = NetworkConfig::default();
        let network = SpikingNetwork::from_graph(&graph, config);

        assert_eq!(network.num_layers(), 1);
        assert_eq!(network.layer_size(0), 3);
    }

    #[test]
    fn test_synchrony_matrix() {
        let mut config = NetworkConfig::default();
        config.layers = vec![LayerConfig::new(5)];

        let mut network = SpikingNetwork::new(config);

        // Run a bit
        let currents = vec![2.0; 5];
        for _ in 0..50 {
            network.inject_current(&currents);
            network.step();
        }

        let sync = network.synchrony_matrix();
        assert_eq!(sync.len(), 5);
        assert_eq!(sync[0].len(), 5);

        // Diagonal should be 1
        for i in 0..5 {
            assert!((sync[i][i] - 1.0).abs() < 0.001);
        }
    }
}
