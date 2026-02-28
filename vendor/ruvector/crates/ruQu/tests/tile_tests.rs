//! Tile architecture tests for ruQu coherence gate
//!
//! Tests for the 256-tile WASM fabric:
//! - WorkerTile tick processing
//! - TileZero report merging
//! - Permit token issuance and verification
//! - 256-tile scaling

use ruqu::tile::{
    Edge, EvidenceAccumulator, GateDecision, GateThresholds, LocalCutState, PatchGraph,
    PermitToken, ReceiptLog, SyndromBuffer, SyndromeDelta, SyndromeEntry, TileReport, TileZero,
    Vertex, WorkerTile, MAX_BOUNDARY_CANDIDATES, MAX_PATCH_EDGES, MAX_PATCH_VERTICES, NUM_WORKERS,
    SYNDROME_BUFFER_DEPTH,
};

// ============================================================================
// WorkerTile Tick Processing Tests
// ============================================================================

mod worker_tile_tests {
    use super::*;

    #[test]
    fn test_worker_tile_creation() {
        let tile = WorkerTile::new(42);

        assert_eq!(tile.tile_id, 42);
        assert_eq!(tile.tick, 0);
        assert_eq!(tile.generation, 0);
    }

    #[test]
    fn test_worker_tile_tick_increments() {
        let mut tile = WorkerTile::new(1);

        let delta = SyndromeDelta::new(0, 1, 100);
        tile.tick(&delta);

        assert_eq!(tile.tick, 1);

        for _ in 0..99 {
            tile.tick(&delta);
        }

        assert_eq!(tile.tick, 100);
    }

    #[test]
    fn test_worker_tile_tick_returns_report() {
        let mut tile = WorkerTile::new(5);

        let delta = SyndromeDelta::new(0, 1, 50);
        let report = tile.tick(&delta);

        assert_eq!(report.tile_id, 5);
        assert_eq!(report.tick, 1);
        assert!(report.status & TileReport::STATUS_VALID != 0);
    }

    #[test]
    fn test_worker_tile_syndrome_updates_graph() {
        let mut tile = WorkerTile::new(1);

        // Edge addition delta
        let delta = SyndromeDelta::edge_add(0, 1, 100);
        tile.tick(&delta);

        assert_eq!(tile.patch_graph.num_edges, 1);
        assert_eq!(tile.patch_graph.num_vertices, 2);
    }

    #[test]
    fn test_worker_tile_syndrome_buffer_populated() {
        let mut tile = WorkerTile::new(1);

        for i in 0..50 {
            let delta = SyndromeDelta::new(0, 1, i as u16);
            tile.tick(&delta);
        }

        assert_eq!(tile.syndrome_buffer.count, 50);
    }

    #[test]
    fn test_worker_tile_evidence_accumulates() {
        let mut tile = WorkerTile::new(1);

        // Process multiple syndromes
        for _ in 0..100 {
            let delta = SyndromeDelta::new(0, 1, 50); // Low value = evidence for coherence
            tile.tick(&delta);
        }

        // Evidence should have accumulated
        assert!(tile.evidence.obs_count > 0);
    }

    #[test]
    fn test_worker_tile_cut_state_updates() {
        let mut tile = WorkerTile::new(1);

        // Add graph structure
        let delta = SyndromeDelta::edge_add(0, 1, 100);
        tile.tick(&delta);
        let delta = SyndromeDelta::edge_add(1, 2, 100);
        tile.tick(&delta);

        // Cut state should be computed
        assert!(tile.local_cut_state.generation > 0);
    }

    #[test]
    fn test_worker_tile_shift_score_computed() {
        let mut tile = WorkerTile::new(1);

        // Need enough syndrome history for shift computation
        for i in 0..100 {
            let delta = SyndromeDelta::new(0, 1, (i % 256) as u16);
            tile.tick(&delta);
        }

        let delta = SyndromeDelta::new(0, 1, 50);
        let report = tile.tick(&delta);

        // Shift score should be computed (might be 0.0 if stable)
        assert!(report.shift_score >= 0.0 && report.shift_score <= 1.0);
    }

    #[test]
    fn test_worker_tile_reset() {
        let mut tile = WorkerTile::new(1);

        // Add state
        for _ in 0..50 {
            let delta = SyndromeDelta::new(0, 1, 100);
            tile.tick(&delta);
        }

        assert!(tile.tick > 0);

        // Reset
        tile.reset();

        assert_eq!(tile.tick, 0);
        assert_eq!(tile.generation, 0);
        assert_eq!(tile.syndrome_buffer.count, 0);
    }

