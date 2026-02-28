//! # Application 3: Artificial Homeostasis in Synthetic Life Simulations
//!
//! Coherence replaces fitness as the primary survival constraint.
//!
//! ## What Breaks Today
//! Artificial life and agent-based simulations explode, stagnate,
//! or need constant tuning.
//!
//! ## Δ-Behavior Application
//! Agents that violate coherence:
//! - Consume more energy
//! - Lose memory
//! - Die earlier
//!
//! ## Exotic Result
//! Evolution that selects for stable regulation, not just reward maximization.
//!
//! This is publishable territory.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// A synthetic organism with homeostatic regulation
pub struct HomeostasticOrganism {
    /// Unique identifier
    pub id: u64,

    /// Internal state variables (e.g., temperature, pH, energy)
    internal_state: HashMap<String, f64>,

    /// Setpoints for each state variable (homeostatic targets)
    setpoints: HashMap<String, f64>,

    /// Tolerance for deviation from setpoint
    tolerances: HashMap<String, f64>,

    /// Current coherence (system-wide stability measure)
    coherence: f64,

    /// Energy reserves
    energy: f64,

    /// Memory capacity (degrades with low coherence)
    memory: Vec<MemoryEntry>,
    max_memory: usize,

    /// Age in simulation ticks
    age: u64,

    /// Is alive
    alive: bool,

    /// Genome (controls regulatory parameters)
    genome: Genome,
}

#[derive(Clone)]
pub struct Genome {
    /// How aggressively to correct deviations
    regulatory_strength: f64,

    /// Energy efficiency
    metabolic_efficiency: f64,

    /// Base coherence maintenance cost
    coherence_maintenance_cost: f64,

    /// Memory retention under stress
    memory_resilience: f64,

    /// Lifespan factor
    longevity: f64,
}

#[derive(Clone)]
pub struct MemoryEntry {
    pub content: String,
    pub importance: f64,
    pub age: u64,
}

/// Actions the organism can take
#[derive(Debug, Clone)]
pub enum Action {
    /// Consume energy from environment
    Eat(f64),
    /// Attempt to reproduce
    Reproduce,
    /// Move in environment
    Move(f64, f64),
    /// Do nothing (rest)
    Rest,
    /// Regulate internal state
    Regulate(String, f64),
}

/// Results of actions
#[derive(Debug)]
pub enum ActionResult {
    Success { energy_cost: f64, coherence_impact: f64 },
    Failed { reason: String },
    Died { cause: DeathCause },
    Reproduced { offspring_id: u64 },
}

#[derive(Debug)]
pub enum DeathCause {
    EnergyDepleted,
    CoherenceCollapse,
    OldAge,
    ExtremeDeviation(String),
}

impl HomeostasticOrganism {
    pub fn new(id: u64, genome: Genome) -> Self {
        let mut internal_state = HashMap::new();
        let mut setpoints = HashMap::new();
        let mut tolerances = HashMap::new();

        // Define homeostatic variables
        internal_state.insert("temperature".to_string(), 37.0);
        setpoints.insert("temperature".to_string(), 37.0);
        tolerances.insert("temperature".to_string(), 2.0);

        internal_state.insert("ph".to_string(), 7.4);
        setpoints.insert("ph".to_string(), 7.4);
        tolerances.insert("ph".to_string(), 0.3);

        internal_state.insert("glucose".to_string(), 100.0);
        setpoints.insert("glucose".to_string(), 100.0);
        tolerances.insert("glucose".to_string(), 30.0);

        Self {
            id,
            internal_state,
            setpoints,
            tolerances,
            coherence: 1.0,
            energy: 100.0,
            memory: Vec::new(),
            max_memory: 100,
            age: 0,
            alive: true,
            genome,
        }
    }

    /// Calculate current coherence based on homeostatic deviation
    /// Returns a valid f64 in range [0.0, 1.0], with NaN/Infinity protection
    fn calculate_coherence(&self) -> f64 {
        let mut total_deviation = 0.0;
        let mut count = 0;

        for (var, &current) in &self.internal_state {
            if let (Some(&setpoint), Some(&tolerance)) =
                (self.setpoints.get(var), self.tolerances.get(var))
            {
                // Validate inputs for NaN/Infinity
                if !current.is_finite() || !setpoint.is_finite() || !tolerance.is_finite() {
                    continue;
                }
                // Avoid division by zero
                if tolerance.abs() < f64::EPSILON {
                    continue;
                }
                let deviation = ((current - setpoint) / tolerance).abs();
                if deviation.is_finite() {
                    total_deviation += deviation.powi(2);
                    count += 1;
                }
            }
        }

        if count == 0 {
            return 1.0;
        }

        // Coherence is inverse of normalized deviation
        let avg_deviation = (total_deviation / count as f64).sqrt();

        // Final NaN/Infinity check
        if !avg_deviation.is_finite() {
            return 0.0; // Safe default for invalid state
        }

        (1.0 / (1.0 + avg_deviation)).clamp(0.0, 1.0)
    }

