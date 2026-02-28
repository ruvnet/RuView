#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Integration tests for speculative decoding
//!
//! These tests verify the speculative decoding implementation works correctly
//! with mock backends.

use ruvllm::speculative::{
    log_softmax, softmax, top_k_filter, top_p_filter, AtomicSpeculativeStats, SpeculationTree,
    SpeculativeConfig, SpeculativeStats, TreeNode, VerificationResult,
};
use std::time::Duration;

#[test]
fn test_speculative_config_defaults() {
    let config = SpeculativeConfig::default();

    assert_eq!(config.lookahead, 4);
    assert!((config.acceptance_threshold - 0.5).abs() < 0.01);
    assert!((config.draft_temperature - 0.0).abs() < 0.01);
    assert!(!config.tree_speculation);
    assert_eq!(config.max_tree_depth, 3);
    assert_eq!(config.tree_branching_factor, 2);
    assert!(config.adaptive_lookahead);
    assert_eq!(config.min_lookahead, 2);
    assert_eq!(config.max_lookahead, 8);
}

#[test]
fn test_speculative_config_custom() {
    let config = SpeculativeConfig {
        lookahead: 6,
        acceptance_threshold: 0.7,
        draft_temperature: 0.1,
        tree_speculation: true,
        max_tree_depth: 4,
        tree_branching_factor: 3,
        draft_top_p: 0.9,
        min_acceptance_ratio: 0.2,
        adaptive_lookahead: false,
        min_lookahead: 3,
        max_lookahead: 10,
    };

    assert_eq!(config.lookahead, 6);
    assert!((config.acceptance_threshold - 0.7).abs() < 0.01);
    assert!(config.tree_speculation);
}

#[test]
fn test_speculative_stats_empty() {
    let stats = SpeculativeStats::new();

    assert_eq!(stats.draft_tokens, 0);
    assert_eq!(stats.accepted_tokens, 0);
    assert!((stats.acceptance_rate - 0.0).abs() < 0.01);
    assert!((stats.speedup - 0.0).abs() < 0.01);
    assert_eq!(stats.main_forward_passes, 0);
}

#[test]
fn test_speculative_stats_record_round() {
    let mut stats = SpeculativeStats::new();

    // Simulate a round: 4 draft tokens, 3 accepted
    stats.record_round(4, 3, 10.0);

    assert_eq!(stats.draft_tokens, 4);
    assert_eq!(stats.accepted_tokens, 3);
    assert!((stats.acceptance_rate - 0.75).abs() < 0.01);
    assert_eq!(stats.main_forward_passes, 1);
    assert_eq!(stats.draft_forward_passes, 4);
    assert_eq!(stats.total_tokens_generated, 4); // 3 accepted + 1 correction

    // Simulate another round: 4 draft, 2 accepted
    stats.record_round(4, 2, 12.0);

    assert_eq!(stats.draft_tokens, 8);
    assert_eq!(stats.accepted_tokens, 5);
    assert!((stats.acceptance_rate - 0.625).abs() < 0.01);
    assert_eq!(stats.main_forward_passes, 2);
    assert_eq!(stats.total_tokens_generated, 7);
}

#[test]
fn test_speculative_stats_speedup_calculation() {
    let mut stats = SpeculativeStats::new();

    // Perfect speculation: all accepted
    stats.record_round(4, 4, 10.0);

    // 5 tokens per pass (4 accepted + 1 continuation)
    assert!((stats.avg_tokens_per_main_pass - 5.0).abs() < 0.01);
    assert!((stats.speedup - 5.0).abs() < 0.01);
}

#[test]
fn test_atomic_speculative_stats() {
    let stats = AtomicSpeculativeStats::new();

    // Record multiple rounds (simulating concurrent access)
    stats.record_round(4, 3, Duration::from_millis(10));
    stats.record_round(4, 4, Duration::from_millis(8));
    stats.record_round(4, 2, Duration::from_millis(12));

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.draft_tokens, 12);
    assert_eq!(snapshot.accepted_tokens, 9);
    assert_eq!(snapshot.main_forward_passes, 3);
    assert!((snapshot.acceptance_rate - 0.75).abs() < 0.01);
}

#[test]
fn test_atomic_stats_reset() {
    let stats = AtomicSpeculativeStats::new();
    stats.record_round(4, 3, Duration::from_millis(10));

    assert_eq!(stats.snapshot().draft_tokens, 4);

    stats.reset();

    assert_eq!(stats.snapshot().draft_tokens, 0);
    assert_eq!(stats.snapshot().accepted_tokens, 0);
}

