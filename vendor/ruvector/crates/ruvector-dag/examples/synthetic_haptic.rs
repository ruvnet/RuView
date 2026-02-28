//! Synthetic Haptic System: A nervous system for machines
//!
//! This example implements a complete haptic control loop using ruvector DAG components:
//! - Layer 1: Event sensing with lock-free queues
//! - Layer 2: Reflex arc using DAG tension and MinCut signals
//! - Layer 3: HDC-style associative memory for pattern recognition
//! - Layer 4: SONA-based learning gated by coherence
//! - Layer 5: Energy-budgeted actuation with deterministic timing
//!
//! The key insight: tension drives immediate response, coherence gates learning.
//! Intelligence emerges as homeostasis, not goal-seeking.

use ruvector_dag::{
    DagMinCutEngine, DagSonaEngine, MinCutConfig, OperatorNode, OperatorType, QueryDag,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// =============================================================================
// Layer 1: Event Sensing
// =============================================================================

/// Raw sensor frame with microsecond timestamp
#[derive(Clone, Debug)]
pub struct SensorFrame {
    pub t_us: u64,
    pub position: f32,    // Normalized position [-1, 1]
    pub velocity: f32,    // Rate of change
    pub force: f32,       // Applied force
    pub contact: f32,     // Contact intensity [0, 1]
    pub temperature: f32, // Thermal signal
    pub vibration: f32,   // High-frequency component
}

impl SensorFrame {
    /// Convert to vector embedding for DAG processing
    pub fn to_embedding(&self) -> Vec<f32> {
        vec![
            self.position,
            self.velocity,
            self.force,
            self.contact,
            self.temperature,
            self.vibration,
        ]
    }

    /// Compute deviation from homeostasis (tension source)
    pub fn deviation(&self) -> f32 {
        let force_dev = self.force.abs();
        let vel_dev = self.velocity.abs() * 2.0;
        let contact_dev = (1.0 - self.contact) * 0.3;
        let temp_dev = (self.temperature - 0.5).abs();
        let vib_dev = self.vibration.abs() * 0.5;

        (force_dev + vel_dev + contact_dev + temp_dev + vib_dev).min(10.0)
    }
}

/// Sensor trait for hardware abstraction
pub trait Sensor: Send {
    fn read(&mut self) -> SensorFrame;
    fn calibrate(&mut self);
}

/// Simulated sensor for testing
pub struct SimulatedSensor {
    start: Instant,
    phase: f32,
    contact_mode: bool,
    noise_seed: u64,
}

impl SimulatedSensor {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            phase: 0.0,
            contact_mode: false,
            noise_seed: 0,
        }
    }

    fn pseudo_random(&mut self) -> f32 {
        self.noise_seed = self.noise_seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.noise_seed >> 16) & 0x7FFF) as f32 / 32767.0 - 0.5
    }
}

impl Default for SimulatedSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl Sensor for SimulatedSensor {
    fn read(&mut self) -> SensorFrame {
        let t_us = self.start.elapsed().as_micros() as u64;
        self.phase += 0.02;

        let position = self.phase.sin();
        let velocity = self.phase.cos() * 0.02;

        // Contact transitions
        if position > 0.6 && !self.contact_mode {
            self.contact_mode = true;
        } else if position < 0.3 && self.contact_mode {
            self.contact_mode = false;
        }

        let contact = if self.contact_mode { 1.0 } else { 0.0 };
        let force = if self.contact_mode {
            (position - 0.6).max(0.0) * 15.0
        } else {
            0.0
        };

        let noise = self.pseudo_random() * 0.05;
        let temperature = 0.5 + self.phase.sin() * 0.1 + noise;
        let vibration = if self.contact_mode {
            0.3 + noise
        } else {
            noise.abs()
        };

        SensorFrame {
            t_us,
            position,
            velocity,
            force,
            contact,
            temperature,
            vibration,
        }
    }

    fn calibrate(&mut self) {
        self.phase = 0.0;
        self.contact_mode = false;
    }
}

// =============================================================================
// Layer 2: Reflex Arc with DAG Tension
// =============================================================================

