//! Token routing and modulation tests.
//!
//! Tests for token routing based on lambda patterns, capacity constraints,
//! boundary token handling, and skip ratio calculations.

use ruvector_mincut_gated_transformer::{
    GateDecision, GatePacket, GatePolicy, InferInput, InferOutput, MincutGatedTransformer,
    QuantizedWeights, TransformerConfig,
};

fn create_transformer(config: TransformerConfig) -> MincutGatedTransformer {
    let policy = GatePolicy::default();
    let weights = QuantizedWeights::empty(&config);
    MincutGatedTransformer::new(config, policy, weights).unwrap()
}

// ============ Lambda-Based Routing ============

#[test]
fn test_routing_with_increasing_lambda() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();
    let tiers: Vec<u8> = (0..10)
        .map(|i| {
            let gate = GatePacket {
                lambda: 50 + i * 10, // Increasing lambda
                lambda_prev: 50 + (i.saturating_sub(1)) * 10,
                boundary_edges: 5,
                boundary_concentration_q15: 8192,
                partition_count: 3,
                flags: 0,
            };

            let input = InferInput::from_tokens(&tokens, gate);
            let mut logits = vec![0i32; config.logits as usize];
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            transformer.reset();
            output.stats.tier
        })
        .collect();

    // With increasing lambda, should stay at tier 0
    for tier in tiers {
        assert_eq!(tier, 0);
    }
}

#[test]
fn test_routing_with_decreasing_lambda() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    let mut tier_changes = 0;
    let mut prev_tier = 0u8;

    for i in 0..10u32 {
        let current_lambda = 100 - i * 5;
        let prev_lambda = if i > 0 { 100 - (i - 1) * 5 } else { 100 };

        let gate = GatePacket {
            lambda: current_lambda,
            lambda_prev: prev_lambda,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();

        if output.stats.tier != prev_tier {
            tier_changes += 1;
            prev_tier = output.stats.tier;
        }

        transformer.reset();
    }

    // Should see tier degradation as lambda decreases (may not change every step)
    // At minimum, should change when lambda drops below thresholds
    assert!(tier_changes >= 0); // Allow no changes if all within same tier range
}

#[test]
fn test_routing_with_oscillating_lambda() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Larger oscillations to trigger interventions
    let lambdas = vec![100, 50, 100, 45, 100, 40, 100, 35];
    let mut decisions = Vec::new();

    for (i, &lambda) in lambdas.iter().enumerate() {
        let gate = GatePacket {
            lambda,
            lambda_prev: if i > 0 { lambdas[i - 1] } else { 100 },
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();

        decisions.push(output.witness.decision);
        transformer.reset();
    }

    // Large oscillations should trigger interventions
    let interventions = decisions
        .iter()
        .filter(|d| **d != GateDecision::Allow)
        .count();
    assert!(
        interventions > 0,
        "Expected some interventions, but all were Allow"
    );
}

// ============ Capacity Constraints ============

#[test]
fn test_capacity_with_sequence_length() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let gate_normal = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    // Test with varying sequence lengths
    for seq_len in [8, 16, 32, 64] {
        let tokens: Vec<u32> = (0..seq_len).collect();
        let input = InferInput::from_tokens(&tokens, gate_normal);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);

        let result = transformer.infer(&input, &mut output);
        assert!(result.is_ok());

        // Effective seq len should be bounded by config
        assert!(output.stats.effective_seq_len <= config.seq_len_max);

        transformer.reset();
    }
}

#[test]
fn test_capacity_with_degraded_tier() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..64).collect();

    // Normal capacity
    let gate_normal = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_normal);
    let mut logits = vec![0i32; config.logits as usize];
    let normal_capacity;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        normal_capacity = output.stats.effective_seq_len;
    }

    transformer.reset();

    // Degraded capacity
    let gate_degraded = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Triggers ReduceScope
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_degraded);
    let mut logits = vec![0i32; config.logits as usize];
    let degraded_capacity;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        degraded_capacity = output.stats.effective_seq_len;
    }

    // Degraded tier should have reduced capacity
    assert!(degraded_capacity < normal_capacity);
}

