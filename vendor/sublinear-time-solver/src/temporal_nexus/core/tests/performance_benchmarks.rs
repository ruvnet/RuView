//! Performance benchmarks for NanosecondScheduler
//!
//! This module provides comprehensive performance benchmarks to validate
//! the <1μs overhead target and measure real-world performance characteristics.

use super::*;

/// Benchmark scheduler tick performance
#[cfg(test)]
mod tick_performance_benchmarks {
    use super::*;

    #[test]
    fn test_bare_tick_performance() {
        let mut scheduler = NanosecondScheduler::new();
        let iterations = 10000;
        let mut timing_samples = Vec::with_capacity(iterations);

        // Warm up
        for _ in 0..100 {
            scheduler.tick().unwrap();
        }

        // Benchmark bare tick performance
        for _ in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                scheduler.tick().unwrap()
            });
            timing_samples.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&timing_samples);

        println!("Bare tick performance benchmark:");
        println!("  Iterations: {}", iterations);
        println!("  Min: {} ns", metrics.min_execution_time_ns);
        println!("  Average: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Median: {} ns", metrics.median_execution_time_ns);
        println!("  Max: {} ns", metrics.max_execution_time_ns);
        println!("  Std deviation: {:.1} ns", metrics.std_deviation_ns);
        println!("  Operations/sec: {:.0}", metrics.operations_per_second);

        // Verify <1μs target
        assert!(metrics.avg_execution_time_ns < 1000.0,
            "Average tick time exceeds 1μs target: {:.1}ns", metrics.avg_execution_time_ns);

        // 95th percentile should also be reasonable
        let mut sorted = timing_samples.clone();
        sorted.sort_unstable();
        let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];

        println!("  95th percentile: {} ns", p95);
        assert!(p95 < 1500, "95th percentile exceeds 1.5μs: {}ns", p95);

        // Should achieve high throughput
        assert!(metrics.operations_per_second > 500_000.0,
            "Throughput too low: {:.0} ops/sec", metrics.operations_per_second);
    }

    #[test]
    fn test_tick_performance_with_tasks() {
        let mut scheduler = NanosecondScheduler::new();

        // Pre-schedule various tasks
        let task_types = [
            ConsciousnessTask::Perception { priority: 100, data: vec![1; 50] },
            ConsciousnessTask::MemoryIntegration { session_id: "bench".to_string(), state: vec![1; 20] },
            ConsciousnessTask::IdentityPreservation { continuity_check: true },
            ConsciousnessTask::StrangeLoopProcessing { iteration: 5, state: vec![1.0; 10] },
            ConsciousnessTask::WindowManagement { window_id: 1, overlap_target: 75.0 },
        ];

        for (i, task) in task_types.iter().enumerate() {
            for j in 0..20 {
                scheduler.schedule_task(task.clone(), j * 100, (j + 1) * 1000).unwrap();
            }
        }

        let iterations = 5000;
        let mut timing_samples = Vec::with_capacity(iterations);

        // Benchmark ticks with task processing
        for _ in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                scheduler.tick().unwrap()
            });
            timing_samples.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&timing_samples);

        println!("Tick performance with tasks:");
        println!("  Tasks completed: {}", scheduler.metrics.tasks_completed);
        println!("  Average tick time: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Median tick time: {} ns", metrics.median_execution_time_ns);
        println!("  Max tick time: {} ns", metrics.max_execution_time_ns);

        // Should still be reasonable with task processing
        assert!(metrics.avg_execution_time_ns < 2000.0,
            "Tick time with tasks too high: {:.1}ns", metrics.avg_execution_time_ns);
    }

    #[test]
    fn test_tick_performance_under_load() {
        let mut scheduler = NanosecondScheduler::new();

        // Create heavy load - fill task queue
        for i in 0..5000 {
            let task = ConsciousnessTask::StrangeLoopProcessing {
                iteration: i % 100,
                state: vec![1.0; 20],
            };
            scheduler.schedule_task(task, 0, 10000).unwrap();
        }

        let iterations = 1000;
        let mut timing_samples = Vec::with_capacity(iterations);

        // Benchmark under heavy load
        for _ in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                scheduler.tick().unwrap()
            });
            timing_samples.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&timing_samples);

        println!("Tick performance under heavy load:");
        println!("  Queue size at start: 5000");
        println!("  Average tick time: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Max tick time: {} ns", metrics.max_execution_time_ns);
        println!("  Tasks processed: {}", scheduler.metrics.tasks_completed);

        // Performance may degrade under load but should remain functional
        assert!(metrics.avg_execution_time_ns < 5000.0,
            "Performance too poor under load: {:.1}ns", metrics.avg_execution_time_ns);
    }

    #[test]
    fn test_scheduling_overhead_benchmark() {
        let mut scheduler = NanosecondScheduler::new();
        let iterations = 10000;

        // Benchmark task scheduling overhead
        let mut scheduling_times = Vec::with_capacity(iterations);

        for i in 0..iterations {
            let task = ConsciousnessTask::Perception {
                priority: (i % 255) as u8,
                data: vec![i as u8; 10],
            };

            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                scheduler.schedule_task(task, i as u64 * 100, (i as u64 + 1) * 1000).unwrap()
            });

            scheduling_times.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&scheduling_times);

        println!("Task scheduling performance:");
        println!("  Tasks scheduled: {}", iterations);
        println!("  Average scheduling time: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Median scheduling time: {} ns", metrics.median_execution_time_ns);
        println!("  Max scheduling time: {} ns", metrics.max_execution_time_ns);
        println!("  Scheduling ops/sec: {:.0}", metrics.operations_per_second);

        // Scheduling should be very fast
        assert!(metrics.avg_execution_time_ns < 500.0,
            "Task scheduling too slow: {:.1}ns", metrics.avg_execution_time_ns);

        assert!(metrics.operations_per_second > 1_000_000.0,
            "Scheduling throughput too low: {:.0} ops/sec", metrics.operations_per_second);
    }
}