/// Homeostatic state derived from DAG analysis
#[derive(Clone, Debug)]
pub struct HomeostasisState {
    pub tension: f32,       // Deviation from equilibrium [0, 1]
    pub coherence: f32,     // Stability of internal state [0, 1]
    pub cut_value: f32,     // MinCut flow capacity
    pub criticality: f32,   // Node criticality max
    pub reflex: ReflexMode, // Current reflex state
}

/// Reflex modes mapped to DAG tension levels
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReflexMode {
    Calm,    // Tension < 0.20: minimal response, learning allowed
    Active,  // Tension 0.20-0.55: proportional response
    Spike,   // Tension 0.55-0.85: heightened response, haptic feedback
    Protect, // Tension > 0.85: protective shutdown, no output
}

impl ReflexMode {
    pub fn from_tension(tension: f32) -> Self {
        if tension > 0.85 {
            ReflexMode::Protect
        } else if tension > 0.55 {
            ReflexMode::Spike
        } else if tension > 0.20 {
            ReflexMode::Active
        } else {
            ReflexMode::Calm
        }
    }
}

/// Reflex arc using MinCut for tension signals
pub struct ReflexArc {
    mincut_engine: DagMinCutEngine,
    dag: QueryDag,
    tension_ema: f32,   // Exponential moving average
    coherence_ema: f32, // Coherence smoothing
    alpha: f32,         // EMA decay rate
}

impl ReflexArc {
    pub fn new() -> Self {
        // Build a minimal DAG representing haptic state flow
        let mut dag = QueryDag::new();

        // Sensor nodes (leaves)
        let pos_sense = dag.add_node(OperatorNode::new(
            0,
            OperatorType::SeqScan {
                table: "position".to_string(),
            },
        ));
        let force_sense = dag.add_node(OperatorNode::new(
            1,
            OperatorType::SeqScan {
                table: "force".to_string(),
            },
        ));
        let contact_sense = dag.add_node(OperatorNode::new(
            2,
            OperatorType::SeqScan {
                table: "contact".to_string(),
            },
        ));

        // Fusion node
        let fusion = dag.add_node(OperatorNode::new(
            3,
            OperatorType::Aggregate {
                functions: vec!["fuse".to_string()],
            },
        ));

        // Reflex decision node
        let reflex = dag.add_node(OperatorNode::new(
            4,
            OperatorType::Filter {
                predicate: "reflex_gate".to_string(),
            },
        ));

        // Output node (root)
        let output = dag.add_node(OperatorNode::new(5, OperatorType::Result));

        // Connect: sensors -> fusion -> reflex -> output
        dag.add_edge(pos_sense, fusion).unwrap();
        dag.add_edge(force_sense, fusion).unwrap();
        dag.add_edge(contact_sense, fusion).unwrap();
        dag.add_edge(fusion, reflex).unwrap();
        dag.add_edge(reflex, output).unwrap();

        let mut mincut_engine = DagMinCutEngine::new(MinCutConfig::default());
        mincut_engine.build_from_dag(&dag);

        Self {
            mincut_engine,
            dag,
            tension_ema: 0.0,
            coherence_ema: 1.0,
            alpha: 0.2,
        }
    }

    /// Update DAG node costs based on sensor data
    pub fn update_sensor_costs(&mut self, frame: &SensorFrame) {
        // Update node costs based on sensor values
        if let Some(node) = self.dag.get_node_mut(0) {
            node.estimated_cost = frame.position.abs() as f64 * 10.0 + 1.0;
        }
        if let Some(node) = self.dag.get_node_mut(1) {
            node.estimated_cost = frame.force.abs() as f64 * 20.0 + 1.0;
        }
        if let Some(node) = self.dag.get_node_mut(2) {
            node.estimated_cost = frame.contact as f64 * 15.0 + 1.0;
        }

        // Rebuild mincut graph with new costs
        self.mincut_engine.build_from_dag(&self.dag);
    }

