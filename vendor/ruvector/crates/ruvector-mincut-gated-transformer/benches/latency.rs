//! Latency benchmarks for mincut gated transformer.
//!
//! Tests inference latency across different tiers and configurations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_mincut_gated_transformer::{
    GatePacket, GatePolicy, InferInput, InferOutput, MincutGatedTransformer, QuantizedWeights,
    SpikePacket, TransformerConfig,
};

fn create_transformer(config: TransformerConfig) -> MincutGatedTransformer {
    let policy = GatePolicy::default();
    let weights = QuantizedWeights::empty(&config);
    MincutGatedTransformer::new(config, policy, weights).unwrap()
}

fn bench_tier0_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("tier0_inference");

    for seq_len in [16, 32, 64].iter() {
        let mut config = TransformerConfig::baseline();
        config.seq_len_max = *seq_len;
        config.seq_len_degraded = seq_len / 2;
        config.seq_len_safe = seq_len / 8;
        let mut transformer = create_transformer(config.clone());

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let tokens: Vec<u32> = (0..*seq_len as u32).collect();
        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];

        group.bench_with_input(BenchmarkId::from_parameter(seq_len), seq_len, |b, _| {
            b.iter(|| {
                let mut output = InferOutput::new(&mut logits);
                transformer.infer(black_box(&input), &mut output).unwrap();
                black_box(output.witness)
            })
        });
    }

    group.finish();
}