#[test]
fn test_tree_node_creation() {
    let node = TreeNode::new(42, 0.8, 0);

    assert_eq!(node.token, 42);
    assert!((node.prob - 0.8).abs() < 0.01);
    assert_eq!(node.depth, 0);
    assert!(node.children.is_empty());
}

#[test]
fn test_tree_node_add_child() {
    let mut root = TreeNode::new(0, 1.0, 0);

    root.add_child(1, 0.6);
    root.add_child(2, 0.3);
    root.add_child(3, 0.1);

    assert_eq!(root.children.len(), 3);
    assert_eq!(root.children[0].token, 1);
    assert_eq!(root.children[1].token, 2);
    assert_eq!(root.children[2].token, 3);
    assert_eq!(root.children[0].depth, 1);
}

#[test]
fn test_tree_node_best_path() {
    let mut root = TreeNode::new(0, 1.0, 0);

    // Build tree:
    //       0
    //      / \
    //     1   2
    //    /   / \
    //   3   4   5

    let child1 = root.add_child(1, 0.6);
    child1.add_child(3, 0.5);

    let child2 = root.add_child(2, 0.3);
    child2.add_child(4, 0.2);
    child2.add_child(5, 0.1);

    // Best path should follow highest probabilities
    let path = root.best_path();
    assert_eq!(path[0], 0);
    assert_eq!(path[1], 1); // 0.6 > 0.3
    assert_eq!(path[2], 3);
}

#[test]
fn test_tree_node_get_paths() {
    let mut root = TreeNode::new(0, 1.0, 0);

    let child1 = root.add_child(1, 0.6);
    child1.add_child(3, 0.5);

    let child2 = root.add_child(2, 0.3);
    child2.add_child(4, 0.2);
    child2.add_child(5, 0.1);

    let paths = root.get_paths();

    // Should have 3 paths:
    // [0, 1, 3]
    // [0, 2, 4]
    // [0, 2, 5]
    assert_eq!(paths.len(), 3);
}

#[test]
fn test_speculation_tree_creation() {
    let tree = SpeculationTree::new(3, 2);

    assert_eq!(tree.max_depth, 3);
    assert_eq!(tree.branching_factor, 2);
    assert_eq!(tree.node_count, 1);
}

#[test]
fn test_speculation_tree_best_path() {
    let mut tree = SpeculationTree::new(3, 2);

    // Build a linear path
    let mut current = &mut tree.root;
    current = current.add_child(10, 0.9);
    tree.node_count += 1;
    current = current.add_child(20, 0.8);
    tree.node_count += 1;
    current.add_child(30, 0.7);
    tree.node_count += 1;

    let best = tree.best_path();

    // Should skip the root placeholder and return [10, 20, 30]
    assert_eq!(best, vec![10, 20, 30]);
}

#[test]
fn test_speculation_tree_clear() {
    let mut tree = SpeculationTree::new(3, 2);

    tree.root.add_child(1, 0.5);
    tree.node_count += 1;

    assert_eq!(tree.node_count, 2);

    tree.clear();

    assert_eq!(tree.node_count, 1);
    assert!(tree.root.children.is_empty());
}

#[test]
fn test_verification_result() {
    let result = VerificationResult {
        accepted_count: 3,
        next_token: 100,
        accepted_logprobs: vec![-0.1, -0.2, -0.3],
        next_logprob: -0.5,
        all_accepted: false,
    };

    assert_eq!(result.accepted_count, 3);
    assert_eq!(result.next_token, 100);
    assert!(!result.all_accepted);
    assert_eq!(result.accepted_logprobs.len(), 3);
}

#[test]
fn test_softmax() {
    let logits = vec![1.0, 2.0, 3.0];
    let probs = softmax(&logits);

    // Probabilities should sum to 1
    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 0.001);

    // Probabilities should be ordered
    assert!(probs[2] > probs[1]);
    assert!(probs[1] > probs[0]);

    // All probabilities should be positive
    assert!(probs.iter().all(|&p| p > 0.0));
}

#[test]
fn test_softmax_with_large_values() {
    // Test numerical stability with large values
    let logits = vec![100.0, 200.0, 300.0];
    let probs = softmax(&logits);

    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 0.001);
}

#[test]
fn test_softmax_with_negative_values() {
    let logits = vec![-1.0, -2.0, -3.0];
    let probs = softmax(&logits);

    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 0.001);
}

#[test]
fn test_log_softmax() {
    let logits = vec![1.0, 2.0, 3.0];
    let log_probs = log_softmax(&logits);

    // All log probabilities should be negative (probabilities < 1)
    assert!(log_probs.iter().all(|&lp| lp <= 0.0));

    // exp(log_softmax) should equal softmax
    let probs: Vec<f32> = log_probs.iter().map(|&lp: &f32| lp.exp()).collect();
    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 0.001);
}

