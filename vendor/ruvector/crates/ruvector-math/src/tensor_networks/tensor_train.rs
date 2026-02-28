//! Tensor Train (TT) Decomposition
//!
//! The Tensor Train format represents a d-dimensional tensor as:
//!
//! A[i1, i2, ..., id] = G1[i1] × G2[i2] × ... × Gd[id]
//!
//! where each Gk[ik] is an (rk-1 × rk) matrix, called a TT-core.
//! The ranks r0 = rd = 1, so the result is a scalar.
//!
//! ## Complexity
//!
//! - Storage: O(d * n * r²) instead of O(n^d)
//! - Dot product: O(d * r²)
//! - Addition: O(d * n * r²) with rank doubling

use super::DenseTensor;

/// Tensor Train configuration
#[derive(Debug, Clone)]
pub struct TensorTrainConfig {
    /// Maximum rank (0 = no limit)
    pub max_rank: usize,
    /// Truncation tolerance
    pub tolerance: f64,
}

impl Default for TensorTrainConfig {
    fn default() -> Self {
        Self {
            max_rank: 0,
            tolerance: 1e-12,
        }
    }
}

/// A single TT-core: 3D tensor of shape (rank_left, mode_size, rank_right)
#[derive(Debug, Clone)]
pub struct TTCore {
    /// Core data in row-major order: [rank_left, mode_size, rank_right]
    pub data: Vec<f64>,
    /// Left rank
    pub rank_left: usize,
    /// Mode size
    pub mode_size: usize,
    /// Right rank
    pub rank_right: usize,
}

impl TTCore {
    /// Create new TT-core
    pub fn new(data: Vec<f64>, rank_left: usize, mode_size: usize, rank_right: usize) -> Self {
        assert_eq!(data.len(), rank_left * mode_size * rank_right);
        Self {
            data,
            rank_left,
            mode_size,
            rank_right,
        }
    }

    /// Create zeros core
    pub fn zeros(rank_left: usize, mode_size: usize, rank_right: usize) -> Self {
        Self {
            data: vec![0.0; rank_left * mode_size * rank_right],
            rank_left,
            mode_size,
            rank_right,
        }
    }

    /// Get the (r_l × r_r) matrix for index i
    pub fn get_matrix(&self, i: usize) -> Vec<f64> {
        let start = i * self.rank_left * self.rank_right;
        let end = start + self.rank_left * self.rank_right;

        // Reshape from [rank_left, mode_size, rank_right] layout
        // to get the i-th slice
        let mut result = vec![0.0; self.rank_left * self.rank_right];
        for rl in 0..self.rank_left {
            for rr in 0..self.rank_right {
                let idx = rl * self.mode_size * self.rank_right + i * self.rank_right + rr;
                result[rl * self.rank_right + rr] = self.data[idx];
            }
        }
        result
    }

    /// Set element at (rank_left, mode, rank_right) position
    pub fn set(&mut self, rl: usize, i: usize, rr: usize, value: f64) {
        let idx = rl * self.mode_size * self.rank_right + i * self.rank_right + rr;
        self.data[idx] = value;
    }

    /// Get element at (rank_left, mode, rank_right) position
    pub fn get(&self, rl: usize, i: usize, rr: usize) -> f64 {
        let idx = rl * self.mode_size * self.rank_right + i * self.rank_right + rr;
        self.data[idx]
    }
}

/// Tensor Train representation
#[derive(Debug, Clone)]
pub struct TensorTrain {
    /// TT-cores
    pub cores: Vec<TTCore>,
    /// Original tensor shape
    pub shape: Vec<usize>,
    /// TT-ranks: [1, r1, r2, ..., r_{d-1}, 1]
    pub ranks: Vec<usize>,
}

impl TensorTrain {
    /// Create TT from cores
    pub fn from_cores(cores: Vec<TTCore>) -> Self {
        let shape: Vec<usize> = cores.iter().map(|c| c.mode_size).collect();
        let mut ranks = vec![1];
        for core in &cores {
            ranks.push(core.rank_right);
        }

        Self {
            cores,
            shape,
            ranks,
        }
    }

