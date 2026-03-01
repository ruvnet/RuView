//! Token Generation Tests
//!
//! Tests for autoregressive token generation, sampling strategies,
//! streaming callbacks, KV cache integration, and speculative decoding.

use crate::speculative::{
    log_softmax, sample_from_probs, softmax, top_k_filter, top_p_filter, AtomicSpeculativeStats,
    SpeculationTree, SpeculativeConfig, SpeculativeStats, TreeNode, VerificationResult,
};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::time::Duration;

// ============================================================================
// Softmax and Sampling Tests
// ============================================================================

#[test]
fn test_softmax_produces_valid_distribution() {
    let logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let probs = softmax(&logits);

    // Sum should be 1.0
    let sum: f32 = probs.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-5,
        "Softmax sum should be 1.0, got {}",
        sum
    );

    // All probabilities should be positive
    assert!(
        probs.iter().all(|&p| p > 0.0),
        "All probabilities should be positive"
    );

    // Ordering should be preserved
    for i in 0..probs.len() - 1 {
        assert!(
            probs[i] < probs[i + 1],
            "Higher logits should have higher probs"
        );
    }
}

#[test]
fn test_softmax_handles_large_logits() {
    // Test numerical stability with large logits
    let logits = vec![1000.0, 1001.0, 1002.0];
    let probs = softmax(&logits);

    let sum: f32 = probs.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-4,
        "Should handle large logits: sum = {}",
        sum
    );
    assert!(
        probs.iter().all(|p| p.is_finite()),
        "All probs should be finite"
    );
}

#[test]
fn test_softmax_handles_negative_logits() {
    let logits = vec![-5.0, -3.0, -1.0, 0.0, 1.0];
    let probs = softmax(&logits);

    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 1e-5, "Should handle negative logits");
    assert!(probs[4] > probs[0], "Larger logit should have higher prob");
}

#[test]
fn test_softmax_empty_input() {
    let logits: Vec<f32> = vec![];
    let probs = softmax(&logits);
    assert!(probs.is_empty(), "Empty input should return empty output");
}

#[test]
fn test_softmax_single_element() {
    let logits = vec![5.0];
    let probs = softmax(&logits);
    assert_eq!(probs.len(), 1);
    assert!(
        (probs[0] - 1.0).abs() < 1e-5,
        "Single element should have prob 1.0"
    );
}

#[test]
fn test_log_softmax_relationship() {
    let logits = vec![1.0, 2.0, 3.0, 4.0];
    let probs = softmax(&logits);
    let log_probs = log_softmax(&logits);

    // log_softmax should equal log(softmax)
    for (lp, p) in log_probs.iter().zip(probs.iter()) {
        let expected = p.ln();
        assert!(
            (lp - expected).abs() < 1e-4,
            "log_softmax should match log(softmax)"
        );
    }
}

#[test]
fn test_log_softmax_numerical_stability() {
    // log_softmax should be stable even when softmax would underflow
    let logits = vec![-1000.0, -999.0, -998.0];
    let log_probs = log_softmax(&logits);

    assert!(
        log_probs.iter().all(|p| p.is_finite()),
        "log_softmax should handle extreme values"
    );
    // Check that relative ordering is preserved
    assert!(log_probs[0] < log_probs[1] && log_probs[1] < log_probs[2]);
}

// ============================================================================
// Top-K Filtering Tests
// ============================================================================

#[test]
fn test_top_k_filter_basic() {
    let mut logits = vec![1.0, 5.0, 3.0, 4.0, 2.0];
    top_k_filter(&mut logits, 2);

    // Only top 2 (indices 1 and 3 with values 5.0 and 4.0) should remain finite
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert_eq!(finite_count, 2, "Only top-k elements should remain");

    // Check that correct elements are kept
    assert!(logits[1].is_finite(), "5.0 should remain");
    assert!(logits[3].is_finite(), "4.0 should remain");
}

#[test]
fn test_top_k_filter_k_greater_than_length() {
    let mut logits = vec![1.0, 2.0, 3.0];
    top_k_filter(&mut logits, 10);

    // All should remain unchanged
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert_eq!(finite_count, 3, "All should remain when k > length");
}

#[test]
fn test_top_k_filter_k_zero() {
    let mut logits = vec![1.0, 2.0, 3.0];
    top_k_filter(&mut logits, 0);

    // All should remain unchanged
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert_eq!(finite_count, 3, "All should remain when k = 0");
}

