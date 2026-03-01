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
//! MicroLoRA Benchmarks for M4 Pro
//!
//! Benchmarks for LoRA adapter operations:
//! - Forward pass latency
//! - SIMD-optimized forward
//! - Gradient accumulation
//! - EWC++ overhead
//! - Adaptation speed
//!
//! Performance targets for M4 Pro:
//! - MicroLoRA forward (rank=2, dim=768): <500us
//! - MicroLoRA forward (rank=2, dim=4096): <1ms
//! - Gradient accumulation: <100us
//! - EWC++ update: <200us

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

/// Target modules for LoRA adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TargetModule {
    QProj,
    VProj,
}

/// Single LoRA adapter for benchmarking
#[derive(Clone)]
struct LoraAdapter {
    lora_a: Vec<f32>,
    lora_b: Vec<f32>,
    in_features: usize,
    out_features: usize,
    rank: usize,
    scaling: f32,
    // Gradients
    grad_a: Vec<f32>,
    grad_b: Vec<f32>,
    grad_count: usize,
}

impl LoraAdapter {
    fn new(in_features: usize, out_features: usize, rank: usize, alpha: f32) -> Self {
        let scaling = alpha / rank as f32;

        // Kaiming initialization for A
        let std_a = (2.0 / in_features as f32).sqrt() * 0.01;
        let lora_a: Vec<f32> = (0..in_features * rank)
            .map(|idx| {
                let seed = idx as f32;
                ((seed * 0.618033988749895) % 1.0 - 0.5) * 2.0 * std_a
            })
            .collect();

        // Zero initialization for B
        let lora_b = vec![0.0; rank * out_features];

        Self {
            lora_a,
            lora_b,
            in_features,
            out_features,
            rank,
            scaling,
            grad_a: vec![0.0; in_features * rank],
            grad_b: vec![0.0; rank * out_features],
            grad_count: 0,
        }
    }

    /// Forward pass: output = x @ A @ B * scaling
    fn forward(&self, x: &[f32]) -> Vec<f32> {
        debug_assert_eq!(x.len(), self.in_features);

        // Down projection: x @ A -> intermediate (rank,)
        let mut intermediate = vec![0.0f32; self.rank];
        for r in 0..self.rank {
            let mut sum = 0.0f32;
            for i in 0..self.in_features {
                sum += x[i] * self.lora_a[i * self.rank + r];
            }
            intermediate[r] = sum;
        }

        // Up projection: intermediate @ B -> output (out_features,)
        let mut output = vec![0.0f32; self.out_features];
        for o in 0..self.out_features {
            let mut sum = 0.0f32;
            for r in 0..self.rank {
                sum += intermediate[r] * self.lora_b[r * self.out_features + o];
            }
            output[o] = sum * self.scaling;
        }

        output
    }

