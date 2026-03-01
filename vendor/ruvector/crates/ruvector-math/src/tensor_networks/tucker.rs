//! Tucker Decomposition
//!
//! A[i1,...,id] = G ×1 U1 ×2 U2 ... ×d Ud
//!
//! where G is a smaller core tensor and Uk are factor matrices.

use super::DenseTensor;

/// Tucker decomposition configuration
#[derive(Debug, Clone)]
pub struct TuckerConfig {
    /// Target ranks for each mode
    pub ranks: Vec<usize>,
    /// Tolerance for truncation
    pub tolerance: f64,
    /// Max iterations for HOSVD power method
    pub max_iters: usize,
}

impl Default for TuckerConfig {
    fn default() -> Self {
        Self {
            ranks: vec![],
            tolerance: 1e-10,
            max_iters: 20,
        }
    }
}

/// Tucker decomposition of a tensor
#[derive(Debug, Clone)]
pub struct TuckerDecomposition {
    /// Core tensor G
    pub core: DenseTensor,
    /// Factor matrices U_k (each stored column-major)
    pub factors: Vec<Vec<f64>>,
    /// Original shape
    pub shape: Vec<usize>,
    /// Core shape (ranks)
    pub core_shape: Vec<usize>,
}

impl TuckerDecomposition {
    /// Higher-Order SVD decomposition
    pub fn hosvd(tensor: &DenseTensor, config: &TuckerConfig) -> Self {
        let d = tensor.order();
        let mut factors = Vec::new();
        let mut core_shape = Vec::new();

        // For each mode, compute factor matrix via SVD of mode-k unfolding
        for k in 0..d {
            let unfolding = mode_k_unfold(tensor, k);
            let (n_k, cols) = (tensor.shape[k], unfolding.len() / tensor.shape[k]);

            // Get target rank
            let rank = if k < config.ranks.len() {
                config.ranks[k].min(n_k)
            } else {
                n_k
            };

            // Compute left singular vectors via power iteration
            let u_k = compute_left_singular_vectors(&unfolding, n_k, cols, rank, config.max_iters);

            factors.push(u_k);
            core_shape.push(rank);
        }

        // Compute core: G = A ×1 U1^T ×2 U2^T ... ×d Ud^T
        let core = compute_core(tensor, &factors, &core_shape);

        Self {
            core,
            factors,
            shape: tensor.shape.clone(),
            core_shape,
        }
    }

    /// Reconstruct full tensor
    pub fn to_dense(&self) -> DenseTensor {
        // Start with core and multiply by each factor matrix
        let mut result = self.core.data.clone();
        let mut current_shape = self.core_shape.clone();

        for (k, factor) in self.factors.iter().enumerate() {
            let n_k = self.shape[k];
            let r_k = self.core_shape[k];

            // Apply U_k to mode k
            result = apply_mode_product(&result, &current_shape, factor, n_k, r_k, k);
            current_shape[k] = n_k;
        }

        DenseTensor::new(result, self.shape.clone())
    }

    /// Compression ratio
    pub fn compression_ratio(&self) -> f64 {
        let original: usize = self.shape.iter().product();
        let core_size: usize = self.core_shape.iter().product();
        let factor_size: usize = self
            .factors
            .iter()
            .enumerate()
            .map(|(k, f)| self.shape[k] * self.core_shape[k])
            .sum();

        original as f64 / (core_size + factor_size) as f64
    }
}

/// Mode-k unfolding of tensor (row-major)
fn mode_k_unfold(tensor: &DenseTensor, k: usize) -> Vec<f64> {
    let d = tensor.order();
    let n_k = tensor.shape[k];
    let cols: usize = tensor
        .shape
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != k)
        .map(|(_, &s)| s)
        .product();

    let mut result = vec![0.0; n_k * cols];

    // Enumerate all indices
    let total_size: usize = tensor.shape.iter().product();
    let mut indices = vec![0usize; d];

    for flat_idx in 0..total_size {
        let val = tensor.data[flat_idx];
        let i_k = indices[k];

        // Compute column index for unfolding
        let mut col = 0;
        let mut stride = 1;
        for m in (0..d).rev() {
            if m != k {
                col += indices[m] * stride;
                stride *= tensor.shape[m];
            }
        }

        result[i_k * cols + col] = val;

        // Increment indices
        for m in (0..d).rev() {
            indices[m] += 1;
            if indices[m] < tensor.shape[m] {
                break;
            }
            indices[m] = 0;
        }
    }

    result
}