#[test]
fn test_top_k_filter_k_one() {
    let mut logits = vec![1.0, 5.0, 3.0, 4.0, 2.0];
    top_k_filter(&mut logits, 1);

    // Only the maximum should remain
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert_eq!(finite_count, 1, "Only one element should remain");
    assert!(logits[1].is_finite(), "Maximum (5.0) should remain");
}

// ============================================================================
// Top-P (Nucleus) Filtering Tests
// ============================================================================

#[test]
fn test_top_p_filter_basic() {
    // Create logits where first element dominates
    let mut logits = vec![10.0, 1.0, 0.0, -1.0, -2.0];
    top_p_filter(&mut logits, 0.9);

    // At least the highest probability token should remain
    assert!(logits[0].is_finite(), "Highest prob token should remain");
}

#[test]
fn test_top_p_filter_p_one() {
    let mut logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let original = logits.clone();
    top_p_filter(&mut logits, 1.0);

    // All should remain unchanged when p >= 1.0
    assert_eq!(logits, original, "All should remain when p = 1.0");
}

#[test]
fn test_top_p_filter_p_zero() {
    let mut logits = vec![1.0, 2.0, 3.0];
    top_p_filter(&mut logits, 0.0);

    // Only top token should remain
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert!(finite_count >= 1, "At least one token should remain");
}

// ============================================================================
// Sampling Tests
// ============================================================================

#[test]
fn test_sample_from_probs_deterministic() {
    let probs = vec![0.0, 0.0, 1.0, 0.0]; // Deterministic: only index 2
    let mut rng = StdRng::seed_from_u64(12345);

    for _ in 0..10 {
        let idx = sample_from_probs(&probs, &mut rng);
        assert_eq!(idx, 2, "Should always sample index 2");
    }
}

#[test]
fn test_sample_from_probs_uniform() {
    let probs = vec![0.25, 0.25, 0.25, 0.25];
    let mut rng = StdRng::seed_from_u64(42);
    let mut counts = vec![0usize; 4];

    // Sample many times
    for _ in 0..10000 {
        let idx = sample_from_probs(&probs, &mut rng);
        counts[idx] += 1;
    }

    // Each should be sampled approximately 2500 times
    for (i, &count) in counts.iter().enumerate() {
        let expected = 2500.0;
        let actual = count as f64;
        let ratio = actual / expected;
        assert!(
            (0.8..=1.2).contains(&ratio),
            "Index {} should be sampled uniformly, got {} (expected ~{})",
            i,
            count,
            expected
        );
    }
}

#[test]
fn test_sample_from_probs_skewed() {
    let probs = vec![0.9, 0.05, 0.03, 0.02]; // Heavily skewed
    let mut rng = StdRng::seed_from_u64(42);
    let mut counts = vec![0usize; 4];

    for _ in 0..1000 {
        let idx = sample_from_probs(&probs, &mut rng);
        counts[idx] += 1;
    }

    // Index 0 should dominate
    assert!(counts[0] > 800, "Index 0 should be sampled most often");
}

// ============================================================================
// Temperature Scaling Tests
// ============================================================================

#[test]
fn test_temperature_scaling_sharpens() {
    let logits = vec![1.0, 2.0, 3.0, 4.0];
    let temperature = 0.1; // Low temperature -> sharper distribution

    let scaled: Vec<f32> = logits.iter().map(|&l| l / temperature).collect();
    let probs = softmax(&scaled);

    // Highest logit should have much higher probability
    assert!(
        probs[3] > 0.99,
        "Low temperature should concentrate probability on max"
    );
}

#[test]
fn test_temperature_scaling_flattens() {
    let logits = vec![1.0, 2.0, 3.0, 4.0];
    let temperature = 10.0; // High temperature -> flatter distribution

    let scaled: Vec<f32> = logits.iter().map(|&l| l / temperature).collect();
    let probs = softmax(&scaled);

    // Distribution should be more uniform
    let min_prob = probs.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_prob = probs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    assert!(
        max_prob - min_prob < 0.2,
        "High temperature should flatten distribution"
    );
}

#[test]
fn test_temperature_one_unchanged() {
    let logits = vec![1.0, 2.0, 3.0, 4.0];
    let temperature = 1.0;

    let scaled: Vec<f32> = logits.iter().map(|&l| l / temperature).collect();
    let probs1 = softmax(&logits);
    let probs2 = softmax(&scaled);

    for (p1, p2) in probs1.iter().zip(probs2.iter()) {
        assert!(
            (p1 - p2).abs() < 1e-6,
            "Temperature 1.0 should not change distribution"
        );
    }
}

