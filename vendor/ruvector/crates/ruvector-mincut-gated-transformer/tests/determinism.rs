//! Determinism tests for mincut gated transformer.
//!
//! Verifies that same inputs with same gate packets yield same outputs.

use ruvector_mincut_gated_transformer::{
    GatePacket, GatePolicy, InferInput, InferOutput, MincutGatedTransformer, QuantizedWeights,
    TransformerConfig,
};

fn create_transformer() -> MincutGatedTransformer {
    let config = TransformerConfig::micro();
    let policy = GatePolicy::default();
    let weights = QuantizedWeights::empty(&config);
    MincutGatedTransformer::new(config, policy, weights).unwrap()
}

#[test]
fn test_deterministic_output_same_inputs() {
    let mut transformer = create_transformer();
    let config = transformer.config().clone();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..16).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    // Run inference twice
    let mut logits1 = vec![0i32; config.logits as usize];
    let witness1;
    {
        let mut output1 = InferOutput::new(&mut logits1);
        transformer.infer(&input, &mut output1).unwrap();
        witness1 = output1.witness;
    }

    // Reset and run again
    transformer.reset();

    let mut logits2 = vec![0i32; config.logits as usize];
    let witness2;
    {
        let mut output2 = InferOutput::new(&mut logits2);
        transformer.infer(&input, &mut output2).unwrap();
        witness2 = output2.witness;
    }

    // Outputs should be identical
    assert_eq!(logits1, logits2, "Logits should be deterministic");
    assert_eq!(witness1.decision, witness2.decision);
    assert_eq!(witness1.reason, witness2.reason);
    assert_eq!(witness1.lambda, witness2.lambda);
}

#[test]
fn test_deterministic_witness_same_gate() {
    let mut transformer = create_transformer();
    let config = transformer.config().clone();

    // Specific gate packet
    let gate = GatePacket {
        lambda: 50,
        lambda_prev: 80,
        boundary_edges: 25, // Will trigger ReduceScope
        boundary_concentration_q15: 10000,
        partition_count: 5,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..16).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let mut logits = vec![0i32; config.logits as usize];
    let witness1;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        witness1 = output.witness;
    }

    // Run again
    transformer.reset();
    let mut logits = vec![0i32; config.logits as usize];
    let witness2;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        witness2 = output.witness;
    }

    // Witnesses should be identical
    assert_eq!(witness1.decision, witness2.decision);
    assert_eq!(witness1.reason, witness2.reason);
    assert_eq!(witness1.lambda, witness2.lambda);
    assert_eq!(witness1.lambda_prev, witness2.lambda_prev);
    assert_eq!(witness1.lambda_delta, witness2.lambda_delta);
    assert_eq!(witness1.effective_seq_len, witness2.effective_seq_len);
    assert_eq!(witness1.effective_window, witness2.effective_window);
    assert_eq!(witness1.kv_writes_enabled, witness2.kv_writes_enabled);
    assert_eq!(
        witness1.external_writes_enabled,
        witness2.external_writes_enabled
    );
}

#[test]
fn test_deterministic_stats() {
    let mut transformer = create_transformer();
    let config = transformer.config().clone();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        ..Default::default()
    };

    let tokens: Vec<u32> = (0..16).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let mut logits = vec![0i32; config.logits as usize];
    let stats1;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        stats1 = output.stats;
    }

    // Run again
    transformer.reset();
    let mut logits = vec![0i32; config.logits as usize];
    let stats2;
    {
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        stats2 = output.stats;
    }

    // Stats should be identical
    assert_eq!(stats1.effective_seq_len, stats2.effective_seq_len);
    assert_eq!(stats1.effective_window, stats2.effective_window);
    assert_eq!(stats1.layers_executed, stats2.layers_executed);
    assert_eq!(stats1.tier, stats2.tier);
    assert_eq!(stats1.qgemm_calls, stats2.qgemm_calls);
}

#[test]
fn test_different_gate_different_output() {
    let mut transformer = create_transformer();
    let config = transformer.config().clone();

    let tokens: Vec<u32> = (0..16).collect();

    // Normal gate
    let gate1 = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        ..Default::default()
    };

    let input1 = InferInput::from_tokens(&tokens, gate1);
    let mut logits1 = vec![0i32; config.logits as usize];
    let witness1;
    {
        let mut output1 = InferOutput::new(&mut logits1);
        transformer.infer(&input1, &mut output1).unwrap();
        witness1 = output1.witness;
    }

    // Reset
    transformer.reset();

    // Gate that triggers intervention
    let gate2 = GatePacket {
        lambda: 10, // Below min - triggers quarantine
        lambda_prev: 100,
        boundary_edges: 5,
        ..Default::default()
    };

    let input2 = InferInput::from_tokens(&tokens, gate2);
    let mut logits2 = vec![0i32; config.logits as usize];
    let witness2;
    {
        let mut output2 = InferOutput::new(&mut logits2);
        transformer.infer(&input2, &mut output2).unwrap();
        witness2 = output2.witness;
    }

    // Decisions should be different
    assert_ne!(witness1.decision, witness2.decision);
}

#[test]
fn test_skip_deterministic() {
    let mut transformer = create_transformer();
    let config = transformer.config().clone();

    let gate = GatePacket {
        lambda: 100,
        flags: GatePacket::FLAG_SKIP,
        ..Default::default()
    };

    let tokens: Vec<u32> = (0..16).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    // Run twice
    let mut logits1 = vec![0i32; config.logits as usize];
    let stats1;
    {
        let mut output1 = InferOutput::new(&mut logits1);
        transformer.infer(&input, &mut output1).unwrap();
        stats1 = output1.stats;
    }

    let mut logits2 = vec![0i32; config.logits as usize];
    let stats2;
    {
        let mut output2 = InferOutput::new(&mut logits2);
        transformer.infer(&input, &mut output2).unwrap();
        stats2 = output2.stats;
    }

    // Both should be skipped
    assert_eq!(stats1.skipped, 1);
    assert_eq!(stats2.skipped, 1);
    assert_eq!(logits1, logits2);
}

#[test]
fn test_cached_signature_determinism() {
    let mut transformer = create_transformer();
    let config = transformer.config().clone();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        ..Default::default()
    };

    let tokens: Vec<u32> = (0..16).collect();
    let input = InferInput::from_tokens(&tokens, gate).with_signature(12345);

    // First call - computes and caches
    let mut logits1 = vec![0i32; config.logits as usize];
    {
        let mut output1 = InferOutput::new(&mut logits1);
        transformer.infer(&input, &mut output1).unwrap();
    }

    // Second call with same signature and skip flag - should use cache
    let gate_skip = GatePacket {
        lambda: 100,
        flags: GatePacket::FLAG_SKIP,
        ..Default::default()
    };
    let input_skip = InferInput::from_tokens(&tokens, gate_skip).with_signature(12345);

    let mut logits2 = vec![0i32; config.logits as usize];
    {
        let mut output2 = InferOutput::new(&mut logits2);
        transformer.infer(&input_skip, &mut output2).unwrap();
    }

    // Cached result should match
    assert_eq!(logits1, logits2);
}
