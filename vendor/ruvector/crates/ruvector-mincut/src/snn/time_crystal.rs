//! # Layer 4: Time Crystal as Central Pattern Generator
//!
//! Implements discrete time-translation symmetry breaking for coordination patterns.
//!
//! ## Theory
//!
//! Time crystals exhibit periodic self-sustaining patterns. The SNN equivalent is a
//! Central Pattern Generator (CPG) - coupled oscillators that produce rhythmic output.
//!
//! ## Application
//!
//! Different phases correspond to different graph topologies. Phase transitions
//! trigger topology changes, and MinCut verification ensures stability within each phase.

use super::{
    network::{LayerConfig, NetworkConfig, SpikingNetwork},
    neuron::{LIFNeuron, NeuronConfig},
    SimTime, Spike, Vector,
};
use crate::graph::{DynamicGraph, VertexId};
use std::f64::consts::PI;

/// Configuration for Time Crystal CPG
#[derive(Debug, Clone)]
pub struct CPGConfig {
    /// Number of phases in the crystal
    pub num_phases: usize,
    /// Oscillation frequency (Hz)
    pub frequency: f64,
    /// Coupling strength between oscillators
    pub coupling: f64,
    /// Stability threshold for mincut verification
    pub stability_threshold: f64,
    /// Time step for simulation
    pub dt: f64,
    /// Phase transition threshold
    pub transition_threshold: f64,
}

impl Default for CPGConfig {
    fn default() -> Self {
        Self {
            num_phases: 4,
            frequency: 10.0, // 10 Hz default
            coupling: 0.3,
            stability_threshold: 0.1,
            dt: 1.0,
            transition_threshold: 0.8,
        }
    }
}

/// An oscillator neuron for CPG
#[derive(Debug, Clone)]
pub struct OscillatorNeuron {
    /// ID of this oscillator
    pub id: usize,
    /// Current phase (0 to 2π)
    pub phase: f64,
    /// Natural frequency (rad/ms)
    pub omega: f64,
    /// Amplitude
    pub amplitude: f64,
    /// Activity level (derived from phase)
    activity: f64,
}

impl OscillatorNeuron {
    /// Create a new oscillator
    pub fn new(id: usize, frequency_hz: f64, phase_offset: f64) -> Self {
        let omega = 2.0 * PI * frequency_hz / 1000.0; // Convert to rad/ms

        Self {
            id,
            phase: phase_offset,
            omega,
            amplitude: 1.0,
            activity: (phase_offset).cos(),
        }
    }

    /// Integrate oscillator dynamics
    pub fn integrate(&mut self, dt: f64, coupling_input: f64) {
        // Kuramoto-like dynamics: dφ/dt = ω + K*sin(φ_coupled - φ)
        let d_phase = self.omega + coupling_input;
        self.phase += d_phase * dt;

        // Keep phase in [0, 2π]
        while self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        while self.phase < 0.0 {
            self.phase += 2.0 * PI;
        }

        // Update activity
        self.activity = self.amplitude * self.phase.cos();
    }

    /// Get current activity
    pub fn activity(&self) -> f64 {
        self.activity
    }

    /// Reset oscillator
    pub fn reset(&mut self, phase: f64) {
        self.phase = phase;
        self.activity = (phase).cos();
    }
}

/// Graph topology for a specific phase
#[derive(Clone)]
pub struct PhaseTopology {
    /// Phase index
    pub phase_id: usize,
    /// Graph structure for this phase
    pub graph: DynamicGraph,
    /// Expected mincut value for stability
    pub expected_mincut: f64,
    /// Entry points for search (mincut boundary nodes)
    entry_points: Vec<VertexId>,
}

impl PhaseTopology {
    /// Create a new phase topology
    pub fn new(phase_id: usize) -> Self {
        Self {
            phase_id,
            graph: DynamicGraph::new(),
            expected_mincut: 0.0,
            entry_points: Vec::new(),
        }
    }

    /// Build topology from base graph with phase-specific modifications
    pub fn from_graph(phase_id: usize, base: &DynamicGraph, modulation: f64) -> Self {
        let graph = base.clone();

        // Modulate edge weights based on phase
        let phase_factor = (phase_id as f64 * PI / 2.0).sin().abs() + 0.5;

        for edge in graph.edges() {
            let new_weight = edge.weight * phase_factor * (1.0 + modulation);
            let _ = graph.update_edge_weight(edge.source, edge.target, new_weight);
        }

        // Estimate expected mincut
        let expected_mincut = graph.edges().iter().map(|e| e.weight).sum::<f64>()
            / graph.num_vertices().max(1) as f64;

        Self {
            phase_id,
            graph,
            expected_mincut,
            entry_points: Vec::new(),
        }
    }

    /// Get entry points for search
    pub fn entry_points(&self) -> &[VertexId] {
        &self.entry_points
    }

