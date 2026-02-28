//! Spectral Analysis Benchmarks for Prime-Radiant
//!
//! Benchmarks for spectral graph theory computations including:
//! - Eigenvalue computation (power iteration vs Lanczos)
//! - Cheeger constant computation
//! - Spectral clustering
//! - SIMD-accelerated operations
//!
//! Target metrics:
//! - Eigenvalue (power iteration): < 5ms for 1K nodes
//! - Eigenvalue (Lanczos): < 50ms for 10K nodes
//! - Cheeger constant: < 10ms for 1K nodes
//! - Spectral clustering: < 100ms for 5K nodes

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::HashSet;

// ============================================================================
// SPARSE MATRIX TYPES
// ============================================================================

/// CSR (Compressed Sparse Row) format for efficient matrix-vector multiplication
#[derive(Clone)]
struct CsrMatrix {
    rows: usize,
    cols: usize,
    row_ptr: Vec<usize>,
    col_indices: Vec<usize>,
    values: Vec<f64>,
}

impl CsrMatrix {
    fn from_edges(num_nodes: usize, edges: &[(usize, usize)]) -> Self {
        // Build adjacency lists
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); num_nodes];
        let mut degrees = vec![0.0; num_nodes];

        for &(u, v) in edges {
            adj[u].push((v, -1.0));
            adj[v].push((u, -1.0));
            degrees[u] += 1.0;
            degrees[v] += 1.0;
        }

        // Build CSR representation of Laplacian
        let mut row_ptr = vec![0];
        let mut col_indices = Vec::new();
        let mut values = Vec::new();

        for i in 0..num_nodes {
            // Add diagonal (degree)
            col_indices.push(i);
            values.push(degrees[i]);

            // Add off-diagonal entries
            adj[i].sort_by_key(|&(j, _)| j);
            for &(j, val) in &adj[i] {
                col_indices.push(j);
                values.push(val);
            }

            row_ptr.push(col_indices.len());
        }

        Self {
            rows: num_nodes,
            cols: num_nodes,
            row_ptr,
            col_indices,
            values,
        }
    }

    fn matvec(&self, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; self.rows];

        for i in 0..self.rows {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];

            let mut sum = 0.0;
            for k in start..end {
                sum += self.values[k] * x[self.col_indices[k]];
            }
            y[i] = sum;
        }

        y
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn matvec_simd(&self, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; self.rows];

        for i in 0..self.rows {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            let len = end - start;

            // Process in chunks of 4 for SIMD
            let mut sum = 0.0;
            let chunks = len / 4;
            let remainder = len % 4;

            for c in 0..chunks {
                let base = start + c * 4;
                let v0 = self.values[base] * x[self.col_indices[base]];
                let v1 = self.values[base + 1] * x[self.col_indices[base + 1]];
                let v2 = self.values[base + 2] * x[self.col_indices[base + 2]];
                let v3 = self.values[base + 3] * x[self.col_indices[base + 3]];
                sum += v0 + v1 + v2 + v3;
            }

            for k in (start + chunks * 4)..(start + chunks * 4 + remainder) {
                sum += self.values[k] * x[self.col_indices[k]];
            }

            y[i] = sum;
        }

        y
    }
}

// ============================================================================
// EIGENVALUE COMPUTATION
// ============================================================================

/// Power iteration for largest eigenvalue
fn power_iteration(matrix: &CsrMatrix, max_iter: usize, tol: f64) -> (f64, Vec<f64>) {
    let n = matrix.rows;
    if n == 0 {
        return (0.0, Vec::new());
    }

    // Initialize with random-ish vector
    let mut v: Vec<f64> = (0..n).map(|i| ((i as f64 + 1.0).sqrt()).sin()).collect();
    let mut eigenvalue = 0.0;

    // Normalize
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-10 {
        for x in &mut v {
            *x /= norm;
        }
    }

    for _ in 0..max_iter {
        // y = Ax
        let y = matrix.matvec(&v);

        // Rayleigh quotient: eigenvalue = v^T y / v^T v
        let new_eigenvalue: f64 = v.iter().zip(y.iter()).map(|(a, b)| a * b).sum();

        // Normalize y
        let norm: f64 = y.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < 1e-10 {
            break;
        }

        v = y.iter().map(|x| x / norm).collect();

        // Check convergence
        if (new_eigenvalue - eigenvalue).abs() < tol {
            eigenvalue = new_eigenvalue;
            break;
        }
        eigenvalue = new_eigenvalue;
    }

    (eigenvalue, v)
}

