//! # Unified Cognitive MinCut Engine
//!
//! Combines all six integration layers into a unified system.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                      COGNITIVE MINCUT ENGINE                                │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                             │
//! │   ┌─────────────────────────────────────────────────────────────────────┐   │
//! │   │                        META-COGNITIVE LAYER                         │   │
//! │   │  Strange Loop + Neural Optimizer + Causal Discovery                 │   │
//! │   └───────────────────────────┬─────────────────────────────────────────┘   │
//! │                               │                                             │
//! │   ┌───────────────────────────▼─────────────────────────────────────────┐   │
//! │   │                      DYNAMICAL SYSTEMS LAYER                        │   │
//! │   │  Temporal Attractors + Time Crystals + Morphogenesis                │   │
//! │   └───────────────────────────┬─────────────────────────────────────────┘   │
//! │                               │                                             │
//! │   ┌───────────────────────────▼─────────────────────────────────────────┐   │
//! │   │                      GRAPH ALGORITHM LAYER                          │   │
//! │   │  Karger-Stein MinCut + Subpolynomial Search + HNSW                  │   │
//! │   └───────────────────────────┬─────────────────────────────────────────┘   │
//! │                               │                                             │
//! │   ┌───────────────────────────▼─────────────────────────────────────────┐   │
//! │   │                      NEUROMORPHIC SUBSTRATE                         │   │
//! │   │  SNN + STDP + Meta-Neuron + CPG                                     │   │
//! │   └─────────────────────────────────────────────────────────────────────┘   │
//! │                                                                             │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Performance Targets
//!
//! | Metric | Unified | Improvement |
//! |--------|---------|-------------|
//! | MinCut (1K nodes) | ~5 μs | 10x |
//! | Energy per query | ~10 μJ | 1000x |

use super::{
    attractor::{AttractorConfig, AttractorDynamics, EnergyLandscape},
    causal::{CausalConfig, CausalDiscoverySNN, CausalGraph, GraphEvent, GraphEventType},
    morphogenetic::{MorphConfig, MorphogeneticSNN, TuringPattern},
    optimizer::{GraphAction, NeuralGraphOptimizer, OptimizationResult, OptimizerConfig},
    strange_loop::{MetaAction, MetaCognitiveMinCut, StrangeLoopConfig},
    time_crystal::{CPGConfig, TimeCrystalCPG},
    SimTime, Spike,
};
use crate::graph::{DynamicGraph, VertexId, Weight};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Configuration for the Cognitive MinCut Engine
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Enable attractor dynamics layer
    pub enable_attractors: bool,
    /// Enable strange loop self-modification
    pub enable_strange_loop: bool,
    /// Enable causal discovery
    pub enable_causal_discovery: bool,
    /// Enable time crystal coordination
    pub enable_time_crystal: bool,
    /// Enable morphogenetic growth
    pub enable_morphogenetic: bool,
    /// Enable neural optimizer
    pub enable_optimizer: bool,
    /// Attractor configuration
    pub attractor_config: AttractorConfig,
    /// Strange loop configuration
    pub strange_loop_config: StrangeLoopConfig,
    /// Causal discovery configuration
    pub causal_config: CausalConfig,
    /// Time crystal configuration
    pub cpg_config: CPGConfig,
    /// Morphogenetic configuration
    pub morph_config: MorphConfig,
    /// Optimizer configuration
    pub optimizer_config: OptimizerConfig,
    /// Time step for unified simulation
    pub dt: f64,
    /// Maximum steps per operation
    pub max_steps: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enable_attractors: true,
            enable_strange_loop: true,
            enable_causal_discovery: true,
            enable_time_crystal: true,
            enable_morphogenetic: false, // Expensive, off by default
            enable_optimizer: true,
            attractor_config: AttractorConfig::default(),
            strange_loop_config: StrangeLoopConfig::default(),
            causal_config: CausalConfig::default(),
            cpg_config: CPGConfig::default(),
            morph_config: MorphConfig::default(),
            optimizer_config: OptimizerConfig::default(),
            dt: 1.0,
            max_steps: 1000,
        }
    }
}

