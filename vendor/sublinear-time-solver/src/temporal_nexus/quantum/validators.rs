//! Energy-Time Uncertainty Relation Validators
//!
//! This module implements validation of the Heisenberg uncertainty principle
//! for energy and time, ensuring that temporal consciousness operations
//! respect fundamental quantum mechanical constraints.
//!
//! ## Heisenberg Uncertainty Principle
//!
//! The energy-time uncertainty relation states:
//! ```
//! ΔE · Δt ≥ ℏ/2
//! ```
//!
//! Where:
//! - ΔE: energy uncertainty (or energy scale of the process)
//! - Δt: time uncertainty (or duration of the process)
//! - ℏ: reduced Planck constant
//!
//! This principle fundamentally limits the precision with which we can
//! simultaneously know the energy and timing of quantum processes.

use crate::temporal_nexus::quantum::{constants, QuantumError, QuantumResult};

/// Energy-time uncertainty principle validator
#[derive(Debug, Clone)]
pub struct UncertaintyValidator {
    /// Reduced Planck constant
    hbar: f64,
    /// Minimum energy scale for consciousness (J)
    min_consciousness_energy: f64,
    /// Temperature for thermal energy calculations (K)
    temperature: f64,
}

impl UncertaintyValidator {
    /// Create new uncertainty validator
    pub fn new() -> Self {
        Self {
            hbar: constants::PLANCK_HBAR,
            min_consciousness_energy: 1e-18, // 1 attojoule minimum
            temperature: constants::ROOM_TEMPERATURE_K,
        }
    }

    /// Create validator with custom temperature
    pub fn with_temperature(temperature_k: f64) -> Self {
        Self {
            hbar: constants::PLANCK_HBAR,
            min_consciousness_energy: 1e-18,
            temperature: temperature_k,
        }
    }

    /// Calculate minimum uncertainty product
    pub fn minimum_uncertainty_product(&self) -> f64 {
        self.hbar / 2.0
    }

    /// Calculate thermal energy at current temperature
    pub fn thermal_energy(&self) -> f64 {
        constants::BOLTZMANN_K * self.temperature
    }

    /// Validate energy-time uncertainty relation
    pub fn validate_energy_time_product(
        &self,
        energy_j: f64,
        time_s: f64,
    ) -> QuantumResult<UncertaintyResult> {
        let product = energy_j * time_s;
        let min_product = self.minimum_uncertainty_product();
        let is_valid = product >= min_product;

        // Calculate energy in electron volts for reporting
        let energy_ev = energy_j / constants::EV_TO_JOULES;
        let thermal_energy_j = self.thermal_energy();
        let thermal_energy_ev = thermal_energy_j / constants::EV_TO_JOULES;

        if !is_valid {
            return Err(QuantumError::UncertaintyViolation {
                product,
                hbar_half: min_product,
            });
        }

        Ok(UncertaintyResult {
            is_valid,
            energy_j,
            energy_ev,
            time_s,
            uncertainty_product: product,
            minimum_product: min_product,
            margin: product / min_product,
            thermal_energy_j,
            thermal_energy_ev,
            temperature_k: self.temperature,
            energy_scale_classification: self.classify_energy_scale(energy_j),
        })
    }

    /// Classify energy scale for consciousness operations
    pub fn classify_energy_scale(&self, energy_j: f64) -> EnergyScale {
        let energy_ev = energy_j / constants::EV_TO_JOULES;
        let _thermal_ev = self.thermal_energy() / constants::EV_TO_JOULES;

        if energy_ev < 1e-6 {
            EnergyScale::SubAttoElectronVolt
        } else if energy_ev < 1e-3 {
            EnergyScale::AttoElectronVolt
        } else if energy_ev < 1.0 {
            EnergyScale::MilliElectronVolt
        } else if energy_ev < 1000.0 {
            EnergyScale::ElectronVolt
        } else if energy_ev < 1e6 {
            EnergyScale::KiloElectronVolt
        } else {
            EnergyScale::MegaElectronVolt
        }
    }

    /// Calculate required energy for given time constraint
    pub fn calculate_required_energy(&self, time_constraint_s: f64) -> f64 {
        self.minimum_uncertainty_product() / time_constraint_s
    }