    /// Create rank-1 TT from vectors
    pub fn from_vectors(vectors: Vec<Vec<f64>>) -> Self {
        let cores: Vec<TTCore> = vectors
            .into_iter()
            .map(|v| {
                let n = v.len();
                TTCore::new(v, 1, n, 1)
            })
            .collect();

        Self::from_cores(cores)
    }

    /// Tensor order
    pub fn order(&self) -> usize {
        self.shape.len()
    }

    /// Maximum TT-rank
    pub fn max_rank(&self) -> usize {
        self.ranks.iter().cloned().max().unwrap_or(1)
    }

    /// Total storage
    pub fn storage(&self) -> usize {
        self.cores.iter().map(|c| c.data.len()).sum()
    }

    /// Evaluate TT at a multi-index
    pub fn eval(&self, indices: &[usize]) -> f64 {
        assert_eq!(indices.len(), self.order());

        // Start with 1x1 "matrix"
        let mut result = vec![1.0];
        let mut current_size = 1;

        for (k, &idx) in indices.iter().enumerate() {
            let core = &self.cores[k];
            let new_size = core.rank_right;
            let mut new_result = vec![0.0; new_size];

            // Matrix-vector product
            for rr in 0..new_size {
                for rl in 0..current_size {
                    new_result[rr] += result[rl] * core.get(rl, idx, rr);
                }
            }

            result = new_result;
            current_size = new_size;
        }

        result[0]
    }

    /// Convert to dense tensor
    pub fn to_dense(&self) -> DenseTensor {
        let total_size: usize = self.shape.iter().product();
        let mut data = vec![0.0; total_size];

        // Enumerate all indices
        let mut indices = vec![0usize; self.order()];
        for flat_idx in 0..total_size {
            data[flat_idx] = self.eval(&indices);

            // Increment indices
            for k in (0..self.order()).rev() {
                indices[k] += 1;
                if indices[k] < self.shape[k] {
                    break;
                }
                indices[k] = 0;
            }
        }

        DenseTensor::new(data, self.shape.clone())
    }

    /// Dot product of two TTs
    pub fn dot(&self, other: &TensorTrain) -> f64 {
        assert_eq!(self.shape, other.shape);

        // Accumulate product of contracted cores
        // Result shape at step k: (r1_k × r2_k)
        let mut z = vec![1.0]; // Start with 1×1
        let mut z_rows = 1;
        let mut z_cols = 1;

        for k in 0..self.order() {
            let c1 = &self.cores[k];
            let c2 = &other.cores[k];
            let n = c1.mode_size;

            let new_rows = c1.rank_right;
            let new_cols = c2.rank_right;
            let mut new_z = vec![0.0; new_rows * new_cols];

            // Contract over mode index and previous ranks
            for i in 0..n {
                for r1l in 0..z_rows {
                    for r2l in 0..z_cols {
                        let z_val = z[r1l * z_cols + r2l];

                        for r1r in 0..c1.rank_right {
                            for r2r in 0..c2.rank_right {
                                new_z[r1r * new_cols + r2r] +=
                                    z_val * c1.get(r1l, i, r1r) * c2.get(r2l, i, r2r);
                            }
                        }
                    }
                }
            }

            z = new_z;
            z_rows = new_rows;
            z_cols = new_cols;
        }

        z[0]
    }

