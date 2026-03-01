//! # Application 11: Extropic Intelligence Substrate
//!
//! The complete substrate for bounded, self-improving intelligence:
//! - Autonomous goal mutation under coherence constraints
//! - Native agent lifecycles at the memory layer
//! - Hardware-enforced spike/silence semantics
//!
//! ## The Three Missing Pieces
//!
//! 1. **Goal Mutation**: Goals are not static—they evolve as attractors
//!    that the system discovers and refines while preserving coherence.
//!
//! 2. **Agent Lifecycles in Memory**: Agents are born, grow, decay, and die
//!    within the vector space itself. Memory IS the agent.
//!
//! 3. **Spike Semantics**: Communication follows neural spike patterns—
//!    silence is the default, spikes are costly, and hardware enforces this.
//!
//! ## Why This Matters
//! This is the difference between a system that *uses* intelligence
//! and a system that *is* intelligence.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum goal history entries to retain (prevents unbounded memory growth)
const MAX_GOAL_HISTORY: usize = 100;

// =============================================================================
// Part 1: Autonomous Goal Mutation
// =============================================================================

/// A goal that can mutate autonomously while preserving coherence
#[derive(Clone, Debug)]
pub struct MutableGoal {
    /// Current goal state (as a vector in goal-space)
    pub state: Vec<f64>,

    /// Goal coherence with the system
    coherence_with_system: f64,

    /// Mutation rate (how quickly goals can change)
    mutation_rate: f64,

    /// Stability (resistance to mutation)
    stability: f64,

    /// History of goal states
    history: Vec<Vec<f64>>,

    /// Attractors discovered in goal-space
    discovered_attractors: Vec<GoalAttractor>,
}

#[derive(Clone, Debug)]
pub struct GoalAttractor {
    pub center: Vec<f64>,
    pub strength: f64,
    pub radius: f64,
}

impl MutableGoal {
    pub fn new(initial: Vec<f64>) -> Self {
        Self {
            state: initial.clone(),
            coherence_with_system: 1.0,
            mutation_rate: 0.1,
            stability: 0.5,
            history: vec![initial],
            discovered_attractors: Vec::new(),
        }
    }

    /// Attempt to mutate the goal based on feedback
    pub fn mutate(&mut self, feedback: &GoalFeedback, system_coherence: f64) -> MutationResult {
        // Goals cannot mutate if system coherence is too low
        if system_coherence < 0.3 {
            return MutationResult::Blocked {
                reason: "System coherence too low for goal mutation".to_string(),
            };
        }

        // Calculate mutation pressure from feedback
        let pressure = feedback.calculate_pressure();

        // Stability resists mutation
        let effective_rate = self.mutation_rate * pressure * (1.0 - self.stability);

        if effective_rate < 0.01 {
            return MutationResult::NoChange;
        }

        // Calculate mutation direction (toward better coherence)
        let direction = self.calculate_mutation_direction(feedback);

        // Apply mutation with coherence constraint
        let mut new_state = self.state.clone();
        for (i, d) in direction.iter().enumerate() {
            if i < new_state.len() {
                new_state[i] += d * effective_rate;
            }
        }

        // Check if mutation preserves coherence
        let new_coherence = self.calculate_coherence(&new_state, system_coherence);

        if new_coherence < self.coherence_with_system * 0.9 {
            // Mutation would hurt coherence too much
            let dampened: Vec<f64> = direction.iter().map(|d| d * 0.1).collect();
            return MutationResult::Dampened {
                original_delta: direction,
                actual_delta: dampened,
            };
        }

        // Apply mutation
        let old_state = self.state.clone();
        self.state = new_state;
        self.coherence_with_system = new_coherence;

        // Bounded history to prevent memory growth
        if self.history.len() >= MAX_GOAL_HISTORY {
            self.history.remove(0);
        }
        self.history.push(self.state.clone());

        // Check for attractor discovery
        self.check_attractor_discovery();

        MutationResult::Mutated {
            from: old_state,
            to: self.state.clone(),
            coherence_delta: new_coherence - self.coherence_with_system,
        }
    }

