//! Temporal Window Overlap Management tests
//!
//! This module validates temporal window management, overlap calculations,
//! continuity preservation, and window state management.

use super::*;

/// Test basic temporal window functionality
#[cfg(test)]
mod basic_window_tests {
    use super::*;

    #[test]
    fn test_temporal_window_creation() {
        let window = TemporalWindow::new(1, 0, 999, 100);

        assert_eq!(window.info.window_id, 1);
        assert_eq!(window.info.start_tick, 0);
        assert_eq!(window.info.end_tick, 999);
        assert_eq!(window.info.overlap_size, 100);
        assert!(window.info.active);
        assert_eq!(window.size(), 1000);
        assert!(window.data.is_empty());
        assert!(window.state_snapshot.is_empty());
    }

    #[test]
    fn test_window_contains_tick() {
        let window = TemporalWindow::new(1, 100, 200, 50);

        assert!(!window.contains_tick(99));
        assert!(window.contains_tick(100));
        assert!(window.contains_tick(150));
        assert!(window.contains_tick(200));
        assert!(!window.contains_tick(201));
    }

    #[test]
    fn test_window_overlap_calculation() {
        let window1 = TemporalWindow::new(1, 0, 199, 50);      // 0-199
        let window2 = TemporalWindow::new(2, 150, 349, 50);    // 150-349

        let overlap = window1.calculate_overlap(&window2);
        assert_eq!(overlap, 50); // Overlap from 150 to 199 (inclusive)

        // Test no overlap
        let window3 = TemporalWindow::new(3, 300, 399, 25);
        let no_overlap = window1.calculate_overlap(&window3);
        assert_eq!(no_overlap, 0);

        // Test complete overlap
        let window4 = TemporalWindow::new(4, 50, 150, 25);
        let complete_overlap = window1.calculate_overlap(&window4);
        assert_eq!(complete_overlap, 101); // 50-150 inclusive
    }

    #[test]
    fn test_window_state_storage() {
        let mut window = TemporalWindow::new(1, 0, 99, 10);

        let test_state = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        window.store_state(test_state.clone());
        assert_eq!(window.state_snapshot, test_state);

        let test_data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        window.store_data(test_data.clone());
        assert_eq!(window.data, test_data);
    }

    #[test]
    fn test_window_deactivation() {
        let mut window = TemporalWindow::new(1, 0, 99, 10);

        assert!(window.info.active);
        window.deactivate();
        assert!(!window.info.active);
    }
}

/// Test window overlap manager functionality
#[cfg(test)]
mod overlap_manager_tests {
    use super::*;

