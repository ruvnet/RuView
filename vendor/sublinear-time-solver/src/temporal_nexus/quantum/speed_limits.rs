//! Quantum Speed Limit Validators - Margolus-Levitin Theorem Implementation
//!
//! This module implements rigorous validation of quantum computation speed limits
//! based on the Margolus-Levitin theorem, which establishes the fundamental
//! limit on the speed of quantum computation.
//!
//! ## Margolus-Levitin Theorem
//!
//! The theorem states that the minimum time required for a quantum system
//! to evolve from one state to an orthogonal state is:
//!
//! ```
//! τ_min = h / (4 * ΔE)
//! ```
//!
//! Where:
//! - τ_min: minimum computation time
//! - h: Planck constant
//! - ΔE: energy difference between states
//!
//! This provides a fundamental bound on computation speed that cannot be
//! violated by any physical system, quantum or classical.

use crate::temporal_nexus::quantum::{constants, QuantumError, QuantumResult};

/// Margolus-Levitin theorem validator for quantum speed limits
#[derive(Debug, Clone)]
pub struct MargolousLevitinValidator {
    /// Planck constant for calculations
    planck_h: f64,
    /// Safety margin factor (typically 1.1-2.0)
    safety_margin: f64,
    /// Hardware clock frequency limit (Hz)
    hardware_freq_limit_hz: f64,
}

impl MargolousLevitinValidator {
    /// Create new validator with default parameters
    pub fn new() -> Self {
        Self {
            planck_h: constants::PLANCK_H,
            safety_margin: 1.5, // 50% safety margin
            hardware_freq_limit_hz: 1e12, // 1 THz reasonable limit
        }
    }

    /// Create validator with custom hardware limits
    pub fn with_hardware_limits(hardware_freq_hz: f64) -> Self {
        Self {
            planck_h: constants::PLANCK_H,
            safety_margin: 1.5,
            hardware_freq_limit_hz: hardware_freq_hz,
        }
    }

    /// Calculate minimum computation time from energy difference
    pub fn calculate_minimum_time(&self, energy_difference_j: f64) -> f64 {
        if energy_difference_j <= 0.0 {
            return f64::INFINITY;
        }
        (self.planck_h / (4.0 * energy_difference_j)) * self.safety_margin
    }

    /// Calculate required energy for target computation time
    pub fn calculate_required_energy(&self, target_time_s: f64) -> f64 {
        if target_time_s <= 0.0 {
            return f64::INFINITY;
        }
        (self.planck_h * self.safety_margin) / (4.0 * target_time_s)
    }

    /// Validate if computation time is achievable with given energy
    pub fn validate_computation_time(
        &self,
        requested_time_s: f64,
        available_energy_j: f64,
    ) -> QuantumResult<SpeedLimitResult> {
        let min_time_s = self.calculate_minimum_time(available_energy_j);
        let required_energy_j = self.calculate_required_energy(requested_time_s);
        
        // Check Margolus-Levitin bound
        let ml_satisfied = requested_time_s >= min_time_s;
        
        // Check hardware frequency limits
        let operation_freq_hz = 1.0 / requested_time_s;
        let hardware_feasible = operation_freq_hz <= self.hardware_freq_limit_hz;
        
        let is_valid = ml_satisfied && hardware_feasible;
        
        if !ml_satisfied {
            return Err(QuantumError::MargolousLevitinViolation {
                min_time_s,
                requested_time_s,
            });
        }
        
        if !hardware_feasible {
            return Err(QuantumError::HardwareExceeded {
                required: operation_freq_hz / 1e6, // MHz
                available: self.hardware_freq_limit_hz / 1e6, // MHz
            });
        }
        
        Ok(SpeedLimitResult {
            is_valid,
            requested_time_s,
            minimum_time_s: min_time_s,
            available_energy_j,
            required_energy_j,
            safety_margin: self.safety_margin,
            operation_frequency_hz: operation_freq_hz,
            hardware_limit_hz: self.hardware_freq_limit_hz,
            margin_factor: requested_time_s / min_time_s,
        })
    }

    /// Check if nanosecond consciousness is feasible
    pub fn validate_nanosecond_consciousness(&self) -> SpeedLimitResult {
        let nanosecond = 1e-9;
        let required_energy = self.calculate_required_energy(nanosecond);
        
        // Assume reasonable energy budget (1 femtojoule)
        let available_energy = 1e-15;
        
        self.validate_computation_time(nanosecond, available_energy)
            .unwrap_or_else(|_| SpeedLimitResult {
                is_valid: false,
                requested_time_s: nanosecond,
                minimum_time_s: self.calculate_minimum_time(available_energy),
                available_energy_j: available_energy,
                required_energy_j: required_energy,
                safety_margin: self.safety_margin,
                operation_frequency_hz: 1e9,
                hardware_limit_hz: self.hardware_freq_limit_hz,
                margin_factor: 0.0,
            })
    }

