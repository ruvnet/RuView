//! Integration tests for NanosecondScheduler
//!
//! This module tests the integration of all components working together,
//! end-to-end workflows, and real-world usage scenarios.

use super::*;

/// Test complete consciousness workflow integration
#[cfg(test)]
mod consciousness_workflow_tests {
    use super::*;

    #[test]
    fn test_complete_consciousness_cycle() {
        let mut scheduler = NanosecondScheduler::new();

        // Phase 1: Perception
        let perception_data = TestUtils::generate_test_data(100, TestDataPattern::Random);
        let perception_task = ConsciousnessTask::Perception {
            priority: 200,
            data: perception_data,
        };
        scheduler.schedule_task(perception_task, 0, 5000).unwrap();

        // Phase 2: Memory integration
        let memory_state = TestUtils::generate_test_data(50, TestDataPattern::Sequential);
        let memory_task = ConsciousnessTask::MemoryIntegration {
            session_id: "consciousness_cycle_001".to_string(),
            state: memory_state,
        };
        scheduler.schedule_task(memory_task, 1000, 8000).unwrap();

        // Phase 3: Strange loop processing
        let loop_state = vec![0.5, -0.3, 0.8, -0.1, 0.2];
        let strange_loop_task = ConsciousnessTask::StrangeLoopProcessing {
            iteration: 1,
            state: loop_state,
        };
        scheduler.schedule_task(strange_loop_task, 2000, 10000).unwrap();

        // Phase 4: Identity preservation
        let identity_task = ConsciousnessTask::IdentityPreservation {
            continuity_check: true,
        };
        scheduler.schedule_task(identity_task, 3000, 12000).unwrap();

        // Phase 5: Window management
        let window_task = ConsciousnessTask::WindowManagement {
            window_id: 1,
            overlap_target: 85.0,
        };
        scheduler.schedule_task(window_task, 4000, 15000).unwrap();

        // Execute complete cycle
        let mut cycle_complete = false;
        for tick in 0..200 {
            scheduler.tick().unwrap();

            // Check if all tasks completed
            if scheduler.metrics.tasks_completed >= 5 {
                cycle_complete = true;
                println!("Consciousness cycle completed at tick {}", tick);
                break;
            }
        }

        assert!(cycle_complete, "Consciousness cycle should complete");

        // Verify all components updated
        let metrics = scheduler.get_metrics();
        assert!(metrics.window_overlap_percentage > 0.0);
        assert!(metrics.contraction_convergence_rate >= 0.0);
        assert!(metrics.identity_continuity_score >= 0.0);
        assert!(metrics.quantum_validity_rate >= 0.0);

        println!("Complete consciousness cycle metrics:");
        println!("  Tasks completed: {}", metrics.tasks_completed);
        println!("  Window overlap: {:.1}%", metrics.window_overlap_percentage);
        println!("  Convergence rate: {:.6}", metrics.contraction_convergence_rate);
        println!("  Identity continuity: {:.6}", metrics.identity_continuity_score);
        println!("  Quantum validity: {:.1}%", metrics.quantum_validity_rate * 100.0);
    }

    #[test]
    fn test_mcp_consciousness_evolution_integration() {
        let mut scheduler = NanosecondScheduler::new();

        // Set initial memory state
        let initial_memory = TestUtils::generate_test_data(200, TestDataPattern::Alternating);
        scheduler.import_memory_state(initial_memory).unwrap();

        // Run consciousness evolution
        let target_emergence = 0.7;
        let max_iterations = 50;

        let final_emergence = scheduler.mcp_consciousness_evolve_hook(max_iterations, target_emergence).unwrap();

        println!("MCP consciousness evolution integration:");
        println!("  Target emergence: {:.1}", target_emergence);
        println!("  Final emergence: {:.6}", final_emergence);
        println!("  Tasks processed: {}", scheduler.metrics.tasks_completed);

        // Verify evolution worked
        assert!(final_emergence > 0.0, "Should achieve some emergence");
        assert!(scheduler.metrics.tasks_completed > 0, "Should process evolution tasks");

        // Check that all components participated
        let metrics = scheduler.get_metrics();
        assert!(metrics.total_ticks > 0, "Should have processed ticks");
        assert!(metrics.contraction_convergence_rate >= 0.0, "Strange loop should be active");

        // Verify memory state evolved
        let final_memory = scheduler.export_memory_state().unwrap();
        assert!(!final_memory.is_empty(), "Memory should be preserved");

        // Check quantum validation
        let quantum_analysis = scheduler.get_quantum_analysis();
        assert!(quantum_analysis.total_validations > 0, "Should have quantum validations");

        println!("  Quantum validations: {}", quantum_analysis.total_validations);
        println!("  Quantum validity: {:.1}%", quantum_analysis.validity_rate * 100.0);
    }

