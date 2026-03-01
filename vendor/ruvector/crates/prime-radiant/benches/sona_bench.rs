//! Benchmarks for SONA Micro-LoRA instant adaptation
//!
//! ADR-014 Performance Target: < 0.05ms (50us) for instant adaptation
//!
//! SONA provides self-optimizing threshold tuning with:
//! - Micro-LoRA: Ultra-low rank (1-2) for instant learning
//! - Base-LoRA: Standard LoRA for background learning
//! - EWC++: Elastic Weight Consolidation to prevent forgetting

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// SONA Types (Simulated for benchmarking)
// ============================================================================

/// Micro-LoRA layer (rank 1-2 for instant adaptation)
pub struct MicroLoRA {
    /// Low-rank factor A (dim x rank)
    pub a: Vec<f32>,
    /// Low-rank factor B (rank x dim)
    pub b: Vec<f32>,
    /// Scaling factor
    pub scale: f32,
    /// Input dimension
    pub dim: usize,
    /// Rank (typically 1-2)
    pub rank: usize,
}

impl MicroLoRA {
    pub fn new(dim: usize, rank: usize) -> Self {
        // Initialize with small random values
        let a: Vec<f32> = (0..dim * rank)
            .map(|i| ((i as f32 * 0.1234).sin() * 0.01))
            .collect();
        let b: Vec<f32> = (0..rank * dim)
            .map(|i| ((i as f32 * 0.5678).cos() * 0.01))
            .collect();

        Self {
            a,
            b,
            scale: 0.1,
            dim,
            rank,
        }
    }

    /// Apply micro-LoRA transform: y = x + scale * B @ A @ x
    #[inline]
    pub fn apply(&self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), self.dim);
        debug_assert_eq!(output.len(), self.dim);

        // Copy input to output first (identity component)
        output.copy_from_slice(input);

        // Compute A @ x -> hidden (rank-dimensional)
        let mut hidden = vec![0.0f32; self.rank];
        for r in 0..self.rank {
            for i in 0..self.dim {
                hidden[r] += self.a[i * self.rank + r] * input[i];
            }
        }

        // Compute B @ hidden and add to output
        for i in 0..self.dim {
            let mut delta = 0.0f32;
            for r in 0..self.rank {
                delta += self.b[r * self.dim + i] * hidden[r];
            }
            output[i] += self.scale * delta;
        }
    }

    /// Apply with pre-allocated hidden buffer (zero allocation)
    #[inline]
    pub fn apply_zero_alloc(&self, input: &[f32], hidden: &mut [f32], output: &mut [f32]) {
        debug_assert_eq!(hidden.len(), self.rank);

        // Copy input
        output.copy_from_slice(input);

        // A @ x
        hidden.fill(0.0);
        for r in 0..self.rank {
            for i in 0..self.dim {
                hidden[r] += self.a[i * self.rank + r] * input[i];
            }
        }

        // B @ hidden
        for i in 0..self.dim {
            let mut delta = 0.0f32;
            for r in 0..self.rank {
                delta += self.b[r * self.dim + i] * hidden[r];
            }
            output[i] += self.scale * delta;
        }
    }

    /// Update weights from gradient (instant learning)
    #[inline]
    pub fn update(&mut self, grad_a: &[f32], grad_b: &[f32], learning_rate: f32) {
        for i in 0..self.a.len() {
            self.a[i] -= learning_rate * grad_a[i];
        }
        for i in 0..self.b.len() {
            self.b[i] -= learning_rate * grad_b[i];
        }
    }
}

/// Base-LoRA layer (higher rank for background learning)
pub struct BaseLoRA {
    pub a: Vec<f32>,
    pub b: Vec<f32>,
    pub scale: f32,
    pub dim: usize,
    pub rank: usize,
}

