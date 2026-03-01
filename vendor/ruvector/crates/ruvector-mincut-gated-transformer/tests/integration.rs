//! Integration tests for mincut-gated transformer features.
//!
//! These tests verify the complete pipeline with various configurations,
//! including tier transitions, early exit, and coherence-based interventions.

use ruvector_mincut_gated_transformer::{
    GateDecision, GatePacket, GatePolicy, GateReason, InferInput, InferOutput,
    MincutGatedTransformer, QuantizedWeights, SpikePacket, TransformerConfig,
};

fn create_transformer(config: TransformerConfig) -> MincutGatedTransformer {
    let policy = GatePolicy::default();
    let weights = QuantizedWeights::empty(&config);
    MincutGatedTransformer::new(config, policy, weights).unwrap()
}

// ============ Full Pipeline Tests ============

#[test]
fn test_full_pipeline_tier0_to_tier1() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    // Start with tier 0 (normal operation)
    let gate_normal = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate_normal);
    let mut logits = vec![0i32; config.logits as usize];

    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();

        assert_eq!(output.witness.decision, GateDecision::Allow);
        assert_eq!(output.stats.tier, 0);
        assert!(output.stats.layers_executed > 2);
        assert_eq!(output.witness.kv_writes_enabled, 1);
        assert_eq!(output.witness.external_writes_enabled, 1);
    }

    // Trigger tier 1 with boundary spike
    let gate_degraded = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Above threshold
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_degraded);
    let mut logits = vec![0i32; config.logits as usize];

    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();

        assert_eq!(output.witness.decision, GateDecision::ReduceScope);
        assert_eq!(output.witness.reason, GateReason::BoundarySpike);
        assert_eq!(output.stats.tier, 1);
        assert!(output.stats.layers_executed < 4);
        assert_eq!(output.witness.kv_writes_enabled, 1);
        assert_eq!(output.witness.external_writes_enabled, 0);
    }
}

#[test]
fn test_full_pipeline_with_stable_lambda() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    // Stable lambda over multiple steps
    let tokens: Vec<u32> = (0..32).collect();

    for step in 0..5u32 {
        let gate = GatePacket {
            lambda: 100 + step, // Gradually increasing
            lambda_prev: 100 + step.saturating_sub(1),
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);

        transformer.infer(&input, &mut output).unwrap();

        // Should always allow with stable/increasing lambda
        assert_eq!(output.witness.decision, GateDecision::Allow);
        assert_eq!(output.stats.tier, 0);
    }
}

#[test]
fn test_early_exit_with_unstable_lambda() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Lambda drop triggers intervention
    let gate = GatePacket {
        lambda: 40,
        lambda_prev: 100, // 60% drop
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    // Should trigger FlushKv due to fast drop
    assert_eq!(output.witness.decision, GateDecision::FlushKv);
    assert_eq!(output.witness.reason, GateReason::LambdaDroppedFast);
    assert!(output.stats.layers_executed < 4);
}

#[test]
fn test_sparse_context_reduces_compute() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..64).collect();

    // Normal gate
    let gate_normal = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input_normal = InferInput::from_tokens(&tokens, gate_normal);
    let mut logits_normal = vec![0i32; config.logits as usize];
    let ops_normal;
    {
        let mut output = InferOutput::new(&mut logits_normal);
        transformer.infer(&input_normal, &mut output).unwrap();
        ops_normal = output.stats.attn_dot_ops;
    }

    transformer.reset();

    // Reduced scope gate (simulates sparse attention)
    let gate_reduced = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Triggers ReduceScope
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input_reduced = InferInput::from_tokens(&tokens, gate_reduced);
    let mut logits_reduced = vec![0i32; config.logits as usize];
    let ops_reduced;
    {
        let mut output = InferOutput::new(&mut logits_reduced);
        transformer.infer(&input_reduced, &mut output).unwrap();
        ops_reduced = output.stats.attn_dot_ops;
    }

    // Reduced scope should perform fewer attention operations
    assert!(ops_reduced < ops_normal);
}

#[test]
fn test_gate_decision_consistency() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Same gate packet should produce same decision
    let gate = GatePacket {
        lambda: 50,
        lambda_prev: 80,
        boundary_edges: 25,
        boundary_concentration_q15: 10000,
        partition_count: 5,
        flags: 0,
    };

    let decisions: Vec<GateDecision> = (0..10)
        .map(|_| {
            let input = InferInput::from_tokens(&tokens, gate);
            let mut logits = vec![0i32; config.logits as usize];
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            transformer.reset();
            output.witness.decision
        })
        .collect();

    // All decisions should be identical
    for decision in &decisions {
        assert_eq!(*decision, decisions[0]);
    }
}