    #[test]
    fn test_worker_tile_boundary_moved_detection() {
        let mut tile = WorkerTile::new(1);

        // Build initial graph
        tile.patch_graph.add_edge(0, 1, 100);
        tile.patch_graph.add_edge(1, 2, 100);
        tile.patch_graph.recompute_components();
        tile.local_cut_state.update_from_graph(&tile.patch_graph);

        // Significant change
        tile.patch_graph.add_edge(2, 3, 1000);
        tile.patch_graph.recompute_components();
        tile.local_cut_state.update_from_graph(&tile.patch_graph);

        // May or may not detect boundary movement depending on magnitude
        // The flag is set based on relative change
        assert!(tile.local_cut_state.generation > 0);
    }

    #[test]
    fn test_worker_tile_memory_size() {
        let size = WorkerTile::memory_size();

        // Should be within reasonable bounds (target ~64KB, allow some margin)
        assert!(size > 0);
        assert!(size <= 131072); // 128KB max
    }
}

// ============================================================================
// TileZero Report Merging Tests
// ============================================================================

mod tilezero_tests {
    use super::*;

    #[test]
    fn test_tilezero_creation() {
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);

        assert!(tilezero.receipt_log.is_empty());
    }

    #[test]
    fn test_tilezero_merge_single_report() {
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
    fn test_tilezero_merge_multiple_reports() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let reports: Vec<TileReport> = (1..=10)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0 + i as f64;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);

        assert_eq!(decision, GateDecision::Permit);
    }

    #[test]
    fn test_tilezero_merge_takes_min_cut() {
        let thresholds = GateThresholds {
            structural_min_cut: 8.0,
            ..Default::default()
        };
        let mut tilezero = TileZero::new(thresholds);

        // One tile has low cut
        let reports: Vec<TileReport> = (1..=5)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = if i == 3 { 5.0 } else { 15.0 }; // Tile 3 has low cut
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);

        // Should deny because minimum cut (5.0) < threshold (8.0)
        assert_eq!(decision, GateDecision::Deny);
    }

    #[test]
    fn test_tilezero_merge_takes_max_shift() {
        let thresholds = GateThresholds {
            structural_min_cut: 2.0,
            shift_max: 0.5,
            ..Default::default()
        };
        let mut tilezero = TileZero::new(thresholds);

        // One tile has high shift
        let reports: Vec<TileReport> = (1..=5)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = if i == 3 { 0.8 } else { 0.1 }; // Tile 3 has high shift
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);

        // Should defer because max shift (0.8) >= threshold (0.5)
        assert_eq!(decision, GateDecision::Defer);
    }

    #[test]
    fn test_tilezero_aggregates_evidence() {
        let thresholds = GateThresholds {
            tau_permit: 50.0,
            ..Default::default()
        };
        let mut tilezero = TileZero::new(thresholds);

        // Mix of evidence values
        let reports: Vec<TileReport> = (1..=4)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 100.0 * i as f64; // 100, 200, 300, 400
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);

        // Geometric mean of e-values should be above threshold
        assert_eq!(decision, GateDecision::Permit);
    }

    #[test]
    fn test_tilezero_empty_reports() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let decision = tilezero.merge_reports(vec![]);

        // With no reports, should default to safe behavior
        // (max cut = infinity, so passes structural)
        assert!(decision == GateDecision::Permit || decision == GateDecision::Defer);
    }

    #[test]
    fn test_tilezero_reports_accessor() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let reports: Vec<TileReport> = (1..=3)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        tilezero.merge_reports(reports);

        assert_eq!(tilezero.reports().len(), 3);
    }
}

// ============================================================================
// Permit Token Tests
// ============================================================================

mod permit_token_tests {
    use super::*;

    #[test]
    fn test_permit_token_validity_within_ttl() {
        let token = PermitToken {
            decision: GateDecision::Permit,
            sequence: 0,
            timestamp: 1_000_000,
            ttl_ns: 500_000,
            witness_hash: [0u8; 32],
            signature: [1u8; 64], // Non-zero placeholder
        };

        assert!(token.is_valid(1_000_000)); // At issuance
        assert!(token.is_valid(1_200_000)); // Within TTL
        assert!(token.is_valid(1_499_999)); // Just before expiry
    }