#[test]
fn test_top_k_filter() {
    let mut logits: Vec<f32> = vec![1.0, 5.0, 3.0, 4.0, 2.0];
    top_k_filter(&mut logits, 2);

    // Only top 2 (5.0 and 4.0) should remain finite
    let finite_count = logits.iter().filter(|&&x| x.is_finite()).count();
    assert_eq!(finite_count, 2);

    // The top values should be unchanged
    assert!((logits[1] - 5.0).abs() < 0.01);
    assert!((logits[3] - 4.0).abs() < 0.01);
}

#[test]
fn test_top_k_filter_k_equals_len() {
    let mut logits: Vec<f32> = vec![1.0, 2.0, 3.0];
    top_k_filter(&mut logits, 3);

    // All values should remain
    let finite_count = logits.iter().filter(|&&x| x.is_finite()).count();
    assert_eq!(finite_count, 3);
}

#[test]
fn test_top_k_filter_k_zero() {
    let mut logits = vec![1.0, 2.0, 3.0];
    let original = logits.clone();
    top_k_filter(&mut logits, 0);

    // No filtering when k=0
    assert_eq!(logits, original);
}

#[test]
fn test_top_p_filter() {
    let mut logits: Vec<f32> = vec![10.0, 5.0, 2.0, 1.0, 0.5];
    top_p_filter(&mut logits, 0.9);

    // Most probability mass should be preserved
    let finite_count = logits.iter().filter(|&&x| x.is_finite()).count();
    assert!(finite_count >= 1 && finite_count <= 4);
}

#[test]
fn test_top_p_filter_p_one() {
    let mut logits = vec![1.0, 2.0, 3.0];
    let original = logits.clone();
    top_p_filter(&mut logits, 1.0);

    // No filtering when p=1.0
    assert_eq!(logits, original);
}

#[test]
fn test_top_p_filter_very_low_p() {
    let mut logits: Vec<f32> = vec![10.0, 1.0, 0.5, 0.1];
    top_p_filter(&mut logits, 0.01);

    // Only the highest probability token should remain
    let finite_count = logits.iter().filter(|&&x| x.is_finite()).count();
    assert!(finite_count >= 1);

    // The top value should be finite
    assert!(logits[0].is_finite());
}

#[test]
fn test_config_serialization() {
    let config = SpeculativeConfig {
        lookahead: 6,
        acceptance_threshold: 0.7,
        draft_temperature: 0.1,
        tree_speculation: true,
        max_tree_depth: 4,
        tree_branching_factor: 3,
        draft_top_p: 0.9,
        min_acceptance_ratio: 0.2,
        adaptive_lookahead: true,
        min_lookahead: 3,
        max_lookahead: 10,
    };

    // Test JSON serialization
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: SpeculativeConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.lookahead, 6);
    assert!(deserialized.tree_speculation);
}

#[test]
fn test_stats_serialization() {
    let mut stats = SpeculativeStats::new();
    stats.record_round(4, 3, 10.0);

    let json = serde_json::to_string(&stats).unwrap();
    let deserialized: SpeculativeStats = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.draft_tokens, 4);
    assert_eq!(deserialized.accepted_tokens, 3);
}

#[test]
fn test_realistic_speculation_scenario() {
    let mut stats = SpeculativeStats::new();

    // Simulate 100 generation rounds with varying acceptance
    for i in 0..100 {
        let draft_count = 4;
        // Acceptance varies: high at start, lower later (simulating diverse output)
        let accepted = if i < 30 {
            4 // 100% acceptance
        } else if i < 60 {
            3 // 75% acceptance
        } else {
            2 // 50% acceptance
        };

        stats.record_round(draft_count, accepted, (i as f64) * 0.1);
    }

    // Verify stats are reasonable
    assert_eq!(stats.draft_tokens, 400);
    assert!(stats.acceptance_rate > 0.5 && stats.acceptance_rate < 1.0);
    assert!(stats.speedup > 1.0); // Should show speedup
    assert_eq!(stats.main_forward_passes, 100);
}

#[test]
fn test_tree_with_deep_nesting() {
    let mut tree = SpeculationTree::new(5, 2);

    // Build a deep tree
    fn build_recursive(node: &mut TreeNode, depth: usize, max_depth: usize) {
        if depth >= max_depth {
            return;
        }

        let child = node.add_child((depth * 10) as u32, 1.0 / (depth + 1) as f32);
        build_recursive(child, depth + 1, max_depth);
    }

    build_recursive(&mut tree.root, 0, 5);

    let best = tree.best_path();
    assert_eq!(best.len(), 5);
}
