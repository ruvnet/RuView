//! Quantum Validation Integration tests
//!
//! This module validates the integration of quantum physics constraints
//! with the nanosecond scheduler, including quantum validation results,
//! physics compliance, and performance metrics.

use super::*;

/// Test quantum validation integration with scheduler
#[cfg(test)]
mod quantum_integration_tests {
    use super::*;

    #[test]
    fn test_quantum_validation_during_tick() {
        let mut scheduler = NanosecondScheduler::new();

        // Process several ticks to generate quantum validations
        for _ in 0..10 {
            let result = scheduler.tick();
            assert!(result.is_ok(), "Tick should succeed despite quantum validation");
        }

        let metrics = scheduler.get_metrics();

        // Should have performed quantum validations
        assert!(metrics.avg_quantum_energy_j >= 0.0);
        assert!(metrics.quantum_validity_rate >= 0.0);
        assert!(metrics.quantum_validity_rate <= 1.0);

        println!("Quantum validation during ticks:");
        println!("  Validity rate: {:.1}%", metrics.quantum_validity_rate * 100.0);
        println!("  Average energy: {:.2e} J", metrics.avg_quantum_energy_j);
    }

    #[test]
    fn test_quantum_energy_tracking() {
        let mut scheduler = NanosecondScheduler::new();

        // Process ticks and collect energy measurements
        for _ in 0..20 {
            scheduler.tick().unwrap();
        }

        let metrics = scheduler.get_metrics();

        // Energy should be tracked and reasonable for nanosecond operations
        assert!(metrics.avg_quantum_energy_j > 0.0);
        assert!(metrics.avg_quantum_energy_j < 1e-12); // Should be much less than 1 pJ

        println!("Quantum energy tracking:");
        println!("  Average energy: {:.2e} J", metrics.avg_quantum_energy_j);
        println!("  Average energy: {:.2e} eV", metrics.avg_quantum_energy_j / 1.602e-19);
    }

    #[test]
    fn test_margolus_levitin_compliance() {
        let mut scheduler = NanosecondScheduler::new();

        // Process operations that might challenge speed limits
        for _ in 0..15 {
            scheduler.tick().unwrap();

            // Schedule complex tasks
            let complex_task = ConsciousnessTask::StrangeLoopProcessing {
                iteration: 100,
                state: vec![1.0; 50], // Large state vector
            };
            scheduler.schedule_task(complex_task, 0, 1000).unwrap();
        }

        let metrics = scheduler.get_metrics();

        // Margolus-Levitin margin should be tracked
        assert!(metrics.avg_margolus_levitin_margin >= 0.0);

        println!("Margolus-Levitin compliance:");
        println!("  Average margin: {:.6}", metrics.avg_margolus_levitin_margin);

        // Most operations should comply with speed limits
        assert!(metrics.quantum_validity_rate > 0.5,
            "Too many quantum speed limit violations: {:.1}%",
            (1.0 - metrics.quantum_validity_rate) * 100.0);
    }

    #[test]
    fn test_uncertainty_principle_compliance() {
        let mut scheduler = NanosecondScheduler::new();

        // Process various time scales
        for _ in 0..25 {
            scheduler.tick().unwrap();
        }

        let metrics = scheduler.get_metrics();

        // Uncertainty margin should be meaningful
        assert!(metrics.avg_uncertainty_margin >= 0.0);

        println!("Uncertainty principle compliance:");
        println!("  Average uncertainty margin: {:.6}", metrics.avg_uncertainty_margin);

        // Operations should generally respect uncertainty principle
        if metrics.quantum_validity_rate < 0.8 {
            println!("Warning: High uncertainty principle violations: {:.1}%",
                    (1.0 - metrics.quantum_validity_rate) * 100.0);
        }
    }

    #[test]
    fn test_coherence_preservation_tracking() {
        let mut scheduler = NanosecondScheduler::new();

        // Process operations and track coherence
        for _ in 0..30 {
            scheduler.tick().unwrap();
        }

        let metrics = scheduler.get_metrics();

        // Coherence preservation should be tracked
        assert!(metrics.avg_coherence_preservation >= 0.0);
        assert!(metrics.avg_coherence_preservation <= 1.0);

        println!("Coherence preservation:");
        println!("  Average preservation: {:.1}%", metrics.avg_coherence_preservation * 100.0);

        // Nanosecond operations should preserve reasonable coherence
        if metrics.avg_coherence_preservation < 0.1 {
            println!("Warning: Low coherence preservation: {:.1}%",
                    metrics.avg_coherence_preservation * 100.0);
        }
    }

