//! Benchmarks for full graph energy computation
//!
//! ADR-014 Performance Target: < 10ms for 10K nodes
//!
//! Global coherence energy: E(S) = sum(w_e * |r_e|^2)
//! This is the aggregate measure of system incoherence.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

// ============================================================================
// Graph Types (Simulated for benchmarking)
// ============================================================================

/// Simplified restriction map for energy benchmarks
#[derive(Clone)]
pub struct RestrictionMap {
    pub matrix: Vec<f32>,
    pub bias: Vec<f32>,
    pub input_dim: usize,
    pub output_dim: usize,
}

impl RestrictionMap {
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

    #[inline]
    pub fn apply_into(&self, input: &[f32], output: &mut [f32]) {
        output.copy_from_slice(&self.bias);
        for i in 0..self.output_dim {
            let row_start = i * self.input_dim;
            for j in 0..self.input_dim {
                output[i] += self.matrix[row_start + j] * input[j];
            }
        }
    }
}

/// Node in sheaf graph
#[derive(Clone)]
pub struct SheafNode {
    pub id: u64,
    pub state: Vec<f32>,
}

/// Edge with restriction maps
#[derive(Clone)]
pub struct SheafEdge {
    pub source: u64,
    pub target: u64,
    pub weight: f32,
    pub rho_source: RestrictionMap,
    pub rho_target: RestrictionMap,
}

impl SheafEdge {
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

/// Full sheaf graph for coherence computation
pub struct SheafGraph {
    pub nodes: HashMap<u64, SheafNode>,
    pub edges: Vec<SheafEdge>,
    pub state_dim: usize,
}

/// Result of energy computation
pub struct CoherenceEnergy {
    pub total_energy: f32,
    pub edge_energies: Vec<f32>,
}

impl SheafGraph {
    /// Generate a random graph for benchmarking
    pub fn random(num_nodes: usize, avg_degree: usize, state_dim: usize, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = || {
            let mut h = DefaultHasher::new();
            seed.hash(&mut h);
            h
        };

        // Generate nodes
        let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
            .map(|id| {
                let state: Vec<f32> = (0..state_dim)
                    .map(|i| {
                        let mut h = hasher();
                        (id, i).hash(&mut h);
                        (h.finish() % 1000) as f32 / 1000.0 - 0.5
                    })
                    .collect();
                (id, SheafNode { id, state })
            })
            .collect();

        // Generate edges (random graph with target average degree)
        let num_edges = (num_nodes * avg_degree) / 2;
        let mut edges = Vec::with_capacity(num_edges);

        for i in 0..num_edges {
            let mut h = hasher();
            (seed, i, "edge").hash(&mut h);
            let source = (h.finish() % num_nodes as u64) as u64;

            let mut h = hasher();
            (seed, i, "target").hash(&mut h);
            let target = (h.finish() % num_nodes as u64) as u64;

            if source != target {
                edges.push(SheafEdge {
                    source,
                    target,
                    weight: 1.0,
                    rho_source: RestrictionMap::identity(state_dim),
                    rho_target: RestrictionMap::identity(state_dim),
                });
            }
        }

        Self {
            nodes,
            edges,
            state_dim,
        }
    }

    /// Generate a chain graph (linear topology)
    pub fn chain(num_nodes: usize, state_dim: usize, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
            .map(|id| {
                let state: Vec<f32> = (0..state_dim)
                    .map(|i| {
                        let mut h = DefaultHasher::new();
                        (seed, id, i).hash(&mut h);
                        (h.finish() % 1000) as f32 / 1000.0 - 0.5
                    })
                    .collect();
                (id, SheafNode { id, state })
            })
            .collect();

        let edges: Vec<SheafEdge> = (0..num_nodes - 1)
            .map(|i| SheafEdge {
                source: i as u64,
                target: (i + 1) as u64,
                weight: 1.0,
                rho_source: RestrictionMap::identity(state_dim),
                rho_target: RestrictionMap::identity(state_dim),
            })
            .collect();

        Self {
            nodes,
            edges,
            state_dim,
        }
    }