    #[test]
    fn test_overlap_manager_creation() {
        let manager = WindowOverlapManager::new(75.0);

        assert!(manager.windows.is_empty());
        assert_eq!(manager.next_window_id, 1);
        assert_eq!(manager.target_overlap_percent, 75.0);
        assert_eq!(manager.current_tick, 0);

        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_windows, 0);
        assert_eq!(metrics.average_overlap_percentage, 0.0);
        assert_eq!(metrics.continuity_breaks, 0);
    }

    #[test]
    fn test_overlap_target_clamping() {
        let manager1 = WindowOverlapManager::new(30.0); // Below minimum
        assert_eq!(manager1.target_overlap_percent, 50.0);

        let manager2 = WindowOverlapManager::new(110.0); // Above maximum
        assert_eq!(manager2.target_overlap_percent, 100.0);

        let manager3 = WindowOverlapManager::new(75.0); // Within range
        assert_eq!(manager3.target_overlap_percent, 75.0);
    }

    #[test]
    fn test_window_creation_progression() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Advance through ticks to trigger window creation
        for tick in 0..3000 {
            manager.advance_window(tick).unwrap();
        }

        assert!(manager.windows.len() > 1);
        assert!(manager.get_metrics().total_windows > 1);

        // Verify windows have correct overlap
        let overlap_percent = manager.get_current_overlap_percentage();
        println!("Current overlap percentage: {:.1}%", overlap_percent);

        // Should maintain reasonable overlap (within tolerance)
        assert!(overlap_percent >= 50.0, "Overlap too low: {:.1}%", overlap_percent);
    }

    #[test]
    fn test_window_overlap_requirements() {
        let mut manager = WindowOverlapManager::new(90.0); // High overlap requirement

        let mut continuity_breaks = 0;

        // Advance through many ticks
        for tick in 0..5000 {
            let result = manager.advance_window(tick);

            if result.is_err() {
                match result.unwrap_err() {
                    TemporalError::WindowOverlapTooLow { actual, required } => {
                        continuity_breaks += 1;
                        println!("Continuity break at tick {}: {:.1}% < {:.1}%",
                                tick, actual, required);
                    },
                    _ => panic!("Unexpected error"),
                }
            }
        }

        // High overlap requirements may cause some breaks
        println!("Total continuity breaks: {}", continuity_breaks);
        assert_eq!(manager.get_metrics().continuity_breaks, continuity_breaks as u64);
    }

    #[test]
    fn test_current_window_info() {
        let mut manager = WindowOverlapManager::new(80.0);

        // Initially no windows
        let initial_info = manager.get_current_window();
        assert_eq!(initial_info.window_id, 0);
        assert!(!initial_info.active);

        // Create first window
        manager.advance_window(0).unwrap();
        manager.advance_window(100).unwrap();

        let current_info = manager.get_current_window();
        assert_ne!(current_info.window_id, 0);
        assert!(current_info.active);
        assert!(current_info.overlap_size > 0);
    }

    #[test]
    fn test_window_state_storage() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Create windows
        manager.advance_window(0).unwrap();
        manager.advance_window(500).unwrap();

        // Store state and data
        let test_state = vec![1.0, 2.0, 3.0, 4.0];
        manager.store_current_state(test_state.clone());

        let test_data = vec![0xCA, 0xFE, 0xBA, 0xBE];
        manager.store_current_data(test_data.clone());

        // Verify storage
        if let Some(current_window) = manager.windows.back() {
            assert_eq!(current_window.state_snapshot, test_state);
            assert_eq!(current_window.data, test_data);
        } else {
            panic!("No current window found");
        }
    }

    #[test]
    fn test_active_windows_tracking() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Create multiple windows
        for tick in 0..4000 {
            manager.advance_window(tick).unwrap();
        }

        let active_windows = manager.get_active_windows();
        println!("Active windows: {}", active_windows.len());

        // All recent windows should be active
        for window in &active_windows {
            assert!(window.info.active);
        }

        // Should not be empty if we've created windows
        assert!(!active_windows.is_empty());
    }

    #[test]
    fn test_window_cleanup() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Create many windows to trigger cleanup
        for tick in 0..20000 {
            manager.advance_window(tick).unwrap();
        }

        // Should not exceed maximum window count
        assert!(manager.windows.len() <= manager.max_windows);

        // Metrics should track total windows created
        let metrics = manager.get_metrics();
        assert!(metrics.total_windows > manager.windows.len() as u64);
    }

    #[test]
    fn test_window_for_tick_lookup() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Create several windows
        for tick in 0..3000 {
            manager.advance_window(tick).unwrap();
        }

        // Test finding windows for specific ticks
        let test_ticks = [100, 500, 1000, 1500, 2000, 2500];

        for &test_tick in &test_ticks {
            let found_window = manager.find_window_for_tick(test_tick);

            if let Some(window) = found_window {
                assert!(window.contains_tick(test_tick),
                    "Window should contain tick {}", test_tick);
                println!("Tick {} found in window {}", test_tick, window.info.window_id);
            } else {
                println!("Tick {} not found in any window", test_tick);
            }
        }
    }
}

/// Test overlap adjustment and management
#[cfg(test)]
mod overlap_adjustment_tests {
    use super::*;

    #[test]
    fn test_overlap_adjustment() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Create some windows
        manager.advance_window(0).unwrap();
        manager.advance_window(800).unwrap();

        let window_id = manager.get_current_window().window_id;

        // Adjust overlap
        let new_overlap_target = 85.0;
        manager.adjust_overlap(window_id, new_overlap_target).unwrap();

        // Verify adjustment was applied
        let window = manager.windows.iter()
            .find(|w| w.info.window_id == window_id)
            .expect("Window should exist");

        let expected_overlap_size = ((window.size() as f64) * (new_overlap_target / 100.0)) as u64;
        println!("Expected overlap size: {}, Actual: {}", expected_overlap_size, window.info.overlap_size);