    /// SIMD-optimized forward for flat f32 slices (adds to output)
    fn forward_simd(&self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), self.in_features);
        debug_assert_eq!(output.len(), self.out_features);

        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.forward_simd_neon(input, output);
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            self.forward_simd_scalar(input, output);
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    unsafe fn forward_simd_neon(&self, input: &[f32], output: &mut [f32]) {
        use std::arch::aarch64::*;

        // Down projection with NEON
        let mut intermediate = vec![0.0f32; self.rank];

        for r in 0..self.rank {
            let mut sum = vdupq_n_f32(0.0);
            let chunks = self.in_features / 4;
            let mut i = 0;

            for _ in 0..chunks {
                let x_v = vld1q_f32(input.as_ptr().add(i));
                // Load A column (strided access - not ideal but works for small rank)
                let a_vals = [
                    self.lora_a[i * self.rank + r],
                    self.lora_a[(i + 1) * self.rank + r],
                    self.lora_a[(i + 2) * self.rank + r],
                    self.lora_a[(i + 3) * self.rank + r],
                ];
                let a_v = vld1q_f32(a_vals.as_ptr());
                sum = vfmaq_f32(sum, x_v, a_v);
                i += 4;
            }

            let mut sum_val = vaddvq_f32(sum);
            for ii in i..self.in_features {
                sum_val += input[ii] * self.lora_a[ii * self.rank + r];
            }
            intermediate[r] = sum_val;
        }

        // Up projection with NEON
        let scaling_vec = vdupq_n_f32(self.scaling);
        let chunks = self.out_features / 4;
        let mut o = 0;

        for _ in 0..chunks {
            let mut out_v = vld1q_f32(output.as_ptr().add(o));

            for r in 0..self.rank {
                let inter_val = vdupq_n_f32(intermediate[r]);
                let b_v = vld1q_f32(self.lora_b.as_ptr().add(r * self.out_features + o));
                out_v = vfmaq_f32(out_v, vmulq_f32(inter_val, b_v), scaling_vec);
            }

            vst1q_f32(output.as_mut_ptr().add(o), out_v);
            o += 4;
        }

        // Remaining elements
        for oo in o..self.out_features {
            let mut sum = output[oo];
            for r in 0..self.rank {
                sum += intermediate[r] * self.lora_b[r * self.out_features + oo] * self.scaling;
            }
            output[oo] = sum;
        }
    }

    #[allow(dead_code)]
    fn forward_simd_scalar(&self, input: &[f32], output: &mut [f32]) {
        let mut intermediate = vec![0.0f32; self.rank];

        for r in 0..self.rank {
            let mut sum = 0.0f32;
            for i in 0..self.in_features {
                sum += input[i] * self.lora_a[i * self.rank + r];
            }
            intermediate[r] = sum;
        }

        for o in 0..self.out_features {
            let mut sum = output[o];
            for r in 0..self.rank {
                sum += intermediate[r] * self.lora_b[r * self.out_features + o] * self.scaling;
            }
            output[o] = sum;
        }
    }

    /// Batched forward pass for efficiency
    fn forward_batch(&self, x: &[f32], batch_size: usize) -> Vec<f32> {
        debug_assert_eq!(x.len(), batch_size * self.in_features);

        let mut outputs = vec![0.0f32; batch_size * self.out_features];

        for b in 0..batch_size {
            let input_offset = b * self.in_features;
            let output_offset = b * self.out_features;

            let input = &x[input_offset..input_offset + self.in_features];
            let output = &mut outputs[output_offset..output_offset + self.out_features];

            self.forward_simd(input, output);
        }

        outputs
    }

    /// Compute gradients for REINFORCE-style update
    fn accumulate_gradient(&mut self, input: &[f32], grad_output: &[f32], reward: f32) {
        debug_assert_eq!(input.len(), self.in_features);
        debug_assert_eq!(grad_output.len(), self.out_features);

        // Compute intermediate activation
        let mut intermediate = vec![0.0f32; self.rank];
        for r in 0..self.rank {
            let mut sum = 0.0f32;
            for i in 0..self.in_features {
                sum += input[i] * self.lora_a[i * self.rank + r];
            }
            intermediate[r] = sum;
        }

        // Gradient for B: outer(intermediate, grad_output) * reward * scaling
        for r in 0..self.rank {
            for o in 0..self.out_features {
                self.grad_b[r * self.out_features + o] +=
                    intermediate[r] * grad_output[o] * reward * self.scaling;
            }
        }

        // Gradient for A: input outer grad_intermediate
        // grad_intermediate = grad_output @ B.T * reward * scaling
        let mut grad_intermediate = vec![0.0f32; self.rank];
        for r in 0..self.rank {
            let mut sum = 0.0f32;
            for o in 0..self.out_features {
                sum += grad_output[o] * self.lora_b[r * self.out_features + o];
            }
            grad_intermediate[r] = sum * reward * self.scaling;
        }

        for i in 0..self.in_features {
            for r in 0..self.rank {
                self.grad_a[i * self.rank + r] += input[i] * grad_intermediate[r];
            }
        }

        self.grad_count += 1;
    }

    /// Apply accumulated gradients with learning rate
    fn apply_gradients(&mut self, learning_rate: f32) {
        if self.grad_count == 0 {
            return;
        }

        let scale = learning_rate / self.grad_count as f32;

        for i in 0..self.lora_a.len() {
            self.lora_a[i] -= self.grad_a[i] * scale;
            self.grad_a[i] = 0.0;
        }

        for i in 0..self.lora_b.len() {
            self.lora_b[i] -= self.grad_b[i] * scale;
            self.grad_b[i] = 0.0;
        }

        self.grad_count = 0;
    }

    /// Apply gradients with EWC++ regularization
    fn apply_gradients_with_ewc(
        &mut self,
        learning_rate: f32,
        fisher_a: &[f32],
        fisher_b: &[f32],
        optimal_a: &[f32],
        optimal_b: &[f32],
        ewc_lambda: f32,
    ) {
        if self.grad_count == 0 {
            return;
        }

        let scale = learning_rate / self.grad_count as f32;

        // Update A with EWC regularization
        for i in 0..self.lora_a.len() {
            let grad = self.grad_a[i] * scale;
            let ewc_penalty = ewc_lambda * fisher_a[i] * (self.lora_a[i] - optimal_a[i]);
            self.lora_a[i] -= grad + ewc_penalty * learning_rate;
            self.grad_a[i] = 0.0;
        }

        // Update B with EWC regularization
        for i in 0..self.lora_b.len() {
            let grad = self.grad_b[i] * scale;
            let ewc_penalty = ewc_lambda * fisher_b[i] * (self.lora_b[i] - optimal_b[i]);
            self.lora_b[i] -= grad + ewc_penalty * learning_rate;
            self.grad_b[i] = 0.0;
        }

        self.grad_count = 0;
    }

    fn param_count(&self) -> usize {
        self.lora_a.len() + self.lora_b.len()
    }

    fn memory_bytes(&self) -> usize {
        self.param_count() * std::mem::size_of::<f32>()
    }
}

