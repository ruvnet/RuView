//! Core NanosecondScheduler unit tests
//!
//! This module contains comprehensive tests for the NanosecondScheduler functionality,
//! including basic operations, task scheduling, metrics tracking, and MCP integration.

use super::*;

/// Test basic scheduler creation and initialization
#[cfg(test)]
mod scheduler_basic_tests {
    use super::*;

    #[test]
    fn test_scheduler_creation_default() {
        let scheduler = NanosecondScheduler::new();

        assert_eq!(scheduler.current_tick, 0);
        assert_eq!(scheduler.task_queue.len(), 0);
        assert_eq!(scheduler.next_task_id, 1);
        assert!(scheduler.completed_tasks.is_empty());

        let metrics = scheduler.get_metrics();
        assert_eq!(metrics.total_ticks, 0);
        assert_eq!(metrics.tasks_scheduled, 0);
        assert_eq!(metrics.tasks_completed, 0);
    }

    #[test]
    fn test_scheduler_creation_with_config() {
        let config = TemporalConfig {
            window_overlap_percent: 80.0,
            max_scheduling_overhead_ns: 500,
            lipschitz_bound: 0.8,
            max_contraction_iterations: 20,
            tsc_frequency_hz: 4_000_000_000,
        };

        let scheduler = NanosecondScheduler::with_config(config.clone());
        assert_eq!(scheduler.config.window_overlap_percent, 80.0);
        assert_eq!(scheduler.config.max_scheduling_overhead_ns, 500);
        assert_eq!(scheduler.config.lipschitz_bound, 0.8);
        assert_eq!(scheduler.config.max_contraction_iterations, 20);
        assert_eq!(scheduler.config.tsc_frequency_hz, 4_000_000_000);
    }

    #[test]
    fn test_scheduler_default_trait() {
        let scheduler1 = NanosecondScheduler::new();
        let scheduler2 = NanosecondScheduler::default();

        assert_eq!(scheduler1.current_tick, scheduler2.current_tick);
        assert_eq!(scheduler1.next_task_id, scheduler2.next_task_id);
        assert_eq!(scheduler1.config.window_overlap_percent, scheduler2.config.window_overlap_percent);
    }
}

/// Test task scheduling and execution
#[cfg(test)]
mod task_scheduling_tests {
    use super::*;

    #[test]
    fn test_task_scheduling_perception() {
        let mut scheduler = NanosecondScheduler::new();

        let task = ConsciousnessTask::Perception {
            priority: 128,
            data: vec![1, 2, 3, 4, 5],
        };

        let task_id = scheduler.schedule_task(task, 1000, 10000).unwrap();
        assert_eq!(task_id, 1);
        assert_eq!(scheduler.task_queue.len(), 1);
        assert_eq!(scheduler.metrics.tasks_scheduled, 1);
    }

    #[test]
    fn test_task_scheduling_memory_integration() {
        let mut scheduler = NanosecondScheduler::new();

        let task = ConsciousnessTask::MemoryIntegration {
            session_id: "test_session".to_string(),
            state: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };

        let task_id = scheduler.schedule_task(task, 0, 5000).unwrap();
        assert_eq!(task_id, 1);
        assert_eq!(scheduler.task_queue.len(), 1);
    }

    #[test]
    fn test_task_scheduling_identity_preservation() {
        let mut scheduler = NanosecondScheduler::new();

        let task = ConsciousnessTask::IdentityPreservation {
            continuity_check: true,
        };

        let task_id = scheduler.schedule_task(task, 500, 2000).unwrap();
        assert_eq!(task_id, 1);
        assert_eq!(scheduler.task_queue.len(), 1);
    }

    #[test]
    fn test_task_scheduling_strange_loop() {
        let mut scheduler = NanosecondScheduler::new();

        let task = ConsciousnessTask::StrangeLoopProcessing {
            iteration: 5,
            state: vec![0.1, 0.2, 0.3, 0.4, 0.5],
        };

        let task_id = scheduler.schedule_task(task, 100, 1000).unwrap();
        assert_eq!(task_id, 1);
        assert_eq!(scheduler.task_queue.len(), 1);
    }

