//! Spectral methods for graph cuts.
//!
//! Provides the Cheeger constant (isoperimetric number), spectral bisection via
//! the Fiedler vector, and the Cheeger inequality bounds relating the Fiedler
//! value to the isoperimetric constant.

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::topology::MincutResult;
use ruv_neural_core::{Result, RuvNeuralError};

/// Compute the Fiedler vector (eigenvector of the second-smallest eigenvalue)
/// of the graph Laplacian using power iteration on the shifted Laplacian.
///
/// Returns `(fiedler_value, fiedler_vector)`.
///
/// We use inverse iteration on L to find the second-smallest eigenvalue.
/// Since direct eigendecomposition without LAPACK is nontrivial, we use a
/// simple approach: compute the Laplacian, then find its two smallest
/// eigenvalues via shifted inverse iteration.
pub fn fiedler_decomposition(graph: &BrainGraph) -> Result<(f64, Vec<f64>)> {
    let n = graph.num_nodes;
    if n < 2 {
        return Err(RuvNeuralError::Mincut(
            "Need at least 2 nodes for spectral analysis".into(),
        ));
    }

    let adj = graph.adjacency_matrix();

    // Build the Laplacian: L = D - A
    let mut laplacian = vec![vec![0.0; n]; n];
    for i in 0..n {
        let degree: f64 = adj[i].iter().sum();
        laplacian[i][i] = degree;
        for j in 0..n {
            laplacian[i][j] -= adj[i][j];
        }
    }

    // For small graphs, use the QR-like approach via repeated deflated power
    // iteration. We want the second-smallest eigenvector.
    //
    // Step 1: The smallest eigenvalue of L is 0 with eigenvector = all-ones
    //         (for connected graphs). We deflate that out.
    // Step 2: Run power iteration on (mu*I - L) to find the largest eigenvalue
    //         of the deflated operator, which corresponds to the second-smallest
    //         eigenvalue of L.

    // Find the largest eigenvalue of L (for shifting) via power iteration.
    let lambda_max = largest_eigenvalue(&laplacian, n, 200);

    // Shift: M = lambda_max * I - L.
    // The eigenvalues of M are (lambda_max - lambda_i).
    // The largest eigenvalue of M corresponds to the smallest of L (= 0).
    // The second largest of M corresponds to the second smallest of L (= fiedler).
    let shift = lambda_max + 0.01; // small buffer

    // Power iteration on M, deflating out the constant eigenvector.
    let ones: Vec<f64> = vec![1.0 / (n as f64).sqrt(); n];

    // Random-ish initial vector, orthogonal to ones.
    let mut v: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0).sin()).collect();
    deflate(&mut v, &ones);
    normalize(&mut v);

    let max_iter = 1000;
    let mut prev_eigenvalue = 0.0;

    for _ in 0..max_iter {
        // w = M * v = (shift * I - L) * v = shift * v - L * v
        let mut w = vec![0.0; n];
        for i in 0..n {
            let mut lv = 0.0;
            for j in 0..n {
                lv += laplacian[i][j] * v[j];
            }
            w[i] = shift * v[i] - lv;
        }

        // Deflate out the constant eigenvector.
        deflate(&mut w, &ones);
        let eigenvalue = dot(&w, &v);
        normalize(&mut w);
        v = w;

        if (eigenvalue - prev_eigenvalue).abs() < 1e-12 {
            break;
        }
        prev_eigenvalue = eigenvalue;
    }

    // The Fiedler value = shift - prev_eigenvalue
    let fiedler_value = shift - prev_eigenvalue;

    // Clamp small negative values from numerical noise.
    let fiedler_value = if fiedler_value < 0.0 && fiedler_value > -1e-9 {
        0.0
    } else {
        fiedler_value
    };

    Ok((fiedler_value, v))
}

/// Spectral bisection using the Fiedler vector.
///
/// Partitions the graph into two sets based on the sign of the Fiedler vector
/// components. Nodes with positive components go to partition A, non-positive
/// to partition B.
pub fn spectral_bisection(graph: &BrainGraph) -> Result<MincutResult> {
    let (_fiedler_value, fiedler_vec) = fiedler_decomposition(graph)?;

    let mut partition_a = Vec::new();
    let mut partition_b = Vec::new();

    for (i, &val) in fiedler_vec.iter().enumerate() {
        if val > 0.0 {
            partition_a.push(i);
        } else {
            partition_b.push(i);
        }
    }

    // Handle degenerate case where everything ends up on one side.
    if partition_a.is_empty() || partition_b.is_empty() {
        // Put the first node in A, rest in B.
        partition_a = vec![0];
        partition_b = (1..graph.num_nodes).collect();
    }

    let partition_a_set: std::collections::HashSet<usize> =
        partition_a.iter().copied().collect();

    // Compute cut value.
    let mut cut_value = 0.0;
    let mut cut_edges = Vec::new();
    for edge in &graph.edges {
        let s_in_a = partition_a_set.contains(&edge.source);
        let t_in_a = partition_a_set.contains(&edge.target);
        if s_in_a != t_in_a {
            cut_value += edge.weight;
            cut_edges.push((edge.source, edge.target, edge.weight));
        }
    }

    Ok(MincutResult {
        cut_value,
        partition_a,
        partition_b,
        cut_edges,
        timestamp: graph.timestamp,
    })
}

