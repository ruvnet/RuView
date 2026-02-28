//! Cohomology Benchmarks for Prime-Radiant
//!
//! Benchmarks for sheaf cohomology computations including:
//! - Coboundary operators at various graph sizes
//! - Cohomology group computation
//! - Sheaf neural network layer operations
//!
//! Target metrics:
//! - Coboundary: < 1ms for 100 nodes, < 10ms for 1K nodes
//! - Cohomology groups: < 5ms for 1K nodes
//! - Sheaf neural layer: < 2ms per forward pass

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::HashMap;

// ============================================================================
// MOCK TYPES FOR COHOMOLOGY BENCHMARKING
// ============================================================================

/// Sparse matrix representation for boundary/coboundary operators
#[derive(Clone)]
struct SparseMatrix {
    rows: usize,
    cols: usize,
    data: Vec<(usize, usize, f64)>, // (row, col, value)
}

impl SparseMatrix {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: Vec::new(),
        }
    }

    fn insert(&mut self, row: usize, col: usize, value: f64) {
        if value.abs() > 1e-10 {
            self.data.push((row, col, value));
        }
    }

    fn multiply_vector(&self, v: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(row, col, val) in &self.data {
            if col < v.len() {
                result[row] += val * v[col];
            }
        }
        result
    }

    fn transpose(&self) -> Self {
        let mut transposed = SparseMatrix::new(self.cols, self.rows);
        for &(row, col, val) in &self.data {
            transposed.insert(col, row, val);
        }
        transposed
    }
}

/// Simplicial complex for cohomology computation
struct SimplicialComplex {
    vertices: Vec<usize>,
    edges: Vec<(usize, usize)>,
    triangles: Vec<(usize, usize, usize)>,
}

impl SimplicialComplex {
    fn from_graph(num_nodes: usize, edges: Vec<(usize, usize)>) -> Self {
        let vertices: Vec<usize> = (0..num_nodes).collect();

        // Find triangles (3-cliques)
        let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();
        for &(u, v) in &edges {
            adjacency.entry(u).or_default().push(v);
            adjacency.entry(v).or_default().push(u);
        }

        let mut triangles = Vec::new();
        for &(u, v) in &edges {
            if let (Some(neighbors_u), Some(neighbors_v)) = (adjacency.get(&u), adjacency.get(&v)) {
                for &w in neighbors_u {
                    if w > v && neighbors_v.contains(&w) {
                        triangles.push((u, v, w));
                    }
                }
            }
        }

        Self {
            vertices,
            edges,
            triangles,
        }
    }

    fn num_vertices(&self) -> usize {
        self.vertices.len()
    }

    fn num_edges(&self) -> usize {
        self.edges.len()
    }

    fn num_triangles(&self) -> usize {
        self.triangles.len()
    }
}

/// Coboundary operator computation
struct CoboundaryOperator {
    /// Coboundary from 0-cochains to 1-cochains (d0)
    d0: SparseMatrix,
    /// Coboundary from 1-cochains to 2-cochains (d1)
    d1: SparseMatrix,
}

impl CoboundaryOperator {
    fn from_complex(complex: &SimplicialComplex) -> Self {
        let num_v = complex.num_vertices();
        let num_e = complex.num_edges();
        let num_t = complex.num_triangles();

        // Build d0: C^0 -> C^1 (vertices to edges)
        let mut d0 = SparseMatrix::new(num_e, num_v);
        for (i, &(u, v)) in complex.edges.iter().enumerate() {
            d0.insert(i, u, -1.0);
            d0.insert(i, v, 1.0);
        }

        // Build d1: C^1 -> C^2 (edges to triangles)
        let mut d1 = SparseMatrix::new(num_t, num_e);

        // Create edge index map
        let edge_map: HashMap<(usize, usize), usize> = complex
            .edges
            .iter()
            .enumerate()
            .map(|(i, &(u, v))| ((u.min(v), u.max(v)), i))
            .collect();

        for (i, &(a, b, c)) in complex.triangles.iter().enumerate() {
            // Triangle boundary: ab - ac + bc
            if let Some(&e_ab) = edge_map.get(&(a.min(b), a.max(b))) {
                d1.insert(i, e_ab, 1.0);
            }
            if let Some(&e_ac) = edge_map.get(&(a.min(c), a.max(c))) {
                d1.insert(i, e_ac, -1.0);
            }
            if let Some(&e_bc) = edge_map.get(&(b.min(c), b.max(c))) {
                d1.insert(i, e_bc, 1.0);
            }
        }

        Self { d0, d1 }
    }

