//! Stress and edge case tests for ruQu coherence gate
//!
//! Tests for high throughput syndrome streaming, memory pressure (64KB budget),
//! rapid decision cycling, and error recovery scenarios.

use ruqu::filters::{
    EvidenceAccumulator, EvidenceConfig, EvidenceFilter, FilterConfig, FilterPipeline, ShiftConfig,
    ShiftFilter, StructuralConfig, StructuralFilter, SystemState, Verdict,
};
use ruqu::syndrome::{DetectorBitmap, SyndromeBuffer, SyndromeDelta, SyndromeRound};
use ruqu::tile::{
    GateDecision, GateThresholds, PatchGraph, ReceiptLog, SyndromeDelta as TileSyndromeDelta,
    TileReport, TileZero, WorkerTile, MAX_PATCH_EDGES, MAX_PATCH_VERTICES, SYNDROME_BUFFER_DEPTH,
};
use ruqu::{TILE_MEMORY_BUDGET, WORKER_TILE_COUNT};

use std::time::Instant;

// ============================================================================
// High Throughput Syndrome Streaming Tests
// ============================================================================

mod throughput_tests {
    use super::*;

    #[test]
    fn test_syndrome_stream_10k_rounds() {
        let mut buffer = SyndromeBuffer::new(1024);

        for i in 0..10_000 {
            let mut detectors = DetectorBitmap::new(64);
            if i % 100 == 0 {
                detectors.set(i as usize % 64, true);
            }
            let round = SyndromeRound::new(i, i, i * 1_000, detectors, 0);
            buffer.push(round);
        }

        // Buffer should still function correctly
        assert_eq!(buffer.len(), 1024);
        assert!(buffer.get(9_999).is_some());
        assert!(buffer.get(8_975).is_none()); // Evicted
    }

    #[test]
    fn test_syndrome_stream_100k_rounds() {
        let mut buffer = SyndromeBuffer::new(1024);

        let start = Instant::now();

        for i in 0..100_000u64 {
            let mut detectors = DetectorBitmap::new(256);
            if i % 10 == 0 {
                for j in 0..(i % 10) as usize {
                    detectors.set(j, true);
                }
            }
            let round = SyndromeRound::new(i, i, i * 1_000, detectors, 0);
            buffer.push(round);
        }

        let duration = start.elapsed();

        // Performance sanity check - should complete in reasonable time
        assert!(
            duration.as_millis() < 5_000,
            "100k rounds took too long: {:?}",
            duration
        );

        // Data integrity
        assert_eq!(buffer.len(), 1024);
    }

