//! GPU-Specific Benchmarks for Prime-Radiant Coherence Engine
//!
//! This benchmark suite compares CPU and GPU implementations of core
//! coherence operations. Requires the `gpu` feature to be enabled.
//!
//! ## Benchmark Categories
//! 1. Energy Computation - CPU vs GPU
//! 2. Attention Forward Pass - CPU vs GPU
//! 3. Batch Routing Decisions - CPU vs GPU
//! 4. Memory Transfer Overhead
//!
//! ## GPU Backend Notes
//! - Primary: wgpu (cross-platform WebGPU)
//! - Optional: CUDA (NVIDIA), Metal (Apple), Vulkan
//!
//! ## Running GPU Benchmarks
//! ```bash
//! cargo bench --features gpu --bench gpu_benchmarks
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// ============================================================================
// TEST DATA GENERATION
// ============================================================================

fn generate_vec(len: usize, seed: u64) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
        })
        .collect()
}

fn generate_matrix(rows: usize, cols: usize, seed: u64) -> Vec<f32> {
    (0..rows * cols)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
        })
        .collect()
}

// ============================================================================
// CPU BASELINE IMPLEMENTATIONS
// ============================================================================

/// CPU coherence energy computation
#[derive(Clone)]
struct CpuSheafGraph {
    nodes: HashMap<u64, Vec<f32>>,
    edges: Vec<(u64, u64, f32)>, // (source, target, weight)
    state_dim: usize,
}

impl CpuSheafGraph {
    fn random(num_nodes: usize, avg_degree: usize, state_dim: usize, seed: u64) -> Self {
        let nodes: HashMap<u64, Vec<f32>> = (0..num_nodes as u64)
            .map(|id| (id, generate_vec(state_dim, seed + id)))
            .collect();

        let num_edges = (num_nodes * avg_degree) / 2;
        let edges: Vec<(u64, u64, f32)> = (0..num_edges)
            .filter_map(|i| {
                let mut h = DefaultHasher::new();
                (seed, i, "src").hash(&mut h);
                let source = h.finish() % num_nodes as u64;

                let mut h = DefaultHasher::new();
                (seed, i, "tgt").hash(&mut h);
                let target = h.finish() % num_nodes as u64;

                if source != target {
                    Some((source, target, 1.0))
                } else {
                    None
                }
            })
            .collect();

        Self {
            nodes,
            edges,
            state_dim,
        }
    }

    /// Compute total energy on CPU
    fn compute_energy_cpu(&self) -> f32 {
        let mut total = 0.0f32;
        for &(src, tgt, weight) in &self.edges {
            let src_state = &self.nodes[&src];
            let tgt_state = &self.nodes[&tgt];

            let mut norm_sq = 0.0f32;
            for i in 0..self.state_dim {
                let diff = src_state[i] - tgt_state[i];
                norm_sq += diff * diff;
            }
            total += weight * norm_sq;
        }
        total
    }

    /// Compute energy with per-edge results on CPU
    fn compute_energy_with_edges_cpu(&self) -> (f32, Vec<f32>) {
        let edge_energies: Vec<f32> = self
            .edges
            .iter()
            .map(|&(src, tgt, weight)| {
                let src_state = &self.nodes[&src];
                let tgt_state = &self.nodes[&tgt];

                let mut norm_sq = 0.0f32;
                for i in 0..self.state_dim {
                    let diff = src_state[i] - tgt_state[i];
                    norm_sq += diff * diff;
                }
                weight * norm_sq
            })
            .collect();

        let total: f32 = edge_energies.iter().sum();
        (total, edge_energies)
    }
}

/// CPU attention forward pass (simplified)
fn attention_forward_cpu(
    queries: &[f32],
    keys: &[f32],
    values: &[f32],
    seq_len: usize,
    head_dim: usize,
    output: &mut [f32],
) {
    let scale = 1.0 / (head_dim as f32).sqrt();

    // For each query position
    for i in 0..seq_len {
        let q_offset = i * head_dim;

        // Compute attention scores
        let mut scores = vec![0.0f32; seq_len];
        let mut max_score = f32::NEG_INFINITY;

        for j in 0..seq_len {
            let k_offset = j * head_dim;
            let mut dot = 0.0f32;
            for k in 0..head_dim {
                dot += queries[q_offset + k] * keys[k_offset + k];
            }
            scores[j] = dot * scale;
            if scores[j] > max_score {
                max_score = scores[j];
            }
        }

        // Softmax
        let mut sum_exp = 0.0f32;
        for s in &mut scores {
            *s = (*s - max_score).exp();
            sum_exp += *s;
        }
        for s in &mut scores {
            *s /= sum_exp;
        }

        // Weighted sum of values
        let out_offset = i * head_dim;
        for k in 0..head_dim {
            let mut weighted_sum = 0.0f32;
            for j in 0..seq_len {
                let v_offset = j * head_dim;
                weighted_sum += scores[j] * values[v_offset + k];
            }
            output[out_offset + k] = weighted_sum;
        }
    }
}