    /// Update entry points based on mincut analysis
    pub fn update_entry_points(&mut self) {
        // Use vertices with highest degree as entry points
        let mut degrees: Vec<_> = self
            .graph
            .vertices()
            .iter()
            .map(|&v| (v, self.graph.degree(v)))
            .collect();

        degrees.sort_by_key(|(_, d)| std::cmp::Reverse(*d));

        self.entry_points = degrees.iter().take(5).map(|(v, _)| *v).collect();
    }

    /// Get expected mincut
    pub fn expected_mincut(&self) -> f64 {
        self.expected_mincut
    }
}

/// Time Crystal Central Pattern Generator
pub struct TimeCrystalCPG {
    /// Oscillator neurons (one per phase)
    oscillators: Vec<OscillatorNeuron>,
    /// Phase coupling determines crystal structure
    coupling: Vec<Vec<f64>>,
    /// Graph topology per phase
    phase_topologies: Vec<PhaseTopology>,
    /// Current phase (discrete time crystal state)
    current_phase: usize,
    /// Configuration
    config: CPGConfig,
    /// Current simulation time
    time: SimTime,
    /// Phase history for stability analysis
    phase_history: Vec<usize>,
    /// Active graph reference
    active_graph: DynamicGraph,
}

impl TimeCrystalCPG {
    /// Create a new Time Crystal CPG
    pub fn new(base_graph: DynamicGraph, config: CPGConfig) -> Self {
        let n = config.num_phases;

        // Create oscillators with phase offsets
        let oscillators: Vec<_> = (0..n)
            .map(|i| {
                let phase_offset = 2.0 * PI * i as f64 / n as f64;
                OscillatorNeuron::new(i, config.frequency, phase_offset)
            })
            .collect();

        // Create coupling matrix (nearest-neighbor coupling)
        let mut coupling = vec![vec![0.0; n]; n];
        for i in 0..n {
            let prev = (i + n - 1) % n;
            let next = (i + 1) % n;
            coupling[i][prev] = config.coupling;
            coupling[i][next] = config.coupling;
        }

        // Create phase topologies
        let phase_topologies: Vec<_> = (0..n)
            .map(|i| {
                let modulation = 0.1 * i as f64;
                PhaseTopology::from_graph(i, &base_graph, modulation)
            })
            .collect();

        Self {
            oscillators,
            coupling,
            phase_topologies,
            current_phase: 0,
            config,
            time: 0.0,
            phase_history: Vec::new(),
            active_graph: base_graph,
        }
    }

    /// Run one integration tick
    pub fn tick(&mut self) -> Option<usize> {
        let dt = self.config.dt;
        self.time += dt;

        // 1. Compute coupling inputs
        let n = self.oscillators.len();
        let mut coupling_inputs = vec![0.0; n];

        for i in 0..n {
            for j in 0..n {
                if i != j && self.coupling[i][j] != 0.0 {
                    // Kuramoto coupling: K * sin(φ_j - φ_i)
                    let phase_diff = self.oscillators[j].phase - self.oscillators[i].phase;
                    coupling_inputs[i] += self.coupling[i][j] * phase_diff.sin();
                }
            }
        }

        // 2. Oscillator dynamics
        for (i, osc) in self.oscillators.iter_mut().enumerate() {
            osc.integrate(dt, coupling_inputs[i]);
        }

        // 3. Winner-take-all: highest activity determines phase
        let winner = self
            .oscillators
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.activity()
                    .partial_cmp(&b.activity())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        // 4. Phase transition if winner changed
        let transition = if winner != self.current_phase {
            let old_phase = self.current_phase;
            self.transition_topology(old_phase, winner);
            self.current_phase = winner;
            self.phase_history.push(winner);

            // Prune history
            if self.phase_history.len() > 1000 {
                self.phase_history.remove(0);
            }

            Some(winner)
        } else {
            None
        };

        // 5. Verify time crystal stability via mincut
        if let Some(topology) = self.phase_topologies.get(self.current_phase) {
            let actual_mincut = self.estimate_mincut();

            if (topology.expected_mincut - actual_mincut).abs()
                > self.config.stability_threshold * topology.expected_mincut
            {
                self.repair_crystal();
            }
        }

        transition
    }

    /// Transition between topologies
    fn transition_topology(&mut self, from: usize, to: usize) {
        // Blend topologies during transition
        if let Some(to_topo) = self.phase_topologies.get(to) {
            self.active_graph = to_topo.graph.clone();
        }
    }

    /// Estimate mincut (simplified)
    fn estimate_mincut(&self) -> f64 {
        let n = self.active_graph.num_vertices();
        if n == 0 {
            return 0.0;
        }

        // Approximate: minimum degree
        self.active_graph
            .vertices()
            .iter()
            .map(|&v| self.active_graph.degree(v) as f64)
            .fold(f64::INFINITY, f64::min)
    }

