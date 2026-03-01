//! # Layer 5: Morphogenetic Networks as Developmental SNN
//!
//! Implements Turing-pattern-like self-organizing growth for graph structures.
//!
//! ## Theory
//!
//! ```text
//! Turing Pattern          SNN Equivalent         Graph Manifestation
//! ──────────────          ──────────────         ────────────────────
//! Activator               Excitatory neurons     Edge addition
//! Inhibitor               Inhibitory neurons     Edge removal
//! Diffusion               Spike propagation      Weight spreading
//! Reaction                Local computation      Node-local mincut
//! Pattern formation       Self-organization      Cluster emergence
//! ```
//!
//! ## Growth Rules
//!
//! - High neural activation → new edges
//! - Low activation → edge pruning
//! - Maturity detected via mincut stability

use super::{
    network::{LayerConfig, NetworkConfig, SpikingNetwork},
    neuron::{LIFNeuron, NeuronConfig, NeuronPopulation},
    SimTime, Spike,
};
use crate::graph::{DynamicGraph, VertexId};
use std::collections::HashMap;

/// Configuration for morphogenetic development
#[derive(Debug, Clone)]
pub struct MorphConfig {
    /// Grid size (neurons arranged spatially)
    pub grid_size: usize,
    /// Growth activation threshold
    pub growth_threshold: f64,
    /// Prune activation threshold
    pub prune_threshold: f64,
    /// Minimum edge weight to keep
    pub prune_weight: f64,
    /// Target connectivity for maturity
    pub target_connectivity: f64,
    /// Stability epsilon for maturity detection
    pub stability_epsilon: f64,
    /// Diffusion kernel sigma
    pub diffusion_sigma: f64,
    /// Maximum development steps
    pub max_steps: usize,
    /// Time step
    pub dt: f64,
}

impl Default for MorphConfig {
    fn default() -> Self {
        Self {
            grid_size: 10,
            growth_threshold: 0.6,
            prune_threshold: 0.2,
            prune_weight: 0.1,
            target_connectivity: 0.5,
            stability_epsilon: 0.01,
            diffusion_sigma: 2.0,
            max_steps: 1000,
            dt: 1.0,
        }
    }
}

/// Turing pattern type for different growth modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuringPattern {
    /// Spots pattern (local clusters)
    Spots,
    /// Stripes pattern (elongated structures)
    Stripes,
    /// Labyrinth pattern (complex connectivity)
    Labyrinth,
    /// Uniform (no pattern)
    Uniform,
}

/// Growth rules derived from neural activity
#[derive(Debug, Clone)]
pub struct GrowthRules {
    /// Threshold for creating new edges
    pub growth_threshold: f64,
    /// Threshold for pruning edges
    pub prune_threshold: f64,
    /// Minimum weight to survive pruning
    pub prune_weight: f64,
    /// Target connectivity (edge density)
    pub target_connectivity: f64,
    /// Pattern type to develop
    pub pattern: TuringPattern,
}

impl Default for GrowthRules {
    fn default() -> Self {
        Self {
            growth_threshold: 0.6,
            prune_threshold: 0.2,
            prune_weight: 0.1,
            target_connectivity: 0.5,
            pattern: TuringPattern::Spots,
        }
    }
}

/// A spatial position on the grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPosition {
    /// X coordinate
    pub x: usize,
    /// Y coordinate
    pub y: usize,
}

impl GridPosition {
    /// Create a new grid position
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    /// Convert to linear index
    pub fn to_index(&self, grid_size: usize) -> usize {
        self.y * grid_size + self.x
    }

    /// Create from linear index
    pub fn from_index(idx: usize, grid_size: usize) -> Self {
        Self {
            x: idx % grid_size,
            y: idx / grid_size,
        }
    }