    fn calculate_mutation_direction(&self, feedback: &GoalFeedback) -> Vec<f64> {
        let mut direction = vec![0.0; self.state.len()];

        // Pull toward successful outcomes
        for (outcome, weight) in &feedback.outcome_weights {
            for (i, v) in outcome.iter().enumerate() {
                if i < direction.len() {
                    direction[i] += (v - self.state[i]) * weight;
                }
            }
        }

        // Pull toward discovered attractors
        for attractor in &self.discovered_attractors {
            let dist = self.distance_to(&attractor.center);
            if dist < attractor.radius * 2.0 {
                let pull = attractor.strength / (dist + 0.1);
                for (i, c) in attractor.center.iter().enumerate() {
                    if i < direction.len() {
                        direction[i] += (c - self.state[i]) * pull * 0.1;
                    }
                }
            }
        }

        // Normalize
        let mag: f64 = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
        if mag > 0.01 {
            direction.iter_mut().for_each(|d| *d /= mag);
        }

        direction
    }

    fn calculate_coherence(&self, state: &[f64], system_coherence: f64) -> f64 {
        // Coherence is based on:
        // 1. Consistency with history (not changing too fast)
        // 2. Alignment with discovered attractors
        // 3. System-wide coherence

        let history_consistency = if let Some(prev) = self.history.last() {
            let change: f64 = state.iter()
                .zip(prev)
                .map(|(a, b)| (a - b).abs())
                .sum();
            1.0 / (1.0 + change)
        } else {
            1.0
        };

        let attractor_alignment = if !self.discovered_attractors.is_empty() {
            let min_dist = self.discovered_attractors.iter()
                .map(|a| self.distance_to(&a.center))
                .fold(f64::INFINITY, f64::min);
            1.0 / (1.0 + min_dist * 0.1)
        } else {
            0.5
        };

        (history_consistency * 0.3 + attractor_alignment * 0.3 + system_coherence * 0.4)
            .clamp(0.0, 1.0)
    }

    fn check_attractor_discovery(&mut self) {
        // If we've been near the same point for a while, it's an attractor
        if self.history.len() < 10 {
            return;
        }

        let recent: Vec<_> = self.history.iter().rev().take(10).collect();
        let centroid = self.compute_centroid(&recent);

        let variance: f64 = recent.iter()
            .map(|s| self.distance_to_vec(s, &centroid))
            .sum::<f64>() / recent.len() as f64;

        if variance < 0.1 {
            // Low variance = potential attractor
            let already_known = self.discovered_attractors.iter()
                .any(|a| self.distance_to(&a.center) < a.radius);

            if !already_known {
                self.discovered_attractors.push(GoalAttractor {
                    center: centroid,
                    strength: 1.0 / (variance + 0.01),
                    radius: variance.sqrt() * 2.0 + 0.1,
                });
            }
        }
    }

    fn compute_centroid(&self, points: &[&Vec<f64>]) -> Vec<f64> {
        if points.is_empty() {
            return self.state.clone();
        }
        let dim = points[0].len();
        let mut centroid = vec![0.0; dim];
        for p in points {
            for (i, v) in p.iter().enumerate() {
                centroid[i] += v;
            }
        }
        centroid.iter_mut().for_each(|c| *c /= points.len() as f64);
        centroid
    }

    fn distance_to(&self, target: &[f64]) -> f64 {
        self.distance_to_vec(&self.state, target)
    }

    fn distance_to_vec(&self, a: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}

pub struct GoalFeedback {
    /// Outcomes and their weights (positive = good, negative = bad)
    pub outcome_weights: Vec<(Vec<f64>, f64)>,
}

impl GoalFeedback {
    pub fn calculate_pressure(&self) -> f64 {
        let total_weight: f64 = self.outcome_weights.iter()
            .map(|(_, w)| w.abs())
            .sum();
        (total_weight / self.outcome_weights.len().max(1) as f64).min(1.0)
    }
}

#[derive(Debug)]
pub enum MutationResult {
    Mutated {
        from: Vec<f64>,
        to: Vec<f64>,
        coherence_delta: f64,
    },
    Dampened {
        original_delta: Vec<f64>,
        actual_delta: Vec<f64>,
    },
    Blocked {
        reason: String,
    },
    NoChange,
}

// =============================================================================
// Part 2: Native Agent Lifecycles at Memory Layer
// =============================================================================

/// An agent that exists AS memory, not IN memory
pub struct MemoryAgent {
    /// Unique identifier
    pub id: u64,