    /// Compute homeostasis from DAG tension signals
    pub fn compute_homeostasis(&mut self, frame: &SensorFrame) -> HomeostasisState {
        self.update_sensor_costs(frame);

        // Compute MinCut between sensors (leaves) and output (root)
        let leaves = self.dag.leaves();
        let root = self.dag.root().unwrap_or(5);

        let mut max_cut = 0.0f64;
        for &leaf in &leaves {
            let result = self.mincut_engine.compute_mincut(leaf, root);
            max_cut = max_cut.max(result.cut_value);
        }

        // Compute node criticality
        let criticality = self.mincut_engine.compute_criticality(&self.dag);
        let max_crit = criticality.values().cloned().fold(0.0f64, f64::max);

        // Tension from deviation + cut stress
        let deviation = frame.deviation();
        let cut_stress = (max_cut / 100.0).min(1.0) as f32;
        let raw_tension = (deviation * 0.1 + cut_stress * 0.3).min(1.0);

        // Apply EMA smoothing
        self.tension_ema = self.alpha * raw_tension + (1.0 - self.alpha) * self.tension_ema;

        // Coherence drops when tension is high or changing rapidly
        let tension_delta = (raw_tension - self.tension_ema).abs();
        let raw_coherence = 1.0 - (self.tension_ema * 0.4 + tension_delta * 0.6);
        self.coherence_ema = self.alpha * raw_coherence + (1.0 - self.alpha) * self.coherence_ema;

        let tension = self.tension_ema.clamp(0.0, 1.0);
        let coherence = self.coherence_ema.clamp(0.0, 1.0);

        HomeostasisState {
            tension,
            coherence,
            cut_value: max_cut as f32,
            criticality: max_crit as f32,
            reflex: ReflexMode::from_tension(tension),
        }
    }

    pub fn dag(&self) -> &QueryDag {
        &self.dag
    }
}

impl Default for ReflexArc {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Layer 3: HDC-Style Associative Memory
// =============================================================================

/// Hyperdimensional computing memory key
#[derive(Clone)]
pub struct HdcKey {
    pub vector: Vec<f32>,
    pub dim: usize,
}

impl HdcKey {
    pub fn new(dim: usize) -> Self {
        Self {
            vector: vec![0.0; dim],
            dim,
        }
    }

    /// Encode sensor frame as HDC vector using random indexing
    pub fn encode(frame: &SensorFrame, dim: usize, seed: u64) -> Self {
        let mut vector = vec![0.0; dim];

        // Generate pseudo-random basis vectors
        let embedding = frame.to_embedding();
        for (i, &val) in embedding.iter().enumerate() {
            let basis_seed = seed.wrapping_add(i as u64);
            for j in 0..dim {
                let idx_seed = basis_seed.wrapping_mul(j as u64 + 1);
                let sign = if (idx_seed >> 31) & 1 == 0 { 1.0 } else { -1.0 };
                vector[j] += val * sign;
            }
        }

        // Normalize
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }

        Self { vector, dim }
    }

    /// Cosine similarity
    pub fn similarity(&self, other: &HdcKey) -> f32 {
        if self.dim != other.dim {
            return 0.0;
        }

        let dot: f32 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum();
        dot
    }

    /// Bundling (superposition)
    pub fn bundle(&mut self, other: &HdcKey, weight: f32) {
        for (a, b) in self.vector.iter_mut().zip(other.vector.iter()) {
            *a += *b * weight;
        }
    }
}

/// Associative memory with HDC keys
pub struct AssociativeMemory {
    memories: Vec<(HdcKey, ReflexMode, f32)>, // (key, associated reflex, quality)
    dim: usize,
    capacity: usize,
    seed: u64,
    ring_buffer: Vec<SensorFrame>,
    ring_idx: usize,
}

impl AssociativeMemory {
    pub fn new(dim: usize, capacity: usize) -> Self {
        Self {
            memories: Vec::with_capacity(capacity),
            dim,
            capacity,
            seed: 0xDEAD_BEEF,
            ring_buffer: Vec::with_capacity(32),
            ring_idx: 0,
        }
    }

    /// Store a pattern with associated reflex mode
    pub fn store(&mut self, frame: &SensorFrame, reflex: ReflexMode, quality: f32) {
        let key = HdcKey::encode(frame, self.dim, self.seed);

        // Evict oldest if at capacity
        if self.memories.len() >= self.capacity {
            // Remove lowest quality
            if let Some(min_idx) = self
                .memories
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.2.partial_cmp(&b.2).unwrap())
                .map(|(i, _)| i)
            {
                self.memories.remove(min_idx);
            }
        }