/// EWC state for benchmarking
struct EwcState {
    fisher_a: Vec<f32>,
    fisher_b: Vec<f32>,
    optimal_a: Vec<f32>,
    optimal_b: Vec<f32>,
}

impl EwcState {
    fn from_adapter(adapter: &LoraAdapter) -> Self {
        Self {
            fisher_a: vec![0.01; adapter.lora_a.len()],
            fisher_b: vec![0.01; adapter.lora_b.len()],
            optimal_a: adapter.lora_a.clone(),
            optimal_b: adapter.lora_b.clone(),
        }
    }

    fn update_fisher(&mut self, grad_a: &[f32], grad_b: &[f32], decay: f32) {
        for i in 0..self.fisher_a.len() {
            self.fisher_a[i] = decay * self.fisher_a[i] + (1.0 - decay) * grad_a[i] * grad_a[i];
        }
        for i in 0..self.fisher_b.len() {
            self.fisher_b[i] = decay * self.fisher_b[i] + (1.0 - decay) * grad_b[i] * grad_b[i];
        }
    }
}

// Helper function to generate random tensor data
fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

// === Benchmark Functions ===

fn bench_lora_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_forward");
    group.sample_size(100);

    for (in_features, out_features) in [(768, 768), (2048, 2048), (4096, 4096)] {
        for rank in [1, 2] {
            let adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);
            let input = random_tensor(in_features);

            let id = BenchmarkId::new(
                format!("dim_{}_rank_{}", in_features, rank),
                adapter.param_count(),
            );

            group.throughput(Throughput::Elements(adapter.param_count() as u64));
            group.bench_function(id, |b| b.iter(|| adapter.forward(black_box(&input))));
        }
    }

    group.finish();
}

fn bench_lora_forward_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_forward_simd");
    group.sample_size(100);

    for (in_features, out_features) in [(768, 768), (2048, 2048), (4096, 4096)] {
        for rank in [1, 2] {
            let adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);
            let input = random_tensor(in_features);
            let mut output = vec![0.0f32; out_features];

            let id = BenchmarkId::new(
                format!("dim_{}_rank_{}", in_features, rank),
                adapter.param_count(),
            );

            group.throughput(Throughput::Elements(adapter.param_count() as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    output.fill(0.0);
                    adapter.forward_simd(black_box(&input), black_box(&mut output));
                })
            });
        }
    }

    group.finish();
}