    /// Generate a dense graph (high connectivity)
    pub fn dense(num_nodes: usize, state_dim: usize, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
            .map(|id| {
                let state: Vec<f32> = (0..state_dim)
                    .map(|i| {
                        let mut h = DefaultHasher::new();
                        (seed, id, i).hash(&mut h);
                        (h.finish() % 1000) as f32 / 1000.0 - 0.5
                    })
                    .collect();
                (id, SheafNode { id, state })
            })
            .collect();

        // Dense: ~30% of possible edges
        let mut edges = Vec::new();
        for i in 0..num_nodes as u64 {
            for j in (i + 1)..num_nodes as u64 {
                let mut h = DefaultHasher::new();
                (seed, i, j).hash(&mut h);
                if h.finish() % 10 < 3 {
                    // 30% probability
                    edges.push(SheafEdge {
                        source: i,
                        target: j,
                        weight: 1.0,
                        rho_source: RestrictionMap::identity(state_dim),
                        rho_target: RestrictionMap::identity(state_dim),
                    });
                }
            }
        }

        Self {
            nodes,
            edges,
            state_dim,
        }
    }

    /// Compute global coherence energy (sequential)
    pub fn compute_energy_sequential(&self) -> CoherenceEnergy {
        let mut source_buf = vec![0.0f32; self.state_dim];
        let mut target_buf = vec![0.0f32; self.state_dim];

        let edge_energies: Vec<f32> = self
            .edges
            .iter()
            .map(|edge| {
                let source_state = &self.nodes[&edge.source].state;
                let target_state = &self.nodes[&edge.target].state;
                edge.weighted_residual_energy_into(
                    source_state,
                    target_state,
                    &mut source_buf,
                    &mut target_buf,
                )
            })
            .collect();

        let total_energy: f32 = edge_energies.iter().sum();

        CoherenceEnergy {
            total_energy,
            edge_energies,
        }
    }

    /// Compute global coherence energy (parallel with rayon)
    #[cfg(feature = "parallel")]
    pub fn compute_energy_parallel(&self) -> CoherenceEnergy {
        use rayon::prelude::*;

        let edge_energies: Vec<f32> = self
            .edges
            .par_iter()
            .map(|edge| {
                let mut source_buf = vec![0.0f32; self.state_dim];
                let mut target_buf = vec![0.0f32; self.state_dim];
                let source_state = &self.nodes[&edge.source].state;
                let target_state = &self.nodes[&edge.target].state;
                edge.weighted_residual_energy_into(
                    source_state,
                    target_state,
                    &mut source_buf,
                    &mut target_buf,
                )
            })
            .collect();

        let total_energy: f32 = edge_energies.par_iter().sum();

        CoherenceEnergy {
            total_energy,
            edge_energies,
        }
    }

    /// Compute just total energy (no per-edge tracking)
    pub fn compute_total_energy(&self) -> f32 {
        let mut source_buf = vec![0.0f32; self.state_dim];
        let mut target_buf = vec![0.0f32; self.state_dim];
        let mut total = 0.0f32;

        for edge in &self.edges {
            let source_state = &self.nodes[&edge.source].state;
            let target_state = &self.nodes[&edge.target].state;
            total += edge.weighted_residual_energy_into(
                source_state,
                target_state,
                &mut source_buf,
                &mut target_buf,
            );
        }

        total
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark full graph energy at various sizes
fn bench_full_graph_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_full_graph");

    // ADR-014 target: 10K nodes in <10ms
    // Test progression: 100, 1K, 10K, 100K
    for num_nodes in [100, 1_000, 10_000] {
        let avg_degree = 4;
        let state_dim = 64;
        let graph = SheafGraph::random(num_nodes, avg_degree, state_dim, 42);

        group.throughput(Throughput::Elements(graph.edges.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("sequential", format!("{}nodes", num_nodes)),
            &num_nodes,
            |b, _| b.iter(|| black_box(graph.compute_energy_sequential())),
        );

        // Total energy only (no per-edge allocation)
        group.bench_with_input(
            BenchmarkId::new("total_only", format!("{}nodes", num_nodes)),
            &num_nodes,
            |b, _| b.iter(|| black_box(graph.compute_total_energy())),
        );
    }

    group.finish();
}

/// Benchmark with 100K nodes (reduced sample size due to runtime)
fn bench_large_graph_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_large_graph");
    group.sample_size(10);

    let num_nodes = 100_000;
    let avg_degree = 4;
    let state_dim = 64;
    let graph = SheafGraph::random(num_nodes, avg_degree, state_dim, 42);

    group.throughput(Throughput::Elements(graph.edges.len() as u64));

    group.bench_function("100K_nodes_total_energy", |b| {
        b.iter(|| black_box(graph.compute_total_energy()))
    });

    group.finish();
}

/// Benchmark energy computation for different graph topologies
fn bench_topology_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_topology");

    let num_nodes = 1000;
    let state_dim = 64;

    // Chain topology (sparse, n-1 edges)
    let chain = SheafGraph::chain(num_nodes, state_dim, 42);
    group.throughput(Throughput::Elements(chain.edges.len() as u64));
    group.bench_function("chain_1000", |b| {
        b.iter(|| black_box(chain.compute_total_energy()))
    });