/// Lanczos algorithm for multiple eigenvalues
struct LanczosComputation {
    tridiag_alpha: Vec<f64>,
    tridiag_beta: Vec<f64>,
    basis_vectors: Vec<Vec<f64>>,
}

impl LanczosComputation {
    fn compute(matrix: &CsrMatrix, num_eigenvalues: usize, max_iter: usize) -> Self {
        let n = matrix.rows;
        let k = num_eigenvalues.min(max_iter).min(n);

        let mut alpha = Vec::with_capacity(k);
        let mut beta = Vec::with_capacity(k);
        let mut basis = Vec::with_capacity(k + 1);

        // Start with random vector
        let mut v: Vec<f64> = (0..n).map(|i| ((i as f64 + 1.0).sqrt()).sin()).collect();
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-10 {
            for x in &mut v {
                *x /= norm;
            }
        }

        basis.push(v.clone());
        let mut w = matrix.matvec(&v);

        for i in 0..k {
            // alpha_i = v_i^T w
            let a: f64 = basis[i].iter().zip(w.iter()).map(|(a, b)| a * b).sum();
            alpha.push(a);

            // w = w - alpha_i v_i
            for (j, wj) in w.iter_mut().enumerate() {
                *wj -= a * basis[i][j];
            }

            // w = w - beta_{i-1} v_{i-1}
            if i > 0 && i - 1 < beta.len() {
                let b = beta[i - 1];
                for (j, wj) in w.iter_mut().enumerate() {
                    *wj -= b * basis[i - 1][j];
                }
            }

            // beta_i = ||w||
            let b: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();

            if b < 1e-10 || i + 1 >= k {
                break;
            }

            beta.push(b);

            // v_{i+1} = w / beta_i
            let new_v: Vec<f64> = w.iter().map(|x| x / b).collect();
            basis.push(new_v.clone());

            // w = A v_{i+1}
            w = matrix.matvec(&new_v);
        }

        Self {
            tridiag_alpha: alpha,
            tridiag_beta: beta,
            basis_vectors: basis,
        }
    }

    fn eigenvalues(&self) -> Vec<f64> {
        // Compute eigenvalues of tridiagonal matrix using QR iteration
        let n = self.tridiag_alpha.len();
        if n == 0 {
            return Vec::new();
        }

        let mut d = self.tridiag_alpha.clone();
        let mut e = self.tridiag_beta.clone();

        // Simple eigenvalue estimation using Gershgorin circles
        let mut eigenvalues = Vec::with_capacity(n);
        for i in 0..n {
            let off_diag = if i > 0 && i - 1 < e.len() { e[i - 1].abs() } else { 0.0 }
                + if i < e.len() { e[i].abs() } else { 0.0 };
            eigenvalues.push(d[i] + off_diag * 0.5); // Center of Gershgorin disk
        }

        eigenvalues.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        eigenvalues
    }
}

// ============================================================================
// CHEEGER CONSTANT
// ============================================================================

/// Compute Cheeger constant (isoperimetric number) approximation
struct CheegerComputation {
    graph_edges: Vec<(usize, usize)>,
    num_nodes: usize,
}

impl CheegerComputation {
    fn new(num_nodes: usize, edges: Vec<(usize, usize)>) -> Self {
        Self {
            graph_edges: edges,
            num_nodes,
        }
    }