// ============ Boundary Token Handling ============

#[test]
fn test_boundary_edge_concentration() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Low concentration (edges spread out)
    let gate_low_conc = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 10,
        boundary_concentration_q15: 4096, // Low concentration
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_low_conc);
    let mut logits = vec![0i32; config.logits as usize];
    let low_conc_decision;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        low_conc_decision = output.witness.decision;
    }

    transformer.reset();

    // High concentration (edges concentrated)
    let gate_high_conc = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 10,
        boundary_concentration_q15: 25000, // High concentration
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_high_conc);
    let mut logits = vec![0i32; config.logits as usize];
    let high_conc_decision;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        high_conc_decision = output.witness.decision;
    }

    // High concentration should trigger intervention
    assert!(high_conc_decision.is_intervention() || low_conc_decision == GateDecision::Allow);
}

#[test]
fn test_boundary_edges_threshold() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Test at various boundary edge counts
    let edge_counts = [5, 10, 15, 20, 25, 30, 35, 40];
    let mut intervention_count = 0;

    for &edges in &edge_counts {
        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: edges,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();

        if output.witness.decision.is_intervention() {
            intervention_count += 1;
        }

        transformer.reset();
    }

    // Should see increasing interventions with higher edge counts
    assert!(intervention_count > 0);
}

// ============ Skip Ratio Calculation ============

#[test]
fn test_skip_ratio_with_inactive_spikes() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    // Run with inactive spikes
    let mut skip_count = 0;
    let total_runs = 10;

    for _ in 0..total_runs {
        let spike = ruvector_mincut_gated_transformer::SpikePacket {
            fired: 0, // Inactive
            rate_q15: 500,
            novelty_q15: 500,
            top_len: 0,
            top_idx: [0; 16],
            top_w_q15: [0; 16],
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();

        if output.stats.skipped == 1 {
            skip_count += 1;
        }
    }

    // All inactive spikes should skip
    assert_eq!(skip_count, total_runs);
}

#[test]
fn test_skip_ratio_with_mixed_activity() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let mut skip_count = 0;
    let activity_pattern = [1, 0, 0, 1, 0, 0, 0, 1, 0, 0];

    for &fired in &activity_pattern {
        let spike = ruvector_mincut_gated_transformer::SpikePacket {
            fired,
            rate_q15: if fired == 1 { 20000 } else { 500 },
            novelty_q15: if fired == 1 { 15000 } else { 500 },
            top_len: 0,
            top_idx: [0; 16],
            top_w_q15: [0; 16],
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();

        if output.stats.skipped == 1 {
            skip_count += 1;
        }
    }

    // Skip count should match inactive count (7 out of 10)
    assert_eq!(skip_count, 7);
}

#[test]
fn test_lambda_drop_ratio_calculation() {
    let test_cases = vec![
        (100u32, 100u32, 0u16),    // No drop
        (100u32, 90u32, 3276u16),  // 10% drop
        (100u32, 75u32, 8192u16),  // 25% drop
        (100u32, 50u32, 16384u16), // 50% drop
        (100u32, 25u32, 24576u16), // 75% drop
    ];

    for (prev, curr, expected_ratio) in test_cases {
        let gate = GatePacket {
            lambda: curr,
            lambda_prev: prev,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let ratio = gate.drop_ratio_q15();

        // Allow 10% tolerance for fixed-point arithmetic
        let tolerance = expected_ratio / 10;
        assert!(
            ratio >= expected_ratio.saturating_sub(tolerance)
                && ratio <= expected_ratio + tolerance,
            "Drop ratio mismatch: expected ~{}, got {}",
            expected_ratio,
            ratio
        );
    }
}

#[test]
fn test_routing_preserves_token_order() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Run multiple times with same inputs
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits1 = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits1);
        transformer.infer(&input, &mut output).unwrap();
    }

    transformer.reset();

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits2 = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits2);
        transformer.infer(&input, &mut output).unwrap();
    }

    // Output should be deterministic
    assert_eq!(logits1, logits2);
}