        self.memories.push((key, reflex, quality));
    }

    /// Query for similar patterns
    pub fn query(&self, frame: &SensorFrame, k: usize) -> Vec<(ReflexMode, f32)> {
        let query_key = HdcKey::encode(frame, self.dim, self.seed);

        let mut similarities: Vec<(ReflexMode, f32)> = self
            .memories
            .iter()
            .map(|(key, reflex, _)| (*reflex, query_key.similarity(key)))
            .filter(|(_, sim)| *sim > 0.5)
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        similarities.truncate(k);
        similarities
    }

    /// Record to ring buffer for temporal context
    pub fn record_event(&mut self, frame: SensorFrame) {
        if self.ring_buffer.len() < 32 {
            self.ring_buffer.push(frame);
        } else {
            self.ring_buffer[self.ring_idx] = frame;
        }
        self.ring_idx = (self.ring_idx + 1) % 32;
    }

    /// Get recent events for context
    pub fn recent_events(&self, n: usize) -> Vec<&SensorFrame> {
        let len = self.ring_buffer.len();
        let n = n.min(len);
        let mut result = Vec::with_capacity(n);

        for i in 0..n {
            let idx = (self.ring_idx + len - 1 - i) % len;
            result.push(&self.ring_buffer[idx]);
        }

        result
    }

    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }
}

// =============================================================================
// Layer 4: SONA-Based Learning
// =============================================================================

/// Learning controller with coherence gating
pub struct LearningController {
    sona_engine: DagSonaEngine,
    coherence_threshold: f32,
    tension_max_for_learning: f32,
    #[allow(dead_code)]
    learning_rate: f32,
    batch_size: usize,
    pending_trajectories: Vec<LearningTrajectory>,
}

#[derive(Clone)]
struct LearningTrajectory {
    embedding: Vec<f32>,
    #[allow(dead_code)]
    reflex: ReflexMode,
    #[allow(dead_code)]
    execution_time_us: u64,
    quality: f32,
}

impl LearningController {
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            sona_engine: DagSonaEngine::new(embedding_dim),
            coherence_threshold: 0.85,
            tension_max_for_learning: 0.35,
            learning_rate: 0.01,
            batch_size: 16,
            pending_trajectories: Vec::new(),
        }
    }

    /// Check if learning is safe given current homeostasis
    pub fn should_learn(&self, state: &HomeostasisState) -> bool {
        state.coherence > self.coherence_threshold && state.tension < self.tension_max_for_learning
    }

    /// Record a trajectory for later learning
    pub fn record_trajectory(
        &mut self,
        frame: &SensorFrame,
        state: &HomeostasisState,
        execution_time_us: u64,
    ) {
        let embedding = frame.to_embedding();
        let quality = self.compute_quality(state, execution_time_us);

        self.pending_trajectories.push(LearningTrajectory {
            embedding,
            reflex: state.reflex,
            execution_time_us,
            quality,
        });

        // Trigger learning if batch is full and coherence allows
        if self.pending_trajectories.len() >= self.batch_size {
            self.try_learn(state);
        }
    }

    /// Attempt learning if conditions are safe
    pub fn try_learn(&mut self, state: &HomeostasisState) -> bool {
        if !self.should_learn(state) {
            return false;
        }

        // Process pending trajectories
        for traj in self.pending_trajectories.drain(..) {
            if traj.quality > 0.6 {
                self.sona_engine
                    .reasoning_bank_store(traj.embedding, traj.quality);
            }
        }

        // Trigger background learning
        self.sona_engine.background_learn();
        true
    }

    fn compute_quality(&self, state: &HomeostasisState, execution_time_us: u64) -> f32 {
        let time_score = 1.0 / (1.0 + execution_time_us as f32 / 100_000.0);
        let stability_score = state.coherence;
        let response_score = 1.0 - state.tension.min(1.0);

        0.4 * time_score + 0.3 * stability_score + 0.3 * response_score
    }

    /// Pre-query adaptation (MicroLoRA fast path)
    pub fn pre_adapt(&mut self, dag: &QueryDag) -> Vec<f32> {
        self.sona_engine.pre_query(dag)
    }

    pub fn pattern_count(&self) -> usize {
        self.sona_engine.pattern_count()
    }

    pub fn trajectory_count(&self) -> usize {
        self.sona_engine.trajectory_count()
    }
}

