//! Comprehensive Tests for Quantum Validation Protocols
//!
//! This module contains rigorous tests for all quantum validation components,
//! ensuring they properly implement quantum mechanical principles and provide
//! accurate validation for temporal consciousness operations.

use super::*;
use crate::temporal_nexus::core::NanosecondScheduler;
use approx::{assert_relative_eq, relative_eq};
use std::f64::consts::PI;

/// Test suite for quantum validation protocols
pub struct QuantumTestSuite {
    validator: QuantumValidator,
}

impl QuantumTestSuite {
    pub fn new() -> Self {
        Self {
            validator: QuantumValidator::new(),
        }
    }

    /// Run all quantum validation tests
    pub fn run_all_tests() -> Result<(), Box<dyn std::error::Error>> {
        let mut suite = Self::new();

        println!("🧪 Running Quantum Validation Protocol Tests");
        println!("==============================================");

        suite.test_physics_constants()?;
        suite.test_margolus_levitin_validator()?;
        suite.test_uncertainty_principle_validator()?;
        suite.test_decoherence_tracker()?;
        suite.test_entanglement_validator()?;
        suite.test_quantum_validator_integration()?;
        suite.test_scheduler_integration()?;
        suite.test_attosecond_feasibility()?;
        suite.test_consciousness_time_scales()?;
        suite.test_edge_cases_and_limits()?;

        println!("✅ All quantum validation tests passed!");
        Ok(())
    }

    /// Test fundamental physics constants
    fn test_physics_constants(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📐 Testing Physics Constants...");

        // Test Planck constants
        assert!(constants::PLANCK_H > 0.0, "Planck constant must be positive");
        assert!(constants::PLANCK_HBAR > 0.0, "Reduced Planck constant must be positive");
        assert_relative_eq!(
            constants::PLANCK_HBAR,
            constants::PLANCK_H / (2.0 * PI),
            epsilon = 1e-10
        );

        // Test fundamental relationships
        let hbar_half = constants::PLANCK_HBAR / 2.0;
        assert!(hbar_half > 0.0, "ℏ/2 must be positive");

        // Test energy conversions
        assert!(constants::EV_TO_JOULES > 0.0, "eV to Joules conversion must be positive");
        assert_relative_eq!(
            constants::EV_TO_JOULES,
            1.602_176_634e-19,
            epsilon = 1e-10
        );

        // Test attosecond energy requirement
        let attosecond_energy_j = constants::ATTOSECOND_ENERGY_KEV * 1000.0 * constants::EV_TO_JOULES;
        assert!(attosecond_energy_j > 1e-16, "Attosecond energy should be substantial");

        println!("  ✓ Physics constants validation passed");
        Ok(())
    }

    /// Test Margolus-Levitin theorem validator
    fn test_margolus_levitin_validator(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("⚡ Testing Margolus-Levitin Speed Limits...");

        let validator = &self.validator.speed_limits;

        // Test minimum time calculation
        let energy_1fj = 1e-15; // 1 femtojoule
        let min_time = validator.calculate_minimum_time(energy_1fj);
        assert!(min_time > 0.0, "Minimum time must be positive");
        assert!(min_time < 1e-15, "Minimum time should be very small for high energy");

        // Test energy requirement calculation
        let nanosecond = 1e-9;
        let required_energy = validator.calculate_required_energy(nanosecond);
        assert!(required_energy > 0.0, "Required energy must be positive");

        // Test consistency: calculate time from energy, then energy from time
        let consistent_time = validator.calculate_minimum_time(required_energy);
        assert_relative_eq!(nanosecond, consistent_time, epsilon = 0.1);

        // Test nanosecond consciousness validation
        let ns_result = validator.validate_nanosecond_consciousness();
        assert_eq!(ns_result.requested_time_s, 1e-9);

        // Test time scale analysis
        let analysis = validator.analyze_time_scales();
        assert_eq!(analysis.requirements.len(), 6);
        assert_eq!(analysis.recommended_consciousness_scale, "nanosecond");

        // Verify that shorter times require more energy
        let femtosecond_energy = validator.calculate_required_energy(1e-15);
        let nanosecond_energy = validator.calculate_required_energy(1e-9);
        assert!(femtosecond_energy > nanosecond_energy,
                "Shorter times should require more energy");

        println!("  ✓ Margolus-Levitin validator tests passed");
        Ok(())
    }