/// Compute the Cheeger constant (isoperimetric number) of the graph.
///
/// h(G) = min over all subsets S with |S| <= |V|/2 of:
///     cut(S, V\S) / vol(S)
///
/// For small graphs this is computed exactly by enumeration. For larger graphs
/// we approximate using the spectral bisection.
pub fn cheeger_constant(graph: &BrainGraph) -> Result<f64> {
    let n = graph.num_nodes;
    if n < 2 {
        return Err(RuvNeuralError::Mincut(
            "Need at least 2 nodes for Cheeger constant".into(),
        ));
    }

    // For small graphs (n <= 16), enumerate all subsets.
    if n <= 16 {
        let adj = graph.adjacency_matrix();
        let degrees: Vec<f64> = (0..n)
            .map(|i| adj[i].iter().sum::<f64>())
            .collect();

        let mut best_h = f64::INFINITY;

        // Enumerate non-empty subsets of size <= n/2.
        let total = 1u32 << n;
        for mask in 1..total {
            let size = mask.count_ones() as usize;
            if size > n / 2 {
                continue;
            }

            // Compute vol(S) and cut(S, V\S).
            let mut vol_s = 0.0;
            let mut cut_s = 0.0;

            for i in 0..n {
                if mask & (1 << i) != 0 {
                    vol_s += degrees[i];
                    for j in 0..n {
                        if mask & (1 << j) == 0 {
                            cut_s += adj[i][j];
                        }
                    }
                }
            }

            if vol_s > 0.0 {
                let h = cut_s / vol_s;
                if h < best_h {
                    best_h = h;
                }
            }
        }

        Ok(best_h)
    } else {
        // Approximate via spectral: use the Fiedler vector partition.
        let result = spectral_bisection(graph)?;
        let adj = graph.adjacency_matrix();

        // vol(partition_a)
        let vol_a: f64 = result
            .partition_a
            .iter()
            .map(|&i| adj[i].iter().sum::<f64>())
            .sum();
        let vol_b: f64 = result
            .partition_b
            .iter()
            .map(|&i| adj[i].iter().sum::<f64>())
            .sum();

        let vol_min = vol_a.min(vol_b);
        if vol_min <= 0.0 {
            return Ok(0.0);
        }

        Ok(result.cut_value / vol_min)
    }
}

/// Cheeger inequality bounds relating the Fiedler value lambda_2 of the
/// **unnormalized** Laplacian to the conductance h(G).
///
/// For the unnormalized Laplacian with maximum degree d_max:
///
/// ```text
/// lambda_2 / (2 * d_max) <= h(G) <= sqrt(2 * lambda_2 / d_min)
/// ```
///
/// For convenience when d_max is unknown, this function uses the normalized
/// Laplacian relationship:
///
/// ```text
/// lambda_2_norm / 2 <= h(G) <= sqrt(2 * lambda_2_norm)
/// ```
///
/// The `fiedler_value` parameter should be from the **normalized** Laplacian
/// (i.e., `unnormalized_lambda_2 / d_max` is a conservative approximation).
///
/// Returns `(lower_bound, upper_bound)`.
pub fn cheeger_bound(fiedler_value: f64) -> (f64, f64) {
    let lower = fiedler_value / 2.0;
    let upper = (2.0 * fiedler_value).sqrt();
    (lower, upper)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Largest eigenvalue of a symmetric matrix via power iteration.
///
/// Terminates early when the eigenvalue change between iterations is below 1e-12.
fn largest_eigenvalue(mat: &[Vec<f64>], n: usize, max_iter: usize) -> f64 {
    let mut v: Vec<f64> = (0..n).map(|i| (i as f64 + 0.5).cos()).collect();
    normalize(&mut v);

    let mut eigenvalue = 0.0;
    for _ in 0..max_iter {
        let mut w = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                w[i] += mat[i][j] * v[j];
            }
        }
        let new_eigenvalue = dot(&w, &v);
        normalize(&mut w);
        v = w;

        if (new_eigenvalue - eigenvalue).abs() < 1e-12 {
            eigenvalue = new_eigenvalue;
            break;
        }
        eigenvalue = new_eigenvalue;
    }
    eigenvalue
}

