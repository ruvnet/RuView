//! # Cognitive Thermodynamics
//!
//! Deep exploration of Landauer's principle and thermodynamic constraints
//! on cognitive processing.
//!
//! ## Key Concepts
//!
//! - **Landauer's Principle**: Erasing 1 bit costs kT ln(2) energy
//! - **Reversible Computation**: Computation without erasure costs no energy
//! - **Cognitive Temperature**: Noise/randomness in cognitive processing
//! - **Maxwell's Demon**: Information-to-work conversion
//! - **Thought Entropy**: Disorder in cognitive states
//!
//! ## Theoretical Foundation
//!
//! Based on:
//! - Landauer (1961) - Irreversibility and Heat Generation
//! - Bennett - Reversible Computation
//! - Szilard Engine - Information thermodynamics
//! - Jarzynski Equality - Non-equilibrium thermodynamics

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Cognitive thermodynamics system
#[derive(Debug)]
pub struct CognitiveThermodynamics {
    /// Cognitive temperature (noise level)
    temperature: f64,
    /// Total entropy of the system
    entropy: ThoughtEntropy,
    /// Energy budget tracking
    energy: EnergyBudget,
    /// Maxwell's demon instance
    demon: MaxwellDemon,
    /// Phase state
    phase: CognitivePhase,
    /// History of thermodynamic events
    history: VecDeque<ThermodynamicEvent>,
    /// Boltzmann constant (normalized)
    k_b: f64,
}

/// Entropy tracking for cognitive system
#[derive(Debug)]
pub struct ThoughtEntropy {
    /// Current entropy level
    current: f64,
    /// Entropy production rate
    production_rate: f64,
    /// Entropy capacity
    capacity: f64,
    /// Entropy components
    components: HashMap<String, f64>,
}

/// Energy budget for cognitive operations
#[derive(Debug, Clone)]
pub struct EnergyBudget {
    /// Available energy
    available: f64,
    /// Total energy consumed
    consumed: f64,
    /// Energy from erasure
    erasure_cost: f64,
    /// Energy recovered from reversible computation
    recovered: f64,
}

/// Maxwell's Demon for cognitive sorting
#[derive(Debug)]
pub struct MaxwellDemon {
    /// Demon's memory (cost of operation)
    memory: Vec<bool>,
    /// Memory capacity
    capacity: usize,
    /// Work extracted
    work_extracted: f64,
    /// Information cost
    information_cost: f64,
    /// Operating state
    active: bool,
}

/// Phase states of cognitive matter
#[derive(Debug, Clone, PartialEq)]
pub enum CognitivePhase {
    /// Solid - highly ordered, low entropy
    Crystalline,
    /// Liquid - flowing thoughts, moderate entropy
    Fluid,
    /// Gas - chaotic, high entropy
    Gaseous,
    /// Critical point - phase transition
    Critical,
    /// Bose-Einstein condensate analog - unified consciousness
    Condensate,
}

/// A thermodynamic event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermodynamicEvent {
    pub event_type: EventType,
    pub entropy_change: f64,
    pub energy_change: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventType {
    Erasure,
    Computation,
    Measurement,
    PhaseTransition,
    DemonOperation,
    HeatDissipation,
}

impl CognitiveThermodynamics {
    /// Create a new cognitive thermodynamics system
    pub fn new(temperature: f64) -> Self {
        Self {
            temperature: temperature.max(0.001), // Avoid division by zero
            entropy: ThoughtEntropy::new(100.0),
            energy: EnergyBudget::new(1000.0),
            demon: MaxwellDemon::new(100),
            phase: CognitivePhase::Fluid,
            history: VecDeque::with_capacity(1000),
            k_b: 1.0, // Normalized Boltzmann constant
        }
    }

    /// Measure current cognitive temperature
    pub fn measure_temperature(&self) -> f64 {
        self.temperature
    }

    /// Set cognitive temperature
    pub fn set_temperature(&mut self, temp: f64) {
        let old_temp = self.temperature;
        self.temperature = temp.max(0.001);

        // Check for phase transition
        self.check_phase_transition(old_temp, self.temperature);
    }