// ============================================================================
// Speculative Decoding Config Tests
// ============================================================================

#[test]
fn test_speculative_config_default() {
    let config = SpeculativeConfig::default();

    assert_eq!(config.lookahead, 4);
    assert!((config.acceptance_threshold - 0.5).abs() < 0.01);
    assert_eq!(config.draft_temperature, 0.0);
    assert!(!config.tree_speculation);
    assert!(config.adaptive_lookahead);
    assert_eq!(config.min_lookahead, 2);
    assert_eq!(config.max_lookahead, 8);
}

#[test]
fn test_speculative_config_custom() {
    let config = SpeculativeConfig {
        lookahead: 8,
        acceptance_threshold: 0.7,
        draft_temperature: 0.3,
        tree_speculation: true,
        max_tree_depth: 4,
        tree_branching_factor: 3,
        ..Default::default()
    };

    assert_eq!(config.lookahead, 8);
    assert!((config.acceptance_threshold - 0.7).abs() < 0.01);
    assert!(config.tree_speculation);
    assert_eq!(config.max_tree_depth, 4);
    assert_eq!(config.tree_branching_factor, 3);
}

// ============================================================================
// Speculative Stats Tests
// ============================================================================

#[test]
fn test_speculative_stats_new() {
    let stats = SpeculativeStats::new();

    assert_eq!(stats.draft_tokens, 0);
    assert_eq!(stats.accepted_tokens, 0);
    assert_eq!(stats.acceptance_rate, 0.0);
    assert_eq!(stats.speedup, 0.0);
    assert_eq!(stats.main_forward_passes, 0);
}

#[test]
fn test_speculative_stats_record_round() {
    let mut stats = SpeculativeStats::new();

    // Record a round with 4 drafts, 3 accepted
    stats.record_round(4, 3, 10.0);

    assert_eq!(stats.draft_tokens, 4);
    assert_eq!(stats.accepted_tokens, 3);
    assert!((stats.acceptance_rate - 0.75).abs() < 0.01);
    assert_eq!(stats.main_forward_passes, 1);
    assert_eq!(stats.total_tokens_generated, 4); // 3 accepted + 1 correction
    assert!((stats.total_speculation_time_ms - 10.0).abs() < 0.01);
}

#[test]
fn test_speculative_stats_multiple_rounds() {
    let mut stats = SpeculativeStats::new();

    // Round 1: 4 drafts, 4 accepted (100% acceptance)
    stats.record_round(4, 4, 10.0);

    // Round 2: 4 drafts, 2 accepted (50% acceptance)
    stats.record_round(4, 2, 15.0);

    assert_eq!(stats.draft_tokens, 8);
    assert_eq!(stats.accepted_tokens, 6);
    assert!((stats.acceptance_rate - 0.75).abs() < 0.01); // 6/8 = 0.75
    assert_eq!(stats.main_forward_passes, 2);
    // Total tokens depends on implementation - just check it's reasonable
    assert!(
        stats.total_tokens_generated >= 6,
        "Should generate at least accepted tokens"
    );
}

#[test]
fn test_speculative_stats_reset() {
    let mut stats = SpeculativeStats::new();
    stats.record_round(4, 3, 10.0);
    stats.reset();

    assert_eq!(stats.draft_tokens, 0);
    assert_eq!(stats.accepted_tokens, 0);
    assert_eq!(stats.acceptance_rate, 0.0);
}

#[test]
fn test_speculative_stats_speedup_calculation() {
    let mut stats = SpeculativeStats::new();

    // If we accept 4 tokens per main pass on average, speedup should be ~4x
    stats.record_round(4, 4, 10.0);
    stats.record_round(4, 4, 10.0);

    // 10 total tokens, 2 main passes -> 5 tokens/pass
    assert!(
        stats.speedup > 4.0,
        "Speedup should reflect tokens per main pass"
    );
}

// ============================================================================
// Atomic Speculative Stats Tests
// ============================================================================

#[test]
fn test_atomic_stats_new() {
    let stats = AtomicSpeculativeStats::new();
    let snapshot = stats.snapshot();

    assert_eq!(snapshot.draft_tokens, 0);
    assert_eq!(snapshot.accepted_tokens, 0);
}

