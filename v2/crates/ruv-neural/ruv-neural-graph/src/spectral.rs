//! Spectral graph properties: Laplacian matrices, Fiedler value, spectral gap.
//!
//! The graph Laplacian encodes the structure of a graph and its eigenvalues
//! reveal fundamental connectivity properties. The Fiedler value (second
//! smallest eigenvalue) measures algebraic connectivity.

use ruv_neural_core::graph::BrainGraph;

/// Compute the combinatorial graph Laplacian L = D - A.
///
/// D is the diagonal degree matrix, A is the adjacency matrix.
/// Returns an `n x n` matrix as `Vec<Vec<f64>>`.
pub fn graph_laplacian(graph: &BrainGraph) -> Vec<Vec<f64>> {
    let n = graph.num_nodes;
    let adj = graph.adjacency_matrix();
    let mut laplacian = vec![vec![0.0; n]; n];

    for i in 0..n {
        let degree: f64 = adj[i].iter().sum();
        laplacian[i][i] = degree;
        for j in 0..n {
            if i != j {
                laplacian[i][j] = -adj[i][j];
            }
        }
    }

    laplacian
}

/// Compute the normalized graph Laplacian L_norm = D^{-1/2} L D^{-1/2}.
///
/// For isolated nodes (degree = 0), the diagonal entry is set to 0.
pub fn normalized_laplacian(graph: &BrainGraph) -> Vec<Vec<f64>> {
    let n = graph.num_nodes;
    let adj = graph.adjacency_matrix();

    // Compute D^{-1/2}
    let degrees: Vec<f64> = (0..n).map(|i| adj[i].iter().sum::<f64>()).collect();
    let d_inv_sqrt: Vec<f64> = degrees
        .iter()
        .map(|&d| if d > 0.0 { 1.0 / d.sqrt() } else { 0.0 })
        .collect();

    let mut l_norm = vec![vec![0.0; n]; n];

    for i in 0..n {
        if degrees[i] > 0.0 {
            l_norm[i][i] = 1.0;
        }
        for j in 0..n {
            if i != j && adj[i][j] > 0.0 {
                l_norm[i][j] = -adj[i][j] * d_inv_sqrt[i] * d_inv_sqrt[j];
            }
        }
    }

    l_norm
}

/// Compute the Fiedler value (algebraic connectivity).
///
/// The Fiedler value is the second smallest eigenvalue of the graph Laplacian.
/// - For a connected graph, Fiedler value > 0.
/// - For a disconnected graph, Fiedler value = 0.
///
/// Uses power iteration with deflation to find the two smallest eigenvalues
/// of the Laplacian (which is positive semidefinite).
pub fn fiedler_value(graph: &BrainGraph) -> f64 {
    let n = graph.num_nodes;
    if n < 2 {
        return 0.0;
    }

    let laplacian = graph_laplacian(graph);

    // The Laplacian is PSD. Its smallest eigenvalue is 0 with eigenvector
    // proportional to the all-ones vector. We need the second smallest.
    //
    // Strategy: use inverse power iteration on (L + alpha*I) shifted to find
    // the smallest eigenvalue, then deflate and find the next.
    // Alternatively, use the shifted inverse iteration directly for lambda_2.
    //
    // Simpler approach: compute L * x repeatedly to find eigenvalues from largest
    // down, or use the fact that lambda_2 = min over x perp to 1 of x^T L x / x^T x.
    //
    // We use inverse iteration with shift to find the Fiedler vector.
    // But since we don't have a linear solver, we use power iteration on
    // (max_eig * I - L) to find the largest eigenvalue of that matrix (which
    // corresponds to the smallest eigenvalue of L).
    //
    // Actually, the simplest reliable approach for moderate n:
    // Use the Rayleigh quotient iteration projected orthogonal to the all-ones vector.

    compute_fiedler_rayleigh(&laplacian, n)
}

/// Compute the spectral gap: lambda_2 - lambda_1.
///
/// Since lambda_1 = 0 for the Laplacian, the spectral gap equals the Fiedler value.
pub fn spectral_gap(graph: &BrainGraph) -> f64 {
    fiedler_value(graph)
}

