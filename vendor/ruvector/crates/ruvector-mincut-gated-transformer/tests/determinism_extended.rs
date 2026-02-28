//! Extended determinism and reproducibility tests.
//!
//! Tests determinism across all configurations, features, and edge cases.

use ruvector_mincut_gated_transformer::{
    GateDecision, GatePacket, GatePolicy, InferInput, InferOutput, MincutGatedTransformer,
    QuantizedWeights, SpikePacket, TransformerConfig,
};

fn create_transformer(config: TransformerConfig, policy: GatePolicy) -> MincutGatedTransformer {
    let weights = QuantizedWeights::empty(&config);
    MincutGatedTransformer::new(config, policy, weights).unwrap()
}

// ============ Cross-Configuration Determinism ============

#[test]
fn test_determinism_baseline_config() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    // Run 10 times
    let results: Vec<Vec<i32>> = (0..10)
        .map(|_| {
            let mut logits = vec![0i32; config.logits as usize];
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            transformer.reset();
            logits
        })
        .collect();

    // All results should be identical
    for i in 1..results.len() {
        assert_eq!(results[0], results[i], "Run {} differs from run 0", i);
    }
}

#[test]
fn test_determinism_micro_config() {
    let config = TransformerConfig::micro();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

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

    let mut logits1 = vec![0i32; config.logits as usize];
    let mut logits2 = vec![0i32; config.logits as usize];

    {
        let mut output = InferOutput::new(&mut logits1);
        transformer.infer(&input, &mut output).unwrap();
    }

    transformer.reset();

    {
        let mut output = InferOutput::new(&mut logits2);
        transformer.infer(&input, &mut output).unwrap();
    }

    assert_eq!(logits1, logits2);
}

// ============ Policy Determinism ============

#[test]
fn test_determinism_conservative_policy() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::conservative();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 45,
        lambda_prev: 50,
        boundary_edges: 8,
        boundary_concentration_q15: 15000,
        partition_count: 6,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let witness1;
    {
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        witness1 = output.witness;
    }

    transformer.reset();

    let witness2;
    {
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        witness2 = output.witness;
    }

    assert_eq!(witness1.decision, witness2.decision);
    assert_eq!(witness1.reason, witness2.reason);
}

#[test]
fn test_determinism_permissive_policy() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::permissive();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 25,
        lambda_prev: 35,
        boundary_edges: 40,
        boundary_concentration_q15: 20000,
        partition_count: 15,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let mut results: Vec<(GateDecision, u8)> = Vec::new();
    for _ in 0..5 {
        let mut logits = vec![0i32; config.logits as usize];
        let decision;
        let tier;
        {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            decision = output.witness.decision;
            tier = output.stats.tier;
        }
        transformer.reset();
        results.push((decision, tier));
    }

    // All should be identical
    for i in 1..results.len() {
        assert_eq!(results[0].0, results[i].0);
        assert_eq!(results[0].1, results[i].1);
    }
}

// ============ Tier Determinism ============

#[test]
fn test_determinism_across_all_tiers() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();

    // Test gates for each tier
    let tier_gates = vec![
        // Tier 0
        GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        },
        // Tier 1
        GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 30,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        },
        // Tier 2
        GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: GatePacket::FLAG_FORCE_SAFE,
        },
        // Tier 3
        GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: GatePacket::FLAG_SKIP,
        },
    ];

    let tokens: Vec<u32> = (0..32).collect();

    for gate in tier_gates {
        let mut transformer = create_transformer(config.clone(), policy.clone());
        let input = InferInput::from_tokens(&tokens, gate);

        let mut results = Vec::new();
        for _ in 0..3 {
            let mut logits = vec![0i32; config.logits as usize];
            let witness;
            let stats;
            {
                let mut output = InferOutput::new(&mut logits);
                transformer.infer(&input, &mut output).unwrap();
                witness = output.witness;
                stats = output.stats;
            }
            results.push((logits, witness, stats));
            transformer.reset();
        }

        // All runs should be identical
        for i in 1..results.len() {
            assert_eq!(results[0].0, results[i].0, "Logits differ");
            assert_eq!(results[0].1.decision, results[i].1.decision);
            assert_eq!(results[0].2.tier, results[i].2.tier);
        }
    }
}

// ============ Spike Determinism ============

#[test]
fn test_determinism_with_spikes() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let spike = SpikePacket {
        fired: 1,
        rate_q15: 20000,
        novelty_q15: 15000,
        top_len: 4,
        top_idx: [5, 10, 15, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        top_w_q15: [16384, 12288, 8192, 4096, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        flags: SpikePacket::FLAG_SPARSE_MASK,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike);

    let mut results: Vec<(Vec<i32>, u8)> = Vec::new();
    for _ in 0..5 {
        let mut logits = vec![0i32; config.logits as usize];
        let tier;
        {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            tier = output.stats.tier;
        }
        transformer.reset();
        results.push((logits, tier));
    }

    // All should be identical
    for i in 1..results.len() {
        assert_eq!(results[0].0, results[i].0);
        assert_eq!(results[0].1, results[i].1);
    }
}

#[test]
fn test_determinism_inactive_spikes() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let spike = SpikePacket {
        fired: 0,
        rate_q15: 500,
        novelty_q15: 500,
        top_len: 0,
        top_idx: [0; 16],
        top_w_q15: [0; 16],
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike);

    let skip_counts: Vec<u8> = (0..10)
        .map(|_| {
            let mut logits = vec![0i32; config.logits as usize];
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            output.stats.skipped
        })
        .collect();

    // All should skip
    assert!(skip_counts.iter().all(|&s| s == 1));
}

// ============ Signature Caching Determinism ============