#[test]
fn test_tier_transitions_with_spikes() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Active spike with normal rate (should run normally)
    let spike_active = SpikePacket {
        fired: 1,
        rate_q15: 15000, // Below spike_rate_max in default policy
        novelty_q15: 15000,
        top_len: 0,
        top_idx: [0; 16],
        top_w_q15: [0; 16],
        flags: 0,
    };

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike_active);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        // Should run without skipping
        assert_eq!(output.stats.skipped, 0);
        // Tier depends on gate conditions
        assert!(output.stats.tier < 3);
    }

    transformer.reset();

    // Tier 3: Inactive spike (should skip)
    let spike_inactive = SpikePacket {
        fired: 0,
        rate_q15: 1000,
        novelty_q15: 1000,
        top_len: 0,
        top_idx: [0; 16],
        top_w_q15: [0; 16],
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike_inactive);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        assert_eq!(output.stats.tier, 3);
        assert_eq!(output.stats.skipped, 1);
    }
}

#[test]
fn test_boundary_concentration_intervention() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // High boundary concentration
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 25000, // Very high
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    assert_eq!(output.witness.decision, GateDecision::ReduceScope);
    assert_eq!(
        output.witness.reason,
        GateReason::BoundaryConcentrationSpike
    );
    assert_eq!(output.stats.tier, 1);
}

#[test]
fn test_partition_drift_detection() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // High partition count
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 15, // Above threshold
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    assert_eq!(output.witness.decision, GateDecision::ReduceScope);
    assert_eq!(output.witness.reason, GateReason::PartitionDrift);
}

#[test]
fn test_spike_storm_protection() {
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

    // Spike storm condition
    let spike = SpikePacket {
        fired: 1,
        rate_q15: 30000, // Very high rate
        novelty_q15: 5000,
        top_len: 0,
        top_idx: [0; 16],
        top_w_q15: [0; 16],
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    transformer.infer(&input, &mut output).unwrap();

    assert_eq!(output.witness.decision, GateDecision::FreezeWrites);
    assert_eq!(output.witness.reason, GateReason::SpikeStorm);
    assert_eq!(output.stats.tier, 2);
}

#[test]
fn test_kv_cache_persistence_across_tiers() {
    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..32).collect();

    // Tier 0 - KV writes enabled
    let gate_allow = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate_allow);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        assert_eq!(output.witness.kv_writes_enabled, 1);
        assert!(output.stats.kv_bytes_touched > 0);
    }

    // Tier 2 - KV writes frozen
    let gate_freeze = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_FORCE_SAFE,
    };

    let input = InferInput::from_tokens(&tokens, gate_freeze);
    let mut logits = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        assert_eq!(output.witness.kv_writes_enabled, 0);
        assert_eq!(output.witness.decision, GateDecision::FreezeWrites);
    }
}

#[test]
fn test_micro_config_full_pipeline() {
    let config = TransformerConfig::micro();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..16).collect();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    let result = transformer.infer(&input, &mut output);
    assert!(result.is_ok());
    assert_eq!(output.witness.decision, GateDecision::Allow);
    assert!(output.stats.layers_executed > 0);
}

#[test]
fn test_policy_variants_integration() {
    let config = TransformerConfig::baseline();
    let tokens: Vec<u32> = (0..32).collect();

    // Test with conservative policy
    let mut transformer_conservative = {
        let policy = GatePolicy::conservative();
        let weights = QuantizedWeights::empty(&config);
        MincutGatedTransformer::new(config.clone(), policy, weights).unwrap()
    };

    let gate = GatePacket {
        lambda: 45,
        lambda_prev: 50,
        boundary_edges: 8,
        boundary_concentration_q15: 12000,
        partition_count: 4,
        flags: 0,
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let conservative_decision;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer_conservative.infer(&input, &mut output).unwrap();
        conservative_decision = output.witness.decision;
    }

    // Test with permissive policy
    let mut transformer_permissive = {
        let policy = GatePolicy::permissive();
        let weights = QuantizedWeights::empty(&config);
        MincutGatedTransformer::new(config.clone(), policy, weights).unwrap()
    };

    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];
    let permissive_decision;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer_permissive.infer(&input, &mut output).unwrap();
        permissive_decision = output.witness.decision;
    }

    // Conservative should be more restrictive than permissive
    assert!(conservative_decision.is_intervention() || permissive_decision == GateDecision::Allow);
}