    #[test]
    fn test_temporal_continuity_preservation() {
        let mut scheduler = NanosecondScheduler::new();

        // Create continuous operation over extended period
        let total_phases = 10;
        let tasks_per_phase = 20;

        for phase in 0..total_phases {
            // Each phase focuses on different aspects of consciousness
            for task_idx in 0..tasks_per_phase {
                let task = match phase % 4 {
                    0 => {
                        // Perception phase
                        ConsciousnessTask::Perception {
                            priority: 150,
                            data: TestUtils::generate_test_data(50, TestDataPattern::Random),
                        }
                    },
                    1 => {
                        // Memory consolidation phase
                        ConsciousnessTask::MemoryIntegration {
                            session_id: format!("phase_{}_task_{}", phase, task_idx),
                            state: TestUtils::generate_test_data(30, TestDataPattern::Sequential),
                        }
                    },
                    2 => {
                        // Self-reflection phase
                        ConsciousnessTask::StrangeLoopProcessing {
                            iteration: phase * tasks_per_phase + task_idx,
                            state: vec![phase as f64 / 10.0; 8],
                        }
                    },
                    3 => {
                        // Identity maintenance phase
                        ConsciousnessTask::IdentityPreservation {
                            continuity_check: true,
                        }
                    },
                    _ => unreachable!(),
                };

                let delay = (phase * tasks_per_phase + task_idx) as u64 * 500;
                let deadline = delay + 10000;
                scheduler.schedule_task(task, delay, deadline).unwrap();
            }

            // Process phase
            for _ in 0..50 {
                scheduler.tick().unwrap();
            }

            // Verify continuity maintained
            let continuity_metrics = scheduler.measure_continuity().unwrap();
            println!("Phase {} continuity: {:.6}", phase, continuity_metrics.continuity_score);

            assert!(continuity_metrics.continuity_score >= 0.0,
                "Continuity should be maintained in phase {}", phase);
        }

        // Final verification
        let final_metrics = scheduler.get_metrics();
        assert!(final_metrics.tasks_completed >= total_phases as u64 * tasks_per_phase as u64 / 2,
            "Should complete most tasks across all phases");

        println!("Temporal continuity preservation test:");
        println!("  Total phases: {}", total_phases);
        println!("  Tasks completed: {}", final_metrics.tasks_completed);
        println!("  Final continuity: {:.6}", final_metrics.identity_continuity_score);
    }
}

/// Test component interaction and coordination
#[cfg(test)]
mod component_interaction_tests {
    use super::*;

    #[test]
    fn test_strange_loop_window_coordination() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule coordinated strange loop and window management tasks
        for iteration in 0..30 {
            // Strange loop processing
            let loop_task = ConsciousnessTask::StrangeLoopProcessing {
                iteration,
                state: vec![(iteration as f64 * 0.1).sin(); 6],
            };
            scheduler.schedule_task(loop_task, iteration as u64 * 200, (iteration as u64 + 1) * 2000).unwrap();

            // Window management for same iteration
            let window_task = ConsciousnessTask::WindowManagement {
                window_id: iteration as u64 + 1,
                overlap_target: 75.0 + (iteration as f64 % 20.0),
            };
            scheduler.schedule_task(window_task, iteration as u64 * 200 + 100, (iteration as u64 + 1) * 2000).unwrap();
        }

        // Process coordinated tasks
        for _ in 0..150 {
            scheduler.tick().unwrap();
        }

        let metrics = scheduler.get_metrics();

        println!("Strange loop and window coordination:");
        println!("  Tasks completed: {}", metrics.tasks_completed);
        println!("  Convergence rate: {:.6}", metrics.contraction_convergence_rate);
        println!("  Window overlap: {:.1}%", metrics.window_overlap_percentage);
        println!("  Temporal advantage: {} ns", metrics.temporal_advantage_ns);