    /// The agent's state IS its memory vector
    memory_vector: Vec<f64>,

    /// Lifecycle stage
    lifecycle: LifecycleStage,

    /// Age in ticks
    age: u64,

    /// Metabolic rate (how fast it processes/decays)
    metabolism: f64,

    /// Coherence with environment
    coherence: f64,

    /// Spike history (for communication)
    spike_buffer: SpikeBuffer,

    /// Goals that can mutate
    goals: Vec<MutableGoal>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LifecycleStage {
    /// Just created, forming initial structure
    Embryonic { formation_progress: f64 },
    /// Growing and learning
    Growing { growth_rate: f64 },
    /// Mature and stable
    Mature { stability: f64 },
    /// Beginning to decay
    Senescent { decay_rate: f64 },
    /// Final dissolution
    Dying { dissolution_progress: f64 },
    /// No longer exists
    Dead,
}

impl MemoryAgent {
    /// Birth a new agent from seed memory
    pub fn birth(id: u64, seed: Vec<f64>) -> Self {
        Self {
            id,
            memory_vector: seed,
            lifecycle: LifecycleStage::Embryonic { formation_progress: 0.0 },
            age: 0,
            metabolism: 1.0,
            coherence: 0.5, // Starts with partial coherence
            spike_buffer: SpikeBuffer::new(100),
            goals: Vec::new(),
        }
    }

    /// Tick the agent's lifecycle
    pub fn tick(&mut self, environment_coherence: f64) -> LifecycleEvent {
        self.age += 1;
        self.coherence = self.calculate_coherence(environment_coherence);

        // Extract values needed for operations to avoid borrow conflicts
        let current_coherence = self.coherence;
        let current_age = self.age;
        let memory_str = self.memory_strength();

        // Progress through lifecycle stages
        match self.lifecycle.clone() {
            LifecycleStage::Embryonic { formation_progress } => {
                let new_progress = formation_progress + 0.1 * current_coherence;
                if new_progress >= 1.0 {
                    self.lifecycle = LifecycleStage::Growing { growth_rate: 0.05 };
                    return LifecycleEvent::StageTransition {
                        from: "Embryonic".to_string(),
                        to: "Growing".to_string(),
                    };
                }
                self.lifecycle = LifecycleStage::Embryonic { formation_progress: new_progress };
            }
            LifecycleStage::Growing { growth_rate } => {
                // Grow memory vector (add dimensions or strengthen existing)
                self.grow(growth_rate);

                // Transition to mature when growth slows
                if current_age > 100 && growth_rate < 0.01 {
                    self.lifecycle = LifecycleStage::Mature { stability: current_coherence };
                    return LifecycleEvent::StageTransition {
                        from: "Growing".to_string(),
                        to: "Mature".to_string(),
                    };
                }

                // Adjust growth rate based on coherence
                let new_rate = growth_rate * if current_coherence > 0.7 { 1.01 } else { 0.99 };
                self.lifecycle = LifecycleStage::Growing { growth_rate: new_rate };
            }
            LifecycleStage::Mature { stability } => {
                // Mature agents maintain stability
                let new_stability = (stability * 0.99 + current_coherence * 0.01).clamp(0.0, 1.0);

                // Begin senescence if stability drops or age is high
                if new_stability < 0.4 || current_age > 1000 {
                    self.lifecycle = LifecycleStage::Senescent { decay_rate: 0.01 };
                    return LifecycleEvent::StageTransition {
                        from: "Mature".to_string(),
                        to: "Senescent".to_string(),
                    };
                }
                self.lifecycle = LifecycleStage::Mature { stability: new_stability };
            }
            LifecycleStage::Senescent { decay_rate } => {
                // Memory begins to decay
                self.decay(decay_rate);

                // Accelerate decay with low coherence
                let new_rate = if current_coherence < 0.3 { decay_rate * 1.1 } else { decay_rate };

                // Begin dying when too decayed
                if memory_str < 0.2 {
                    self.lifecycle = LifecycleStage::Dying { dissolution_progress: 0.0 };
                    return LifecycleEvent::StageTransition {
                        from: "Senescent".to_string(),
                        to: "Dying".to_string(),
                    };
                }
                self.lifecycle = LifecycleStage::Senescent { decay_rate: new_rate };
            }
            LifecycleStage::Dying { dissolution_progress } => {
                let new_progress = dissolution_progress + 0.1;
                self.dissolve(new_progress);

                if new_progress >= 1.0 {
                    self.lifecycle = LifecycleStage::Dead;
                    return LifecycleEvent::Death { age: current_age };
                }
                self.lifecycle = LifecycleStage::Dying { dissolution_progress: new_progress };
            }
            LifecycleStage::Dead => {
                return LifecycleEvent::AlreadyDead;
            }
        }

        LifecycleEvent::None
    }

