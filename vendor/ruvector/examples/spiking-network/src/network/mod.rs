//! Spiking neural network implementation.
//!
//! This module provides the core network structure with event-driven processing.
//!
//! ## Event-Driven Architecture
//!
//! Unlike conventional ANNs that evaluate every neuron every cycle, this network:
//! - Processes only when spikes arrive
//! - Skips silent neurons entirely
//! - Routes tiny spike events (few bits each)
//! - Maintains microsecond-scale latency
//!
//! ## Network Topologies
//!
//! Supports ASIC-friendly connectivity patterns:
//! - Local 2D grids (minimal routing)
//! - Small-world networks (efficient paths)
//! - Hierarchical layers (feedforward)
//! - Custom sparse connectivity

mod synapse;
mod topology;

pub use synapse::{Synapse, SynapseType};
pub use topology::{ConnectionPattern, LocalConnectivity, TopologyConfig};

use crate::encoding::{SparseSpikes, SpikeEvent};
use crate::error::{Result, SpikingError};
use crate::neuron::{LIFNeuron, SpikingNeuron};
use indexmap::IndexMap;
use parking_lot::RwLock;
use priority_queue::PriorityQueue;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::cmp::Reverse;
use std::sync::Arc;

/// Network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Number of neurons
    pub num_neurons: usize,
    /// Simulation timestep (ms)
    pub dt: f32,
    /// Enable parallel processing
    pub parallel: bool,
    /// Maximum synapses per neuron (for ASIC budgeting)
    pub max_synapses_per_neuron: usize,
    /// Topology configuration
    pub topology: TopologyConfig,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            num_neurons: 1000,
            dt: 1.0,
            parallel: true,
            max_synapses_per_neuron: 100,
            topology: TopologyConfig::default(),
        }
    }
}

/// Statistics from network simulation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Total spikes generated
    pub total_spikes: usize,
    /// Spikes per timestep
    pub spikes_per_step: Vec<usize>,
    /// Active neuron count per timestep
    pub active_neurons_per_step: Vec<usize>,
    /// Energy consumed (picojoules)
    pub energy_consumed: f64,
    /// Simulation time (ms)
    pub simulation_time: f32,
    /// Average firing rate (Hz)
    pub avg_firing_rate: f32,
    /// Network sparsity (fraction of silent neurons)
    pub sparsity: f32,
}

impl NetworkStats {
    /// Calculate statistics after simulation.
    pub fn finalize(&mut self) {
        if self.spikes_per_step.is_empty() {
            return;
        }

        self.total_spikes = self.spikes_per_step.iter().sum();

        let num_neurons = if !self.active_neurons_per_step.is_empty() {
            self.active_neurons_per_step.iter().max().copied().unwrap_or(1)
        } else {
            1
        };

        // Calculate average firing rate
        if self.simulation_time > 0.0 && num_neurons > 0 {
            self.avg_firing_rate = (self.total_spikes as f32)
                / (num_neurons as f32)
                / self.simulation_time
                * 1000.0;
        }

        // Calculate sparsity
        let total_possible: usize = self.active_neurons_per_step.iter().sum();
        if total_possible > 0 {
            self.sparsity = 1.0 - (self.total_spikes as f32 / total_possible as f32);
        }
    }
}

/// Event in the priority queue for event-driven simulation.
#[derive(Debug, Clone)]
struct NetworkEvent {
    /// Target neuron index
    target: usize,
    /// Event time
    time: f32,
    /// Input current to deliver
    current: f32,
}

impl PartialEq for NetworkEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.target == other.target
    }
}

impl Eq for NetworkEvent {}

impl std::hash::Hash for NetworkEvent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.time.to_bits().hash(state);
    }
}

/// Spiking neural network with event-driven processing.
pub struct SpikingNetwork {
    /// Network configuration
    config: NetworkConfig,
    /// Neurons (using LIF as default)
    neurons: Vec<LIFNeuron>,
    /// Outgoing connections: source -> [(target, synapse)]
    connections: Vec<SmallVec<[(usize, Synapse); 16]>>,
    /// Event queue for spike scheduling
    event_queue: PriorityQueue<NetworkEvent, Reverse<i64>>,
    /// Current simulation time
    current_time: f32,
    /// Statistics collector
    stats: NetworkStats,
    /// Output spikes (for external readout)
    output_spikes: Arc<RwLock<Vec<SpikeEvent>>>,
}