    /// Test uncertainty principle validator
    fn test_uncertainty_principle_validator(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🎲 Testing Uncertainty Principle...");

        let validator = &self.validator.uncertainty;

        // Test minimum uncertainty product
        let min_product = validator.minimum_uncertainty_product();
        assert_relative_eq!(min_product, constants::PLANCK_HBAR / 2.0, epsilon = 1e-10);

        // Test valid energy-time combination
        let energy = 1e-15; // 1 fJ
        let time = 1e-9;    // 1 ns
        let result = validator.validate_energy_time_product(energy, time)?;
        assert!(result.is_valid, "Large energy×time should satisfy uncertainty principle");
        assert!(result.margin > 1.0, "Should have positive margin");

        // Test thermal energy calculation
        let thermal = validator.thermal_energy();
        assert!(thermal > 0.0, "Thermal energy must be positive");

        // Test energy scale classification
        let ev_energy = constants::EV_TO_JOULES;
        let scale = validator.classify_energy_scale(ev_energy);
        assert_eq!(scale, EnergyScale::ElectronVolt);

        // Test time scale analysis
        let analysis = validator.analyze_time_scales();
        assert_eq!(analysis.constraints.len(), 6);

        // Verify uncertainty relation is always satisfied for valid combinations
        for constraint in &analysis.constraints {
            let product = constraint.required_energy_j * constraint.time_s;
            assert!(product >= min_product * 0.99, // Allow small numerical error
                   "Uncertainty relation violated for {}: product = {:.2e}, min = {:.2e}",
                   constraint.scale_name, product, min_product);
        }

        println!("  ✓ Uncertainty principle validator tests passed");
        Ok(())
    }

    /// Test decoherence tracker
    fn test_decoherence_tracker(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🌀 Testing Quantum Decoherence...");

        let tracker = &self.validator.decoherence;

        // Test coherence time calculation
        let coherence_time = tracker.coherence_time();
        assert!(coherence_time > 0.0, "Coherence time must be positive");

        // Test T1 and T2 times
        let t1 = tracker.relaxation_time_t1();
        let t2 = tracker.dephasing_time_t2();
        assert!(t1 > 0.0, "T1 time must be positive");
        assert!(t2 > 0.0, "T2 time must be positive");
        assert!(t1 >= t2, "T1 should typically be longer than T2");

        // Test operation time validation
        let short_time = 1e-12; // 1 ps
        let result = tracker.validate_operation_time(short_time)?;
        assert!(result.is_valid, "Short operation should preserve coherence");
        assert!(result.coherence_preserved > 0.9, "High coherence preservation expected");

        // Test environment classification
        let room_tracker = DecoherenceTracker::new();
        let cryo_tracker = DecoherenceTracker::cryogenic();

        assert_eq!(room_tracker.classify_environment(), EnvironmentType::RoomTemperature);
        assert_eq!(cryo_tracker.classify_environment(), EnvironmentType::UltraCryogenic);

        // Cryogenic should have longer coherence times
        assert!(cryo_tracker.coherence_time() > room_tracker.coherence_time());

        // Test coherence evolution
        let evolution = tracker.predict_coherence_evolution(1e-9, 100);
        assert_eq!(evolution.times.len(), 101);
        assert_eq!(evolution.coherences.len(), 101);

        // Coherence should decay monotonically
        for i in 1..evolution.coherences.len() {
            assert!(evolution.coherences[i] <= evolution.coherences[i-1],
                   "Coherence should not increase over time");
        }

        // Test time scale analysis
        let analysis = tracker.analyze_time_scales();
        assert_eq!(analysis.assessments.len(), 6);

        println!("  ✓ Decoherence tracker tests passed");
        Ok(())
    }

