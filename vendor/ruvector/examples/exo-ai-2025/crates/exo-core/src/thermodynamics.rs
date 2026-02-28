//! Landauer's Principle and Thermodynamic Efficiency Tracking
//!
//! This module implements thermodynamic efficiency metrics based on
//! Landauer's principle - the fundamental limit of computation.
//!
//! # Landauer's Principle
//!
//! Minimum energy to erase one bit of information at temperature T:
//! ```text
//! E_min = k_B * T * ln(2)
//! ```
//!
//! At room temperature (300K):
//! - E_min ≈ 0.018 eV ≈ 2.9 × 10⁻²¹ J per bit
//!
//! # Current State of Computing
//!
//! - Modern CMOS: ~1000× above Landauer limit
//! - Biological neurons: ~10× above Landauer limit
//! - Reversible computing: Potential 4000× improvement
//!
//! # Usage
//!
//! ```rust,no_run
//! use exo_core::thermodynamics::{ThermodynamicTracker, Operation};
//!
//! let tracker = ThermodynamicTracker::new(300.0); // Room temperature
//!
//! tracker.record_operation(Operation::BitErasure { count: 1000 });
//! tracker.record_operation(Operation::VectorSimilarity { dimensions: 384 });
//!
//! let report = tracker.efficiency_report();
//! println!("Efficiency ratio: {}x above Landauer", report.efficiency_ratio);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Boltzmann constant in joules per kelvin
pub const BOLTZMANN_K: f64 = 1.380649e-23;

/// Electron volt in joules
pub const EV_TO_JOULES: f64 = 1.602176634e-19;

/// Landauer limit at room temperature (300K) in joules
pub const LANDAUER_LIMIT_300K: f64 = 2.87e-21; // k_B * T * ln(2)

/// Landauer limit at room temperature in electron volts
pub const LANDAUER_LIMIT_300K_EV: f64 = 0.0179; // ~0.018 eV

/// Compute Landauer limit for a given temperature
///
/// # Arguments
/// * `temperature_kelvin` - Temperature in Kelvin
///
/// # Returns
/// * Minimum energy per bit erasure in joules
pub fn landauer_limit(temperature_kelvin: f64) -> f64 {
    BOLTZMANN_K * temperature_kelvin * std::f64::consts::LN_2
}

/// Types of computational operations for energy tracking
#[derive(Debug, Clone, Copy)]
pub enum Operation {
    /// Bit erasure (irreversible operation)
    BitErasure { count: u64 },

    /// Bit copy (theoretically reversible)
    BitCopy { count: u64 },

    /// Vector similarity computation
    VectorSimilarity { dimensions: usize },

    /// Matrix-vector multiplication
    MatrixVectorMultiply { rows: usize, cols: usize },

    /// Neural network forward pass
    NeuralForward { parameters: u64 },

    /// Memory read (near-reversible)
    MemoryRead { bytes: u64 },

    /// Memory write (includes erasure)
    MemoryWrite { bytes: u64 },

    /// HNSW graph traversal
    GraphTraversal { hops: u64 },

    /// Custom operation with known bit erasures
    Custom { bit_erasures: u64 },
}

impl Operation {
    /// Estimate the number of bit erasures for this operation
    ///
    /// These are rough estimates based on typical implementations.
    /// Actual values depend on hardware and implementation details.
    pub fn estimated_bit_erasures(&self) -> u64 {
        match self {
            Operation::BitErasure { count } => *count,
            Operation::BitCopy { count } => *count / 10, // Mostly reversible
            Operation::VectorSimilarity { dimensions } => {
                // ~32 ops per dimension, ~1 erasure per op
                (*dimensions as u64) * 32
            }
            Operation::MatrixVectorMultiply { rows, cols } => {
                // 2*N*M ops for NxM matrix
                (*rows as u64) * (*cols as u64) * 2
            }
            Operation::NeuralForward { parameters } => {
                // ~2 erasures per parameter (multiply-accumulate)
                parameters * 2
            }
            Operation::MemoryRead { bytes } => {
                // Mostly reversible, small overhead
                bytes * 8 / 100
            }
            Operation::MemoryWrite { bytes } => {
                // Write = read + erase old + write new
                bytes * 8 * 2
            }
            Operation::GraphTraversal { hops } => {
                // ~10 comparisons per hop
                hops * 10
            }
            Operation::Custom { bit_erasures } => *bit_erasures,
        }
    }
}

/// Energy estimate for an operation
#[derive(Debug, Clone, Copy)]
pub struct EnergyEstimate {
    /// Theoretical minimum (Landauer limit)
    pub landauer_minimum_joules: f64,

    /// Estimated actual energy (current technology)
    pub estimated_actual_joules: f64,

