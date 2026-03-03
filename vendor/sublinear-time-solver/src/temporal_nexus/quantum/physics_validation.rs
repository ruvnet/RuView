//! Physics Constants and Computational Bounds Validation
//!
//! This module provides comprehensive validation of physical constants
//! and computational bounds used throughout the quantum validation framework.
//! It ensures that all constants are within accepted ranges and that
//! computational algorithms are numerically stable.

use super::constants;
use std::f64::consts::PI;

/// Physics constants validation report
#[derive(Debug, Clone)]
pub struct PhysicsValidationReport {
    pub constants_valid: bool,
    pub computational_bounds_valid: bool,
    pub numerical_stability_valid: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Comprehensive physics validation suite
pub struct PhysicsValidator;

impl PhysicsValidator {
    /// Validate all physics constants against known values
    pub fn validate_constants() -> PhysicsValidationReport {
        let mut report = PhysicsValidationReport {
            constants_valid: true,
            computational_bounds_valid: true,
            numerical_stability_valid: true,
            issues: Vec::new(),
            warnings: Vec::new(),
            recommendations: Vec::new(),
        };

        // Validate fundamental constants
        Self::validate_planck_constants(&mut report);
        Self::validate_boltzmann_constant(&mut report);
        Self::validate_speed_of_light(&mut report);
        Self::validate_electron_volt_conversion(&mut report);
        Self::validate_derived_constants(&mut report);
        Self::validate_consciousness_constants(&mut report);
        Self::validate_computational_bounds(&mut report);
        Self::validate_numerical_stability(&mut report);

        report
    }

    /// Validate Planck constants
    fn validate_planck_constants(report: &mut PhysicsValidationReport) {
        // CODATA 2018 values
        let expected_h = 6.626_070_15e-34;
        let expected_hbar = expected_h / (2.0 * PI);

        // Validate Planck constant
        if (constants::PLANCK_H - expected_h).abs() > 1e-42 {
            report.issues.push(format!(
                "Planck constant deviation: expected {:.10e}, got {:.10e}",
                expected_h, constants::PLANCK_H
            ));
            report.constants_valid = false;
        }

        // Validate reduced Planck constant
        if (constants::PLANCK_HBAR - expected_hbar).abs() > 1e-42 {
            report.issues.push(format!(
                "Reduced Planck constant deviation: expected {:.10e}, got {:.10e}",
                expected_hbar, constants::PLANCK_HBAR
            ));
            report.constants_valid = false;
        }

        // Validate relationship
        let calculated_hbar = constants::PLANCK_H / (2.0 * PI);
        if (constants::PLANCK_HBAR - calculated_hbar).abs() > 1e-50 {
            report.issues.push(format!(
                "Planck constant relationship violated: ℏ ≠ h/(2π), error = {:.2e}",
                (constants::PLANCK_HBAR - calculated_hbar).abs()
            ));
            report.constants_valid = false;
        }

        // Validate positivity
        if constants::PLANCK_H <= 0.0 || constants::PLANCK_HBAR <= 0.0 {
            report.issues.push("Planck constants must be positive".to_string());
            report.constants_valid = false;
        }
    }

    /// Validate Boltzmann constant
    fn validate_boltzmann_constant(report: &mut PhysicsValidationReport) {
        let expected_kb = 1.380_649e-23; // CODATA 2018

        if (constants::BOLTZMANN_K - expected_kb).abs() > 1e-31 {
            report.issues.push(format!(
                "Boltzmann constant deviation: expected {:.10e}, got {:.10e}",
                expected_kb, constants::BOLTZMANN_K
            ));
            report.constants_valid = false;
        }

        if constants::BOLTZMANN_K <= 0.0 {
            report.issues.push("Boltzmann constant must be positive".to_string());
            report.constants_valid = false;
        }
    }

    /// Validate speed of light
    fn validate_speed_of_light(report: &mut PhysicsValidationReport) {
        let expected_c = 299_792_458.0; // Exact by definition

        if (constants::SPEED_OF_LIGHT - expected_c).abs() > 1e-6 {
            report.issues.push(format!(
                "Speed of light deviation: expected {:.1}, got {:.1}",
                expected_c, constants::SPEED_OF_LIGHT
            ));
            report.constants_valid = false;
        }

        if constants::SPEED_OF_LIGHT <= 0.0 {
            report.issues.push("Speed of light must be positive".to_string());
            report.constants_valid = false;
        }
    }