    fn check_phase_transition(&mut self, old: f64, new: f64) {
        // Critical temperatures for phase transitions
        const T_FREEZE: f64 = 100.0;
        const T_BOIL: f64 = 500.0;
        const T_CRITICAL: f64 = 1000.0;
        const T_CONDENSATE: f64 = 10.0;

        let old_phase = self.phase.clone();

        self.phase = if new < T_CONDENSATE {
            CognitivePhase::Condensate
        } else if new < T_FREEZE {
            CognitivePhase::Crystalline
        } else if new < T_BOIL {
            CognitivePhase::Fluid
        } else if new < T_CRITICAL {
            CognitivePhase::Gaseous
        } else {
            CognitivePhase::Critical
        };

        if old_phase != self.phase {
            // Record phase transition
            self.record_event(ThermodynamicEvent {
                event_type: EventType::PhaseTransition,
                entropy_change: (new - old).abs() * 0.1,
                energy_change: -(new - old).abs() * self.k_b,
                timestamp: self.current_time(),
            });
        }
    }

    /// Compute Landauer cost of erasing n bits
    pub fn landauer_cost(&self, bits: usize) -> f64 {
        // E = n * k_B * T * ln(2)
        bits as f64 * self.k_b * self.temperature * std::f64::consts::LN_2
    }

    /// Erase information (irreversible)
    pub fn erase(&mut self, bits: usize) -> ErasureResult {
        let cost = self.landauer_cost(bits);

        if self.energy.available < cost {
            return ErasureResult {
                success: false,
                bits_erased: 0,
                energy_cost: 0.0,
                entropy_increase: 0.0,
            };
        }

        // Consume energy
        self.energy.available -= cost;
        self.energy.consumed += cost;
        self.energy.erasure_cost += cost;

        // Increase entropy (heat dissipation)
        let entropy_increase = bits as f64 * std::f64::consts::LN_2;
        self.entropy.current += entropy_increase;
        self.entropy.production_rate = entropy_increase;

        self.record_event(ThermodynamicEvent {
            event_type: EventType::Erasure,
            entropy_change: entropy_increase,
            energy_change: -cost,
            timestamp: self.current_time(),
        });

        ErasureResult {
            success: true,
            bits_erased: bits,
            energy_cost: cost,
            entropy_increase,
        }
    }

    /// Perform reversible computation
    pub fn reversible_compute<T>(
        &mut self,
        input: T,
        forward: impl Fn(T) -> T,
        _backward: impl Fn(T) -> T,
    ) -> T {
        // Reversible computation has no erasure cost
        // Only the logical transformation happens

        self.record_event(ThermodynamicEvent {
            event_type: EventType::Computation,
            entropy_change: 0.0, // Reversible = no entropy change
            energy_change: 0.0,
            timestamp: self.current_time(),
        });

        forward(input)
    }

    /// Perform measurement (gains information, increases entropy elsewhere)
    pub fn measure(&mut self, precision_bits: usize) -> MeasurementResult {
        // Measurement is fundamentally irreversible
        // Gains information but produces entropy

        let information_gained = precision_bits as f64;
        let entropy_cost = precision_bits as f64 * std::f64::consts::LN_2;
        let energy_cost = self.landauer_cost(precision_bits);

        self.entropy.current += entropy_cost;
        self.energy.available -= energy_cost;
        self.energy.consumed += energy_cost;

        self.record_event(ThermodynamicEvent {
            event_type: EventType::Measurement,
            entropy_change: entropy_cost,
            energy_change: -energy_cost,
            timestamp: self.current_time(),
        });

        MeasurementResult {
            information_gained,
            entropy_cost,
            energy_cost,
        }
    }

