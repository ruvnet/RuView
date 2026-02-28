//! Comprehensive tests for report merging from multiple tiles
//!
//! Tests cover:
//! - Merging strategies (SimpleAverage, WeightedAverage, Median, Maximum, BFT)
//! - Edge cases (empty reports, conflicting epochs)
//! - Node and edge aggregation
//! - Property-based tests for merge invariants

use cognitum_gate_tilezero::merge::{
    EdgeSummary, MergeError, MergeStrategy, MergedReport, NodeSummary, ReportMerger, WorkerReport,
};

fn create_test_report(tile_id: u8, epoch: u64) -> WorkerReport {
    let mut report = WorkerReport::new(tile_id, epoch);
    report.confidence = 0.9;
    report.local_mincut = 1.0;
    report
}

fn add_test_node(report: &mut WorkerReport, id: &str, weight: f64, coherence: f64) {
    report.add_node(NodeSummary {
        id: id.to_string(),
        weight,
        edge_count: 5,
        coherence,
    });
}

fn add_test_boundary_edge(report: &mut WorkerReport, source: &str, target: &str, capacity: f64) {
    report.add_boundary_edge(EdgeSummary {
        source: source.to_string(),
        target: target.to_string(),
        capacity,
        is_boundary: true,
    });
}

#[cfg(test)]
mod basic_merging {
    use super::*;

    #[test]
    fn test_merge_single_report() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);
        let mut report = create_test_report(1, 0);
        add_test_node(&mut report, "node1", 1.0, 0.9);

        let merged = merger.merge(&[report]).unwrap();
        assert_eq!(merged.worker_count, 1);
        assert_eq!(merged.epoch, 0);
        assert!(merged.super_nodes.contains_key("node1"));
    }

    #[test]
    fn test_merge_multiple_reports() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let reports: Vec<_> = (1..=3)
            .map(|i| {
                let mut report = create_test_report(i, 0);
                add_test_node(&mut report, "node1", i as f64 * 0.1, 0.9);
                report
            })
            .collect();

        let merged = merger.merge(&reports).unwrap();
        assert_eq!(merged.worker_count, 3);

        let node = merged.super_nodes.get("node1").unwrap();
        // Average of 0.1, 0.2, 0.3 = 0.2
        assert!((node.weight - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_merge_empty_reports() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);
        let result = merger.merge(&[]);
        assert!(matches!(result, Err(MergeError::EmptyReports)));
    }

    #[test]
    fn test_merge_conflicting_epochs() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);
        let reports = vec![create_test_report(1, 0), create_test_report(2, 1)];

        let result = merger.merge(&reports);
        assert!(matches!(result, Err(MergeError::ConflictingEpochs)));
    }
}

#[cfg(test)]
mod merge_strategies {
    use super::*;