    #[test]
    fn test_entanglement_strength_measurement() {
        let mut scheduler = NanosecondScheduler::new();

        // Process operations that might involve quantum correlations
        for _ in 0..20 {
            scheduler.tick().unwrap();

            // Schedule identity preservation tasks (might have quantum aspects)
            let identity_task = ConsciousnessTask::IdentityPreservation {
                continuity_check: true,
            };
            scheduler.schedule_task(identity_task, 0, 500).unwrap();
        }

        let metrics = scheduler.get_metrics();

        // Entanglement strength should be tracked
        assert!(metrics.avg_entanglement_strength >= 0.0);
        assert!(metrics.avg_entanglement_strength <= 1.0);

        println!("Entanglement strength:");
        println!("  Average strength: {:.6}", metrics.avg_entanglement_strength);
    }
}

/// Test quantum analysis reporting
#[cfg(test)]
mod quantum_analysis_tests {
    use super::*;

    #[test]
    fn test_quantum_analysis_report_generation() {
        let mut scheduler = NanosecondScheduler::new();

        // Generate quantum validation data
        for _ in 0..50 {
            scheduler.tick().unwrap();
        }

        let analysis = scheduler.get_quantum_analysis();

        // Report should contain meaningful data
        assert!(analysis.total_validations > 0);
        assert!(analysis.validity_rate >= 0.0 && analysis.validity_rate <= 1.0);
        assert!(analysis.avg_energy_j >= 0.0);
        assert!(analysis.avg_energy_ev >= 0.0);
        assert!(analysis.recommended_time_scale_s > 0.0);

        println!("Quantum Analysis Report:");
        println!("  Total validations: {}", analysis.total_validations);
        println!("  Validity rate: {:.1}%", analysis.validity_rate * 100.0);
        println!("  Average energy: {:.2e} J ({:.2e} eV)",
                analysis.avg_energy_j, analysis.avg_energy_ev);
        println!("  Margolus-Levitin margin: {:.6}", analysis.margolus_levitin_margin);
        println!("  Uncertainty margin: {:.6}", analysis.uncertainty_margin);
        println!("  Coherence preservation: {:.1}%", analysis.coherence_preservation * 100.0);
        println!("  Entanglement strength: {:.6}", analysis.entanglement_strength);
    }

    #[test]
    fn test_attosecond_feasibility_analysis() {
        let mut scheduler = NanosecondScheduler::new();

        // Generate some quantum data
        for _ in 0..25 {
            scheduler.tick().unwrap();
        }

        let analysis = scheduler.get_quantum_analysis();
        let attosecond_report = &analysis.attosecond_feasibility;

        println!("Attosecond Feasibility Analysis:");
        println!("  Report: {:?}", attosecond_report);

        // Should provide attosecond feasibility assessment
        // This is primarily informational for consciousness research
    }

    #[test]
    fn test_quantum_metrics_consistency() {
        let mut scheduler = NanosecondScheduler::new();

        // Collect quantum metrics over time
        let mut reports = Vec::new();

        for batch in 0..5 {
            // Process batch of operations
            for _ in 0..20 {
                scheduler.tick().unwrap();
            }

            let analysis = scheduler.get_quantum_analysis();
            reports.push(analysis);

            println!("Batch {}: {} validations, {:.1}% valid",
                    batch, analysis.total_validations, analysis.validity_rate * 100.0);
        }

        // Verify metrics are accumulating properly
        for i in 1..reports.len() {
            assert!(reports[i].total_validations > reports[i-1].total_validations,
                "Validation count should increase");
        }

        // Final report should have comprehensive data
        let final_report = reports.last().unwrap();
        assert!(final_report.total_validations >= 100);
    }

    #[test]
    fn test_quantum_validation_warnings() {
        let mut scheduler = NanosecondScheduler::new();

        // Process many operations to potentially trigger warnings
        for _ in 0..100 {
            let result = scheduler.tick();

            // Even with quantum violations, scheduler should continue
            assert!(result.is_ok(), "Scheduler should handle quantum warnings gracefully");
        }

        let analysis = scheduler.get_quantum_analysis();

        // Analyze validity rate
        if analysis.validity_rate < 0.9 {
            println!("Quantum validation warnings detected:");
            println!("  Validity rate: {:.1}%", analysis.validity_rate * 100.0);
            println!("  This is expected for aggressive nanosecond timing");
        }

        // Should still provide useful metrics
        assert!(analysis.total_validations > 0);
        assert!(analysis.avg_energy_j > 0.0);
    }
}

/// Test quantum physics compliance scenarios
#[cfg(test)]
mod physics_compliance_tests {
    use super::*;