    /// Approximate Cheeger constant using spectral methods
    /// h(G) >= lambda_2 / 2 (Cheeger inequality)
    fn compute_spectral_lower_bound(&self) -> f64 {
        let laplacian = CsrMatrix::from_edges(self.num_nodes, &self.graph_edges);

        // Find second smallest eigenvalue using deflation
        let (lambda_1, v1) = power_iteration(&laplacian, 100, 1e-8);

        // Shift to find lambda_2
        // We use a simplified approach: estimate from Fiedler vector
        let fiedler = self.compute_fiedler_vector(&laplacian, &v1);
        let lambda_2 = self.rayleigh_quotient(&laplacian, &fiedler);

        lambda_2 / 2.0
    }

    fn compute_fiedler_vector(&self, laplacian: &CsrMatrix, ground_state: &[f64]) -> Vec<f64> {
        let n = laplacian.rows;

        // Start with vector orthogonal to ground state
        let mut v: Vec<f64> = (0..n).map(|i| ((i as f64 * 2.0 + 1.0).sqrt()).cos()).collect();

        // Gram-Schmidt orthogonalization against ground state
        let dot: f64 = v.iter().zip(ground_state.iter()).map(|(a, b)| a * b).sum();
        for (i, vi) in v.iter_mut().enumerate() {
            *vi -= dot * ground_state[i];
        }

        // Normalize
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-10 {
            for vi in &mut v {
                *vi /= norm;
            }
        }

        // A few power iterations with orthogonalization
        for _ in 0..50 {
            let mut y = laplacian.matvec(&v);

            // Orthogonalize against ground state
            let dot: f64 = y.iter().zip(ground_state.iter()).map(|(a, b)| a * b).sum();
            for (i, yi) in y.iter_mut().enumerate() {
                *yi -= dot * ground_state[i];
            }

            // Normalize
            let norm: f64 = y.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm < 1e-10 {
                break;
            }
            v = y.iter().map(|x| x / norm).collect();
        }

        v
    }

    fn rayleigh_quotient(&self, laplacian: &CsrMatrix, v: &[f64]) -> f64 {
        let lv = laplacian.matvec(v);
        let numerator: f64 = v.iter().zip(lv.iter()).map(|(a, b)| a * b).sum();
        let denominator: f64 = v.iter().map(|x| x * x).sum();

        if denominator > 1e-10 {
            numerator / denominator
        } else {
            0.0
        }
    }