    #[test]
    fn test_simple_average() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let reports: Vec<_> = [1.0, 2.0, 3.0]
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let mut r = create_test_report(i as u8, 0);
                add_test_node(&mut r, "node", w, 0.9);
                r
            })
            .collect();

        let merged = merger.merge(&reports).unwrap();
        let node = merged.super_nodes.get("node").unwrap();
        assert!((node.weight - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_weighted_average() {
        let merger = ReportMerger::new(MergeStrategy::WeightedAverage);

        let mut reports = Vec::new();

        // High coherence node has weight 1.0, low coherence has weight 3.0
        let mut r1 = create_test_report(1, 0);
        add_test_node(&mut r1, "node", 1.0, 0.9);
        reports.push(r1);

        let mut r2 = create_test_report(2, 0);
        add_test_node(&mut r2, "node", 3.0, 0.3);
        reports.push(r2);

        let merged = merger.merge(&reports).unwrap();
        let node = merged.super_nodes.get("node").unwrap();

        // Weight should be biased toward the high-coherence value
        // weighted = (1.0 * 0.9 + 3.0 * 0.3) / (0.9 + 0.3) = 1.8 / 1.2 = 1.5
        assert!((node.weight - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_median() {
        let merger = ReportMerger::new(MergeStrategy::Median);

        let weights = [1.0, 5.0, 2.0, 8.0, 3.0]; // Median = 3.0
        let reports: Vec<_> = weights
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let mut r = create_test_report(i as u8, 0);
                add_test_node(&mut r, "node", w, 0.9);
                r
            })
            .collect();

        let merged = merger.merge(&reports).unwrap();
        let node = merged.super_nodes.get("node").unwrap();
        assert!((node.weight - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_median_even_count() {
        let merger = ReportMerger::new(MergeStrategy::Median);

        let weights = [1.0, 2.0, 3.0, 4.0]; // Median = (2.0 + 3.0) / 2 = 2.5
        let reports: Vec<_> = weights
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let mut r = create_test_report(i as u8, 0);
                add_test_node(&mut r, "node", w, 0.9);
                r
            })
            .collect();

        let merged = merger.merge(&reports).unwrap();
        let node = merged.super_nodes.get("node").unwrap();
        assert!((node.weight - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_maximum() {
        let merger = ReportMerger::new(MergeStrategy::Maximum);

        let weights = [1.0, 5.0, 2.0, 8.0, 3.0];
        let reports: Vec<_> = weights
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let mut r = create_test_report(i as u8, 0);
                add_test_node(&mut r, "node", w, 0.9);
                r
            })
            .collect();

        let merged = merger.merge(&reports).unwrap();
        let node = merged.super_nodes.get("node").unwrap();
        assert!((node.weight - 8.0).abs() < 0.001);
    }

    #[test]
    fn test_byzantine_fault_tolerant() {
        let merger = ReportMerger::new(MergeStrategy::ByzantineFaultTolerant);

        // 6 reports: 4 honest (weight ~2.0), 2 Byzantine (weight 100.0)
        let mut reports = Vec::new();
        for i in 0..4 {
            let mut r = create_test_report(i, 0);
            add_test_node(&mut r, "node", 2.0, 0.9);
            reports.push(r);
        }
        for i in 4..6 {
            let mut r = create_test_report(i, 0);
            add_test_node(&mut r, "node", 100.0, 0.9);
            reports.push(r);
        }

        let merged = merger.merge(&reports).unwrap();
        let node = merged.super_nodes.get("node").unwrap();

        // BFT should exclude Byzantine values (top 2/3 of sorted = 4 lowest)
        // Average of 4 lowest: 2.0
        assert!(node.weight < 50.0); // Should not be influenced by 100.0
    }
}

#[cfg(test)]
mod edge_merging {
    use super::*;

    #[test]
    fn test_merge_boundary_edges() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        add_test_boundary_edge(&mut r1, "A", "B", 1.0);
        add_test_boundary_edge(&mut r1, "B", "C", 2.0);

        let mut r2 = create_test_report(2, 0);
        add_test_boundary_edge(&mut r2, "A", "B", 3.0); // Same edge, different capacity
        add_test_boundary_edge(&mut r2, "C", "D", 4.0);

        let merged = merger.merge(&[r1, r2]).unwrap();

        // Should have 3 unique edges
        assert_eq!(merged.boundary_edges.len(), 3);

        // Find the A-B edge
        let ab_edge = merged
            .boundary_edges
            .iter()
            .find(|e| (e.source == "A" && e.target == "B") || (e.source == "B" && e.target == "A"))
            .unwrap();

        // Average of 1.0 and 3.0 = 2.0
        assert!((ab_edge.capacity - 2.0).abs() < 0.001);
        assert_eq!(ab_edge.report_count, 2);
    }

    #[test]
    fn test_edge_normalization() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        add_test_boundary_edge(&mut r1, "A", "B", 1.0);

        let mut r2 = create_test_report(2, 0);
        add_test_boundary_edge(&mut r2, "B", "A", 1.0); // Reverse order

        let merged = merger.merge(&[r1, r2]).unwrap();

        // Should be recognized as the same edge
        assert_eq!(merged.boundary_edges.len(), 1);
        assert_eq!(merged.boundary_edges[0].report_count, 2);
    }
}

#[cfg(test)]
mod node_aggregation {
    use super::*;

    #[test]
    fn test_contributors_tracked() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        add_test_node(&mut r1, "node", 1.0, 0.9);

        let mut r2 = create_test_report(2, 0);
        add_test_node(&mut r2, "node", 2.0, 0.9);

        let merged = merger.merge(&[r1, r2]).unwrap();
        let node = merged.super_nodes.get("node").unwrap();

        assert!(node.contributors.contains(&1));
        assert!(node.contributors.contains(&2));
    }

    #[test]
    fn test_edge_count_summed() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        r1.add_node(NodeSummary {
            id: "node".to_string(),
            weight: 1.0,
            edge_count: 10,
            coherence: 0.9,
        });

        let mut r2 = create_test_report(2, 0);
        r2.add_node(NodeSummary {
            id: "node".to_string(),
            weight: 1.0,
            edge_count: 20,
            coherence: 0.9,
        });

        let merged = merger.merge(&[r1, r2]).unwrap();
        let node = merged.super_nodes.get("node").unwrap();

        assert_eq!(node.total_edge_count, 30);
    }

    #[test]
    fn test_coherence_averaged() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        r1.add_node(NodeSummary {
            id: "node".to_string(),
            weight: 1.0,
            edge_count: 5,
            coherence: 0.8,
        });

        let mut r2 = create_test_report(2, 0);
        r2.add_node(NodeSummary {
            id: "node".to_string(),
            weight: 1.0,
            edge_count: 5,
            coherence: 0.6,
        });

        let merged = merger.merge(&[r1, r2]).unwrap();
        let node = merged.super_nodes.get("node").unwrap();

        assert!((node.avg_coherence - 0.7).abs() < 0.001);
    }
}