/// Metrics from engine operation
#[derive(Debug, Clone, Default)]
pub struct EngineMetrics {
    /// Total time spent in computation
    pub total_time: Duration,
    /// Time in attractor dynamics
    pub attractor_time: Duration,
    /// Time in strange loop
    pub strange_loop_time: Duration,
    /// Time in causal discovery
    pub causal_time: Duration,
    /// Time in time crystal
    pub cpg_time: Duration,
    /// Time in morphogenesis
    pub morph_time: Duration,
    /// Time in optimization
    pub optimizer_time: Duration,
    /// Number of spikes generated
    pub total_spikes: usize,
    /// Energy estimate (arbitrary units)
    pub energy_estimate: f64,
    /// Current mincut value
    pub mincut_value: f64,
    /// Global synchrony
    pub synchrony: f64,
    /// Steps taken
    pub steps: usize,
}

/// Operation mode for the engine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationMode {
    /// Optimize graph structure
    Optimize,
    /// Discover causal relationships
    CausalDiscovery,
    /// Evolve to attractor
    Attractor,
    /// Coordinated phase operation
    Crystal,
    /// Self-modifying operation
    MetaCognitive,
    /// Grow structure
    Morphogenetic,
    /// All systems active
    Full,
}

/// Maximum event history size to prevent memory exhaustion
const MAX_EVENT_HISTORY: usize = 10_000;

/// The unified Cognitive MinCut Engine
pub struct CognitiveMinCutEngine {
    /// Primary graph being optimized
    graph: DynamicGraph,
    /// Configuration
    config: EngineConfig,
    /// Attractor dynamics subsystem
    attractor: Option<AttractorDynamics>,
    /// Strange loop subsystem
    strange_loop: Option<MetaCognitiveMinCut>,
    /// Causal discovery subsystem
    causal: Option<CausalDiscoverySNN>,
    /// Time crystal CPG subsystem
    time_crystal: Option<TimeCrystalCPG>,
    /// Morphogenetic subsystem
    morphogenetic: Option<MorphogeneticSNN>,
    /// Neural optimizer subsystem
    optimizer: Option<NeuralGraphOptimizer>,
    /// Current simulation time
    time: SimTime,
    /// Accumulated metrics
    metrics: EngineMetrics,
    /// Event history for causal analysis
    event_history: Vec<(GraphEvent, SimTime)>,
    /// Current operation mode
    mode: OperationMode,
}

impl CognitiveMinCutEngine {
    /// Create a new Cognitive MinCut Engine
    pub fn new(graph: DynamicGraph, config: EngineConfig) -> Self {
        let attractor = if config.enable_attractors {
            Some(AttractorDynamics::new(
                graph.clone(),
                config.attractor_config.clone(),
            ))
        } else {
            None
        };

        let strange_loop = if config.enable_strange_loop {
            let mut sl_config = config.strange_loop_config.clone();
            sl_config.level0_size = graph.num_vertices();
            Some(MetaCognitiveMinCut::new(graph.clone(), sl_config))
        } else {
            None
        };

        let causal = if config.enable_causal_discovery {
            Some(CausalDiscoverySNN::new(config.causal_config.clone()))
        } else {
            None
        };

        let time_crystal = if config.enable_time_crystal {
            Some(TimeCrystalCPG::new(
                graph.clone(),
                config.cpg_config.clone(),
            ))
        } else {
            None
        };

        let morphogenetic = if config.enable_morphogenetic {
            Some(MorphogeneticSNN::new(config.morph_config.clone()))
        } else {
            None
        };

        let optimizer = if config.enable_optimizer {
            Some(NeuralGraphOptimizer::new(
                graph.clone(),
                config.optimizer_config.clone(),
            ))
        } else {
            None
        };

        Self {
            graph,
            config,
            attractor,
            strange_loop,
            causal,
            time_crystal,
            morphogenetic,
            optimizer,
            time: 0.0,
            metrics: EngineMetrics::default(),
            event_history: Vec::new(),
            mode: OperationMode::Full,
        }
    }

    /// Set operation mode
    pub fn set_mode(&mut self, mode: OperationMode) {
        self.mode = mode;
    }

