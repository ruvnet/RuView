//! Tropical Matrices
//!
//! Matrix operations in the tropical semiring.
//! Applications:
//! - Shortest path algorithms (Floyd-Warshall)
//! - Scheduling optimization
//! - Graph eigenvalue problems

use super::semiring::{Tropical, TropicalMin};

/// Tropical matrix (max-plus)
#[derive(Debug, Clone)]
pub struct TropicalMatrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl TropicalMatrix {
    /// Create zero matrix (all -∞)
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![f64::NEG_INFINITY; rows * cols],
        }
    }

    /// Create identity matrix (0 on diagonal, -∞ elsewhere)
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, 0.0);
        }
        m
    }

    /// Create from 2D data
    pub fn from_rows(data: Vec<Vec<f64>>) -> Self {
        let rows = data.len();
        let cols = if rows > 0 { data[0].len() } else { 0 };
        let flat: Vec<f64> = data.into_iter().flatten().collect();
        Self {
            rows,
            cols,
            data: flat,
        }
    }

    /// Get element (returns -∞ for out of bounds)
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        if i >= self.rows || j >= self.cols {
            return f64::NEG_INFINITY;
        }
        self.data[i * self.cols + j]
    }

    /// Set element (no-op for out of bounds)
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, val: f64) {
        if i >= self.rows || j >= self.cols {
            return;
        }
        self.data[i * self.cols + j] = val;
    }

    /// Matrix dimensions
    pub fn dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Tropical matrix multiplication: C[i,k] = max_j(A[i,j] + B[j,k])
    pub fn mul(&self, other: &Self) -> Self {
        assert_eq!(self.cols, other.rows, "Dimension mismatch");

        let mut result = Self::zeros(self.rows, other.cols);

        for i in 0..self.rows {
            for k in 0..other.cols {
                let mut max_val = f64::NEG_INFINITY;
                for j in 0..self.cols {
                    let a = self.get(i, j);
                    let b = other.get(j, k);

                    if a != f64::NEG_INFINITY && b != f64::NEG_INFINITY {
                        max_val = max_val.max(a + b);
                    }
                }
                result.set(i, k, max_val);
            }
        }

        result
    }

    /// Tropical matrix power: A^n (n tropical multiplications)
    pub fn pow(&self, n: usize) -> Self {
        assert_eq!(self.rows, self.cols, "Must be square");

        if n == 0 {
            return Self::identity(self.rows);
        }

        let mut result = self.clone();
        for _ in 1..n {
            result = result.mul(self);
        }
        result
    }

    /// Tropical matrix closure: A* = I ⊕ A ⊕ A² ⊕ ... ⊕ A^n
    /// Computes all shortest paths (min-plus version is Floyd-Warshall)
    pub fn closure(&self) -> Self {
        assert_eq!(self.rows, self.cols, "Must be square");
        let n = self.rows;

        let mut result = Self::identity(n);
        let mut power = self.clone();

        for _ in 0..n {
            // result = result ⊕ power
            for i in 0..n {
                for j in 0..n {
                    let old = result.get(i, j);
                    let new = power.get(i, j);
                    result.set(i, j, old.max(new));
                }
            }
            power = power.mul(self);
        }

        result
    }

    /// Find tropical eigenvalue (max cycle mean)
    /// Returns the maximum average weight of any cycle
    pub fn max_cycle_mean(&self) -> f64 {
        assert_eq!(self.rows, self.cols, "Must be square");
        let n = self.rows;

        // Karp's algorithm for maximum cycle mean
        let mut d = vec![vec![f64::NEG_INFINITY; n + 1]; n];

        // Initialize d[i][0] = 0 for all i
        for i in 0..n {
            d[i][0] = 0.0;
        }

        // Dynamic programming
        for k in 1..=n {
            for i in 0..n {
                for j in 0..n {
                    let w = self.get(i, j);
                    if w != f64::NEG_INFINITY && d[j][k - 1] != f64::NEG_INFINITY {
                        d[i][k] = d[i][k].max(w + d[j][k - 1]);
                    }
                }
            }
        }

        // Compute max cycle mean
        let mut lambda = f64::NEG_INFINITY;
        for i in 0..n {
            if d[i][n] != f64::NEG_INFINITY {
                let mut min_ratio = f64::INFINITY;
                for k in 0..n {
                    // Security: prevent division by zero when k == n
                    if k < n && d[i][k] != f64::NEG_INFINITY {
                        let divisor = (n - k) as f64;
                        if divisor > 0.0 {
                            let ratio = (d[i][n] - d[i][k]) / divisor;
                            min_ratio = min_ratio.min(ratio);
                        }
                    }
                }
                lambda = lambda.max(min_ratio);
            }
        }

        lambda
    }
}

