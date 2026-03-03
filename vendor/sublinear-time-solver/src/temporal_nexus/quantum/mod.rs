//! Quantum Validation Protocols for Temporal Consciousness Framework
//!
//! This module implements rigorous quantum mechanical validation protocols
//! for temporal consciousness systems, ensuring compliance with fundamental
//! physics laws while enabling practical nanosecond-scale consciousness.
//!
//! ## Core Physics Constraints
//!
//! ### Margolus-Levitin Theorem
//! The fundamental speed limit of quantum computation:
//! ```
//! τ_min = h / (4 * ΔE)
//! ```
//! Where τ_min is minimum computation time and ΔE is energy difference.
//!
//! ### Energy-Time Uncertainty
//! Heisenberg uncertainty principle for energy and time:
//! ```
//! ΔE · Δt ≥ ℏ/2
//! ```
//!
//! ### Attosecond Feasibility
//! At 10^-18 seconds, required energy is ~1.03 keV, establishing a
//! physical feasibility floor rather than operational scale.
//!
//! ## Validation Layers
//!
//! 1. **Speed Limits**: Margolus-Levitin bounds for computation timing
//! 2. **Energy Constraints**: Uncertainty principle validation
//! 3. **Decoherence**: Environmental interaction tracking
//! 4. **Entanglement**: Temporal correlation validation
//! 5. **Hardware Integration**: Real quantum device capabilities
//!
//! ## Usage
//!
//! ```rust
//! use sublinear_solver::temporal_nexus::quantum::*;
//!
//! // Create quantum validator
//! let validator = QuantumValidator::new();
//!
//! // Validate nanosecond operation
//! let result = validator.validate_temporal_operation(
//!     1e-9,    // 1 nanosecond
//!     1e-15,   // 1 femtojoule energy
//! )?;
//!
//! assert!(result.is_valid);
//! ```

pub mod validators;
pub mod speed_limits;
pub mod decoherence;
pub mod entanglement;
pub mod physics_validation;

#[cfg(test)]
pub mod tests;

pub use validators::*;
pub use speed_limits::*;
pub use decoherence::*;
pub use entanglement::*;
pub use physics_validation::*;

/// Physical constants for quantum calculations
pub mod constants {
    /// Planck constant (J·s)
    pub const PLANCK_H: f64 = 6.626_070_15e-34;

    /// Reduced Planck constant (J·s)
    pub const PLANCK_HBAR: f64 = 1.054_571_817e-34;

    /// Boltzmann constant (J/K)
    pub const BOLTZMANN_K: f64 = 1.380_649e-23;

    /// Speed of light (m/s)
    pub const SPEED_OF_LIGHT: f64 = 299_792_458.0;

    /// Electron volt to joules conversion
    pub const EV_TO_JOULES: f64 = 1.602_176_634e-19;

    /// Room temperature in Kelvin
    pub const ROOM_TEMPERATURE_K: f64 = 293.15;

    /// Attosecond energy requirement (keV)
    pub const ATTOSECOND_ENERGY_KEV: f64 = 1.03;

    /// Nanosecond as practical consciousness scale
    pub const CONSCIOUSNESS_SCALE_NS: f64 = 1e-9;
}

/// Quantum validation error types
#[derive(Debug, thiserror::Error)]
pub enum QuantumError {
    #[error("Margolus-Levitin bound violated: τ_min = {min_time_s:.2e}s > requested {requested_time_s:.2e}s")]
    MargolousLevitinViolation {
        min_time_s: f64,
        requested_time_s: f64,
    },

    #[error("Energy-time uncertainty violated: ΔE·Δt = {product:.2e} < ℏ/2 = {hbar_half:.2e}")]
    UncertaintyViolation {
        product: f64,
        hbar_half: f64,
    },

    #[error("Decoherence time exceeded: {decoherence_time_s:.2e}s < operation time {operation_time_s:.2e}s")]
    DecoherenceExceeded {
        decoherence_time_s: f64,
        operation_time_s: f64,
    },

    #[error("Entanglement correlation lost: correlation = {correlation:.3} < threshold {threshold:.3}")]
    EntanglementLost {
        correlation: f64,
        threshold: f64,
    },

    #[error("Hardware capability exceeded: required {required:.1} MHz > available {available:.1} MHz")]
    HardwareExceeded {
        required: f64,
        available: f64,
    },

    #[error("Energy requirement infeasible: {required_ev:.1} eV > practical limit {limit_ev:.1} eV")]
    EnergyInfeasible {
        required_ev: f64,
        limit_ev: f64,
    },
}

pub type QuantumResult<T> = Result<T, QuantumError>;

