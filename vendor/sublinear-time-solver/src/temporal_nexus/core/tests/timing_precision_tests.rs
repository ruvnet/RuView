//! High-precision timing tests using TSC (Time Stamp Counter)
//!
//! This module validates nanosecond precision timing with real hardware TSC,
//! not simulation. Tests verify timing accuracy, precision, and overhead.

use super::*;
use std::time::{Duration, Instant};

/// Test TSC timestamp accuracy and precision
#[cfg(test)]
mod tsc_precision_tests {
    use super::*;

    #[test]
    fn test_tsc_timestamp_now() {
        let ts1 = TscTimestamp::now();
        let ts2 = TscTimestamp::now();
        let ts3 = TscTimestamp::now();

        // Timestamps should be monotonically increasing
        assert!(ts2.0 >= ts1.0);
        assert!(ts3.0 >= ts2.0);

        // Should have some difference (CPU cycles passed)
        assert!(ts3.0 > ts1.0);
    }

    #[test]
    fn test_tsc_timestamp_ordering() {
        let mut timestamps = Vec::new();

        // Collect multiple timestamps
        for _ in 0..100 {
            timestamps.push(TscTimestamp::now());
            // Small delay to ensure timestamp progression
            for _ in 0..10 {
                std::hint::black_box(());
            }
        }

        // Verify monotonic ordering
        for window in timestamps.windows(2) {
            assert!(window[1] >= window[0], "TSC timestamps should be monotonic");
        }
    }

    #[test]
    fn test_nanos_since_calculation() {
        let tsc_freq = TestConfig::detect_tsc_frequency();
        println!("Detected TSC frequency: {:.2} GHz", tsc_freq as f64 / 1e9);

        let start = TscTimestamp::now();

        // Precise delay
        std::thread::sleep(Duration::from_nanos(1000));

        let end = TscTimestamp::now();
        let measured_ns = end.nanos_since(start, tsc_freq);

        println!("Expected: ~1000ns, Measured: {}ns", measured_ns);

        // Should be reasonably close to expected (within 10μs tolerance for thread scheduling)
        assert!(measured_ns >= 500, "Measured time too small: {}ns", measured_ns);
        assert!(measured_ns <= 50_000, "Measured time too large: {}ns", measured_ns);
    }

    #[test]
    fn test_add_nanos_precision() {
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let base = TscTimestamp::now();

        // Add specific nanosecond amounts
        let increments = [100, 1000, 10000, 100000];

        for &increment in &increments {
            let incremented = base.add_nanos(increment, tsc_freq);
            let calculated_diff = incremented.nanos_since(base, tsc_freq);

            println!("Added: {}ns, Calculated: {}ns", increment, calculated_diff);

            // Should be very close (within 1% or 10ns, whichever is larger)
            let tolerance = std::cmp::max(10, increment / 100);
            let diff = if calculated_diff > increment {
                calculated_diff - increment
            } else {
                increment - calculated_diff
            };

            assert!(diff <= tolerance,
                "Add/calculate precision error: expected {}ns, got {}ns, diff {}ns > tolerance {}ns",
                increment, calculated_diff, diff, tolerance);
        }
    }

    #[test]
    fn test_tsc_frequency_stability() {
        let mut frequencies = Vec::new();

        // Measure TSC frequency multiple times
        for _ in 0..5 {
            frequencies.push(TestConfig::detect_tsc_frequency());
            std::thread::sleep(Duration::from_millis(10));
        }

        // Should be stable (within 1%)
        let first_freq = frequencies[0];
        for &freq in &frequencies {
            let diff_percent = if freq > first_freq {
                ((freq - first_freq) as f64 / first_freq as f64) * 100.0
            } else {
                ((first_freq - freq) as f64 / first_freq as f64) * 100.0
            };

            assert!(diff_percent < 1.0,
                "TSC frequency not stable: {:.2}% difference", diff_percent);
        }

        println!("TSC frequency stable at: {:.2} GHz ± {:.2}%",
                first_freq as f64 / 1e9,
                frequencies.iter().map(|&f| ((f as f64 - first_freq as f64).abs() / first_freq as f64) * 100.0).fold(0.0, f64::max));
    }
}

/// Test timing precision under different workloads
#[cfg(test)]
mod precision_under_load_tests {
    use super::*;