    #[test]
    fn test_worker_tile_high_throughput() {
        let mut tile = WorkerTile::new(1);

        let start = Instant::now();

        for i in 0..10_000 {
            let delta =
                TileSyndromeDelta::new((i % 64) as u16, ((i + 1) % 64) as u16, (i % 256) as u16);
            tile.tick(&delta);
        }

        let duration = start.elapsed();

        assert_eq!(tile.tick, 10_000);
        assert!(
            duration.as_millis() < 5_000,
            "10k ticks took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_tilezero_high_report_throughput() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let start = Instant::now();

        for _ in 0..1_000 {
            let reports: Vec<TileReport> = (1..=50)
                .map(|i| {
                    let mut report = TileReport::new(i);
                    report.local_cut = 10.0;
                    report.shift_score = 0.1;
                    report.e_value = 200.0;
                    report
                })
                .collect();

            tilezero.merge_reports(reports);
        }

        let duration = start.elapsed();

        assert_eq!(tilezero.receipt_log.len(), 1_000);
        assert!(
            duration.as_millis() < 5_000,
            "1000 merges took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_bitmap_operations_throughput() {
        let mut a = DetectorBitmap::new(1024);
        let mut b = DetectorBitmap::new(1024);

        // Setup
        for i in (0..1024).step_by(2) {
            a.set(i, true);
        }
        for i in (1..1024).step_by(2) {
            b.set(i, true);
        }

        let start = Instant::now();

        for _ in 0..100_000 {
            let _ = a.xor(&b);
            let _ = a.and(&b);
            let _ = a.or(&b);
        }

        let duration = start.elapsed();

        // 300k bitmap operations should be fast (SIMD-like)
        assert!(
            duration.as_millis() < 2_000,
            "Bitmap ops took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_popcount_throughput() {
        let mut bitmap = DetectorBitmap::new(1024);

        for i in (0..1024).step_by(3) {
            bitmap.set(i, true);
        }

        let start = Instant::now();

        let mut total = 0usize;
        for _ in 0..1_000_000 {
            total += bitmap.popcount();
        }

        let duration = start.elapsed();

        // 1M popcounts should be very fast (hardware instruction)
        assert!(
            duration.as_millis() < 1_000,
            "Popcount ops took too long: {:?}",
            duration
        );
        assert!(total > 0); // Prevent optimization
    }
}

// ============================================================================
// Memory Pressure Tests (64KB Budget)
// ============================================================================

mod memory_pressure_tests {
    use super::*;

    #[test]
    fn test_worker_tile_memory_budget() {
        let size = WorkerTile::memory_size();

        // Target is 64KB per tile, allow up to 128KB
        assert!(
            size <= TILE_MEMORY_BUDGET * 2,
            "WorkerTile exceeds 128KB budget: {} bytes",
            size
        );

        // Log actual size for monitoring
        println!(
            "WorkerTile memory: {} bytes ({:.1}% of 64KB)",
            size,
            (size as f64 / 65536.0) * 100.0
        );
    }

    #[test]
    fn test_patch_graph_memory_budget() {
        let size = PatchGraph::memory_size();

        // PatchGraph should be ~32KB
        assert!(size <= 65536, "PatchGraph exceeds 64KB: {} bytes", size);

        println!("PatchGraph memory: {} bytes", size);
    }

    #[test]
    fn test_syndrome_buffer_memory_budget() {
        let size = ruqu::tile::SyndromBuffer::memory_size();

        // SyndromBuffer should be ~16KB
        assert!(size <= 32768, "SyndromBuffer exceeds 32KB: {} bytes", size);

        println!("SyndromBuffer memory: {} bytes", size);
    }

    #[test]
    fn test_multiple_tiles_memory() {
        // Simulate 256-tile fabric memory
        let tile_size = WorkerTile::memory_size();
        let total_memory = tile_size * 255; // 255 worker tiles

        // Total should be reasonable (target ~16MB for all tiles)
        let mb = total_memory / (1024 * 1024);
        println!("Total fabric memory (255 tiles): {} MB", mb);

        assert!(mb < 64, "Total fabric memory exceeds 64MB: {} MB", mb);
    }

    #[test]
    fn test_patch_graph_at_capacity() {
        let mut graph = PatchGraph::new();

        // Fill to edge capacity
        let mut edge_count = 0;
        for v1 in 0..16 {
            for v2 in (v1 + 1)..16 {
                if graph.add_edge(v1, v2, 100).is_some() {
                    edge_count += 1;
                }
            }
        }

        // Should handle many edges
        assert!(edge_count > 0);
        assert_eq!(graph.num_edges as usize, edge_count);
    }

    #[test]
    fn test_patch_graph_vertex_limit() {
        let mut graph = PatchGraph::new();

        // Try to use vertices up to limit
        for i in 0..(MAX_PATCH_VERTICES - 1) {
            let v1 = i as u16;
            let v2 = (i + 1) as u16;
            if v2 < MAX_PATCH_VERTICES as u16 {
                graph.add_edge(v1, v2, 100);
            }
        }

        assert!(graph.num_vertices <= MAX_PATCH_VERTICES as u16);
    }

    #[test]
    fn test_syndrome_buffer_at_depth() {
        let mut buffer = ruqu::tile::SyndromBuffer::new();

        // Fill to depth
        for i in 0..SYNDROME_BUFFER_DEPTH as u32 {
            let entry = ruqu::tile::SyndromeEntry {
                round: i,
                syndrome: [i as u8; 8],
                flags: 0,
            };
            buffer.append(entry);
        }

        assert_eq!(buffer.count as usize, SYNDROME_BUFFER_DEPTH);

        // Overflow
        let entry = ruqu::tile::SyndromeEntry {
            round: SYNDROME_BUFFER_DEPTH as u32,
            syndrome: [0; 8],
            flags: 0,
        };
        buffer.append(entry);

        assert_eq!(buffer.count as usize, SYNDROME_BUFFER_DEPTH);
    }

    #[test]
    fn test_receipt_log_growth() {
        let mut log = ReceiptLog::new();

        // Log many receipts
        for i in 0..10_000 {
            log.append(GateDecision::Permit, i, i * 1_000, [0u8; 32]);
        }

        assert_eq!(log.len(), 10_000);

        // Should still be searchable
        assert!(log.get(5_000).is_some());
    }
}

// ============================================================================
// Rapid Decision Cycling Tests
// ============================================================================

mod rapid_decision_tests {
    use super::*;

    #[test]
    fn test_rapid_permit_deny_cycling() {
        let thresholds = GateThresholds {
            structural_min_cut: 5.0,
            ..Default::default()
        };
        let mut tilezero = TileZero::new(thresholds);

        for i in 0..1_000 {
            let cut_value = if i % 2 == 0 { 10.0 } else { 1.0 };

            let reports: Vec<TileReport> = (1..=5)
                .map(|j| {
                    let mut report = TileReport::new(j);
                    report.local_cut = cut_value;
                    report.shift_score = 0.1;
                    report.e_value = 200.0;
                    report
                })
                .collect();

            let decision = tilezero.merge_reports(reports);

            if cut_value < 5.0 {
                assert_eq!(decision, GateDecision::Deny);
            } else {
                assert_eq!(decision, GateDecision::Permit);
            }
        }

        assert_eq!(tilezero.receipt_log.len(), 1_000);
    }

    #[test]
    fn test_rapid_filter_evaluation() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 2.0,
                use_subpolynomial: false,
                ..Default::default()
            },
            shift: ShiftConfig {
                threshold: 0.5,
                ..Default::default()
            },
            evidence: EvidenceConfig {
                tau_permit: 10.0,
                tau_deny: 0.1,
                ..Default::default()
            },
        };

        let mut pipeline = FilterPipeline::new(config);
        pipeline.structural_mut().insert_edge(1, 2, 3.0).unwrap();
        pipeline.structural_mut().insert_edge(2, 3, 3.0).unwrap();

        let state = SystemState::new(3);

        let start = Instant::now();

        for _ in 0..10_000 {
            let _ = pipeline.evaluate(&state);
        }

        let duration = start.elapsed();

        // 10k evaluations should be fast
        assert!(
            duration.as_millis() < 5_000,
            "10k evaluations took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_evidence_rapid_accumulation() {
        let mut acc = EvidenceAccumulator::new();

        let start = Instant::now();

        for _ in 0..100_000 {
            acc.update(1.1);
        }

        let duration = start.elapsed();

        // 100k updates should be fast
        assert!(
            duration.as_millis() < 1_000,
            "100k evidence updates took too long: {:?}",
            duration
        );

        // E-value should be very high
        assert!(acc.e_value() > 1e10);
    }

    #[test]
    fn test_shift_filter_rapid_updates() {
        let mut filter = ShiftFilter::new(0.5, 100);

        let start = Instant::now();

        for i in 0..100_000 {
            filter.update(i % 64, (i as f64) % 10.0);
        }

        let duration = start.elapsed();

        assert!(
            duration.as_millis() < 2_000,
            "100k shift updates took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_decision_state_transitions() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let mut last_decision = GateDecision::Permit;
        let mut transitions = 0;

        for i in 0..1_000 {
            // Vary parameters to cause state changes
            let cut_value = 5.0 + (i as f64).sin() * 10.0;
            let shift_score = 0.3 + (i as f64).cos().abs() * 0.4;

            let reports: Vec<TileReport> = (1..=5)
                .map(|j| {
                    let mut report = TileReport::new(j);
                    report.local_cut = cut_value.max(0.1);
                    report.shift_score = shift_score;
                    report.e_value = 200.0;
                    report
                })
                .collect();

            let decision = tilezero.merge_reports(reports);

            if decision != last_decision {
                transitions += 1;
                last_decision = decision;
            }
        }

        // Should have some state transitions
        println!("Decision state transitions: {}", transitions);
        assert!(transitions > 0);
    }
}

// ============================================================================
// Error Recovery Tests
// ============================================================================

mod error_recovery_tests {
    use super::*;

    #[test]
    fn test_structural_filter_edge_operation_errors() {
        let mut filter = StructuralFilter::new(5.0);

        // Duplicate edge
        filter.insert_edge(1, 2, 1.0).unwrap();
        let result = filter.insert_edge(1, 2, 1.0);
        assert!(result.is_err());

        // Delete nonexistent
        let result = filter.delete_edge(5, 6);
        assert!(result.is_err());

        // Filter should still work
        let state = SystemState::new(2);
        let eval = filter.evaluate(&state);
        assert!(eval.compute_time_us < 1_000_000);
    }

    #[test]
    fn test_patch_graph_recovery_from_bad_operations() {
        let mut graph = PatchGraph::new();

        // Add valid edges
        graph.add_edge(0, 1, 100);
        graph.add_edge(1, 2, 100);

        // Try invalid operations
        let _ = graph.add_edge(0, 0, 100); // Self-loop
        let _ = graph.add_edge(MAX_PATCH_VERTICES as u16, 0, 100); // Out of bounds
        let _ = graph.remove_edge(5, 6); // Nonexistent

        // Graph should still be valid
        assert_eq!(graph.num_edges, 2);
        assert!(graph.estimate_local_cut() > 0.0);
    }

    #[test]
    fn test_buffer_recovery_from_rapid_operations() {
        let mut buffer = SyndromeBuffer::new(100);

        // Rapid push/clear cycles
        for cycle in 0..100 {
            for i in 0..50 {
                let round = SyndromeRound::new(
                    cycle * 50 + i,
                    cycle * 50 + i,
                    (cycle * 50 + i) * 1_000,
                    DetectorBitmap::new(64),
                    0,
                );
                buffer.push(round);
            }

            if cycle % 10 == 0 {
                buffer.clear();
            }
        }

        // Buffer should be valid
        assert!(buffer.len() <= 100);
    }

    #[test]
    fn test_worker_tile_reset_recovery() {
        let mut tile = WorkerTile::new(1);

        // Build up state
        for _ in 0..100 {
            let delta = TileSyndromeDelta::new(0, 1, 100);
            tile.tick(&delta);
        }

        // Add graph structure
        tile.patch_graph.add_edge(0, 1, 100);
        tile.patch_graph.add_edge(1, 2, 100);

        // Reset
        tile.reset();

        // Should be clean
        assert_eq!(tile.tick, 0);
        assert_eq!(tile.patch_graph.num_edges, 0);
        assert_eq!(tile.syndrome_buffer.count, 0);

        // Should work again
        let delta = TileSyndromeDelta::new(0, 1, 50);
        let report = tile.tick(&delta);
        assert_eq!(report.tick, 1);
    }

    #[test]
    fn test_filter_pipeline_reset_recovery() {
        let config = FilterConfig::default();
        let mut pipeline = FilterPipeline::new(config);

        // Build up state
        for _ in 0..100 {
            pipeline.shift_mut().update(0, 1.0);
            pipeline.evidence_mut().update(2.0);
        }

        // Reset
        pipeline.reset();

        // Evidence should be back to neutral
        let state = SystemState::new(10);
        let result = pipeline.evaluate(&state);
        assert!((result.evidence.e_value - 1.0).abs() < 0.5);
    }

    #[test]
    fn test_evidence_overflow_protection() {
        let mut acc = EvidenceAccumulator::new();

        // Try to overflow with extreme values
        for _ in 0..1000 {
            acc.update(1e100); // Very large (will be clamped)
        }

        // Should not panic or be NaN/Inf
        assert!(acc.e_value().is_finite());

        // Reset should work
        acc.reset();
        assert_eq!(acc.e_value(), 1.0);
    }

    #[test]
    fn test_evidence_underflow_protection() {
        let mut acc = EvidenceAccumulator::new();

        // Try to underflow with tiny values
        for _ in 0..1000 {
            acc.update(1e-100); // Very small (will be clamped)
        }

        // Should not panic or be NaN/Inf
        assert!(acc.e_value().is_finite());
        assert!(acc.e_value() >= 0.0);
    }
}

// ============================================================================
// Concurrent-Style Stress Tests (Sequential Simulation)
// ============================================================================

mod concurrent_stress_tests {
    use super::*;

    #[test]
    fn test_multiple_workers_same_syndrome_pattern() {
        let mut workers: Vec<WorkerTile> = (1..=10).map(WorkerTile::new).collect();

        // All workers process same syndrome pattern
        for round in 0..100 {
            let delta = TileSyndromeDelta::new(
                (round % 64) as u16,
                ((round + 1) % 64) as u16,
                (round % 256) as u16,
            );

            for worker in &mut workers {
                worker.tick(&delta);
            }
        }

        // All workers should be in sync
        for worker in &workers {
            assert_eq!(worker.tick, 100);
        }
    }

    #[test]
    fn test_multiple_workers_different_patterns() {
        let mut workers: Vec<WorkerTile> = (1..=50).map(WorkerTile::new).collect();

        // Each worker gets unique pattern
        for round in 0..100 {
            for (i, worker) in workers.iter_mut().enumerate() {
                let delta = TileSyndromeDelta::new(
                    ((round + i) % 64) as u16,
                    ((round + i + 1) % 64) as u16,
                    ((round + i) % 256) as u16,
                );
                worker.tick(&delta);
            }
        }

        // All workers should have processed 100 rounds
        for worker in &workers {
            assert_eq!(worker.tick, 100);
        }
    }

    #[test]
    fn test_tilezero_varying_report_counts() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        // Vary the number of reports each cycle
        for i in 0..100 {
            let report_count = 1 + (i % 20);

            let reports: Vec<TileReport> = (1..=report_count as u8)
                .map(|j| {
                    let mut report = TileReport::new(j);
                    report.local_cut = 10.0;
                    report.shift_score = 0.1;
                    report.e_value = 200.0;
                    report
                })
                .collect();

            tilezero.merge_reports(reports);
        }

        assert_eq!(tilezero.receipt_log.len(), 100);
    }

    #[test]
    fn test_interleaved_operations() {
        let mut buffer = SyndromeBuffer::new(100);
        let mut filter = ShiftFilter::new(0.5, 100);
        let mut evidence = EvidenceAccumulator::new();

        // Interleave different operations
        for i in 0..1_000 {
            // Buffer operation
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);

            // Filter operation
            filter.update(i as usize % 64, (i as f64) % 10.0);

            // Evidence operation
            evidence.update(1.0 + (i as f64 % 10.0) / 100.0);

            // Occasional window access
            if i % 100 == 0 {
                let _ = buffer.window(10);
            }
        }

        // All should be functional
        assert_eq!(buffer.len(), 100);
        assert!(evidence.e_value() > 1.0);
    }
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

mod boundary_tests {
    use super::*;