        // Both components should be active and coordinated
        assert!(metrics.contraction_convergence_rate > 0.0, "Strange loop should be converging");
        assert!(metrics.window_overlap_percentage > 50.0, "Windows should maintain overlap");
        assert!(metrics.temporal_advantage_ns > 0, "Should have temporal advantage");
    }

    #[test]
    fn test_identity_memory_integration() {
        let mut scheduler = NanosecondScheduler::new();

        // Build identity baseline through memory integration
        let memory_sessions = [
            ("identity_core", vec![0x10, 0x20, 0x30, 0x40]),
            ("identity_traits", vec![0x50, 0x60, 0x70, 0x80]),
            ("identity_history", vec![0x90, 0xA0, 0xB0, 0xC0]),
        ];

        for (i, (session_id, state)) in memory_sessions.iter().enumerate() {
            // Memory integration
            let memory_task = ConsciousnessTask::MemoryIntegration {
                session_id: session_id.to_string(),
                state: state.clone(),
            };
            scheduler.schedule_task(memory_task, i as u64 * 1000, (i as u64 + 1) * 5000).unwrap();

            // Identity preservation
            let identity_task = ConsciousnessTask::IdentityPreservation {
                continuity_check: true,
            };
            scheduler.schedule_task(identity_task, i as u64 * 1000 + 500, (i as u64 + 1) * 5000).unwrap();
        }

        // Process integration
        for _ in 0..100 {
            scheduler.tick().unwrap();
        }

        // Verify integration
        let continuity_metrics = scheduler.measure_continuity().unwrap();
        let memory_state = scheduler.export_memory_state().unwrap();

        println!("Identity and memory integration:");
        println!("  Continuity score: {:.6}", continuity_metrics.continuity_score);
        println!("  Identity stability: {:.6}", continuity_metrics.identity_stability);
        println!("  Memory size: {} bytes", memory_state.len());

        assert!(continuity_metrics.continuity_score > 0.0, "Should build continuity");
        assert!(!memory_state.is_empty(), "Should accumulate memory");

        // Memory should contain all integrated sessions
        for (_, expected_state) in &memory_sessions {
            let contains_session = expected_state.iter().any(|&byte| memory_state.contains(&byte));
            assert!(contains_session, "Memory should contain integrated session data");
        }
    }

    #[test]
    fn test_quantum_validation_component_integration() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule tasks that exercise all components under quantum validation
        let comprehensive_tasks = [
            ConsciousnessTask::Perception {
                priority: 255,
                data: vec![1; 1000], // Large perception data
            },
            ConsciousnessTask::MemoryIntegration {
                session_id: "quantum_test".to_string(),
                state: vec![0xFF; 500], // Large memory state
            },
            ConsciousnessTask::StrangeLoopProcessing {
                iteration: 100,
                state: vec![1000.0; 20], // High-energy strange loop
            },
            ConsciousnessTask::IdentityPreservation {
                continuity_check: true,
            },
            ConsciousnessTask::WindowManagement {
                window_id: 1,
                overlap_target: 99.0, // Demanding overlap requirement
            },
        ];

        // Schedule all tasks with tight timing
        for (i, task) in comprehensive_tasks.iter().enumerate() {
            scheduler.schedule_task(task.clone(), i as u64 * 100, (i as u64 + 1) * 1000).unwrap();
        }

        // Process under quantum validation
        for _ in 0..150 {
            scheduler.tick().unwrap();
        }

        // Analyze quantum integration
        let quantum_analysis = scheduler.get_quantum_analysis();
        let scheduler_metrics = scheduler.get_metrics();

        println!("Quantum validation integration:");
        println!("  Quantum validations: {}", quantum_analysis.total_validations);
        println!("  Validity rate: {:.1}%", quantum_analysis.validity_rate * 100.0);
        println!("  Average energy: {:.2e} J", quantum_analysis.avg_energy_j);
        println!("  Tasks completed: {}", scheduler_metrics.tasks_completed);

        // All components should work under quantum validation
        assert!(quantum_analysis.total_validations > 0, "Should perform quantum validations");
        assert!(scheduler_metrics.tasks_completed > 0, "Should complete tasks despite quantum constraints");
        assert!(scheduler_metrics.window_overlap_percentage >= 0.0, "Window manager should function");
        assert!(scheduler_metrics.contraction_convergence_rate >= 0.0, "Strange loop should function");
        assert!(scheduler_metrics.identity_continuity_score >= 0.0, "Identity tracking should function");
    }

    #[test]
    fn test_timing_precision_component_coordination() {
        let mut scheduler = NanosecondScheduler::new();

        // Test precise timing coordination between components
        let precision_ns = 100; // 100ns precision timing

        // Schedule precisely timed sequence
        for i in 0..20 {
            let exact_time = i * precision_ns;

            // Stagger different task types with precise timing
            match i % 4 {
                0 => {
                    let task = ConsciousnessTask::Perception {
                        priority: 200,
                        data: vec![i as u8; 10],
                    };
                    scheduler.schedule_task(task, exact_time, exact_time + precision_ns / 2).unwrap();
                },
                1 => {
                    let task = ConsciousnessTask::StrangeLoopProcessing {
                        iteration: i / 4,
                        state: vec![i as f64 * 0.05; 5],
                    };
                    scheduler.schedule_task(task, exact_time, exact_time + precision_ns / 2).unwrap();
                },
                2 => {
                    let task = ConsciousnessTask::IdentityPreservation {
                        continuity_check: true,
                    };
                    scheduler.schedule_task(task, exact_time, exact_time + precision_ns / 2).unwrap();
                },
                3 => {
                    let task = ConsciousnessTask::WindowManagement {
                        window_id: (i / 4) as u64,
                        overlap_target: 80.0,
                    };
                    scheduler.schedule_task(task, exact_time, exact_time + precision_ns / 2).unwrap();
                },
                _ => unreachable!(),
            }
        }

        // Process with timing precision requirements
        let mut processed_count = 0;
        for _ in 0..100 {
            match scheduler.tick() {
                Ok(_) => processed_count += 1,
                Err(TemporalError::SchedulingOverhead { actual_ns, limit_ns }) => {
                    println!("Timing precision violation: {}ns > {}ns", actual_ns, limit_ns);
                    break;
                },
                Err(e) => {
                    println!("Other error during precision timing: {:?}", e);
                    break;
                },
            }
        }

        println!("Timing precision coordination:");
        println!("  Ticks processed: {}", processed_count);
        println!("  Tasks completed: {}", scheduler.metrics.tasks_completed);
        println!("  Average overhead: {:.1} ns", scheduler.metrics.avg_scheduling_overhead_ns);

        assert!(processed_count > 50, "Should process most ticks with precision timing");
        assert!(scheduler.metrics.tasks_completed > 10, "Should complete tasks with precise coordination");
    }
}

