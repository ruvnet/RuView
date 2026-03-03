//! Edge case and error handling tests
//!
//! This module tests boundary conditions, error scenarios, recovery mechanisms,
//! and robustness of the NanosecondScheduler under adverse conditions.

use super::*;

/// Test boundary value edge cases
#[cfg(test)]
mod boundary_value_tests {
    use super::*;

    #[test]
    fn test_zero_values() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule task with zero delay and deadline
        let task = ConsciousnessTask::Perception {
            priority: 0,
            data: vec![],
        };

        let result = scheduler.schedule_task(task, 0, 0);
        assert!(result.is_ok(), "Should handle zero delay/deadline");

        // Process tick
        let tick_result = scheduler.tick();
        assert!(tick_result.is_ok(), "Should handle zero-deadline task");
    }

    #[test]
    fn test_maximum_values() {
        let mut scheduler = NanosecondScheduler::new();

        // Test with maximum possible values
        let max_task = ConsciousnessTask::Perception {
            priority: 255,
            data: vec![255; 1000],
        };

        let result = scheduler.schedule_task(max_task, u64::MAX / 2, u64::MAX / 2);
        assert!(result.is_ok(), "Should handle large delay/deadline values");

        // Test with maximum priority strange loop
        let max_strange_loop = ConsciousnessTask::StrangeLoopProcessing {
            iteration: usize::MAX / 1000, // Avoid overflow
            state: vec![f64::MAX / 1e6; 100], // Large but finite values
        };

        let result2 = scheduler.schedule_task(max_strange_loop, 1000, 10000);
        assert!(result2.is_ok(), "Should handle maximum value tasks");
    }

    #[test]
    fn test_extreme_timestamps() {
        let mut scheduler = NanosecondScheduler::new();

        // Test TSC timestamp edge cases
        let ts_zero = TscTimestamp(0);
        let ts_max = TscTimestamp(u64::MAX);

        // Should handle extreme timestamp values
        let duration = ts_max.nanos_since(ts_zero, scheduler.config.tsc_frequency_hz);
        assert!(duration > 0, "Should calculate duration for extreme timestamps");

        // Test adding nanos to extreme values
        let added = ts_zero.add_nanos(1000, scheduler.config.tsc_frequency_hz);
        assert!(added.0 > 0, "Should add nanoseconds to zero timestamp");
    }

    #[test]
    fn test_empty_data_structures() {
        let mut scheduler = NanosecondScheduler::new();

        // Test with empty data
        let empty_task = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![],
        };
        scheduler.schedule_task(empty_task, 0, 1000).unwrap();

        let empty_memory_task = ConsciousnessTask::MemoryIntegration {
            session_id: String::new(),
            state: vec![],
        };
        scheduler.schedule_task(empty_memory_task, 0, 1000).unwrap();

        let empty_strange_loop = ConsciousnessTask::StrangeLoopProcessing {
            iteration: 0,
            state: vec![],
        };
        scheduler.schedule_task(empty_strange_loop, 0, 1000).unwrap();

        // Should process empty tasks without errors
        for _ in 0..10 {
            scheduler.tick().unwrap();
        }

        assert!(scheduler.metrics.tasks_completed > 0);
    }

    #[test]
    fn test_floating_point_edge_cases() {
        let mut operator = StrangeLoopOperator::new(0.9, 100);

        // Test with special floating point values
        let special_values = [
            vec![0.0, -0.0, 1.0, -1.0],
            vec![f64::MIN, f64::MAX / 1e10], // Scaled down to avoid overflow
            vec![f64::EPSILON, -f64::EPSILON],
            vec![1.0 / 3.0, 2.0 / 3.0], // Non-terminating decimals
        ];

        for (i, state) in special_values.iter().enumerate() {
            let result = operator.process_iteration(i as f64, state);
            assert!(result.is_ok(), "Should handle special float values in iteration {}", i);

            // Check that results are finite
            let fixed_point = operator.get_fixed_point();
            for &value in fixed_point {
                assert!(value.is_finite(), "Fixed point should contain finite values");
            }
        }
    }

    #[test]
    fn test_very_large_data() {
        let mut scheduler = NanosecondScheduler::new();

        // Test with very large data payload
        let large_data = vec![0x42; 10 * 1024 * 1024]; // 10MB
        let large_task = ConsciousnessTask::Perception {
            priority: 100,
            data: large_data,
        };

        let result = scheduler.schedule_task(large_task, 0, 100000);
        assert!(result.is_ok(), "Should handle very large data payloads");

        // Should process without crashing
        scheduler.tick().unwrap();
    }

    #[test]
    fn test_string_boundary_cases() {
        let mut scheduler = NanosecondScheduler::new();

        // Test with various string edge cases
        let string_cases = [
            String::new(),                    // Empty
            " ".repeat(10000),               // Large spaces
            "a".repeat(1000),                // Large single char
            "🌟🚀⚡".repeat(100),            // Unicode
            "\n\t\r\0".repeat(50),          // Control characters
        ];

        for (i, session_id) in string_cases.iter().enumerate() {
            let task = ConsciousnessTask::MemoryIntegration {
                session_id: session_id.clone(),
                state: vec![i as u8; 10],
            };

            let result = scheduler.schedule_task(task, 0, 1000);
            assert!(result.is_ok(), "Should handle string edge case {}", i);
        }

        // Process all tasks
        for _ in 0..20 {
            scheduler.tick().unwrap();
        }
    }
}

