//! Comprehensive Coherence Engine Benchmarks
//!
//! This benchmark suite covers the core coherence computation primitives
//! across varying dimensions, graph sizes, and topologies.
//!
//! ## Performance Targets (ADR-014)
//! - Residual computation: < 1us per edge
//! - Energy computation: < 10ms for 10K nodes
//! - Incremental update: < 100us for single node
//!
//! ## Benchmark Categories
//! 1. Coherence Core - residual, energy, incremental
//! 2. Restriction Maps - identity, diagonal, dense, sparse
//! 3. Scaling Tests - nodes, edges, dimensions

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

// ============================================================================
// BENCHMARK TYPES
// ============================================================================

/// Linear restriction map: y = Ax + b
#[derive(Clone)]
pub struct RestrictionMap {
    pub matrix: Vec<f32>,
    pub bias: Vec<f32>,
    pub input_dim: usize,
    pub output_dim: usize,
    pub map_type: MapType,
}

#[derive(Clone, Copy, Debug)]
pub enum MapType {
    Identity,
    Diagonal,
    Dense,
    Sparse { density: f32 },
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
            map_type: MapType::Identity,
        }
    }

    /// Create diagonal restriction map (scaling)
    pub fn diagonal(dim: usize, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut matrix = vec![0.0f32; dim * dim];
        for i in 0..dim {
            let mut hasher = DefaultHasher::new();
            (seed, i, "diag").hash(&mut hasher);
            let val = (hasher.finish() % 1000) as f32 / 500.0; // 0 to 2
            matrix[i * dim + i] = val;
        }
        Self {
            matrix,
            bias: vec![0.0; dim],
            input_dim: dim,
            output_dim: dim,
            map_type: MapType::Diagonal,
        }
    }

    /// Create dense random restriction map
    pub fn dense(input_dim: usize, output_dim: usize, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut matrix = Vec::with_capacity(output_dim * input_dim);
        for i in 0..(output_dim * input_dim) {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            let val = (hasher.finish() % 1000) as f32 / 1000.0 - 0.5;
            matrix.push(val);
        }

        let mut bias = Vec::with_capacity(output_dim);
        for i in 0..output_dim {
            let mut hasher = DefaultHasher::new();
            (seed, i, "bias").hash(&mut hasher);
            let val = (hasher.finish() % 100) as f32 / 1000.0;
            bias.push(val);
        }

        Self {
            matrix,
            bias,
            input_dim,
            output_dim,
            map_type: MapType::Dense,
        }
    }

    /// Create sparse restriction map with given density
    pub fn sparse(input_dim: usize, output_dim: usize, density: f32, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut matrix = vec![0.0f32; output_dim * input_dim];
        let density_threshold = (density * 1000.0) as u64;

        for i in 0..(output_dim * input_dim) {
            let mut hasher = DefaultHasher::new();
            (seed, i, "sparse").hash(&mut hasher);
            if hasher.finish() % 1000 < density_threshold {
                let mut hasher = DefaultHasher::new();
                (seed, i, "val").hash(&mut hasher);
                let val = (hasher.finish() % 1000) as f32 / 1000.0 - 0.5;
                matrix[i] = val;
            }
        }

        Self {
            matrix,
            bias: vec![0.0; output_dim],
            input_dim,
            output_dim,
            map_type: MapType::Sparse { density },
        }
    }

    /// Apply restriction map: y = Ax + b (allocating)
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

    /// Apply restriction map with pre-allocated buffer (zero allocation)
    #[inline]
    pub fn apply_into(&self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), self.input_dim);
        debug_assert_eq!(output.len(), self.output_dim);

        output.copy_from_slice(&self.bias);

        for i in 0..self.output_dim {
            let row_start = i * self.input_dim;
            for j in 0..self.input_dim {
                output[i] += self.matrix[row_start + j] * input[j];
            }
        }
    }

    /// Apply identity map (optimized fast path)
    #[inline]
    pub fn apply_identity_into(&self, input: &[f32], output: &mut [f32]) {
        debug_assert!(matches!(self.map_type, MapType::Identity));
        output.copy_from_slice(input);
    }

    /// Apply diagonal map (optimized)
    #[inline]
    pub fn apply_diagonal_into(&self, input: &[f32], output: &mut [f32]) {
        debug_assert!(matches!(self.map_type, MapType::Diagonal));
        let dim = self.input_dim;
        for i in 0..dim {
            output[i] = self.matrix[i * dim + i] * input[i] + self.bias[i];
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
    pub id: u64,
    pub source: u64,
    pub target: u64,
    pub weight: f32,
    pub rho_source: RestrictionMap,
    pub rho_target: RestrictionMap,
}

impl SheafEdge {
    /// Calculate residual with pre-allocated buffers
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

    /// Calculate weighted residual energy: w_e * |r_e|^2
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
    pub edge_dim: usize,
}

impl SheafGraph {
    /// Generate a random graph for benchmarking
    pub fn random(num_nodes: usize, avg_degree: usize, state_dim: usize, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
            .map(|id| {
                let state: Vec<f32> = (0..state_dim)
                    .map(|i| {
                        let mut hasher = DefaultHasher::new();
                        (seed, id, i).hash(&mut hasher);
                        (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
                    })
                    .collect();
                (id, SheafNode { id, state })
            })
            .collect();

        let num_edges = (num_nodes * avg_degree) / 2;
        let mut edges = Vec::with_capacity(num_edges);

        for i in 0..num_edges {
            let mut h = DefaultHasher::new();
            (seed, i, "source").hash(&mut h);
            let source = h.finish() % num_nodes as u64;

            let mut h = DefaultHasher::new();
            (seed, i, "target").hash(&mut h);
            let target = h.finish() % num_nodes as u64;

            if source != target {
                edges.push(SheafEdge {
                    id: i as u64,
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
            edge_dim: state_dim,
        }
    }

    /// Generate graph with specific restriction map type
    pub fn with_restriction_type(
        num_nodes: usize,
        avg_degree: usize,
        state_dim: usize,
        edge_dim: usize,
        map_type: MapType,
        seed: u64,
    ) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
            .map(|id| {
                let state: Vec<f32> = (0..state_dim)
                    .map(|i| {
                        let mut hasher = DefaultHasher::new();
                        (seed, id, i).hash(&mut hasher);
                        (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
                    })
                    .collect();
                (id, SheafNode { id, state })
            })
            .collect();

        let num_edges = (num_nodes * avg_degree) / 2;
        let mut edges = Vec::with_capacity(num_edges);

        for i in 0..num_edges {
            let mut h = DefaultHasher::new();
            (seed, i, "source").hash(&mut h);
            let source = h.finish() % num_nodes as u64;

            let mut h = DefaultHasher::new();
            (seed, i, "target").hash(&mut h);
            let target = h.finish() % num_nodes as u64;

            if source != target {
                let rho_source = match map_type {
                    MapType::Identity => RestrictionMap::identity(state_dim),
                    MapType::Diagonal => RestrictionMap::diagonal(state_dim, seed + i as u64),
                    MapType::Dense => RestrictionMap::dense(state_dim, edge_dim, seed + i as u64),
                    MapType::Sparse { density } => {
                        RestrictionMap::sparse(state_dim, edge_dim, density, seed + i as u64)
                    }
                };
                let rho_target = match map_type {
                    MapType::Identity => RestrictionMap::identity(state_dim),
                    MapType::Diagonal => {
                        RestrictionMap::diagonal(state_dim, seed + i as u64 + 1000)
                    }
                    MapType::Dense => {
                        RestrictionMap::dense(state_dim, edge_dim, seed + i as u64 + 1000)
                    }
                    MapType::Sparse { density } => {
                        RestrictionMap::sparse(state_dim, edge_dim, density, seed + i as u64 + 1000)
                    }
                };

                edges.push(SheafEdge {
                    id: i as u64,
                    source,
                    target,
                    weight: 1.0,
                    rho_source,
                    rho_target,
                });
            }
        }

        Self {
            nodes,
            edges,
            state_dim,
            edge_dim,
        }
    }

    /// Compute global coherence energy (sequential)
    pub fn compute_total_energy(&self) -> f32 {
        let mut source_buf = vec![0.0f32; self.edge_dim];
        let mut target_buf = vec![0.0f32; self.edge_dim];
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

    /// Compute energy with per-edge tracking
    pub fn compute_energy_with_edges(&self) -> (f32, Vec<f32>) {
        let mut source_buf = vec![0.0f32; self.edge_dim];
        let mut target_buf = vec![0.0f32; self.edge_dim];

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

        let total: f32 = edge_energies.iter().sum();
        (total, edge_energies)
    }
}

// ============================================================================
// HELPER FUNCTIONS
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

/// Compute squared norm (naive)
#[inline]
fn norm_sq_naive(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum()
}

/// Compute squared norm (unrolled)
#[inline]
fn norm_sq_unrolled(v: &[f32]) -> f32 {
    let chunks = v.chunks_exact(4);
    let remainder = chunks.remainder();

    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for chunk in chunks {
        acc0 += chunk[0] * chunk[0];
        acc1 += chunk[1] * chunk[1];
        acc2 += chunk[2] * chunk[2];
        acc3 += chunk[3] * chunk[3];
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;
    for &x in remainder {
        sum += x * x;
    }
    sum
}

// ============================================================================
// COHERENCE CORE BENCHMARKS
// ============================================================================

/// Benchmark single edge residual computation at varying dimensions
fn bench_residual_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("coherence_residual");
    group.throughput(Throughput::Elements(1));

    // ADR-014 target dimensions: 64, 256, 1024
    for dim in [64, 256, 1024] {
        let rho_source = RestrictionMap::identity(dim);
        let rho_target = RestrictionMap::identity(dim);
        let source_state = generate_state(dim, 42);
        let target_state = generate_state(dim, 123);

        let edge = SheafEdge {
            id: 0,
            source: 0,
            target: 1,
            weight: 1.0,
            rho_source,
            rho_target,
        };

        let mut source_buf = vec![0.0f32; dim];
        let mut target_buf = vec![0.0f32; dim];
        let mut residual = vec![0.0f32; dim];

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| {
                edge.residual_into(
                    black_box(&source_state),
                    black_box(&target_state),
                    &mut source_buf,
                    &mut target_buf,
                    &mut residual,
                );
                black_box(residual[0])
            })
        });
    }

    group.finish();
}

/// Benchmark full graph energy computation at varying sizes
fn bench_energy_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("coherence_energy");

    // ADR-014 targets: 100, 1K, 10K, 100K nodes
    let sizes = [(100, 100), (1_000, 50), (10_000, 20), (100_000, 10)];

    for (num_nodes, sample_size) in sizes {
        let graph = SheafGraph::random(num_nodes, 4, 64, 42);

        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(graph.edges.len() as u64));

        group.bench_with_input(BenchmarkId::new("nodes", num_nodes), &num_nodes, |b, _| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    group.finish();
}

/// Benchmark incremental single node update
fn bench_incremental_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("coherence_incremental");

    // Simulated incremental update tracking
    struct IncrementalTracker {
        graph: SheafGraph,
        node_to_edges: HashMap<u64, Vec<usize>>,
        edge_energies: Vec<f32>,
        total_energy: f32,
    }

    impl IncrementalTracker {
        fn new(graph: SheafGraph) -> Self {
            let mut node_to_edges: HashMap<u64, Vec<usize>> = HashMap::new();
            for (idx, edge) in graph.edges.iter().enumerate() {
                node_to_edges.entry(edge.source).or_default().push(idx);
                node_to_edges.entry(edge.target).or_default().push(idx);
            }

            let (total_energy, edge_energies) = graph.compute_energy_with_edges();

            Self {
                graph,
                node_to_edges,
                edge_energies,
                total_energy,
            }
        }

        fn update_node(&mut self, node_id: u64, new_state: Vec<f32>) {
            if let Some(node) = self.graph.nodes.get_mut(&node_id) {
                node.state = new_state;
            }

            let affected = self
                .node_to_edges
                .get(&node_id)
                .cloned()
                .unwrap_or_default();
            let mut source_buf = vec![0.0f32; self.graph.edge_dim];
            let mut target_buf = vec![0.0f32; self.graph.edge_dim];

            for &edge_idx in &affected {
                let edge = &self.graph.edges[edge_idx];
                let source_state = &self.graph.nodes[&edge.source].state;
                let target_state = &self.graph.nodes[&edge.target].state;

                let old_energy = self.edge_energies[edge_idx];
                let new_energy = edge.weighted_residual_energy_into(
                    source_state,
                    target_state,
                    &mut source_buf,
                    &mut target_buf,
                );

                self.total_energy += new_energy - old_energy;
                self.edge_energies[edge_idx] = new_energy;
            }
        }
    }

    // ADR-014 target: <100us for single node update
    for num_nodes in [1_000, 10_000, 100_000] {
        let graph = SheafGraph::random(num_nodes, 4, 64, 42);
        let mut tracker = IncrementalTracker::new(graph);
        let node_id = (num_nodes / 2) as u64;

        let sample_size = if num_nodes > 50_000 { 20 } else { 100 };
        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("single_node", num_nodes),
            &num_nodes,
            |b, _| {
                b.iter(|| {
                    let new_state = generate_state(64, rand::random());
                    tracker.update_node(black_box(node_id), new_state);
                    black_box(tracker.total_energy)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark restriction map application
fn bench_restriction_map_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("coherence_restriction_map");
    group.throughput(Throughput::Elements(1));

    let dim = 64;
    let input = generate_state(dim, 42);

    // Identity map
    {
        let rho = RestrictionMap::identity(dim);
        let mut output = vec![0.0f32; dim];

        group.bench_function("identity", |b| {
            b.iter(|| {
                rho.apply_identity_into(black_box(&input), &mut output);
                black_box(output[0])
            })
        });
    }

    // Diagonal map
    {
        let rho = RestrictionMap::diagonal(dim, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_function("diagonal", |b| {
            b.iter(|| {
                rho.apply_diagonal_into(black_box(&input), &mut output);
                black_box(output[0])
            })
        });
    }

    // Dense map (64x64)
    {
        let rho = RestrictionMap::dense(dim, dim, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_function("dense_64x64", |b| {
            b.iter(|| {
                rho.apply_into(black_box(&input), &mut output);
                black_box(output[0])
            })
        });
    }

    // Dense projection (64x32)
    {
        let rho = RestrictionMap::dense(64, 32, 42);
        let mut output = vec![0.0f32; 32];

        group.bench_function("dense_64x32", |b| {
            b.iter(|| {
                rho.apply_into(black_box(&input), &mut output);
                black_box(output[0])
            })
        });
    }

    // Sparse map (10% density)
    {
        let rho = RestrictionMap::sparse(dim, dim, 0.1, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_function("sparse_10pct", |b| {
            b.iter(|| {
                rho.apply_into(black_box(&input), &mut output);
                black_box(output[0])
            })
        });
    }

    // Sparse map (30% density)
    {
        let rho = RestrictionMap::sparse(dim, dim, 0.3, 42);
        let mut output = vec![0.0f32; dim];

        group.bench_function("sparse_30pct", |b| {
            b.iter(|| {
                rho.apply_into(black_box(&input), &mut output);
                black_box(output[0])
            })
        });
    }

    group.finish();
}

// ============================================================================
// SCALING BENCHMARKS
// ============================================================================

/// Benchmark energy computation scaling with node count
fn bench_scaling_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_nodes");

    let node_counts = [100, 500, 1000, 2000, 5000, 10000];

    for &num_nodes in &node_counts {
        let graph = SheafGraph::random(num_nodes, 4, 64, 42);

        let sample_size = if num_nodes > 5000 { 20 } else { 50 };
        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(graph.edges.len() as u64));

        group.bench_with_input(BenchmarkId::new("energy", num_nodes), &num_nodes, |b, _| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    group.finish();
}

/// Benchmark energy computation scaling with edge density
fn bench_scaling_edges(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_edges");

    let num_nodes = 1000;
    let avg_degrees = [2, 4, 8, 16, 32, 64];

    for &avg_degree in &avg_degrees {
        let graph = SheafGraph::random(num_nodes, avg_degree, 64, 42);

        group.throughput(Throughput::Elements(graph.edges.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("avg_degree", avg_degree),
            &avg_degree,
            |b, _| b.iter(|| black_box(graph.compute_total_energy())),
        );
    }

    group.finish();
}

/// Benchmark computation scaling with state vector dimension
fn bench_scaling_dimension(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_dimension");

    let num_nodes = 1000;
    let dimensions = [16, 32, 64, 128, 256, 512, 1024];

    for &dim in &dimensions {
        let graph = SheafGraph::random(num_nodes, 4, dim, 42);

        let sample_size = if dim > 512 { 20 } else { 50 };
        group.sample_size(sample_size);
        group.throughput(Throughput::Elements(graph.edges.len() as u64));

        group.bench_with_input(BenchmarkId::new("state_dim", dim), &dim, |b, _| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    group.finish();
}

/// Benchmark with different restriction map types
fn bench_restriction_map_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("restriction_map_types");

    let num_nodes = 1000;
    let state_dim = 64;

    // Identity maps
    {
        let graph = SheafGraph::with_restriction_type(
            num_nodes,
            4,
            state_dim,
            state_dim,
            MapType::Identity,
            42,
        );
        group.throughput(Throughput::Elements(graph.edges.len() as u64));
        group.bench_function("identity", |b| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    // Diagonal maps
    {
        let graph = SheafGraph::with_restriction_type(
            num_nodes,
            4,
            state_dim,
            state_dim,
            MapType::Diagonal,
            42,
        );
        group.bench_function("diagonal", |b| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    // Dense maps
    {
        let graph = SheafGraph::with_restriction_type(
            num_nodes,
            4,
            state_dim,
            state_dim,
            MapType::Dense,
            42,
        );
        group.bench_function("dense", |b| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    // Dense projection (64 -> 32)
    {
        let graph =
            SheafGraph::with_restriction_type(num_nodes, 4, state_dim, 32, MapType::Dense, 42);
        group.bench_function("dense_projection", |b| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    // Sparse 10%
    {
        let graph = SheafGraph::with_restriction_type(
            num_nodes,
            4,
            state_dim,
            state_dim,
            MapType::Sparse { density: 0.1 },
            42,
        );
        group.bench_function("sparse_10pct", |b| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    group.finish();
}

// ============================================================================
// NORM COMPUTATION BENCHMARKS
// ============================================================================

/// Benchmark norm computation variants
fn bench_norm_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("norm_computation");

    for dim in [64, 256, 1024] {
        let v = generate_state(dim, 42);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("naive", dim), &dim, |b, _| {
            b.iter(|| black_box(norm_sq_naive(black_box(&v))))
        });

        group.bench_with_input(BenchmarkId::new("unrolled", dim), &dim, |b, _| {
            b.iter(|| black_box(norm_sq_unrolled(black_box(&v))))
        });

        // Iterator-based (auto-vectorization friendly)
        group.bench_with_input(BenchmarkId::new("iter_fold", dim), &dim, |b, _| {
            b.iter(|| {
                let sum: f32 = black_box(&v).iter().fold(0.0, |acc, &x| acc + x * x);
                black_box(sum)
            })
        });
    }

    group.finish();
}

// ============================================================================
// BATCH PROCESSING BENCHMARKS
// ============================================================================

/// Benchmark batch residual computation
fn bench_batch_residual(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_residual");

    let dim = 64;

    for batch_size in [10, 100, 1000] {
        let edges: Vec<SheafEdge> = (0..batch_size)
            .map(|i| SheafEdge {
                id: i as u64,
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

        // Sequential processing
        group.bench_with_input(
            BenchmarkId::new("sequential", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    let mut source_buf = vec![0.0f32; dim];
                    let mut target_buf = vec![0.0f32; dim];
                    let mut total = 0.0f32;

                    for (i, edge) in edges.iter().enumerate() {
                        total += edge.weighted_residual_energy_into(
                            &states[i],
                            &states[i + 1],
                            &mut source_buf,
                            &mut target_buf,
                        );
                    }
                    black_box(total)
                })
            },
        );

        // Separate buffer per edge (more allocations but parallelizable)
        group.bench_with_input(
            BenchmarkId::new("per_edge_buffers", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    let total: f32 = edges
                        .iter()
                        .enumerate()
                        .map(|(i, edge)| {
                            let mut source_buf = vec![0.0f32; dim];
                            let mut target_buf = vec![0.0f32; dim];
                            edge.weighted_residual_energy_into(
                                &states[i],
                                &states[i + 1],
                                &mut source_buf,
                                &mut target_buf,
                            )
                        })
                        .sum();
                    black_box(total)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark memory access patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    let num_nodes = 10000;
    let dim = 64;

    // Chain graph (sequential access)
    {
        let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
            .map(|id| {
                (
                    id,
                    SheafNode {
                        id,
                        state: generate_state(dim, id),
                    },
                )
            })
            .collect();

        let edges: Vec<SheafEdge> = (0..num_nodes - 1)
            .map(|i| SheafEdge {
                id: i as u64,
                source: i as u64,
                target: (i + 1) as u64,
                weight: 1.0,
                rho_source: RestrictionMap::identity(dim),
                rho_target: RestrictionMap::identity(dim),
            })
            .collect();

        let graph = SheafGraph {
            nodes,
            edges,
            state_dim: dim,
            edge_dim: dim,
        };

        group.throughput(Throughput::Elements(graph.edges.len() as u64));
        group.bench_function("sequential_access", |b| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    // Random graph (random access)
    {
        let graph = SheafGraph::random(num_nodes, 4, dim, 42);
        group.bench_function("random_access", |b| {
            b.iter(|| black_box(graph.compute_total_energy()))
        });
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    coherence_core,
    bench_residual_computation,
    bench_energy_computation,
    bench_incremental_update,
    bench_restriction_map_apply,
);

criterion_group!(
    scaling_tests,
    bench_scaling_nodes,
    bench_scaling_edges,
    bench_scaling_dimension,
    bench_restriction_map_types,
);

criterion_group!(
    optimization_tests,
    bench_norm_computation,
    bench_batch_residual,
    bench_memory_patterns,
);

criterion_main!(coherence_core, scaling_tests, optimization_tests);