    /// Validate electron volt conversion
    fn validate_electron_volt_conversion(report: &mut PhysicsValidationReport) {
        let expected_ev_to_j = 1.602_176_634e-19; // CODATA 2018

        if (constants::EV_TO_JOULES - expected_ev_to_j).abs() > 1e-27 {
            report.issues.push(format!(
                "eV to Joules conversion deviation: expected {:.10e}, got {:.10e}",
                expected_ev_to_j, constants::EV_TO_JOULES
            ));
            report.constants_valid = false;
        }

        if constants::EV_TO_JOULES <= 0.0 {
            report.issues.push("eV to Joules conversion must be positive".to_string());
            report.constants_valid = false;
        }
    }

    /// Validate derived constants
    fn validate_derived_constants(report: &mut PhysicsValidationReport) {
        // Validate room temperature (should be reasonable)
        if constants::ROOM_TEMPERATURE_K < 250.0 || constants::ROOM_TEMPERATURE_K > 350.0 {
            report.warnings.push(format!(
                "Room temperature seems unusual: {:.1} K",
                constants::ROOM_TEMPERATURE_K
            ));
        }

        // Validate thermal energy at room temperature
        let thermal_energy = constants::BOLTZMANN_K * constants::ROOM_TEMPERATURE_K;
        let thermal_energy_ev = thermal_energy / constants::EV_TO_JOULES;

        if thermal_energy_ev < 0.020 || thermal_energy_ev > 0.030 {
            report.warnings.push(format!(
                "Room temperature thermal energy unusual: {:.3} eV (expected ~0.025 eV)",
                thermal_energy_ev
            ));
        }

        // Validate minimum uncertainty product
        let min_uncertainty = constants::PLANCK_HBAR / 2.0;
        if min_uncertainty <= 0.0 {
            report.issues.push("Minimum uncertainty product must be positive".to_string());
            report.constants_valid = false;
        }
    }

    /// Validate consciousness-specific constants
    fn validate_consciousness_constants(report: &mut PhysicsValidationReport) {
        // Validate attosecond energy requirement
        if constants::ATTOSECOND_ENERGY_KEV <= 0.0 {
            report.issues.push("Attosecond energy requirement must be positive".to_string());
            report.constants_valid = false;
        }

        if constants::ATTOSECOND_ENERGY_KEV < 0.5 || constants::ATTOSECOND_ENERGY_KEV > 5.0 {
            report.warnings.push(format!(
                "Attosecond energy requirement seems unusual: {:.2} keV",
                constants::ATTOSECOND_ENERGY_KEV
            ));
        }

        // Validate consciousness scale
        if constants::CONSCIOUSNESS_SCALE_NS != 1e-9 {
            report.issues.push(format!(
                "Consciousness scale incorrect: expected 1e-9, got {:.2e}",
                constants::CONSCIOUSNESS_SCALE_NS
            ));
            report.constants_valid = false;
        }

        // Validate that attosecond operation requires substantial energy
        let attosecond_energy_j = constants::ATTOSECOND_ENERGY_KEV * 1000.0 * constants::EV_TO_JOULES;
        let thermal_energy_j = constants::BOLTZMANN_K * constants::ROOM_TEMPERATURE_K;
        let energy_ratio = attosecond_energy_j / thermal_energy_j;

        if energy_ratio < 1000.0 {
            report.warnings.push(format!(
                "Attosecond energy only {:.0}x thermal energy (expected >1000x)",
                energy_ratio
            ));
        }
    }

    /// Validate computational bounds
    fn validate_computational_bounds(report: &mut PhysicsValidationReport) {
        // Test Margolus-Levitin bound calculation
        let test_energy = 1e-15; // 1 fJ
        let min_time = constants::PLANCK_H / (4.0 * test_energy);

        if min_time <= 0.0 || !min_time.is_finite() {
            report.issues.push("Margolus-Levitin calculation produces invalid result".to_string());
            report.computational_bounds_valid = false;
        }

        if min_time > 1e-15 {
            report.warnings.push(format!(
                "Margolus-Levitin time seems large for 1 fJ: {:.2e} s",
                min_time
            ));
        }

        // Test uncertainty principle calculation
        let uncertainty_product = test_energy * min_time;
        let min_uncertainty = constants::PLANCK_HBAR / 2.0;

        if uncertainty_product < min_uncertainty {
            report.issues.push(format!(
                "Uncertainty principle violation in test: ΔE·Δt = {:.2e} < ℏ/2 = {:.2e}",
                uncertainty_product, min_uncertainty
            ));
            report.computational_bounds_valid = false;
        }

        // Test energy scale boundaries
        let max_reasonable_energy = 1e-12; // 1 pJ
        let min_reasonable_energy = 1e-21; // 1 zJ

        if constants::PLANCK_H / (4.0 * max_reasonable_energy) > 1e-18 {
            report.warnings.push("Maximum energy bound may be too restrictive".to_string());
        }

        if constants::PLANCK_H / (4.0 * min_reasonable_energy) < 1e-15 {
            report.warnings.push("Minimum energy bound may be too permissive".to_string());
        }
    }