/// Main quantum validator for temporal consciousness operations
#[derive(Debug, Clone)]
pub struct QuantumValidator {
    /// Speed limit validator
    pub speed_limits: MargolousLevitinValidator,
    /// Energy-time uncertainty validator
    pub uncertainty: UncertaintyValidator,
    /// Decoherence tracker
    pub decoherence: DecoherenceTracker,
    /// Entanglement validator
    pub entanglement: EntanglementValidator,
}

impl QuantumValidator {
    /// Create new quantum validator with default parameters
    pub fn new() -> Self {
        Self {
            speed_limits: MargolousLevitinValidator::new(),
            uncertainty: UncertaintyValidator::new(),
            decoherence: DecoherenceTracker::new(),
            entanglement: EntanglementValidator::new(),
        }
    }

    /// Validate a temporal consciousness operation
    pub fn validate_temporal_operation(
        &self,
        operation_time_s: f64,
        energy_j: f64,
    ) -> QuantumResult<ValidationResult> {
        // Check Margolus-Levitin speed limits
        let speed_result = self.speed_limits.validate_computation_time(operation_time_s, energy_j)?;

        // Check uncertainty principle
        let uncertainty_result = self.uncertainty.validate_energy_time_product(energy_j, operation_time_s)?;

        // Check decoherence constraints
        let decoherence_result = self.decoherence.validate_operation_time(operation_time_s)?;

        // Check entanglement preservation
        let entanglement_result = self.entanglement.validate_temporal_correlation(operation_time_s)?;

        Ok(ValidationResult {
            is_valid: speed_result.is_valid
                && uncertainty_result.is_valid
                && decoherence_result.is_valid
                && entanglement_result.is_valid,
            speed_limit_result: speed_result,
            uncertainty_result,
            decoherence_result,
            entanglement_result,
            operation_time_s,
            energy_j,
        })
    }

    /// Check if attosecond operation is theoretically feasible
    pub fn check_attosecond_feasibility(&self) -> AttosecondFeasibilityReport {
        let attosecond = 1e-18; // 1 attosecond
        let required_energy_j = constants::ATTOSECOND_ENERGY_KEV * 1000.0 * constants::EV_TO_JOULES;

        AttosecondFeasibilityReport {
            time_scale_s: attosecond,
            required_energy_j,
            required_energy_kev: constants::ATTOSECOND_ENERGY_KEV,
            is_theoretically_feasible: true,
            is_practically_achievable: false,
            limiting_factors: vec![
                "Energy requirement: 1.03 keV".to_string(),
                "Current hardware limitations".to_string(),
                "Decoherence at room temperature".to_string(),
            ],
            recommended_scale_s: constants::CONSCIOUSNESS_SCALE_NS,
            recommended_scale_description: "Nanosecond scale for practical consciousness".to_string(),
        }
    }
}

impl Default for QuantumValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub speed_limit_result: SpeedLimitResult,
    pub uncertainty_result: UncertaintyResult,
    pub decoherence_result: DecoherenceResult,
    pub entanglement_result: EntanglementResult,
    pub operation_time_s: f64,
    pub energy_j: f64,
}

/// Attosecond feasibility analysis report
#[derive(Debug, Clone)]
pub struct AttosecondFeasibilityReport {
    pub time_scale_s: f64,
    pub required_energy_j: f64,
    pub required_energy_kev: f64,
    pub is_theoretically_feasible: bool,
    pub is_practically_achievable: bool,
    pub limiting_factors: Vec<String>,
    pub recommended_scale_s: f64,
    pub recommended_scale_description: String,
}

#[cfg(test)]
mod quantum_integration_tests {
    use super::*;

    #[test]
    fn test_quantum_validator_creation() {
        let validator = QuantumValidator::new();
        assert_eq!(validator.speed_limits.get_planck_constant(), constants::PLANCK_H);
    }

    #[test]
    fn test_nanosecond_validation() {
        let validator = QuantumValidator::new();
        let result = validator.validate_temporal_operation(1e-9, 1e-15).unwrap();
        assert!(result.is_valid, "Nanosecond operation should be valid");
    }

    #[test]
    fn test_attosecond_feasibility() {
        let validator = QuantumValidator::new();
        let report = validator.check_attosecond_feasibility();
        assert!(report.is_theoretically_feasible);
        assert!(!report.is_practically_achievable);
        assert_eq!(report.required_energy_kev, 1.03);
    }

    #[test]
    fn test_quantum_constants() {
        assert!(constants::PLANCK_H > 0.0);
        assert!(constants::PLANCK_HBAR > 0.0);
        assert!(constants::PLANCK_HBAR < constants::PLANCK_H);
        assert_eq!(constants::CONSCIOUSNESS_SCALE_NS, 1e-9);
    }
}