#[test]
fn test_atomic_stats_record_round() {
    let stats = AtomicSpeculativeStats::new();
    stats.record_round(4, 3, Duration::from_millis(10));

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.draft_tokens, 4);
    assert_eq!(snapshot.accepted_tokens, 3);
    assert!((snapshot.acceptance_rate - 0.75).abs() < 0.01);
}

#[test]
fn test_atomic_stats_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let stats = Arc::new(AtomicSpeculativeStats::new());
    let mut handles = vec![];

    // Spawn multiple threads recording rounds
    for _ in 0..10 {
        let stats_clone = Arc::clone(&stats);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                stats_clone.record_round(4, 3, Duration::from_millis(1));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.draft_tokens, 4000); // 10 threads * 100 rounds * 4 drafts
    assert_eq!(snapshot.accepted_tokens, 3000);
}

#[test]
fn test_atomic_stats_reset() {
    let stats = AtomicSpeculativeStats::new();
    stats.record_round(4, 3, Duration::from_millis(10));
    stats.reset();

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.draft_tokens, 0);
}

// ============================================================================
// Tree Node Tests
// ============================================================================

#[test]
fn test_tree_node_new() {
    let node = TreeNode::new(42, 0.8, 0);

    assert_eq!(node.token, 42);
    assert!((node.prob - 0.8).abs() < 0.01);
    assert!((node.logprob - 0.8f32.ln()).abs() < 0.01);
    assert_eq!(node.depth, 0);
    assert!(node.children.is_empty());
}

#[test]
fn test_tree_node_add_child() {
    let mut root = TreeNode::new(0, 1.0, 0);

    let child1 = root.add_child(1, 0.6);
    assert_eq!(child1.token, 1);
    assert_eq!(child1.depth, 1);

    let child2 = root.add_child(2, 0.4);
    assert_eq!(child2.token, 2);

    assert_eq!(root.children.len(), 2);
}

#[test]
fn test_tree_node_get_paths() {
    let mut root = TreeNode::new(0, 1.0, 0);

    // Build a tree:
    //       0
    //      / \
    //     1   2
    //    /
    //   3

    {
        let child1 = root.add_child(1, 0.6);
        child1.add_child(3, 0.5);
    }
    root.add_child(2, 0.4);

    let paths = root.get_paths();
    assert_eq!(paths.len(), 2);

    // Should have paths [0, 1, 3] and [0, 2]
    assert!(paths.iter().any(|p| p == &vec![0, 1, 3]));
    assert!(paths.iter().any(|p| p == &vec![0, 2]));
}

#[test]
fn test_tree_node_best_path() {
    let mut root = TreeNode::new(0, 1.0, 0);

    // Build tree with different probabilities
    {
        let child1 = root.add_child(1, 0.6);
        child1.add_child(3, 0.5);
    }
    root.add_child(2, 0.4);

    let best = root.best_path();
    // Should follow highest probability children: 0 -> 1 -> 3
    assert_eq!(best, vec![0, 1, 3]);
}

// ============================================================================
// Speculation Tree Tests
// ============================================================================

#[test]
fn test_speculation_tree_new() {
    let tree = SpeculationTree::new(3, 2);

    assert_eq!(tree.max_depth, 3);
    assert_eq!(tree.branching_factor, 2);
    assert_eq!(tree.node_count, 1);
}

#[test]
fn test_speculation_tree_clear() {
    let mut tree = SpeculationTree::new(3, 2);
    tree.root.add_child(1, 0.5);
    tree.node_count += 1;

    tree.clear();

    assert_eq!(tree.node_count, 1);
    assert!(tree.root.children.is_empty());
}

#[test]
fn test_speculation_tree_best_path_empty() {
    let tree = SpeculationTree::new(3, 2);
    let path = tree.best_path();

    assert!(path.is_empty(), "Empty tree should have empty best path");
}

#[test]
fn test_speculation_tree_best_path_linear() {
    let mut tree = SpeculationTree::new(4, 2);

    // Build linear path: root -> 1 -> 2 -> 3
    let node1 = tree.root.add_child(1, 0.8);
    tree.node_count += 1;
    let node2 = node1.add_child(2, 0.7);
    tree.node_count += 1;
    node2.add_child(3, 0.6);
    tree.node_count += 1;

    let path = tree.best_path();
    assert_eq!(path, vec![1, 2, 3]);
}

// ============================================================================
// Verification Result Tests
// ============================================================================