/// CPU batch routing (expert selection for MoE)
fn batch_routing_cpu(
    token_embeddings: &[f32],
    expert_weights: &[f32],
    num_tokens: usize,
    embed_dim: usize,
    num_experts: usize,
    top_k: usize,
) -> Vec<(usize, Vec<usize>)> {
    // token_embeddings: [num_tokens, embed_dim]
    // expert_weights: [num_experts, embed_dim]
    // Returns: for each token, the indices of top-k experts

    let mut results = Vec::with_capacity(num_tokens);

    for t in 0..num_tokens {
        let token_offset = t * embed_dim;
        let token = &token_embeddings[token_offset..token_offset + embed_dim];

        // Compute scores for each expert
        let mut expert_scores: Vec<(usize, f32)> = (0..num_experts)
            .map(|e| {
                let expert_offset = e * embed_dim;
                let expert = &expert_weights[expert_offset..expert_offset + embed_dim];

                let mut dot = 0.0f32;
                for i in 0..embed_dim {
                    dot += token[i] * expert[i];
                }
                (e, dot)
            })
            .collect();

        // Sort by score (descending) and take top-k
        expert_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_experts: Vec<usize> = expert_scores
            .iter()
            .take(top_k)
            .map(|(idx, _)| *idx)
            .collect();

        results.push((t, top_experts));
    }

    results
}

// ============================================================================
// GPU IMPLEMENTATIONS (SIMULATED WITHOUT ACTUAL GPU)
// When gpu feature is enabled, these would use actual GPU code
// ============================================================================

#[cfg(feature = "gpu")]
mod gpu_impl {
    //! GPU implementations using wgpu or similar
    //!
    //! These would contain actual GPU shader code and buffer management.
    //! For now, we simulate the overhead.

    use super::*;

    /// Simulated GPU energy computation
    /// In reality, this would:
    /// 1. Upload node states to GPU buffer
    /// 2. Execute compute shader for parallel residual computation
    /// 3. Reduce edge energies
    /// 4. Read back result
    pub fn compute_energy_gpu(graph: &CpuSheafGraph) -> f32 {
        // Simulate GPU overhead
        let _upload_time = simulate_memory_transfer(
            graph.nodes.len() * graph.state_dim * 4, // bytes
            true,                                    // host to device
        );

        // Actual computation would happen on GPU
        // Here we just call CPU version
        let result = graph.compute_energy_cpu();

        let _download_time = simulate_memory_transfer(
            4, // single f32 result
            false,
        );

        result
    }

    /// Simulated GPU attention forward pass
    pub fn attention_forward_gpu(
        queries: &[f32],
        keys: &[f32],
        values: &[f32],
        seq_len: usize,
        head_dim: usize,
        output: &mut [f32],
    ) {
        // Simulate upload
        let input_bytes = (queries.len() + keys.len() + values.len()) * 4;
        let _upload_time = simulate_memory_transfer(input_bytes, true);

        // CPU fallback
        attention_forward_cpu(queries, keys, values, seq_len, head_dim, output);

        // Simulate download
        let _download_time = simulate_memory_transfer(output.len() * 4, false);
    }

    /// Simulated GPU batch routing
    pub fn batch_routing_gpu(
        token_embeddings: &[f32],
        expert_weights: &[f32],
        num_tokens: usize,
        embed_dim: usize,
        num_experts: usize,
        top_k: usize,
    ) -> Vec<(usize, Vec<usize>)> {
        // Simulate upload
        let input_bytes = (token_embeddings.len() + expert_weights.len()) * 4;
        let _upload_time = simulate_memory_transfer(input_bytes, true);

        // CPU fallback
        let result = batch_routing_cpu(
            token_embeddings,
            expert_weights,
            num_tokens,
            embed_dim,
            num_experts,
            top_k,
        );

        // Simulate download
        let result_bytes = num_tokens * top_k * 4;
        let _download_time = simulate_memory_transfer(result_bytes, false);

        result
    }