    #[test]
    fn test_precision_with_cpu_load() {
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create CPU load in background
        let _load_handle = std::thread::spawn(|| {
            for _ in 0..1000000 {
                std::hint::black_box(fastrand::u64(..));
            }
        });

        let measurements = measure_timing_precision(tsc_freq, 100);

        // Even under load, precision should be maintained
        let avg_error = measurements.iter().sum::<f64>() / measurements.len() as f64;
        let max_error = measurements.iter().cloned().fold(0.0, f64::max);

        println!("Timing precision under CPU load:");
        println!("  Average error: {:.1}%", avg_error);
        println!("  Maximum error: {:.1}%", max_error);

        assert!(avg_error < 5.0, "Average timing error too high under load: {:.1}%", avg_error);
        assert!(max_error < 20.0, "Maximum timing error too high under load: {:.1}%", max_error);
    }

    #[test]
    fn test_precision_with_memory_pressure() {
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create memory pressure
        let _memory_pressure: Vec<Vec<u8>> = (0..100)
            .map(|_| vec![0; 1024 * 1024]) // 1MB each
            .collect();

        let measurements = measure_timing_precision(tsc_freq, 50);

        let avg_error = measurements.iter().sum::<f64>() / measurements.len() as f64;
        println!("Timing precision under memory pressure: {:.1}% avg error", avg_error);

        assert!(avg_error < 10.0, "Timing precision degraded under memory pressure");
    }

    #[test]
    fn test_precision_with_scheduler_interference() {
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create multiple threads to interfere with scheduling
        let handles: Vec<_> = (0..4).map(|_| {
            std::thread::spawn(|| {
                for _ in 0..100000 {
                    std::thread::yield_now();
                }
            })
        }).collect();

        let measurements = measure_timing_precision(tsc_freq, 50);

        // Wait for background threads
        for handle in handles {
            handle.join().unwrap();
        }

        let avg_error = measurements.iter().sum::<f64>() / measurements.len() as f64;
        println!("Timing precision with scheduler interference: {:.1}% avg error", avg_error);

        assert!(avg_error < 15.0, "Timing precision severely affected by scheduler interference");
    }

    fn measure_timing_precision(tsc_freq: u64, samples: usize) -> Vec<f64> {
        let mut errors = Vec::new();

        for _ in 0..samples {
            let expected_ns = 1000; // 1 microsecond
            let start = TscTimestamp::now();

            // Busy wait for expected duration
            let target_cycles = (expected_ns * tsc_freq) / 1_000_000_000;
            let target_tsc = start.0 + target_cycles;

            while TscTimestamp::now().0 < target_tsc {
                std::hint::black_box(());
            }

            let end = TscTimestamp::now();
            let actual_ns = end.nanos_since(start, tsc_freq);

            let error_percent = ((actual_ns as f64 - expected_ns as f64).abs() / expected_ns as f64) * 100.0;
            errors.push(error_percent);
        }

        errors
    }
}

/// Test timing overhead and performance
#[cfg(test)]
mod timing_overhead_tests {
    use super::*;

    #[test]
    fn test_tsc_read_overhead() {
        let iterations = 10000;
        let start = TscTimestamp::now();

        // Measure TSC read overhead
        for _ in 0..iterations {
            std::hint::black_box(TscTimestamp::now());
        }

        let end = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let total_time_ns = end.nanos_since(start, tsc_freq);
        let avg_read_time_ns = total_time_ns / iterations;

        println!("TSC read overhead: {} ns per read", avg_read_time_ns);

        // TSC read should be very fast (< 100ns)
        assert!(avg_read_time_ns < 100,
            "TSC read too slow: {}ns per read", avg_read_time_ns);
    }

    #[test]
    fn test_nanos_since_calculation_overhead() {
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let ts1 = TscTimestamp::now();
        let ts2 = TscTimestamp::now();

        let iterations = 10000;
        let start = TscTimestamp::now();

        // Measure calculation overhead
        for _ in 0..iterations {
            std::hint::black_box(ts2.nanos_since(ts1, tsc_freq));
        }

        let end = TscTimestamp::now();
        let total_time_ns = end.nanos_since(start, tsc_freq);
        let avg_calc_time_ns = total_time_ns / iterations;

        println!("nanos_since calculation overhead: {} ns per calculation", avg_calc_time_ns);

        // Calculation should be fast (< 50ns)
        assert!(avg_calc_time_ns < 50,
            "nanos_since calculation too slow: {}ns per calculation", avg_calc_time_ns);
    }