    /// Calculate maximum time for given energy budget
    pub fn calculate_maximum_time(&self, energy_budget_j: f64) -> f64 {
        if energy_budget_j <= 0.0 {
            return 0.0;
        }
        energy_budget_j / self.minimum_uncertainty_product()
    }

    /// Validate nanosecond consciousness energy requirements
    pub fn validate_nanosecond_consciousness(&self) -> UncertaintyResult {
        let nanosecond = 1e-9;
        let required_energy = self.calculate_required_energy(nanosecond);

        self.validate_energy_time_product(required_energy, nanosecond)
            .unwrap_or_else(|_| UncertaintyResult {
                is_valid: false,
                energy_j: required_energy,
                energy_ev: required_energy / constants::EV_TO_JOULES,
                time_s: nanosecond,
                uncertainty_product: required_energy * nanosecond,
                minimum_product: self.minimum_uncertainty_product(),
                margin: 1.0,
                thermal_energy_j: self.thermal_energy(),
                thermal_energy_ev: self.thermal_energy() / constants::EV_TO_JOULES,
                temperature_k: self.temperature,
                energy_scale_classification: self.classify_energy_scale(required_energy),
            })
    }

    /// Analyze uncertainty constraints across time scales
    pub fn analyze_time_scales(&self) -> UncertaintyAnalysis {
        let scales = vec![
            ("attosecond", 1e-18),
            ("femtosecond", 1e-15),
            ("picosecond", 1e-12),
            ("nanosecond", 1e-9),
            ("microsecond", 1e-6),
            ("millisecond", 1e-3),
        ];

        let mut constraints = Vec::new();

        for (name, time_s) in scales {
            let required_energy_j = self.calculate_required_energy(time_s);
            let required_energy_ev = required_energy_j / constants::EV_TO_JOULES;
            let thermal_ratio = required_energy_j / self.thermal_energy();

            constraints.push(UncertaintyConstraint {
                scale_name: name.to_string(),
                time_s,
                required_energy_j,
                required_energy_ev,
                thermal_energy_ratio: thermal_ratio,
                is_above_thermal: thermal_ratio > 1.0,
                is_feasible: required_energy_ev < 1000.0, // 1 keV practical limit
                energy_scale: self.classify_energy_scale(required_energy_j),
            });
        }

        UncertaintyAnalysis {
            temperature_k: self.temperature,
            thermal_energy_j: self.thermal_energy(),
            thermal_energy_ev: self.thermal_energy() / constants::EV_TO_JOULES,
            hbar: self.hbar,
            minimum_product: self.minimum_uncertainty_product(),
            constraints,
            recommended_scale: "nanosecond".to_string(),
        }
    }
}

impl Default for UncertaintyValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of uncertainty relation validation
#[derive(Debug, Clone)]
pub struct UncertaintyResult {
    pub is_valid: bool,
    pub energy_j: f64,
    pub energy_ev: f64,
    pub time_s: f64,
    pub uncertainty_product: f64,
    pub minimum_product: f64,
    pub margin: f64,
    pub thermal_energy_j: f64,
    pub thermal_energy_ev: f64,
    pub temperature_k: f64,
    pub energy_scale_classification: EnergyScale,
}

impl UncertaintyResult {
    /// Get human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Uncertainty Check: {} (ΔE·Δt = {:.2e}, min = {:.2e}, margin = {:.1}x)",
            if self.is_valid { "PASS" } else { "FAIL" },
            self.uncertainty_product,
            self.minimum_product,
            self.margin
        )
    }
}

/// Energy scale classification for consciousness operations
#[derive(Debug, Clone, PartialEq)]
pub enum EnergyScale {
    SubAttoElectronVolt,  // < 1 aeV
    AttoElectronVolt,     // 1 aeV - 1 feV
    MilliElectronVolt,    // 1 feV - 1 eV
    ElectronVolt,         // 1 eV - 1 keV
    KiloElectronVolt,     // 1 keV - 1 MeV
    MegaElectronVolt,     // > 1 MeV
}

impl EnergyScale {
    pub fn description(&self) -> &'static str {
        match self {
            EnergyScale::SubAttoElectronVolt => "Sub-atto-eV (quantum vacuum scale)",
            EnergyScale::AttoElectronVolt => "Atto-eV (ultra-low energy)",
            EnergyScale::MilliElectronVolt => "Milli-eV (molecular vibrations)",
            EnergyScale::ElectronVolt => "eV (atomic scale)",
            EnergyScale::KiloElectronVolt => "keV (X-ray scale)",
            EnergyScale::MegaElectronVolt => "MeV (nuclear scale)",
        }
    }
}