/// Benchmark component performance
#[cfg(test)]
mod component_performance_benchmarks {
    use super::*;

    #[test]
    fn test_strange_loop_performance() {
        let mut operator = StrangeLoopOperator::new(0.9, 100);
        let state = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let iterations = 10000;

        let mut timing_samples = Vec::with_capacity(iterations);

        // Benchmark strange loop processing
        for i in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                operator.process_iteration(i as f64, &state).unwrap()
            });
            timing_samples.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&timing_samples);

        println!("Strange loop performance:");
        println!("  Iterations: {}", iterations);
        println!("  Average time: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Median time: {} ns", metrics.median_execution_time_ns);
        println!("  Operations/sec: {:.0}", metrics.operations_per_second);

        // Should be efficient for real-time use
        assert!(metrics.avg_execution_time_ns < 1000.0,
            "Strange loop processing too slow: {:.1}ns", metrics.avg_execution_time_ns);
    }

    #[test]
    fn test_window_manager_performance() {
        let mut manager = WindowOverlapManager::new(75.0);
        let iterations = 10000;

        let mut timing_samples = Vec::with_capacity(iterations);

        // Benchmark window advancement
        for tick in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                manager.advance_window(tick as u64).unwrap()
            });
            timing_samples.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&timing_samples);

        println!("Window manager performance:");
        println!("  Ticks processed: {}", iterations);
        println!("  Average time: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Operations/sec: {:.0}", metrics.operations_per_second);
        println!("  Windows created: {}", manager.get_metrics().total_windows);

        // Should be efficient
        assert!(metrics.avg_execution_time_ns < 1000.0,
            "Window management too slow: {:.1}ns", metrics.avg_execution_time_ns);
    }

    #[test]
    fn test_identity_tracker_performance() {
        let mut tracker = IdentityContinuityTracker::new();
        let iterations = 5000;
        let tsc_freq = TestConfig::detect_tsc_frequency();

        let mut timing_samples = Vec::with_capacity(iterations);

        // Benchmark identity tracking
        for i in 0..iterations {
            let timestamp = TscTimestamp::now().add_nanos(i * 1000, tsc_freq);
            let state = TestUtils::generate_test_data(50, TestDataPattern::Random);

            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                tracker.track_continuity(timestamp, &state).unwrap()
            });
            timing_samples.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&timing_samples);

        println!("Identity tracker performance:");
        println!("  Snapshots tracked: {}", iterations);
        println!("  Average time: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Operations/sec: {:.0}", metrics.operations_per_second);

        // Should be efficient for continuous tracking
        assert!(metrics.avg_execution_time_ns < 2000.0,
            "Identity tracking too slow: {:.1}ns", metrics.avg_execution_time_ns);
    }

    #[test]
    fn test_tsc_timestamp_performance() {
        let iterations = 100000;

        // Benchmark TSC read performance
        let mut read_times = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                std::hint::black_box(TscTimestamp::now())
            });
            read_times.push(duration_ns);
        }

        let read_metrics = PerformanceMetrics::from_samples(&read_times);

        // Benchmark timestamp arithmetic
        let ts1 = TscTimestamp::now();
        let ts2 = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        let mut calc_times = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                std::hint::black_box(ts2.nanos_since(ts1, tsc_freq))
            });
            calc_times.push(duration_ns);
        }

        let calc_metrics = PerformanceMetrics::from_samples(&calc_times);

        println!("TSC timestamp performance:");
        println!("  TSC read time: {:.1} ns", read_metrics.avg_execution_time_ns);
        println!("  Calculation time: {:.1} ns", calc_metrics.avg_execution_time_ns);
        println!("  Read ops/sec: {:.0}", read_metrics.operations_per_second);
        println!("  Calc ops/sec: {:.0}", calc_metrics.operations_per_second);

        // Should be extremely fast
        assert!(read_metrics.avg_execution_time_ns < 100.0,
            "TSC read too slow: {:.1}ns", read_metrics.avg_execution_time_ns);
        assert!(calc_metrics.avg_execution_time_ns < 50.0,
            "TSC calculation too slow: {:.1}ns", calc_metrics.avg_execution_time_ns);
    }
}

