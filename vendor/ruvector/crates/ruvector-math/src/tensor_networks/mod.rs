//! Tensor Networks
//!
//! Efficient representations of high-dimensional tensors using network decompositions.
//!
//! ## Background
//!
//! High-dimensional tensors suffer from the "curse of dimensionality" - a tensor of
//! order d with mode sizes n has O(n^d) elements. Tensor networks provide compressed
//! representations with controllable approximation error.
//!
//! ## Decompositions
//!
//! - **Tensor Train (TT)**: A[i1,...,id] = G1[i1] × G2[i2] × ... × Gd[id]
//! - **Tucker**: Core tensor with factor matrices
//! - **CP (CANDECOMP/PARAFAC)**: Sum of rank-1 tensors
//!
//! ## Applications
//!
//! - Quantum-inspired algorithms
//! - High-dimensional integration
//! - Attention mechanism compression
//! - Scientific computing

mod contraction;
mod cp_decomposition;
mod tensor_train;
mod tucker;

pub use contraction::{NetworkContraction, TensorNetwork, TensorNode};
pub use cp_decomposition::{CPConfig, CPDecomposition};
pub use tensor_train::{TTCore, TensorTrain, TensorTrainConfig};
pub use tucker::{TuckerConfig, TuckerDecomposition};

/// Dense tensor for input/output
#[derive(Debug, Clone)]
pub struct DenseTensor {
    /// Tensor data in row-major order
    pub data: Vec<f64>,
    /// Shape of the tensor
    pub shape: Vec<usize>,
}

impl DenseTensor {
    /// Create tensor from data and shape
    pub fn new(data: Vec<f64>, shape: Vec<usize>) -> Self {
        let expected_size: usize = shape.iter().product();
        assert_eq!(data.len(), expected_size, "Data size must match shape");
        Self { data, shape }
    }

    /// Create zeros tensor
    pub fn zeros(shape: Vec<usize>) -> Self {
        let size: usize = shape.iter().product();
        Self {
            data: vec![0.0; size],
            shape,
        }
    }

    /// Create ones tensor
    pub fn ones(shape: Vec<usize>) -> Self {
        let size: usize = shape.iter().product();
        Self {
            data: vec![1.0; size],
            shape,
        }
    }

    /// Create random tensor
    pub fn random(shape: Vec<usize>, seed: u64) -> Self {
        let size: usize = shape.iter().product();
        let mut data = Vec::with_capacity(size);

        let mut s = seed;
        for _ in 0..size {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = ((s >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0;
            data.push(x);
        }

        Self { data, shape }
    }

    /// Get tensor order (number of dimensions)
    pub fn order(&self) -> usize {
        self.shape.len()
    }

    /// Get linear index from multi-index
    pub fn linear_index(&self, indices: &[usize]) -> usize {
        let mut idx = 0;
        let mut stride = 1;
        for (i, &s) in self.shape.iter().enumerate().rev() {
            idx += indices[i] * stride;
            stride *= s;
        }
        idx
    }

    /// Get element at multi-index
    pub fn get(&self, indices: &[usize]) -> f64 {
        self.data[self.linear_index(indices)]
    }

    /// Set element at multi-index
    pub fn set(&mut self, indices: &[usize], value: f64) {
        let idx = self.linear_index(indices);
        self.data[idx] = value;
    }

    /// Compute Frobenius norm
    pub fn frobenius_norm(&self) -> f64 {
        self.data.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Reshape tensor (view only, same data)
    pub fn reshape(&self, new_shape: Vec<usize>) -> Self {
        let new_size: usize = new_shape.iter().product();
        assert_eq!(self.data.len(), new_size, "New shape must have same size");
        Self {
            data: self.data.clone(),
            shape: new_shape,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dense_tensor() {
        let t = DenseTensor::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]);

        assert_eq!(t.order(), 2);
        assert!((t.get(&[0, 0]) - 1.0).abs() < 1e-10);
        assert!((t.get(&[1, 2]) - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_frobenius_norm() {
        let t = DenseTensor::new(vec![3.0, 4.0], vec![2]);
        assert!((t.frobenius_norm() - 5.0).abs() < 1e-10);
    }
}