impl BaseLoRA {
    pub fn new(dim: usize, rank: usize) -> Self {
        let a: Vec<f32> = (0..dim * rank)
            .map(|i| ((i as f32 * 0.3456).sin() * 0.01))
            .collect();
        let b: Vec<f32> = (0..rank * dim)
            .map(|i| ((i as f32 * 0.7890).cos() * 0.01))
            .collect();

        Self {
            a,
            b,
            scale: 0.05,
            dim,
            rank,
        }
    }

    #[inline]
    pub fn apply(&self, input: &[f32], output: &mut [f32]) {
        output.copy_from_slice(input);

        let mut hidden = vec![0.0f32; self.rank];
        for r in 0..self.rank {
            for i in 0..self.dim {
                hidden[r] += self.a[i * self.rank + r] * input[i];
            }
        }

        for i in 0..self.dim {
            let mut delta = 0.0f32;
            for r in 0..self.rank {
                delta += self.b[r * self.dim + i] * hidden[r];
            }
            output[i] += self.scale * delta;
        }
    }
}

/// EWC++ weight importance
pub struct EwcPlusPlus {
    /// Fisher information diagonal
    pub fisher: Vec<f32>,
    /// Optimal weights from previous tasks
    pub optimal_weights: Vec<f32>,
    /// Regularization strength
    pub lambda: f32,
}

impl EwcPlusPlus {
    pub fn new(param_count: usize, lambda: f32) -> Self {
        Self {
            fisher: vec![1.0; param_count],
            optimal_weights: vec![0.0; param_count],
            lambda,
        }
    }

    /// Compute EWC penalty for given weights
    #[inline]
    pub fn penalty(&self, weights: &[f32]) -> f32 {
        let mut penalty = 0.0f32;
        for i in 0..weights.len().min(self.fisher.len()) {
            let diff = weights[i] - self.optimal_weights[i];
            penalty += self.fisher[i] * diff * diff;
        }
        self.lambda * 0.5 * penalty
    }

    /// Update Fisher information (consolidation)
    pub fn consolidate(&mut self, weights: &[f32], new_fisher: &[f32]) {
        for i in 0..self.fisher.len().min(new_fisher.len()) {
            // Online Fisher update (running average)
            self.fisher[i] = 0.9 * self.fisher[i] + 0.1 * new_fisher[i];
            self.optimal_weights[i] = weights[i];
        }
    }
}

/// Trajectory step for learning
#[derive(Clone)]
pub struct TrajectoryStep {
    pub state: Vec<f32>,
    pub action_embedding: Vec<f32>,
    pub reward: f32,
}

/// Trajectory builder
pub struct TrajectoryBuilder {
    pub initial_state: Vec<f32>,
    pub steps: Vec<TrajectoryStep>,
}

impl TrajectoryBuilder {
    pub fn new(initial_state: Vec<f32>) -> Self {
        Self {
            initial_state,
            steps: Vec::new(),
        }
    }

    pub fn add_step(&mut self, state: Vec<f32>, action: Vec<f32>, reward: f32) {
        self.steps.push(TrajectoryStep {
            state,
            action_embedding: action,
            reward,
        });
    }
}

/// SONA engine (simplified for benchmarking)
pub struct SonaEngine {
    pub micro_lora: MicroLoRA,
    pub base_lora: BaseLoRA,
    pub ewc: EwcPlusPlus,
    pub dim: usize,
}

impl SonaEngine {
    pub fn new(dim: usize) -> Self {
        let micro_rank = 2;
        let base_rank = 8;
        let param_count = dim * micro_rank * 2 + dim * base_rank * 2;

        Self {
            micro_lora: MicroLoRA::new(dim, micro_rank),
            base_lora: BaseLoRA::new(dim, base_rank),
            ewc: EwcPlusPlus::new(param_count, 0.4),
            dim,
        }
    }

    /// Begin trajectory
    pub fn begin_trajectory(&self, initial_state: Vec<f32>) -> TrajectoryBuilder {
        TrajectoryBuilder::new(initial_state)
    }

