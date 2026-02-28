//! Benchmarks for single residual calculation
//!
//! ADR-014 Performance Target: < 1us per residual calculation
//!
//! Residual is the core primitive: r_e = rho_u(x_u) - rho_v(x_v)
//! This measures the local constraint violation at each edge.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// Restriction Map Types (Simulated for benchmarking)
// ============================================================================

/// Linear restriction map: y = Ax + b
/// Maps node state to shared constraint space
#[derive(Clone)]
pub struct RestrictionMap {
    /// Linear transformation matrix (row-major, output_dim x input_dim)
    pub matrix: Vec<f32>,
    /// Bias vector
    pub bias: Vec<f32>,
    /// Input dimension
    pub input_dim: usize,
    /// Output dimension
    pub output_dim: usize,
}

impl RestrictionMap {
    /// Create identity restriction map
    pub fn identity(dim: usize) -> Self {
        let mut matrix = vec![0.0f32; dim * dim];
        for i in 0..dim {
            matrix[i * dim + i] = 1.0;
        }
        Self {
            matrix,
            bias: vec![0.0; dim],
            input_dim: dim,
            output_dim: dim,
        }
    }

    /// Create random restriction map for testing
    pub fn random(input_dim: usize, output_dim: usize, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut matrix = Vec::with_capacity(output_dim * input_dim);
        let mut bias = Vec::with_capacity(output_dim);

        for i in 0..(output_dim * input_dim) {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            let val = (hasher.finish() % 1000) as f32 / 1000.0 - 0.5;
            matrix.push(val);
        }

        for i in 0..output_dim {
            let mut hasher = DefaultHasher::new();
            (seed, i, "bias").hash(&mut hasher);
            let val = (hasher.finish() % 1000) as f32 / 1000.0 - 0.5;
            bias.push(val);
        }

        Self {
            matrix,
            bias,
            input_dim,
            output_dim,
        }
    }

    /// Apply restriction map: y = Ax + b
    #[inline]
    pub fn apply(&self, input: &[f32]) -> Vec<f32> {
        debug_assert_eq!(input.len(), self.input_dim);
        let mut output = self.bias.clone();

        for i in 0..self.output_dim {
            let row_start = i * self.input_dim;
            for j in 0..self.input_dim {
                output[i] += self.matrix[row_start + j] * input[j];
            }
        }

        output
    }

    /// Apply restriction map with SIMD-friendly layout (output buffer provided)
    #[inline]
    pub fn apply_into(&self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), self.input_dim);
        debug_assert_eq!(output.len(), self.output_dim);

        // Copy bias first
        output.copy_from_slice(&self.bias);

        // Matrix-vector multiply
        for i in 0..self.output_dim {
            let row_start = i * self.input_dim;
            for j in 0..self.input_dim {
                output[i] += self.matrix[row_start + j] * input[j];
            }
        }
    }
}

/// Edge with restriction maps
pub struct SheafEdge {
    pub source: u64,
    pub target: u64,
    pub weight: f32,
    pub rho_source: RestrictionMap,
    pub rho_target: RestrictionMap,
}

impl SheafEdge {
    /// Calculate the edge residual (local mismatch)
    /// r_e = rho_u(x_u) - rho_v(x_v)
    #[inline]
    pub fn residual(&self, source_state: &[f32], target_state: &[f32]) -> Vec<f32> {
        let projected_source = self.rho_source.apply(source_state);
        let projected_target = self.rho_target.apply(target_state);

        projected_source
            .iter()
            .zip(projected_target.iter())
            .map(|(a, b)| a - b)
            .collect()
    }

    /// Calculate residual with pre-allocated buffers (zero allocation)
    #[inline]
    pub fn residual_into(
        &self,
        source_state: &[f32],
        target_state: &[f32],
        source_buf: &mut [f32],
        target_buf: &mut [f32],
        residual: &mut [f32],
    ) {
        self.rho_source.apply_into(source_state, source_buf);
        self.rho_target.apply_into(target_state, target_buf);

        for i in 0..residual.len() {
            residual[i] = source_buf[i] - target_buf[i];
        }
    }

    /// Calculate weighted residual norm squared: w_e * |r_e|^2
    #[inline]
    pub fn weighted_residual_energy(&self, source: &[f32], target: &[f32]) -> f32 {
        let r = self.residual(source, target);
        let norm_sq: f32 = r.iter().map(|x| x * x).sum();
        self.weight * norm_sq
    }