/// Tropical eigenvalue and eigenvector
#[derive(Debug, Clone)]
pub struct TropicalEigen {
    /// Eigenvalue (cycle mean)
    pub eigenvalue: f64,
    /// Eigenvector
    pub eigenvector: Vec<f64>,
}

impl TropicalEigen {
    /// Compute tropical eigenpair using power iteration
    /// Finds λ and v such that A ⊗ v = λ ⊗ v (i.e., max_j(A[i,j] + v[j]) = λ + v[i])
    pub fn power_iteration(matrix: &TropicalMatrix, max_iters: usize) -> Option<Self> {
        let n = matrix.rows;
        if n == 0 {
            return None;
        }

        // Start with uniform vector
        let mut v: Vec<f64> = vec![0.0; n];
        let mut eigenvalue = 0.0f64;

        for _ in 0..max_iters {
            // Compute A ⊗ v
            let mut av = vec![f64::NEG_INFINITY; n];
            for i in 0..n {
                for j in 0..n {
                    let aij = matrix.get(i, j);
                    if aij != f64::NEG_INFINITY && v[j] != f64::NEG_INFINITY {
                        av[i] = av[i].max(aij + v[j]);
                    }
                }
            }

            // Find max to normalize
            let max_av = av.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if max_av == f64::NEG_INFINITY {
                return None;
            }

            // Eigenvalue = growth rate
            let new_eigenvalue = max_av - v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            // Normalize: v = av - max(av)
            for i in 0..n {
                v[i] = av[i] - max_av;
            }

            // Check convergence
            if (new_eigenvalue - eigenvalue).abs() < 1e-10 {
                return Some(TropicalEigen {
                    eigenvalue: new_eigenvalue,
                    eigenvector: v,
                });
            }

            eigenvalue = new_eigenvalue;
        }

        Some(TropicalEigen {
            eigenvalue,
            eigenvector: v,
        })
    }
}

/// Min-plus matrix for shortest paths
#[derive(Debug, Clone)]
pub struct MinPlusMatrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl MinPlusMatrix {
    /// Create from adjacency weights (+∞ for no edge)
    pub fn from_adjacency(adj: Vec<Vec<f64>>) -> Self {
        let rows = adj.len();
        let cols = if rows > 0 { adj[0].len() } else { 0 };
        let data: Vec<f64> = adj.into_iter().flatten().collect();
        Self { rows, cols, data }
    }

    /// Get element (returns +∞ for out of bounds)
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        if i >= self.rows || j >= self.cols {
            return f64::INFINITY;
        }
        self.data[i * self.cols + j]
    }

    /// Set element (no-op for out of bounds)
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, val: f64) {
        if i >= self.rows || j >= self.cols {
            return;
        }
        self.data[i * self.cols + j] = val;
    }

    /// Floyd-Warshall all-pairs shortest paths (min-plus closure)
    pub fn all_pairs_shortest_paths(&self) -> Self {
        let n = self.rows;
        let mut dist = self.clone();

        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    let via_k = dist.get(i, k) + dist.get(k, j);
                    if via_k < dist.get(i, j) {
                        dist.set(i, j, via_k);
                    }
                }
            }
        }

        dist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tropical_matrix_mul() {
        // A = [[0, 1], [-∞, 2]]
        let a = TropicalMatrix::from_rows(vec![vec![0.0, 1.0], vec![f64::NEG_INFINITY, 2.0]]);

        // A² = [[max(0+0, 1-∞), max(0+1, 1+2)], ...]
        let a2 = a.mul(&a);

        assert!((a2.get(0, 1) - 3.0).abs() < 1e-10); // max(0+1, 1+2) = 3
    }

    #[test]
    fn test_tropical_identity() {
        let i = TropicalMatrix::identity(3);
        let a = TropicalMatrix::from_rows(vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ]);

        let ia = i.mul(&a);
        for row in 0..3 {
            for col in 0..3 {
                assert!((ia.get(row, col) - a.get(row, col)).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_max_cycle_mean() {
        // Simple cycle: 0 -> 1 (weight 3), 1 -> 0 (weight 1)
        // Cycle mean = (3 + 1) / 2 = 2
        let a = TropicalMatrix::from_rows(vec![
            vec![f64::NEG_INFINITY, 3.0],
            vec![1.0, f64::NEG_INFINITY],
        ]);

        let mcm = a.max_cycle_mean();
        assert!((mcm - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_floyd_warshall() {
        // Graph: 0 -1-> 1 -2-> 2, 0 -5-> 2
        let adj = MinPlusMatrix::from_adjacency(vec![
            vec![0.0, 1.0, 5.0],
            vec![f64::INFINITY, 0.0, 2.0],
            vec![f64::INFINITY, f64::INFINITY, 0.0],
        ]);

        let dist = adj.all_pairs_shortest_paths();

        // Shortest 0->2 is via 1: 1 + 2 = 3
        assert!((dist.get(0, 2) - 3.0).abs() < 1e-10);
    }
}