impl SpikingNetwork {
    /// Create a new spiking network.
    pub fn new(config: NetworkConfig) -> Result<Self> {
        if config.num_neurons == 0 {
            return Err(SpikingError::InvalidParams("num_neurons must be > 0".into()));
        }

        // Initialize neurons
        let neurons: Vec<LIFNeuron> = (0..config.num_neurons)
            .map(|_| LIFNeuron::with_defaults())
            .collect();

        // Initialize connection storage
        let connections = vec![SmallVec::new(); config.num_neurons];

        Ok(Self {
            config,
            neurons,
            connections,
            event_queue: PriorityQueue::new(),
            current_time: 0.0,
            stats: NetworkStats::default(),
            output_spikes: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Create network with given number of neurons and default config.
    pub fn with_neurons(num_neurons: usize) -> Result<Self> {
        Self::new(NetworkConfig {
            num_neurons,
            ..Default::default()
        })
    }

    /// Add a synaptic connection.
    pub fn connect(&mut self, source: usize, target: usize, synapse: Synapse) -> Result<()> {
        if source >= self.config.num_neurons || target >= self.config.num_neurons {
            return Err(SpikingError::TopologyError("Invalid neuron indices".into()));
        }

        if self.connections[source].len() >= self.config.max_synapses_per_neuron {
            return Err(SpikingError::ResourceExhausted(format!(
                "Max synapses ({}) reached for neuron {}",
                self.config.max_synapses_per_neuron, source
            )));
        }

        self.connections[source].push((target, synapse));
        Ok(())
    }

    /// Build network topology from configuration.
    pub fn build_topology(&mut self) -> Result<()> {
        let pattern = self.config.topology.pattern.clone();
        let num_neurons = self.config.num_neurons;

        match pattern {
            ConnectionPattern::AllToAll { probability } => {
                self.build_random_connections(probability)?;
            }
            ConnectionPattern::LocalGrid { width, radius } => {
                self.build_local_grid(width, radius)?;
            }
            ConnectionPattern::SmallWorld {
                k,
                rewire_prob,
            } => {
                self.build_small_world(k, rewire_prob)?;
            }
            ConnectionPattern::Feedforward { layer_sizes } => {
                self.build_feedforward(&layer_sizes)?;
            }
            ConnectionPattern::Custom => {
                // Connections added manually
            }
        }

        Ok(())
    }

    fn build_random_connections(&mut self, probability: f32) -> Result<()> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n = self.config.num_neurons;

        for src in 0..n {
            for tgt in 0..n {
                if src != tgt && rng.gen::<f32>() < probability {
                    let weight = rng.gen_range(0.1..1.0);
                    let synapse = Synapse::excitatory(weight);
                    let _ = self.connect(src, tgt, synapse);
                }
            }
        }
        Ok(())
    }

    fn build_local_grid(&mut self, width: usize, radius: usize) -> Result<()> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let height = self.config.num_neurons / width;

        for y in 0..height {
            for x in 0..width {
                let src = y * width + x;

                // Connect to neighbors within radius
                for dy in -(radius as i32)..=(radius as i32) {
                    for dx in -(radius as i32)..=(radius as i32) {
                        if dx == 0 && dy == 0 {
                            continue;
                        }

                        let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                        let ny = (y as i32 + dy).rem_euclid(height as i32) as usize;
                        let tgt = ny * width + nx;

                        if tgt < self.config.num_neurons {
                            let distance = ((dx * dx + dy * dy) as f32).sqrt();
                            let weight = 1.0 / distance * rng.gen_range(0.5..1.0);
                            let synapse = Synapse::excitatory(weight);
                            let _ = self.connect(src, tgt, synapse);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn build_small_world(&mut self, k: usize, rewire_prob: f32) -> Result<()> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n = self.config.num_neurons;

        // Start with ring lattice
        for i in 0..n {
            for j in 1..=k / 2 {
                let neighbor = (i + j) % n;
                let weight = rng.gen_range(0.5..1.0);
                let synapse = Synapse::excitatory(weight);
                let _ = self.connect(i, neighbor, synapse);
            }
        }

        // Rewire with probability
        for i in 0..n {
            if rng.gen::<f32>() < rewire_prob {
                let new_target = rng.gen_range(0..n);
                if new_target != i {
                    let weight = rng.gen_range(0.5..1.0);
                    let synapse = Synapse::excitatory(weight);
                    let _ = self.connect(i, new_target, synapse);
                }
            }
        }
        Ok(())
    }

    fn build_feedforward(&mut self, layer_sizes: &[usize]) -> Result<()> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut offset = 0;

        for i in 0..layer_sizes.len() - 1 {
            let src_size = layer_sizes[i];
            let tgt_size = layer_sizes[i + 1];
            let tgt_offset = offset + src_size;

            for src in 0..src_size {
                for tgt in 0..tgt_size {
                    let weight = rng.gen_range(0.1..1.0);
                    let synapse = Synapse::excitatory(weight);
                    let _ = self.connect(offset + src, tgt_offset + tgt, synapse);
                }
            }

            offset = tgt_offset;
        }
        Ok(())
    }

    /// Inject external input spikes.
    pub fn inject_spikes(&mut self, spikes: &SparseSpikes) {
        for event in &spikes.events {
            if (event.source as usize) < self.config.num_neurons {
                self.schedule_event(
                    event.source as usize,
                    event.time,
                    1.0, // Unit current for input spikes
                );
            }
        }
    }

    /// Schedule an event for future processing.
    fn schedule_event(&mut self, target: usize, time: f32, current: f32) {
        let event = NetworkEvent {
            target,
            time,
            current,
        };
        // Priority is negative time (earlier = higher priority)
        let priority = Reverse((time * 1000.0) as i64);
        self.event_queue.push(event, priority);
    }

    /// Process one timestep using event-driven simulation.
    pub fn step(&mut self) -> usize {
        let dt = self.config.dt;
        let next_time = self.current_time + dt;
        let mut spikes_this_step = 0;

        // Process events up to next_time
        while let Some((event, _)) = self.event_queue.peek() {
            if event.time > next_time {
                break;
            }

            let event = self.event_queue.pop().unwrap().0;
            self.neurons[event.target].receive_input(event.current);
        }

        // Update all neurons and collect spikes
        let spike_indices: Vec<usize> = if self.config.parallel {
            self.neurons
                .par_iter_mut()
                .enumerate()
                .filter_map(|(i, neuron)| {
                    if neuron.update(dt) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            self.neurons
                .iter_mut()
                .enumerate()
                .filter_map(|(i, neuron)| {
                    if neuron.update(dt) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Propagate spikes
        for &src in &spike_indices {
            spikes_this_step += 1;

            // Record output spike
            {
                let mut outputs = self.output_spikes.write();
                outputs.push(SpikeEvent::new(src as u32, self.current_time));
            }

            // Schedule postsynaptic events
            for &(target, ref synapse) in &self.connections[src] {
                let arrival_time = self.current_time + synapse.delay;
                let current = synapse.weight * synapse.sign();
                self.schedule_event(target, arrival_time, current);
            }
        }

        // Update statistics
        self.stats.spikes_per_step.push(spikes_this_step);
        self.stats.active_neurons_per_step.push(self.config.num_neurons);
        self.stats.energy_consumed += self.estimate_step_energy(spikes_this_step);

        self.current_time = next_time;
        spikes_this_step
    }

    /// Run simulation for given duration.
    pub fn run(&mut self, duration_ms: f32) -> NetworkStats {
        let num_steps = (duration_ms / self.config.dt) as usize;

        for _ in 0..num_steps {
            self.step();
        }

        self.stats.simulation_time = duration_ms;
        self.stats.finalize();
        self.stats.clone()
    }

    /// Get output spikes.
    pub fn output_spikes(&self) -> Vec<SpikeEvent> {
        self.output_spikes.read().clone()
    }

    /// Clear output spike buffer.
    pub fn clear_outputs(&mut self) {
        self.output_spikes.write().clear();
    }

    /// Reset network to initial state.
    pub fn reset(&mut self) {
        for neuron in &mut self.neurons {
            neuron.reset();
        }
        self.event_queue.clear();
        self.current_time = 0.0;
        self.stats = NetworkStats::default();
        self.output_spikes.write().clear();
    }

    /// Get current time.
    pub fn current_time(&self) -> f32 {
        self.current_time
    }

    /// Get number of neurons.
    pub fn num_neurons(&self) -> usize {
        self.config.num_neurons
    }

    /// Get total number of synapses.
    pub fn num_synapses(&self) -> usize {
        self.connections.iter().map(|c| c.len()).sum()
    }

    /// Estimate energy for one step.
    fn estimate_step_energy(&self, num_spikes: usize) -> f64 {
        // Base energy for updates
        let update_energy = self.config.num_neurons as f64 * 3.0; // pJ per neuron

        // Spike energy
        let spike_energy = num_spikes as f64 * 10.0; // pJ per spike

        update_energy + spike_energy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_creation() {
        let network = SpikingNetwork::with_neurons(100).unwrap();
        assert_eq!(network.num_neurons(), 100);
    }

    #[test]
    fn test_basic_connection() {
        let mut network = SpikingNetwork::with_neurons(10).unwrap();
        let synapse = Synapse::excitatory(0.5);
        network.connect(0, 1, synapse).unwrap();
        assert_eq!(network.num_synapses(), 1);
    }

    #[test]
    fn test_simulation_step() {
        let mut network = SpikingNetwork::with_neurons(10).unwrap();

        // Add some connections
        for i in 0..9 {
            network.connect(i, i + 1, Synapse::excitatory(0.5)).unwrap();
        }

        // Inject strong input to first neuron
        let mut spikes = SparseSpikes::new(10, 1);
        spikes.add_spike(0, 0);
        network.inject_spikes(&spikes);

        // Run a few steps
        let stats = network.run(100.0);
        assert!(stats.total_spikes > 0);
    }

    #[test]
    fn test_sparsity_tracking() {
        let mut network = SpikingNetwork::with_neurons(100).unwrap();
        network.build_topology().unwrap();

        let stats = network.run(100.0);
        // Without input, should be very sparse
        assert!(stats.sparsity > 0.9);
    }
}