    /// Run Maxwell's demon to extract work
    pub fn run_demon(&mut self, operations: usize) -> DemonResult {
        if !self.demon.active {
            return DemonResult {
                work_extracted: 0.0,
                memory_used: 0,
                erasure_cost: 0.0,
                net_work: 0.0,
            };
        }

        let ops = operations.min(self.demon.capacity - self.demon.memory.len());
        if ops == 0 {
            // Demon must erase memory first
            let erase_cost = self.landauer_cost(self.demon.memory.len());
            self.demon.memory.clear();
            self.demon.information_cost += erase_cost;
            self.energy.available -= erase_cost;

            return DemonResult {
                work_extracted: 0.0,
                memory_used: 0,
                erasure_cost: erase_cost,
                net_work: -erase_cost,
            };
        }

        // Each operation records 1 bit and extracts k_B * T * ln(2) work
        let work_per_op = self.k_b * self.temperature * std::f64::consts::LN_2;
        let total_work = ops as f64 * work_per_op;

        for _ in 0..ops {
            self.demon.memory.push(true);
        }
        self.demon.work_extracted += total_work;

        self.record_event(ThermodynamicEvent {
            event_type: EventType::DemonOperation,
            entropy_change: -(ops as f64) * std::f64::consts::LN_2, // Local decrease
            energy_change: total_work,
            timestamp: self.current_time(),
        });

        DemonResult {
            work_extracted: total_work,
            memory_used: ops,
            erasure_cost: 0.0,
            net_work: total_work,
        }
    }

    /// Get current phase
    pub fn phase(&self) -> &CognitivePhase {
        &self.phase
    }

    /// Get entropy
    pub fn entropy(&self) -> &ThoughtEntropy {
        &self.entropy
    }

    /// Get energy budget
    pub fn energy(&self) -> &EnergyBudget {
        &self.energy
    }

    /// Add energy to the system
    pub fn add_energy(&mut self, amount: f64) {
        self.energy.available += amount;
    }

    /// Calculate free energy (available for work)
    pub fn free_energy(&self) -> f64 {
        // F = E - T*S
        self.energy.available - self.temperature * self.entropy.current
    }

    /// Calculate efficiency
    pub fn efficiency(&self) -> f64 {
        if self.energy.consumed == 0.0 {
            return 1.0;
        }
        self.energy.recovered / self.energy.consumed
    }

    /// Get Carnot efficiency limit
    pub fn carnot_limit(&self, cold_temp: f64) -> f64 {
        if self.temperature <= cold_temp {
            return 0.0;
        }
        1.0 - cold_temp / self.temperature
    }

    fn record_event(&mut self, event: ThermodynamicEvent) {
        self.history.push_back(event);
        if self.history.len() > 1000 {
            self.history.pop_front();
        }
    }

    fn current_time(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Get thermodynamic statistics
    pub fn statistics(&self) -> ThermodynamicStatistics {
        ThermodynamicStatistics {
            temperature: self.temperature,
            entropy: self.entropy.current,
            free_energy: self.free_energy(),
            energy_available: self.energy.available,
            efficiency: self.efficiency(),
            phase: self.phase.clone(),
            demon_work: self.demon.work_extracted,
        }
    }
}

impl ThoughtEntropy {
    /// Create new entropy tracker
    pub fn new(capacity: f64) -> Self {
        Self {
            current: 0.0,
            production_rate: 0.0,
            capacity,
            components: HashMap::new(),
        }
    }

    /// Get current entropy
    pub fn current(&self) -> f64 {
        self.current
    }

    /// Set entropy for a component
    pub fn set_component(&mut self, name: &str, entropy: f64) {
        self.components.insert(name.to_string(), entropy);
        self.current = self.components.values().sum();
    }

    /// Get entropy headroom
    pub fn headroom(&self) -> f64 {
        (self.capacity - self.current).max(0.0)
    }

    /// Is at maximum entropy?
    pub fn is_maximum(&self) -> bool {
        self.current >= self.capacity * 0.99
    }
}

impl EnergyBudget {
    /// Create new energy budget
    pub fn new(initial: f64) -> Self {
        Self {
            available: initial,
            consumed: 0.0,
            erasure_cost: 0.0,
            recovered: 0.0,
        }
    }

    /// Get available energy
    pub fn available(&self) -> f64 {
        self.available
    }

    /// Get total consumed
    pub fn consumed(&self) -> f64 {
        self.consumed
    }
}

impl MaxwellDemon {
    /// Create new Maxwell's demon
    pub fn new(capacity: usize) -> Self {
        Self {
            memory: Vec::with_capacity(capacity),
            capacity,
            work_extracted: 0.0,
            information_cost: 0.0,
            active: true,
        }
    }

    /// Activate demon
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate demon
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Get work extracted
    pub fn work_extracted(&self) -> f64 {
        self.work_extracted
    }

    /// Get net work (accounting for erasure)
    pub fn net_work(&self) -> f64 {
        self.work_extracted - self.information_cost
    }