    /// Efficiency ratio (actual / minimum)
    pub efficiency_ratio: f64,

    /// Number of bit erasures
    pub bit_erasures: u64,
}

/// Thermodynamic efficiency tracker
///
/// Tracks computational operations and calculates energy efficiency
/// relative to the Landauer limit.
pub struct ThermodynamicTracker {
    /// Operating temperature in Kelvin
    temperature: f64,

    /// Landauer limit at operating temperature
    landauer_limit: f64,

    /// Total bit erasures recorded
    total_erasures: Arc<AtomicU64>,

    /// Total operations recorded
    total_operations: Arc<AtomicU64>,

    /// Assumed efficiency multiplier above Landauer (typical: 1000x for CMOS)
    technology_multiplier: f64,
}

impl ThermodynamicTracker {
    /// Create a new tracker at the specified temperature
    ///
    /// # Arguments
    /// * `temperature_kelvin` - Operating temperature (default: 300K room temp)
    pub fn new(temperature_kelvin: f64) -> Self {
        Self {
            temperature: temperature_kelvin,
            landauer_limit: landauer_limit(temperature_kelvin),
            total_erasures: Arc::new(AtomicU64::new(0)),
            total_operations: Arc::new(AtomicU64::new(0)),
            technology_multiplier: 1000.0, // Current CMOS ~1000x above limit
        }
    }

    /// Create a tracker at room temperature (300K)
    pub fn room_temperature() -> Self {
        Self::new(300.0)
    }

    /// Set the technology multiplier
    ///
    /// - CMOS 2024: ~1000x
    /// - Biological: ~10x
    /// - Reversible (theoretical): ~1x
    /// - Future neuromorphic: ~100x
    pub fn with_technology_multiplier(mut self, multiplier: f64) -> Self {
        self.technology_multiplier = multiplier;
        self
    }

    /// Record an operation
    pub fn record_operation(&self, operation: Operation) {
        let erasures = operation.estimated_bit_erasures();
        self.total_erasures.fetch_add(erasures, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Estimate energy for an operation
    pub fn estimate_energy(&self, operation: Operation) -> EnergyEstimate {
        let bit_erasures = operation.estimated_bit_erasures();
        let landauer_minimum = (bit_erasures as f64) * self.landauer_limit;
        let estimated_actual = landauer_minimum * self.technology_multiplier;

        EnergyEstimate {
            landauer_minimum_joules: landauer_minimum,
            estimated_actual_joules: estimated_actual,
            efficiency_ratio: self.technology_multiplier,
            bit_erasures,
        }
    }

    /// Get total bit erasures recorded
    pub fn total_erasures(&self) -> u64 {
        self.total_erasures.load(Ordering::Relaxed)
    }

    /// Get total operations recorded
    pub fn total_operations(&self) -> u64 {
        self.total_operations.load(Ordering::Relaxed)
    }

    /// Calculate total theoretical minimum energy (Landauer limit)
    pub fn total_landauer_minimum(&self) -> f64 {
        (self.total_erasures() as f64) * self.landauer_limit
    }

    /// Calculate estimated actual energy usage
    pub fn total_estimated_energy(&self) -> f64 {
        self.total_landauer_minimum() * self.technology_multiplier
    }

    /// Generate an efficiency report
    pub fn efficiency_report(&self) -> EfficiencyReport {
        let total_erasures = self.total_erasures();
        let landauer_minimum = self.total_landauer_minimum();
        let estimated_actual = self.total_estimated_energy();

        // Calculate potential savings with reversible computing
        let reversible_potential = estimated_actual - landauer_minimum;

        EfficiencyReport {
            temperature_kelvin: self.temperature,
            landauer_limit_per_bit: self.landauer_limit,
            total_bit_erasures: total_erasures,
            total_operations: self.total_operations(),
            landauer_minimum_joules: landauer_minimum,
            landauer_minimum_ev: landauer_minimum / EV_TO_JOULES,
            estimated_actual_joules: estimated_actual,
            efficiency_ratio: self.technology_multiplier,
            reversible_savings_potential: reversible_potential,
            reversible_improvement_factor: self.technology_multiplier,
        }
    }

    /// Reset all counters
    pub fn reset(&self) {
        self.total_erasures.store(0, Ordering::Relaxed);
        self.total_operations.store(0, Ordering::Relaxed);
    }
}

impl Default for ThermodynamicTracker {
    fn default() -> Self {
        Self::room_temperature()
    }
}

/// Efficiency report
#[derive(Debug, Clone)]
pub struct EfficiencyReport {
    /// Operating temperature
    pub temperature_kelvin: f64,

    /// Landauer limit per bit at operating temperature
    pub landauer_limit_per_bit: f64,

    /// Total irreversible bit erasures
    pub total_bit_erasures: u64,

    /// Total operations tracked
    pub total_operations: u64,

    /// Theoretical minimum energy (Landauer limit)
    pub landauer_minimum_joules: f64,

    /// Landauer minimum in electron volts
    pub landauer_minimum_ev: f64,

    /// Estimated actual energy with current technology
    pub estimated_actual_joules: f64,

    /// How many times above Landauer limit
    pub efficiency_ratio: f64,

    /// Potential energy savings with reversible computing
    pub reversible_savings_potential: f64,

    /// Improvement factor possible with reversible computing
    pub reversible_improvement_factor: f64,
}

impl std::fmt::Display for EfficiencyReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Thermodynamic Efficiency Report ===")?;
        writeln!(f, "Temperature: {:.1}K", self.temperature_kelvin)?;
        writeln!(
            f,
            "Landauer limit: {:.2e} J/bit",
            self.landauer_limit_per_bit
        )?;
        writeln!(f)?;
        writeln!(f, "Operations tracked: {}", self.total_operations)?;
        writeln!(f, "Total bit erasures: {}", self.total_bit_erasures)?;
        writeln!(f)?;
        writeln!(
            f,
            "Theoretical minimum: {:.2e} J ({:.2e} eV)",
            self.landauer_minimum_joules, self.landauer_minimum_ev
        )?;
        writeln!(
            f,
            "Estimated actual:   {:.2e} J",
            self.estimated_actual_joules
        )?;
        writeln!(
            f,
            "Efficiency ratio:   {:.0}× above Landauer",
            self.efficiency_ratio
        )?;
        writeln!(f)?;
        writeln!(f, "Reversible computing potential:")?;
        writeln!(
            f,
            "  - Savings: {:.2e} J ({:.1}%)",
            self.reversible_savings_potential,
            (self.reversible_savings_potential / self.estimated_actual_joules) * 100.0
        )?;
        writeln!(
            f,
            "  - Improvement factor: {:.0}×",
            self.reversible_improvement_factor
        )?;
        Ok(())
    }
}