fn bench_tier1_degraded(c: &mut Criterion) {
    let mut group = c.benchmark_group("tier1_degraded");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    // Gate packet that triggers tier 1 (ReduceScope)
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Above default max of 20
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let tokens: Vec<u32> = (0..64).collect();
    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];

    group.bench_function("baseline_64_degraded", |b| {
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_tier2_safe(c: &mut Criterion) {
    let mut group = c.benchmark_group("tier2_safe");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    // Gate packet that triggers tier 2 (FreezeWrites via force flag)
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_FORCE_SAFE,
    };

    let tokens: Vec<u32> = (0..64).collect();
    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];

    group.bench_function("baseline_64_safe", |b| {
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_tier3_skip(c: &mut Criterion) {
    let mut group = c.benchmark_group("tier3_skip");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    // Gate packet that triggers skip
    let gate = GatePacket {
        lambda: 100,
        flags: GatePacket::FLAG_SKIP,
        ..Default::default()
    };

    let tokens: Vec<u32> = (0..64).collect();
    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];

    group.bench_function("baseline_64_skip", |b| {
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_spike_inactive_skip(c: &mut Criterion) {
    let mut group = c.benchmark_group("spike_inactive");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        ..Default::default()
    };

    let spike = SpikePacket {
        fired: 0, // Not fired - should skip
        ..Default::default()
    };

    let tokens: Vec<u32> = (0..64).collect();
    let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike);
    let mut logits = vec![0i32; config.logits as usize];

    group.bench_function("baseline_64_spike_inactive", |b| {
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_window_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("window_sweep");

    for window in [4, 8, 16, 32].iter() {
        let mut config = TransformerConfig::baseline();
        config.window_normal = *window;
        config.window_degraded = window / 2;
        let mut transformer = create_transformer(config.clone());

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 5,
            ..Default::default()
        };

        let tokens: Vec<u32> = (0..64).collect();
        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];

        group.bench_with_input(BenchmarkId::from_parameter(window), window, |b, _| {
            b.iter(|| {
                let mut output = InferOutput::new(&mut logits);
                transformer.infer(black_box(&input), &mut output).unwrap();
                black_box(output.witness)
            })
        });
    }

    group.finish();
}

fn bench_micro_config(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_config");

    let config = TransformerConfig::micro();
    let mut transformer = create_transformer(config.clone());

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        ..Default::default()
    };

    let tokens: Vec<u32> = (0..32).collect();
    let input = InferInput::from_tokens(&tokens, gate);
    let mut logits = vec![0i32; config.logits as usize];

    group.bench_function("micro_32", |b| {
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_mod_routing_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("mod_routing");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..64).collect();

    // Baseline without routing overhead
    let gate_normal = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("no_routing_overhead", |b| {
        let input = InferInput::from_tokens(&tokens, gate_normal);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // With routing overhead (boundary spike)
    let gate_routing = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("with_routing_overhead", |b| {
        let input = InferInput::from_tokens(&tokens, gate_routing);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_early_exit_speedup(c: &mut Criterion) {
    let mut group = c.benchmark_group("early_exit");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..64).collect();

    // Full execution
    let gate_full = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("full_execution", |b| {
        let input = InferInput::from_tokens(&tokens, gate_full);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // Early exit (tier 1)
    let gate_exit = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("early_exit_tier1", |b| {
        let input = InferInput::from_tokens(&tokens, gate_exit);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // Minimal execution (tier 2)
    let gate_minimal = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_FORCE_SAFE,
    };

    group.bench_function("minimal_tier2", |b| {
        let input = InferInput::from_tokens(&tokens, gate_minimal);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_sparse_vs_dense_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_attention");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..64).collect();

    // Dense attention (normal window)
    let gate_dense = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("dense_attention", |b| {
        let input = InferInput::from_tokens(&tokens, gate_dense);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // Sparse attention (reduced scope)
    let gate_sparse = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("sparse_attention", |b| {
        let input = InferInput::from_tokens(&tokens, gate_sparse);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_spike_vs_standard_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("spike_attention");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..64).collect();
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    // Standard (no spikes)
    group.bench_function("standard_no_spikes", |b| {
        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // With active spikes
    let spike_active = SpikePacket {
        fired: 1,
        rate_q15: 20000,
        novelty_q15: 15000,
        top_len: 8,
        top_idx: [2, 8, 14, 20, 26, 32, 38, 44, 0, 0, 0, 0, 0, 0, 0, 0],
        top_w_q15: [14336; 16],
        flags: SpikePacket::FLAG_SPARSE_MASK,
    };

    group.bench_function("with_active_spikes", |b| {
        let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike_active);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // With inactive spikes (skip path)
    let spike_inactive = SpikePacket {
        fired: 0,
        rate_q15: 500,
        novelty_q15: 500,
        top_len: 0,
        top_idx: [0; 16],
        top_w_q15: [0; 16],
        flags: 0,
    };

    group.bench_function("inactive_spikes_skip", |b| {
        let input = InferInput::from_tokens(&tokens, gate).with_spikes(spike_inactive);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_lambda_drop_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("lambda_patterns");

    let config = TransformerConfig::baseline();
    let mut transformer = create_transformer(config.clone());

    let tokens: Vec<u32> = (0..64).collect();

    // Stable lambda
    let gate_stable = GatePacket {
        lambda: 100,
        lambda_prev: 98,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("stable_lambda", |b| {
        let input = InferInput::from_tokens(&tokens, gate_stable);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // Fast lambda drop
    let gate_drop = GatePacket {
        lambda: 40,
        lambda_prev: 100,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    group.bench_function("fast_lambda_drop", |b| {
        let input = InferInput::from_tokens(&tokens, gate_drop);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

fn bench_policy_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_comparison");

    let config = TransformerConfig::baseline();
    let tokens: Vec<u32> = (0..64).collect();

    let gate = GatePacket {
        lambda: 45,
        lambda_prev: 50,
        boundary_edges: 12,
        boundary_concentration_q15: 15000,
        partition_count: 6,
        flags: 0,
    };

    // Default policy
    group.bench_function("default_policy", |b| {
        let policy = GatePolicy::default();
        let weights = QuantizedWeights::empty(&config);
        let mut transformer = MincutGatedTransformer::new(config.clone(), policy, weights).unwrap();
        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // Conservative policy
    group.bench_function("conservative_policy", |b| {
        let policy = GatePolicy::conservative();
        let weights = QuantizedWeights::empty(&config);
        let mut transformer = MincutGatedTransformer::new(config.clone(), policy, weights).unwrap();
        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    // Permissive policy
    group.bench_function("permissive_policy", |b| {
        let policy = GatePolicy::permissive();
        let weights = QuantizedWeights::empty(&config);
        let mut transformer = MincutGatedTransformer::new(config.clone(), policy, weights).unwrap();
        let input = InferInput::from_tokens(&tokens, gate);
        let mut logits = vec![0i32; config.logits as usize];
        b.iter(|| {
            let mut output = InferOutput::new(&mut logits);
            transformer.infer(black_box(&input), &mut output).unwrap();
            black_box(output.witness)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tier0_inference,
    bench_tier1_degraded,
    bench_tier2_safe,
    bench_tier3_skip,
    bench_spike_inactive_skip,
    bench_window_sizes,
    bench_micro_config,
    bench_mod_routing_overhead,
    bench_early_exit_speedup,
    bench_sparse_vs_dense_attention,
    bench_spike_vs_standard_attention,
    bench_lambda_drop_patterns,
    bench_policy_variants,
);

criterion_main!(benches);