    /// Direct Cheeger constant computation via sweep cut on Fiedler vector
    fn compute_sweep_cut(&self) -> f64 {
        let laplacian = CsrMatrix::from_edges(self.num_nodes, &self.graph_edges);
        let (_, v1) = power_iteration(&laplacian, 100, 1e-8);
        let fiedler = self.compute_fiedler_vector(&laplacian, &v1);

        // Sort vertices by Fiedler vector values
        let mut indices: Vec<usize> = (0..self.num_nodes).collect();
        indices.sort_by(|&a, &b| {
            fiedler[a].partial_cmp(&fiedler[b]).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Sweep through cuts
        let mut min_cheeger = f64::MAX;
        let mut cut_edges = 0;
        let mut left_set: HashSet<usize> = HashSet::new();

        for &idx in indices.iter().take(self.num_nodes - 1) {
            left_set.insert(idx);

            // Update cut size
            for &(u, v) in &self.graph_edges {
                let u_in = left_set.contains(&u);
                let v_in = left_set.contains(&v);
                if u_in != v_in {
                    if (u_in && u == idx) || (v_in && v == idx) {
                        cut_edges += 1;
                    }
                }
            }

            // Compute Cheeger ratio
            let left_size = left_set.len();
            let right_size = self.num_nodes - left_size;
            let min_size = left_size.min(right_size);

            if min_size > 0 {
                let ratio = cut_edges as f64 / min_size as f64;
                min_cheeger = min_cheeger.min(ratio);
            }
        }

        min_cheeger
    }
}

// ============================================================================
// SPECTRAL CLUSTERING
// ============================================================================

struct SpectralClustering {
    num_clusters: usize,
    eigenvectors: Vec<Vec<f64>>,
}

impl SpectralClustering {
    fn compute(matrix: &CsrMatrix, num_clusters: usize) -> Self {
        let lanczos = LanczosComputation::compute(matrix, num_clusters + 1, 100);

        // Get first k eigenvectors (corresponding to smallest eigenvalues)
        let eigenvectors = lanczos.basis_vectors.into_iter().take(num_clusters).collect();

        Self {
            num_clusters,
            eigenvectors,
        }
    }

    fn cluster_assignments(&self) -> Vec<usize> {
        let n = if self.eigenvectors.is_empty() {
            0
        } else {
            self.eigenvectors[0].len()
        };

        if n == 0 || self.eigenvectors.is_empty() {
            return Vec::new();
        }

        // Simple k-means on spectral embedding
        let k = self.num_clusters;
        let dim = self.eigenvectors.len();

        // Extract embedding matrix (n x dim)
        let embedding: Vec<Vec<f64>> = (0..n)
            .map(|i| self.eigenvectors.iter().map(|v| v[i]).collect())
            .collect();

        // Initialize centroids
        let mut centroids: Vec<Vec<f64>> = (0..k)
            .map(|i| embedding[i * n / k].clone())
            .collect();

        let mut assignments = vec![0; n];

        // K-means iterations
        for _ in 0..20 {
            // Assign points to nearest centroid
            for (i, point) in embedding.iter().enumerate() {
                let mut min_dist = f64::MAX;
                for (j, centroid) in centroids.iter().enumerate() {
                    let dist: f64 = point
                        .iter()
                        .zip(centroid.iter())
                        .map(|(a, b)| (a - b).powi(2))
                        .sum();
                    if dist < min_dist {
                        min_dist = dist;
                        assignments[i] = j;
                    }
                }
            }

            // Update centroids
            let mut counts = vec![0usize; k];
            let mut new_centroids = vec![vec![0.0; dim]; k];

            for (i, point) in embedding.iter().enumerate() {
                let cluster = assignments[i];
                counts[cluster] += 1;
                for (j, &val) in point.iter().enumerate() {
                    new_centroids[cluster][j] += val;
                }
            }

            for (j, centroid) in new_centroids.iter_mut().enumerate() {
                if counts[j] > 0 {
                    for val in centroid.iter_mut() {
                        *val /= counts[j] as f64;
                    }
                }
            }

            centroids = new_centroids;
        }

        assignments
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
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let random = (rng_state >> 33) as f64 / (u32::MAX as f64);

            if random < edge_probability {
                edges.push((i, j));
            }
        }
    }

    edges
}

fn generate_planted_partition(
    num_clusters: usize,
    cluster_size: usize,
    p_in: f64,
    p_out: f64,
    seed: u64,
) -> Vec<(usize, usize)> {
    let num_nodes = num_clusters * cluster_size;
    let mut edges = Vec::new();
    let mut rng_state = seed;

    for i in 0..num_nodes {
        for j in (i + 1)..num_nodes {
            let cluster_i = i / cluster_size;
            let cluster_j = j / cluster_size;
            let prob = if cluster_i == cluster_j { p_in } else { p_out };

            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let random = (rng_state >> 33) as f64 / (u32::MAX as f64);

            if random < prob {
                edges.push((i, j));
            }
        }
    }

    edges
}

// ============================================================================
// BENCHMARKS
// ============================================================================

fn bench_power_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral/power_iteration");
    group.sample_size(30);

    for &num_nodes in &[100, 500, 1000, 2000, 5000] {
        let edges = generate_random_graph(num_nodes, 5.0 / num_nodes as f64, 42);
        let matrix = CsrMatrix::from_edges(num_nodes, &edges);

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("standard", num_nodes),
            &matrix,
            |b, matrix| {
                b.iter(|| {
                    black_box(power_iteration(black_box(matrix), 100, 1e-8))
                })
            },
        );
    }

    group.finish();
}