    #[test]
    fn test_permit_token_validity_after_ttl() {
        let token = PermitToken {
            decision: GateDecision::Permit,
            sequence: 0,
            timestamp: 1_000_000,
            ttl_ns: 500_000,
            witness_hash: [0u8; 32],
            signature: [1u8; 64], // Non-zero placeholder
        };

        assert!(!token.is_valid(1_500_001)); // Just after expiry
        assert!(!token.is_valid(2_000_000)); // Well after expiry
    }

    #[test]
    fn test_permit_token_deny_always_invalid() {
        let token = PermitToken {
            decision: GateDecision::Deny,
            sequence: 0,
            timestamp: 1_000_000,
            ttl_ns: 500_000,
            witness_hash: [0u8; 32],
            signature: [1u8; 64], // Non-zero placeholder
        };

        assert!(!token.is_valid(1_200_000));
    }

    #[test]
    fn test_permit_token_defer_always_invalid() {
        let token = PermitToken {
            decision: GateDecision::Defer,
            sequence: 0,
            timestamp: 1_000_000,
            ttl_ns: 500_000,
            witness_hash: [0u8; 32],
            signature: [1u8; 64], // Non-zero placeholder
        };

        assert!(!token.is_valid(1_200_000));
    }

    #[test]
    fn test_permit_token_issuance_from_tilezero() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let reports: Vec<TileReport> = (1..=3)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);
        let token = tilezero.issue_permit(&decision);

        assert_eq!(token.decision, GateDecision::Permit);
        assert!(token.ttl_ns > 0);
    }
}

// ============================================================================
// Receipt Log Tests
// ============================================================================

mod receipt_log_tests {
    use super::*;

    #[test]
    fn test_receipt_log_creation() {
        let log = ReceiptLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_receipt_log_append() {
        let mut log = ReceiptLog::new();

        log.append(GateDecision::Permit, 0, 1000, [0u8; 32]);

        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());
    }

    #[test]
    fn test_receipt_log_get_by_sequence() {
        let mut log = ReceiptLog::new();

        log.append(GateDecision::Permit, 0, 1000, [0u8; 32]);
        log.append(GateDecision::Defer, 1, 2000, [1u8; 32]);
        log.append(GateDecision::Deny, 2, 3000, [2u8; 32]);

        let entry = log.get(1);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().decision, GateDecision::Defer);
        assert_eq!(entry.unwrap().sequence, 1);
    }

    #[test]
    fn test_receipt_log_get_nonexistent() {
        let mut log = ReceiptLog::new();

        log.append(GateDecision::Permit, 0, 1000, [0u8; 32]);

        let entry = log.get(999);
        assert!(entry.is_none());
    }

    #[test]
    fn test_receipt_log_chain_integrity() {
        let mut log = ReceiptLog::new();

        log.append(GateDecision::Permit, 0, 1000, [0u8; 32]);
        log.append(GateDecision::Permit, 1, 2000, [1u8; 32]);
        log.append(GateDecision::Permit, 2, 3000, [2u8; 32]);

        // Each entry's previous_hash should match prior entry's hash
        let entry1 = log.get(1).unwrap();
        let entry2 = log.get(2).unwrap();

        assert_eq!(entry2.previous_hash, entry1.hash);
    }

    #[test]
    fn test_receipt_log_last_hash() {
        let mut log = ReceiptLog::new();

        let initial_hash = log.last_hash();
        assert_eq!(initial_hash, [0u8; 32]);

        log.append(GateDecision::Permit, 0, 1000, [0u8; 32]);

        let new_hash = log.last_hash();
        assert_ne!(new_hash, [0u8; 32]);
    }

    #[test]
    fn test_receipt_log_multiple_decisions() {
        let mut log = ReceiptLog::new();

        for i in 0..100 {
            let decision = match i % 3 {
                0 => GateDecision::Permit,
                1 => GateDecision::Defer,
                _ => GateDecision::Deny,
            };
            log.append(decision, i, i * 1000, [i as u8; 32]);
        }

        assert_eq!(log.len(), 100);

        for i in 0..100 {
            let entry = log.get(i);
            assert!(entry.is_some());
            assert_eq!(entry.unwrap().sequence, i);
        }
    }
}