    fn apply_d0(&self, cochain: &[f64]) -> Vec<f64> {
        self.d0.multiply_vector(cochain)
    }

    fn apply_d1(&self, cochain: &[f64]) -> Vec<f64> {
        self.d1.multiply_vector(cochain)
    }
}

/// Cohomology group computation via Hodge decomposition
struct CohomologyComputer {
    coboundary: CoboundaryOperator,
    laplacian_0: SparseMatrix,
    laplacian_1: SparseMatrix,
}

impl CohomologyComputer {
    fn new(complex: &SimplicialComplex) -> Self {
        let coboundary = CoboundaryOperator::from_complex(complex);

        // Hodge Laplacian L_k = d_k^* d_k + d_{k-1} d_{k-1}^*
        // For 0-forms: L_0 = d_0^* d_0
        // For 1-forms: L_1 = d_1^* d_1 + d_0 d_0^*

        let d0_t = coboundary.d0.transpose();
        let d1_t = coboundary.d1.transpose();

        // Simplified Laplacian computation (degree matrix - adjacency)
        let laplacian_0 = Self::compute_graph_laplacian(complex);
        let laplacian_1 = Self::compute_edge_laplacian(complex);

        Self {
            coboundary,
            laplacian_0,
            laplacian_1,
        }
    }

    fn compute_graph_laplacian(complex: &SimplicialComplex) -> SparseMatrix {
        let n = complex.num_vertices();
        let mut laplacian = SparseMatrix::new(n, n);
        let mut degrees = vec![0.0; n];

        for &(u, v) in &complex.edges {
            degrees[u] += 1.0;
            degrees[v] += 1.0;
            laplacian.insert(u, v, -1.0);
            laplacian.insert(v, u, -1.0);
        }

        for (i, &d) in degrees.iter().enumerate() {
            laplacian.insert(i, i, d);
        }

        laplacian
    }

    fn compute_edge_laplacian(complex: &SimplicialComplex) -> SparseMatrix {
        let m = complex.num_edges();
        let mut laplacian = SparseMatrix::new(m, m);

        // Edge Laplacian: edges sharing a vertex are connected
        for (i, &(u1, v1)) in complex.edges.iter().enumerate() {
            let mut degree = 0.0;
            for (j, &(u2, v2)) in complex.edges.iter().enumerate() {
                if i != j && (u1 == u2 || u1 == v2 || v1 == u2 || v1 == v2) {
                    laplacian.insert(i, j, -1.0);
                    degree += 1.0;
                }
            }
            laplacian.insert(i, i, degree);
        }

        laplacian
    }

    fn compute_betti_0(&self) -> usize {
        // Betti_0 = dim(ker(d0)) = connected components
        // Use power iteration to estimate null space dimension
        self.estimate_kernel_dimension(&self.laplacian_0, 1e-6)
    }

    fn compute_betti_1(&self) -> usize {
        // Betti_1 = dim(ker(L_1)) = number of independent cycles
        self.estimate_kernel_dimension(&self.laplacian_1, 1e-6)
    }

    fn estimate_kernel_dimension(&self, laplacian: &SparseMatrix, tolerance: f64) -> usize {
        // Count eigenvalues near zero using power iteration on shifted matrix
        let n = laplacian.rows;
        if n == 0 {
            return 0;
        }

        // Simplified: use trace-based estimation
        let mut trace = 0.0;
        for &(row, col, val) in &laplacian.data {
            if row == col {
                trace += val;
            }
        }

        // Estimate kernel dimension from spectral gap
        let avg_degree = trace / n as f64;
        if avg_degree < tolerance {
            n
        } else {
            1 // At least one connected component
        }
    }

    fn compute_cohomology_class(&self, cochain: &[f64]) -> Vec<f64> {
        // Project cochain onto harmonic forms (kernel of Laplacian)
        let d_cochain = self.coboundary.apply_d0(cochain);

        // Subtract exact part
        let mut harmonic = cochain.to_vec();
        let exact_energy: f64 = d_cochain.iter().map(|x| x * x).sum();

        if exact_energy > 1e-10 {
            // Simple projection (full implementation would use Hodge decomposition)
            let scale = 1.0 / (1.0 + exact_energy.sqrt());
            for h in &mut harmonic {
                *h *= scale;
            }
        }

        harmonic
    }
}