/// Test error conditions and recovery
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_task_queue_overflow_recovery() {
        let mut scheduler = NanosecondScheduler::new();

        // Fill queue to capacity
        for i in 0..10000 {
            let task = ConsciousnessTask::Perception {
                priority: 100,
                data: vec![i as u8],
            };

            let result = scheduler.schedule_task(task, i * 100, (i + 1) * 1000);
            assert!(result.is_ok(), "Should schedule up to capacity");
        }

        // Next task should fail with overflow
        let overflow_task = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![42],
        };

        let overflow_result = scheduler.schedule_task(overflow_task, 0, 1000);
        assert!(overflow_result.is_err(), "Should detect queue overflow");

        match overflow_result.unwrap_err() {
            TemporalError::TaskQueueOverflow { current_size, max_size } => {
                assert_eq!(current_size, 10000);
                assert_eq!(max_size, 10000);
            },
            _ => panic!("Expected TaskQueueOverflow error"),
        }

        // Process some tasks to make room
        for _ in 0..100 {
            scheduler.tick().unwrap();
        }

        // Should be able to schedule again
        let recovery_task = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![123],
        };

        let recovery_result = scheduler.schedule_task(recovery_task, 0, 1000);
        assert!(recovery_result.is_ok(), "Should recover from overflow");
    }

    #[test]
    fn test_scheduling_overhead_violations() {
        let tight_config = TemporalConfig {
            max_scheduling_overhead_ns: 1, // Extremely tight limit
            ..Default::default()
        };

        let mut scheduler = NanosecondScheduler::with_config(tight_config);

        // Schedule a complex task that might exceed overhead
        let complex_task = ConsciousnessTask::StrangeLoopProcessing {
            iteration: 1000,
            state: vec![1.0; 1000],
        };
        scheduler.schedule_task(complex_task, 0, 1000).unwrap();

        // Process tick - may trigger overhead violation
        let mut overhead_violations = 0;
        for _ in 0..100 {
            let result = scheduler.tick();

            match result {
                Ok(_) => continue,
                Err(TemporalError::SchedulingOverhead { actual_ns, limit_ns }) => {
                    assert!(actual_ns > limit_ns);
                    overhead_violations += 1;
                    break;
                },
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        if overhead_violations > 0 {
            println!("Successfully detected scheduling overhead violations: {}", overhead_violations);
        } else {
            println!("No overhead violations detected (system may be very fast)");
        }
    }

    #[test]
    fn test_window_overlap_violations() {
        let strict_config = TemporalConfig {
            window_overlap_percent: 99.0, // Very strict overlap requirement
            ..Default::default()
        };

        let mut scheduler = NanosecondScheduler::with_config(strict_config);

        // Process many ticks to potentially trigger overlap violations
        let mut overlap_violations = 0;
        for tick in 0..2000 {
            let result = scheduler.tick();

            match result {
                Ok(_) => continue,
                Err(TemporalError::WindowOverlapTooLow { actual, required }) => {
                    assert!(actual < required);
                    overlap_violations += 1;
                    println!("Overlap violation at tick {}: {:.1}% < {:.1}%", tick, actual, required);
                    break;
                },
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        if overlap_violations == 0 {
            println!("No overlap violations detected with 99% requirement");
        }
    }

    #[test]
    fn test_identity_continuity_violations() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule identity preservation task
        let identity_task = ConsciousnessTask::IdentityPreservation {
            continuity_check: true,
        };
        scheduler.schedule_task(identity_task, 0, 1000).unwrap();

        // Process task
        for _ in 0..10 {
            scheduler.tick().unwrap();
        }

        // Check continuity metrics
        let continuity_metrics = scheduler.measure_continuity();
        assert!(continuity_metrics.is_ok(), "Should be able to measure continuity");

        let metrics = continuity_metrics.unwrap();
        assert!(metrics.continuity_score >= 0.0);
        assert!(metrics.identity_stability >= 0.0);
    }

    #[test]
    fn test_quantum_validation_failures() {
        let mut scheduler = NanosecondScheduler::new();

        // Process operations that might challenge quantum limits
        for _ in 0..50 {
            scheduler.tick().unwrap();

            // Schedule high-energy tasks
            let energy_task = ConsciousnessTask::StrangeLoopProcessing {
                iteration: 1000,
                state: vec![1000.0; 100], // High-energy state
            };
            scheduler.schedule_task(energy_task, 0, 1).unwrap(); // Very tight deadline
        }

        let quantum_analysis = scheduler.get_quantum_analysis();

        println!("Quantum validation under stress:");
        println!("  Validity rate: {:.1}%", quantum_analysis.validity_rate * 100.0);
        println!("  Total validations: {}", quantum_analysis.total_validations);

        // System should continue operating despite quantum violations
        assert!(quantum_analysis.total_validations > 0);

        if quantum_analysis.validity_rate < 0.8 {
            println!("Expected: Some quantum validation failures under aggressive timing");
        }
    }

    #[test]
    fn test_memory_exhaustion_simulation() {
        let mut scheduler = NanosecondScheduler::new();

        // Import increasingly large memory states
        for size_kb in [1, 10, 100, 1000, 10000] {
            let large_state = vec![0x55; size_kb * 1024];

            let import_result = scheduler.import_memory_state(large_state);
            if import_result.is_err() {
                println!("Memory limit reached at {}KB", size_kb);
                break;
            }

            // Test that export still works
            let export_result = scheduler.export_memory_state();
            assert!(export_result.is_ok(), "Export should work after import");
        }

        // Scheduler should remain functional
        scheduler.tick().unwrap();
    }

    #[test]
    fn test_concurrent_access_errors() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let scheduler = Arc::new(Mutex::new(NanosecondScheduler::new()));

        // Simulate potential race conditions
        let handles: Vec<_> = (0..8).map(|thread_id| {
            let sched_clone = scheduler.clone();

            thread::spawn(move || {
                let mut errors = Vec::new();

                for i in 0..100 {
                    // Attempt to get lock and perform operations
                    match sched_clone.try_lock() {
                        Ok(mut sched) => {
                            // Perform various operations
                            let task = ConsciousnessTask::Perception {
                                priority: thread_id as u8,
                                data: vec![i as u8; 10],
                            };

                            if let Err(e) = sched.schedule_task(task, 0, 1000) {
                                errors.push(format!("Schedule error: {:?}", e));
                            }

                            if let Err(e) = sched.tick() {
                                errors.push(format!("Tick error: {:?}", e));
                            }
                        },
                        Err(_) => {
                            // Lock contention - this is expected
                            continue;
                        }
                    }
                }

                errors
            })
        }).collect();

        // Collect all errors
        let mut all_errors = Vec::new();
        for handle in handles {
            let errors = handle.join().unwrap();
            all_errors.extend(errors);
        }

        println!("Concurrent access errors: {}", all_errors.len());
        for error in &all_errors {
            println!("  {}", error);
        }

        // Some errors may be expected under contention
        if all_errors.len() > 50 {
            panic!("Too many concurrent access errors: {}", all_errors.len());
        }
    }
}

/// Test recovery and resilience mechanisms
#[cfg(test)]
mod resilience_tests {
    use super::*;

    #[test]
    fn test_state_corruption_recovery() {
        let mut scheduler = NanosecondScheduler::new();

        // Build normal state
        for i in 0..50 {
            let task = ConsciousnessTask::Perception {
                priority: 100,
                data: vec![i as u8; 10],
            };
            scheduler.schedule_task(task, 0, 1000).unwrap();
            scheduler.tick().unwrap();
        }

        let normal_metrics = scheduler.get_metrics().clone();

        // Simulate state corruption by importing corrupted memory
        let corrupted_state = vec![0xFF; 1000]; // All-ones pattern
        scheduler.import_memory_state(corrupted_state).unwrap();

        // System should continue operating
        for _ in 0..20 {
            let result = scheduler.tick();
            assert!(result.is_ok(), "Should recover from state corruption");
        }

        // Schedule new tasks after corruption
        for i in 0..10 {
            let task = ConsciousnessTask::IdentityPreservation {
                continuity_check: true,
            };
            scheduler.schedule_task(task, 0, 1000).unwrap();
        }

        // Process recovery tasks
        for _ in 0..30 {
            scheduler.tick().unwrap();
        }

        let recovery_metrics = scheduler.get_metrics();
        assert!(recovery_metrics.tasks_completed > normal_metrics.tasks_completed);
    }

    #[test]
    fn test_temporal_discontinuity_handling() {
        let mut scheduler = NanosecondScheduler::new();

        // Create temporal discontinuity by jumping time
        let start_time = TscTimestamp::now();

        // Schedule task in the "past"
        let past_task = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![1, 2, 3],
        };
        scheduler.schedule_task(past_task, 0, 1000).unwrap();

        // Simulate large time jump (several seconds in TSC cycles)
        let large_jump = 3_000_000_000; // 1 second at 3GHz
        let future_time = TscTimestamp(start_time.0 + large_jump);

        // Schedule task in the "future"
        let future_task = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![4, 5, 6],
        };
        scheduler.schedule_task(future_task, large_jump / 1000, large_jump / 500).unwrap();

        // Process should handle temporal discontinuity
        for _ in 0..100 {
            let result = scheduler.tick();
            assert!(result.is_ok(), "Should handle temporal discontinuity");
        }

        assert!(scheduler.metrics.tasks_completed > 0);
    }

    #[test]
    fn test_component_failure_isolation() {
        let mut scheduler = NanosecondScheduler::new();

        // Test that failure in one component doesn't crash others

        // Schedule tasks that might stress different components
        let stress_tasks = [
            // Strange loop with extreme values
            ConsciousnessTask::StrangeLoopProcessing {
                iteration: usize::MAX / 1000000,
                state: vec![f64::MAX / 1e10; 50],
            },
            // Identity preservation
            ConsciousnessTask::IdentityPreservation {
                continuity_check: true,
            },
            // Window management with extreme overlap
            ConsciousnessTask::WindowManagement {
                window_id: u64::MAX / 1000,
                overlap_target: 100.0,
            },
            // Memory integration with large state
            ConsciousnessTask::MemoryIntegration {
                session_id: "stress_test".to_string(),
                state: vec![0xFF; 10000],
            },
        ];

        for task in stress_tasks.iter() {
            scheduler.schedule_task(task.clone(), 0, 10000).unwrap();
        }

        // Process with potential component stress
        let mut successful_ticks = 0;
        for _ in 0..100 {
            match scheduler.tick() {
                Ok(_) => successful_ticks += 1,
                Err(e) => {
                    println!("Tick failed (expected under stress): {:?}", e);
                    // Continue processing - failures should be isolated
                }
            }
        }

        println!("Successful ticks under component stress: {}/{}", successful_ticks, 100);
        assert!(successful_ticks > 50, "Too many failures - poor isolation");
    }

    #[test]
    fn test_gradual_degradation_handling() {
        let mut scheduler = NanosecondScheduler::new();

        // Simulate gradual system degradation
        for degradation_level in 1..=10 {
            // Increase load gradually
            for _ in 0..degradation_level * 10 {
                let task = ConsciousnessTask::StrangeLoopProcessing {
                    iteration: degradation_level * 10,
                    state: vec![1.0; degradation_level * 5],
                };
                scheduler.schedule_task(task, 0, degradation_level as u64 * 1000).unwrap();
            }

            // Measure performance at this degradation level
            let start_time = std::time::Instant::now();
            let mut ticks_processed = 0;

            while start_time.elapsed().as_millis() < 10 && ticks_processed < 100 {
                if scheduler.tick().is_ok() {
                    ticks_processed += 1;
                }
            }

            let performance = ticks_processed as f64 / start_time.elapsed().as_secs_f64();
            println!("Degradation level {}: {:.0} ticks/sec", degradation_level, performance);

            // System should degrade gracefully, not crash
            assert!(performance > 100.0, "Performance degraded too severely at level {}", degradation_level);
        }
    }

    #[test]
    fn test_resource_cleanup_after_errors() {
        let mut scheduler = NanosecondScheduler::new();

        // Cause various error conditions and verify cleanup
        for error_type in 0..5 {
            match error_type {
                0 => {
                    // Queue overflow
                    for i in 0..15000 {
                        let task = ConsciousnessTask::Perception {
                            priority: 100,
                            data: vec![i as u8],
                        };
                        let _ = scheduler.schedule_task(task, 0, 1000);
                    }
                },
                1 => {
                    // Large memory operations
                    let large_state = vec![0x42; 5 * 1024 * 1024];
                    let _ = scheduler.import_memory_state(large_state);
                },
                2 => {
                    // Extreme timing operations
                    for _ in 0..100 {
                        let _ = scheduler.tick();
                    }
                },
                3 => {
                    // Identity continuity stress
                    for _ in 0..50 {
                        let task = ConsciousnessTask::IdentityPreservation {
                            continuity_check: true,
                        };
                        let _ = scheduler.schedule_task(task, 0, 1);
                    }
                },
                4 => {
                    // Window management stress
                    for i in 0..100 {
                        let task = ConsciousnessTask::WindowManagement {
                            window_id: i,
                            overlap_target: 95.0 + (i as f64 % 5.0),
                        };
                        let _ = scheduler.schedule_task(task, 0, 1000);
                    }
                },
                _ => unreachable!(),
            }

            // Process some ticks after each error condition
            for _ in 0..20 {
                let _ = scheduler.tick();
            }
        }

        // Verify system is still responsive after all error conditions
        let final_task = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![0xAA, 0xBB, 0xCC],
        };

        let schedule_result = scheduler.schedule_task(final_task, 0, 1000);
        assert!(schedule_result.is_ok(), "System should be responsive after error recovery");

        scheduler.tick().unwrap();
        assert!(scheduler.metrics.total_ticks > 0);
    }
}