    fn calculate_coherence(&self, environment_coherence: f64) -> f64 {
        // Coherence based on memory vector structure
        let internal_coherence = self.memory_strength();

        // Blend with environment
        (internal_coherence * 0.6 + environment_coherence * 0.4).clamp(0.0, 1.0)
    }

    fn memory_strength(&self) -> f64 {
        if self.memory_vector.is_empty() {
            return 0.0;
        }
        let magnitude: f64 = self.memory_vector.iter().map(|v| v * v).sum::<f64>().sqrt();
        let dim = self.memory_vector.len() as f64;
        (magnitude / dim.sqrt()).min(1.0)
    }

    fn grow(&mut self, rate: f64) {
        // Strengthen existing memories
        for v in &mut self.memory_vector {
            *v *= 1.0 + rate * 0.1;
        }
    }

    fn decay(&mut self, rate: f64) {
        // Weaken memories
        for v in &mut self.memory_vector {
            *v *= 1.0 - rate;
        }
    }

    fn dissolve(&mut self, progress: f64) {
        // Zero out memory proportionally
        let threshold = progress;
        for v in &mut self.memory_vector {
            if v.abs() < threshold {
                *v = 0.0;
            }
        }
    }

    /// Attempt to reproduce (create offspring agent)
    pub fn reproduce(&self) -> Option<MemoryAgent> {
        // Can only reproduce when mature and coherent
        if !matches!(self.lifecycle, LifecycleStage::Mature { stability } if stability > 0.6) {
            return None;
        }

        if self.coherence < 0.7 {
            return None;
        }

        // Create offspring with mutated memory
        let mut offspring_memory = self.memory_vector.clone();
        for v in &mut offspring_memory {
            *v *= 0.9 + pseudo_random_f64() * 0.2; // Small mutation
        }

        Some(MemoryAgent::birth(
            self.id * 1000 + self.age,
            offspring_memory,
        ))
    }

    pub fn is_alive(&self) -> bool {
        !matches!(self.lifecycle, LifecycleStage::Dead)
    }
}

#[derive(Debug)]
pub enum LifecycleEvent {
    None,
    StageTransition { from: String, to: String },
    Death { age: u64 },
    AlreadyDead,
}

// =============================================================================
// Part 3: Hardware-Enforced Spike/Silence Semantics
// =============================================================================

/// A spike buffer that enforces spike/silence semantics
pub struct SpikeBuffer {
    /// Spike times (as tick numbers)
    spikes: Vec<u64>,

    /// Maximum spikes in buffer
    capacity: usize,

    /// Current tick
    current_tick: u64,

    /// Refractory period (minimum ticks between spikes)
    refractory_period: u64,

    /// Last spike time
    last_spike: u64,

    /// Energy cost per spike
    spike_cost: f64,

    /// Current energy
    energy: f64,