fn bench_lora_forward_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_forward_batch");
    group.sample_size(50);

    let in_features = 4096;
    let out_features = 4096;
    let rank = 2;

    let adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);

    for batch_size in [1, 8, 16, 32, 64] {
        let input = random_tensor(batch_size * in_features);

        let id = BenchmarkId::new(format!("batch_{}", batch_size), batch_size);

        group.throughput(Throughput::Elements(
            (batch_size * adapter.param_count()) as u64,
        ));
        group.bench_function(id, |b| {
            b.iter(|| adapter.forward_batch(black_box(&input), batch_size))
        });
    }

    group.finish();
}

fn bench_lora_gradient_accumulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_gradient_accumulation");
    group.sample_size(100);

    for (in_features, out_features) in [(768, 768), (2048, 2048), (4096, 4096)] {
        let rank = 2;
        let mut adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);
        let input = random_tensor(in_features);
        let grad_output = random_tensor(out_features);

        let id = BenchmarkId::new(format!("dim_{}", in_features), in_features);

        group.throughput(Throughput::Elements(adapter.param_count() as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                adapter.accumulate_gradient(black_box(&input), black_box(&grad_output), 0.8);
            })
        });
    }

    group.finish();
}

fn bench_lora_apply_gradients(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_apply_gradients");
    group.sample_size(100);

    for (in_features, out_features) in [(768, 768), (2048, 2048), (4096, 4096)] {
        let rank = 2;
        let mut adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);
        let input = random_tensor(in_features);
        let grad_output = random_tensor(out_features);

        // Accumulate some gradients first
        for _ in 0..10 {
            adapter.accumulate_gradient(&input, &grad_output, 0.8);
        }

        let id = BenchmarkId::new(format!("dim_{}", in_features), in_features);

        group.throughput(Throughput::Elements(adapter.param_count() as u64));
        group.bench_function(id, |b| {
            b.iter_batched(
                || {
                    let mut a = adapter.clone();
                    for _ in 0..10 {
                        a.accumulate_gradient(&input, &grad_output, 0.8);
                    }
                    a
                },
                |mut a| {
                    a.apply_gradients(black_box(0.01));
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_lora_ewc_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_ewc_update");
    group.sample_size(100);

    for (in_features, out_features) in [(768, 768), (2048, 2048), (4096, 4096)] {
        let rank = 2;
        let adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);
        let ewc = EwcState::from_adapter(&adapter);
        let input = random_tensor(in_features);
        let grad_output = random_tensor(out_features);

        let id = BenchmarkId::new(format!("dim_{}", in_features), in_features);

        group.throughput(Throughput::Elements(adapter.param_count() as u64));
        group.bench_function(id, |b| {
            b.iter_batched(
                || {
                    let mut a = adapter.clone();
                    for _ in 0..10 {
                        a.accumulate_gradient(&input, &grad_output, 0.8);
                    }
                    a
                },
                |mut a| {
                    a.apply_gradients_with_ewc(
                        black_box(0.01),
                        black_box(&ewc.fisher_a),
                        black_box(&ewc.fisher_b),
                        black_box(&ewc.optimal_a),
                        black_box(&ewc.optimal_b),
                        black_box(0.1),
                    );
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_lora_adaptation_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_adaptation_cycle");
    group.sample_size(50);

    // Full adaptation cycle: forward + gradient + apply
    for (in_features, out_features) in [(768, 768), (2048, 2048), (4096, 4096)] {
        let rank = 2;
        let input = random_tensor(in_features);
        let grad_output = random_tensor(out_features);

        let id = BenchmarkId::new(format!("dim_{}", in_features), in_features);

        group.bench_function(id, |b| {
            b.iter_batched(
                || LoraAdapter::new(in_features, out_features, rank, 4.0),
                |mut adapter| {
                    // Forward
                    let _output = adapter.forward(black_box(&input));
                    // Gradient
                    adapter.accumulate_gradient(black_box(&input), black_box(&grad_output), 0.8);
                    // Apply
                    adapter.apply_gradients(black_box(0.01));
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_lora_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_memory");
    group.sample_size(100);

    // Test memory efficiency at different scales
    let configs = [
        ("rank1_768", 768, 768, 1),
        ("rank2_768", 768, 768, 2),
        ("rank1_4096", 4096, 4096, 1),
        ("rank2_4096", 4096, 4096, 2),
        ("rank2_4096x11008", 4096, 11008, 2), // MLP-like
    ];

    for (name, in_features, out_features, rank) in configs {
        let adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);
        let input = random_tensor(in_features);

        let memory_bytes = adapter.memory_bytes();

        let id = BenchmarkId::new(format!("{}_{}KB", name, memory_bytes / 1024), memory_bytes);

        group.throughput(Throughput::Bytes(memory_bytes as u64));
        group.bench_function(id, |b| b.iter(|| adapter.forward(black_box(&input))));
    }

    group.finish();
}

fn bench_ewc_fisher_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("ewc_fisher_update");
    group.sample_size(100);

    for (in_features, out_features) in [(768, 768), (2048, 2048), (4096, 4096)] {
        let rank = 2;
        let adapter = LoraAdapter::new(in_features, out_features, rank, 4.0);
        let mut ewc = EwcState::from_adapter(&adapter);
        let grad_a = random_tensor(in_features * rank);
        let grad_b = random_tensor(rank * out_features);

        let id = BenchmarkId::new(format!("dim_{}", in_features), in_features);

        group.throughput(Throughput::Elements(adapter.param_count() as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                ewc.update_fisher(black_box(&grad_a), black_box(&grad_b), 0.9);
            })
        });
    }

    group.finish();
}

fn bench_lora_vs_dense(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_vs_dense_overhead");
    group.sample_size(50);

    // Compare LoRA overhead vs dense matmul
    let dim = 4096;
    let rank = 2;

    let adapter = LoraAdapter::new(dim, dim, rank, 4.0);
    let input = random_tensor(dim);

    // LoRA forward
    group.bench_function(BenchmarkId::new("lora_rank2", dim), |b| {
        b.iter(|| adapter.forward(black_box(&input)))
    });

    // Equivalent dense GEMV (what LoRA replaces)
    let dense_weight = random_tensor(dim * dim);

    group.bench_function(BenchmarkId::new("dense_equivalent", dim), |b| {
        b.iter(|| {
            let mut dense_output = vec![0.0f32; dim];
            for i in 0..dim {
                let mut sum = 0.0f32;
                for j in 0..dim {
                    sum += input[j] * dense_weight[j * dim + i];
                }
                dense_output[i] = sum;
            }
            black_box(dense_output)
        })
    });

    group.finish();
}

fn bench_multiple_adapters(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_adapters");
    group.sample_size(50);

    // Test applying multiple LoRA adapters (Q, K, V, O projections)
    let dim = 4096;
    let rank = 2;

    let adapters: Vec<LoraAdapter> = (0..4)
        .map(|_| LoraAdapter::new(dim, dim, rank, 4.0))
        .collect();
    let input = random_tensor(dim);

    group.bench_function(BenchmarkId::new("4_adapters_sequential", 4), |b| {
        b.iter(|| {
            let mut outputs: Vec<Vec<f32>> = Vec::with_capacity(4);
            for adapter in &adapters {
                outputs.push(adapter.forward(black_box(&input)));
            }
            outputs
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lora_forward,
    bench_lora_forward_simd,
    bench_lora_forward_batch,
    bench_lora_gradient_accumulation,
    bench_lora_apply_gradients,
    bench_lora_ewc_update,
    bench_lora_adaptation_cycle,
    bench_lora_memory_footprint,
    bench_ewc_fisher_update,
    bench_lora_vs_dense,
    bench_multiple_adapters,
);

criterion_main!(benches);