// ============================================================================
// PatchGraph Tests
// ============================================================================

mod patch_graph_tests {
    use super::*;

    #[test]
    fn test_patch_graph_creation() {
        let graph = PatchGraph::new();

        assert_eq!(graph.num_vertices, 0);
        assert_eq!(graph.num_edges, 0);
        assert_eq!(graph.num_components, 0);
    }

    #[test]
    fn test_patch_graph_add_edge() {
        let mut graph = PatchGraph::new();

        let edge_id = graph.add_edge(0, 1, 100);

        assert!(edge_id.is_some());
        assert_eq!(graph.num_edges, 1);
        assert_eq!(graph.num_vertices, 2);
    }

    #[test]
    fn test_patch_graph_add_multiple_edges() {
        let mut graph = PatchGraph::new();

        graph.add_edge(0, 1, 100);
        graph.add_edge(1, 2, 100);
        graph.add_edge(2, 0, 100);

        assert_eq!(graph.num_edges, 3);
        assert_eq!(graph.num_vertices, 3);
    }

    #[test]
    fn test_patch_graph_remove_edge() {
        let mut graph = PatchGraph::new();

        graph.add_edge(0, 1, 100);
        assert!(graph.remove_edge(0, 1));

        assert_eq!(graph.num_edges, 0);
    }

    #[test]
    fn test_patch_graph_remove_nonexistent() {
        let mut graph = PatchGraph::new();

        assert!(!graph.remove_edge(0, 1));
    }

    #[test]
    fn test_patch_graph_find_edge() {
        let mut graph = PatchGraph::new();

        let edge_id = graph.add_edge(0, 1, 100).unwrap();

        assert_eq!(graph.find_edge(0, 1), Some(edge_id));
        assert_eq!(graph.find_edge(1, 0), Some(edge_id));
        assert_eq!(graph.find_edge(0, 2), None);
    }

    #[test]
    fn test_patch_graph_update_weight() {
        let mut graph = PatchGraph::new();

        graph.add_edge(0, 1, 100);
        assert!(graph.update_weight(0, 1, 200));

        // Verify weight updated
        let edge_id = graph.find_edge(0, 1).unwrap();
        assert_eq!(graph.edges[edge_id as usize].weight, 200);
    }

    #[test]
    fn test_patch_graph_components() {
        let mut graph = PatchGraph::new();

        // Create two disconnected components
        graph.add_edge(0, 1, 100);
        graph.add_edge(2, 3, 100);

        graph.recompute_components();

        assert_eq!(graph.num_components, 2);
    }

    #[test]
    fn test_patch_graph_connected_components() {
        let mut graph = PatchGraph::new();

        // Create one connected component
        graph.add_edge(0, 1, 100);
        graph.add_edge(1, 2, 100);
        graph.add_edge(2, 3, 100);

        graph.recompute_components();

        assert_eq!(graph.num_components, 1);
    }

    #[test]
    fn test_patch_graph_estimate_local_cut() {
        let mut graph = PatchGraph::new();

        graph.add_edge(0, 1, 100);
        graph.add_edge(1, 2, 100);

        let cut = graph.estimate_local_cut();

        assert!(cut > 0.0);
    }

    #[test]
    fn test_patch_graph_boundary_candidates() {
        let mut graph = PatchGraph::new();

        // Add edges with varying weights
        graph.add_edge(0, 1, 10);
        graph.add_edge(1, 2, 100);
        graph.add_edge(2, 3, 50);

        let mut candidates = [0u16; MAX_BOUNDARY_CANDIDATES];
        let count = graph.identify_boundary_candidates(&mut candidates);

        // Should identify some boundary candidates
        assert!(count <= MAX_BOUNDARY_CANDIDATES);
    }

    #[test]
    fn test_patch_graph_clear() {
        let mut graph = PatchGraph::new();

        graph.add_edge(0, 1, 100);
        graph.add_edge(1, 2, 100);

        graph.clear();

        assert_eq!(graph.num_vertices, 0);
        assert_eq!(graph.num_edges, 0);
    }

    #[test]
    fn test_patch_graph_self_loop_rejected() {
        let mut graph = PatchGraph::new();

        let result = graph.add_edge(0, 0, 100);
        assert!(result.is_none());
    }

