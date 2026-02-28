//! Attention Mechanism Latency Benchmarks
//!
//! Benchmark each attention mechanism at 100 tokens.
//! Target: <100 microseconds per mechanism.
//!
//! Run with: cargo bench --bench attention_latency

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

/// Generate random f32 vector for benchmarking
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    (0..dim)
        .map(|i| {
            let x = ((seed.wrapping_mul(i as u64 + 1).wrapping_mul(0x5DEECE66D)) % 1000) as f32;
            (x / 500.0) - 1.0 // Range [-1, 1]
        })
        .collect()
}

/// Generate batch of random vectors
fn random_vectors(count: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    (0..count)
        .map(|i| random_vector(dim, seed.wrapping_add(i as u64)))
        .collect()
}

fn bench_all_attention_mechanisms(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_mechanisms");

    // Test parameters
    let dim = 64;
    let num_heads = 8;
    let seq_len = 100; // Target: 100 tokens

    // Generate test data
    let queries = random_vectors(seq_len, dim, 42);
    let keys = random_vectors(seq_len, dim, 123);
    let values = random_vectors(seq_len, dim, 456);

    // Set throughput for tokens/second calculation
    group.throughput(Throughput::Elements(seq_len as u64));

    // ========================================================================
    // Multi-Head Attention Benchmark
    // ========================================================================

    group.bench_function("multi_head_attention", |b| {
        // TODO: When implemented:
        // let attention = MultiHeadAttention::new(dim, num_heads);

        b.iter(|| {
            // TODO: Replace with actual attention computation
            // attention.forward(&queries, &keys, &values)

            // Placeholder: simulate attention computation
            let mut output = vec![0.0f32; dim];
            for q in &queries {
                for (k, v) in keys.iter().zip(values.iter()) {
                    let score: f32 = q.iter().zip(k.iter()).map(|(a, b)| a * b).sum();
                    for (o, vi) in output.iter_mut().zip(v.iter()) {
                        *o += score * vi * 0.001;
                    }
                }
            }
            output
        });
    });

    // ========================================================================
    // Mamba SSM Benchmark
    // ========================================================================

    group.bench_function("mamba_ssm", |b| {
        // TODO: When implemented:
        // let mamba = MambaSSM::new(dim);

        b.iter(|| {
            // TODO: Replace with actual Mamba SSM computation
            // mamba.forward(&queries)

            // Placeholder: simulate O(n) selective scan
            let mut hidden = vec![0.0f32; dim];
            for input in &queries {
                for (h, x) in hidden.iter_mut().zip(input.iter()) {
                    *h = *h * 0.9 + *x * 0.1;
                }
            }
            hidden
        });
    });

    // ========================================================================
    // RWKV Attention Benchmark
    // ========================================================================

    group.bench_function("rwkv_attention", |b| {
        // TODO: When implemented:
        // let rwkv = RWKVAttention::new(dim);

        b.iter(|| {
            // TODO: Replace with actual RWKV computation
            // rwkv.forward(&queries)

            // Placeholder: simulate linear attention
            let mut state = vec![0.0f32; dim];
            for input in &queries {
                for (s, x) in state.iter_mut().zip(input.iter()) {
                    *s = *s * 0.95 + *x;
                }
            }
            state
        });
    });

    // ========================================================================
    // Flash Attention Approximation Benchmark
    // ========================================================================

    group.bench_function("flash_attention_approx", |b| {
        // TODO: When implemented:
        // let flash = FlashAttention::new(dim);

        b.iter(|| {
            // TODO: Replace with actual Flash Attention
            // flash.forward(&queries, &keys, &values)

            // Placeholder: simulate tiled computation
            let tile_size = 16;
            let mut output = vec![0.0f32; dim];
            for tile_start in (0..seq_len).step_by(tile_size) {
                let tile_end = (tile_start + tile_size).min(seq_len);
                for i in tile_start..tile_end {
                    for j in 0..dim {
                        output[j] += queries[i][j] * 0.01;
                    }
                }
            }
            output
        });
    });

    // ========================================================================
    // Hyperbolic Attention Benchmark
    // ========================================================================

    group.bench_function("hyperbolic_attention", |b| {
        // TODO: When implemented:
        // let hyp_attn = HyperbolicAttention::new(dim, -1.0);

        b.iter(|| {
            // TODO: Replace with actual hyperbolic attention
            // hyp_attn.forward(&queries[0], &keys, &values)

            // Placeholder: simulate Poincare operations
            let query = &queries[0];
            let mut output = vec![0.0f32; dim];
            for (k, v) in keys.iter().zip(values.iter()) {
                // Simplified Poincare distance
                let dist: f32 = query.iter().zip(k.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt();
                let weight = (-dist).exp();
                for (o, vi) in output.iter_mut().zip(v.iter()) {
                    *o += weight * vi;
                }
            }
            output
        });
    });

    group.finish();
}

fn bench_attention_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_scaling");

    let dim = 64;

    // Test different sequence lengths
    for seq_len in [32, 64, 128, 256, 512].iter() {
        let queries = random_vectors(*seq_len, dim, 42);
        let keys = random_vectors(*seq_len, dim, 123);
        let values = random_vectors(*seq_len, dim, 456);

        group.throughput(Throughput::Elements(*seq_len as u64));

        group.bench_with_input(
            BenchmarkId::new("multi_head", seq_len),
            &(&queries, &keys, &values),
            |b, (q, k, v)| {
                b.iter(|| {
                    // TODO: Replace with actual attention
                    let mut output = vec![0.0f32; dim];
                    for qi in q.iter() {
                        for (ki, vi) in k.iter().zip(v.iter()) {
                            let score: f32 = qi.iter().zip(ki.iter())
                                .map(|(a, b)| a * b).sum();
                            for (o, vij) in output.iter_mut().zip(vi.iter()) {
                                *o += score * vij * 0.001;
                            }
                        }
                    }
                    output
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("mamba_ssm", seq_len),
            &(&queries,),
            |b, (input,)| {
                b.iter(|| {
                    // TODO: Replace with actual Mamba SSM
                    let mut hidden = vec![0.0f32; dim];
                    for inp in input.iter() {
                        for (h, x) in hidden.iter_mut().zip(inp.iter()) {
                            *h = *h * 0.9 + *x * 0.1;
                        }
                    }
                    hidden
                });
            },
        );
    }

    group.finish();
}

fn bench_attention_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_memory");

    // Test memory-efficient vs standard attention
    let dim = 64;
    let seq_len = 256;

    let queries = random_vectors(seq_len, dim, 42);
    let keys = random_vectors(seq_len, dim, 123);
    let values = random_vectors(seq_len, dim, 456);

    group.bench_function("standard_attention", |b| {
        b.iter(|| {
            // Full attention matrix: O(n^2) memory
            let mut attn_matrix = vec![vec![0.0f32; seq_len]; seq_len];
            for i in 0..seq_len {
                for j in 0..seq_len {
                    attn_matrix[i][j] = queries[i].iter()
                        .zip(keys[j].iter())
                        .map(|(a, b)| a * b)
                        .sum();
                }
            }
            attn_matrix
        });
    });

    group.bench_function("memory_efficient_attention", |b| {
        b.iter(|| {
            // Compute attention row by row: O(n) memory
            let mut output = vec![vec![0.0f32; dim]; seq_len];
            for i in 0..seq_len {
                let mut scores = vec![0.0f32; seq_len];
                for j in 0..seq_len {
                    scores[j] = queries[i].iter()
                        .zip(keys[j].iter())
                        .map(|(a, b)| a * b)
                        .sum();
                }
                // Softmax
                let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let exp_sum: f32 = scores.iter().map(|s| (s - max).exp()).sum();
                for (j, score) in scores.iter().enumerate() {
                    let weight = (score - max).exp() / exp_sum;
                    for (k, v) in output[i].iter_mut().zip(values[j].iter()) {
                        *k += weight * v;
                    }
                }
            }
            output
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_all_attention_mechanisms,
    bench_attention_scaling,
    bench_attention_memory
);

criterion_main!(benches);
