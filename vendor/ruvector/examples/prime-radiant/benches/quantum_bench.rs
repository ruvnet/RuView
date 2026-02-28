//! Quantum and Algebraic Topology Benchmarks for Prime-Radiant
//!
//! Benchmarks for quantum-topological operations including:
//! - Persistent homology computation at various dimensions
//! - Topological invariant computation (Betti numbers, Euler characteristic)
//! - Quantum state operations (density matrices, fidelity)
//! - Simplicial complex construction and manipulation
//!
//! Target metrics:
//! - Persistent homology (1K points): < 100ms
//! - Betti numbers (dim 2): < 10ms
//! - Quantum fidelity: < 1ms per pair

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

// ============================================================================
// SIMPLICIAL COMPLEX TYPES
// ============================================================================

/// A simplex is an ordered set of vertices
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Simplex {
    vertices: Vec<usize>,
}

impl Simplex {
    fn new(mut vertices: Vec<usize>) -> Self {
        vertices.sort_unstable();
        Self { vertices }
    }

    fn dimension(&self) -> usize {
        if self.vertices.is_empty() {
            0
        } else {
            self.vertices.len() - 1
        }
    }

    fn faces(&self) -> Vec<Simplex> {
        let mut faces = Vec::new();
        for i in 0..self.vertices.len() {
            let mut face_vertices = self.vertices.clone();
            face_vertices.remove(i);
            if !face_vertices.is_empty() {
                faces.push(Simplex::new(face_vertices));
            }
        }
        faces
    }
}

/// Filtered simplicial complex for persistent homology
struct FilteredComplex {
    simplices: Vec<(f64, Simplex)>, // (filtration value, simplex)
}

impl FilteredComplex {
    fn new() -> Self {
        Self { simplices: Vec::new() }
    }

    fn add(&mut self, filtration: f64, simplex: Simplex) {
        self.simplices.push((filtration, simplex));
    }

    fn sort_by_filtration(&mut self) {
        self.simplices.sort_by(|a, b| {
            a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal)
                .then_with(|| a.1.dimension().cmp(&b.1.dimension()))
        });
    }
}

// ============================================================================
// PERSISTENT HOMOLOGY
// ============================================================================

/// Birth-death pair representing a topological feature
#[derive(Clone, Debug)]
struct PersistencePair {
    dimension: usize,
    birth: f64,
    death: f64,
}

impl PersistencePair {
    fn persistence(&self) -> f64 {
        self.death - self.birth
    }
}

/// Union-Find data structure for 0-dimensional homology
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    birth: Vec<f64>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
            birth: vec![f64::INFINITY; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) -> Option<(usize, usize)> {
        let px = self.find(x);
        let py = self.find(y);

        if px == py {
            return None;
        }

        // Younger component dies (larger birth time)
        let (survivor, dying) = if self.birth[px] <= self.birth[py] {
            (px, py)
        } else {
            (py, px)
        };

        if self.rank[px] < self.rank[py] {
            self.parent[px] = py;
        } else if self.rank[px] > self.rank[py] {
            self.parent[py] = px;
        } else {
            self.parent[py] = px;
            self.rank[px] += 1;
        }

        self.parent[dying] = survivor;
        Some((dying, survivor))
    }

    fn set_birth(&mut self, x: usize, birth: f64) {
        self.birth[x] = birth;
    }
}