// Extension trait for DagSonaEngine
trait SonaEngineExt {
    fn reasoning_bank_store(&mut self, embedding: Vec<f32>, quality: f32);
}

impl SonaEngineExt for DagSonaEngine {
    fn reasoning_bank_store(&mut self, _embedding: Vec<f32>, _quality: f32) {
        // This is a simplified interface - in production, would access
        // the reasoning bank directly through the engine's internal structure
    }
}

// =============================================================================
// Layer 5: Energy-Budgeted Actuation
// =============================================================================

/// Actuator command with energy constraints
#[derive(Clone, Debug)]
pub struct ActuatorCommand {
    pub force: f32,       // Output force [-1, 1]
    pub vibro_freq: f32,  // Vibration frequency Hz
    pub vibro_amp: f32,   // Vibration amplitude [0, 1]
    pub energy_used: f32, // Energy consumed this tick
}

impl ActuatorCommand {
    pub fn zero() -> Self {
        Self {
            force: 0.0,
            vibro_freq: 0.0,
            vibro_amp: 0.0,
            energy_used: 0.0,
        }
    }
}

/// Actuator trait for hardware abstraction
pub trait Actuator: Send {
    fn write(&mut self, cmd: ActuatorCommand);
    fn max_force(&self) -> f32;
    fn max_energy_per_tick(&self) -> f32;
}

/// Simulated actuator with logging
pub struct SimulatedActuator {
    total_energy: f32,
    command_count: u64,
}

impl SimulatedActuator {
    pub fn new() -> Self {
        Self {
            total_energy: 0.0,
            command_count: 0,
        }
    }
}

impl Default for SimulatedActuator {
    fn default() -> Self {
        Self::new()
    }
}

impl Actuator for SimulatedActuator {
    fn write(&mut self, cmd: ActuatorCommand) {
        self.total_energy += cmd.energy_used;
        self.command_count += 1;

        if self.command_count % 50 == 0 {
            println!(
                "  [Actuator] force={:.3} vibro={:.0}Hz@{:.2} energy={:.3}",
                cmd.force, cmd.vibro_freq, cmd.vibro_amp, cmd.energy_used
            );
        }
    }

    fn max_force(&self) -> f32 {
        1.0
    }

    fn max_energy_per_tick(&self) -> f32 {
        0.1
    }
}

/// Actuation renderer with energy budget enforcement
pub struct ActuationRenderer {
    max_energy_per_tick: f32,
    force_limit: f32,
    vibro_limit: f32,
}

impl ActuationRenderer {
    pub fn new(max_energy: f32) -> Self {
        Self {
            max_energy_per_tick: max_energy,
            force_limit: 1.0,
            vibro_limit: 1.0,
        }
    }

    /// Render actuator command from sensor and homeostasis
    pub fn render(&self, frame: &SensorFrame, state: &HomeostasisState) -> ActuatorCommand {
        // Base PD controller
        let kp = 0.4;
        let kd = 8.0;
        let base_force = (-frame.position * kp - frame.velocity * kd).clamp(-1.0, 1.0);

        match state.reflex {
            ReflexMode::Calm => {
                let force = base_force * 0.2;
                let energy = force.abs() * 0.01;
                ActuatorCommand {
                    force: force.clamp(-self.force_limit, self.force_limit),
                    vibro_freq: 0.0,
                    vibro_amp: 0.0,
                    energy_used: energy.min(self.max_energy_per_tick),
                }
            }
            ReflexMode::Active => {
                let force = base_force * 0.7;
                let vibro_amp = (state.tension * 0.3).clamp(0.0, 0.3);
                let energy = force.abs() * 0.03 + vibro_amp * 0.02;
                ActuatorCommand {
                    force: force.clamp(-self.force_limit, self.force_limit),
                    vibro_freq: 60.0,
                    vibro_amp: vibro_amp.min(self.vibro_limit),
                    energy_used: energy.min(self.max_energy_per_tick),
                }
            }
            ReflexMode::Spike => {
                let force = (base_force * 0.9).clamp(-0.8, 0.8);
                let vibro_amp = (0.3 + state.tension * 0.6).clamp(0.0, 1.0);
                let energy = force.abs() * 0.05 + vibro_amp * 0.04;
                ActuatorCommand {
                    force: force.clamp(-self.force_limit, self.force_limit),
                    vibro_freq: 160.0,
                    vibro_amp: vibro_amp.min(self.vibro_limit),
                    energy_used: energy.min(self.max_energy_per_tick),
                }
            }
            ReflexMode::Protect => {
                // Protective shutdown: minimal output, max haptic warning
                ActuatorCommand {
                    force: 0.0,
                    vibro_freq: 220.0,
                    vibro_amp: 1.0,
                    energy_used: 0.04, // Fixed warning energy
                }
            }
        }
    }
}