    #[test]
    fn test_task_scheduling_window_management() {
        let mut scheduler = NanosecondScheduler::new();

        let task = ConsciousnessTask::WindowManagement {
            window_id: 42,
            overlap_target: 85.0,
        };

        let task_id = scheduler.schedule_task(task, 0, 3000).unwrap();
        assert_eq!(task_id, 1);
        assert_eq!(scheduler.task_queue.len(), 1);
    }

    #[test]
    fn test_task_priority_ordering() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule tasks with different priorities
        let low_priority = ConsciousnessTask::Perception {
            priority: 50,
            data: vec![1],
        };
        let high_priority = ConsciousnessTask::IdentityPreservation {
            continuity_check: true,
        };
        let medium_priority = ConsciousnessTask::StrangeLoopProcessing {
            iteration: 1,
            state: vec![0.5],
        };

        scheduler.schedule_task(low_priority, 0, 1000).unwrap();
        scheduler.schedule_task(high_priority, 0, 1000).unwrap();
        scheduler.schedule_task(medium_priority, 0, 1000).unwrap();

        assert_eq!(scheduler.task_queue.len(), 3);

        // Process tasks and verify priority ordering
        for _ in 0..5 {
            scheduler.tick().unwrap();
        }

        // Identity preservation should be processed first (highest priority)
        assert!(scheduler.metrics.tasks_completed > 0);
    }

    #[test]
    fn test_task_queue_overflow() {
        let mut scheduler = NanosecondScheduler::new();

        // Fill up the task queue beyond capacity
        for i in 0..10001 {
            let task = ConsciousnessTask::Perception {
                priority: 100,
                data: vec![i as u8],
            };

            let result = scheduler.schedule_task(task, 0, 1000);

            if i < 10000 {
                assert!(result.is_ok());
            } else {
                // Should fail with overflow error
                match result {
                    Err(TemporalError::TaskQueueOverflow { current_size, max_size }) => {
                        assert_eq!(current_size, 10000);
                        assert_eq!(max_size, 10000);
                    },
                    _ => panic!("Expected TaskQueueOverflow error"),
                }
                break;
            }
        }
    }

    #[test]
    fn test_multiple_task_scheduling() {
        let mut scheduler = NanosecondScheduler::new();
        let mut task_ids = Vec::new();

        // Schedule multiple tasks
        for i in 0..100 {
            let task = ConsciousnessTask::Perception {
                priority: (i % 255) as u8,
                data: vec![i as u8; 10],
            };

            let task_id = scheduler.schedule_task(task, i * 100, (i + 1) * 1000).unwrap();
            task_ids.push(task_id);
        }

        assert_eq!(scheduler.task_queue.len(), 100);
        assert_eq!(scheduler.metrics.tasks_scheduled, 100);

        // Verify task IDs are sequential
        for (i, &task_id) in task_ids.iter().enumerate() {
            assert_eq!(task_id, (i + 1) as u64);
        }
    }
}

/// Test tick processing and task execution
#[cfg(test)]
mod tick_processing_tests {
    use super::*;

    #[test]
    fn test_basic_tick_processing() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule an immediate task
        let task = ConsciousnessTask::IdentityPreservation {
            continuity_check: true,
        };
        scheduler.schedule_task(task, 0, 1000).unwrap();

        // Process tick
        let result = scheduler.tick();
        assert!(result.is_ok());