    /// End trajectory and trigger learning
    pub fn end_trajectory(&mut self, builder: TrajectoryBuilder, final_reward: f32) {
        // Simplified learning: update micro-LoRA based on reward
        let lr = 0.001 * final_reward.max(0.0);

        // Pseudo-gradient (simplified)
        let grad_a: Vec<f32> = self.micro_lora.a.iter().map(|w| w * lr).collect();
        let grad_b: Vec<f32> = self.micro_lora.b.iter().map(|w| w * lr).collect();

        self.micro_lora.update(&grad_a, &grad_b, lr);
    }

    /// Apply micro-LoRA (instant)
    #[inline]
    pub fn apply_micro(&self, input: &[f32], output: &mut [f32]) {
        self.micro_lora.apply(input, output);
    }

    /// Apply base-LoRA (background)
    pub fn apply_base(&self, input: &[f32], output: &mut [f32]) {
        self.base_lora.apply(input, output);
    }

    /// Apply both LoRAs combined
    pub fn apply_combined(&self, input: &[f32], output: &mut [f32]) {
        // Apply micro first
        let mut intermediate = vec![0.0f32; self.dim];
        self.micro_lora.apply(input, &mut intermediate);
        // Then base
        self.base_lora.apply(&intermediate, output);
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn generate_state(dim: usize, seed: u64) -> Vec<f32> {
    (0..dim)
        .map(|i| ((seed as f32 * 0.123 + i as f32 * 0.456).sin()))
        .collect()
}

/// Benchmark Micro-LoRA application (target: <50us)
fn bench_micro_lora_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_micro_lora_apply");
    group.throughput(Throughput::Elements(1));

    for dim in [64, 128, 256, 512] {
        let lora = MicroLoRA::new(dim, 2); // Rank 2
        let input = generate_state(dim, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| lora.apply(black_box(&input), black_box(&mut output)))
        });
    }

    // Different ranks
    let dim = 256;
    for rank in [1, 2, 4] {
        let lora = MicroLoRA::new(dim, rank);
        let input = generate_state(dim, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("rank", rank), &rank, |b, _| {
            b.iter(|| lora.apply(black_box(&input), black_box(&mut output)))
        });
    }

    group.finish();
}

/// Benchmark zero-allocation Micro-LoRA
fn bench_micro_lora_zero_alloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_micro_lora_zero_alloc");
    group.throughput(Throughput::Elements(1));

    for dim in [64, 128, 256, 512] {
        let lora = MicroLoRA::new(dim, 2);
        let input = generate_state(dim, 42);
        let mut hidden = vec![0.0f32; 2];
        let mut output = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| {
                lora.apply_zero_alloc(
                    black_box(&input),
                    black_box(&mut hidden),
                    black_box(&mut output),
                )
            })
        });
    }

    group.finish();
}

/// Benchmark Base-LoRA application
fn bench_base_lora_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_base_lora_apply");
    group.throughput(Throughput::Elements(1));

    for dim in [64, 128, 256, 512] {
        let lora = BaseLoRA::new(dim, 8); // Rank 8
        let input = generate_state(dim, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| lora.apply(black_box(&input), black_box(&mut output)))
        });
    }

    // Different ranks
    let dim = 256;
    for rank in [4, 8, 16, 32] {
        let lora = BaseLoRA::new(dim, rank);
        let input = generate_state(dim, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("rank", rank), &rank, |b, _| {
            b.iter(|| lora.apply(black_box(&input), black_box(&mut output)))
        });
    }

    group.finish();
}

/// Benchmark EWC++ penalty computation
fn bench_ewc_penalty(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_ewc_penalty");
    group.throughput(Throughput::Elements(1));

    for param_count in [1000, 10000, 100000] {
        let ewc = EwcPlusPlus::new(param_count, 0.4);
        let weights: Vec<f32> = (0..param_count).map(|i| (i as f32 * 0.001).sin()).collect();

        group.bench_with_input(
            BenchmarkId::new("params", param_count),
            &param_count,
            |b, _| b.iter(|| black_box(ewc.penalty(black_box(&weights)))),
        );
    }

    group.finish();
}