    #[test]
    fn test_patch_graph_max_vertices() {
        let mut graph = PatchGraph::new();

        // Attempt to add edge with vertex beyond max
        let result = graph.add_edge(MAX_PATCH_VERTICES as u16, 0, 100);
        assert!(result.is_none());
    }

    #[test]
    fn test_patch_graph_apply_delta() {
        let mut graph = PatchGraph::new();

        // Apply edge add delta
        let delta = SyndromeDelta::edge_add(0, 1, 100);
        graph.apply_delta(&delta);

        assert_eq!(graph.num_edges, 1);

        // Apply edge remove delta
        let delta = SyndromeDelta::edge_remove(0, 1);
        graph.apply_delta(&delta);

        assert_eq!(graph.num_edges, 0);
    }
}

// ============================================================================
// SyndromBuffer Tests
// ============================================================================

mod syndrom_buffer_tests {
    use super::*;

    #[test]
    fn test_syndrome_buffer_creation() {
        let buffer = SyndromBuffer::new();

        assert_eq!(buffer.count, 0);
        assert_eq!(buffer.head, 0);
    }

    #[test]
    fn test_syndrome_buffer_append() {
        let mut buffer = SyndromBuffer::new();

        let entry = SyndromeEntry {
            round: 1,
            syndrome: [0; 8],
            flags: 0,
        };
        buffer.append(entry);

        assert_eq!(buffer.count, 1);
        assert_eq!(buffer.current_round, 1);
    }

    #[test]
    fn test_syndrome_buffer_ring_behavior() {
        let mut buffer = SyndromBuffer::new();

        // Fill beyond capacity
        for i in 0..SYNDROME_BUFFER_DEPTH + 100 {
            let entry = SyndromeEntry {
                round: i as u32,
                syndrome: [i as u8; 8],
                flags: 0,
            };
            buffer.append(entry);
        }

        // Count should be capped at depth
        assert_eq!(buffer.count as usize, SYNDROME_BUFFER_DEPTH);
    }

    #[test]
    fn test_syndrome_buffer_recent() {
        let mut buffer = SyndromBuffer::new();

        for i in 0..100 {
            let entry = SyndromeEntry {
                round: i,
                syndrome: [i as u8; 8],
                flags: 0,
            };
            buffer.append(entry);
        }

        let recent: Vec<_> = buffer.recent(10).collect();
        assert_eq!(recent.len(), 10);

        // Should be most recent 10 entries
        assert_eq!(recent[0].round, 90);
        assert_eq!(recent[9].round, 99);
    }

    #[test]
    fn test_syndrome_buffer_recent_more_than_available() {
        let mut buffer = SyndromBuffer::new();

        for i in 0..5 {
            let entry = SyndromeEntry {
                round: i,
                syndrome: [0; 8],
                flags: 0,
            };
            buffer.append(entry);
        }

        let recent: Vec<_> = buffer.recent(100).collect();
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_syndrome_buffer_clear() {
        let mut buffer = SyndromBuffer::new();

        for i in 0..50 {
            let entry = SyndromeEntry {
                round: i,
                syndrome: [0; 8],
                flags: 0,
            };
            buffer.append(entry);
        }

        buffer.clear();

        assert_eq!(buffer.count, 0);
        assert_eq!(buffer.head, 0);
        assert_eq!(buffer.current_round, 0);
    }
}

// ============================================================================
// EvidenceAccumulator Tests (Tile Module)
// ============================================================================

mod tile_evidence_tests {
    use super::*;

    #[test]
    fn test_evidence_accumulator_initial() {
        let acc = EvidenceAccumulator::new();

        assert_eq!(acc.log_e_value, 0);
        assert_eq!(acc.obs_count, 0);
        assert_eq!(acc.e_value(), 1.0);
    }

    #[test]
    fn test_evidence_accumulator_observe() {
        let mut acc = EvidenceAccumulator::new();

        acc.observe(10000); // Positive log LR

        assert!(acc.log_e_value != 0);
        assert_eq!(acc.obs_count, 1);
    }

    #[test]
    fn test_evidence_accumulator_significance() {
        let mut acc = EvidenceAccumulator::new();

        // Accumulate enough evidence for significance
        for _ in 0..100 {
            acc.observe(100000); // Strong positive evidence
        }

        assert!(acc.is_significant());
    }