/// Analysis of uncertainty constraints across time scales
#[derive(Debug, Clone)]
pub struct UncertaintyAnalysis {
    pub temperature_k: f64,
    pub thermal_energy_j: f64,
    pub thermal_energy_ev: f64,
    pub hbar: f64,
    pub minimum_product: f64,
    pub constraints: Vec<UncertaintyConstraint>,
    pub recommended_scale: String,
}

/// Uncertainty constraint for a specific time scale
#[derive(Debug, Clone)]
pub struct UncertaintyConstraint {
    pub scale_name: String,
    pub time_s: f64,
    pub required_energy_j: f64,
    pub required_energy_ev: f64,
    pub thermal_energy_ratio: f64,
    pub is_above_thermal: bool,
    pub is_feasible: bool,
    pub energy_scale: EnergyScale,
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_uncertainty_principle_validation() {
        let validator = UncertaintyValidator::new();

        // Valid case: large energy and time
        let result = validator.validate_energy_time_product(1e-15, 1e-9).unwrap();
        assert!(result.is_valid);
        assert!(result.margin > 1.0);

        // Invalid case would be caught by error
        let min_product = validator.minimum_uncertainty_product();
        let small_energy = 1e-40;
        let small_time = 1e-40;
        assert!(small_energy * small_time < min_product);
    }

    #[test]
    fn test_nanosecond_consciousness_validation() {
        let validator = UncertaintyValidator::new();
        let result = validator.validate_nanosecond_consciousness();

        assert_eq!(result.time_s, 1e-9);
        // Nanosecond consciousness should be feasible
        assert!(result.energy_ev < 1000.0); // Under 1 keV
    }

    #[test]
    fn test_energy_scale_classification() {
        let validator = UncertaintyValidator::new();

        // Test different energy scales
        let atto_ev_energy = 1e-21; // 1 aeV in joules
        let ev_energy = constants::EV_TO_JOULES; // 1 eV
        let kev_energy = 1000.0 * constants::EV_TO_JOULES; // 1 keV

        assert_eq!(validator.classify_energy_scale(atto_ev_energy), EnergyScale::SubAttoElectronVolt);
        assert_eq!(validator.classify_energy_scale(ev_energy), EnergyScale::ElectronVolt);
        assert_eq!(validator.classify_energy_scale(kev_energy), EnergyScale::KiloElectronVolt);
    }

    #[test]
    fn test_time_scale_analysis() {
        let validator = UncertaintyValidator::new();
        let analysis = validator.analyze_time_scales();

        assert_eq!(analysis.constraints.len(), 6);
        assert_eq!(analysis.recommended_scale, "nanosecond");

        // Check that required energies increase as time decreases
        let mut prev_energy = 0.0;
        for constraint in analysis.constraints.iter().rev() {
            assert!(constraint.required_energy_j > prev_energy);
            prev_energy = constraint.required_energy_j;
        }
    }

    #[test]
    fn test_thermal_energy_calculations() {
        let validator = UncertaintyValidator::with_temperature(300.0); // 300K
        let thermal_j = validator.thermal_energy();
        let thermal_ev = thermal_j / constants::EV_TO_JOULES;

        // At 300K, thermal energy should be ~26 meV
        assert_relative_eq!(thermal_ev, 0.026, epsilon = 0.01);
    }

    #[test]
    fn test_energy_time_consistency() {
        let validator = UncertaintyValidator::new();

        let time = 1e-9;
        let required_energy = validator.calculate_required_energy(time);
        let max_time = validator.calculate_maximum_time(required_energy);

        // Should be consistent within numerical precision
        assert_relative_eq!(time, max_time, epsilon = 1e-10);
    }

    #[test]
    fn test_minimum_uncertainty_product() {
        let validator = UncertaintyValidator::new();
        let min_product = validator.minimum_uncertainty_product();

        // Should equal ℏ/2
        assert_relative_eq!(min_product, constants::PLANCK_HBAR / 2.0, epsilon = 1e-10);
    }
}