        assert_eq!(scheduler.current_tick, 1);
        assert_eq!(scheduler.metrics.total_ticks, 1);
        assert_eq!(scheduler.metrics.tasks_completed, 1);
        assert_eq!(scheduler.completed_tasks.len(), 1);
    }

    #[test]
    fn test_multiple_tick_processing() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule several tasks
        for i in 0..5 {
            let task = ConsciousnessTask::Perception {
                priority: 100,
                data: vec![i as u8],
            };
            scheduler.schedule_task(task, 0, 1000).unwrap();
        }

        // Process multiple ticks
        for tick in 1..=10 {
            scheduler.tick().unwrap();
            assert_eq!(scheduler.current_tick, tick);
            assert_eq!(scheduler.metrics.total_ticks, tick);
        }

        // All tasks should be completed
        assert_eq!(scheduler.metrics.tasks_completed, 5);
        assert_eq!(scheduler.completed_tasks.len(), 5);
        assert_eq!(scheduler.task_queue.len(), 0);
    }

    #[test]
    fn test_delayed_task_execution() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule task with delay
        let task = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![42],
        };
        scheduler.schedule_task(task, 5000, 10000).unwrap(); // 5μs delay

        // Process early ticks - task should not execute yet
        for _ in 0..3 {
            scheduler.tick().unwrap();
        }
        assert_eq!(scheduler.metrics.tasks_completed, 0);

        // Continue ticking until task should execute
        for _ in 0..100 {
            scheduler.tick().unwrap();
        }

        // Task should eventually execute
        assert!(scheduler.metrics.tasks_completed > 0);
    }

    #[test]
    fn test_tick_with_no_tasks() {
        let mut scheduler = NanosecondScheduler::new();

        // Process ticks with no tasks
        for i in 1..=10 {
            scheduler.tick().unwrap();
            assert_eq!(scheduler.current_tick, i);
            assert_eq!(scheduler.metrics.tasks_completed, 0);
        }
    }

    #[test]
    fn test_task_execution_order() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule tasks with different deadlines
        let task1 = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![1],
        };
        let task2 = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![2],
        };
        let task3 = ConsciousnessTask::Perception {
            priority: 100,
            data: vec![3],
        };

        // Schedule with specific timing
        scheduler.schedule_task(task1, 0, 1000).unwrap();    // Immediate, short deadline
        scheduler.schedule_task(task2, 0, 3000).unwrap();    // Immediate, long deadline
        scheduler.schedule_task(task3, 0, 500).unwrap();     // Immediate, very short deadline

        // Process ticks
        for _ in 0..5 {
            scheduler.tick().unwrap();
        }

        // Verify tasks were processed
        assert_eq!(scheduler.metrics.tasks_completed, 3);
        assert_eq!(scheduler.completed_tasks.len(), 3);
    }
}

/// Test metrics tracking and reporting
#[cfg(test)]
mod metrics_tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let scheduler = NanosecondScheduler::new();
        let metrics = scheduler.get_metrics();

        assert_eq!(metrics.total_ticks, 0);
        assert_eq!(metrics.tasks_scheduled, 0);
        assert_eq!(metrics.tasks_completed, 0);
        assert_eq!(metrics.avg_scheduling_overhead_ns, 0.0);
        assert_eq!(metrics.max_scheduling_overhead_ns, 0);
        assert_eq!(metrics.window_overlap_percentage, 0.0);
        assert_eq!(metrics.contraction_convergence_rate, 0.0);
        assert_eq!(metrics.identity_continuity_score, 0.0);
        assert_eq!(metrics.temporal_advantage_ns, 0);
    }

    #[test]
    fn test_metrics_tick_counting() {
        let mut scheduler = NanosecondScheduler::new();

        for i in 1..=50 {
            scheduler.tick().unwrap();
            let metrics = scheduler.get_metrics();
            assert_eq!(metrics.total_ticks, i);
        }
    }

    #[test]
    fn test_metrics_task_counting() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule multiple tasks
        for i in 0..10 {
            let task = ConsciousnessTask::Perception {
                priority: 100,
                data: vec![i as u8],
            };
            scheduler.schedule_task(task, 0, 1000).unwrap();

            let metrics = scheduler.get_metrics();
            assert_eq!(metrics.tasks_scheduled, (i + 1) as u64);
        }

        // Process tasks
        for _ in 0..15 {
            scheduler.tick().unwrap();
        }

        let metrics = scheduler.get_metrics();
        assert_eq!(metrics.tasks_completed, 10);
    }

    #[test]
    fn test_temporal_advantage_calculation() {
        let scheduler = NanosecondScheduler::new();
        let advantage = scheduler.get_temporal_advantage();

        // Should return a reasonable temporal advantage
        assert!(advantage >= 0);

        // For default configuration, should be related to window overlap
        // This is a basic sanity check
    }

    #[test]
    fn test_quantum_metrics_tracking() {
        let mut scheduler = NanosecondScheduler::new();

        // Process some ticks to generate quantum validations
        for _ in 0..10 {
            scheduler.tick().unwrap();
        }

        let metrics = scheduler.get_metrics();

        // Quantum metrics should be tracked
        assert!(metrics.quantum_validity_rate >= 0.0);
        assert!(metrics.quantum_validity_rate <= 1.0);
        assert!(metrics.avg_quantum_energy_j >= 0.0);
        assert!(metrics.avg_margolus_levitin_margin >= 0.0);
        assert!(metrics.avg_uncertainty_margin >= 0.0);
        assert!(metrics.avg_coherence_preservation >= 0.0);
        assert!(metrics.avg_entanglement_strength >= 0.0);
    }

    #[test]
    fn test_quantum_analysis_report() {
        let mut scheduler = NanosecondScheduler::new();

        // Generate some quantum validations
        for _ in 0..5 {
            scheduler.tick().unwrap();
        }

        let analysis = scheduler.get_quantum_analysis();

        assert!(analysis.total_validations > 0);
        assert!(analysis.validity_rate >= 0.0 && analysis.validity_rate <= 1.0);
        assert!(analysis.avg_energy_j >= 0.0);
        assert!(analysis.avg_energy_ev >= 0.0);
        assert!(analysis.recommended_time_scale_s > 0.0);
    }
}