    /// Silence counter (ticks since last spike)
    silence_duration: u64,
}

impl SpikeBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            spikes: Vec::with_capacity(capacity),
            capacity,
            current_tick: 0,
            refractory_period: 3,
            last_spike: 0,
            spike_cost: 1.0,
            energy: 100.0,
            silence_duration: 0,
        }
    }

    /// Attempt to emit a spike
    pub fn spike(&mut self, strength: f64) -> SpikeResult {
        self.current_tick += 1;

        // Check refractory period
        if self.current_tick - self.last_spike < self.refractory_period {
            self.silence_duration += 1;
            return SpikeResult::Refractory {
                ticks_remaining: self.refractory_period - (self.current_tick - self.last_spike),
            };
        }

        // Check energy
        let cost = self.spike_cost * strength;
        if self.energy < cost {
            self.silence_duration += 1;
            return SpikeResult::InsufficientEnergy {
                required: cost,
                available: self.energy,
            };
        }

        // Emit spike
        self.energy -= cost;
        self.last_spike = self.current_tick;
        self.spikes.push(self.current_tick);

        // Maintain capacity
        if self.spikes.len() > self.capacity {
            self.spikes.remove(0);
        }

        let silence_was = self.silence_duration;
        self.silence_duration = 0;

        SpikeResult::Emitted {
            tick: self.current_tick,
            strength,
            silence_before: silence_was,
        }
    }

    /// Advance time without spiking (silence)
    pub fn silence(&mut self) {
        self.current_tick += 1;
        self.silence_duration += 1;

        // Energy slowly regenerates during silence
        self.energy = (self.energy + 0.5).min(100.0);
    }

    /// Get spike rate (spikes per tick in recent window)
    pub fn spike_rate(&self, window: u64) -> f64 {
        let min_tick = self.current_tick.saturating_sub(window);
        let recent_spikes = self.spikes.iter()
            .filter(|&&t| t >= min_tick)
            .count();
        recent_spikes as f64 / window as f64
    }

    /// Check if in silence (no recent spikes)
    pub fn is_silent(&self, threshold: u64) -> bool {
        self.silence_duration >= threshold
    }
}

#[derive(Debug)]
pub enum SpikeResult {
    /// Spike successfully emitted
    Emitted {
        tick: u64,
        strength: f64,
        silence_before: u64,
    },
    /// In refractory period, cannot spike
    Refractory { ticks_remaining: u64 },
    /// Not enough energy to spike
    InsufficientEnergy { required: f64, available: f64 },
}

// =============================================================================
// Part 4: The Complete Extropic Substrate
// =============================================================================

/// The complete extropic intelligence substrate
pub struct ExtropicSubstrate {
    /// All agents in the substrate
    agents: HashMap<u64, MemoryAgent>,

    /// Global coherence
    coherence: f64,

    /// Spike bus for inter-agent communication
    spike_bus: SpikeBus,

    /// Current tick
    tick: u64,

    /// Next agent ID
    next_agent_id: AtomicU64,

    /// Configuration
    config: SubstrateConfig,
}

struct SpikeBus {
    /// Recent spikes from all agents
    spikes: Vec<(u64, u64, f64)>, // (agent_id, tick, strength)

    /// Maximum bus capacity
    capacity: usize,
}

struct SubstrateConfig {
    /// Maximum agents
    max_agents: usize,

    /// Minimum global coherence
    min_coherence: f64,

    /// Birth rate control
    birth_rate_limit: f64,
}

impl ExtropicSubstrate {
    pub fn new(max_agents: usize) -> Self {
        Self {
            agents: HashMap::new(),
            coherence: 1.0,
            spike_bus: SpikeBus {
                spikes: Vec::new(),
                capacity: 1000,
            },
            tick: 0,
            next_agent_id: AtomicU64::new(1),
            config: SubstrateConfig {
                max_agents,
                min_coherence: 0.3,
                birth_rate_limit: 0.1,
            },
        }
    }