    /// Weighted residual energy with pre-allocated buffers
    #[inline]
    pub fn weighted_residual_energy_into(
        &self,
        source: &[f32],
        target: &[f32],
        source_buf: &mut [f32],
        target_buf: &mut [f32],
    ) -> f32 {
        self.rho_source.apply_into(source, source_buf);
        self.rho_target.apply_into(target, target_buf);

        let mut norm_sq = 0.0f32;
        for i in 0..source_buf.len() {
            let diff = source_buf[i] - target_buf[i];
            norm_sq += diff * diff;
        }

        self.weight * norm_sq
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn generate_state(dim: usize, seed: u64) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    (0..dim)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
        })
        .collect()
}

/// Benchmark single residual calculation at various dimensions
fn bench_single_residual(c: &mut Criterion) {
    let mut group = c.benchmark_group("residual_single");
    group.throughput(Throughput::Elements(1));

    // Test dimensions relevant for coherence engine:
    // 8: Minimal state
    // 32: Compact embedding
    // 64: Standard embedding
    // 128: Rich state
    // 256: Large state
    for dim in [8, 32, 64, 128, 256] {
        let rho_source = RestrictionMap::identity(dim);
        let rho_target = RestrictionMap::identity(dim);
        let source_state = generate_state(dim, 42);
        let target_state = generate_state(dim, 123);

        let edge = SheafEdge {
            source: 0,
            target: 1,
            weight: 1.0,
            rho_source,
            rho_target,
        };

        group.bench_with_input(BenchmarkId::new("identity_map", dim), &dim, |b, _| {
            b.iter(|| edge.residual(black_box(&source_state), black_box(&target_state)))
        });
    }

    // Test with projection (non-identity maps)
    for (input_dim, output_dim) in [(64, 32), (128, 64), (256, 128)] {
        let rho_source = RestrictionMap::random(input_dim, output_dim, 42);
        let rho_target = RestrictionMap::random(input_dim, output_dim, 123);
        let source_state = generate_state(input_dim, 42);
        let target_state = generate_state(input_dim, 123);

        let edge = SheafEdge {
            source: 0,
            target: 1,
            weight: 1.0,
            rho_source,
            rho_target,
        };

        group.bench_with_input(
            BenchmarkId::new("projection_map", format!("{}to{}", input_dim, output_dim)),
            &(input_dim, output_dim),
            |b, _| b.iter(|| edge.residual(black_box(&source_state), black_box(&target_state))),
        );
    }

    group.finish();
}

/// Benchmark residual calculation with pre-allocated buffers (zero allocation)
fn bench_residual_zero_alloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("residual_zero_alloc");
    group.throughput(Throughput::Elements(1));

    for dim in [32, 64, 128, 256] {
        let rho_source = RestrictionMap::identity(dim);
        let rho_target = RestrictionMap::identity(dim);
        let source_state = generate_state(dim, 42);
        let target_state = generate_state(dim, 123);

        let edge = SheafEdge {
            source: 0,
            target: 1,
            weight: 1.0,
            rho_source,
            rho_target,
        };

        // Pre-allocate buffers
        let mut source_buf = vec![0.0f32; dim];
        let mut target_buf = vec![0.0f32; dim];
        let mut residual = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| {
                edge.residual_into(
                    black_box(&source_state),
                    black_box(&target_state),
                    black_box(&mut source_buf),
                    black_box(&mut target_buf),
                    black_box(&mut residual),
                )
            })
        });
    }

    group.finish();
}

/// Benchmark weighted residual energy computation
fn bench_weighted_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("residual_weighted_energy");
    group.throughput(Throughput::Elements(1));

    for dim in [32, 64, 128, 256] {
        let rho_source = RestrictionMap::identity(dim);
        let rho_target = RestrictionMap::identity(dim);
        let source_state = generate_state(dim, 42);
        let target_state = generate_state(dim, 123);

        let edge = SheafEdge {
            source: 0,
            target: 1,
            weight: 1.5,
            rho_source,
            rho_target,
        };

        group.bench_with_input(BenchmarkId::new("allocating", dim), &dim, |b, _| {
            b.iter(|| {
                edge.weighted_residual_energy(black_box(&source_state), black_box(&target_state))
            })
        });

        // Pre-allocate buffers for zero-alloc version
        let mut source_buf = vec![0.0f32; dim];
        let mut target_buf = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("zero_alloc", dim), &dim, |b, _| {
            b.iter(|| {
                edge.weighted_residual_energy_into(
                    black_box(&source_state),
                    black_box(&target_state),
                    black_box(&mut source_buf),
                    black_box(&mut target_buf),
                )
            })
        });
    }

    group.finish();
}

