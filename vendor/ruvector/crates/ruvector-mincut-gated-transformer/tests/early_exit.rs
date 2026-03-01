//! Early exit condition tests.
//!
//! Tests for tier-based early termination, speculation/verification,
//! and fallback to full computation.

use ruvector_mincut_gated_transformer::{
    GateDecision, GatePacket, GatePolicy, GateReason, InferInput, InferOutput,
    MincutGatedTransformer, QuantizedWeights, TransformerConfig,
};

fn create_transformer(config: TransformerConfig) -> MincutGatedTransformer {
    let policy = GatePolicy::default();
    let weights = QuantizedWeights::empty(&config);
    MincutGatedTransformer::new(config, policy, weights).unwrap()
}

// ============ Early Exit Conditions ============

#[test]
fn test_early_exit_on_low_lambda() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Lambda below minimum triggers quarantine and early exit
    let gate = GatePacket {
        lambda: 20, // Below default min of 30
        lambda_prev: 100,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    // Should trigger early intervention
    assert_eq!(output.witness.decision, GateDecision::QuarantineUpdates);
    assert_eq!(output.witness.reason, GateReason::LambdaBelowMin);
    assert!(output.stats.layers_executed < config.layers);
    assert_eq!(output.stats.tier, 2);
}

#[test]
fn test_early_exit_on_lambda_drop() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Fast lambda drop triggers flush and reduced execution
    let gate = GatePacket {
        lambda: 35,
        lambda_prev: 100, // 65% drop
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    assert_eq!(output.witness.decision, GateDecision::FlushKv);
    assert_eq!(output.witness.reason, GateReason::LambdaDroppedFast);
    assert!(output.stats.layers_executed < config.layers);
}

#[test]
fn test_early_exit_tier_selection() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Different conditions should select different tiers
    let test_cases = vec![
        // (lambda, lambda_prev, boundary_edges, expected_tier_range)
        (100, 95, 5, 0..=0),  // Normal - tier 0
        (100, 95, 30, 1..=1), // Boundary spike - tier 1
        (20, 100, 5, 2..=2),  // Low lambda - tier 2
        (100, 95, 5, 0..=0),  // Normal again - tier 0
    ];

    for (lambda, lambda_prev, boundary_edges, expected_tier_range) in test_cases {
        let gate = GatePacket {
            lambda,
            lambda_prev,
            boundary_edges,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);

        transformer.infer(&input, &mut output).unwrap();

        assert!(
            expected_tier_range.contains(&output.stats.tier),
            "Tier {} not in expected range {:?} for lambda={}, lambda_prev={}, boundary_edges={}",
            output.stats.tier,
            expected_tier_range,
            lambda,
            lambda_prev,
            boundary_edges
        );

        transformer.reset();
    }
}

// ============ Speculation and Verification ============

#[test]
fn test_speculative_execution_with_stable_lambda() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Stable lambda allows speculative full execution
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 98,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    // Should use full layers (tier 0)
    assert_eq!(output.stats.tier, 0);
    assert_eq!(output.stats.layers_executed, config.layers);
    assert_eq!(output.witness.decision, GateDecision::Allow);
}

#[test]
fn test_speculation_fallback_on_instability() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Start with stable state
    let gate_stable = GatePacket {
        lambda: 100,
        lambda_prev: 98,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_stable);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        assert_eq!(output.stats.tier, 0);
    }

    // Sudden instability should fallback to reduced execution
    let gate_unstable = GatePacket {
        lambda: 40,
        lambda_prev: 100, // 60% drop
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_unstable);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        assert_eq!(output.witness.decision, GateDecision::FlushKv);
        assert!(output.stats.layers_executed < config.layers);
    }
}