    /// Validate numerical stability
    fn validate_numerical_stability(report: &mut PhysicsValidationReport) {
        // Test precision at extreme scales
        let very_small = 1e-30;
        let very_large = 1e30;

        // Test that calculations don't overflow/underflow
        let product: f64 = very_small * very_large;
        if !product.is_finite() || product == 0.0 {
            report.issues.push("Numerical instability in extreme scale calculations".to_string());
            report.numerical_stability_valid = false;
        }

        // Test floating point precision near physical constants
        let h_plus_epsilon = constants::PLANCK_H + 1e-50;
        if h_plus_epsilon == constants::PLANCK_H {
            report.warnings.push("Limited floating point precision near Planck constant".to_string());
        }

        // Test reciprocal calculations
        let energy = 1e-15;
        let time1 = constants::PLANCK_H / (4.0 * energy);
        let energy_back = constants::PLANCK_H / (4.0 * time1);
        let relative_error = (energy - energy_back).abs() / energy;

        if relative_error > 1e-14 {
            report.warnings.push(format!(
                "Numerical precision loss in reciprocal calculations: {:.2e} relative error",
                relative_error
            ));
        }

        // Test trigonometric precision in hbar calculation
        let hbar_calculated = constants::PLANCK_H / (2.0 * PI);
        let relative_error_hbar = (constants::PLANCK_HBAR - hbar_calculated).abs() / constants::PLANCK_HBAR;

        if relative_error_hbar > 1e-15 {
            report.warnings.push(format!(
                "Numerical precision loss in ℏ calculation: {:.2e} relative error",
                relative_error_hbar
            ));
        }
    }

    /// Generate physics validation summary
    pub fn generate_summary(report: &PhysicsValidationReport) -> String {
        let mut summary = String::new();

        summary.push_str("🔬 Physics Constants & Computational Bounds Validation\\n");
        summary.push_str("=====================================================\\n\\n");

        // Overall status
        if report.constants_valid && report.computational_bounds_valid && report.numerical_stability_valid {
            summary.push_str("✅ Overall Status: PASS\\n");
        } else {
            summary.push_str("❌ Overall Status: FAIL\\n");
        }

        summary.push_str(&format!("   Constants Valid: {}\\n",
                                 if report.constants_valid { "✅" } else { "❌" }));
        summary.push_str(&format!("   Computational Bounds Valid: {}\\n",
                                 if report.computational_bounds_valid { "✅" } else { "❌" }));
        summary.push_str(&format!("   Numerical Stability Valid: {}\\n\\n",
                                 if report.numerical_stability_valid { "✅" } else { "❌" }));

        // Issues
        if !report.issues.is_empty() {
            summary.push_str("🚨 Issues:\\n");
            for issue in &report.issues {
                summary.push_str(&format!("   • {}\\n", issue));
            }
            summary.push_str("\\n");
        }

        // Warnings
        if !report.warnings.is_empty() {
            summary.push_str("⚠️  Warnings:\\n");
            for warning in &report.warnings {
                summary.push_str(&format!("   • {}\\n", warning));
            }
            summary.push_str("\\n");
        }

        // Recommendations
        if !report.recommendations.is_empty() {
            summary.push_str("💡 Recommendations:\\n");
            for rec in &report.recommendations {
                summary.push_str(&format!("   • {}\\n", rec));
            }
            summary.push_str("\\n");
        }

        // Physical constant values
        summary.push_str("📏 Physical Constants:\\n");
        summary.push_str(&format!("   Planck constant (h): {:.10e} J⋅s\\n", constants::PLANCK_H));
        summary.push_str(&format!("   Reduced Planck (ℏ): {:.10e} J⋅s\\n", constants::PLANCK_HBAR));
        summary.push_str(&format!("   Boltzmann (kB): {:.10e} J/K\\n", constants::BOLTZMANN_K));
        summary.push_str(&format!("   Speed of light (c): {:.0} m/s\\n", constants::SPEED_OF_LIGHT));
        summary.push_str(&format!("   eV to Joules: {:.10e}\\n", constants::EV_TO_JOULES));
        summary.push_str(&format!("   Room temperature: {:.1} K\\n", constants::ROOM_TEMPERATURE_K));
        summary.push_str(&format!("   Attosecond energy: {:.2} keV\\n", constants::ATTOSECOND_ENERGY_KEV));
        summary.push_str(&format!("   Consciousness scale: {:.0e} s\\n", constants::CONSCIOUSNESS_SCALE_NS));

        // Computational bounds
        summary.push_str("\\n🧮 Computational Bounds:\\n");
        let test_energy = 1e-15;
        let min_time = constants::PLANCK_H / (4.0 * test_energy);
        let min_uncertainty = constants::PLANCK_HBAR / 2.0;
        let thermal_energy = constants::BOLTZMANN_K * constants::ROOM_TEMPERATURE_K;

        summary.push_str(&format!("   Min time (1 fJ): {:.2e} s\\n", min_time));
        summary.push_str(&format!("   Min uncertainty: {:.2e} J⋅s\\n", min_uncertainty));
        summary.push_str(&format!("   Thermal energy: {:.2e} J ({:.1} meV)\\n",
                                 thermal_energy, thermal_energy / constants::EV_TO_JOULES * 1000.0));

        summary
    }