/// Compute the Fiedler value using projected power iteration.
///
/// Projects out the all-ones eigenvector (corresponding to lambda_1 = 0),
/// then uses power iteration on (alpha*I - L) to find the largest eigenvalue
/// of that shifted matrix. The Fiedler value is then alpha - largest_eigenvalue.
fn compute_fiedler_rayleigh(laplacian: &[Vec<f64>], n: usize) -> f64 {
    if n < 2 {
        return 0.0;
    }

    // Estimate max eigenvalue for shifting (Gershgorin bound)
    let alpha = laplacian
        .iter()
        .map(|row| row.iter().map(|x| x.abs()).sum::<f64>())
        .fold(0.0_f64, |a, b| a.max(b))
        * 1.1;

    if alpha <= 0.0 {
        return 0.0;
    }

    // Construct M = alpha*I - L
    // The eigenvalues of M are alpha - lambda_i(L).
    // The largest eigenvalue of M corresponds to the smallest eigenvalue of L (which is 0).
    // The second largest eigenvalue of M corresponds to lambda_2 of L.
    // We need to deflate out the first eigenvector (all-ones) and do power iteration.

    // Normalized all-ones vector
    let inv_sqrt_n = 1.0 / (n as f64).sqrt();

    // Initialize random-ish vector orthogonal to all-ones
    let mut v: Vec<f64> = (0..n).map(|i| (i as f64 + 0.5).sin()).collect();

    // Project out the all-ones component
    project_out_ones(&mut v, inv_sqrt_n, n);
    normalize(&mut v);

    let max_iter = 1000;
    let tol = 1e-10;

    for _ in 0..max_iter {
        // w = M * v = (alpha*I - L) * v
        let mut w = vec![0.0; n];
        for i in 0..n {
            w[i] = alpha * v[i];
            for j in 0..n {
                w[i] -= laplacian[i][j] * v[j];
            }
        }

        // Project out the all-ones component
        project_out_ones(&mut w, inv_sqrt_n, n);

        let norm_w = norm(&w);
        if norm_w < 1e-15 {
            // The vector collapsed, Fiedler value is likely alpha
            return alpha;
        }

        // Rayleigh quotient: eigenvalue of M = v^T * w / v^T * v
        let eigenvalue_m: f64 = v.iter().zip(w.iter()).map(|(a, b)| a * b).sum::<f64>();

        // Normalize
        for x in &mut w {
            *x /= norm_w;
        }

        // Check convergence
        let diff: f64 = v
            .iter()
            .zip(w.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();

        v = w;

        if diff < tol {
            // Fiedler value = alpha - eigenvalue_of_M
            let fiedler = alpha - eigenvalue_m;
            return fiedler.max(0.0);
        }
    }

    // Final estimate
    let mut w = vec![0.0; n];
    for i in 0..n {
        w[i] = alpha * v[i];
        for j in 0..n {
            w[i] -= laplacian[i][j] * v[j];
        }
    }
    project_out_ones(&mut w, inv_sqrt_n, n);

    let eigenvalue_m: f64 = v.iter().zip(w.iter()).map(|(a, b)| a * b).sum::<f64>();
    (alpha - eigenvalue_m).max(0.0)
}

/// Project vector v orthogonal to the all-ones vector.
fn project_out_ones(v: &mut [f64], inv_sqrt_n: f64, _n: usize) {
    let dot: f64 = v.iter().sum::<f64>() * inv_sqrt_n;
    for x in v.iter_mut() {
        *x -= dot * inv_sqrt_n;
    }
}

/// L2 norm of a vector.
fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Normalize a vector in-place.
fn normalize(v: &mut [f64]) {
    let n = norm(v);
    if n > 0.0 {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_edge(s: usize, t: usize, w: f64) -> BrainEdge {
        BrainEdge {
            source: s,
            target: t,
            weight: w,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        }
    }

    fn complete_graph(n: usize) -> BrainGraph {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                edges.push(make_edge(i, j, 1.0));
            }
        }
        BrainGraph {
            num_nodes: n,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(n),
        }
    }

    #[test]
    fn laplacian_row_sums_zero() {
        let g = complete_graph(5);
        let l = graph_laplacian(&g);
        for row in &l {
            let sum: f64 = row.iter().sum();
            assert!(sum.abs() < 1e-10, "Row sum should be 0, got {}", sum);
        }
    }

    #[test]
    fn laplacian_diagonal_is_degree() {
        let g = complete_graph(5);
        let l = graph_laplacian(&g);
        // Each node in K5 has degree 4
        for i in 0..5 {
            assert!((l[i][i] - 4.0).abs() < 1e-10);
        }
    }

    #[test]
    fn normalized_laplacian_diagonal_connected() {
        let g = complete_graph(5);
        let ln = normalized_laplacian(&g);
        // For connected nodes, diagonal should be 1.0
        for i in 0..5 {
            assert!((ln[i][i] - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn fiedler_value_connected_graph() {
        let g = complete_graph(6);
        let f = fiedler_value(&g);
        // For K_n, all non-zero eigenvalues of L are n. So fiedler = n = 6.
        assert!(f > 0.0, "Connected graph should have fiedler > 0, got {}", f);
        assert!((f - 6.0).abs() < 0.5, "K6 fiedler should be ~6.0, got {}", f);
    }

    #[test]
    fn fiedler_value_disconnected_graph() {
        // Two isolated components: nodes 0,1 connected; nodes 2,3 connected; no bridge.
        let g = BrainGraph {
            num_nodes: 4,
            edges: vec![make_edge(0, 1, 1.0), make_edge(2, 3, 1.0)],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };
        let f = fiedler_value(&g);
        assert!(f < 1e-6, "Disconnected graph should have fiedler ~0, got {}", f);
    }

    #[test]
    fn spectral_gap_equals_fiedler() {
        let g = complete_graph(5);
        assert_eq!(spectral_gap(&g), fiedler_value(&g));
    }
}