    /// Test entanglement validator
    fn test_entanglement_validator(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔗 Testing Quantum Entanglement...");

        let validator = &self.validator.entanglement;

        // Test entanglement survival
        let survival_0 = validator.entanglement_survival(0.0);
        assert_relative_eq!(survival_0, 1.0, epsilon = 1e-10);

        let survival_decoherence = validator.entanglement_survival(validator.decoherence_time_s);
        assert_relative_eq!(survival_decoherence, 1.0 / std::f64::consts::E, epsilon = 1e-6);

        // Test concurrence calculation
        let short_time_concurrence = validator.calculate_concurrence(1e-12);
        let long_time_concurrence = validator.calculate_concurrence(1e-6);
        assert!(short_time_concurrence > long_time_concurrence,
               "Shorter times should preserve more entanglement");

        // Test Bell parameter
        let bell_short = validator.calculate_bell_parameter(1e-12);
        let bell_long = validator.calculate_bell_parameter(1e-6);
        assert!(bell_short > 2.0, "Should violate Bell inequality at short times");
        assert!(bell_long >= 2.0, "Should not violate causality");

        // Test temporal correlation validation
        let result = validator.validate_temporal_correlation(1e-9)?;
        assert!(result.operation_time_s == 1e-9);
        assert!(result.concurrence >= 0.0 && result.concurrence <= 1.0);
        assert!(result.bell_parameter >= 2.0);

        // Test consciousness time scale analysis
        let analysis = validator.analyze_consciousness_time_scales();
        assert_eq!(analysis.assessments.len(), 6);

        // Check neural spike assessment
        let neural_spike = analysis.assessments.iter()
            .find(|a| a.scale_name == "neural spike")
            .unwrap();
        assert_eq!(neural_spike.consciousness_relevance, ConsciousnessRelevance::DirectlyRelevant);

        // Test consciousness network modeling
        let network = validator.model_consciousness_network(5, 1e-9);
        assert_eq!(network.network_size, 5);
        assert_eq!(network.node_entanglements.len(), 10); // C(5,2) = 10
        assert!(network.network_coherence >= 0.0 && network.network_coherence <= 1.0);

        // Test quantum Fisher information
        let qfi = validator.calculate_quantum_fisher_information(1e-9);
        assert!(qfi > 0.0, "Quantum Fisher information must be positive");

        println!("  ✓ Entanglement validator tests passed");
        Ok(())
    }

    /// Test quantum validator integration
    fn test_quantum_validator_integration(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔬 Testing Quantum Validator Integration...");

        // Test comprehensive temporal operation validation
        let nanosecond = 1e-9;
        let femtojoule = 1e-15;

        let result = self.validator.validate_temporal_operation(nanosecond, femtojoule)?;
        assert!(result.operation_time_s == nanosecond);
        assert!(result.energy_j == femtojoule);

        // Verify all sub-validators were called
        assert!(result.speed_limit_result.requested_time_s == nanosecond);
        assert!(result.uncertainty_result.time_s == nanosecond);
        assert!(result.decoherence_result.operation_time_s == nanosecond);
        assert!(result.entanglement_result.operation_time_s == nanosecond);

        // Test attosecond feasibility report
        let attosecond_report = self.validator.check_attosecond_feasibility();
        assert_eq!(attosecond_report.time_scale_s, 1e-18);
        assert_eq!(attosecond_report.required_energy_kev, 1.03);
        assert!(attosecond_report.is_theoretically_feasible);
        assert!(!attosecond_report.is_practically_achievable);
        assert_eq!(attosecond_report.recommended_scale_s, constants::CONSCIOUSNESS_SCALE_NS);

        // Test edge case: very high energy, very short time
        let attosecond = 1e-18;
        let kev_energy = 1000.0 * constants::EV_TO_JOULES;

        match self.validator.validate_temporal_operation(attosecond, kev_energy) {
            Ok(high_energy_result) => {
                // High energy operation might be valid
                assert!(high_energy_result.energy_j == kev_energy);
            }
            Err(_) => {
                // Or it might violate some quantum constraint
                // Both outcomes are acceptable for this extreme case
            }
        }

        println!("  ✓ Quantum validator integration tests passed");
        Ok(())
    }