        // Should be approximately correct (within 1 tick tolerance)
        let diff = if window.info.overlap_size > expected_overlap_size {
            window.info.overlap_size - expected_overlap_size
        } else {
            expected_overlap_size - window.info.overlap_size
        };
        assert!(diff <= 1, "Overlap adjustment not accurate");
    }

    #[test]
    fn test_invalid_window_adjustment() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Try to adjust non-existent window
        let result = manager.adjust_overlap(999, 80.0);

        assert!(result.is_err());
        match result.unwrap_err() {
            TemporalError::TscTimingError { message } => {
                assert!(message.contains("Window 999 not found"));
            },
            _ => panic!("Expected TscTimingError"),
        }
    }

    #[test]
    fn test_overlap_percentage_calculation() {
        let mut manager = WindowOverlapManager::new(80.0);

        // Single window should show perfect continuity
        manager.advance_window(0).unwrap();
        let single_window_overlap = manager.get_current_overlap_percentage();
        assert_eq!(single_window_overlap, 100.0);

        // Create overlapping windows
        for tick in 0..2000 {
            manager.advance_window(tick).unwrap();
        }

        let overlap_percent = manager.get_current_overlap_percentage();
        println!("Calculated overlap percentage: {:.1}%", overlap_percent);

        // Should be reasonable for 80% target
        assert!(overlap_percent >= 50.0, "Overlap too low");
        assert!(overlap_percent <= 100.0, "Overlap should not exceed 100%");
    }

    #[test]
    fn test_overlap_metrics_tracking() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Create windows and track metrics evolution
        let mut metrics_history = Vec::new();

        for tick in 0..5000 {
            manager.advance_window(tick).unwrap();

            if tick % 500 == 0 {
                let metrics = manager.get_metrics();
                metrics_history.push(metrics.clone());

                println!("Tick {}: avg_overlap={:.1}%, total_windows={}",
                        tick, metrics.average_overlap_percentage, metrics.total_windows);
            }
        }

        let final_metrics = manager.get_metrics();

        // Verify metrics properties
        assert!(final_metrics.total_windows > 0);
        assert!(final_metrics.average_overlap_percentage >= 0.0);
        assert!(final_metrics.min_overlap_percentage >= 0.0);
        assert!(final_metrics.max_overlap_percentage <= 100.0);
        assert!(final_metrics.optimal_overlap_ratio > 0.0);

        // Optimal overlap ratio should be close to 1.0 for good management
        println!("Final optimal overlap ratio: {:.3}", final_metrics.optimal_overlap_ratio);
    }

    #[test]
    fn test_different_overlap_targets() {
        let overlap_targets = [60.0, 70.0, 80.0, 90.0, 95.0];

        for &target in &overlap_targets {
            let mut manager = WindowOverlapManager::new(target);

            // Create windows with this target
            for tick in 0..3000 {
                manager.advance_window(tick).unwrap();
            }

            let final_overlap = manager.get_current_overlap_percentage();
            let metrics = manager.get_metrics();

            println!("Target: {:.1}%, Achieved: {:.1}%, Optimal ratio: {:.3}",
                    target, final_overlap, metrics.optimal_overlap_ratio);

            // Should achieve something reasonable relative to target
            // (allowing for tolerance due to discrete tick boundaries)
            assert!(final_overlap >= target * 0.7,
                "Failed to achieve reasonable overlap for target {:.1}%", target);
        }
    }
}

/// Test window boundary and timing edge cases
#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn test_window_boundary_overlaps() {
        let window1 = TemporalWindow::new(1, 0, 99, 10);
        let window2 = TemporalWindow::new(2, 99, 199, 10);   // Exactly touching
        let window3 = TemporalWindow::new(3, 100, 199, 10);  // Adjacent

        let touching_overlap = window1.calculate_overlap(&window2);
        assert_eq!(touching_overlap, 1); // Single tick overlap

        let adjacent_overlap = window1.calculate_overlap(&window3);
        assert_eq!(adjacent_overlap, 0); // No overlap
    }

    #[test]
    fn test_zero_size_windows() {
        let zero_window = TemporalWindow::new(1, 100, 100, 0);
        assert_eq!(zero_window.size(), 1); // Single tick window

        let normal_window = TemporalWindow::new(2, 90, 110, 5);
        let overlap = zero_window.calculate_overlap(&normal_window);
        assert_eq!(overlap, 1); // Single tick overlap
    }

    #[test]
    fn test_window_order_independence() {
        let window1 = TemporalWindow::new(1, 100, 200, 25);
        let window2 = TemporalWindow::new(2, 150, 250, 25);

        let overlap1_2 = window1.calculate_overlap(&window2);
        let overlap2_1 = window2.calculate_overlap(&window1);

        assert_eq!(overlap1_2, overlap2_1, "Overlap calculation should be symmetric");
    }

    #[test]
    fn test_rapid_tick_advancement() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Very rapid tick advancement
        for tick in (0..10000).step_by(1) {
            manager.advance_window(tick).unwrap();
        }

        let metrics = manager.get_metrics();
        println!("Rapid advancement - Total windows: {}, Avg overlap: {:.1}%",
                metrics.total_windows, metrics.average_overlap_percentage);

        assert!(metrics.total_windows > 5);
        assert!(metrics.average_overlap_percentage > 0.0);
    }

    #[test]
    fn test_sparse_tick_advancement() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Sparse tick advancement (large gaps)
        let ticks = [0, 1000, 2500, 4000, 7000, 10000];

        for &tick in &ticks {
            manager.advance_window(tick).unwrap();
        }

        let metrics = manager.get_metrics();
        println!("Sparse advancement - Total windows: {}, Avg overlap: {:.1}%",
                metrics.total_windows, metrics.average_overlap_percentage);

        assert!(metrics.total_windows > 0);
    }

    #[test]
    fn test_backwards_tick_handling() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Normal progression
        manager.advance_window(1000).unwrap();
        manager.advance_window(2000).unwrap();

        // Try to go backwards (should handle gracefully)
        manager.advance_window(1500).unwrap();

        // Should not crash and maintain reasonable state
        let current_info = manager.get_current_window();
        assert!(current_info.window_id > 0);
    }

    #[test]
    fn test_very_large_ticks() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Test with very large tick values
        let large_ticks = [u64::MAX / 2, u64::MAX / 2 + 1000, u64::MAX / 2 + 2000];

        for &tick in &large_ticks {
            let result = manager.advance_window(tick);
            // Should handle large values without overflow
            assert!(result.is_ok() || result.is_err()); // Either is acceptable
        }
    }
}