#[cfg(test)]
mod global_mincut_estimate {
    use super::*;

    #[test]
    fn test_mincut_from_local_values() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut reports = Vec::new();
        for i in 0..3 {
            let mut r = create_test_report(i, 0);
            r.local_mincut = 1.0 + i as f64;
            reports.push(r);
        }

        let merged = merger.merge(&reports).unwrap();

        // Should have some estimate based on local values
        assert!(merged.global_mincut_estimate > 0.0);
    }

    #[test]
    fn test_mincut_with_boundaries() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        r1.local_mincut = 5.0;
        add_test_boundary_edge(&mut r1, "A", "B", 1.0);

        let merged = merger.merge(&[r1]).unwrap();

        // Boundary edges should affect the estimate
        assert!(merged.global_mincut_estimate > 0.0);
    }
}

#[cfg(test)]
mod confidence_aggregation {
    use super::*;

    #[test]
    fn test_geometric_mean_confidence() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut reports = Vec::new();
        for i in 0..3 {
            let mut r = create_test_report(i, 0);
            r.confidence = 0.8;
            reports.push(r);
        }

        let merged = merger.merge(&reports).unwrap();

        // Geometric mean of [0.8, 0.8, 0.8] = 0.8
        assert!((merged.confidence - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_bft_confidence() {
        let merger = ReportMerger::new(MergeStrategy::ByzantineFaultTolerant);

        let mut reports = Vec::new();
        let confidences = [0.9, 0.85, 0.88, 0.2, 0.1]; // Two low-confidence outliers

        for (i, &c) in confidences.iter().enumerate() {
            let mut r = create_test_report(i as u8, 0);
            r.confidence = c;
            reports.push(r);
        }

        let merged = merger.merge(&reports).unwrap();

        // BFT should use conservative estimate (minimum of top 2/3)
        assert!(merged.confidence > 0.5); // Should not be dragged down by 0.1, 0.2
    }
}

#[cfg(test)]
mod state_hash {
    use super::*;

    #[test]
    fn test_state_hash_computed() {
        let mut report = create_test_report(1, 0);
        add_test_node(&mut report, "node1", 1.0, 0.9);

        report.compute_state_hash();
        assert_ne!(report.state_hash, [0u8; 32]);
    }

    #[test]
    fn test_state_hash_deterministic() {
        let mut r1 = create_test_report(1, 0);
        add_test_node(&mut r1, "node1", 1.0, 0.9);
        r1.compute_state_hash();

        let mut r2 = create_test_report(1, 0);
        add_test_node(&mut r2, "node1", 1.0, 0.9);
        r2.compute_state_hash();

        assert_eq!(r1.state_hash, r2.state_hash);
    }

    #[test]
    fn test_state_hash_changes_with_data() {
        let mut r1 = create_test_report(1, 0);
        add_test_node(&mut r1, "node1", 1.0, 0.9);
        r1.compute_state_hash();

        let mut r2 = create_test_report(1, 0);
        add_test_node(&mut r2, "node1", 2.0, 0.9); // Different weight
        r2.compute_state_hash();

        assert_ne!(r1.state_hash, r2.state_hash);
    }
}

#[cfg(test)]
mod multiple_nodes {
    use super::*;

    #[test]
    fn test_merge_disjoint_nodes() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        add_test_node(&mut r1, "node_a", 1.0, 0.9);

        let mut r2 = create_test_report(2, 0);
        add_test_node(&mut r2, "node_b", 2.0, 0.9);

        let merged = merger.merge(&[r1, r2]).unwrap();

        assert!(merged.super_nodes.contains_key("node_a"));
        assert!(merged.super_nodes.contains_key("node_b"));
        assert_eq!(merged.super_nodes.len(), 2);
    }

    #[test]
    fn test_merge_overlapping_nodes() {
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        let mut r1 = create_test_report(1, 0);
        add_test_node(&mut r1, "shared", 1.0, 0.9);
        add_test_node(&mut r1, "only_r1", 2.0, 0.9);

        let mut r2 = create_test_report(2, 0);
        add_test_node(&mut r2, "shared", 3.0, 0.9);
        add_test_node(&mut r2, "only_r2", 4.0, 0.9);

        let merged = merger.merge(&[r1, r2]).unwrap();

        assert_eq!(merged.super_nodes.len(), 3);

        let shared = merged.super_nodes.get("shared").unwrap();
        assert!((shared.weight - 2.0).abs() < 0.001); // Average of 1.0 and 3.0
    }
}

// Property-based tests
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_merge_preserves_epoch(epoch in 0u64..1000) {
            let merger = ReportMerger::new(MergeStrategy::SimpleAverage);
            let r1 = create_test_report(1, epoch);
            let r2 = create_test_report(2, epoch);

            let merged = merger.merge(&[r1, r2]).unwrap();
            assert_eq!(merged.epoch, epoch);
        }