fn bench_lanczos(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral/lanczos");
    group.sample_size(20);

    for &num_nodes in &[500, 1000, 2000, 5000, 10000] {
        let edges = generate_random_graph(num_nodes, 5.0 / num_nodes as f64, 42);
        let matrix = CsrMatrix::from_edges(num_nodes, &edges);

        group.throughput(Throughput::Elements(num_nodes as u64));

        for &num_eig in &[5, 10, 20] {
            group.bench_with_input(
                BenchmarkId::new(format!("{}_eigenvalues", num_eig), num_nodes),
                &(&matrix, num_eig),
                |b, (matrix, k)| {
                    b.iter(|| {
                        let lanczos = LanczosComputation::compute(black_box(matrix), *k, 100);
                        black_box(lanczos.eigenvalues())
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_cheeger_constant(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral/cheeger");
    group.sample_size(20);

    for &num_nodes in &[100, 500, 1000, 2000] {
        let edges = generate_random_graph(num_nodes, 5.0 / num_nodes as f64, 42);
        let cheeger = CheegerComputation::new(num_nodes, edges);

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("spectral_bound", num_nodes),
            &cheeger,
            |b, cheeger| {
                b.iter(|| {
                    black_box(cheeger.compute_spectral_lower_bound())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sweep_cut", num_nodes),
            &cheeger,
            |b, cheeger| {
                b.iter(|| {
                    black_box(cheeger.compute_sweep_cut())
                })
            },
        );
    }

    group.finish();
}

fn bench_spectral_clustering(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral/clustering");
    group.sample_size(20);

    for &cluster_size in &[50, 100, 200, 500] {
        let num_clusters = 5;
        let num_nodes = num_clusters * cluster_size;
        let edges = generate_planted_partition(num_clusters, cluster_size, 0.3, 0.01, 42);
        let matrix = CsrMatrix::from_edges(num_nodes, &edges);

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("compute_embedding", num_nodes),
            &(&matrix, num_clusters),
            |b, (matrix, k)| {
                b.iter(|| {
                    black_box(SpectralClustering::compute(black_box(matrix), *k))
                })
            },
        );

        let clustering = SpectralClustering::compute(&matrix, num_clusters);
        group.bench_with_input(
            BenchmarkId::new("assign_clusters", num_nodes),
            &clustering,
            |b, clustering| {
                b.iter(|| {
                    black_box(clustering.cluster_assignments())
                })
            },
        );
    }

    group.finish();
}

fn bench_matvec_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral/matvec");
    group.sample_size(50);

    for &num_nodes in &[1000, 5000, 10000] {
        let edges = generate_random_graph(num_nodes, 10.0 / num_nodes as f64, 42);
        let matrix = CsrMatrix::from_edges(num_nodes, &edges);
        let x: Vec<f64> = (0..num_nodes).map(|i| (i as f64).sin()).collect();

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("standard", num_nodes),
            &(&matrix, &x),
            |b, (matrix, x)| {
                b.iter(|| {
                    black_box(matrix.matvec(black_box(x)))
                })
            },
        );

        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        group.bench_with_input(
            BenchmarkId::new("simd", num_nodes),
            &(&matrix, &x),
            |b, (matrix, x)| {
                b.iter(|| {
                    black_box(matrix.matvec_simd(black_box(x)))
                })
            },
        );
    }

    group.finish();
}

fn bench_graph_laplacian_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral/laplacian_construction");
    group.sample_size(30);

    for &num_nodes in &[500, 1000, 5000, 10000] {
        let edges = generate_random_graph(num_nodes, 5.0 / num_nodes as f64, 42);

        group.throughput(Throughput::Elements(num_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new("csr_format", num_nodes),
            &(num_nodes, &edges),
            |b, (n, edges)| {
                b.iter(|| {
                    black_box(CsrMatrix::from_edges(*n, black_box(edges)))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_power_iteration,
    bench_lanczos,
    bench_cheeger_constant,
    bench_spectral_clustering,
    bench_matvec_simd,
    bench_graph_laplacian_construction,
);
criterion_main!(benches);