/// Benchmark batch residual computation (for parallel evaluation)
fn bench_batch_residual(c: &mut Criterion) {
    let mut group = c.benchmark_group("residual_batch");

    for batch_size in [10, 100, 1000] {
        let dim = 64;

        // Create batch of edges
        let edges: Vec<SheafEdge> = (0..batch_size)
            .map(|i| SheafEdge {
                source: i as u64,
                target: (i + 1) as u64,
                weight: 1.0,
                rho_source: RestrictionMap::identity(dim),
                rho_target: RestrictionMap::identity(dim),
            })
            .collect();

        let states: Vec<Vec<f32>> = (0..batch_size + 1)
            .map(|i| generate_state(dim, i as u64))
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));

        // Sequential computation
        group.bench_with_input(
            BenchmarkId::new("sequential", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    let mut total_energy = 0.0f32;
                    for (i, edge) in edges.iter().enumerate() {
                        total_energy += edge.weighted_residual_energy(
                            black_box(&states[i]),
                            black_box(&states[i + 1]),
                        );
                    }
                    black_box(total_energy)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark restriction map application alone
fn bench_restriction_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("restriction_map");
    group.throughput(Throughput::Elements(1));

    // Identity maps
    for dim in [32, 64, 128, 256] {
        let rho = RestrictionMap::identity(dim);
        let input = generate_state(dim, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("identity_apply", dim), &dim, |b, _| {
            b.iter(|| rho.apply(black_box(&input)))
        });

        group.bench_with_input(
            BenchmarkId::new("identity_apply_into", dim),
            &dim,
            |b, _| b.iter(|| rho.apply_into(black_box(&input), black_box(&mut output))),
        );
    }

    // Projection maps (dense matrix multiply)
    for (input_dim, output_dim) in [(64, 32), (128, 64), (256, 128), (512, 256)] {
        let rho = RestrictionMap::random(input_dim, output_dim, 42);
        let input = generate_state(input_dim, 42);
        let mut output = vec![0.0f32; output_dim];

        group.bench_with_input(
            BenchmarkId::new("projection_apply", format!("{}x{}", input_dim, output_dim)),
            &(input_dim, output_dim),
            |b, _| b.iter(|| rho.apply(black_box(&input))),
        );

        group.bench_with_input(
            BenchmarkId::new(
                "projection_apply_into",
                format!("{}x{}", input_dim, output_dim),
            ),
            &(input_dim, output_dim),
            |b, _| b.iter(|| rho.apply_into(black_box(&input), black_box(&mut output))),
        );
    }

    group.finish();
}

/// Benchmark SIMD-optimized residual patterns
fn bench_simd_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("residual_simd_patterns");
    group.throughput(Throughput::Elements(1));

    // Aligned dimensions for SIMD (multiples of 8 for AVX2, 16 for AVX-512)
    for dim in [32, 64, 128, 256, 512] {
        let a = generate_state(dim, 42);
        let b = generate_state(dim, 123);

        // Scalar subtraction and norm
        group.bench_with_input(
            BenchmarkId::new("scalar_diff_norm", dim),
            &dim,
            |b_iter, _| {
                b_iter.iter(|| {
                    let mut norm_sq = 0.0f32;
                    for i in 0..dim {
                        let diff = a[i] - b[i];
                        norm_sq += diff * diff;
                    }
                    black_box(norm_sq)
                })
            },
        );

        // Iterator-based (auto-vectorization friendly)
        group.bench_with_input(
            BenchmarkId::new("iter_diff_norm", dim),
            &dim,
            |b_iter, _| {
                b_iter.iter(|| {
                    let norm_sq: f32 = a
                        .iter()
                        .zip(b.iter())
                        .map(|(x, y)| {
                            let d = x - y;
                            d * d
                        })
                        .sum();
                    black_box(norm_sq)
                })
            },
        );

        // Chunked for explicit SIMD opportunity
        group.bench_with_input(
            BenchmarkId::new("chunked_diff_norm", dim),
            &dim,
            |b_iter, _| {
                b_iter.iter(|| {
                    let mut accum = [0.0f32; 8];
                    for (chunk_a, chunk_b) in a.chunks(8).zip(b.chunks(8)) {
                        for i in 0..chunk_a.len() {
                            let d = chunk_a[i] - chunk_b[i];
                            accum[i] += d * d;
                        }
                    }
                    black_box(accum.iter().sum::<f32>())
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_residual,
    bench_residual_zero_alloc,
    bench_weighted_energy,
    bench_batch_residual,
    bench_restriction_map,
    bench_simd_patterns,
);

criterion_main!(benches);