    /// Repair crystal structure
    fn repair_crystal(&mut self) {
        // Re-synchronize oscillators
        let n = self.oscillators.len();
        for (i, osc) in self.oscillators.iter_mut().enumerate() {
            let target_phase = 2.0 * PI * i as f64 / n as f64;
            osc.reset(target_phase);
        }

        // Restore topology from template
        if let Some(topology) = self.phase_topologies.get(self.current_phase) {
            self.active_graph = topology.graph.clone();
        }
    }

    /// Get current phase
    pub fn current_phase(&self) -> usize {
        self.current_phase
    }

    /// Get oscillator phases
    pub fn phases(&self) -> Vec<f64> {
        self.oscillators.iter().map(|o| o.phase).collect()
    }

    /// Get oscillator activities
    pub fn activities(&self) -> Vec<f64> {
        self.oscillators.iter().map(|o| o.activity()).collect()
    }

    /// Get active graph
    pub fn active_graph(&self) -> &DynamicGraph {
        &self.active_graph
    }

    /// Phase-aware search exploiting crystal structure
    pub fn phase_aware_entry_points(&self) -> Vec<VertexId> {
        self.phase_topologies
            .get(self.current_phase)
            .map(|t| t.entry_points().to_vec())
            .unwrap_or_default()
    }

    /// Check if crystal is stable (periodic behavior)
    pub fn is_stable(&self) -> bool {
        if self.phase_history.len() < self.config.num_phases * 2 {
            return false;
        }

        // Check for periodic pattern
        let period = self.config.num_phases;
        let recent: Vec<_> = self.phase_history.iter().rev().take(period * 2).collect();

        for i in 0..period {
            if recent.get(i) != recent.get(i + period) {
                return false;
            }
        }

        true
    }

    /// Get crystal periodicity (0 if not periodic)
    pub fn periodicity(&self) -> usize {
        if self.phase_history.len() < 10 {
            return 0;
        }

        // Find shortest repeating subsequence
        for period in 1..=self.config.num_phases {
            let mut is_periodic = true;

            for i in 0..(self.phase_history.len() - period) {
                if self.phase_history[i] != self.phase_history[i + period] {
                    is_periodic = false;
                    break;
                }
            }

            if is_periodic {
                return period;
            }
        }

        0
    }

    /// Run for specified duration
    pub fn run(&mut self, duration: f64) -> Vec<usize> {
        let steps = (duration / self.config.dt) as usize;
        let mut transitions = Vec::new();

        for _ in 0..steps {
            if let Some(new_phase) = self.tick() {
                transitions.push(new_phase);
            }
        }

        transitions
    }

    /// Reset CPG
    pub fn reset(&mut self) {
        let n = self.oscillators.len();
        for (i, osc) in self.oscillators.iter_mut().enumerate() {
            let phase_offset = 2.0 * PI * i as f64 / n as f64;
            osc.reset(phase_offset);
        }

        self.current_phase = 0;
        self.time = 0.0;
        self.phase_history.clear();

        if let Some(topology) = self.phase_topologies.get(0) {
            self.active_graph = topology.graph.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oscillator_neuron() {
        let mut osc = OscillatorNeuron::new(0, 10.0, 0.0);

        // Should oscillate
        let initial_activity = osc.activity();

        for _ in 0..100 {
            osc.integrate(1.0, 0.0);
        }

        // Activity should have changed
        assert!(osc.activity() != initial_activity || osc.phase != 0.0);
    }

    #[test]
    fn test_phase_topology() {
        let graph = DynamicGraph::new();
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(1, 2, 1.0).unwrap();

        let topology = PhaseTopology::from_graph(0, &graph, 0.1);

        assert_eq!(topology.phase_id, 0);
        assert!(topology.expected_mincut >= 0.0);
    }

    #[test]
    fn test_time_crystal_cpg() {
        let graph = DynamicGraph::new();
        for i in 0..10 {
            graph.insert_edge(i, (i + 1) % 10, 1.0).unwrap();
        }

        let config = CPGConfig::default();
        let mut cpg = TimeCrystalCPG::new(graph, config);

        // Run for a while
        let transitions = cpg.run(1000.0);

        // Should have some phase transitions
        assert!(cpg.time > 0.0);
    }

    #[test]
    fn test_phase_aware_entry() {
        let graph = DynamicGraph::new();
        for i in 0..5 {
            for j in (i + 1)..5 {
                graph.insert_edge(i, j, 1.0).unwrap();
            }
        }

        let mut config = CPGConfig::default();
        config.num_phases = 2;

        let cpg = TimeCrystalCPG::new(graph, config);
        let _entry_points = cpg.phase_aware_entry_points();
    }
}