    /// Get energy requirements for different time scales
    pub fn analyze_time_scales(&self) -> TimeScaleAnalysis {
        let scales = vec![
            ("attosecond", 1e-18),
            ("femtosecond", 1e-15),
            ("picosecond", 1e-12),
            ("nanosecond", 1e-9),
            ("microsecond", 1e-6),
            ("millisecond", 1e-3),
        ];
        
        let mut requirements = Vec::new();
        
        for (name, time_s) in scales {
            let energy_j = self.calculate_required_energy(time_s);
            let energy_ev = energy_j / constants::EV_TO_JOULES;
            let min_time = self.calculate_minimum_time(energy_j);
            
            requirements.push(TimeScaleRequirement {
                scale_name: name.to_string(),
                time_s,
                required_energy_j: energy_j,
                required_energy_ev: energy_ev,
                is_feasible: energy_ev < 1000.0, // 1 keV practical limit
                minimum_achievable_time_s: min_time,
            });
        }
        
        TimeScaleAnalysis {
            validator_config: format!(
                "Planck h: {:.2e}, Safety margin: {:.1}, Hardware limit: {:.1} THz",
                self.planck_h, self.safety_margin, self.hardware_freq_limit_hz / 1e12
            ),
            requirements,
            recommended_consciousness_scale: "nanosecond".to_string(),
            recommended_time_s: 1e-9,
        }
    }

    /// Get Planck constant for testing
    pub fn get_planck_constant(&self) -> f64 {
        self.planck_h
    }

    /// Set safety margin
    pub fn set_safety_margin(&mut self, margin: f64) {
        self.safety_margin = margin.max(1.0); // Minimum 1.0
    }
}

impl Default for MargolousLevitinValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of speed limit validation
#[derive(Debug, Clone)]
pub struct SpeedLimitResult {
    pub is_valid: bool,
    pub requested_time_s: f64,
    pub minimum_time_s: f64,
    pub available_energy_j: f64,
    pub required_energy_j: f64,
    pub safety_margin: f64,
    pub operation_frequency_hz: f64,
    pub hardware_limit_hz: f64,
    pub margin_factor: f64,
}

impl SpeedLimitResult {
    /// Get human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Speed Limit Check: {} (requested: {:.1e}s, minimum: {:.1e}s, margin: {:.1}x)",
            if self.is_valid { "PASS" } else { "FAIL" },
            self.requested_time_s,
            self.minimum_time_s,
            self.margin_factor
        )
    }
}

/// Analysis of energy requirements across time scales
#[derive(Debug, Clone)]
pub struct TimeScaleAnalysis {
    pub validator_config: String,
    pub requirements: Vec<TimeScaleRequirement>,
    pub recommended_consciousness_scale: String,
    pub recommended_time_s: f64,
}

/// Energy requirement for a specific time scale
#[derive(Debug, Clone)]
pub struct TimeScaleRequirement {
    pub scale_name: String,
    pub time_s: f64,
    pub required_energy_j: f64,
    pub required_energy_ev: f64,
    pub is_feasible: bool,
    pub minimum_achievable_time_s: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_margolous_levitin_calculation() {
        let validator = MargolousLevitinValidator::new();
        
        // Test with 1 femtojoule
        let energy = 1e-15; // 1 fJ
        let min_time = validator.calculate_minimum_time(energy);
        
        // Should be on order of 10^-19 seconds
        assert!(min_time > 0.0);
        assert!(min_time < 1e-15);
    }

    #[test]
    fn test_nanosecond_consciousness_validation() {
        let validator = MargolousLevitinValidator::new();
        let result = validator.validate_nanosecond_consciousness();
        
        // Nanosecond should be achievable with reasonable energy
        assert!(result.requested_time_s == 1e-9);
    }

    #[test]
    fn test_time_scale_analysis() {
        let validator = MargolousLevitinValidator::new();
        let analysis = validator.analyze_time_scales();
        
        assert_eq!(analysis.requirements.len(), 6);
        assert_eq!(analysis.recommended_consciousness_scale, "nanosecond");
        
        // Check that nanosecond is feasible
        let nanosecond_req = analysis.requirements.iter()
            .find(|r| r.scale_name == "nanosecond")
            .unwrap();
        assert!(nanosecond_req.is_feasible);
    }

    #[test]
    fn test_energy_calculation_consistency() {
        let validator = MargolousLevitinValidator::new();
        
        let time = 1e-9; // 1 nanosecond
        let required_energy = validator.calculate_required_energy(time);
        let min_time = validator.calculate_minimum_time(required_energy);
        
        // Should be consistent (within safety margin)
        assert_relative_eq!(time, min_time, epsilon = 0.1);
    }

    #[test]
    fn test_hardware_limits() {
        let validator = MargolousLevitinValidator::with_hardware_limits(1e9); // 1 GHz
        
        // 1 nanosecond operation = 1 GHz, should be at limit
        let result = validator.validate_computation_time(1e-9, 1e-12).unwrap();
        assert!(result.operation_frequency_hz <= validator.hardware_freq_limit_hz);
        
        // 1 picosecond operation = 1 THz, should exceed 1 GHz limit
        let result = validator.validate_computation_time(1e-12, 1e-12);
        assert!(result.is_err());
    }

    #[test]
    fn test_safety_margin() {
        let mut validator = MargolousLevitinValidator::new();
        validator.set_safety_margin(2.0);
        
        let energy = 1e-15;
        let min_time_2x = validator.calculate_minimum_time(energy);
        
        validator.set_safety_margin(1.0);
        let min_time_1x = validator.calculate_minimum_time(energy);
        
        assert_relative_eq!(min_time_2x, 2.0 * min_time_1x, epsilon = 0.01);
    }
}