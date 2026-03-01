//! Graph Laplacian
//!
//! Constructs various Laplacian matrices from key similarities.

use serde::{Deserialize, Serialize};

/// Type of Laplacian to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaplacianType {
    /// Unnormalized: L = D - W
    Unnormalized,
    /// Symmetric normalized: L = I - D^{-1/2} W D^{-1/2}
    SymmetricNormalized,
    /// Random walk: L = I - D^{-1} W
    RandomWalk,
}

/// Graph Laplacian for attention
#[derive(Debug, Clone)]
pub struct GraphLaplacian {
    /// Weight matrix (dense)
    weights: Vec<f32>,
    /// Degree vector
    degrees: Vec<f32>,
    /// Number of nodes
    n: usize,
    /// Laplacian type
    lap_type: LaplacianType,
}

impl GraphLaplacian {
    /// Build Laplacian from keys using Gaussian kernel
    pub fn from_keys(keys: &[&[f32]], sigma: f32, lap_type: LaplacianType) -> Self {
        let n = keys.len();
        let sigma2 = (sigma * sigma).max(1e-9);

        let mut weights = vec![0.0f32; n * n];
        let mut degrees = vec![0.0f32; n];

        // Build weight matrix
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }

                let dist2 = Self::l2_sq(keys[i], keys[j]);
                let w = (-dist2 / (2.0 * sigma2)).exp();

                weights[i * n + j] = w;
                degrees[i] += w;
            }
        }

        Self {
            weights,
            degrees,
            n,
            lap_type,
        }
    }

    /// Build sparse Laplacian using k-NN
    pub fn from_keys_knn(keys: &[&[f32]], k: usize, sigma: f32, lap_type: LaplacianType) -> Self {
        let n = keys.len();
        // Security: prevent integer underflow when n=0 or n=1
        let k = if n > 1 { k.min(n - 1) } else { 0 };
        let sigma2 = (sigma * sigma).max(1e-9);

        let mut weights = vec![0.0f32; n * n];
        let mut degrees = vec![0.0f32; n];

        // For each node, find k-NN
        for i in 0..n {
            let mut dists: Vec<(usize, f32)> = (0..n)
                .filter(|&j| j != i)
                .map(|j| (j, Self::l2_sq(keys[i], keys[j])))
                .collect();

            dists.sort_unstable_by(|a, b| {
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            });

            // Keep only k nearest
            for (j, dist2) in dists.iter().take(k) {
                let w = (-dist2 / (2.0 * sigma2)).exp();
                weights[i * n + j] = w;
                weights[*j * n + i] = w; // Make symmetric
            }
        }

        // Recompute degrees
        for i in 0..n {
            for j in 0..n {
                degrees[i] += weights[i * n + j];
            }
        }

        Self {
            weights,
            degrees,
            n,
            lap_type,
        }
    }

    /// Apply Laplacian to vector: L * x
    pub fn apply(&self, x: &[f32]) -> Vec<f32> {
        let mut result = vec![0.0f32; self.n];

        match self.lap_type {
            LaplacianType::Unnormalized => {
                // L * x = D * x - W * x
                for i in 0..self.n {
                    result[i] = self.degrees[i] * x[i];
                    for j in 0..self.n {
                        result[i] -= self.weights[i * self.n + j] * x[j];
                    }
                }
            }
            LaplacianType::SymmetricNormalized => {
                // L * x = x - D^{-1/2} W D^{-1/2} x
                let d_inv_sqrt: Vec<f32> = self
                    .degrees
                    .iter()
                    .map(|&d| if d > 0.0 { 1.0 / d.sqrt() } else { 0.0 })
                    .collect();

                for i in 0..self.n {
                    result[i] = x[i];
                    for j in 0..self.n {
                        let w_norm = d_inv_sqrt[i] * self.weights[i * self.n + j] * d_inv_sqrt[j];
                        result[i] -= w_norm * x[j];
                    }
                }
            }
            LaplacianType::RandomWalk => {
                // L * x = x - D^{-1} W * x
                for i in 0..self.n {
                    result[i] = x[i];
                    let d_inv = if self.degrees[i] > 0.0 {
                        1.0 / self.degrees[i]
                    } else {
                        0.0
                    };
                    for j in 0..self.n {
                        result[i] -= d_inv * self.weights[i * self.n + j] * x[j];
                    }
                }
            }
        }

        result
    }

    /// Get number of nodes
    pub fn num_nodes(&self) -> usize {
        self.n
    }

    /// Get degree of node i
    pub fn degree(&self, i: usize) -> f32 {
        self.degrees.get(i).copied().unwrap_or(0.0)
    }

    /// Get weight between nodes i and j
    pub fn weight(&self, i: usize, j: usize) -> f32 {
        if i < self.n && j < self.n {
            self.weights[i * self.n + j]
        } else {
            0.0
        }
    }

    /// L2 squared distance
    #[inline]
    fn l2_sq(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        let mut sum = 0.0f32;
        for i in 0..len {
            let d = a[i] - b[i];
            sum += d * d;
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_laplacian_build() {
        let keys: Vec<Vec<f32>> = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let lap = GraphLaplacian::from_keys(&keys_refs, 1.0, LaplacianType::Unnormalized);

        assert_eq!(lap.num_nodes(), 3);
        assert!(lap.degree(0) > 0.0);
    }

    #[test]
    fn test_laplacian_apply() {
        let keys: Vec<Vec<f32>> = vec![vec![0.0], vec![1.0], vec![2.0]];
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let lap = GraphLaplacian::from_keys(&keys_refs, 1.0, LaplacianType::Unnormalized);

        // Constant vector should give zero (L * 1 = 0)
        let x = vec![1.0, 1.0, 1.0];
        let lx = lap.apply(&x);

        let sum: f32 = lx.iter().map(|v| v.abs()).sum();
        assert!(sum < 1e-3);
    }

    #[test]
    fn test_knn_laplacian() {
        let keys: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32]).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let lap = GraphLaplacian::from_keys_knn(&keys_refs, 3, 1.0, LaplacianType::RandomWalk);

        assert_eq!(lap.num_nodes(), 10);
    }
}