/// Technology profiles for different computing paradigms
pub mod technology_profiles {
    /// Current CMOS technology (~1000× above Landauer)
    pub const CMOS_2024: f64 = 1000.0;

    /// Biological neurons (~10× above Landauer)
    pub const BIOLOGICAL: f64 = 10.0;

    /// Future neuromorphic (~100× above Landauer)
    pub const NEUROMORPHIC_PROJECTED: f64 = 100.0;

    /// Reversible computing (approaching 1× limit)
    pub const REVERSIBLE_IDEAL: f64 = 1.0;

    /// Near-term reversible (~10× above Landauer)
    pub const REVERSIBLE_2028: f64 = 10.0;

    /// Superconducting qubits (cold, but higher per operation)
    pub const SUPERCONDUCTING: f64 = 100.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landauer_limit_room_temp() {
        let limit = landauer_limit(300.0);
        // Should be approximately 2.87e-21 J
        assert!((limit - 2.87e-21).abs() < 1e-22);
    }

    #[test]
    fn test_tracker_operations() {
        let tracker = ThermodynamicTracker::room_temperature();

        tracker.record_operation(Operation::BitErasure { count: 1000 });
        tracker.record_operation(Operation::VectorSimilarity { dimensions: 384 });

        assert_eq!(tracker.total_operations(), 2);
        assert!(tracker.total_erasures() > 1000); // Includes vector ops
    }

    #[test]
    fn test_energy_estimate() {
        let tracker = ThermodynamicTracker::room_temperature();
        let estimate = tracker.estimate_energy(Operation::BitErasure { count: 1 });

        assert!((estimate.landauer_minimum_joules - LANDAUER_LIMIT_300K).abs() < 1e-22);
        assert_eq!(estimate.efficiency_ratio, 1000.0);
    }

    #[test]
    fn test_efficiency_report() {
        let tracker = ThermodynamicTracker::room_temperature().with_technology_multiplier(1000.0);

        tracker.record_operation(Operation::BitErasure { count: 1_000_000 });

        let report = tracker.efficiency_report();

        assert_eq!(report.total_bit_erasures, 1_000_000);
        assert_eq!(report.efficiency_ratio, 1000.0);
        assert!(report.reversible_savings_potential > 0.0);
    }

    #[test]
    fn test_technology_profiles() {
        // Verify reversible computing is most efficient
        assert!(technology_profiles::REVERSIBLE_IDEAL < technology_profiles::BIOLOGICAL);
        assert!(technology_profiles::BIOLOGICAL < technology_profiles::NEUROMORPHIC_PROJECTED);
        assert!(technology_profiles::NEUROMORPHIC_PROJECTED < technology_profiles::CMOS_2024);
    }
}