    // Random topology (avg degree 4)
    let random = SheafGraph::random(num_nodes, 4, state_dim, 42);
    group.throughput(Throughput::Elements(random.edges.len() as u64));
    group.bench_function("random_1000_deg4", |b| {
        b.iter(|| black_box(random.compute_total_energy()))
    });

    // Dense topology (~30% edges)
    let dense = SheafGraph::dense(100, state_dim, 42); // Smaller for dense
    group.throughput(Throughput::Elements(dense.edges.len() as u64));
    group.bench_function("dense_100", |b| {
        b.iter(|| black_box(dense.compute_total_energy()))
    });

    group.finish();
}

/// Benchmark impact of state dimension on energy computation
fn bench_state_dimension(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_state_dim");

    let num_nodes = 1000;
    let avg_degree = 4;

    for state_dim in [8, 32, 64, 128, 256] {
        let graph = SheafGraph::random(num_nodes, avg_degree, state_dim, 42);

        group.throughput(Throughput::Elements(graph.edges.len() as u64));
        group.bench_with_input(BenchmarkId::new("dim", state_dim), &state_dim, |b, _| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    group.finish();
}

/// Benchmark edge density scaling
fn bench_edge_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_edge_density");

    let num_nodes = 1000;
    let state_dim = 64;

    // Varying average degree
    for avg_degree in [2, 4, 8, 16, 32] {
        let graph = SheafGraph::random(num_nodes, avg_degree, state_dim, 42);

        group.throughput(Throughput::Elements(graph.edges.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("avg_degree", avg_degree),
            &avg_degree,
            |b, _| b.iter(|| black_box(graph.compute_total_energy())),
        );
    }

    group.finish();
}

/// Benchmark scope-based energy aggregation
fn bench_scoped_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_scoped");

    let num_nodes = 10_000;
    let avg_degree = 4;
    let state_dim = 64;
    let graph = SheafGraph::random(num_nodes, avg_degree, state_dim, 42);

    // Simulate scope-based aggregation (e.g., by namespace)
    let num_scopes = 10;
    let scope_assignments: Vec<usize> = graph
        .edges
        .iter()
        .enumerate()
        .map(|(i, _)| i % num_scopes)
        .collect();

    group.bench_function("aggregate_by_scope", |b| {
        b.iter(|| {
            let mut source_buf = vec![0.0f32; state_dim];
            let mut target_buf = vec![0.0f32; state_dim];
            let mut scope_energies = vec![0.0f32; num_scopes];

            for (i, edge) in graph.edges.iter().enumerate() {
                let source_state = &graph.nodes[&edge.source].state;
                let target_state = &graph.nodes[&edge.target].state;
                let energy = edge.weighted_residual_energy_into(
                    source_state,
                    target_state,
                    &mut source_buf,
                    &mut target_buf,
                );
                scope_energies[scope_assignments[i]] += energy;
            }

            black_box(scope_energies)
        })
    });

    group.finish();
}

/// Benchmark energy fingerprint computation
fn bench_energy_fingerprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_fingerprint");

    let num_nodes = 1000;
    let avg_degree = 4;
    let state_dim = 64;
    let graph = SheafGraph::random(num_nodes, avg_degree, state_dim, 42);

    group.bench_function("compute_with_fingerprint", |b| {
        b.iter(|| {
            let energy = graph.compute_energy_sequential();

            // Compute fingerprint from edge energies
            let mut fingerprint = 0u64;
            for e in &energy.edge_energies {
                fingerprint ^= e.to_bits() as u64;
                fingerprint = fingerprint.rotate_left(7);
            }

            black_box((energy.total_energy, fingerprint))
        })
    });

    group.finish();
}

/// Benchmark memory access patterns for energy computation
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_memory");

    let num_nodes = 10_000;
    let state_dim = 64;

    // Sequential node access (chain)
    let chain = SheafGraph::chain(num_nodes, state_dim, 42);
    group.bench_function("sequential_access", |b| {
        b.iter(|| black_box(chain.compute_total_energy()))
    });

    // Random node access
    let random = SheafGraph::random(num_nodes, 4, state_dim, 42);
    group.bench_function("random_access", |b| {
        b.iter(|| black_box(random.compute_total_energy()))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_full_graph_energy,
    bench_large_graph_energy,
    bench_topology_impact,
    bench_state_dimension,
    bench_edge_density,
    bench_scoped_energy,
    bench_energy_fingerprint,
    bench_memory_patterns,
);

criterion_main!(benches);