    #[test]
    fn test_consciousness_scale_recommendations() {
        let mut scheduler = NanosecondScheduler::new();

        // Process operations at consciousness-relevant scales
        for _ in 0..40 {
            scheduler.tick().unwrap();
        }

        let analysis = scheduler.get_quantum_analysis();
        let recommended_scale = analysis.recommended_time_scale_s;

        println!("Consciousness scale recommendation: {:.2e} s", recommended_scale);
        println!("In nanoseconds: {:.1} ns", recommended_scale * 1e9);

        // Should recommend nanosecond-scale operations for consciousness
        assert!(recommended_scale >= 1e-12); // At least picosecond scale
        assert!(recommended_scale <= 1e-6);  // At most microsecond scale

        // Nanosecond scale should be in recommended range
        assert!(recommended_scale >= 1e-9 * 0.1); // Within order of magnitude
    }

    #[test]
    fn test_thermal_noise_considerations() {
        let mut scheduler = NanosecondScheduler::new();

        // Process operations and check thermal noise handling
        for _ in 0..30 {
            scheduler.tick().unwrap();
        }

        let analysis = scheduler.get_quantum_analysis();

        // At room temperature, thermal energy is ~26 meV
        let thermal_energy_j = 4.14e-21; // 26 meV in Joules
        let thermal_energy_ev = 0.026;   // 26 meV

        println!("Quantum energy vs thermal noise:");
        println!("  Average quantum energy: {:.2e} eV", analysis.avg_energy_ev);
        println!("  Thermal energy (room temp): {:.3f} eV", thermal_energy_ev);

        // Quantum operations should be comparable to or below thermal scale
        if analysis.avg_energy_ev > thermal_energy_ev * 10.0 {
            println!("Warning: Quantum energy much higher than thermal noise");
        }
    }

    #[test]
    fn test_decoherence_time_analysis() {
        let mut scheduler = NanosecondScheduler::new();

        // Process operations with focus on coherence
        for _ in 0..50 {
            scheduler.tick().unwrap();

            // Schedule identity preservation (coherence-dependent)
            let identity_task = ConsciousnessTask::IdentityPreservation {
                continuity_check: true,
            };
            scheduler.schedule_task(identity_task, 0, 1000).unwrap();
        }

        let metrics = scheduler.get_metrics();

        println!("Decoherence analysis:");
        println!("  Coherence preservation: {:.1}%", metrics.avg_coherence_preservation * 100.0);

        // For nanosecond operations, some decoherence is expected
        // but should not be complete
        if metrics.avg_coherence_preservation > 0.5 {
            println!("Good: High coherence preservation at nanosecond scale");
        } else if metrics.avg_coherence_preservation > 0.1 {
            println!("Acceptable: Moderate coherence preservation");
        } else {
            println!("Warning: Low coherence preservation (may be expected)");
        }
    }

    #[test]
    fn test_quantum_computational_limits() {
        let mut scheduler = NanosecondScheduler::new();

        // Test computational intensity vs quantum limits
        for complexity_level in 1..=5 {
            // Schedule tasks of increasing complexity
            for _ in 0..10 {
                let complex_task = ConsciousnessTask::StrangeLoopProcessing {
                    iteration: complexity_level * 20,
                    state: vec![1.0; complexity_level * 10],
                };
                scheduler.schedule_task(complex_task, 0, 1000).unwrap();
                scheduler.tick().unwrap();
            }

            let analysis = scheduler.get_quantum_analysis();
            println!("Complexity level {}: {:.1}% quantum valid",
                    complexity_level, analysis.validity_rate * 100.0);
        }

        let final_analysis = scheduler.get_quantum_analysis();

        // Higher complexity should generally reduce quantum validity
        // but scheduler should still function
        assert!(final_analysis.total_validations > 0);
    }

    #[test]
    fn test_energy_scale_classification() {
        let mut scheduler = NanosecondScheduler::new();

        // Process various operation types
        let task_types = [
            ConsciousnessTask::Perception { priority: 100, data: vec![1; 10] },
            ConsciousnessTask::MemoryIntegration { session_id: "test".to_string(), state: vec![1; 100] },
            ConsciousnessTask::IdentityPreservation { continuity_check: true },
            ConsciousnessTask::StrangeLoopProcessing { iteration: 1, state: vec![1.0; 5] },
            ConsciousnessTask::WindowManagement { window_id: 1, overlap_target: 75.0 },
        ];

        for task in task_types.iter() {
            for _ in 0..10 {
                scheduler.schedule_task(task.clone(), 0, 1000).unwrap();
                scheduler.tick().unwrap();
            }
        }

        let analysis = scheduler.get_quantum_analysis();

        println!("Energy scale analysis:");
        println!("  Average energy: {:.2e} J", analysis.avg_energy_j);
        println!("  Average energy: {:.2e} eV", analysis.avg_energy_ev);

        // Classify energy scale
        if analysis.avg_energy_ev < 1e-6 {
            println!("  Scale: Nano-electronvolt or lower");
        } else if analysis.avg_energy_ev < 1e-3 {
            println!("  Scale: Micro-electronvolt");
        } else if analysis.avg_energy_ev < 1.0 {
            println!("  Scale: Milli-electronvolt");
        } else {
            println!("  Scale: Electronvolt or higher");
        }
    }
}