/// Benchmark EWC++ consolidation
fn bench_ewc_consolidate(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_ewc_consolidate");

    for param_count in [1000, 10000, 100000] {
        let mut ewc = EwcPlusPlus::new(param_count, 0.4);
        let weights: Vec<f32> = (0..param_count).map(|i| (i as f32 * 0.001).sin()).collect();
        let new_fisher: Vec<f32> = (0..param_count)
            .map(|i| (i as f32 * 0.002).cos().abs())
            .collect();

        group.bench_with_input(
            BenchmarkId::new("params", param_count),
            &param_count,
            |b, _| b.iter(|| ewc.consolidate(black_box(&weights), black_box(&new_fisher))),
        );
    }

    group.finish();
}

/// Benchmark full trajectory learning cycle
fn bench_trajectory_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_trajectory_learning");

    let dim = 256;
    let mut engine = SonaEngine::new(dim);

    // Single step trajectory
    group.bench_function("single_step_trajectory", |b| {
        b.iter(|| {
            let mut builder = engine.begin_trajectory(generate_state(dim, 42));
            builder.add_step(generate_state(dim, 43), vec![], 0.8);
            engine.end_trajectory(builder, black_box(0.85));
        })
    });

    // Multi-step trajectory
    group.bench_function("10_step_trajectory", |b| {
        b.iter(|| {
            let mut builder = engine.begin_trajectory(generate_state(dim, 42));
            for i in 0..10 {
                builder.add_step(generate_state(dim, 43 + i), vec![], 0.5 + (i as f32) * 0.05);
            }
            engine.end_trajectory(builder, black_box(0.9));
        })
    });

    group.finish();
}

/// Benchmark combined LoRA application
fn bench_combined_lora(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_combined_lora");

    for dim in [64, 128, 256, 512] {
        let engine = SonaEngine::new(dim);
        let input = generate_state(dim, 42);
        let mut output = vec![0.0f32; dim];

        // Micro only
        group.bench_with_input(BenchmarkId::new("micro_only", dim), &dim, |b, _| {
            b.iter(|| engine.apply_micro(black_box(&input), black_box(&mut output)))
        });

        // Base only
        group.bench_with_input(BenchmarkId::new("base_only", dim), &dim, |b, _| {
            b.iter(|| engine.apply_base(black_box(&input), black_box(&mut output)))
        });

        // Combined
        group.bench_with_input(BenchmarkId::new("combined", dim), &dim, |b, _| {
            b.iter(|| engine.apply_combined(black_box(&input), black_box(&mut output)))
        });
    }

    group.finish();
}

/// Benchmark batch inference
fn bench_batch_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_batch_inference");

    let dim = 256;
    let engine = SonaEngine::new(dim);

    for batch_size in [1, 10, 100, 1000] {
        let inputs: Vec<Vec<f32>> = (0..batch_size)
            .map(|i| generate_state(dim, i as u64))
            .collect();
        let mut outputs: Vec<Vec<f32>> = (0..batch_size).map(|_| vec![0.0f32; dim]).collect();

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
                        engine.apply_micro(input, output);
                    }
                    black_box(outputs.len())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark weight update (instant learning)
fn bench_weight_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona_weight_update");

    for dim in [64, 128, 256, 512] {
        let mut lora = MicroLoRA::new(dim, 2);
        let grad_a: Vec<f32> = (0..dim * 2).map(|i| (i as f32 * 0.001).sin()).collect();
        let grad_b: Vec<f32> = (0..2 * dim).map(|i| (i as f32 * 0.002).cos()).collect();

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| {
                lora.update(black_box(&grad_a), black_box(&grad_b), black_box(0.001));
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_micro_lora_apply,
    bench_micro_lora_zero_alloc,
    bench_base_lora_apply,
    bench_ewc_penalty,
    bench_ewc_consolidate,
    bench_trajectory_learning,
    bench_combined_lora,
    bench_batch_inference,
    bench_weight_update,
);

criterion_main!(benches);