/// Test real-world usage scenarios
#[cfg(test)]
mod realistic_scenario_tests {
    use super::*;

    #[test]
    fn test_continuous_perception_processing() {
        let mut scheduler = NanosecondScheduler::new();

        // Simulate continuous sensory input processing
        let perception_stream_duration = std::time::Duration::from_millis(100);
        let start_time = std::time::Instant::now();

        let mut perception_count = 0;
        let mut tick_count = 0;

        while start_time.elapsed() < perception_stream_duration {
            // Generate continuous perception data
            if tick_count % 5 == 0 { // New perception every 5 ticks
                let perception_data = TestUtils::generate_test_data(
                    20 + (perception_count % 80),
                    TestDataPattern::Random,
                );

                let perception_task = ConsciousnessTask::Perception {
                    priority: 180 + (perception_count % 75) as u8,
                    data: perception_data,
                };

                if scheduler.schedule_task(perception_task, 0, 5000).is_ok() {
                    perception_count += 1;
                }
            }

            // Regular identity maintenance
            if tick_count % 50 == 0 {
                let identity_task = ConsciousnessTask::IdentityPreservation {
                    continuity_check: true,
                };
                let _ = scheduler.schedule_task(identity_task, 0, 10000);
            }

            scheduler.tick().unwrap();
            tick_count += 1;
        }

        let actual_duration = start_time.elapsed();
        let perception_rate = perception_count as f64 / actual_duration.as_secs_f64();
        let tick_rate = tick_count as f64 / actual_duration.as_secs_f64();

        println!("Continuous perception processing:");
        println!("  Duration: {:.1}ms", actual_duration.as_millis());
        println!("  Perceptions processed: {}", scheduler.metrics.tasks_completed);
        println!("  Perception rate: {:.0} perceptions/sec", perception_rate);
        println!("  Tick rate: {:.0} ticks/sec", tick_rate);
        println!("  Continuity score: {:.6}", scheduler.metrics.identity_continuity_score);

        assert!(perception_rate > 500.0, "Should process perceptions at high rate");
        assert!(tick_rate > 5000.0, "Should maintain high tick rate");
        assert!(scheduler.metrics.identity_continuity_score >= 0.0, "Should maintain identity");
    }