    #[test]
    fn test_add_nanos_overhead() {
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let base_ts = TscTimestamp::now();

        let iterations = 10000;
        let start = TscTimestamp::now();

        // Measure add_nanos overhead
        for i in 0..iterations {
            std::hint::black_box(base_ts.add_nanos(i * 100, tsc_freq));
        }

        let end = TscTimestamp::now();
        let total_time_ns = end.nanos_since(start, tsc_freq);
        let avg_add_time_ns = total_time_ns / iterations;

        println!("add_nanos overhead: {} ns per operation", avg_add_time_ns);

        // Should be very fast (< 30ns)
        assert!(avg_add_time_ns < 30,
            "add_nanos too slow: {}ns per operation", avg_add_time_ns);
    }

    #[test]
    fn test_scheduler_tick_timing_overhead() {
        let mut scheduler = NanosecondScheduler::new();

        // Measure bare tick overhead
        let iterations = 1000;
        let mut tick_times = Vec::new();

        for _ in 0..iterations {
            let start = TscTimestamp::now();
            scheduler.tick().unwrap();
            let end = TscTimestamp::now();

            let tick_time_ns = end.nanos_since(start, scheduler.config.tsc_frequency_hz);
            tick_times.push(tick_time_ns);
        }

        // Calculate statistics
        tick_times.sort_unstable();
        let min_time = tick_times[0];
        let max_time = tick_times[tick_times.len() - 1];
        let median_time = tick_times[tick_times.len() / 2];
        let avg_time = tick_times.iter().sum::<u64>() as f64 / tick_times.len() as f64;

        println!("Scheduler tick timing overhead:");
        println!("  Min: {} ns", min_time);
        println!("  Avg: {:.1} ns", avg_time);
        println!("  Median: {} ns", median_time);
        println!("  Max: {} ns", max_time);

        // Verify meets <1μs requirement
        assert!(avg_time < 1000.0, "Average tick time exceeds 1μs: {:.1}ns", avg_time);
        assert!(median_time < 1000, "Median tick time exceeds 1μs: {}ns", median_time);

        // 95th percentile should also be reasonable
        let p95_index = (tick_times.len() as f64 * 0.95) as usize;
        let p95_time = tick_times[p95_index];
        println!("  95th percentile: {} ns", p95_time);

        assert!(p95_time < 2000, "95th percentile tick time too high: {}ns", p95_time);
    }
}

/// Test timing precision in different scenarios
#[cfg(test)]
mod scenario_precision_tests {
    use super::*;

    #[test]
    fn test_microsecond_precision() {
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let test_durations = [1000, 2000, 5000, 10000]; // 1-10 microseconds

        for &expected_ns in &test_durations {
            let mut errors = Vec::new();

            for _ in 0..20 {
                let start = TscTimestamp::now();

                // Busy wait for precise duration
                let target_cycles = (expected_ns * tsc_freq) / 1_000_000_000;
                let end_target = start.0 + target_cycles;

                while TscTimestamp::now().0 < end_target {
                    // Tight loop
                }

                let end = TscTimestamp::now();
                let actual_ns = end.nanos_since(start, tsc_freq);
                let error_ns = if actual_ns > expected_ns {
                    actual_ns - expected_ns
                } else {
                    expected_ns - actual_ns
                };

                errors.push(error_ns);
            }

            let avg_error = errors.iter().sum::<u64>() as f64 / errors.len() as f64;
            let max_error = *errors.iter().max().unwrap();

            println!("{}μs timing precision: avg_error={:.1}ns, max_error={}ns",
                    expected_ns / 1000, avg_error, max_error);

            // For microsecond timing, should be accurate within 100ns average
            assert!(avg_error < 100.0,
                "Microsecond timing not precise enough: {:.1}ns average error", avg_error);
        }
    }