    #[test]
    fn test_empty_state_handling() {
        // Empty filter pipeline
        let config = FilterConfig::default();
        let pipeline = FilterPipeline::new(config);
        let state = SystemState::new(0);
        let result = pipeline.evaluate(&state);
        assert!(result.verdict.is_some());

        // Empty tilezero
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);
        let decision = tilezero.merge_reports(vec![]);
        // Empty reports should produce some decision
        assert!(decision == GateDecision::Permit || decision == GateDecision::Defer);
    }

    #[test]
    fn test_single_element_handling() {
        // Single round buffer
        let mut buffer = SyndromeBuffer::new(1);
        buffer.push(SyndromeRound::new(0, 0, 0, DetectorBitmap::new(64), 0));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.window(1).len(), 1);

        // Single bit bitmap
        let mut bitmap = DetectorBitmap::new(1);
        bitmap.set(0, true);
        assert_eq!(bitmap.fired_count(), 1);

        // Single report
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);
        let mut report = TileReport::new(1);
        report.local_cut = 10.0;
        report.shift_score = 0.1;
        report.e_value = 200.0;
        let decision = tilezero.merge_reports(vec![report]);
        assert_eq!(decision, GateDecision::Permit);
    }

    #[test]
    fn test_maximum_values() {
        // Max detectors
        let mut bitmap = DetectorBitmap::new(1024);
        for i in 0..1024 {
            bitmap.set(i, true);
        }
        assert_eq!(bitmap.fired_count(), 1024);

        // Max tile ID
        let tile = WorkerTile::new(255);
        assert_eq!(tile.tile_id, 255);

        // Very high e-value
        let mut evidence = EvidenceAccumulator::new();
        for _ in 0..100 {
            evidence.update(10.0);
        }
        assert!(evidence.e_value().is_finite());
    }

    #[test]
    fn test_minimum_values() {
        // Min detector count
        let bitmap = DetectorBitmap::new(0);
        assert_eq!(bitmap.fired_count(), 0);

        // Very low e-value
        let mut evidence = EvidenceAccumulator::new();
        for _ in 0..100 {
            evidence.update(0.1);
        }
        let e = evidence.e_value();
        assert!(e.is_finite());
        assert!(e >= 0.0);
    }

    #[test]
    fn test_threshold_boundaries() {
        let thresholds = GateThresholds {
            structural_min_cut: 5.0,
            shift_max: 0.5,
            tau_deny: 0.01,
            tau_permit: 100.0,
            permit_ttl_ns: 4_000_000,
        };
        let mut tilezero = TileZero::new(thresholds);

        // Exactly at threshold
        let mut report = TileReport::new(1);
        report.local_cut = 5.0; // Exactly at threshold
        report.shift_score = 0.5; // Exactly at threshold
        report.e_value = 100.0; // Exactly at threshold

        let decision = tilezero.merge_reports(vec![report]);

        // At threshold behavior
        assert!(decision == GateDecision::Permit || decision == GateDecision::Defer);
    }

    #[test]
    fn test_just_below_thresholds() {
        let thresholds = GateThresholds {
            structural_min_cut: 5.0,
            shift_max: 0.5,
            tau_deny: 0.01,
            tau_permit: 100.0,
            permit_ttl_ns: 4_000_000,
        };
        let mut tilezero = TileZero::new(thresholds);

        // Just below structural threshold
        let mut report = TileReport::new(1);
        report.local_cut = 4.99;
        report.shift_score = 0.1;
        report.e_value = 200.0;

        let decision = tilezero.merge_reports(vec![report]);
        assert_eq!(decision, GateDecision::Deny);
    }
}