#[test]
fn test_verification_prevents_invalid_cache() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();
    let signature = 12345u64;

    // First run with stable conditions - cache result
    let gate_stable = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_stable).with_signature(signature);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
    }

    // Second run with unstable conditions but skip flag
    // Should not use cached result from stable conditions
    let gate_unstable_skip = GatePacket {
        lambda: 20, // Unstable
        lambda_prev: 100,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_SKIP,
    };

    let input = InferInput::from_tokens(&tokens, gate_unstable_skip).with_signature(signature);
    let mut logits2 = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits2);
        transformer.infer(&input, &mut output).unwrap();
        assert_eq!(output.stats.skipped, 1);
    }
}

// ============ Fallback to Full Computation ============

#[test]
fn test_fallback_after_failed_early_exit() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Trigger early exit with boundary spike
    let gate_exit = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 35, // High boundary edges
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_exit);
    let mut logits = vec![0i32; config.logits as usize];
    let early_exit_layers;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        early_exit_layers = output.stats.layers_executed;
        assert!(output.stats.layers_executed < config.layers);
    }

    transformer.reset();

    // Return to stable conditions - should use full computation
    let gate_stable = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_stable);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        assert!(output.stats.layers_executed > early_exit_layers);
        assert_eq!(output.stats.layers_executed, config.layers);
    }
}

#[test]
fn test_force_safe_minimum_computation() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Force safe mode
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_FORCE_SAFE,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    // Should use minimal computation
    assert_eq!(output.witness.decision, GateDecision::FreezeWrites);
    assert_eq!(output.stats.tier, 2);
    assert_eq!(output.stats.layers_executed, 1);
    assert_eq!(output.witness.kv_writes_enabled, 0);
}

#[test]
fn test_progressive_degradation() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    let conditions = vec![
        (100, 95, 5, "stable"),
        (95, 100, 10, "slight_boundary_increase"),
        (90, 95, 20, "boundary_spike"),
        (85, 90, 35, "severe_boundary_spike"),
        (40, 85, 40, "lambda_drop_and_boundary"),
    ];

    let mut prev_layers = config.layers;

    for (lambda, lambda_prev, boundary_edges, _desc) in conditions {
        let gate = GatePacket {
            lambda,
            lambda_prev,
            boundary_edges,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);

        transformer.infer(&input, &mut output).unwrap();

        // Should see progressive degradation or maintenance
        assert!(output.stats.layers_executed <= prev_layers);
        prev_layers = output.stats.layers_executed;

        transformer.reset();
    }
}

#[test]
fn test_layer_execution_counts() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Tier 0 - full execution
    let gate_t0 = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_t0);
    let mut logits = vec![0i32; config.logits as usize];
    let t0_layers;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        t0_layers = output.stats.layers_executed;
    }

    transformer.reset();

    // Tier 1 - reduced execution
    let gate_t1 = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_t1);
    let mut logits = vec![0i32; config.logits as usize];
    let t1_layers;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        t1_layers = output.stats.layers_executed;
    }

    transformer.reset();

    // Tier 2 - minimal execution
    let gate_t2 = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_FORCE_SAFE,
    };

    let input = InferInput::from_tokens(&tokens, gate_t2);
    let mut logits = vec![0i32; config.logits as usize];
    let t2_layers;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        t2_layers = output.stats.layers_executed;
    }

    // Verify tier ordering
    assert!(t0_layers > t1_layers);
    assert!(t1_layers > t2_layers);
    assert_eq!(t2_layers, 1);
}

#[test]
fn test_early_exit_operation_counts() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Full computation
    let gate_full = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_full);
    let mut logits = vec![0i32; config.logits as usize];
    let full_ops;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        full_ops = output.stats.attn_dot_ops + output.stats.ffn_ops;
    }

    transformer.reset();

    // Early exit
    let gate_exit = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_exit);
    let mut logits = vec![0i32; config.logits as usize];
    let exit_ops;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        exit_ops = output.stats.attn_dot_ops + output.stats.ffn_ops;
    }

    // Early exit should perform fewer operations
    assert!(exit_ops < full_ops);
}