    /// Energy cost scales with coherence violation
    fn action_energy_cost(&self, base_cost: f64) -> f64 {
        // Lower coherence = higher energy cost (incoherent states are expensive)
        let coherence_penalty = 1.0 / self.coherence.max(0.1);
        base_cost * coherence_penalty
    }

    /// Perform an action
    pub fn act(&mut self, action: Action) -> ActionResult {
        if !self.alive {
            return ActionResult::Failed { reason: "Dead".to_string() };
        }

        // Update coherence first
        self.coherence = self.calculate_coherence();

        // Apply coherence-based degradation
        self.apply_coherence_effects();

        let result = match action {
            Action::Eat(amount) => self.eat(amount),
            Action::Reproduce => self.reproduce(),
            Action::Move(dx, dy) => self.move_action(dx, dy),
            Action::Rest => self.rest(),
            Action::Regulate(var, target) => self.regulate(&var, target),
        };

        // Age and check death conditions
        self.age += 1;
        self.check_death();

        result
    }

    fn apply_coherence_effects(&mut self) {
        // Low coherence causes memory loss
        if self.coherence < 0.5 {
            let memory_loss_rate = (1.0 - self.coherence) * (1.0 - self.genome.memory_resilience);
            let memories_to_lose = (self.memory.len() as f64 * memory_loss_rate * 0.1) as usize;

            // Lose least important memories first
            self.memory.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
            self.memory.truncate(self.memory.len().saturating_sub(memories_to_lose));
        }

        // Coherence maintenance costs energy
        let maintenance_cost = self.genome.coherence_maintenance_cost / self.coherence.max(0.1);
        self.energy -= maintenance_cost;
    }

    fn eat(&mut self, amount: f64) -> ActionResult {
        let base_cost = 2.0;
        let cost = self.action_energy_cost(base_cost);

        if self.energy < cost {
            return ActionResult::Failed { reason: "Not enough energy to eat".to_string() };
        }

        self.energy -= cost;
        self.energy += amount * self.genome.metabolic_efficiency;

        // Eating affects glucose
        if let Some(glucose) = self.internal_state.get_mut("glucose") {
            *glucose += amount * 0.5;
        }

        ActionResult::Success {
            energy_cost: cost,
            coherence_impact: self.calculate_coherence() - self.coherence,
        }
    }

    fn regulate(&mut self, var: &str, target: f64) -> ActionResult {
        let base_cost = 5.0;
        let cost = self.action_energy_cost(base_cost);

        if self.energy < cost {
            return ActionResult::Failed { reason: "Not enough energy to regulate".to_string() };
        }

        self.energy -= cost;

        if let Some(current) = self.internal_state.get_mut(var) {
            let diff = target - *current;
            // Apply regulation with genome-determined strength
            *current += diff * self.genome.regulatory_strength;
        }

        let new_coherence = self.calculate_coherence();
        let impact = new_coherence - self.coherence;
        self.coherence = new_coherence;

        ActionResult::Success {
            energy_cost: cost,
            coherence_impact: impact,
        }
    }

    fn reproduce(&mut self) -> ActionResult {
        let base_cost = 50.0;
        let cost = self.action_energy_cost(base_cost);

        // Reproduction requires high coherence
        if self.coherence < 0.7 {
            return ActionResult::Failed {
                reason: "Coherence too low to reproduce".to_string()
            };
        }

        if self.energy < cost {
            return ActionResult::Failed { reason: "Not enough energy to reproduce".to_string() };
        }

        self.energy -= cost;

        // Create mutated genome for offspring
        let offspring_genome = self.genome.mutate();
        let offspring_id = self.id * 1000 + self.age; // Simple ID generation

        ActionResult::Reproduced { offspring_id }
    }

    fn move_action(&mut self, _dx: f64, _dy: f64) -> ActionResult {
        let base_cost = 3.0;
        let cost = self.action_energy_cost(base_cost);

        if self.energy < cost {
            return ActionResult::Failed { reason: "Not enough energy to move".to_string() };
        }

        self.energy -= cost;

        // Moving affects temperature
        if let Some(temp) = self.internal_state.get_mut("temperature") {
            *temp += 0.1; // Movement generates heat
        }

        ActionResult::Success {
            energy_cost: cost,
            coherence_impact: 0.0,
        }
    }