/// Sheaf neural network layer
struct SheafNeuralLayer {
    /// Node feature dimension
    node_dim: usize,
    /// Edge feature dimension (stalk dimension)
    edge_dim: usize,
    /// Restriction map weights (per edge type)
    restriction_weights: Vec<Vec<f64>>,
    /// Aggregation weights
    aggregation_weights: Vec<f64>,
}

impl SheafNeuralLayer {
    fn new(node_dim: usize, edge_dim: usize, num_edges: usize) -> Self {
        // Initialize with random weights
        let restriction_weights: Vec<Vec<f64>> = (0..num_edges)
            .map(|_| {
                (0..node_dim * edge_dim)
                    .map(|i| ((i as f64 * 0.1).sin() * 0.1))
                    .collect()
            })
            .collect();

        let aggregation_weights: Vec<f64> = (0..edge_dim * node_dim)
            .map(|i| ((i as f64 * 0.2).cos() * 0.1))
            .collect();

        Self {
            node_dim,
            edge_dim,
            restriction_weights,
            aggregation_weights,
        }
    }

    fn forward(&self, node_features: &[Vec<f64>], edges: &[(usize, usize)]) -> Vec<Vec<f64>> {
        let num_nodes = node_features.len();
        let mut output = vec![vec![0.0; self.node_dim]; num_nodes];

        // Message passing with sheaf structure
        for (edge_idx, &(src, dst)) in edges.iter().enumerate() {
            if src >= num_nodes || dst >= num_nodes {
                continue;
            }

            // Apply restriction map to source
            let restricted = self.apply_restriction(
                &node_features[src],
                edge_idx % self.restriction_weights.len(),
            );

            // Aggregate at destination
            for (i, &r) in restricted.iter().enumerate().take(self.node_dim) {
                output[dst][i] += r;
            }
        }

        // Apply non-linearity (ReLU)
        for node_output in &mut output {
            for val in node_output {
                *val = val.max(0.0);
            }
        }

        output
    }

    fn apply_restriction(&self, features: &[f64], edge_idx: usize) -> Vec<f64> {
        let weights = &self.restriction_weights[edge_idx];
        let mut result = vec![0.0; self.edge_dim];

        for (i, r) in result.iter_mut().enumerate() {
            for (j, &f) in features.iter().enumerate().take(self.node_dim) {
                let w_idx = i * self.node_dim + j;
                if w_idx < weights.len() {
                    *r += weights[w_idx] * f;
                }
            }
        }

        result
    }

    fn compute_cohomology_loss(&self, node_features: &[Vec<f64>], edges: &[(usize, usize)]) -> f64 {
        // Sheaf Laplacian-based loss: measures deviation from global section
        let mut loss = 0.0;

        for (edge_idx, &(src, dst)) in edges.iter().enumerate() {
            if src >= node_features.len() || dst >= node_features.len() {
                continue;
            }

            let restricted_src = self.apply_restriction(
                &node_features[src],
                edge_idx % self.restriction_weights.len(),
            );
            let restricted_dst = self.apply_restriction(
                &node_features[dst],
                edge_idx % self.restriction_weights.len(),
            );

            // Residual: difference of restricted sections
            for (rs, rd) in restricted_src.iter().zip(restricted_dst.iter()) {
                let diff = rs - rd;
                loss += diff * diff;
            }
        }

        loss
    }
}

// ============================================================================
// GRAPH GENERATORS
// ============================================================================

fn generate_random_graph(num_nodes: usize, edge_probability: f64, seed: u64) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    let mut rng_state = seed;

    for i in 0..num_nodes {
        for j in (i + 1)..num_nodes {
            // Simple LCG for deterministic "random" numbers
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let random = (rng_state >> 33) as f64 / (u32::MAX as f64);

            if random < edge_probability {
                edges.push((i, j));
            }
        }
    }

    edges
}

fn generate_grid_graph(width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let node = y * width + x;

            // Right neighbor
            if x + 1 < width {
                edges.push((node, node + 1));
            }

            // Bottom neighbor
            if y + 1 < height {
                edges.push((node, node + width));
            }
        }
    }

    edges
}

// ============================================================================
// BENCHMARKS
// ============================================================================

fn bench_coboundary_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cohomology/coboundary");
    group.sample_size(50);

    for &num_nodes in &[100, 500, 1000, 5000, 10000] {
        let edges = generate_random_graph(num_nodes, 3.0 / num_nodes as f64, 42);
        let complex = SimplicialComplex::from_graph(num_nodes, edges);
        let coboundary = CoboundaryOperator::from_complex(&complex);

        let cochain: Vec<f64> = (0..num_nodes).map(|i| (i as f64).sin()).collect();

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("d0_apply", num_nodes),
            &(&coboundary, &cochain),
            |b, (cob, cochain)| {
                b.iter(|| {
                    black_box(cob.apply_d0(black_box(cochain)))
                })
            },
        );
    }

    group.finish();
}