/// Benchmark throughput and scalability
#[cfg(test)]
mod throughput_benchmarks {
    use super::*;

    #[test]
    fn test_sustained_throughput() {
        let mut scheduler = NanosecondScheduler::new();
        let duration_ms = 1000; // 1 second test
        let start_time = std::time::Instant::now();

        let mut tick_count = 0;
        let mut task_count = 0;

        // Sustained operation for 1 second
        while start_time.elapsed().as_millis() < duration_ms {
            // Schedule tasks periodically
            if tick_count % 10 == 0 {
                let task = ConsciousnessTask::Perception {
                    priority: 100,
                    data: vec![(tick_count % 256) as u8; 20],
                };
                scheduler.schedule_task(task, 0, 10000).unwrap();
                task_count += 1;
            }

            scheduler.tick().unwrap();
            tick_count += 1;
        }

        let actual_duration = start_time.elapsed();
        let ticks_per_second = tick_count as f64 / actual_duration.as_secs_f64();
        let tasks_per_second = scheduler.metrics.tasks_completed as f64 / actual_duration.as_secs_f64();

        println!("Sustained throughput ({}ms):", actual_duration.as_millis());
        println!("  Total ticks: {}", tick_count);
        println!("  Tasks scheduled: {}", task_count);
        println!("  Tasks completed: {}", scheduler.metrics.tasks_completed);
        println!("  Ticks/second: {:.0}", ticks_per_second);
        println!("  Tasks/second: {:.0}", tasks_per_second);

        // Should maintain high sustained throughput
        assert!(ticks_per_second > 100_000.0,
            "Sustained tick rate too low: {:.0} ticks/sec", ticks_per_second);

        assert!(tasks_per_second > 5_000.0,
            "Sustained task rate too low: {:.0} tasks/sec", tasks_per_second);
    }

    #[test]
    fn test_burst_performance() {
        let mut scheduler = NanosecondScheduler::new();

        // Burst test: schedule many tasks quickly, then process
        let burst_size = 1000;
        let burst_start = std::time::Instant::now();

        for i in 0..burst_size {
            let task = ConsciousnessTask::StrangeLoopProcessing {
                iteration: i,
                state: vec![i as f64; 5],
            };
            scheduler.schedule_task(task, 0, 10000).unwrap();
        }

        let scheduling_time = burst_start.elapsed();

        // Process burst
        let processing_start = std::time::Instant::now();
        let mut ticks_needed = 0;

        while scheduler.metrics.tasks_completed < burst_size as u64 {
            scheduler.tick().unwrap();
            ticks_needed += 1;

            // Safety limit
            if ticks_needed > burst_size * 2 {
                break;
            }
        }

        let processing_time = processing_start.elapsed();

        println!("Burst performance ({} tasks):", burst_size);
        println!("  Scheduling time: {:.2}ms", scheduling_time.as_millis());
        println!("  Processing time: {:.2}ms", processing_time.as_millis());
        println!("  Ticks needed: {}", ticks_needed);
        println!("  Tasks completed: {}", scheduler.metrics.tasks_completed);

        let scheduling_rate = burst_size as f64 / scheduling_time.as_secs_f64();
        let processing_rate = scheduler.metrics.tasks_completed as f64 / processing_time.as_secs_f64();

        println!("  Scheduling rate: {:.0} tasks/sec", scheduling_rate);
        println!("  Processing rate: {:.0} tasks/sec", processing_rate);

        // Should handle bursts efficiently
        assert!(scheduling_rate > 100_000.0,
            "Burst scheduling too slow: {:.0} tasks/sec", scheduling_rate);
        assert!(processing_rate > 10_000.0,
            "Burst processing too slow: {:.0} tasks/sec", processing_rate);
    }