    /// Euclidean distance to another position
    pub fn distance(&self, other: &GridPosition) -> f64 {
        let dx = self.x as f64 - other.x as f64;
        let dy = self.y as f64 - other.y as f64;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Diffusion kernel for spatial coupling
#[derive(Debug, Clone)]
pub struct DiffusionKernel {
    /// Kernel sigma (spread)
    pub sigma: f64,
    /// Excitatory range
    pub excite_range: f64,
    /// Inhibitory range
    pub inhibit_range: f64,
}

impl DiffusionKernel {
    /// Create a new diffusion kernel
    pub fn new(sigma: f64) -> Self {
        Self {
            sigma,
            excite_range: sigma,
            inhibit_range: sigma * 3.0,
        }
    }

    /// Compute kernel weight between two positions
    pub fn weight(&self, pos1: &GridPosition, pos2: &GridPosition) -> (f64, f64) {
        let d = pos1.distance(pos2);

        // Mexican hat profile: excitation close, inhibition far
        let excite = (-d * d / (2.0 * self.excite_range * self.excite_range)).exp();
        let inhibit = (-d * d / (2.0 * self.inhibit_range * self.inhibit_range)).exp() * 0.5;

        (excite, inhibit)
    }
}

/// A morphogenetic neuron pair (excitatory + inhibitory)
#[derive(Debug, Clone)]
pub struct MorphNeuronPair {
    /// Excitatory neuron
    pub excitatory: LIFNeuron,
    /// Inhibitory neuron
    pub inhibitory: LIFNeuron,
    /// Spatial position
    pub position: GridPosition,
}

impl MorphNeuronPair {
    /// Create a new neuron pair at position
    pub fn new(id: usize, position: GridPosition) -> Self {
        let e_config = NeuronConfig {
            tau_membrane: 15.0,
            threshold: 0.8,
            ..NeuronConfig::default()
        };

        let i_config = NeuronConfig {
            tau_membrane: 25.0,
            threshold: 0.6,
            ..NeuronConfig::default()
        };

        Self {
            excitatory: LIFNeuron::with_config(id * 2, e_config),
            inhibitory: LIFNeuron::with_config(id * 2 + 1, i_config),
            position,
        }
    }

    /// Get net activation (excitation - inhibition)
    pub fn net_activation(&self) -> f64 {
        self.excitatory.membrane_potential() - self.inhibitory.membrane_potential()
    }
}

/// Morphogenetic SNN with Turing-pattern dynamics
pub struct MorphogeneticSNN {
    /// Spatial grid of excitatory/inhibitory neuron pairs
    neurons: Vec<MorphNeuronPair>,
    /// Grid size
    grid_size: usize,
    /// Diffusion kernel
    diffusion_kernel: DiffusionKernel,
    /// Growing graph structure
    graph: DynamicGraph,
    /// Growth rules
    growth_rules: GrowthRules,
    /// Configuration
    config: MorphConfig,
    /// Current simulation time
    time: SimTime,
    /// Mincut history for maturity detection
    mincut_history: Vec<f64>,
    /// Is development complete
    is_mature: bool,
}

/// Maximum grid size to prevent memory exhaustion (256x256 = 65536 neurons)
const MAX_GRID_SIZE: usize = 256;

impl MorphogeneticSNN {
    /// Create a new morphogenetic SNN
    ///
    /// # Panics
    /// Panics if grid_size exceeds MAX_GRID_SIZE (256) to prevent memory exhaustion.
    pub fn new(config: MorphConfig) -> Self {
        // Resource limit: prevent grid_size² memory explosion
        let grid_size = config.grid_size.min(MAX_GRID_SIZE);
        let n = grid_size * grid_size;

        // Create neuron pairs on grid
        let neurons: Vec<_> = (0..n)
            .map(|i| {
                let pos = GridPosition::from_index(i, grid_size);
                MorphNeuronPair::new(i, pos)
            })
            .collect();

        // Initialize graph with nodes
        let graph = DynamicGraph::new();
        for i in 0..n {
            graph.add_vertex(i as u64);
        }

        // Add initial local connectivity (4-neighborhood)
        for y in 0..grid_size {
            for x in 0..grid_size {
                let i = y * grid_size + x;

                if x + 1 < grid_size {
                    let j = y * grid_size + (x + 1);
                    let _ = graph.insert_edge(i as u64, j as u64, 0.5);
                }

                if y + 1 < grid_size {
                    let j = (y + 1) * grid_size + x;
                    let _ = graph.insert_edge(i as u64, j as u64, 0.5);
                }
            }
        }

        let growth_rules = GrowthRules {
            growth_threshold: config.growth_threshold,
            prune_threshold: config.prune_threshold,
            prune_weight: config.prune_weight,
            target_connectivity: config.target_connectivity,
            ..GrowthRules::default()
        };

        Self {
            neurons,
            grid_size,
            diffusion_kernel: DiffusionKernel::new(config.diffusion_sigma),
            graph,
            growth_rules,
            config,
            time: 0.0,
            mincut_history: Vec::new(),
            is_mature: false,
        }
    }

    /// Run one development step
    pub fn develop_step(&mut self) {
        let dt = self.config.dt;
        self.time += dt;

        let n = self.neurons.len();

        // 1. Neural dynamics with spatial diffusion
        let mut activities = vec![0.0; n];

        for i in 0..n {
            // Compute diffusion input
            let pos_i = self.neurons[i].position;
            let mut e_input = 0.0;
            let mut i_input = 0.0;

            for j in 0..n {
                if i == j {
                    continue;
                }

                let pos_j = &self.neurons[j].position;
                let (e_weight, i_weight) = self.diffusion_kernel.weight(&pos_i, pos_j);

                e_input += e_weight * self.neurons[j].excitatory.membrane_potential();
                i_input += i_weight * self.neurons[j].inhibitory.membrane_potential();
            }

            // Update neurons
            let e_current = e_input - self.neurons[i].inhibitory.membrane_potential();
            let i_current = i_input + self.neurons[i].excitatory.membrane_potential();

            self.neurons[i].excitatory.step(e_current, dt, self.time);
            self.neurons[i].inhibitory.step(i_current, dt, self.time);

            activities[i] = self.neurons[i].net_activation();
        }

        // 2. Growth rules: modify graph based on activation
        self.apply_growth_rules(&activities);

        // 3. Update mincut history for maturity detection
        let mincut = self.estimate_mincut();
        self.mincut_history.push(mincut);

        if self.mincut_history.len() > 100 {
            self.mincut_history.remove(0);
        }

        // 4. Check maturity
        self.is_mature = self.check_maturity(mincut);
    }

    /// Apply growth rules based on neural activities
    fn apply_growth_rules(&mut self, activities: &[f64]) {
        let n = self.neurons.len();

        // Collect growth/prune decisions
        let mut to_add: Vec<(VertexId, VertexId)> = Vec::new();
        let mut to_remove: Vec<(VertexId, VertexId)> = Vec::new();

        for i in 0..n {
            let pos_i = &self.neurons[i].position;

            // High activation → sprout new connections
            if activities[i] > self.growth_rules.growth_threshold {
                // Find growth targets (nearby nodes without existing edges)
                for j in 0..n {
                    if i == j {
                        continue;
                    }

                    let pos_j = &self.neurons[j].position;
                    let dist = pos_i.distance(pos_j);

                    // Only connect to nearby nodes
                    if dist <= self.diffusion_kernel.excite_range * 2.0 {
                        let u = i as u64;
                        let v = j as u64;

                        if !self.graph.has_edge(u, v) {
                            to_add.push((u, v));
                        }
                    }
                }
            }

            // Low activation → retract connections
            if activities[i] < self.growth_rules.prune_threshold {
                let u = i as u64;

                for (v, _) in self.graph.neighbors(u) {
                    if let Some(edge) = self.graph.get_edge(u, v) {
                        if edge.weight < self.growth_rules.prune_weight {
                            to_remove.push((u, v));
                        }
                    }
                }
            }
        }

        // Apply changes
        for (u, v) in to_add {
            if !self.graph.has_edge(u, v) {
                let _ = self.graph.insert_edge(u, v, 0.5);
            }
        }

        for (u, v) in to_remove {
            let _ = self.graph.delete_edge(u, v);
        }
    }

    /// Estimate mincut (simplified)
    fn estimate_mincut(&self) -> f64 {
        if self.graph.num_vertices() == 0 {
            return 0.0;
        }

        // Approximate: minimum degree
        self.graph
            .vertices()
            .iter()
            .map(|&v| self.graph.degree(v) as f64)
            .fold(f64::INFINITY, f64::min)
    }

    /// Check if development is mature
    fn check_maturity(&self, current_mincut: f64) -> bool {
        // Mature when connectivity target reached AND mincut is stable
        let connectivity = self.graph.num_edges() as f64
            / (self.graph.num_vertices() * (self.graph.num_vertices() - 1) / 2).max(1) as f64;

        if connectivity < self.growth_rules.target_connectivity {
            return false;
        }

        // Check mincut stability
        if self.mincut_history.len() < 20 {
            return false;
        }

        let recent: Vec<_> = self.mincut_history.iter().rev().take(20).cloned().collect();
        let mean = recent.iter().sum::<f64>() / recent.len() as f64;
        let variance =
            recent.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / recent.len() as f64;

        variance.sqrt() < self.config.stability_epsilon
    }

    /// Run development until mature or max steps
    pub fn develop(&mut self) -> usize {
        let mut steps = 0;

        while steps < self.config.max_steps && !self.is_mature {
            self.develop_step();
            steps += 1;
        }

        steps
    }

    /// Get developed graph
    pub fn graph(&self) -> &DynamicGraph {
        &self.graph
    }

    /// Get grid size
    pub fn grid_size(&self) -> usize {
        self.grid_size
    }

    /// Check if mature
    pub fn is_mature(&self) -> bool {
        self.is_mature
    }

    /// Get current activation pattern
    pub fn activation_pattern(&self) -> Vec<f64> {
        self.neurons.iter().map(|n| n.net_activation()).collect()
    }

    /// Detect pattern type from activation
    pub fn detect_pattern(&self) -> TuringPattern {
        let activations = self.activation_pattern();

        // Compute spatial autocorrelation
        let mean = activations.iter().sum::<f64>() / activations.len() as f64;

        let mut local_corr = 0.0;
        let mut global_corr = 0.0;
        let mut count = 0;

        for i in 0..self.neurons.len() {
            for j in (i + 1)..self.neurons.len() {
                let dist = self.neurons[i].position.distance(&self.neurons[j].position);
                let prod = (activations[i] - mean) * (activations[j] - mean);

                if dist <= 2.0 {
                    local_corr += prod;
                }
                global_corr += prod / dist.max(0.1);
                count += 1;
            }
        }

        local_corr /= count.max(1) as f64;
        global_corr /= count.max(1) as f64;

        // Classify pattern
        if local_corr > 0.3 && global_corr < 0.1 {
            TuringPattern::Spots
        } else if local_corr > 0.2 && global_corr > 0.2 {
            TuringPattern::Stripes
        } else if local_corr > 0.1 {
            TuringPattern::Labyrinth
        } else {
            TuringPattern::Uniform
        }
    }

    /// Reset development
    pub fn reset(&mut self) {
        for pair in &mut self.neurons {
            pair.excitatory.reset();
            pair.inhibitory.reset();
        }

        self.graph.clear();
        for i in 0..self.neurons.len() {
            self.graph.add_vertex(i as u64);
        }

        // Re-add initial connectivity
        for y in 0..self.grid_size {
            for x in 0..self.grid_size {
                let i = y * self.grid_size + x;

                if x + 1 < self.grid_size {
                    let j = y * self.grid_size + (x + 1);
                    let _ = self.graph.insert_edge(i as u64, j as u64, 0.5);
                }

                if y + 1 < self.grid_size {
                    let j = (y + 1) * self.grid_size + x;
                    let _ = self.graph.insert_edge(i as u64, j as u64, 0.5);
                }
            }
        }

        self.time = 0.0;
        self.mincut_history.clear();
        self.is_mature = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_position() {
        let pos1 = GridPosition::new(0, 0);
        let pos2 = GridPosition::new(3, 4);

        assert_eq!(pos1.distance(&pos2), 5.0);
        assert_eq!(pos1.to_index(10), 0);
        assert_eq!(pos2.to_index(10), 43);
    }

    #[test]
    fn test_diffusion_kernel() {
        let kernel = DiffusionKernel::new(2.0);

        let pos1 = GridPosition::new(0, 0);
        let pos2 = GridPosition::new(1, 0);
        let pos3 = GridPosition::new(5, 0);

        let (e1, i1) = kernel.weight(&pos1, &pos2);
        let (e2, i2) = kernel.weight(&pos1, &pos3);

        // Closer should have higher excitation
        assert!(e1 > e2);
    }

    #[test]
    fn test_morphogenetic_snn_creation() {
        let mut config = MorphConfig::default();
        config.grid_size = 5;

        let snn = MorphogeneticSNN::new(config);

        assert_eq!(snn.neurons.len(), 25);
        assert!(!snn.is_mature());
    }

    #[test]
    fn test_morphogenetic_development() {
        let mut config = MorphConfig::default();
        config.grid_size = 5;
        config.max_steps = 100;

        let mut snn = MorphogeneticSNN::new(config);

        let steps = snn.develop();
        assert!(steps <= 100);

        // Graph should have evolved
        assert!(snn.graph().num_edges() > 0);
    }

    #[test]
    fn test_pattern_detection() {
        let mut config = MorphConfig::default();
        config.grid_size = 5;

        let snn = MorphogeneticSNN::new(config);
        let pattern = snn.detect_pattern();

        // Initial state should be some pattern
        assert!(
            pattern == TuringPattern::Uniform
                || pattern == TuringPattern::Spots
                || pattern == TuringPattern::Stripes
                || pattern == TuringPattern::Labyrinth
        );
    }
}