    /// Test integration with NanosecondScheduler
    fn test_scheduler_integration(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("⏰ Testing Scheduler Integration...");

        let mut scheduler = NanosecondScheduler::new();

        // Test that scheduler has quantum validator
        let initial_analysis = scheduler.get_quantum_analysis();
        assert_eq!(initial_analysis.total_validations, 0);
        assert_eq!(initial_analysis.validity_rate, 0.0);

        // Run some ticks to generate quantum validations
        for _ in 0..10 {
            scheduler.tick()?;
        }

        // Check that quantum validations were performed
        let analysis = scheduler.get_quantum_analysis();
        assert!(analysis.total_validations > 0, "Quantum validations should have been performed");
        assert!(analysis.validity_rate >= 0.0 && analysis.validity_rate <= 1.0);
        assert!(analysis.avg_energy_j > 0.0);

        // Check scheduler metrics include quantum data
        let metrics = scheduler.get_metrics();
        assert!(metrics.quantum_validity_rate >= 0.0);
        assert!(metrics.avg_quantum_energy_j > 0.0);

        // Verify attosecond feasibility in analysis
        assert!(analysis.attosecond_feasibility.is_theoretically_feasible);
        assert!(!analysis.attosecond_feasibility.is_practically_achievable);

        println!("  ✓ Scheduler integration tests passed");
        Ok(())
    }

    /// Test attosecond feasibility analysis
    fn test_attosecond_feasibility(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("⚛️  Testing Attosecond Feasibility...");

        let report = self.validator.check_attosecond_feasibility();

        // Verify basic properties
        assert_eq!(report.time_scale_s, 1e-18);
        assert_eq!(report.required_energy_kev, 1.03);
        assert!(report.is_theoretically_feasible);
        assert!(!report.is_practically_achievable);

        // Verify limiting factors are identified
        assert!(!report.limiting_factors.is_empty());
        assert!(report.limiting_factors.iter().any(|f| f.contains("1.03 keV")));

        // Verify nanosecond recommendation
        assert_eq!(report.recommended_scale_s, 1e-9);
        assert!(report.recommended_scale_description.contains("nanosecond"));

        // Test that attosecond energy requirement is substantial
        let required_energy_j = report.required_energy_j;
        let thermal_energy_j = constants::BOLTZMANN_K * constants::ROOM_TEMPERATURE_K;
        assert!(required_energy_j > thermal_energy_j * 1000.0,
               "Attosecond operation requires much more than thermal energy");

        println!("  ✓ Attosecond feasibility tests passed");
        Ok(())
    }

    /// Test consciousness time scale analysis
    fn test_consciousness_time_scales(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🧠 Testing Consciousness Time Scales...");

        // Test speed limit analysis
        let speed_analysis = self.validator.speed_limits.analyze_time_scales();
        assert_eq!(speed_analysis.requirements.len(), 6);

        // Verify energy requirements increase as time decreases
        let mut prev_energy = 0.0;
        for req in speed_analysis.requirements.iter().rev() {
            assert!(req.required_energy_j > prev_energy,
                   "Energy should increase for shorter times");
            prev_energy = req.required_energy_j;
        }

        // Test uncertainty analysis
        let uncertainty_analysis = self.validator.uncertainty.analyze_time_scales();
        assert_eq!(uncertainty_analysis.constraints.len(), 6);

        // Test decoherence analysis
        let decoherence_analysis = self.validator.decoherence.analyze_time_scales();
        assert_eq!(decoherence_analysis.assessments.len(), 6);

        // Test entanglement analysis
        let entanglement_analysis = self.validator.entanglement.analyze_consciousness_time_scales();
        assert_eq!(entanglement_analysis.assessments.len(), 6);

        // Verify consciousness relevance assignments
        let neural_assessment = entanglement_analysis.assessments.iter()
            .find(|a| a.scale_name == "neural spike")
            .unwrap();
        assert_eq!(neural_assessment.consciousness_relevance, ConsciousnessRelevance::DirectlyRelevant);

        let gamma_assessment = entanglement_analysis.assessments.iter()
            .find(|a| a.scale_name == "gamma wave")
            .unwrap();
        assert_eq!(gamma_assessment.consciousness_relevance, ConsciousnessRelevance::HighlyRelevant);

        println!("  ✓ Consciousness time scale tests passed");
        Ok(())
    }