// ============================================================================
// Proptest Stress Tests
// ============================================================================

#[cfg(test)]
mod proptest_stress {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn prop_buffer_survives_random_operations(
            pushes in prop::collection::vec(0u64..10000, 100..1000),
            capacity in 10usize..200
        ) {
            let mut buffer = SyndromeBuffer::new(capacity);

            for round_id in pushes {
                let round = SyndromeRound::new(round_id, round_id, round_id * 1000, DetectorBitmap::new(64), 0);
                buffer.push(round);
            }

            // Buffer should be valid
            prop_assert!(buffer.len() <= capacity);
            prop_assert!(!buffer.statistics().avg_firing_rate.is_nan());
        }

        #[test]
        fn prop_worker_survives_random_deltas(
            syndromes in prop::collection::vec((0u16..64, 0u16..64, 0u16..256), 100..500)
        ) {
            let mut worker = WorkerTile::new(1);

            for (src, tgt, val) in syndromes {
                let delta = TileSyndromeDelta::new(src, tgt.max(1), val);
                worker.tick(&delta);
            }

            // Worker should be valid
            prop_assert!(worker.tick > 0);
        }

        #[test]
        fn prop_tilezero_survives_random_reports(
            report_values in prop::collection::vec(
                (0.0f64..20.0, 0.0f64..1.0, 0.01f64..500.0),
                1..50
            )
        ) {
            let thresholds = GateThresholds::default();
            let mut tilezero = TileZero::new(thresholds);

            let reports: Vec<TileReport> = report_values
                .iter()
                .enumerate()
                .map(|(i, (cut, shift, e_val))| {
                    let mut report = TileReport::new((i + 1) as u8);
                    report.local_cut = *cut;
                    report.shift_score = *shift;
                    report.e_value = *e_val;
                    report
                })
                .collect();

            let decision = tilezero.merge_reports(reports);

            // Decision should be valid
            prop_assert!(matches!(decision, GateDecision::Permit | GateDecision::Defer | GateDecision::Deny));
        }
    }
}