    /// Run one integration step
    pub fn step(&mut self) -> Vec<Spike> {
        let start = Instant::now();
        let mut all_spikes = Vec::new();

        match self.mode {
            OperationMode::Optimize => {
                self.step_optimizer(&mut all_spikes);
            }
            OperationMode::CausalDiscovery => {
                self.step_causal();
            }
            OperationMode::Attractor => {
                self.step_attractor(&mut all_spikes);
            }
            OperationMode::Crystal => {
                self.step_time_crystal();
            }
            OperationMode::MetaCognitive => {
                self.step_strange_loop();
            }
            OperationMode::Morphogenetic => {
                self.step_morphogenetic();
            }
            OperationMode::Full => {
                self.step_full(&mut all_spikes);
            }
        }

        self.time += self.config.dt;
        self.metrics.steps += 1;
        self.metrics.total_time += start.elapsed();
        self.metrics.total_spikes += all_spikes.len();

        all_spikes
    }

    /// Full integration step (all subsystems)
    fn step_full(&mut self, all_spikes: &mut Vec<Spike>) {
        // 1. Attractor dynamics (energy landscape)
        self.step_attractor(all_spikes);

        // 2. Strange loop (meta-cognition)
        self.step_strange_loop();

        // 3. Time crystal (coordination)
        self.step_time_crystal();

        // 4. Neural optimizer (learning)
        self.step_optimizer(all_spikes);

        // 5. Causal discovery (from accumulated events)
        self.step_causal();

        // 6. Synchronize graph states
        self.synchronize_graphs();

        // Update energy estimate
        self.metrics.energy_estimate += all_spikes.len() as f64 * 0.001;
    }

    /// Step attractor dynamics
    fn step_attractor(&mut self, spikes: &mut Vec<Spike>) {
        if let Some(ref mut attractor) = self.attractor {
            let start = Instant::now();
            let new_spikes = attractor.step();
            spikes.extend(new_spikes);
            self.metrics.attractor_time += start.elapsed();
            self.metrics.synchrony = attractor.snn().global_synchrony();
        }
    }

    /// Step strange loop
    fn step_strange_loop(&mut self) {
        if let Some(ref mut sl) = self.strange_loop {
            let start = Instant::now();
            let action = sl.strange_loop_step();

            // Record meta-action as event
            let event = match action {
                MetaAction::Strengthen(_) => GraphEvent {
                    event_type: GraphEventType::WeightChange,
                    vertex: None,
                    edge: None,
                    data: 1.0,
                },
                MetaAction::Prune(_) => GraphEvent {
                    event_type: GraphEventType::EdgeDelete,
                    vertex: None,
                    edge: None,
                    data: -1.0,
                },
                MetaAction::Restructure => GraphEvent {
                    event_type: GraphEventType::ComponentMerge,
                    vertex: None,
                    edge: None,
                    data: 0.0,
                },
                MetaAction::NoOp => GraphEvent {
                    event_type: GraphEventType::MinCutChange,
                    vertex: None,
                    edge: None,
                    data: 0.0,
                },
            };

            self.event_history.push((event, self.time));
            self.metrics.strange_loop_time += start.elapsed();
        }
    }

    /// Step causal discovery
    fn step_causal(&mut self) {
        if let Some(ref mut causal) = self.causal {
            let start = Instant::now();

            // Process accumulated events
            for (event, ts) in &self.event_history {
                causal.observe_event(event.clone(), *ts);
            }
            self.event_history.clear();

            self.metrics.causal_time += start.elapsed();
        }
    }

    /// Step time crystal CPG
    fn step_time_crystal(&mut self) {
        if let Some(ref mut cpg) = self.time_crystal {
            let start = Instant::now();
            let _ = cpg.tick();
            self.metrics.cpg_time += start.elapsed();
        }
    }

    /// Step morphogenetic development
    fn step_morphogenetic(&mut self) {
        if let Some(ref mut morph) = self.morphogenetic {
            let start = Instant::now();
            morph.develop_step();
            self.metrics.morph_time += start.elapsed();
        }
    }

    /// Step neural optimizer
    fn step_optimizer(&mut self, _spikes: &mut Vec<Spike>) {
        if let Some(ref mut opt) = self.optimizer {
            let start = Instant::now();
            let result = opt.optimize_step();

            // Record optimization action as event
            let event = match result.action {
                GraphAction::AddEdge(u, v, _) => GraphEvent {
                    event_type: GraphEventType::EdgeInsert,
                    vertex: None,
                    edge: Some((u, v)),
                    data: result.reward,
                },
                GraphAction::RemoveEdge(u, v) => GraphEvent {
                    event_type: GraphEventType::EdgeDelete,
                    vertex: None,
                    edge: Some((u, v)),
                    data: result.reward,
                },
                _ => GraphEvent {
                    event_type: GraphEventType::WeightChange,
                    vertex: None,
                    edge: None,
                    data: result.reward,
                },
            };

            self.event_history.push((event, self.time));
            self.metrics.mincut_value = result.new_mincut;
            self.metrics.optimizer_time += start.elapsed();
        }
    }