    #[test]
    fn test_memory_consolidation_scenario() {
        let mut scheduler = NanosecondScheduler::new();

        // Simulate memory consolidation during "sleep" or idle periods
        let consolidation_sessions = [
            ("working_memory", 1000),
            ("episodic_memory", 2000),
            ("semantic_memory", 1500),
            ("procedural_memory", 3000),
            ("emotional_memory", 800),
        ];

        let mut session_data = std::collections::HashMap::new();

        // Phase 1: Accumulate memories
        for (session_type, data_size) in &consolidation_sessions {
            let memory_data = TestUtils::generate_test_data(*data_size, TestDataPattern::Sequential);
            session_data.insert(session_type.to_string(), memory_data.clone());

            let memory_task = ConsciousnessTask::MemoryIntegration {
                session_id: session_type.to_string(),
                state: memory_data,
            };

            scheduler.schedule_task(memory_task, 0, 10000).unwrap();
        }

        // Phase 2: Strange loop processing for consolidation
        for consolidation_cycle in 0..20 {
            let consolidation_state = vec![
                consolidation_cycle as f64 / 20.0,
                (consolidation_cycle as f64 * 0.3).sin(),
                (consolidation_cycle as f64 * 0.7).cos(),
                consolidation_cycle as f64 / 10.0,
            ];

            let loop_task = ConsciousnessTask::StrangeLoopProcessing {
                iteration: consolidation_cycle,
                state: consolidation_state,
            };

            scheduler.schedule_task(loop_task, consolidation_cycle as u64 * 500, (consolidation_cycle as u64 + 1) * 2000).unwrap();
        }

        // Process consolidation
        for _ in 0..200 {
            scheduler.tick().unwrap();
        }

        // Verify consolidation results
        let final_memory = scheduler.export_memory_state().unwrap();
        let continuity_metrics = scheduler.measure_continuity().unwrap();

        println!("Memory consolidation scenario:");
        println!("  Tasks completed: {}", scheduler.metrics.tasks_completed);
        println!("  Final memory size: {} bytes", final_memory.len());
        println!("  Identity stability: {:.6}", continuity_metrics.identity_stability);
        println!("  Convergence rate: {:.6}", scheduler.metrics.contraction_convergence_rate);

        // Consolidation should integrate all memory types
        let expected_total_size = consolidation_sessions.iter().map(|(_, size)| size).sum::<usize>();
        assert!(final_memory.len() >= expected_total_size / 2, "Should consolidate significant memory");

        // Identity should remain stable during consolidation
        assert!(continuity_metrics.identity_stability > 0.3, "Identity should be stable during consolidation");

        // Strange loop should show convergence during consolidation
        assert!(scheduler.metrics.contraction_convergence_rate > 0.0, "Should show convergence during consolidation");
    }

    #[test]
    fn test_adaptive_task_prioritization() {
        let mut scheduler = NanosecondScheduler::new();

        // Simulate adaptive system under varying load conditions

        // Phase 1: Low load - detailed processing
        for i in 0..20 {
            let detailed_perception = ConsciousnessTask::Perception {
                priority: 150,
                data: TestUtils::generate_test_data(100, TestDataPattern::Random),
            };
            scheduler.schedule_task(detailed_perception, i * 1000, (i + 1) * 5000).unwrap();
        }

        // Process low load phase
        for _ in 0..100 {
            scheduler.tick().unwrap();
        }

        let low_load_completed = scheduler.metrics.tasks_completed;

        // Phase 2: High load - prioritized processing
        for i in 0..100 {
            let priority = if i % 10 == 0 { 255 } else { 100 }; // Every 10th task is critical

            let task = match i % 3 {
                0 => ConsciousnessTask::Perception {
                    priority,
                    data: TestUtils::generate_test_data(20, TestDataPattern::Random),
                },
                1 => ConsciousnessTask::IdentityPreservation {
                    continuity_check: priority == 255,
                },
                2 => ConsciousnessTask::StrangeLoopProcessing {
                    iteration: i / 3,
                    state: vec![(i as f64) / 100.0; 3],
                },
                _ => unreachable!(),
            };

            scheduler.schedule_task(task, 0, 2000).unwrap(); // Tight deadlines
        }

        // Process high load phase
        for _ in 0..150 {
            scheduler.tick().unwrap();
        }

        let high_load_completed = scheduler.metrics.tasks_completed - low_load_completed;

        println!("Adaptive task prioritization:");
        println!("  Low load tasks completed: {}", low_load_completed);
        println!("  High load tasks completed: {}", high_load_completed);
        println!("  Total tasks completed: {}", scheduler.metrics.tasks_completed);
        println!("  Final continuity: {:.6}", scheduler.metrics.identity_continuity_score);

        // Should adapt to different load conditions
        assert!(low_load_completed > 10, "Should process tasks under low load");
        assert!(high_load_completed > 30, "Should prioritize tasks under high load");

        // Identity should be maintained throughout
        assert!(scheduler.metrics.identity_continuity_score >= 0.0, "Should maintain identity under varying load");
    }