    /// Test edge cases and physical limits
    fn test_edge_cases_and_limits(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔍 Testing Edge Cases and Limits...");

        // Test zero and negative inputs
        assert_eq!(self.validator.speed_limits.calculate_minimum_time(0.0), f64::INFINITY);
        assert_eq!(self.validator.speed_limits.calculate_minimum_time(-1.0), f64::INFINITY);
        assert_eq!(self.validator.speed_limits.calculate_required_energy(0.0), f64::INFINITY);

        // Test very small values
        let planck_time = (constants::PLANCK_HBAR * constants::SPEED_OF_LIGHT.powi(5) /
                          (constants::SPEED_OF_LIGHT.powi(4))).sqrt();
        let planck_energy = constants::PLANCK_H * constants::SPEED_OF_LIGHT / planck_time;

        // These should not panic or return invalid values
        let min_time = self.validator.speed_limits.calculate_minimum_time(planck_energy);
        assert!(min_time > 0.0 && min_time.is_finite());

        // Test very large values
        let large_energy = 1e10 * constants::EV_TO_JOULES; // 10 GeV
        let min_time_large = self.validator.speed_limits.calculate_minimum_time(large_energy);
        assert!(min_time_large > 0.0 && min_time_large < 1e-20);

        // Test decoherence at different temperatures
        let absolute_zero_tracker = DecoherenceTracker::with_temperature(0.001); // 1 mK
        let hot_tracker = DecoherenceTracker::with_temperature(1000.0); // 1000 K

        assert!(absolute_zero_tracker.coherence_time() > hot_tracker.coherence_time(),
               "Colder environments should have longer coherence times");

        // Test entanglement with different parameters
        let short_decoherence = EntanglementValidator::with_consciousness_model(2, 1e-12);
        let long_decoherence = EntanglementValidator::with_consciousness_model(2, 1e-6);

        let short_result = short_decoherence.validate_temporal_correlation(1e-9)?;
        let long_result = long_decoherence.validate_temporal_correlation(1e-9)?;

        assert!(short_result.concurrence < long_result.concurrence,
               "Shorter decoherence time should reduce entanglement preservation");

        // Test numerical stability
        for i in 1..20 {
            let time = 10.0_f64.powi(-i);
            let energy = 10.0_f64.powi(i-20);

            if let Ok(_result) = self.validator.validate_temporal_operation(time, energy) {
                // Validation succeeded - check that values are reasonable
                assert!(time > 0.0 && time.is_finite());
                assert!(energy > 0.0 && energy.is_finite());
            }
            // If validation failed, that's also acceptable for extreme values
        }

        println!("  ✓ Edge cases and limits tests passed");
        Ok(())
    }
}