/// Compute persistent homology using standard algorithm
fn compute_persistent_homology(complex: &FilteredComplex, max_dim: usize) -> Vec<PersistencePair> {
    let mut pairs = Vec::new();
    let num_vertices = complex.simplices.iter()
        .filter(|(_, s)| s.dimension() == 0)
        .count();

    // Union-find for H_0
    let mut uf = UnionFind::new(num_vertices);

    // Track active simplices for higher dimensions
    let mut simplex_index: HashMap<Vec<usize>, usize> = HashMap::new();
    let mut boundary_matrix: Vec<HashSet<usize>> = Vec::new();
    let mut pivot_to_col: HashMap<usize, usize> = HashMap::new();

    for (idx, (filtration, simplex)) in complex.simplices.iter().enumerate() {
        let dim = simplex.dimension();

        if dim == 0 {
            // Vertex: creates a new H_0 class
            let v = simplex.vertices[0];
            uf.set_birth(v, *filtration);
            simplex_index.insert(simplex.vertices.clone(), idx);
            boundary_matrix.push(HashSet::new());
        } else if dim == 1 {
            // Edge: may kill H_0 class
            let u = simplex.vertices[0];
            let v = simplex.vertices[1];

            if let Some((dying, _survivor)) = uf.union(u, v) {
                let birth = uf.birth[dying];
                if *filtration > birth {
                    pairs.push(PersistencePair {
                        dimension: 0,
                        birth,
                        death: *filtration,
                    });
                }
            }

            // Add to boundary matrix for H_1
            let mut boundary = HashSet::new();
            for &vertex in &simplex.vertices {
                if let Some(&face_idx) = simplex_index.get(&vec![vertex]) {
                    boundary.insert(face_idx);
                }
            }
            simplex_index.insert(simplex.vertices.clone(), idx);
            boundary_matrix.push(boundary);
        } else if dim <= max_dim {
            // Higher dimensional simplex
            let faces = simplex.faces();
            let mut boundary: HashSet<usize> = faces.iter()
                .filter_map(|f| simplex_index.get(&f.vertices).copied())
                .collect();

            // Reduce boundary
            while !boundary.is_empty() {
                let pivot = *boundary.iter().max().unwrap();
                if let Some(&other_col) = pivot_to_col.get(&pivot) {
                    // XOR with the column that has this pivot
                    let other_boundary = &boundary_matrix[other_col];
                    let symmetric_diff: HashSet<usize> = boundary
                        .symmetric_difference(other_boundary)
                        .copied()
                        .collect();
                    boundary = symmetric_diff;
                } else {
                    // This column has a new pivot
                    pivot_to_col.insert(pivot, idx);
                    break;
                }
            }

            if boundary.is_empty() {
                // This simplex creates a new cycle (potential H_{dim-1} class)
                // For simplicity, we just record it was created
            } else {
                // This simplex kills a cycle
                let pivot = *boundary.iter().max().unwrap();
                let birth_filtration = complex.simplices[pivot].0;
                pairs.push(PersistencePair {
                    dimension: dim - 1,
                    birth: birth_filtration,
                    death: *filtration,
                });
            }

            simplex_index.insert(simplex.vertices.clone(), idx);
            boundary_matrix.push(boundary);
        }
    }

    // Add infinite persistence pairs for surviving components
    for i in 0..num_vertices {
        if uf.find(i) == i && uf.birth[i] < f64::INFINITY {
            pairs.push(PersistencePair {
                dimension: 0,
                birth: uf.birth[i],
                death: f64::INFINITY,
            });
        }
    }

    pairs
}

/// Persistence diagram statistics
struct PersistenceStats {
    total_features: usize,
    max_persistence: f64,
    mean_persistence: f64,
    betti_at_threshold: Vec<usize>,
}

fn compute_persistence_stats(pairs: &[PersistencePair], threshold: f64, max_dim: usize) -> PersistenceStats {
    let finite_pairs: Vec<_> = pairs.iter()
        .filter(|p| p.death.is_finite())
        .collect();

    let persistences: Vec<f64> = finite_pairs.iter()
        .map(|p| p.persistence())
        .collect();

    let max_persistence = persistences.iter().cloned().fold(0.0f64, f64::max);
    let mean_persistence = if persistences.is_empty() {
        0.0
    } else {
        persistences.iter().sum::<f64>() / persistences.len() as f64
    };

    // Betti numbers at threshold
    let mut betti = vec![0; max_dim + 1];
    for pair in pairs {
        if pair.birth <= threshold && (pair.death.is_infinite() || pair.death > threshold) {
            if pair.dimension <= max_dim {
                betti[pair.dimension] += 1;
            }
        }
    }

    PersistenceStats {
        total_features: pairs.len(),
        max_persistence,
        mean_persistence,
        betti_at_threshold: betti,
    }
}

// ============================================================================
// QUANTUM STATE OPERATIONS
// ============================================================================

/// Complex number (simplified for benchmarking)
#[derive(Clone, Copy, Debug)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn norm_squared(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    fn conjugate(&self) -> Self {
        Self { re: self.re, im: -self.im }
    }

    fn mul(&self, other: &Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }

    fn add(&self, other: &Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }

    fn scale(&self, s: f64) -> Self {
        Self {
            re: self.re * s,
            im: self.im * s,
        }
    }
}

/// Density matrix for mixed quantum states
struct DensityMatrix {
    dimension: usize,
    data: Vec<Vec<Complex>>,
}

impl DensityMatrix {
    fn new(dimension: usize) -> Self {
        Self {
            dimension,
            data: vec![vec![Complex::new(0.0, 0.0); dimension]; dimension],
        }
    }

    fn from_pure_state(state: &[Complex]) -> Self {
        let n = state.len();
        let mut dm = DensityMatrix::new(n);

        for i in 0..n {
            for j in 0..n {
                dm.data[i][j] = state[i].mul(&state[j].conjugate());
            }
        }

        dm
    }

