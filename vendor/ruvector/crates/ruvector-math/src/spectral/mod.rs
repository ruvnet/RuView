//! Spectral Methods for Graph Analysis
//!
//! Chebyshev polynomials and spectral graph theory for efficient
//! diffusion and filtering without eigendecomposition.
//!
//! ## Key Capabilities
//!
//! - **Chebyshev Graph Filtering**: O(Km) filtering where K is polynomial degree
//! - **Graph Diffusion**: Heat kernel approximation via Chebyshev expansion
//! - **Spectral Clustering**: Efficient k-way partitioning
//! - **Wavelet Transforms**: Multi-scale graph analysis
//!
//! ## Integration with Mincut
//!
//! Spectral methods pair naturally with mincut:
//! - Mincut identifies partition boundaries
//! - Chebyshev smooths attention within partitions
//! - Spectral clustering provides initial segmentation hints
//!
//! ## Mathematical Background
//!
//! Chebyshev polynomials T_k(x) satisfy:
//! - T_0(x) = 1
//! - T_1(x) = x
//! - T_{k+1}(x) = 2x·T_k(x) - T_{k-1}(x)
//!
//! This recurrence enables O(K) evaluation of degree-K polynomial filters.

mod chebyshev;
mod clustering;
mod graph_filter;
mod wavelets;

pub use chebyshev::{ChebyshevExpansion, ChebyshevPolynomial};
pub use clustering::{ClusteringConfig, SpectralClustering};
pub use graph_filter::{FilterType, GraphFilter, SpectralFilter};
pub use wavelets::{GraphWavelet, SpectralWaveletTransform, WaveletScale};

/// Scaled Laplacian for Chebyshev approximation
/// L_scaled = 2L/λ_max - I (eigenvalues in [-1, 1])
#[derive(Debug, Clone)]
pub struct ScaledLaplacian {
    /// Sparse representation: (row, col, value)
    pub entries: Vec<(usize, usize, f64)>,
    /// Matrix dimension
    pub n: usize,
    /// Estimated maximum eigenvalue
    pub lambda_max: f64,
}

impl ScaledLaplacian {
    /// Build from adjacency matrix (dense)
    pub fn from_adjacency(adj: &[f64], n: usize) -> Self {
        // Compute degree and Laplacian
        let mut degrees = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                degrees[i] += adj[i * n + j];
            }
        }

        // Build sparse Laplacian entries
        let mut entries = Vec::new();
        for i in 0..n {
            // Diagonal: degree
            if degrees[i] > 0.0 {
                entries.push((i, i, degrees[i]));
            }
            // Off-diagonal: -adjacency
            for j in 0..n {
                if i != j && adj[i * n + j] != 0.0 {
                    entries.push((i, j, -adj[i * n + j]));
                }
            }
        }

        // Estimate λ_max via power iteration
        let lambda_max = Self::estimate_lambda_max(&entries, n, 20);

        // Scale to [-1, 1]: L_scaled = 2L/λ_max - I
        let scale = 2.0 / lambda_max;
        let scaled_entries: Vec<(usize, usize, f64)> = entries
            .iter()
            .map(|&(i, j, v)| {
                if i == j {
                    (i, j, scale * v - 1.0)
                } else {
                    (i, j, scale * v)
                }
            })
            .collect();

        Self {
            entries: scaled_entries,
            n,
            lambda_max,
        }
    }

    /// Build from sparse adjacency list
    pub fn from_sparse_adjacency(edges: &[(usize, usize, f64)], n: usize) -> Self {
        // Compute degrees
        let mut degrees = vec![0.0; n];
        for &(i, j, w) in edges {
            degrees[i] += w;
            if i != j {
                degrees[j] += w; // Symmetric
            }
        }

        // Build Laplacian entries
        let mut entries = Vec::new();
        for i in 0..n {
            if degrees[i] > 0.0 {
                entries.push((i, i, degrees[i]));
            }
        }
        for &(i, j, w) in edges {
            if w != 0.0 {
                entries.push((i, j, -w));
                if i != j {
                    entries.push((j, i, -w));
                }
            }
        }

        let lambda_max = Self::estimate_lambda_max(&entries, n, 20);
        let scale = 2.0 / lambda_max;

        let scaled_entries: Vec<(usize, usize, f64)> = entries
            .iter()
            .map(|&(i, j, v)| {
                if i == j {
                    (i, j, scale * v - 1.0)
                } else {
                    (i, j, scale * v)
                }
            })
            .collect();

        Self {
            entries: scaled_entries,
            n,
            lambda_max,
        }
    }

    /// Estimate maximum eigenvalue via power iteration
    fn estimate_lambda_max(entries: &[(usize, usize, f64)], n: usize, iters: usize) -> f64 {
        let mut x = vec![1.0 / (n as f64).sqrt(); n];
        let mut lambda = 1.0;

        for _ in 0..iters {
            // y = L * x
            let mut y = vec![0.0; n];
            for &(i, j, v) in entries {
                y[i] += v * x[j];
            }

            // Estimate eigenvalue
            let mut dot = 0.0;
            let mut norm_sq = 0.0;
            for i in 0..n {
                dot += x[i] * y[i];
                norm_sq += y[i] * y[i];
            }
            lambda = dot;

            // Normalize
            let norm = norm_sq.sqrt().max(1e-15);
            for i in 0..n {
                x[i] = y[i] / norm;
            }
        }

        lambda.abs().max(1.0)
    }

    /// Apply scaled Laplacian to vector: y = L_scaled * x
    pub fn apply(&self, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; self.n];
        for &(i, j, v) in &self.entries {
            if j < x.len() {
                y[i] += v * x[j];
            }
        }
        y
    }

    /// Get original (unscaled) maximum eigenvalue estimate
    pub fn lambda_max(&self) -> f64 {
        self.lambda_max
    }
}

/// Normalized Laplacian (symmetric or random walk)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LaplacianNorm {
    /// Unnormalized: L = D - A
    Unnormalized,
    /// Symmetric: L_sym = D^{-1/2} L D^{-1/2}
    Symmetric,
    /// Random walk: L_rw = D^{-1} L
    RandomWalk,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaled_laplacian() {
        // Simple 3-node path graph: 0 -- 1 -- 2
        let adj = vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];

        let laplacian = ScaledLaplacian::from_adjacency(&adj, 3);

        assert_eq!(laplacian.n, 3);
        assert!(laplacian.lambda_max > 0.0);

        // Apply to vector
        let x = vec![1.0, 0.0, -1.0];
        let y = laplacian.apply(&x);
        assert_eq!(y.len(), 3);
    }

    #[test]
    fn test_sparse_laplacian() {
        // Same path graph as sparse edges
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0)];
        let laplacian = ScaledLaplacian::from_sparse_adjacency(&edges, 3);

        assert_eq!(laplacian.n, 3);
        assert!(laplacian.lambda_max > 0.0);
    }
}