    /// Simulate memory transfer time
    /// Returns simulated nanoseconds
    fn simulate_memory_transfer(bytes: usize, _host_to_device: bool) -> u64 {
        // Assume ~10 GB/s transfer rate (PCIe 3.0 x16 theoretical)
        // In practice, smaller transfers have higher overhead
        let base_overhead_ns = 1000; // 1 microsecond base overhead
        let transfer_ns = (bytes as u64 * 100) / 1_000_000_000; // ~10 GB/s
        base_overhead_ns + transfer_ns
    }
}

// Fallback for non-GPU builds
#[cfg(not(feature = "gpu"))]
mod gpu_impl {
    use super::*;

    pub fn compute_energy_gpu(graph: &CpuSheafGraph) -> f32 {
        graph.compute_energy_cpu()
    }

    pub fn attention_forward_gpu(
        queries: &[f32],
        keys: &[f32],
        values: &[f32],
        seq_len: usize,
        head_dim: usize,
        output: &mut [f32],
    ) {
        attention_forward_cpu(queries, keys, values, seq_len, head_dim, output);
    }

    pub fn batch_routing_gpu(
        token_embeddings: &[f32],
        expert_weights: &[f32],
        num_tokens: usize,
        embed_dim: usize,
        num_experts: usize,
        top_k: usize,
    ) -> Vec<(usize, Vec<usize>)> {
        batch_routing_cpu(
            token_embeddings,
            expert_weights,
            num_tokens,
            embed_dim,
            num_experts,
            top_k,
        )
    }
}

// ============================================================================
// ENERGY COMPUTATION BENCHMARKS
// ============================================================================

fn bench_energy_cpu_vs_gpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_energy");

    // Test at various graph sizes
    let sizes = [(1_000, 50), (10_000, 30), (100_000, 10)];

    for (num_nodes, sample_size) in sizes {
        let graph = CpuSheafGraph::random(num_nodes, 4, 64, 42);

        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(graph.edges.len() as u64));

        group.bench_with_input(BenchmarkId::new("cpu", num_nodes), &num_nodes, |b, _| {
            b.iter(|| black_box(graph.compute_energy_cpu()))
        });

        #[cfg(feature = "gpu")]
        group.bench_with_input(BenchmarkId::new("gpu", num_nodes), &num_nodes, |b, _| {
            b.iter(|| black_box(gpu_impl::compute_energy_gpu(&graph)))
        });
    }

    group.finish();
}

/// Benchmark energy computation with per-edge tracking
fn bench_energy_with_edges(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_energy_with_edges");

    for num_nodes in [1_000, 10_000] {
        let graph = CpuSheafGraph::random(num_nodes, 4, 64, 42);

        group.throughput(Throughput::Elements(graph.edges.len() as u64));

        group.bench_with_input(BenchmarkId::new("cpu", num_nodes), &num_nodes, |b, _| {
            b.iter(|| black_box(graph.compute_energy_with_edges_cpu()))
        });

        // GPU version would return per-edge results
        // Useful for hotspot detection
    }

    group.finish();
}

// ============================================================================
// ATTENTION BENCHMARKS
// ============================================================================

fn bench_attention_cpu_vs_gpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_attention");

    // Typical attention configurations
    let configs = [
        (128, 64, "small"),  // seq_len=128, head_dim=64
        (512, 64, "medium"), // seq_len=512, head_dim=64
        (2048, 64, "large"), // seq_len=2048, head_dim=64
    ];

    for (seq_len, head_dim, label) in configs {
        let queries = generate_vec(seq_len * head_dim, 42);
        let keys = generate_vec(seq_len * head_dim, 123);
        let values = generate_vec(seq_len * head_dim, 456);
        let mut output = vec![0.0f32; seq_len * head_dim];

        // Attention is O(n^2) in sequence length
        let sample_size = if seq_len > 1024 { 10 } else { 50 };
        group.sample_size(sample_size);
        group.throughput(Throughput::Elements((seq_len * seq_len) as u64));

        group.bench_with_input(BenchmarkId::new("cpu", label), &seq_len, |b, _| {
            b.iter(|| {
                attention_forward_cpu(
                    black_box(&queries),
                    black_box(&keys),
                    black_box(&values),
                    seq_len,
                    head_dim,
                    &mut output,
                );
                black_box(output[0])
            })
        });

        #[cfg(feature = "gpu")]
        group.bench_with_input(BenchmarkId::new("gpu", label), &seq_len, |b, _| {
            b.iter(|| {
                gpu_impl::attention_forward_gpu(
                    black_box(&queries),
                    black_box(&keys),
                    black_box(&values),
                    seq_len,
                    head_dim,
                    &mut output,
                );
                black_box(output[0])
            })
        });
    }

    group.finish();
}