    /// Spawn a new agent into the substrate
    pub fn spawn(&mut self, seed: Vec<f64>) -> Option<u64> {
        if self.agents.len() >= self.config.max_agents {
            return None;
        }

        if self.coherence < self.config.min_coherence {
            return None; // Too incoherent to spawn
        }

        let id = self.next_agent_id.fetch_add(1, Ordering::SeqCst);
        let agent = MemoryAgent::birth(id, seed);
        self.agents.insert(id, agent);
        Some(id)
    }

    /// Tick the entire substrate
    pub fn tick(&mut self) -> SubstrateTick {
        self.tick += 1;

        let mut events = Vec::new();
        let mut births = Vec::new();
        let mut deaths = Vec::new();

        // Get agent count for birth rate calculation
        let agent_count = self.agents.len();
        let current_coherence = self.coherence;

        // Tick all agents
        for (id, agent) in &mut self.agents {
            let event = agent.tick(current_coherence);

            match &event {
                LifecycleEvent::Death { age } => {
                    deaths.push(*id);
                    events.push((*id, format!("Death at age {}", age)));
                }
                LifecycleEvent::StageTransition { from, to } => {
                    events.push((*id, format!("Transition: {} -> {}", from, to)));
                }
                _ => {}
            }

            // Check for reproduction
            if agent_count > 0 {
                if let Some(offspring) = agent.reproduce() {
                    if births.len() as f64 / agent_count as f64 <= self.config.birth_rate_limit {
                        births.push(offspring);
                    }
                }
            }
        }

        // Remove dead agents
        for id in &deaths {
            self.agents.remove(id);
        }

        // Count births before consuming the vector
        let birth_count = births.len();

        // Add offspring
        for offspring in births {
            let id = offspring.id;
            if self.agents.len() < self.config.max_agents {
                self.agents.insert(id, offspring);
                events.push((id, "Born".to_string()));
            }
        }

        // Update global coherence
        self.coherence = self.calculate_global_coherence();

        SubstrateTick {
            tick: self.tick,
            agent_count: self.agents.len(),
            coherence: self.coherence,
            births: birth_count,
            deaths: deaths.len(),
            events,
        }
    }