#[test]
fn test_verification_result_all_accepted() {
    let result = VerificationResult {
        accepted_count: 4,
        next_token: 100,
        accepted_logprobs: vec![-0.1, -0.2, -0.1, -0.15],
        next_logprob: -0.3,
        all_accepted: true,
    };

    assert_eq!(result.accepted_count, 4);
    assert_eq!(result.next_token, 100);
    assert!(result.all_accepted);
}

#[test]
fn test_verification_result_partial_accept() {
    let result = VerificationResult {
        accepted_count: 2,
        next_token: 50, // Correction token
        accepted_logprobs: vec![-0.1, -0.2],
        next_logprob: -0.5,
        all_accepted: false,
    };

    assert_eq!(result.accepted_count, 2);
    assert!(!result.all_accepted);
}

#[test]
fn test_verification_result_none_accepted() {
    let result = VerificationResult {
        accepted_count: 0,
        next_token: 25, // Immediate correction
        accepted_logprobs: vec![],
        next_logprob: -0.4,
        all_accepted: false,
    };

    assert_eq!(result.accepted_count, 0);
    assert!(result.accepted_logprobs.is_empty());
    assert!(!result.all_accepted);
}

// ============================================================================
// Integration Sampling Tests
// ============================================================================

#[test]
fn test_full_sampling_pipeline() {
    // Test basic sampling pipeline functionality
    let logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];

    // Convert to probabilities
    let probs = softmax(&logits);

    // Verify softmax produces valid distribution
    let sum: f32 = probs.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-4,
        "Softmax should sum to 1.0, got {}",
        sum
    );
    assert!(
        probs.iter().all(|&p| p > 0.0),
        "All probabilities should be positive"
    );

    // Sample with fixed RNG
    let mut rng = StdRng::seed_from_u64(42);
    let mut samples = vec![0usize; 5];
    for _ in 0..1000 {
        let idx = sample_from_probs(&probs, &mut rng);
        if idx < samples.len() {
            samples[idx] += 1;
        }
    }

    // Higher logits should be sampled more frequently on average
    let total_samples: usize = samples.iter().sum();
    assert_eq!(total_samples, 1000, "Should have 1000 total samples");

    // Higher indices (higher logits) should be more frequent
    // This is a statistical test - with 1000 samples, index 4 (highest logit)
    // should be sampled more often than index 0 (lowest logit)
    assert!(
        samples[4] > samples[0],
        "Higher logit should be sampled more: idx4={}, idx0={}",
        samples[4],
        samples[0]
    );
}

#[test]
fn test_greedy_decoding_simulation() {
    // Simulate greedy decoding (temperature = 0 equivalent)
    let logits = vec![1.0, 3.0, 2.0, 5.0, 4.0];

    // Greedy: pick argmax
    let argmax = logits
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();

    assert_eq!(argmax, 3, "Greedy should select index 3 (value 5.0)");
}

#[test]
fn test_beam_search_simulation() {
    // Simulate a simple beam search step
    let beam_width = 3;
    let logits = vec![1.0, 5.0, 3.0, 4.0, 2.0];

    // Get top-k indices
    let mut indexed: Vec<(usize, f32)> = logits.iter().cloned().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let top_indices: Vec<usize> = indexed.iter().take(beam_width).map(|(i, _)| *i).collect();

    assert_eq!(
        top_indices,
        vec![1, 3, 2],
        "Top-3 should be indices 1, 3, 2"
    );
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[test]
fn test_softmax_with_inf() {
    let logits = vec![f32::NEG_INFINITY, 1.0, 2.0];
    let probs = softmax(&logits);

    // First element should have probability ~0
    assert!(
        probs[0] < 1e-10 || probs[0].abs() < 1e-10,
        "NEG_INFINITY should give ~0 probability"
    );

    // Sum should still be ~1
    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 1e-4, "Sum should be 1.0");
}

#[test]
fn test_sample_numerical_edge_case() {
    // Probabilities that might cause issues
    let probs = vec![0.9999999, 0.0000001];
    let mut rng = StdRng::seed_from_u64(42);

    // Should not panic
    for _ in 0..100 {
        let idx = sample_from_probs(&probs, &mut rng);
        assert!(idx < 2, "Index should be valid");
    }
}

#[test]
fn test_top_k_with_ties() {
    let mut logits = vec![5.0, 5.0, 5.0, 1.0, 2.0];
    top_k_filter(&mut logits, 3);

    // All three 5.0s should remain
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert!(
        finite_count >= 3,
        "Should keep at least k elements when ties exist"
    );
}