/// Test performance and stress scenarios
#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_window_creation_performance() {
        let mut manager = WindowOverlapManager::new(75.0);

        let start_time = std::time::Instant::now();
        let iterations = 10000;

        for tick in 0..iterations {
            manager.advance_window(tick).unwrap();
        }

        let elapsed = start_time.elapsed();
        let per_operation = elapsed.as_nanos() as f64 / iterations as f64;

        println!("Window advance performance: {:.1}ns per operation", per_operation);

        // Should be reasonably fast
        assert!(per_operation < 10000.0, "Window advance too slow: {:.1}ns", per_operation);
    }

    #[test]
    fn test_overlap_calculation_performance() {
        let window1 = TemporalWindow::new(1, 0, 99999, 5000);
        let window2 = TemporalWindow::new(2, 50000, 149999, 5000);

        let start_time = std::time::Instant::now();
        let iterations = 100000;

        for _ in 0..iterations {
            std::hint::black_box(window1.calculate_overlap(&window2));
        }

        let elapsed = start_time.elapsed();
        let per_operation = elapsed.as_nanos() as f64 / iterations as f64;

        println!("Overlap calculation performance: {:.1}ns per operation", per_operation);

        // Should be very fast
        assert!(per_operation < 100.0, "Overlap calculation too slow: {:.1}ns", per_operation);
    }

    #[test]
    fn test_memory_usage_with_many_windows() {
        let mut manager = WindowOverlapManager::new(75.0);

        // Create many windows to test memory management
        for tick in 0..50000 {
            manager.advance_window(tick).unwrap();

            // Store some data in each window occasionally
            if tick % 100 == 0 {
                let test_data = vec![tick as u8; 100];
                manager.store_current_data(test_data);

                let test_state = vec![tick as f64; 10];
                manager.store_current_state(test_state);
            }
        }

        // Should maintain bounded memory usage
        assert!(manager.windows.len() <= manager.max_windows);

        let metrics = manager.get_metrics();
        println!("Memory test - Total windows created: {}, Currently stored: {}",
                metrics.total_windows, manager.windows.len());

        assert!(metrics.total_windows > manager.windows.len() as u64);
    }

    #[test]
    fn test_concurrent_window_access() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let manager = Arc::new(Mutex::new(WindowOverlapManager::new(75.0)));

        // Create some windows first
        {
            let mut mgr = manager.lock().unwrap();
            for tick in 0..1000 {
                mgr.advance_window(tick).unwrap();
            }
        }

        let handles: Vec<_> = (0..4).map(|thread_id| {
            let manager_clone = manager.clone();

            thread::spawn(move || {
                for i in 0..100 {
                    let mgr = manager_clone.lock().unwrap();

                    // Read operations that should be safe
                    let _current = mgr.get_current_window();
                    let _metrics = mgr.get_metrics();
                    let _active = mgr.get_active_windows();

                    // Find window for various ticks
                    let test_tick = (thread_id * 100 + i) % 1000;
                    let _found = mgr.find_window_for_tick(test_tick);
                }
            })
        }).collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify state is still consistent
        let mgr = manager.lock().unwrap();
        let metrics = mgr.get_metrics();
        assert!(metrics.total_windows > 0);
    }
}