fn bench_cohomology_groups(c: &mut Criterion) {
    let mut group = c.benchmark_group("cohomology/groups");
    group.sample_size(30);

    for &num_nodes in &[100, 500, 1000, 2000] {
        let edges = generate_random_graph(num_nodes, 4.0 / num_nodes as f64, 42);
        let complex = SimplicialComplex::from_graph(num_nodes, edges);

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("betti_0", num_nodes),
            &complex,
            |b, complex| {
                b.iter(|| {
                    let computer = CohomologyComputer::new(black_box(complex));
                    black_box(computer.compute_betti_0())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("betti_1", num_nodes),
            &complex,
            |b, complex| {
                b.iter(|| {
                    let computer = CohomologyComputer::new(black_box(complex));
                    black_box(computer.compute_betti_1())
                })
            },
        );
    }

    group.finish();
}

fn bench_cohomology_class(c: &mut Criterion) {
    let mut group = c.benchmark_group("cohomology/class_computation");
    group.sample_size(50);

    for &num_nodes in &[100, 500, 1000] {
        let edges = generate_random_graph(num_nodes, 4.0 / num_nodes as f64, 42);
        let complex = SimplicialComplex::from_graph(num_nodes, edges);
        let computer = CohomologyComputer::new(&complex);

        let cochain: Vec<f64> = (0..num_nodes).map(|i| (i as f64 * 0.1).sin()).collect();

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("project_harmonic", num_nodes),
            &(&computer, &cochain),
            |b, (comp, cochain)| {
                b.iter(|| {
                    black_box(comp.compute_cohomology_class(black_box(cochain)))
                })
            },
        );
    }

    group.finish();
}

fn bench_sheaf_neural_layer(c: &mut Criterion) {
    let mut group = c.benchmark_group("cohomology/sheaf_neural");
    group.sample_size(50);

    let feature_dim = 64;
    let edge_dim = 32;

    for &num_nodes in &[100, 500, 1000, 2000] {
        let edges = generate_random_graph(num_nodes, 5.0 / num_nodes as f64, 42);
        let num_edges = edges.len();

        let layer = SheafNeuralLayer::new(feature_dim, edge_dim, num_edges.max(1));

        let node_features: Vec<Vec<f64>> = (0..num_nodes)
            .map(|i| (0..feature_dim).map(|j| ((i + j) as f64 * 0.1).sin()).collect())
            .collect();

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("forward", num_nodes),
            &(&layer, &node_features, &edges),
            |b, (layer, features, edges)| {
                b.iter(|| {
                    black_box(layer.forward(black_box(features), black_box(edges)))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("cohomology_loss", num_nodes),
            &(&layer, &node_features, &edges),
            |b, (layer, features, edges)| {
                b.iter(|| {
                    black_box(layer.compute_cohomology_loss(black_box(features), black_box(edges)))
                })
            },
        );
    }

    group.finish();
}

fn bench_grid_topology(c: &mut Criterion) {
    let mut group = c.benchmark_group("cohomology/grid_topology");
    group.sample_size(30);

    for &size in &[10, 20, 32, 50] {
        let num_nodes = size * size;
        let edges = generate_grid_graph(size, size);
        let complex = SimplicialComplex::from_graph(num_nodes, edges.clone());

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("build_coboundary", format!("{}x{}", size, size)),
            &complex,
            |b, complex| {
                b.iter(|| {
                    black_box(CoboundaryOperator::from_complex(black_box(complex)))
                })
            },
        );

        let layer = SheafNeuralLayer::new(32, 16, edges.len().max(1));
        let features: Vec<Vec<f64>> = (0..num_nodes)
            .map(|i| (0..32).map(|j| ((i + j) as f64 * 0.1).cos()).collect())
            .collect();

        group.bench_with_input(
            BenchmarkId::new("sheaf_layer", format!("{}x{}", size, size)),
            &(&layer, &features, &edges),
            |b, (layer, features, edges)| {
                b.iter(|| {
                    black_box(layer.forward(black_box(features), black_box(edges)))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_coboundary_computation,
    bench_cohomology_groups,
    bench_cohomology_class,
    bench_sheaf_neural_layer,
    bench_grid_topology,
);
criterion_main!(benches);