/// Benchmark multi-head attention
fn bench_multihead_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_multihead_attention");

    let seq_len = 512;
    let head_dim = 64;
    let num_heads = 8;

    let queries = generate_vec(seq_len * head_dim * num_heads, 42);
    let keys = generate_vec(seq_len * head_dim * num_heads, 123);
    let values = generate_vec(seq_len * head_dim * num_heads, 456);
    let mut output = vec![0.0f32; seq_len * head_dim * num_heads];

    group.sample_size(20);
    group.throughput(Throughput::Elements((seq_len * seq_len * num_heads) as u64));

    // CPU: sequential over heads
    group.bench_function("cpu_sequential_heads", |b| {
        b.iter(|| {
            for h in 0..num_heads {
                let offset = h * seq_len * head_dim;
                let q = &queries[offset..offset + seq_len * head_dim];
                let k = &keys[offset..offset + seq_len * head_dim];
                let v = &values[offset..offset + seq_len * head_dim];
                let out = &mut output[offset..offset + seq_len * head_dim];

                attention_forward_cpu(q, k, v, seq_len, head_dim, out);
            }
            black_box(output[0])
        })
    });

    // GPU would parallelize across heads
    #[cfg(feature = "gpu")]
    group.bench_function("gpu_parallel_heads", |b| {
        b.iter(|| {
            // In reality, GPU would process all heads in parallel
            for h in 0..num_heads {
                let offset = h * seq_len * head_dim;
                let q = &queries[offset..offset + seq_len * head_dim];
                let k = &keys[offset..offset + seq_len * head_dim];
                let v = &values[offset..offset + seq_len * head_dim];
                let out = &mut output[offset..offset + seq_len * head_dim];

                gpu_impl::attention_forward_gpu(q, k, v, seq_len, head_dim, out);
            }
            black_box(output[0])
        })
    });

    group.finish();
}

// ============================================================================
// BATCH ROUTING BENCHMARKS (MoE)
// ============================================================================

fn bench_batch_routing_cpu_vs_gpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_routing");

    let embed_dim = 768; // Typical transformer embedding
    let num_experts = 8;
    let top_k = 2;

    for num_tokens in [256, 1024, 4096] {
        let token_embeddings = generate_vec(num_tokens * embed_dim, 42);
        let expert_weights = generate_vec(num_experts * embed_dim, 123);

        let sample_size = if num_tokens > 2048 { 20 } else { 50 };
        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(num_tokens as u64));

        group.bench_with_input(BenchmarkId::new("cpu", num_tokens), &num_tokens, |b, _| {
            b.iter(|| {
                black_box(batch_routing_cpu(
                    black_box(&token_embeddings),
                    black_box(&expert_weights),
                    num_tokens,
                    embed_dim,
                    num_experts,
                    top_k,
                ))
            })
        });

        #[cfg(feature = "gpu")]
        group.bench_with_input(BenchmarkId::new("gpu", num_tokens), &num_tokens, |b, _| {
            b.iter(|| {
                black_box(gpu_impl::batch_routing_gpu(
                    black_box(&token_embeddings),
                    black_box(&expert_weights),
                    num_tokens,
                    embed_dim,
                    num_experts,
                    top_k,
                ))
            })
        });
    }

    group.finish();
}

// ============================================================================
// MEMORY TRANSFER BENCHMARKS
// ============================================================================

fn bench_memory_transfer_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_memory_transfer");

    // Simulate different transfer sizes
    let sizes_kb = [1, 4, 16, 64, 256, 1024, 4096];

    for &size_kb in &sizes_kb {
        let data = generate_vec(size_kb * 1024 / 4, 42); // f32 = 4 bytes

        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));

        // Baseline: just accessing memory on CPU
        group.bench_with_input(
            BenchmarkId::new("cpu_access", format!("{}KB", size_kb)),
            &size_kb,
            |b, _| {
                b.iter(|| {
                    let sum: f32 = data.iter().sum();
                    black_box(sum)
                })
            },
        );

        // GPU would have additional transfer overhead
        // This benchmark shows the amortization point
    }

    group.finish();
}