    /// Synchronize graph states across subsystems
    fn synchronize_graphs(&mut self) {
        // Use optimizer's graph as primary
        if let Some(ref opt) = self.optimizer {
            self.graph = opt.graph().clone();
        }

        // Update attractor subsystem with current graph state
        if let Some(ref mut attractor) = self.attractor {
            // Create new attractor dynamics with updated graph
            // This preserves configuration while syncing graph
            *attractor = AttractorDynamics::new(self.graph.clone(), attractor.config().clone());
        }

        // Limit event history size to prevent memory exhaustion
        if self.event_history.len() > MAX_EVENT_HISTORY {
            // Keep only the most recent events
            let drain_count = self.event_history.len() - MAX_EVENT_HISTORY;
            self.event_history.drain(..drain_count);
        }
    }

    /// Run for specified number of steps
    pub fn run(&mut self, steps: usize) -> Vec<Spike> {
        let mut all_spikes = Vec::new();

        for _ in 0..steps {
            let spikes = self.step();
            all_spikes.extend(spikes);
        }

        all_spikes
    }

    /// Run until convergence or max steps
    pub fn run_until_converged(&mut self) -> (Vec<Spike>, bool) {
        let mut all_spikes = Vec::new();
        let mut prev_energy = f64::MAX;
        let mut stable_count = 0;

        for _ in 0..self.config.max_steps {
            let spikes = self.step();
            all_spikes.extend(spikes);

            let energy = self.current_energy();
            if (energy - prev_energy).abs() < 0.001 {
                stable_count += 1;
                if stable_count > 10 {
                    return (all_spikes, true);
                }
            } else {
                stable_count = 0;
            }
            prev_energy = energy;
        }

        (all_spikes, false)
    }

    /// Get current energy
    pub fn current_energy(&self) -> f64 {
        if let Some(ref attractor) = self.attractor {
            attractor.energy()
        } else {
            -self.metrics.mincut_value - self.metrics.synchrony
        }
    }

    /// Get primary graph
    pub fn graph(&self) -> &DynamicGraph {
        &self.graph
    }

    /// Get mutable graph
    pub fn graph_mut(&mut self) -> &mut DynamicGraph {
        &mut self.graph
    }

    /// Get metrics
    pub fn metrics(&self) -> &EngineMetrics {
        &self.metrics
    }

    /// Get causal graph (if available)
    pub fn causal_graph(&self) -> Option<CausalGraph> {
        self.causal.as_ref().map(|c| c.extract_causal_graph())
    }

    /// Get current phase (from time crystal)
    pub fn current_phase(&self) -> Option<usize> {
        self.time_crystal.as_ref().map(|tc| tc.current_phase())
    }

    /// Get attractor status
    pub fn at_attractor(&self) -> bool {
        self.attractor
            .as_ref()
            .map(|a| a.reached_attractor())
            .unwrap_or(false)
    }

    /// Get morphogenetic pattern
    pub fn pattern(&self) -> Option<TuringPattern> {
        self.morphogenetic.as_ref().map(|m| m.detect_pattern())
    }

    /// Get energy landscape
    pub fn energy_landscape(&self) -> Option<&EnergyLandscape> {
        self.attractor.as_ref().map(|a| a.energy_landscape())
    }

    /// Subpolynomial search exploiting all learned structures
    pub fn search(&self, query: &[f64], k: usize) -> Vec<VertexId> {
        // Combine skip regions from all subsystems
        let mut skip_regions = Vec::new();

        // From attractor dynamics
        if let Some(ref attractor) = self.attractor {
            let skip = attractor.get_skip_mask();
            skip_regions.extend(skip.iter().map(|(u, v)| (*u, *v)));
        }

        // From neural optimizer
        if let Some(ref opt) = self.optimizer {
            return opt.search(query, k);
        }

        // Fallback: return all vertices
        self.graph.vertices().into_iter().take(k).collect()
    }

    /// Record external event for causal analysis
    pub fn record_event(&mut self, event: GraphEvent) {
        self.event_history.push((event, self.time));
    }