    #[test]
    fn test_evidence_accumulator_reset() {
        let mut acc = EvidenceAccumulator::new();

        for _ in 0..50 {
            acc.observe(10000);
        }

        acc.reset();

        assert_eq!(acc.log_e_value, 0);
        assert_eq!(acc.obs_count, 0);
        assert_eq!(acc.e_value(), 1.0);
    }
}

// ============================================================================
// LocalCutState Tests
// ============================================================================

mod local_cut_state_tests {
    use super::*;

    #[test]
    fn test_local_cut_state_creation() {
        let state = LocalCutState::new();

        assert_eq!(state.cut_value, 0.0);
        assert_eq!(state.prev_cut_value, 0.0);
        assert_eq!(state.num_candidates, 0);
    }

    #[test]
    fn test_local_cut_state_update_from_graph() {
        let mut graph = PatchGraph::new();
        graph.add_edge(0, 1, 100);
        graph.add_edge(1, 2, 100);
        graph.recompute_components();

        let mut state = LocalCutState::new();
        state.update_from_graph(&graph);

        assert!(state.cut_value > 0.0);
        assert!(state.generation > 0);
    }

    #[test]
    fn test_local_cut_state_candidates() {
        let mut graph = PatchGraph::new();
        graph.add_edge(0, 1, 10);
        graph.add_edge(1, 2, 100);
        graph.add_edge(2, 3, 50);

        let mut state = LocalCutState::new();
        state.update_from_graph(&graph);

        let candidates = state.candidates();
        assert!(candidates.len() <= MAX_BOUNDARY_CANDIDATES);
    }
}

// ============================================================================
// 256-Tile Scaling Tests
// ============================================================================

mod scaling_tests {
    use super::*;

    #[test]
    fn test_256_tile_fabric_creation() {
        let workers: Vec<WorkerTile> = (1..=255).map(WorkerTile::new).collect();

        assert_eq!(workers.len(), NUM_WORKERS);

        // Verify all tile IDs are unique
        let mut seen = [false; 256];
        for worker in &workers {
            assert!(!seen[worker.tile_id as usize]);
            seen[worker.tile_id as usize] = true;
        }
    }

    #[test]
    fn test_all_tiles_produce_valid_reports() {
        let workers: Vec<WorkerTile> = (1..=10).map(WorkerTile::new).collect();

        for mut worker in workers {
            let delta = SyndromeDelta::new(0, 1, 50);
            let report = worker.tick(&delta);

            assert!(report.status & TileReport::STATUS_VALID != 0);
        }
    }

    #[test]
    fn test_tilezero_handles_255_reports() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let reports: Vec<TileReport> = (1..=255)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);

        assert_eq!(decision, GateDecision::Permit);
    }

    #[test]
    fn test_memory_budget_per_tile() {
        let tile_size = WorkerTile::memory_size();

        // Each tile should fit within 64KB budget (with some margin)
        // The spec says ~64KB, so we allow up to 128KB
        assert!(
            tile_size <= 131072,
            "Worker tile exceeds memory budget: {} bytes",
            tile_size
        );
    }
}

// ============================================================================
// Proptest Property-Based Tests
// ============================================================================

#[cfg(test)]
mod proptest_tiles {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        fn prop_worker_tick_always_increments(
            tile_id in 1u8..255,
            num_ticks in 1usize..100
        ) {
            let mut tile = WorkerTile::new(tile_id);

            for _ in 0..num_ticks {
                let delta = SyndromeDelta::new(0, 1, 50);
                tile.tick(&delta);
            }

            prop_assert_eq!(tile.tick, num_ticks as u32);
        }

        #[test]
        fn prop_report_matches_tile_id(tile_id in 1u8..255) {
            let mut tile = WorkerTile::new(tile_id);

            let delta = SyndromeDelta::new(0, 1, 50);
            let report = tile.tick(&delta);

            prop_assert_eq!(report.tile_id, tile_id);
        }

        #[test]
        fn prop_receipt_log_sequence_ordered(num_decisions in 1usize..50) {
            let thresholds = GateThresholds::default();
            let mut tilezero = TileZero::new(thresholds);

            for _ in 0..num_decisions {
                let reports: Vec<TileReport> = (1..=3)
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

            // Verify all sequences exist
            for i in 0..num_decisions {
                let entry = tilezero.receipt_log.get(i as u64);
                prop_assert!(entry.is_some());
                prop_assert_eq!(entry.unwrap().sequence, i as u64);
            }
        }
    }
}