        #[test]
        fn prop_merge_counts_workers(n in 1usize..10) {
            let merger = ReportMerger::new(MergeStrategy::SimpleAverage);
            let reports: Vec<_> = (0..n)
                .map(|i| create_test_report(i as u8, 0))
                .collect();

            let merged = merger.merge(&reports).unwrap();
            assert_eq!(merged.worker_count, n);
        }

        #[test]
        fn prop_average_in_range(weights in proptest::collection::vec(0.1f64..100.0, 2..10)) {
            let merger = ReportMerger::new(MergeStrategy::SimpleAverage);
            let reports: Vec<_> = weights
                .iter()
                .enumerate()
                .map(|(i, &w)| {
                    let mut r = create_test_report(i as u8, 0);
                    add_test_node(&mut r, "node", w, 0.9);
                    r
                })
                .collect();

            let merged = merger.merge(&reports).unwrap();
            let node = merged.super_nodes.get("node").unwrap();

            let min = weights.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = weights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            assert!(node.weight >= min);
            assert!(node.weight <= max);
        }

        #[test]
        fn prop_maximum_is_largest(weights in proptest::collection::vec(0.1f64..100.0, 2..10)) {
            let merger = ReportMerger::new(MergeStrategy::Maximum);
            let reports: Vec<_> = weights
                .iter()
                .enumerate()
                .map(|(i, &w)| {
                    let mut r = create_test_report(i as u8, 0);
                    add_test_node(&mut r, "node", w, 0.9);
                    r
                })
                .collect();

            let merged = merger.merge(&reports).unwrap();
            let node = merged.super_nodes.get("node").unwrap();

            let max = weights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            assert!((node.weight - max).abs() < 0.001);
        }
    }
}