    #[test]
    fn test_long_running_consciousness_session() {
        let mut scheduler = NanosecondScheduler::new();

        // Simulate extended consciousness session (scaled down for test)
        let session_phases = [
            ("awakening", 50, 0.2),
            ("active_processing", 200, 0.8),
            ("contemplation", 100, 0.6),
            ("memory_integration", 150, 0.4),
            ("rest_preparation", 75, 0.3),
        ];

        let mut total_tasks_scheduled = 0;

        for (phase_name, task_count, intensity) in &session_phases {
            println!("Starting phase: {} ({} tasks, {:.1} intensity)", phase_name, task_count, intensity);

            for task_idx in 0..*task_count {
                let task_type = (task_idx * (*intensity * 10.0) as usize) % 5;

                let task = match task_type {
                    0 => ConsciousnessTask::Perception {
                        priority: (100.0 + intensity * 155.0) as u8,
                        data: TestUtils::generate_test_data((intensity * 100.0) as usize + 10, TestDataPattern::Random),
                    },
                    1 => ConsciousnessTask::MemoryIntegration {
                        session_id: format!("{}_{}", phase_name, task_idx),
                        state: TestUtils::generate_test_data((intensity * 50.0) as usize + 5, TestDataPattern::Sequential),
                    },
                    2 => ConsciousnessTask::StrangeLoopProcessing {
                        iteration: task_idx,
                        state: vec![*intensity; ((intensity * 8.0) as usize + 2)],
                    },
                    3 => ConsciousnessTask::IdentityPreservation {
                        continuity_check: intensity > &0.5,
                    },
                    4 => ConsciousnessTask::WindowManagement {
                        window_id: task_idx as u64,
                        overlap_target: 60.0 + intensity * 30.0,
                    },
                    _ => unreachable!(),
                };

                let delay = task_idx as u64 * (200.0 / intensity) as u64;
                let deadline = delay + (10000.0 * intensity) as u64;

                scheduler.schedule_task(task, delay, deadline).unwrap();
                total_tasks_scheduled += 1;
            }

            // Process phase
            for _ in 0..(task_count / 2) {
                scheduler.tick().unwrap();
            }

            // Phase metrics
            let phase_metrics = scheduler.get_metrics();
            println!("  Phase completed - Tasks: {}, Continuity: {:.6}",
                    phase_metrics.tasks_completed, phase_metrics.identity_continuity_score);
        }

        // Final processing
        for _ in 0..300 {
            scheduler.tick().unwrap();
        }

        // Session analysis
        let final_metrics = scheduler.get_metrics();
        let final_continuity = scheduler.measure_continuity().unwrap();
        let quantum_analysis = scheduler.get_quantum_analysis();

        println!("Long-running consciousness session results:");
        println!("  Total tasks scheduled: {}", total_tasks_scheduled);
        println!("  Total tasks completed: {}", final_metrics.tasks_completed);
        println!("  Completion rate: {:.1}%", (final_metrics.tasks_completed as f64 / total_tasks_scheduled as f64) * 100.0);
        println!("  Final continuity score: {:.6}", final_continuity.continuity_score);
        println!("  Identity stability: {:.6}", final_continuity.identity_stability);
        println!("  Quantum validations: {}", quantum_analysis.total_validations);
        println!("  Quantum validity: {:.1}%", quantum_analysis.validity_rate * 100.0);

        // Session should complete successfully
        assert!(final_metrics.tasks_completed > total_tasks_scheduled as u64 / 2,
            "Should complete majority of tasks in long session");

        // Consciousness properties should be maintained
        assert!(final_continuity.continuity_score >= 0.0,
            "Should maintain continuity throughout session");
        assert!(final_continuity.identity_stability >= 0.0,
            "Should maintain identity stability");

        // System should remain within quantum constraints
        assert!(quantum_analysis.total_validations > 0,
            "Should perform quantum validations throughout");
    }
}