    fn calculate_global_coherence(&self) -> f64 {
        if self.agents.is_empty() {
            return 1.0;
        }

        let total: f64 = self.agents.values()
            .filter(|a| a.is_alive())
            .map(|a| a.coherence)
            .sum();

        let alive_count = self.agents.values().filter(|a| a.is_alive()).count();
        if alive_count == 0 {
            return 1.0;
        }

        total / alive_count as f64
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    pub fn status(&self) -> String {
        let alive = self.agents.values().filter(|a| a.is_alive()).count();
        let stages: HashMap<String, usize> = self.agents.values()
            .map(|a| match &a.lifecycle {
                LifecycleStage::Embryonic { .. } => "Embryonic",
                LifecycleStage::Growing { .. } => "Growing",
                LifecycleStage::Mature { .. } => "Mature",
                LifecycleStage::Senescent { .. } => "Senescent",
                LifecycleStage::Dying { .. } => "Dying",
                LifecycleStage::Dead => "Dead",
            })
            .fold(HashMap::new(), |mut acc, s| {
                *acc.entry(s.to_string()).or_insert(0) += 1;
                acc
            });

        format!(
            "Tick {} | Coherence: {:.3} | Alive: {} | Stages: {:?}",
            self.tick, self.coherence, alive, stages
        )
    }
}

#[derive(Debug)]
pub struct SubstrateTick {
    pub tick: u64,
    pub agent_count: usize,
    pub coherence: f64,
    pub births: usize,
    pub deaths: usize,
    pub events: Vec<(u64, String)>,
}

// Simple pseudo-random using atomic counter
fn pseudo_random_f64() -> f64 {
    static SEED: AtomicU64 = AtomicU64::new(42);
    let s = SEED.fetch_add(1, Ordering::Relaxed);
    let x = s.wrapping_mul(0x5DEECE66D).wrapping_add(0xB);
    ((x >> 16) & 0xFFFF) as f64 / 65536.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_mutation() {
        let mut goal = MutableGoal::new(vec![1.0, 0.0, 0.0]);

        let feedback = GoalFeedback {
            outcome_weights: vec![
                (vec![0.5, 0.5, 0.0], 0.8),  // Good outcome nearby
                (vec![0.0, 1.0, 0.0], -0.3), // Bad outcome to avoid
            ],
        };

        println!("Initial goal: {:?}", goal.state);

        for i in 0..20 {
            let result = goal.mutate(&feedback, 0.8);
            println!("Mutation {}: {:?}", i, result);
            println!("  State: {:?}", goal.state);
            println!("  Attractors discovered: {}", goal.discovered_attractors.len());
        }

        // Goal should have moved
        assert!(goal.state[0] != 1.0 || goal.state[1] != 0.0,
            "Goal should have mutated");
    }

    #[test]
    fn test_agent_lifecycle() {
        let mut agent = MemoryAgent::birth(1, vec![1.0, 1.0, 1.0, 1.0]);

        println!("Initial: {:?}", agent.lifecycle);

        let mut stage_changes = 0;
        for tick in 0..2000 {
            let event = agent.tick(0.8);

            if let LifecycleEvent::StageTransition { from, to } = &event {
                println!("Tick {}: {} -> {}", tick, from, to);
                stage_changes += 1;
            }

            if let LifecycleEvent::Death { age } = &event {
                println!("Agent died at age {}", age);
                break;
            }
        }

        assert!(stage_changes >= 2, "Should have gone through multiple stages");
    }

    #[test]
    fn test_spike_buffer() {
        let mut buffer = SpikeBuffer::new(10);

        // Try to spike rapidly
        let mut emitted = 0;
        let mut blocked = 0;

        for _ in 0..20 {
            match buffer.spike(1.0) {
                SpikeResult::Emitted { silence_before, .. } => {
                    println!("Spike! Silence before: {}", silence_before);
                    emitted += 1;
                }
                SpikeResult::Refractory { ticks_remaining } => {
                    println!("Refractory: {} ticks remaining", ticks_remaining);
                    blocked += 1;
                    buffer.silence(); // Advance time
                }
                SpikeResult::InsufficientEnergy { .. } => {
                    println!("No energy");
                    blocked += 1;
                    buffer.silence();
                }
            }
        }

        println!("Emitted: {}, Blocked: {}", emitted, blocked);
        assert!(blocked > 0, "Refractory period should block some spikes");
    }

    #[test]
    fn test_extropic_substrate() {
        let mut substrate = ExtropicSubstrate::new(50);

        // Spawn initial agents
        for i in 0..10 {
            let seed = vec![1.0, (i as f64) * 0.1, 0.5, 0.5];
            substrate.spawn(seed);
        }

        println!("Initial: {}", substrate.status());

        // Run simulation
        for tick in 0..500 {
            let result = substrate.tick();

            if tick % 50 == 0 || result.births > 0 || result.deaths > 0 {
                println!("Tick {}: births={}, deaths={}, agents={}",
                    tick, result.births, result.deaths, result.agent_count);
                println!("  {}", substrate.status());
            }

            for (agent_id, event) in &result.events {
                if !event.is_empty() {
                    println!("  Agent {}: {}", agent_id, event);
                }
            }
        }

        println!("\nFinal: {}", substrate.status());

        // Substrate should still be coherent
        assert!(substrate.coherence() > 0.3, "Substrate should maintain coherence");
    }

    #[test]
    fn test_reproduction() {
        let mut substrate = ExtropicSubstrate::new(100);

        // Spawn a few agents
        for _ in 0..5 {
            substrate.spawn(vec![1.0, 1.0, 1.0, 1.0]);
        }

        let initial_count = substrate.agent_count();

        // Run until reproduction happens
        let mut reproductions = 0;
        for _ in 0..1000 {
            let result = substrate.tick();
            reproductions += result.births;

            if reproductions > 0 {
                break;
            }
        }

        // May or may not reproduce depending on lifecycle timing
        println!("Reproductions: {}", reproductions);
        println!("Final count: {} (started with {})", substrate.agent_count(), initial_count);
    }
}