    /// Memory usage fraction
    pub fn memory_usage(&self) -> f64 {
        self.memory.len() as f64 / self.capacity as f64
    }
}

/// Result of erasure operation
#[derive(Debug, Clone)]
pub struct ErasureResult {
    pub success: bool,
    pub bits_erased: usize,
    pub energy_cost: f64,
    pub entropy_increase: f64,
}

/// Result of measurement
#[derive(Debug, Clone)]
pub struct MeasurementResult {
    pub information_gained: f64,
    pub entropy_cost: f64,
    pub energy_cost: f64,
}

/// Result of demon operation
#[derive(Debug, Clone)]
pub struct DemonResult {
    pub work_extracted: f64,
    pub memory_used: usize,
    pub erasure_cost: f64,
    pub net_work: f64,
}

/// Thermodynamic statistics
#[derive(Debug, Clone)]
pub struct ThermodynamicStatistics {
    pub temperature: f64,
    pub entropy: f64,
    pub free_energy: f64,
    pub energy_available: f64,
    pub efficiency: f64,
    pub phase: CognitivePhase,
    pub demon_work: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermodynamics_creation() {
        let thermo = CognitiveThermodynamics::new(300.0);
        assert_eq!(thermo.measure_temperature(), 300.0);
    }

    #[test]
    fn test_landauer_cost() {
        let thermo = CognitiveThermodynamics::new(300.0);

        let cost_1bit = thermo.landauer_cost(1);
        let cost_2bits = thermo.landauer_cost(2);

        // Cost should scale linearly
        assert!((cost_2bits - 2.0 * cost_1bit).abs() < 0.001);
    }

    #[test]
    fn test_erasure() {
        let mut thermo = CognitiveThermodynamics::new(300.0);
        // Add enough energy for the erasure to succeed
        thermo.add_energy(10000.0);
        let initial_energy = thermo.energy().available();

        let result = thermo.erase(10);

        assert!(result.success);
        assert_eq!(result.bits_erased, 10);
        assert!(thermo.energy().available() < initial_energy);
        assert!(thermo.entropy().current() > 0.0);
    }

    #[test]
    fn test_reversible_computation() {
        let mut thermo = CognitiveThermodynamics::new(300.0);

        let input = 5;
        let output = thermo.reversible_compute(
            input,
            |x| x * 2, // forward
            |x| x / 2, // backward
        );

        assert_eq!(output, 10);
        // Reversible computation shouldn't increase entropy significantly
    }

    #[test]
    fn test_phase_transitions() {
        let mut thermo = CognitiveThermodynamics::new(300.0);

        // Start in Fluid phase
        assert_eq!(*thermo.phase(), CognitivePhase::Fluid);

        // Cool down
        thermo.set_temperature(50.0);
        assert_eq!(*thermo.phase(), CognitivePhase::Crystalline);

        // Heat up
        thermo.set_temperature(600.0);
        assert_eq!(*thermo.phase(), CognitivePhase::Gaseous);

        // Extreme cooling
        thermo.set_temperature(5.0);
        assert_eq!(*thermo.phase(), CognitivePhase::Condensate);
    }

    #[test]
    fn test_maxwell_demon() {
        let mut thermo = CognitiveThermodynamics::new(300.0);

        let result = thermo.run_demon(10);

        assert!(result.work_extracted > 0.0);
        assert_eq!(result.memory_used, 10);
    }

    #[test]
    fn test_free_energy() {
        let thermo = CognitiveThermodynamics::new(300.0);
        let free = thermo.free_energy();

        // Free energy should be positive initially
        assert!(free > 0.0);
    }

    #[test]
    fn test_entropy_components() {
        let mut entropy = ThoughtEntropy::new(100.0);

        entropy.set_component("perception", 10.0);
        entropy.set_component("memory", 15.0);

        assert_eq!(entropy.current(), 25.0);
        assert!(!entropy.is_maximum());
    }

    #[test]
    fn test_demon_memory_limit() {
        let mut thermo = CognitiveThermodynamics::new(300.0);

        // Fill demon memory
        for _ in 0..10 {
            thermo.run_demon(10);
        }

        // Demon should need to erase memory eventually
        let usage = thermo.demon.memory_usage();
        assert!(usage > 0.0);
    }
}