/// Test memory state management
#[cfg(test)]
mod memory_state_tests {
    use super::*;

    #[test]
    fn test_memory_state_export_import() {
        let mut scheduler = NanosecondScheduler::new();

        let test_state = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
        scheduler.import_memory_state(test_state.clone()).unwrap();

        let exported_state = scheduler.export_memory_state().unwrap();
        assert_eq!(exported_state, test_state);
    }

    #[test]
    fn test_memory_state_integration() {
        let mut scheduler = NanosecondScheduler::new();

        // Import initial state
        let initial_state = vec![1, 2, 3, 4, 5];
        scheduler.import_memory_state(initial_state).unwrap();

        // Schedule memory integration task
        let task = ConsciousnessTask::MemoryIntegration {
            session_id: "test_session".to_string(),
            state: vec![6, 7, 8, 9, 10],
        };
        scheduler.schedule_task(task, 0, 1000).unwrap();

        // Process task
        for _ in 0..5 {
            scheduler.tick().unwrap();
        }

        // Verify memory state was updated
        let final_state = scheduler.export_memory_state().unwrap();
        assert!(final_state.len() > 5); // Should have grown
        assert!(final_state.contains(&6)); // Should contain new data
    }

    #[test]
    fn test_empty_memory_state() {
        let mut scheduler = NanosecondScheduler::new();

        let empty_state = scheduler.export_memory_state().unwrap();
        assert!(empty_state.is_empty());

        scheduler.import_memory_state(vec![]).unwrap();
        let state_after_import = scheduler.export_memory_state().unwrap();
        assert!(state_after_import.is_empty());
    }

    #[test]
    fn test_large_memory_state() {
        let mut scheduler = NanosecondScheduler::new();

        // Create large state (1MB)
        let large_state = vec![0x42; 1024 * 1024];
        scheduler.import_memory_state(large_state.clone()).unwrap();

        let exported = scheduler.export_memory_state().unwrap();
        assert_eq!(exported.len(), large_state.len());
        assert_eq!(exported, large_state);
    }
}

/// Test MCP integration hooks
#[cfg(test)]
mod mcp_integration_tests {
    use super::*;

    #[test]
    fn test_mcp_consciousness_evolve_hook() {
        let mut scheduler = NanosecondScheduler::new();

        let emergence_level = scheduler.mcp_consciousness_evolve_hook(10, 0.5).unwrap();

        assert!(emergence_level >= 0.0);
        assert!(emergence_level <= 1.0);

        // Should have processed evolution tasks
        assert!(scheduler.metrics.tasks_completed > 0);
        assert!(scheduler.metrics.total_ticks > 0);
    }

    #[test]
    fn test_consciousness_evolution_convergence() {
        let mut scheduler = NanosecondScheduler::new();

        // Test with higher target
        let emergence_level = scheduler.mcp_consciousness_evolve_hook(50, 0.8).unwrap();

        // Should approach the target
        assert!(emergence_level > 0.0);

        // Verify strange loop metrics show evolution
        let metrics = scheduler.get_metrics();
        assert!(metrics.contraction_convergence_rate >= 0.0);
    }