    /// Quick validation check for runtime use
    pub fn quick_check() -> bool {
        // Essential checks that must pass
        constants::PLANCK_H > 0.0 &&
        constants::PLANCK_HBAR > 0.0 &&
        constants::BOLTZMANN_K > 0.0 &&
        constants::SPEED_OF_LIGHT > 0.0 &&
        constants::EV_TO_JOULES > 0.0 &&
        (constants::PLANCK_HBAR - constants::PLANCK_H / (2.0 * PI)).abs() < 1e-40 &&
        constants::CONSCIOUSNESS_SCALE_NS == 1e-9
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_validation() {
        let report = PhysicsValidator::validate_constants();

        // Print full report for debugging
        if !report.constants_valid || !report.computational_bounds_valid || !report.numerical_stability_valid {
            println!("{}", PhysicsValidator::generate_summary(&report));
        }

        assert!(report.constants_valid, "Physics constants must be valid");
        assert!(report.computational_bounds_valid, "Computational bounds must be valid");
        assert!(report.numerical_stability_valid, "Numerical calculations must be stable");
    }

    #[test]
    fn test_quick_check() {
        assert!(PhysicsValidator::quick_check(), "Quick physics check must pass");
    }

    #[test]
    fn test_constant_relationships() {
        // Test fundamental relationships
        let calculated_hbar = constants::PLANCK_H / (2.0 * PI);
        assert!((constants::PLANCK_HBAR - calculated_hbar).abs() < 1e-40);

        // Test unit conversions
        let one_ev_in_joules = constants::EV_TO_JOULES;
        assert!(one_ev_in_joules > 1e-20 && one_ev_in_joules < 1e-18);

        // Test energy scales
        let thermal_energy = constants::BOLTZMANN_K * constants::ROOM_TEMPERATURE_K;
        let thermal_ev = thermal_energy / constants::EV_TO_JOULES;
        assert!(thermal_ev > 0.02 && thermal_ev < 0.03); // ~25 meV

        // Test attosecond energy
        let attosecond_energy_j = constants::ATTOSECOND_ENERGY_KEV * 1000.0 * constants::EV_TO_JOULES;
        assert!(attosecond_energy_j > thermal_energy * 1000.0);
    }

    #[test]
    fn test_numerical_precision() {
        // Test that we can distinguish small differences
        let base = constants::PLANCK_H;
        let epsilon = base * 1e-15;
        assert!(base + epsilon > base);

        // Test reciprocal precision
        let energy = 1e-15;
        let time = constants::PLANCK_H / (4.0 * energy);
        let energy_back = constants::PLANCK_H / (4.0 * time);
        let relative_error = (energy - energy_back).abs() / energy;
        assert!(relative_error < 1e-12);
    }

    #[test]
    fn test_physical_reasonableness() {
        // Test that calculated times are physically reasonable
        let femtojoule = 1e-15;
        let min_time = constants::PLANCK_H / (4.0 * femtojoule);
        assert!(min_time > 1e-25); // Much larger than Planck time
        assert!(min_time < 1e-15); // Much smaller than femtosecond

        // Test that uncertainty product is reasonable
        let uncertainty_product = femtojoule * min_time;
        let min_uncertainty = constants::PLANCK_HBAR / 2.0;
        assert!(uncertainty_product >= min_uncertainty);

        // Test consciousness time scale
        assert_eq!(constants::CONSCIOUSNESS_SCALE_NS, 1e-9);
        let ns_energy = constants::PLANCK_H / (4.0 * constants::CONSCIOUSNESS_SCALE_NS);
        let ns_energy_ev = ns_energy / constants::EV_TO_JOULES;
        assert!(ns_energy_ev < 1.0); // Should be sub-eV
    }
}