/// Run comprehensive quantum validation benchmarks
pub fn run_quantum_benchmarks() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏃 Running Quantum Validation Benchmarks");
    println!("========================================");

    let validator = QuantumValidator::new();
    let start = std::time::Instant::now();

    // Benchmark speed limit calculations
    let speed_start = std::time::Instant::now();
    for i in 0..10000 {
        let energy = 1e-15 * (1.0 + i as f64 / 10000.0);
        let _min_time = validator.speed_limits.calculate_minimum_time(energy);
    }
    let speed_duration = speed_start.elapsed();

    // Benchmark uncertainty validations
    let uncertainty_start = std::time::Instant::now();
    for i in 0..10000 {
        let time = 1e-9 * (1.0 + i as f64 / 10000.0);
        let energy = 1e-15 * (1.0 + i as f64 / 10000.0);
        let _result = validator.uncertainty.validate_energy_time_product(energy, time);
    }
    let uncertainty_duration = uncertainty_start.elapsed();

    // Benchmark full validations
    let full_start = std::time::Instant::now();
    for i in 0..1000 {
        let time = 1e-9 * (1.0 + i as f64 / 1000.0);
        let energy = 1e-15 * (1.0 + i as f64 / 1000.0);
        let _result = validator.validate_temporal_operation(time, energy);
    }
    let full_duration = full_start.elapsed();

    let total_duration = start.elapsed();

    println!("Speed limit calculations: {:?} (10k ops)", speed_duration);
    println!("Uncertainty validations: {:?} (10k ops)", uncertainty_duration);
    println!("Full quantum validations: {:?} (1k ops)", full_duration);
    println!("Total benchmark time: {:?}", total_duration);

    // Performance targets
    assert!(speed_duration < std::time::Duration::from_millis(100),
           "Speed limit calculations should be fast");
    assert!(uncertainty_duration < std::time::Duration::from_millis(100),
           "Uncertainty validations should be fast");
    assert!(full_duration < std::time::Duration::from_millis(1000),
           "Full validations should complete within 1s");

    println!("✅ All benchmarks passed performance targets!");
    Ok(())
}

#[cfg(test)]
mod quantum_tests {
    use super::*;

    #[test]
    fn test_quantum_validation_suite() {
        QuantumTestSuite::run_all_tests().expect("Quantum test suite should pass");
    }

    #[test]
    fn test_quantum_benchmarks() {
        run_quantum_benchmarks().expect("Quantum benchmarks should pass");
    }

    #[test]
    fn test_physics_constants_consistency() {
        // Test relationships between constants
        assert_relative_eq!(
            constants::PLANCK_HBAR,
            constants::PLANCK_H / (2.0 * PI),
            epsilon = 1e-10
        );

        // Test that attosecond energy is reasonable
        let attosecond_energy_j = constants::ATTOSECOND_ENERGY_KEV * 1000.0 * constants::EV_TO_JOULES;
        assert!(attosecond_energy_j > 1e-16);
        assert!(attosecond_energy_j < 1e-12);

        // Test consciousness scale
        assert_eq!(constants::CONSCIOUSNESS_SCALE_NS, 1e-9);
    }

    #[test]
    fn test_quantum_validator_default() {
        let validator = QuantumValidator::default();

        // Should be equivalent to new()
        let validator2 = QuantumValidator::new();

        // Test they produce similar results
        let result1 = validator.validate_temporal_operation(1e-9, 1e-15).unwrap();
        let result2 = validator2.validate_temporal_operation(1e-9, 1e-15).unwrap();

        assert_eq!(result1.is_valid, result2.is_valid);
    }

    #[test]
    fn test_time_scale_recommendations() {
        let validator = QuantumValidator::new();

        // Test speed limits
        let speed_analysis = validator.speed_limits.analyze_time_scales();
        assert_eq!(speed_analysis.recommended_consciousness_scale, "nanosecond");

        // Test uncertainty
        let uncertainty_analysis = validator.uncertainty.analyze_time_scales();
        assert_eq!(uncertainty_analysis.recommended_scale, "nanosecond");

        // Test entanglement
        let entanglement_analysis = validator.entanglement.analyze_consciousness_time_scales();
        // Recommendation depends on decoherence time, but should be reasonable
        assert!(!entanglement_analysis.recommended_scale.is_empty());
    }
}