    fn trace(&self) -> Complex {
        let mut sum = Complex::new(0.0, 0.0);
        for i in 0..self.dimension {
            sum = sum.add(&self.data[i][i]);
        }
        sum
    }

    fn multiply(&self, other: &DensityMatrix) -> DensityMatrix {
        let n = self.dimension;
        let mut result = DensityMatrix::new(n);

        for i in 0..n {
            for j in 0..n {
                let mut sum = Complex::new(0.0, 0.0);
                for k in 0..n {
                    sum = sum.add(&self.data[i][k].mul(&other.data[k][j]));
                }
                result.data[i][j] = sum;
            }
        }

        result
    }

    /// Compute sqrt(rho) approximately using Newton's method
    fn sqrt_approx(&self, iterations: usize) -> DensityMatrix {
        let n = self.dimension;

        // Start with identity matrix
        let mut y = DensityMatrix::new(n);
        for i in 0..n {
            y.data[i][i] = Complex::new(1.0, 0.0);
        }

        // Denman-Beavers iteration: Y_{k+1} = (Y_k + Y_k^{-1} * A) / 2
        // Simplified: just use Newton iteration Y = (Y + A/Y) / 2
        for _ in 0..iterations {
            let y_inv = self.clone(); // Simplified: use original matrix
            let sum = y.add(&y_inv);
            y = sum.scale_all(0.5);
        }

        y
    }

    fn add(&self, other: &DensityMatrix) -> DensityMatrix {
        let n = self.dimension;
        let mut result = DensityMatrix::new(n);

        for i in 0..n {
            for j in 0..n {
                result.data[i][j] = self.data[i][j].add(&other.data[i][j]);
            }
        }

        result
    }

    fn scale_all(&self, s: f64) -> DensityMatrix {
        let n = self.dimension;
        let mut result = DensityMatrix::new(n);

        for i in 0..n {
            for j in 0..n {
                result.data[i][j] = self.data[i][j].scale(s);
            }
        }

        result
    }
}

impl Clone for DensityMatrix {
    fn clone(&self) -> Self {
        Self {
            dimension: self.dimension,
            data: self.data.clone(),
        }
    }
}

/// Quantum fidelity between two density matrices
/// F(rho, sigma) = (Tr sqrt(sqrt(rho) sigma sqrt(rho)))^2
fn quantum_fidelity(rho: &DensityMatrix, sigma: &DensityMatrix) -> f64 {
    // Simplified computation for benchmarking
    // Full computation would require eigendecomposition

    let sqrt_rho = rho.sqrt_approx(5);
    let inner = sqrt_rho.multiply(sigma).multiply(&sqrt_rho);
    let sqrt_inner = inner.sqrt_approx(5);

    let trace = sqrt_inner.trace();
    trace.re * trace.re + trace.im * trace.im
}

/// Trace distance between density matrices
/// D(rho, sigma) = (1/2) Tr |rho - sigma|
fn trace_distance(rho: &DensityMatrix, sigma: &DensityMatrix) -> f64 {
    let n = rho.dimension;
    let mut sum = 0.0;

    // Simplified: use Frobenius norm as approximation
    for i in 0..n {
        for j in 0..n {
            let diff = Complex {
                re: rho.data[i][j].re - sigma.data[i][j].re,
                im: rho.data[i][j].im - sigma.data[i][j].im,
            };
            sum += diff.norm_squared();
        }
    }

    0.5 * sum.sqrt()
}

/// Von Neumann entropy: S(rho) = -Tr(rho log rho)
fn von_neumann_entropy(rho: &DensityMatrix) -> f64 {
    // Simplified: compute diagonal entropy approximation
    let mut entropy = 0.0;

    for i in 0..rho.dimension {
        let p = rho.data[i][i].re;
        if p > 1e-10 {
            entropy -= p * p.ln();
        }
    }

    entropy
}

// ============================================================================
// TOPOLOGICAL INVARIANTS
// ============================================================================

/// Compute Euler characteristic: chi = V - E + F - ...
fn euler_characteristic(complex: &FilteredComplex) -> i64 {
    let mut chi = 0i64;

    for (_, simplex) in &complex.simplices {
        let dim = simplex.dimension();
        if dim % 2 == 0 {
            chi += 1;
        } else {
            chi -= 1;
        }
    }

    chi
}