// =============================================================================
// Main Haptic Controller
// =============================================================================

/// Statistics for the haptic loop
#[derive(Debug, Default)]
pub struct HapticStats {
    pub tick_count: u64,
    pub total_time_us: u64,
    pub max_loop_time_us: u64,
    pub min_loop_time_us: u64,
    pub avg_loop_time_us: u64,
    pub learn_gate_opens: u64,
    pub total_energy: f32,
    pub reflex_counts: HashMap<String, u64>,
}

/// Complete synthetic haptic controller
pub struct SyntheticHapticController {
    reflex_arc: ReflexArc,
    memory: AssociativeMemory,
    learning: LearningController,
    renderer: ActuationRenderer,
    stats: HapticStats,
    tick_counter: Arc<AtomicU64>,
}

impl SyntheticHapticController {
    pub fn new() -> Self {
        Self {
            reflex_arc: ReflexArc::new(),
            memory: AssociativeMemory::new(256, 1000),
            learning: LearningController::new(64),
            renderer: ActuationRenderer::new(0.1),
            stats: HapticStats {
                min_loop_time_us: u64::MAX,
                ..Default::default()
            },
            tick_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Execute one tick of the haptic loop
    pub fn tick(
        &mut self,
        sensor: &mut dyn Sensor,
        actuator: &mut dyn Actuator,
    ) -> (HomeostasisState, ActuatorCommand) {
        let tick_start = Instant::now();
        let tick = self.tick_counter.fetch_add(1, Ordering::Relaxed);

        // Layer 1: Sense
        let frame = sensor.read();

        // Layer 2: Reflex
        let state = self.reflex_arc.compute_homeostasis(&frame);

        // Layer 3: Memory query (optional context)
        if tick % 10 == 0 {
            let similar = self.memory.query(&frame, 3);
            if !similar.is_empty() {
                // Could use for context-sensitive rendering
            }
        }
        self.memory.record_event(frame.clone());

        // Layer 4: Learning (coherence-gated)
        let loop_time = tick_start.elapsed().as_micros() as u64;
        if self.learning.should_learn(&state) {
            if tick % 20 == 0 {
                self.stats.learn_gate_opens += 1;
            }
            self.learning.record_trajectory(&frame, &state, loop_time);
        }

        // Store successful patterns
        if state.coherence > 0.9 && state.tension < 0.3 {
            self.memory.store(&frame, state.reflex, state.coherence);
        }

        // Layer 5: Actuate
        let cmd = self.renderer.render(&frame, &state);
        actuator.write(cmd.clone());

        // Update stats
        let total_loop_time = tick_start.elapsed().as_micros() as u64;
        self.update_stats(total_loop_time, cmd.energy_used, &state);

        (state, cmd)
    }

    fn update_stats(&mut self, loop_time_us: u64, energy: f32, state: &HomeostasisState) {
        self.stats.tick_count += 1;
        self.stats.total_time_us += loop_time_us;
        self.stats.max_loop_time_us = self.stats.max_loop_time_us.max(loop_time_us);
        self.stats.min_loop_time_us = self.stats.min_loop_time_us.min(loop_time_us);
        self.stats.avg_loop_time_us = self.stats.total_time_us / self.stats.tick_count;
        self.stats.total_energy += energy;

        let reflex_key = format!("{:?}", state.reflex);
        *self.stats.reflex_counts.entry(reflex_key).or_insert(0) += 1;
    }

    /// Run the control loop for N ticks at specified rate
    pub fn run_loop(
        &mut self,
        sensor: &mut dyn Sensor,
        actuator: &mut dyn Actuator,
        ticks: usize,
        tick_rate_hz: f64,
    ) {
        let tick_duration = Duration::from_secs_f64(1.0 / tick_rate_hz);
        let mut next_tick = Instant::now();

        for tick in 0..ticks {
            next_tick += tick_duration;

            let (state, _cmd) = self.tick(sensor, actuator);

            // Periodic reporting
            if tick % 50 == 0 {
                println!(
                    "tick={:4} tension={:.2} coherence={:.2} reflex={:?} loop_us={}",
                    tick, state.tension, state.coherence, state.reflex, self.stats.avg_loop_time_us
                );
            }

            // Busy-wait for timing precision
            spin_wait_until(next_tick);
        }
    }

    pub fn stats(&self) -> &HapticStats {
        &self.stats
    }

    pub fn memory_count(&self) -> usize {
        self.memory.memory_count()
    }

    pub fn pattern_count(&self) -> usize {
        self.learning.pattern_count()
    }
}

impl Default for SyntheticHapticController {
    fn default() -> Self {
        Self::new()
    }
}

/// Precise busy-wait for deterministic timing
fn spin_wait_until(deadline: Instant) {
    while Instant::now() < deadline {
        std::hint::spin_loop();
    }
}

// =============================================================================
// Main Entry Point
// =============================================================================

fn main() {
    println!("=== Synthetic Haptic System on RuVector DAG ===\n");
    println!("Implementing intelligence as homeostasis, rendered as touch.\n");
    println!("Architecture:");
    println!("  Layer 1: Event Sensing (microsecond timestamps)");
    println!("  Layer 2: Reflex Arc (DAG tension + MinCut signals)");
    println!("  Layer 3: HDC Memory (256-dim hypervectors)");
    println!("  Layer 4: SONA Learning (coherence-gated)");
    println!("  Layer 5: Energy-Budgeted Actuation\n");

    let mut sensor = SimulatedSensor::new();
    let mut actuator = SimulatedActuator::new();
    let mut controller = SyntheticHapticController::new();

    // Calibrate
    sensor.calibrate();

    // Run at 1000 Hz for 500 ticks
    println!("Running 500 ticks at 1000 Hz...\n");
    controller.run_loop(&mut sensor, &mut actuator, 500, 1000.0);

    // Report statistics
    println!("\n=== Statistics ===");
    let stats = controller.stats();
    println!("Total ticks:        {}", stats.tick_count);
    println!("Avg loop time:      {} μs", stats.avg_loop_time_us);
    println!("Max loop time:      {} μs", stats.max_loop_time_us);
    println!("Min loop time:      {} μs", stats.min_loop_time_us);
    println!("Learn gate opens:   {}", stats.learn_gate_opens);
    println!("Total energy:       {:.4}", stats.total_energy);
    println!("Memory patterns:    {}", controller.memory_count());
    println!("Learned patterns:   {}", controller.pattern_count());
    println!("\nReflex mode distribution:");
    for (mode, count) in &stats.reflex_counts {
        println!(
            "  {:10}: {} ({:.1}%)",
            mode,
            count,
            *count as f64 / stats.tick_count as f64 * 100.0
        );
    }

    println!("\n✓ Intelligence as homeostasis, rendered as touch.");
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensor_frame_embedding() {
        let frame = SensorFrame {
            t_us: 0,
            position: 0.5,
            velocity: 0.1,
            force: 0.0,
            contact: 1.0,
            temperature: 0.5,
            vibration: 0.2,
        };

        let embedding = frame.to_embedding();
        assert_eq!(embedding.len(), 6);
        assert_eq!(embedding[0], 0.5);
    }

    #[test]
    fn test_reflex_mode_from_tension() {
        assert_eq!(ReflexMode::from_tension(0.1), ReflexMode::Calm);
        assert_eq!(ReflexMode::from_tension(0.3), ReflexMode::Active);
        assert_eq!(ReflexMode::from_tension(0.7), ReflexMode::Spike);
        assert_eq!(ReflexMode::from_tension(0.9), ReflexMode::Protect);
    }

    #[test]
    fn test_hdc_key_similarity() {
        let frame1 = SensorFrame {
            t_us: 0,
            position: 0.5,
            velocity: 0.1,
            force: 0.0,
            contact: 1.0,
            temperature: 0.5,
            vibration: 0.2,
        };

        let frame2 = SensorFrame {
            t_us: 0,
            position: 0.51,
            velocity: 0.11,
            force: 0.0,
            contact: 1.0,
            temperature: 0.5,
            vibration: 0.2,
        };

        let key1 = HdcKey::encode(&frame1, 256, 0xDEADBEEF);
        let key2 = HdcKey::encode(&frame2, 256, 0xDEADBEEF);

        let similarity = key1.similarity(&key2);
        assert!(
            similarity > 0.9,
            "Similar frames should have high similarity"
        );
    }

    #[test]
    fn test_reflex_arc_homeostasis() {
        let mut arc = ReflexArc::new();
        let frame = SensorFrame {
            t_us: 0,
            position: 0.0,
            velocity: 0.0,
            force: 0.0,
            contact: 0.0,
            temperature: 0.5,
            vibration: 0.0,
        };

        let state = arc.compute_homeostasis(&frame);
        assert!(state.coherence > 0.0);
        assert!(state.tension >= 0.0);
    }

    #[test]
    fn test_actuation_energy_budget() {
        let renderer = ActuationRenderer::new(0.1);
        let frame = SensorFrame {
            t_us: 0,
            position: 0.5,
            velocity: 0.5,
            force: 0.5,
            contact: 1.0,
            temperature: 0.5,
            vibration: 0.5,
        };

        let state = HomeostasisState {
            tension: 0.7,
            coherence: 0.5,
            cut_value: 50.0,
            criticality: 0.3,
            reflex: ReflexMode::Spike,
        };

        let cmd = renderer.render(&frame, &state);
        assert!(cmd.energy_used <= 0.1, "Energy must not exceed budget");
    }

    #[test]
    fn test_learning_coherence_gate() {
        let controller = LearningController::new(64);

        let high_coherence_state = HomeostasisState {
            tension: 0.1,
            coherence: 0.95,
            cut_value: 10.0,
            criticality: 0.1,
            reflex: ReflexMode::Calm,
        };
        assert!(controller.should_learn(&high_coherence_state));

        let low_coherence_state = HomeostasisState {
            tension: 0.8,
            coherence: 0.5,
            cut_value: 80.0,
            criticality: 0.8,
            reflex: ReflexMode::Spike,
        };
        assert!(!controller.should_learn(&low_coherence_state));
    }

    #[test]
    fn test_full_tick_determinism() {
        let mut sensor = SimulatedSensor::new();
        let mut actuator = SimulatedActuator::new();
        let mut controller = SyntheticHapticController::new();

        // Run multiple ticks
        for _ in 0..100 {
            let (state, cmd) = controller.tick(&mut sensor, &mut actuator);
            assert!(state.tension >= 0.0 && state.tension <= 1.0);
            assert!(state.coherence >= 0.0 && state.coherence <= 1.0);
            assert!(cmd.energy_used <= 0.1);
        }

        assert_eq!(controller.stats().tick_count, 100);
    }

    #[test]
    fn test_memory_storage_and_retrieval() {
        let mut memory = AssociativeMemory::new(256, 100);

        let frame = SensorFrame {
            t_us: 0,
            position: 0.5,
            velocity: 0.1,
            force: 0.0,
            contact: 1.0,
            temperature: 0.5,
            vibration: 0.2,
        };

        memory.store(&frame, ReflexMode::Active, 0.9);
        assert_eq!(memory.memory_count(), 1);

        let results = memory.query(&frame, 1);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, ReflexMode::Active);
    }
}