    fn rest(&mut self) -> ActionResult {
        // Resting is cheap and helps regulate
        let cost = 0.5;
        self.energy -= cost;

        // Slowly return to setpoints
        for (var, current) in self.internal_state.iter_mut() {
            if let Some(&setpoint) = self.setpoints.get(var) {
                let diff = setpoint - *current;
                *current += diff * 0.1;
            }
        }

        ActionResult::Success {
            energy_cost: cost,
            coherence_impact: self.calculate_coherence() - self.coherence,
        }
    }

    fn check_death(&mut self) {
        // Death by energy depletion
        if self.energy <= 0.0 {
            self.alive = false;
            return;
        }

        // Death by coherence collapse
        if self.coherence < 0.1 {
            self.alive = false;
            return;
        }

        // Death by extreme deviation
        for (var, &current) in &self.internal_state {
            if let (Some(&setpoint), Some(&tolerance)) =
                (self.setpoints.get(var), self.tolerances.get(var))
            {
                if (current - setpoint).abs() > tolerance * 5.0 {
                    self.alive = false;
                    return;
                }
            }
        }

        // Death by old age (modified by longevity gene)
        let max_age = (1000.0 * self.genome.longevity) as u64;
        if self.age > max_age {
            self.alive = false;
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn status(&self) -> String {
        format!(
            "Organism {} | Age: {} | Energy: {:.1} | Coherence: {:.2} | Memory: {}",
            self.id, self.age, self.energy, self.coherence, self.memory.len()
        )
    }
}

impl Genome {
    pub fn random() -> Self {
        Self {
            regulatory_strength: 0.1 + rand_f64() * 0.4,
            metabolic_efficiency: 0.5 + rand_f64() * 0.5,
            coherence_maintenance_cost: 0.5 + rand_f64() * 1.5,
            memory_resilience: rand_f64(),
            longevity: 0.5 + rand_f64() * 1.0,
        }
    }

    pub fn mutate(&self) -> Self {
        Self {
            regulatory_strength: mutate_value(self.regulatory_strength, 0.05, 0.1, 0.9),
            metabolic_efficiency: mutate_value(self.metabolic_efficiency, 0.05, 0.3, 1.0),
            coherence_maintenance_cost: mutate_value(self.coherence_maintenance_cost, 0.1, 0.1, 3.0),
            memory_resilience: mutate_value(self.memory_resilience, 0.05, 0.0, 1.0),
            longevity: mutate_value(self.longevity, 0.05, 0.3, 2.0),
        }
    }
}

/// Thread-safe atomic seed for pseudo-random number generation
static SEED: AtomicU64 = AtomicU64::new(12345);

fn rand_f64() -> f64 {
    // Simple LCG for reproducibility in tests - now thread-safe
    let old = SEED.fetch_add(1, Ordering::Relaxed);
    let new = old.wrapping_mul(1103515245).wrapping_add(12345);
    // Store back for next call (best-effort, races are acceptable for RNG)
    let _ = SEED.compare_exchange(old + 1, new, Ordering::Relaxed, Ordering::Relaxed);
    ((new >> 16) & 0x7fff) as f64 / 32768.0
}

fn mutate_value(value: f64, mutation_rate: f64, min: f64, max: f64) -> f64 {
    let mutation = (rand_f64() - 0.5) * 2.0 * mutation_rate;
    (value + mutation).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_homeostatic_survival() {
        let genome = Genome::random();
        let mut organism = HomeostasticOrganism::new(1, genome);

        let mut ticks = 0;
        while organism.is_alive() && ticks < 1000 {
            // Simple behavior: eat when hungry, regulate when unstable
            let action = if organism.energy < 50.0 {
                Action::Eat(20.0)
            } else if organism.coherence < 0.8 {
                Action::Regulate("temperature".to_string(), 37.0)
            } else {
                Action::Rest
            };

            let _ = organism.act(action);
            ticks += 1;

            if ticks % 100 == 0 {
                println!("{}", organism.status());
            }
        }

        println!("Survived {} ticks", ticks);
        println!("Final: {}", organism.status());
    }

    #[test]
    fn test_coherence_based_death() {
        let genome = Genome::random();
        let mut organism = HomeostasticOrganism::new(2, genome);

        // Deliberately destabilize
        if let Some(temp) = organism.internal_state.get_mut("temperature") {
            *temp = 50.0; // Extreme fever
        }

        let mut ticks = 0;
        while organism.is_alive() && ticks < 100 {
            let _ = organism.act(Action::Rest);
            ticks += 1;
        }

        // Organism should die from coherence collapse or extreme deviation
        assert!(!organism.is_alive(), "Organism should die from instability");
    }
}