    /// Frobenius norm: ||A||_F = sqrt(<A, A>)
    pub fn frobenius_norm(&self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Add two TTs (result has rank r1 + r2)
    pub fn add(&self, other: &TensorTrain) -> TensorTrain {
        assert_eq!(self.shape, other.shape);

        let mut new_cores = Vec::new();

        for k in 0..self.order() {
            let c1 = &self.cores[k];
            let c2 = &other.cores[k];

            let new_rl = if k == 0 {
                1
            } else {
                c1.rank_left + c2.rank_left
            };
            let new_rr = if k == self.order() - 1 {
                1
            } else {
                c1.rank_right + c2.rank_right
            };
            let n = c1.mode_size;

            let mut new_data = vec![0.0; new_rl * n * new_rr];
            let mut new_core = TTCore::new(new_data.clone(), new_rl, n, new_rr);

            for i in 0..n {
                if k == 0 {
                    // First core: [c1, c2] horizontally
                    for rr1 in 0..c1.rank_right {
                        new_core.set(0, i, rr1, c1.get(0, i, rr1));
                    }
                    for rr2 in 0..c2.rank_right {
                        new_core.set(0, i, c1.rank_right + rr2, c2.get(0, i, rr2));
                    }
                } else if k == self.order() - 1 {
                    // Last core: [c1; c2] vertically
                    for rl1 in 0..c1.rank_left {
                        new_core.set(rl1, i, 0, c1.get(rl1, i, 0));
                    }
                    for rl2 in 0..c2.rank_left {
                        new_core.set(c1.rank_left + rl2, i, 0, c2.get(rl2, i, 0));
                    }
                } else {
                    // Middle core: block diagonal
                    for rl1 in 0..c1.rank_left {
                        for rr1 in 0..c1.rank_right {
                            new_core.set(rl1, i, rr1, c1.get(rl1, i, rr1));
                        }
                    }
                    for rl2 in 0..c2.rank_left {
                        for rr2 in 0..c2.rank_right {
                            new_core.set(
                                c1.rank_left + rl2,
                                i,
                                c1.rank_right + rr2,
                                c2.get(rl2, i, rr2),
                            );
                        }
                    }
                }
            }

            new_cores.push(new_core);
        }

        TensorTrain::from_cores(new_cores)
    }

    /// Scale by a constant
    pub fn scale(&self, alpha: f64) -> TensorTrain {
        let mut new_cores = self.cores.clone();

        // Scale first core only
        for val in new_cores[0].data.iter_mut() {
            *val *= alpha;
        }

        TensorTrain::from_cores(new_cores)
    }

    /// TT-SVD decomposition from dense tensor
    pub fn from_dense(tensor: &DenseTensor, config: &TensorTrainConfig) -> Self {
        let d = tensor.order();
        if d == 0 {
            return TensorTrain::from_cores(vec![]);
        }

        let mut cores = Vec::new();
        let mut c = tensor.data.clone();
        let mut remaining_shape = tensor.shape.clone();
        let mut left_rank = 1usize;

        for k in 0..d - 1 {
            let n_k = remaining_shape[0];
            let rest_size: usize = remaining_shape[1..].iter().product();

            // Reshape C to (left_rank * n_k) × rest_size
            let rows = left_rank * n_k;
            let cols = rest_size;

            // Simple SVD via power iteration (for demonstration)
            let (u, s, vt, new_rank) = simple_svd(&c, rows, cols, config);

            // Create core from U
            let core = TTCore::new(u, left_rank, n_k, new_rank);
            cores.push(core);

            // C = S * Vt for next iteration
            c = Vec::with_capacity(new_rank * cols);
            for i in 0..new_rank {
                for j in 0..cols {
                    c.push(s[i] * vt[i * cols + j]);
                }
            }

            left_rank = new_rank;
            remaining_shape.remove(0);
        }

        // Last core
        let n_d = remaining_shape[0];
        let last_core = TTCore::new(c, left_rank, n_d, 1);
        cores.push(last_core);

        TensorTrain::from_cores(cores)
    }
}

/// Simple truncated SVD using power iteration
/// Returns (U, S, Vt, rank)
fn simple_svd(
    a: &[f64],
    rows: usize,
    cols: usize,
    config: &TensorTrainConfig,
) -> (Vec<f64>, Vec<f64>, Vec<f64>, usize) {
    let max_rank = if config.max_rank > 0 {
        config.max_rank.min(rows).min(cols)
    } else {
        rows.min(cols)
    };

    let mut u = Vec::new();
    let mut s = Vec::new();
    let mut vt = Vec::new();

    let mut a_residual = a.to_vec();

    for _ in 0..max_rank {
        // Power iteration to find top singular vector
        let (sigma, u_vec, v_vec) = power_iteration(&a_residual, rows, cols, 20);

        if sigma < config.tolerance {
            break;
        }

        s.push(sigma);
        u.extend(u_vec.iter());
        vt.extend(v_vec.iter());

        // Deflate: A = A - sigma * u * v^T
        for i in 0..rows {
            for j in 0..cols {
                a_residual[i * cols + j] -= sigma * u_vec[i] * v_vec[j];
            }
        }
    }

    let rank = s.len();
    (u, s, vt, rank.max(1))
}

/// Power iteration for largest singular value
fn power_iteration(
    a: &[f64],
    rows: usize,
    cols: usize,
    max_iter: usize,
) -> (f64, Vec<f64>, Vec<f64>) {
    // Initialize random v
    let mut v: Vec<f64> = (0..cols)
        .map(|i| ((i * 2654435769) as f64 / 4294967296.0) * 2.0 - 1.0)
        .collect();
    normalize(&mut v);

    let mut u = vec![0.0; rows];

    for _ in 0..max_iter {
        // u = A * v
        for i in 0..rows {
            u[i] = 0.0;
            for j in 0..cols {
                u[i] += a[i * cols + j] * v[j];
            }
        }
        normalize(&mut u);

        // v = A^T * u
        for j in 0..cols {
            v[j] = 0.0;
            for i in 0..rows {
                v[j] += a[i * cols + j] * u[i];
            }
        }
        normalize(&mut v);
    }

    // Compute singular value
    let mut av = vec![0.0; rows];
    for i in 0..rows {
        for j in 0..cols {
            av[i] += a[i * cols + j] * v[j];
        }
    }
    let sigma: f64 = u.iter().zip(av.iter()).map(|(ui, avi)| ui * avi).sum();

    (sigma.abs(), u, v)
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

    #[test]
    fn test_tt_eval() {
        // Rank-1 TT representing outer product of [1,2] and [3,4]
        let v1 = vec![1.0, 2.0];
        let v2 = vec![3.0, 4.0];
        let tt = TensorTrain::from_vectors(vec![v1, v2]);

        // Should equal v1[i] * v2[j]
        assert!((tt.eval(&[0, 0]) - 3.0).abs() < 1e-10);
        assert!((tt.eval(&[0, 1]) - 4.0).abs() < 1e-10);
        assert!((tt.eval(&[1, 0]) - 6.0).abs() < 1e-10);
        assert!((tt.eval(&[1, 1]) - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_tt_dot() {
        let v1 = vec![1.0, 2.0];
        let v2 = vec![3.0, 4.0];
        let tt = TensorTrain::from_vectors(vec![v1, v2]);

        // <A, A> = sum of squares
        let norm_sq = tt.dot(&tt);
        // Elements: 3, 4, 6, 8 -> sum of squares = 9 + 16 + 36 + 64 = 125
        assert!((norm_sq - 125.0).abs() < 1e-10);
    }

    #[test]
    fn test_tt_from_dense() {
        let tensor = DenseTensor::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]);
        let tt = TensorTrain::from_dense(&tensor, &TensorTrainConfig::default());

        // Check reconstruction
        let reconstructed = tt.to_dense();
        let error: f64 = tensor
            .data
            .iter()
            .zip(reconstructed.data.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();

        assert!(error < 1e-6);
    }

    #[test]
    fn test_tt_add() {
        let v1 = vec![1.0, 2.0];
        let v2 = vec![3.0, 4.0];
        let tt1 = TensorTrain::from_vectors(vec![v1.clone(), v2.clone()]);
        let tt2 = TensorTrain::from_vectors(vec![v1, v2]);

        let sum = tt1.add(&tt2);

        // Should be 2 * tt1
        assert!((sum.eval(&[0, 0]) - 6.0).abs() < 1e-10);
        assert!((sum.eval(&[1, 1]) - 16.0).abs() < 1e-10);
    }
}