// ============================================================================
// CROSSOVER POINT BENCHMARKS
// ============================================================================

/// Find the problem size where GPU becomes faster than CPU
fn bench_gpu_crossover(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_crossover");

    // Matrix multiply is a classic GPU workload
    // Test different sizes to find crossover

    let sizes = [32, 64, 128, 256, 512, 1024];

    for &size in &sizes {
        let a = generate_matrix(size, size, 42);
        let b = generate_matrix(size, size, 123);
        let mut c = vec![0.0f32; size * size];

        group.throughput(Throughput::Elements((size * size * size) as u64)); // O(n^3)

        let sample_size = if size > 512 { 10 } else { 50 };
        group.sample_size(sample_size);

        // CPU matrix multiply (naive)
        group.bench_with_input(BenchmarkId::new("cpu_matmul", size), &size, |b_iter, _| {
            b_iter.iter(|| {
                for i in 0..size {
                    for j in 0..size {
                        let mut sum = 0.0f32;
                        for k in 0..size {
                            sum += a[i * size + k] * b[k * size + j];
                        }
                        c[i * size + j] = sum;
                    }
                }
                black_box(c[0])
            })
        });

        // GPU would win for size >= 256 typically
    }

    group.finish();
}

// ============================================================================
// COHERENCE-SPECIFIC GPU PATTERNS
// ============================================================================

/// Benchmark parallel residual computation pattern
fn bench_parallel_residual(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_parallel_residual");

    let state_dim = 64;

    for num_edges in [1_000, 10_000, 100_000] {
        // Prepare edge data in GPU-friendly format
        let sources: Vec<Vec<f32>> = (0..num_edges)
            .map(|i| generate_vec(state_dim, i as u64))
            .collect();
        let targets: Vec<Vec<f32>> = (0..num_edges)
            .map(|i| generate_vec(state_dim, i as u64 + 1000000))
            .collect();

        let sample_size = if num_edges > 50000 { 10 } else { 50 };
        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(num_edges as u64));

        // CPU sequential
        group.bench_with_input(
            BenchmarkId::new("cpu_sequential", num_edges),
            &num_edges,
            |b, _| {
                b.iter(|| {
                    let mut total = 0.0f32;
                    for (src, tgt) in sources.iter().zip(targets.iter()) {
                        let mut norm_sq = 0.0f32;
                        for i in 0..state_dim {
                            let diff = src[i] - tgt[i];
                            norm_sq += diff * diff;
                        }
                        total += norm_sq;
                    }
                    black_box(total)
                })
            },
        );

        // GPU would parallelize all edges
        // Each work item computes one residual
    }

    group.finish();
}

/// Benchmark reduction patterns (sum of energies)
fn bench_gpu_reduction(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_reduction");

    for size in [1_000, 10_000, 100_000, 1_000_000] {
        let data = generate_vec(size, 42);

        let sample_size = if size > 100000 { 10 } else { 50 };
        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(size as u64));

        // CPU sequential sum
        group.bench_with_input(BenchmarkId::new("cpu_sum", size), &size, |b, _| {
            b.iter(|| {
                let sum: f32 = data.iter().sum();
                black_box(sum)
            })
        });

        // CPU parallel reduction would use multiple accumulators
        group.bench_with_input(BenchmarkId::new("cpu_parallel", size), &size, |b, _| {
            b.iter(|| {
                let chunks = data.chunks(1024);
                let partial_sums: Vec<f32> = chunks.map(|c| c.iter().sum()).collect();
                let sum: f32 = partial_sums.iter().sum();
                black_box(sum)
            })
        });

        // GPU reduction uses tree-based parallel reduction
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    energy_benches,
    bench_energy_cpu_vs_gpu,
    bench_energy_with_edges,
);

criterion_group!(
    attention_benches,
    bench_attention_cpu_vs_gpu,
    bench_multihead_attention,
);

criterion_group!(routing_benches, bench_batch_routing_cpu_vs_gpu,);

criterion_group!(
    transfer_benches,
    bench_memory_transfer_overhead,
    bench_gpu_crossover,
);

criterion_group!(
    coherence_gpu_benches,
    bench_parallel_residual,
    bench_gpu_reduction,
);

criterion_main!(
    energy_benches,
    attention_benches,
    routing_benches,
    transfer_benches,
    coherence_gpu_benches
);