    #[test]
    fn test_nanosecond_resolution() {
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Test very short durations
        let test_durations = [10, 50, 100, 500]; // 10-500 nanoseconds

        for &expected_ns in &test_durations {
            let mut successful_measurements = 0;
            let attempts = 100;

            for _ in 0..attempts {
                let start = TscTimestamp::now();

                // Try to wait for nanosecond duration
                let target_cycles = (expected_ns * tsc_freq) / 1_000_000_000;

                if target_cycles == 0 {
                    // Duration too short for this TSC frequency
                    continue;
                }

                let end_target = start.0 + target_cycles;
                while TscTimestamp::now().0 < end_target {
                    // Minimal loop
                }

                let end = TscTimestamp::now();
                let actual_ns = end.nanos_since(start, tsc_freq);

                // Check if measurement is reasonable
                if actual_ns >= expected_ns / 2 && actual_ns <= expected_ns * 5 {
                    successful_measurements += 1;
                }
            }

            let success_rate = successful_measurements as f64 / attempts as f64;
            println!("{}ns timing: {:.1}% successful measurements",
                    expected_ns, success_rate * 100.0);

            // For longer durations, should have better success rate
            if expected_ns >= 100 {
                assert!(success_rate > 0.5,
                    "Nanosecond timing resolution too poor for {}ns", expected_ns);
            }
        }
    }

    #[test]
    fn test_timing_stability_over_time() {
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let target_duration_ns = 10000; // 10 microseconds
        let measurements_per_batch = 10;
        let num_batches = 20;

        let mut batch_averages = Vec::new();

        for batch in 0..num_batches {
            let mut batch_measurements = Vec::new();

            for _ in 0..measurements_per_batch {
                let start = TscTimestamp::now();

                // Precise timing
                let target_cycles = (target_duration_ns * tsc_freq) / 1_000_000_000;
                let end_target = start.0 + target_cycles;

                while TscTimestamp::now().0 < end_target {
                    std::hint::black_box(());
                }

                let end = TscTimestamp::now();
                let actual_ns = end.nanos_since(start, tsc_freq);
                batch_measurements.push(actual_ns);
            }

            let batch_avg = batch_measurements.iter().sum::<u64>() as f64 / batch_measurements.len() as f64;
            batch_averages.push(batch_avg);

            println!("Batch {}: avg={:.1}ns", batch, batch_avg);

            // Small delay between batches
            std::thread::sleep(Duration::from_millis(10));
        }

        // Check stability across batches
        let overall_avg = batch_averages.iter().sum::<f64>() / batch_averages.len() as f64;
        let max_deviation = batch_averages.iter()
            .map(|&avg| (avg - overall_avg).abs())
            .fold(0.0, f64::max);

        let stability_percent = (max_deviation / overall_avg) * 100.0;

        println!("Timing stability over time:");
        println!("  Overall average: {:.1}ns", overall_avg);
        println!("  Max deviation: {:.1}ns ({:.2}%)", max_deviation, stability_percent);

        assert!(stability_percent < 5.0,
            "Timing not stable over time: {:.2}% deviation", stability_percent);
    }

    #[test]
    fn test_concurrent_timing_accuracy() {
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let num_threads = 4;
        let measurements_per_thread = 100;

        let handles: Vec<_> = (0..num_threads).map(|thread_id| {
            std::thread::spawn(move || {
                let mut measurements = Vec::new();
                let target_duration_ns = 5000; // 5 microseconds

                for _ in 0..measurements_per_thread {
                    let start = TscTimestamp::now();

                    let target_cycles = (target_duration_ns * tsc_freq) / 1_000_000_000;
                    let end_target = start.0 + target_cycles;

                    while TscTimestamp::now().0 < end_target {
                        std::hint::black_box(());
                    }

                    let end = TscTimestamp::now();
                    let actual_ns = end.nanos_since(start, tsc_freq);
                    measurements.push(actual_ns);

                    // Small delay to allow other threads
                    for _ in 0..100 {
                        std::hint::black_box(());
                    }
                }

                (thread_id, measurements)
            })
        }).collect();

        // Collect results from all threads
        let mut all_results = Vec::new();
        for handle in handles {
            let (thread_id, measurements) = handle.join().unwrap();
            let avg = measurements.iter().sum::<u64>() as f64 / measurements.len() as f64;
            println!("Thread {}: avg={:.1}ns", thread_id, avg);
            all_results.extend(measurements);
        }

        // Analyze overall accuracy
        let overall_avg = all_results.iter().sum::<u64>() as f64 / all_results.len() as f64;
        let target_ns = 5000.0;
        let accuracy_error = ((overall_avg - target_ns).abs() / target_ns) * 100.0;

        println!("Concurrent timing accuracy: {:.1}ns avg ({:.2}% error)",
                overall_avg, accuracy_error);

        assert!(accuracy_error < 10.0,
            "Concurrent timing accuracy poor: {:.2}% error", accuracy_error);
    }
}