#[test]
fn test_cache_hit_determinism() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let signature = 54321u64;

    // First run - cache miss
    let input = InferInput::from_tokens(&tokens, gate).with_signature(signature);
    let mut logits1 = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits1);
        transformer.infer(&input, &mut output).unwrap();
    }

    // Second run - cache hit with skip flag
    let gate_skip = GatePacket {
        lambda: 100,
        flags: GatePacket::FLAG_SKIP,
        ..gate
    };

    let input = InferInput::from_tokens(&tokens, gate_skip).with_signature(signature);
    let mut logits2 = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits2);
        transformer.infer(&input, &mut output).unwrap();
    }

    // Third run - another cache hit
    let input = InferInput::from_tokens(&tokens, gate_skip).with_signature(signature);
    let mut logits3 = vec![0i32; config.logits as usize];
    {
        let mut output = InferOutput::new(&mut logits3);
        transformer.infer(&input, &mut output).unwrap();
    }

    // All cached results should match original
    assert_eq!(logits1, logits2);
    assert_eq!(logits1, logits3);
}

// ============ Lambda Pattern Determinism ============

#[test]
fn test_determinism_lambda_sequences() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();

    let lambda_sequences = vec![
        vec![100, 95, 90, 85, 80],
        vec![50, 55, 60, 65, 70],
        vec![100, 50, 100, 50, 100],
        vec![30, 30, 30, 30, 30],
    ];

    let tokens: Vec<u32> = (0..32).collect();

    for sequence in lambda_sequences {
        let mut transformer1 = create_transformer(config.clone(), policy.clone());
        let mut transformer2 = create_transformer(config.clone(), policy.clone());

        let mut results1 = Vec::new();
        let mut results2 = Vec::new();

        for (i, &lambda) in sequence.iter().enumerate() {
            let prev_lambda = if i > 0 { sequence[i - 1] } else { lambda };
            let gate = GatePacket {
                lambda,
                lambda_prev: prev_lambda,
                boundary_edges: 5,
                boundary_concentration_q15: 8192,
                partition_count: 3,
                flags: 0,
            };

            let input = InferInput::from_tokens(&tokens, gate);

            // Run on transformer1
            let mut logits1 = vec![0i32; config.logits as usize];
            let decision1;
            {
                let mut output = InferOutput::new(&mut logits1);
                transformer1.infer(&input, &mut output).unwrap();
                decision1 = output.witness.decision;
            }
            results1.push((logits1, decision1));

            // Run on transformer2
            let mut logits2 = vec![0i32; config.logits as usize];
            let decision2;
            {
                let mut output = InferOutput::new(&mut logits2);
                transformer2.infer(&input, &mut output).unwrap();
                decision2 = output.witness.decision;
            }
            results2.push((logits2, decision2));
        }

        // Both transformers should produce identical sequences
        assert_eq!(results1, results2);
    }
}

// ============ Edge Case Determinism ============

#[test]
fn test_determinism_zero_lambda() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 0,
        lambda_prev: 100,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let results: Vec<GateDecision> = (0..3)
        .map(|_| {
            let mut logits = vec![0i32; config.logits as usize];
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            transformer.reset();
            output.witness.decision
        })
        .collect();

    // All should be identical
    for i in 1..results.len() {
        assert_eq!(results[0], results[i]);
    }
}

#[test]
fn test_determinism_max_values() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: u32::MAX,
        lambda_prev: u32::MAX,
        boundary_edges: u16::MAX,
        boundary_concentration_q15: 32767,
        partition_count: u16::MAX,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let mut logits1 = vec![0i32; config.logits as usize];
    let mut logits2 = vec![0i32; config.logits as usize];

    {
        let mut output = InferOutput::new(&mut logits1);
        transformer.infer(&input, &mut output).unwrap();
    }

    transformer.reset();

    {
        let mut output = InferOutput::new(&mut logits2);
        transformer.infer(&input, &mut output).unwrap();
    }

    assert_eq!(logits1, logits2);
}

// ============ Stats Determinism ============

#[test]
fn test_stats_reproducibility() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 15,
        boundary_concentration_q15: 12000,
        partition_count: 5,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let stats_list: Vec<_> = (0..5)
        .map(|_| {
            let mut logits = vec![0i32; config.logits as usize];
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(&input, &mut output).unwrap();
            transformer.reset();
            output.stats
        })
        .collect();

    // All stats should be identical
    for i in 1..stats_list.len() {
        assert_eq!(
            stats_list[0].effective_seq_len,
            stats_list[i].effective_seq_len
        );
        assert_eq!(
            stats_list[0].effective_window,
            stats_list[i].effective_window
        );
        assert_eq!(stats_list[0].layers_executed, stats_list[i].layers_executed);
        assert_eq!(stats_list[0].tier, stats_list[i].tier);
        assert_eq!(stats_list[0].qgemm_calls, stats_list[i].qgemm_calls);
        assert_eq!(stats_list[0].attn_dot_ops, stats_list[i].attn_dot_ops);
        assert_eq!(stats_list[0].ffn_ops, stats_list[i].ffn_ops);
    }
}

// ============ Reset Determinism ============

#[test]
fn test_reset_clears_state_deterministically() {
    let config = TransformerConfig::baseline();
    let policy = GatePolicy::default();
    let mut transformer = create_transformer(config.clone(), policy);

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);

    let mut results = Vec::new();

    // Run, reset, run pattern multiple times
    for _ in 0..5 {
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);
        transformer.infer(&input, &mut output).unwrap();
        results.push(logits);
        transformer.reset();
    }

    // All results should be identical
    for i in 1..results.len() {
        assert_eq!(results[0], results[i]);
    }
}