/// Test extreme performance and stress conditions
#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    fn test_sustained_maximum_load() {
        let mut scheduler = NanosecondScheduler::new();

        let test_duration = std::time::Duration::from_millis(500);
        let start_time = std::time::Instant::now();

        let mut operations = 0;

        // Sustain maximum load
        while start_time.elapsed() < test_duration {
            // Try to schedule task
            let task = ConsciousnessTask::Perception {
                priority: fastrand::u8(..),
                data: vec![fastrand::u8(..), fastrand::u8(..)],
            };

            if scheduler.schedule_task(task, 0, 10000).is_ok() {
                operations += 1;
            }

            // Process tick
            if scheduler.tick().is_ok() {
                operations += 1;
            }

            // Occasionally check metrics
            if operations % 1000 == 0 {
                let _ = scheduler.get_metrics();
                operations += 1;
            }
        }

        let actual_duration = start_time.elapsed();
        let ops_per_second = operations as f64 / actual_duration.as_secs_f64();

        println!("Sustained maximum load test:");
        println!("  Duration: {:.1}ms", actual_duration.as_millis());
        println!("  Operations: {}", operations);
        println!("  Operations/sec: {:.0}", ops_per_second);
        println!("  Tasks completed: {}", scheduler.metrics.tasks_completed);

        assert!(ops_per_second > 100_000.0, "Sustained load performance too low");
    }

    #[test]
    fn test_memory_pressure_resilience() {
        let mut scheduler = NanosecondScheduler::new();

        // Create memory pressure with large allocations
        let mut memory_hogs = Vec::new();

        for size_mb in 1..=50 {
            let allocation = vec![size_mb as u8; size_mb * 1024 * 1024];
            memory_hogs.push(allocation);

            // Test scheduler under increasing memory pressure
            for _ in 0..10 {
                let task = ConsciousnessTask::MemoryIntegration {
                    session_id: format!("pressure_test_{}", size_mb),
                    state: vec![size_mb as u8; 1000],
                };

                if scheduler.schedule_task(task, 0, 1000).is_err() {
                    break; // Memory exhausted
                }

                if scheduler.tick().is_err() {
                    break; // System stressed
                }
            }

            println!("Memory pressure at {}MB: {} tasks completed",
                    size_mb, scheduler.metrics.tasks_completed);

            // Don't let test consume all system memory
            if size_mb > 20 {
                break;
            }
        }

        // System should remain functional under memory pressure
        assert!(scheduler.metrics.tasks_completed > 0);
    }

    #[test]
    fn test_rapid_configuration_changes() {
        let mut scheduler = NanosecondScheduler::new();

        // Rapidly change configurations through MCP hooks
        for iteration in 0..100 {
            // Simulate consciousness evolution with varying parameters
            let target = 0.1 + (iteration as f64 % 10.0) / 20.0; // 0.1 to 0.6
            let iterations_param = 5 + (iteration % 20); // 5 to 25

            let _ = scheduler.mcp_consciousness_evolve_hook(iterations_param, target);

            // Process some ticks between changes
            for _ in 0..5 {
                let _ = scheduler.tick();
            }

            // Schedule varying tasks
            let task_variety = iteration % 5;
            let task = match task_variety {
                0 => ConsciousnessTask::Perception {
                    priority: (iteration % 256) as u8,
                    data: vec![(iteration % 256) as u8; iteration % 50 + 1],
                },
                1 => ConsciousnessTask::MemoryIntegration {
                    session_id: format!("rapid_{}", iteration),
                    state: vec![(iteration % 256) as u8; 10],
                },
                2 => ConsciousnessTask::IdentityPreservation {
                    continuity_check: iteration % 2 == 0,
                },
                3 => ConsciousnessTask::StrangeLoopProcessing {
                    iteration: iteration % 100,
                    state: vec![iteration as f64 / 100.0; iteration % 10 + 1],
                },
                4 => ConsciousnessTask::WindowManagement {
                    window_id: iteration as u64,
                    overlap_target: 50.0 + (iteration % 50) as f64,
                },
                _ => unreachable!(),
            };

            let _ = scheduler.schedule_task(task, 0, 1000);
        }

        // System should handle rapid changes gracefully
        let final_metrics = scheduler.get_metrics();
        assert!(final_metrics.total_ticks > 100);
        assert!(final_metrics.tasks_completed > 50);

        println!("Rapid configuration changes test:");
        println!("  Final ticks: {}", final_metrics.total_ticks);
        println!("  Final tasks completed: {}", final_metrics.tasks_completed);
    }

    #[test]
    fn test_extreme_timing_precision_stress() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule tasks with extreme timing precision requirements
        for i in 0..1000 {
            let precision_ns = 1 + (i % 100); // 1-100ns precision
            let task = ConsciousnessTask::Perception {
                priority: 255, // Highest priority for timing
                data: vec![i as u8],
            };

            scheduler.schedule_task(task, i, i + precision_ns).unwrap();
        }

        // Process with high timing precision demands
        let mut timing_violations = 0;
        for _ in 0..2000 {
            match scheduler.tick() {
                Ok(_) => {},
                Err(TemporalError::SchedulingOverhead { .. }) => {
                    timing_violations += 1;
                },
                Err(e) => {
                    println!("Unexpected error under timing stress: {:?}", e);
                },
            }
        }

        println!("Extreme timing precision stress:");
        println!("  Timing violations: {}", timing_violations);
        println!("  Tasks completed: {}", scheduler.metrics.tasks_completed);

        // Should complete most tasks despite extreme precision demands
        assert!(scheduler.metrics.tasks_completed > 500);
    }
}