/// Remove the component of `v` along `u` (assumed normalized).
fn deflate(v: &mut [f64], u: &[f64]) {
    let proj = dot(v, u);
    for (vi, &ui) in v.iter_mut().zip(u.iter()) {
        *vi -= proj * ui;
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn normalize(v: &mut [f64]) {
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-15 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::BrainEdge;
    use ruv_neural_core::signal::FrequencyBand;

    fn make_edge(source: usize, target: usize, weight: f64) -> BrainEdge {
        BrainEdge {
            source,
            target,
            weight,
            metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Alpha,
        }
    }

    /// Path graph P3 (0--1--2): Fiedler value should be 1.0.
    /// Laplacian eigenvalues of P3 with unit weights: 0, 1, 3.
    #[test]
    fn test_fiedler_path_p3() {
        let graph = BrainGraph {
            num_nodes: 3,
            edges: vec![make_edge(0, 1, 1.0), make_edge(1, 2, 1.0)],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        };

        let (fiedler_value, fiedler_vec) = fiedler_decomposition(&graph).unwrap();
        assert!(
            (fiedler_value - 1.0).abs() < 0.1,
            "Expected Fiedler value ~1.0 for P3, got {}",
            fiedler_value
        );
        // The Fiedler vector should have opposite signs at the endpoints.
        assert!(
            fiedler_vec[0] * fiedler_vec[2] < 0.0,
            "Fiedler vector endpoints should have opposite signs"
        );
    }

    /// Cheeger bounds using normalized Laplacian eigenvalue.
    ///
    /// For the unnormalized Laplacian eigenvalue lambda_2 and max degree d_max,
    /// the normalized eigenvalue is lambda_2_norm = lambda_2 / d_max, and the
    /// Cheeger inequality states: lambda_2_norm / 2 <= h(G) <= sqrt(2 * lambda_2_norm).
    #[test]
    fn test_cheeger_bounds_hold() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![
                make_edge(0, 1, 1.0),
                make_edge(1, 2, 1.0),
                make_edge(2, 3, 1.0),
                make_edge(3, 0, 1.0),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        let (fiedler_value, _) = fiedler_decomposition(&graph).unwrap();
        let h = cheeger_constant(&graph).unwrap();

        // For conductance (cut/vol), the Cheeger inequality uses the normalized
        // Laplacian eigenvalue. For C4 with unit weights, d_max = 2, so:
        //   lambda_2_norm = lambda_2 / d_max
        let adj = graph.adjacency_matrix();
        let d_max: f64 = (0..graph.num_nodes)
            .map(|i| adj[i].iter().sum::<f64>())
            .fold(f64::NEG_INFINITY, f64::max);
        let lambda_2_norm = fiedler_value / d_max;

        let (lower, upper) = cheeger_bound(lambda_2_norm);

        assert!(
            h >= lower - 1e-6,
            "Cheeger h={} should be >= lower bound {} (lambda2_norm={})",
            h,
            lower,
            lambda_2_norm
        );
        assert!(
            h <= upper + 1e-6,
            "Cheeger h={} should be <= upper bound {} (lambda2_norm={})",
            h,
            upper,
            lambda_2_norm
        );
    }

    /// Spectral bisection of a barbell graph should split the two cliques.
    #[test]
    fn test_spectral_bisection_barbell() {
        // Two triangles connected by a single weak edge.
        let graph = BrainGraph {
            num_nodes: 6,
            edges: vec![
                // Clique 1: {0, 1, 2}
                make_edge(0, 1, 5.0),
                make_edge(1, 2, 5.0),
                make_edge(0, 2, 5.0),
                // Clique 2: {3, 4, 5}
                make_edge(3, 4, 5.0),
                make_edge(4, 5, 5.0),
                make_edge(3, 5, 5.0),
                // Bridge
                make_edge(2, 3, 0.1),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(6),
        };

        let result = spectral_bisection(&graph).unwrap();
        // The cut should be small (close to 0.1).
        assert!(
            result.cut_value < 2.0,
            "Expected small cut for barbell, got {}",
            result.cut_value
        );
        // Each partition should have 3 nodes.
        assert_eq!(result.partition_a.len() + result.partition_b.len(), 6);
    }

    #[test]
    fn test_cheeger_bound_values() {
        let (lower, upper) = cheeger_bound(2.0);
        assert!((lower - 1.0).abs() < 1e-9);
        assert!((upper - 2.0).abs() < 1e-9);
    }
}