/// Compute left singular vectors via power iteration
fn compute_left_singular_vectors(
    a: &[f64],
    rows: usize,
    cols: usize,
    rank: usize,
    max_iters: usize,
) -> Vec<f64> {
    let mut u = vec![0.0; rows * rank];

    // Compute A * A^T iteratively
    for r in 0..rank {
        // Initialize random vector
        let mut v: Vec<f64> = (0..rows)
            .map(|i| {
                let x = ((i * 2654435769 + r * 1103515245) as f64 / 4294967296.0) * 2.0 - 1.0;
                x
            })
            .collect();
        normalize(&mut v);

        // Power iteration
        for _ in 0..max_iters {
            // w = A * A^T * v
            let mut av = vec![0.0; cols];
            for i in 0..rows {
                for j in 0..cols {
                    av[j] += a[i * cols + j] * v[i];
                }
            }

            let mut aatv = vec![0.0; rows];
            for i in 0..rows {
                for j in 0..cols {
                    aatv[i] += a[i * cols + j] * av[j];
                }
            }

            // Orthogonalize against previous vectors
            for prev in 0..r {
                let mut dot = 0.0;
                for i in 0..rows {
                    dot += aatv[i] * u[i * rank + prev];
                }
                for i in 0..rows {
                    aatv[i] -= dot * u[i * rank + prev];
                }
            }

            v = aatv;
            normalize(&mut v);
        }

        // Store in U
        for i in 0..rows {
            u[i * rank + r] = v[i];
        }
    }

    u
}

fn normalize(v: &mut [f64]) {
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-15 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Compute core tensor G = A ×1 U1^T ... ×d Ud^T
fn compute_core(tensor: &DenseTensor, factors: &[Vec<f64>], core_shape: &[usize]) -> DenseTensor {
    let mut result = tensor.data.clone();
    let mut current_shape = tensor.shape.clone();

    for (k, factor) in factors.iter().enumerate() {
        let n_k = tensor.shape[k];
        let r_k = core_shape[k];

        // Apply U_k^T to mode k
        result = apply_mode_product_transpose(&result, &current_shape, factor, n_k, r_k, k);
        current_shape[k] = r_k;
    }

    DenseTensor::new(result, core_shape.to_vec())
}

/// Apply mode-k product: result[...,:,...] = A[...,:,...] * U (n_k -> r_k)
fn apply_mode_product_transpose(
    data: &[f64],
    shape: &[usize],
    u: &[f64],
    n_k: usize,
    r_k: usize,
    k: usize,
) -> Vec<f64> {
    let d = shape.len();
    let mut new_shape = shape.to_vec();
    new_shape[k] = r_k;

    let new_size: usize = new_shape.iter().product();
    let mut result = vec![0.0; new_size];

    // Enumerate old indices
    let old_size: usize = shape.iter().product();
    let mut old_indices = vec![0usize; d];

    for _ in 0..old_size {
        let old_idx = compute_linear_index(&old_indices, shape);
        let val = data[old_idx];
        let i_k = old_indices[k];

        // For each r in [0, r_k), accumulate
        for r in 0..r_k {
            let mut new_indices = old_indices.clone();
            new_indices[k] = r;
            let new_idx = compute_linear_index(&new_indices, &new_shape);

            // U is (n_k × r_k), stored row-major
            result[new_idx] += val * u[i_k * r_k + r];
        }

        // Increment indices
        for m in (0..d).rev() {
            old_indices[m] += 1;
            if old_indices[m] < shape[m] {
                break;
            }
            old_indices[m] = 0;
        }
    }

    result
}

/// Apply mode-k product: result[...,:,...] = A[...,:,...] * U^T (r_k -> n_k)
fn apply_mode_product(
    data: &[f64],
    shape: &[usize],
    u: &[f64],
    n_k: usize,
    r_k: usize,
    k: usize,
) -> Vec<f64> {
    let d = shape.len();
    let mut new_shape = shape.to_vec();
    new_shape[k] = n_k;

    let new_size: usize = new_shape.iter().product();
    let mut result = vec![0.0; new_size];

    // Enumerate old indices
    let old_size: usize = shape.iter().product();
    let mut old_indices = vec![0usize; d];

    for _ in 0..old_size {
        let old_idx = compute_linear_index(&old_indices, shape);
        let val = data[old_idx];
        let r = old_indices[k];

        // For each i in [0, n_k), accumulate
        for i in 0..n_k {
            let mut new_indices = old_indices.clone();
            new_indices[k] = i;
            let new_idx = compute_linear_index(&new_indices, &new_shape);

            // U is (n_k × r_k), U^T[r, i] = U[i, r]
            result[new_idx] += val * u[i * r_k + r];
        }

        // Increment indices
        for m in (0..d).rev() {
            old_indices[m] += 1;
            if old_indices[m] < shape[m] {
                break;
            }
            old_indices[m] = 0;
        }
    }

    result
}

fn compute_linear_index(indices: &[usize], shape: &[usize]) -> usize {
    let mut idx = 0;
    let mut stride = 1;
    for i in (0..shape.len()).rev() {
        idx += indices[i] * stride;
        stride *= shape[i];
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tucker_hosvd() {
        let tensor = DenseTensor::random(vec![4, 5, 3], 42);

        let config = TuckerConfig {
            ranks: vec![2, 3, 2],
            ..Default::default()
        };

        let tucker = TuckerDecomposition::hosvd(&tensor, &config);

        assert_eq!(tucker.core_shape, vec![2, 3, 2]);
        assert!(tucker.compression_ratio() > 1.0);
    }

    #[test]
    fn test_mode_unfold() {
        let tensor = DenseTensor::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]);

        let unfold0 = mode_k_unfold(&tensor, 0);
        // Mode-0 unfolding: 2×3 matrix, rows = original rows
        assert_eq!(unfold0.len(), 6);
    }
}