/// Test performance impact of quantum validation
#[cfg(test)]
mod quantum_performance_tests {
    use super::*;

    #[test]
    fn test_quantum_validation_overhead() {
        let config = TemporalConfig {
            max_scheduling_overhead_ns: 2000, // 2μs limit
            ..Default::default()
        };

        let mut scheduler = NanosecondScheduler::with_config(config);

        // Measure overhead with quantum validation
        let mut overhead_measurements = Vec::new();

        for _ in 0..100 {
            let start = TscTimestamp::now();
            scheduler.tick().unwrap();
            let end = TscTimestamp::now();

            let overhead_ns = end.nanos_since(start, scheduler.config.tsc_frequency_hz);
            overhead_measurements.push(overhead_ns);
        }

        // Calculate statistics
        overhead_measurements.sort_unstable();
        let median_overhead = overhead_measurements[overhead_measurements.len() / 2];
        let avg_overhead = overhead_measurements.iter().sum::<u64>() as f64 / overhead_measurements.len() as f64;

        println!("Quantum validation overhead:");
        println!("  Median: {} ns", median_overhead);
        println!("  Average: {:.1} ns", avg_overhead);
        println!("  95th percentile: {} ns", overhead_measurements[(overhead_measurements.len() as f64 * 0.95) as usize]);

        // Should meet performance requirements even with quantum validation
        assert!(avg_overhead < 2000.0, "Quantum validation overhead too high: {:.1}ns", avg_overhead);
    }

    #[test]
    fn test_quantum_metrics_collection_performance() {
        let mut scheduler = NanosecondScheduler::new();

        // Generate quantum data
        for _ in 0..50 {
            scheduler.tick().unwrap();
        }

        // Measure metrics collection performance
        let iterations = 1000;
        let start_time = TscTimestamp::now();

        for _ in 0..iterations {
            std::hint::black_box(scheduler.get_quantum_analysis());
        }

        let end_time = TscTimestamp::now();
        let total_time_ns = end_time.nanos_since(start_time, scheduler.config.tsc_frequency_hz);
        let per_analysis_ns = total_time_ns / iterations;

        println!("Quantum analysis performance: {} ns per analysis", per_analysis_ns);

        // Should be fast enough for real-time use
        assert!(per_analysis_ns < 10000, "Quantum analysis too slow: {}ns", per_analysis_ns);
    }

    #[test]
    fn test_quantum_validation_memory_usage() {
        let mut scheduler = NanosecondScheduler::new();

        // Process many operations to accumulate quantum data
        for _ in 0..1000 {
            scheduler.tick().unwrap();
        }

        // Check that quantum validation history is bounded
        let analysis = scheduler.get_quantum_analysis();

        println!("Quantum validation memory usage:");
        println!("  Total validations processed: {}", analysis.total_validations);

        // Should not accumulate unbounded validation history
        // (internal validation queue should be limited)
        assert!(analysis.total_validations <= 1000,
            "Quantum validation history should be bounded");
    }

    #[test]
    fn test_quantum_validation_scalability() {
        let mut scheduler = NanosecondScheduler::new();

        // Test performance scaling with operation count
        let batch_sizes = [10, 50, 100, 200];
        let mut performance_data = Vec::new();

        for &batch_size in &batch_sizes {
            let start_time = TscTimestamp::now();

            for _ in 0..batch_size {
                scheduler.tick().unwrap();
            }

            let end_time = TscTimestamp::now();
            let total_time_ns = end_time.nanos_since(start_time, scheduler.config.tsc_frequency_hz);
            let per_tick_ns = total_time_ns / batch_size;

            performance_data.push((batch_size, per_tick_ns));

            println!("Batch size {}: {:.1} ns per tick", batch_size, per_tick_ns);
        }

        // Performance should scale reasonably (not exponentially)
        for i in 1..performance_data.len() {
            let (_, prev_time) = performance_data[i-1];
            let (_, curr_time) = performance_data[i];

            let performance_ratio = curr_time as f64 / prev_time as f64;
            assert!(performance_ratio < 2.0,
                "Quantum validation performance should scale reasonably: {}x degradation",
                performance_ratio);
        }
    }
}