    #[test]
    fn test_memory_performance() {
        let mut scheduler = NanosecondScheduler::new();

        // Test with large memory operations
        let large_state = vec![0x42; 1024 * 1024]; // 1MB state

        let iterations = 100;
        let mut timing_samples = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let (_, duration_ns) = TestUtils::measure_execution_time(|| {
                scheduler.import_memory_state(large_state.clone()).unwrap();
                let _exported = scheduler.export_memory_state().unwrap();
            });
            timing_samples.push(duration_ns);
        }

        let metrics = PerformanceMetrics::from_samples(&timing_samples);

        println!("Memory performance (1MB operations):");
        println!("  Average time: {:.1} ns", metrics.avg_execution_time_ns);
        println!("  Operations/sec: {:.0}", metrics.operations_per_second);

        // Should handle large memory operations efficiently
        assert!(metrics.avg_execution_time_ns < 1_000_000.0,
            "Memory operations too slow: {:.1}ns", metrics.avg_execution_time_ns);
    }

    #[test]
    fn test_concurrent_access_simulation() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let scheduler = Arc::new(Mutex::new(NanosecondScheduler::new()));
        let num_threads = 4;
        let operations_per_thread = 1000;

        // Simulate concurrent access patterns
        let handles: Vec<_> = (0..num_threads).map(|thread_id| {
            let scheduler_clone = scheduler.clone();

            thread::spawn(move || {
                let mut local_times = Vec::new();

                for i in 0..operations_per_thread {
                    let start = std::time::Instant::now();

                    // Different threads do different operations
                    {
                        let mut sched = scheduler_clone.lock().unwrap();

                        match thread_id % 3 {
                            0 => {
                                // Schedule tasks
                                let task = ConsciousnessTask::Perception {
                                    priority: 100,
                                    data: vec![i as u8; 10],
                                };
                                let _ = sched.schedule_task(task, 0, 1000);
                            },
                            1 => {
                                // Process ticks
                                let _ = sched.tick();
                            },
                            2 => {
                                // Read metrics
                                let _ = sched.get_metrics();
                                let _ = sched.get_temporal_advantage();
                            },
                            _ => unreachable!(),
                        }
                    }

                    let duration = start.elapsed();
                    local_times.push(duration.as_nanos() as u64);
                }

                local_times
            })
        }).collect();

        // Collect results
        let start_time = std::time::Instant::now();
        let mut all_times = Vec::new();

        for handle in handles {
            let times = handle.join().unwrap();
            all_times.extend(times);
        }

        let total_time = start_time.elapsed();
        let metrics = PerformanceMetrics::from_samples(&all_times);

        println!("Concurrent access simulation:");
        println!("  Threads: {}", num_threads);
        println!("  Operations per thread: {}", operations_per_thread);
        println!("  Total time: {:.2}ms", total_time.as_millis());
        println!("  Average operation time: {:.1} ns", metrics.avg_execution_time_ns);

        // Should handle concurrent access reasonably
        assert!(metrics.avg_execution_time_ns < 100_000.0,
            "Concurrent operations too slow: {:.1}ns", metrics.avg_execution_time_ns);
    }
}

/// Benchmark specific performance targets
#[cfg(test)]
mod target_validation_tests {
    use super::*;