    /// Reset all subsystems
    pub fn reset(&mut self) {
        self.time = 0.0;
        self.metrics = EngineMetrics::default();
        self.event_history.clear();

        if let Some(ref mut attractor) = self.attractor {
            attractor.reset();
        }
        if let Some(ref mut sl) = self.strange_loop {
            sl.reset();
        }
        if let Some(ref mut causal) = self.causal {
            causal.reset();
        }
        if let Some(ref mut cpg) = self.time_crystal {
            cpg.reset();
        }
        if let Some(ref mut morph) = self.morphogenetic {
            morph.reset();
        }
        if let Some(ref mut opt) = self.optimizer {
            opt.reset();
        }
    }

    /// Get summary of engine state
    pub fn summary(&self) -> EngineSummary {
        EngineSummary {
            mode: self.mode,
            time: self.time,
            graph_vertices: self.graph.num_vertices(),
            graph_edges: self.graph.num_edges(),
            mincut: self.metrics.mincut_value,
            synchrony: self.metrics.synchrony,
            at_attractor: self.at_attractor(),
            current_phase: self.current_phase(),
            pattern: self.pattern(),
            total_spikes: self.metrics.total_spikes,
            energy: self.metrics.energy_estimate,
        }
    }
}

/// Summary of engine state
#[derive(Debug, Clone)]
pub struct EngineSummary {
    /// Current operation mode
    pub mode: OperationMode,
    /// Simulation time
    pub time: SimTime,
    /// Number of graph vertices
    pub graph_vertices: usize,
    /// Number of graph edges
    pub graph_edges: usize,
    /// Current mincut value
    pub mincut: f64,
    /// Global synchrony
    pub synchrony: f64,
    /// At attractor?
    pub at_attractor: bool,
    /// Current time crystal phase
    pub current_phase: Option<usize>,
    /// Morphogenetic pattern
    pub pattern: Option<TuringPattern>,
    /// Total spikes generated
    pub total_spikes: usize,
    /// Energy estimate
    pub energy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> DynamicGraph {
        let graph = DynamicGraph::new();
        for i in 0..10 {
            graph.insert_edge(i, (i + 1) % 10, 1.0).unwrap();
        }
        graph
    }

    #[test]
    fn test_engine_creation() {
        let graph = create_test_graph();
        let config = EngineConfig::default();

        let engine = CognitiveMinCutEngine::new(graph, config);

        assert_eq!(engine.graph().num_vertices(), 10);
    }

    #[test]
    fn test_engine_step() {
        let graph = create_test_graph();
        let config = EngineConfig::default();

        let mut engine = CognitiveMinCutEngine::new(graph, config);
        engine.set_mode(OperationMode::Optimize);

        let spikes = engine.step();
        assert!(engine.metrics().steps == 1);
    }

    #[test]
    fn test_engine_run() {
        let graph = create_test_graph();
        let mut config = EngineConfig::default();
        config.enable_morphogenetic = false; // Expensive

        let mut engine = CognitiveMinCutEngine::new(graph, config);

        let spikes = engine.run(10);
        assert_eq!(engine.metrics().steps, 10);
    }

    #[test]
    fn test_engine_modes() {
        let graph = create_test_graph();
        let config = EngineConfig::default();

        let mut engine = CognitiveMinCutEngine::new(graph, config);

        // Test each mode
        for mode in [
            OperationMode::Optimize,
            OperationMode::CausalDiscovery,
            OperationMode::Attractor,
            OperationMode::Crystal,
            OperationMode::MetaCognitive,
        ] {
            engine.set_mode(mode);
            engine.step();
        }
    }

    #[test]
    fn test_engine_summary() {
        let graph = create_test_graph();
        let config = EngineConfig::default();

        let mut engine = CognitiveMinCutEngine::new(graph, config);
        engine.run(5);

        let summary = engine.summary();
        assert_eq!(summary.graph_vertices, 10);
        assert!(summary.time > 0.0);
    }

    #[test]
    fn test_record_event() {
        let graph = create_test_graph();
        let config = EngineConfig::default();

        let mut engine = CognitiveMinCutEngine::new(graph, config);

        engine.record_event(GraphEvent {
            event_type: GraphEventType::EdgeInsert,
            vertex: None,
            edge: Some((0, 5)),
            data: 1.0,
        });

        engine.step();
    }
}