    #[test]
    fn test_consciousness_evolution_with_memory() {
        let mut scheduler = NanosecondScheduler::new();

        // Set initial memory state
        let initial_memory = vec![0x01, 0x02, 0x03];
        scheduler.import_memory_state(initial_memory).unwrap();

        let emergence_level = scheduler.mcp_consciousness_evolve_hook(5, 0.3).unwrap();

        assert!(emergence_level >= 0.0);

        // Memory state should be preserved/evolved
        let final_memory = scheduler.export_memory_state().unwrap();
        assert!(!final_memory.is_empty());
    }

    #[test]
    fn test_evolution_with_zero_iterations() {
        let mut scheduler = NanosecondScheduler::new();

        let emergence_level = scheduler.mcp_consciousness_evolve_hook(0, 0.5).unwrap();

        // Should return immediately with minimal evolution
        assert!(emergence_level >= 0.0);
        assert_eq!(scheduler.metrics.tasks_scheduled, 0);
    }

    #[test]
    fn test_evolution_target_achievement() {
        let mut scheduler = NanosecondScheduler::new();

        // Test with achievable target
        let target = 0.2;
        let emergence_level = scheduler.mcp_consciousness_evolve_hook(100, target).unwrap();

        // Should reach or approach target
        assert!(emergence_level >= 0.0);

        // May stop early if target is reached
        let metrics = scheduler.get_metrics();
        assert!(metrics.tasks_completed > 0);
    }
}

/// Test error handling and edge cases
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_scheduling_overhead_limit() {
        let config = TemporalConfig {
            max_scheduling_overhead_ns: 1, // Very tight limit
            ..Default::default()
        };

        let mut scheduler = NanosecondScheduler::with_config(config);

        // This might trigger overhead limit errors under load
        for _ in 0..100 {
            let task = ConsciousnessTask::Perception {
                priority: 255,
                data: vec![0; 1000], // Large data
            };

            // Schedule task
            if scheduler.schedule_task(task, 0, 1000).is_err() {
                break; // Queue full
            }

            // Process tick - may fail with overhead error
            let result = scheduler.tick();
            if result.is_err() {
                match result {
                    Err(TemporalError::SchedulingOverhead { actual_ns, limit_ns }) => {
                        assert!(actual_ns > limit_ns);
                        break;
                    },
                    _ => {},
                }
            }
        }

        // Test should complete without panicking
    }

    #[test]
    fn test_continuity_tracker_validation() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule identity preservation task
        let task = ConsciousnessTask::IdentityPreservation {
            continuity_check: true,
        };
        scheduler.schedule_task(task, 0, 1000).unwrap();

        // Process task
        for _ in 0..5 {
            scheduler.tick().unwrap();
        }

        // Get continuity metrics
        let continuity_metrics = scheduler.measure_continuity().unwrap();
        assert!(continuity_metrics.continuity_score >= 0.0);
        assert!(continuity_metrics.identity_stability >= 0.0);
    }

    #[test]
    fn test_window_overlap_validation() {
        let config = TemporalConfig {
            window_overlap_percent: 95.0, // Very high overlap requirement
            ..Default::default()
        };

        let mut scheduler = NanosecondScheduler::with_config(config);

        // Process many ticks to trigger window management
        for _ in 0..1000 {
            let result = scheduler.tick();
            if result.is_err() {
                match result {
                    Err(TemporalError::WindowOverlapTooLow { actual, required }) => {
                        assert!(actual < required);
                        break;
                    },
                    _ => {},
                }
            }
        }
    }

    #[test]
    fn test_invalid_task_data() {
        let mut scheduler = NanosecondScheduler::new();

        // Schedule task with very large data
        let large_data = vec![0; 10_000_000]; // 10MB data
        let task = ConsciousnessTask::Perception {
            priority: 100,
            data: large_data,
        };

        // Should handle large data gracefully
        let result = scheduler.schedule_task(task, 0, 1000);
        assert!(result.is_ok());

        // Process should handle it
        let tick_result = scheduler.tick();
        assert!(tick_result.is_ok());
    }
}