    #[test]
    fn test_one_microsecond_target() {
        let mut scheduler = NanosecondScheduler::new();

        // Test various scenarios against 1μs target
        let test_scenarios = [
            ("bare_tick", || scheduler.tick().unwrap()),
            ("simple_perception", || {
                let task = ConsciousnessTask::Perception {
                    priority: 100,
                    data: vec![1, 2, 3],
                };
                scheduler.schedule_task(task, 0, 1000).unwrap();
                scheduler.tick().unwrap();
            }),
            ("identity_check", || {
                let task = ConsciousnessTask::IdentityPreservation {
                    continuity_check: true,
                };
                scheduler.schedule_task(task, 0, 1000).unwrap();
                scheduler.tick().unwrap();
            }),
            ("memory_integration", || {
                let task = ConsciousnessTask::MemoryIntegration {
                    session_id: "test".to_string(),
                    state: vec![1, 2, 3, 4, 5],
                };
                scheduler.schedule_task(task, 0, 1000).unwrap();
                scheduler.tick().unwrap();
            }),
        ];

        for (scenario_name, operation) in test_scenarios.iter() {
            let iterations = 1000;
            let mut times = Vec::with_capacity(iterations);

            for _ in 0..iterations {
                let (_, duration_ns) = TestUtils::measure_execution_time(operation);
                times.push(duration_ns);
            }

            let metrics = PerformanceMetrics::from_samples(&times);

            println!("1μs target validation - {}:", scenario_name);
            println!("  Average: {:.1} ns", metrics.avg_execution_time_ns);
            println!("  95th percentile: {} ns", times[(times.len() as f64 * 0.95) as usize]);

            // Verify meets <1μs target
            assert!(metrics.avg_execution_time_ns < 1000.0,
                "{} exceeds 1μs target: {:.1}ns", scenario_name, metrics.avg_execution_time_ns);
        }
    }

    #[test]
    fn test_nanosecond_precision_target() {
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Test timing precision at various scales
        let test_durations = [100, 500, 1000, 5000, 10000]; // nanoseconds

        for &target_ns in &test_durations {
            let mut actual_times = Vec::new();

            for _ in 0..50 {
                let start = TscTimestamp::now();

                // Busy wait for target duration
                let target_cycles = (target_ns * tsc_freq) / 1_000_000_000;
                let end_target = start.0 + target_cycles;

                while TscTimestamp::now().0 < end_target {
                    std::hint::black_box(());
                }

                let end = TscTimestamp::now();
                let actual_ns = end.nanos_since(start, tsc_freq);
                actual_times.push(actual_ns);
            }

            let avg_actual = actual_times.iter().sum::<u64>() as f64 / actual_times.len() as f64;
            let precision_error = ((avg_actual - target_ns as f64).abs() / target_ns as f64) * 100.0;

            println!("Precision target {}ns: avg={:.1}ns, error={:.1}%",
                    target_ns, avg_actual, precision_error);

            // Should achieve reasonable precision
            if target_ns >= 1000 {
                assert!(precision_error < 10.0,
                    "Precision error too high for {}ns: {:.1}%", target_ns, precision_error);
            }
        }
    }

    #[test]
    fn test_throughput_targets() {
        let mut scheduler = NanosecondScheduler::new();

        // Target: >1M ticks/second
        let duration = std::time::Duration::from_millis(100);
        let start_time = std::time::Instant::now();
        let mut tick_count = 0;

        while start_time.elapsed() < duration {
            scheduler.tick().unwrap();
            tick_count += 1;
        }

        let actual_duration = start_time.elapsed();
        let ticks_per_second = tick_count as f64 / actual_duration.as_secs_f64();

        println!("Throughput validation:");
        println!("  Ticks in {}ms: {}", actual_duration.as_millis(), tick_count);
        println!("  Ticks/second: {:.0}", ticks_per_second);

        assert!(ticks_per_second > 1_000_000.0,
            "Failed to achieve 1M ticks/sec target: {:.0}", ticks_per_second);

        // Target: >100k tasks/second (with task processing)
        scheduler = NanosecondScheduler::new();

        // Schedule many small tasks
        for i in 0..1000 {
            let task = ConsciousnessTask::Perception {
                priority: 100,
                data: vec![i as u8],
            };
            scheduler.schedule_task(task, 0, 10000).unwrap();
        }

        let start_time = std::time::Instant::now();
        let initial_completed = scheduler.metrics.tasks_completed;

        // Process for 100ms
        while start_time.elapsed().as_millis() < 100 {
            scheduler.tick().unwrap();
        }

        let tasks_processed = scheduler.metrics.tasks_completed - initial_completed;
        let tasks_per_second = tasks_processed as f64 / start_time.elapsed().as_secs_f64();

        println!("Task processing throughput:");
        println!("  Tasks processed: {}", tasks_processed);
        println!("  Tasks/second: {:.0}", tasks_per_second);

        assert!(tasks_per_second > 100_000.0,
            "Failed to achieve 100k tasks/sec target: {:.0}", tasks_per_second);
    }
}