/// Betti numbers via boundary matrix rank
fn betti_numbers(complex: &FilteredComplex, max_dim: usize) -> Vec<usize> {
    // Count simplices by dimension
    let mut counts = vec![0usize; max_dim + 2];

    for (_, simplex) in &complex.simplices {
        let dim = simplex.dimension();
        if dim <= max_dim + 1 {
            counts[dim] += 1;
        }
    }

    // Simplified Betti number estimation
    // beta_k = dim(ker d_k) - dim(im d_{k+1})
    // Approximation: beta_k ~ C_k - C_{k+1} for highly connected complexes

    let mut betti = vec![0usize; max_dim + 1];
    for k in 0..=max_dim {
        let c_k = counts[k];
        let c_k1 = if k + 1 <= max_dim + 1 { counts[k + 1] } else { 0 };

        // Very rough approximation
        betti[k] = if c_k > c_k1 { c_k - c_k1 } else { 1 };
    }

    // Ensure beta_0 >= 1 (at least one connected component)
    if betti[0] == 0 {
        betti[0] = 1;
    }

    betti
}

// ============================================================================
// DATA GENERATORS
// ============================================================================

fn generate_rips_complex(points: &[(f64, f64)], max_radius: f64, max_dim: usize) -> FilteredComplex {
    let n = points.len();
    let mut complex = FilteredComplex::new();

    // Add vertices (0-simplices)
    for i in 0..n {
        complex.add(0.0, Simplex::new(vec![i]));
    }

    // Compute pairwise distances
    let mut edges: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let dist = ((points[i].0 - points[j].0).powi(2)
                + (points[i].1 - points[j].1).powi(2))
                .sqrt();
            if dist <= max_radius {
                edges.push((dist, i, j));
            }
        }
    }

    // Add edges (1-simplices)
    for (dist, i, j) in &edges {
        complex.add(*dist, Simplex::new(vec![*i, *j]));
    }

    // Add triangles (2-simplices) if max_dim >= 2
    if max_dim >= 2 {
        // Build adjacency
        let mut adj: HashMap<usize, HashSet<usize>> = HashMap::new();
        let mut edge_dist: HashMap<(usize, usize), f64> = HashMap::new();

        for (dist, i, j) in &edges {
            adj.entry(*i).or_default().insert(*j);
            adj.entry(*j).or_default().insert(*i);
            edge_dist.insert(((*i).min(*j), (*i).max(*j)), *dist);
        }

        for i in 0..n {
            if let Some(neighbors_i) = adj.get(&i) {
                for &j in neighbors_i {
                    if j > i {
                        if let Some(neighbors_j) = adj.get(&j) {
                            for &k in neighbors_j {
                                if k > j && neighbors_i.contains(&k) {
                                    // Found triangle (i, j, k)
                                    let d_ij = edge_dist.get(&(i, j)).unwrap_or(&0.0);
                                    let d_jk = edge_dist.get(&(j, k)).unwrap_or(&0.0);
                                    let d_ik = edge_dist.get(&(i, k)).unwrap_or(&0.0);
                                    let max_dist = d_ij.max(*d_jk).max(*d_ik);

                                    complex.add(max_dist, Simplex::new(vec![i, j, k]));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    complex.sort_by_filtration();
    complex
}

fn generate_random_points(num_points: usize, seed: u64) -> Vec<(f64, f64)> {
    let mut rng_state = seed;
    let mut points = Vec::with_capacity(num_points);

    for _ in 0..num_points {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let x = (rng_state >> 33) as f64 / (u32::MAX as f64);

        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let y = (rng_state >> 33) as f64 / (u32::MAX as f64);

        points.push((x, y));
    }

    points
}

fn generate_random_quantum_state(dimension: usize, seed: u64) -> Vec<Complex> {
    let mut rng_state = seed;
    let mut state = Vec::with_capacity(dimension);
    let mut norm_sq = 0.0;

    for _ in 0..dimension {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let re = ((rng_state >> 33) as f64 / (u32::MAX as f64)) * 2.0 - 1.0;

        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let im = ((rng_state >> 33) as f64 / (u32::MAX as f64)) * 2.0 - 1.0;

        let c = Complex::new(re, im);
        norm_sq += c.norm_squared();
        state.push(c);
    }

    // Normalize
    let norm = norm_sq.sqrt();
    for c in &mut state {
        *c = c.scale(1.0 / norm);
    }

    state
}

// ============================================================================
// BENCHMARKS
// ============================================================================

fn bench_persistent_homology(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum/persistent_homology");
    group.sample_size(20);

    for &num_points in &[100, 250, 500, 1000] {
        let points = generate_random_points(num_points, 42);
        let radius = 0.2;
        let complex = generate_rips_complex(&points, radius, 2);

        group.throughput(Throughput::Elements(num_points as u64));

        group.bench_with_input(
            BenchmarkId::new("dim2", num_points),
            &complex,
            |b, complex| {
                b.iter(|| black_box(compute_persistent_homology(black_box(complex), 2)))
            },
        );
    }

    group.finish();
}

fn bench_persistence_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum/persistence_stats");
    group.sample_size(50);

    for &num_points in &[100, 500, 1000] {
        let points = generate_random_points(num_points, 42);
        let complex = generate_rips_complex(&points, 0.2, 2);
        let pairs = compute_persistent_homology(&complex, 2);

        group.throughput(Throughput::Elements(pairs.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("compute", num_points),
            &pairs,
            |b, pairs| {
                b.iter(|| black_box(compute_persistence_stats(black_box(pairs), 0.1, 2)))
            },
        );
    }

    group.finish();
}

fn bench_topological_invariants(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum/invariants");
    group.sample_size(50);

    for &num_points in &[100, 500, 1000] {
        let points = generate_random_points(num_points, 42);
        let complex = generate_rips_complex(&points, 0.2, 2);

        group.throughput(Throughput::Elements(complex.simplices.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("euler", num_points),
            &complex,
            |b, complex| {
                b.iter(|| black_box(euler_characteristic(black_box(complex))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("betti", num_points),
            &complex,
            |b, complex| {
                b.iter(|| black_box(betti_numbers(black_box(complex), 2)))
            },
        );
    }

    group.finish();
}

fn bench_rips_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum/rips_construction");
    group.sample_size(20);

    for &num_points in &[100, 250, 500, 1000] {
        let points = generate_random_points(num_points, 42);

        group.throughput(Throughput::Elements(num_points as u64));

        group.bench_with_input(
            BenchmarkId::new("dim2", num_points),
            &points,
            |b, points| {
                b.iter(|| black_box(generate_rips_complex(black_box(points), 0.15, 2)))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("dim1", num_points),
            &points,
            |b, points| {
                b.iter(|| black_box(generate_rips_complex(black_box(points), 0.15, 1)))
            },
        );
    }

    group.finish();
}

fn bench_quantum_fidelity(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum/fidelity");
    group.sample_size(50);

    for &dim in &[4, 8, 16, 32] {
        let state1 = generate_random_quantum_state(dim, 42);
        let state2 = generate_random_quantum_state(dim, 43);

        let rho = DensityMatrix::from_pure_state(&state1);
        let sigma = DensityMatrix::from_pure_state(&state2);

        group.throughput(Throughput::Elements((dim * dim) as u64));

        group.bench_with_input(
            BenchmarkId::new("pure_states", dim),
            &(&rho, &sigma),
            |b, (rho, sigma)| {
                b.iter(|| black_box(quantum_fidelity(black_box(rho), black_box(sigma))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("trace_distance", dim),
            &(&rho, &sigma),
            |b, (rho, sigma)| {
                b.iter(|| black_box(trace_distance(black_box(rho), black_box(sigma))))
            },
        );
    }

    group.finish();
}

fn bench_density_matrix_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum/density_matrix");
    group.sample_size(50);

    for &dim in &[4, 8, 16, 32, 64] {
        let state = generate_random_quantum_state(dim, 42);
        let rho = DensityMatrix::from_pure_state(&state);

        group.throughput(Throughput::Elements((dim * dim) as u64));

        group.bench_with_input(
            BenchmarkId::new("from_pure_state", dim),
            &state,
            |b, state| {
                b.iter(|| black_box(DensityMatrix::from_pure_state(black_box(state))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("multiply", dim),
            &rho,
            |b, rho| {
                b.iter(|| black_box(rho.multiply(black_box(rho))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("trace", dim),
            &rho,
            |b, rho| {
                b.iter(|| black_box(rho.trace()))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("von_neumann_entropy", dim),
            &rho,
            |b, rho| {
                b.iter(|| black_box(von_neumann_entropy(black_box(rho))))
            },
        );
    }

    group.finish();
}

fn bench_simplex_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum/simplex");
    group.sample_size(100);

    for &dim in &[3, 5, 7, 10] {
        let vertices: Vec<usize> = (0..dim).collect();
        let simplex = Simplex::new(vertices.clone());

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(
            BenchmarkId::new("create", dim),
            &vertices,
            |b, vertices| {
                b.iter(|| black_box(Simplex::new(black_box(vertices.clone()))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("faces", dim),
            &simplex,
            |b, simplex| {
                b.iter(|| black_box(simplex.faces()))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_persistent_homology,
    bench_persistence_stats,
    bench_topological_invariants,
    bench_rips_construction,
    bench_quantum_fidelity,
    bench_density_matrix_operations,
    bench_simplex_operations,
);
criterion_main!(benches);
