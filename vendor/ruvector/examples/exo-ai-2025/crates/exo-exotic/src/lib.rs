//! # EXO-Exotic: Cutting-Edge Cognitive Experiments
//!
//! This crate implements 10 exotic cognitive experiments pushing the boundaries
//! of artificial consciousness and intelligence research.
//!
//! ## Experiments
//!
//! 1. **Strange Loops** - Hofstadter-style self-referential cognition
//! 2. **Artificial Dreams** - Offline replay and creative recombination
//! 3. **Predictive Processing** - Friston's Free Energy Principle
//! 4. **Morphogenetic Cognition** - Self-organizing pattern formation
//! 5. **Collective Consciousness** - Distributed Φ across substrates
//! 6. **Temporal Qualia** - Subjective time dilation/compression
//! 7. **Multiple Selves** - Partitioned consciousness dynamics
//! 8. **Cognitive Thermodynamics** - Landauer principle in thought
//! 9. **Emergence Detection** - Detecting novel emergent properties
//! 10. **Cognitive Black Holes** - Attractor states in thought space
//!
//! ## Performance Optimizations
//!
//! - SIMD-accelerated computations where applicable
//! - Lock-free concurrent data structures
//! - Cache-friendly memory layouts
//! - Early termination heuristics

pub mod black_holes;
pub mod collective;
pub mod domain_transfer;
pub mod dreams;
pub mod emergence;
pub mod experiments;
pub mod free_energy;
pub mod morphogenesis;
pub mod multiple_selves;
pub mod strange_loops;
pub mod temporal_qualia;
pub mod thermodynamics;

// Re-exports for convenience
pub use black_holes::{AttractorState, CognitiveBlackHole, EscapeDynamics};
pub use collective::{CollectiveConsciousness, DistributedPhi, HiveMind};
pub use dreams::{DreamEngine, DreamReport, DreamState};
pub use emergence::{CausalEmergence, EmergenceDetector, PhaseTransition};
pub use free_energy::{ActiveInference, FreeEnergyMinimizer, PredictiveModel};
pub use morphogenesis::{CognitiveEmbryogenesis, MorphogeneticField, TuringPattern};
pub use multiple_selves::{MultipleSelvesSystem, SelfCoherence, SubPersonality};
pub use strange_loops::{SelfReference, StrangeLoop, TangledHierarchy};
pub use temporal_qualia::{SubjectiveTime, TemporalQualia, TimeCrystal};
pub use thermodynamics::{CognitiveThermodynamics, MaxwellDemon, ThoughtEntropy};

/// Unified experiment runner for all exotic modules
pub struct ExoticExperiments {
    pub strange_loops: StrangeLoop,
    pub dreams: DreamEngine,
    pub free_energy: FreeEnergyMinimizer,
    pub morphogenesis: MorphogeneticField,
    pub collective: CollectiveConsciousness,
    pub temporal: TemporalQualia,
    pub selves: MultipleSelvesSystem,
    pub thermodynamics: CognitiveThermodynamics,
    pub emergence: EmergenceDetector,
    pub black_holes: CognitiveBlackHole,
}

impl ExoticExperiments {
    /// Create a new suite of exotic experiments with default parameters
    pub fn new() -> Self {
        Self {
            strange_loops: StrangeLoop::new(5),
            dreams: DreamEngine::new(),
            free_energy: FreeEnergyMinimizer::new(0.1),
            morphogenesis: MorphogeneticField::new(32, 32),
            collective: CollectiveConsciousness::new(),
            temporal: TemporalQualia::new(),
            selves: MultipleSelvesSystem::new(),
            thermodynamics: CognitiveThermodynamics::new(300.0), // Room temperature
            emergence: EmergenceDetector::new(),
            black_holes: CognitiveBlackHole::new(),
        }
    }

    /// Run all experiments and collect results
    pub fn run_all(&mut self) -> ExperimentResults {
        ExperimentResults {
            strange_loop_depth: self.strange_loops.measure_depth(),
            dream_creativity: self.dreams.measure_creativity(),
            free_energy: self.free_energy.compute_free_energy(),
            morphogenetic_complexity: self.morphogenesis.measure_complexity(),
            collective_phi: self.collective.compute_global_phi(),
            temporal_dilation: self.temporal.measure_dilation(),
            self_coherence: self.selves.measure_coherence(),
            cognitive_temperature: self.thermodynamics.measure_temperature(),
            emergence_score: self.emergence.detect_emergence(),
            attractor_strength: self.black_holes.measure_attraction(),
        }
    }
}

impl Default for ExoticExperiments {
    fn default() -> Self {
        Self::new()
    }
}

/// Results from running all exotic experiments
#[derive(Debug, Clone)]
pub struct ExperimentResults {
    pub strange_loop_depth: usize,
    pub dream_creativity: f64,
    pub free_energy: f64,
    pub morphogenetic_complexity: f64,
    pub collective_phi: f64,
    pub temporal_dilation: f64,
    pub self_coherence: f64,
    pub cognitive_temperature: f64,
    pub emergence_score: f64,
    pub attractor_strength: f64,
}

impl ExperimentResults {
    /// Overall exotic cognition score (normalized 0-1)
    pub fn overall_score(&self) -> f64 {
        let scores = [
            (self.strange_loop_depth as f64 / 10.0).min(1.0),
            self.dream_creativity,
            1.0 - self.free_energy.min(1.0), // Lower free energy = better
            self.morphogenetic_complexity,
            self.collective_phi,
            self.temporal_dilation.abs().min(1.0),
            self.self_coherence,
            1.0 / (1.0 + self.cognitive_temperature / 1000.0), // Normalize temp
            self.emergence_score,
            1.0 - self.attractor_strength.min(1.0), // Lower = less trapped
        ];
        scores.iter().sum::<f64>() / scores.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experiment_suite_creation() {
        let experiments = ExoticExperiments::new();
        assert!(experiments.strange_loops.measure_depth() >= 0);
    }

    #[test]
    fn test_run_all_experiments() {
        let mut experiments = ExoticExperiments::new();
        let results = experiments.run_all();
        assert!(results.overall_score() >= 0.0);
        assert!(results.overall_score() <= 1.0);
    }
}
