//! Tensor operations for GNN computations.
//!
//! Provides efficient tensor operations including:
//! - Matrix multiplication
//! - Element-wise operations
//! - Activation functions
//! - Weight initialization
//! - Normalization

use crate::error::{GnnError, Result};
use rand::Rng;
use rand_distr::{Distribution, Normal, Uniform};

/// Basic tensor operations for GNN computations
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    /// Flattened tensor data
    pub data: Vec<f32>,
    /// Shape of the tensor (dimensions)
    pub shape: Vec<usize>,
}

impl Tensor {
    /// Create a new tensor from data and shape
    ///
    /// # Arguments
    /// * `data` - Flattened tensor data
    /// * `shape` - Dimensions of the tensor
    ///
    /// # Returns
    /// A new `Tensor` instance
    ///
    /// # Errors
    /// Returns `GnnError::InvalidShape` if data length doesn't match shape
    pub fn new(data: Vec<f32>, shape: Vec<usize>) -> Result<Self> {
        let expected_len: usize = shape.iter().product();
        if data.len() != expected_len {
            return Err(GnnError::invalid_shape(format!(
                "Data length {} doesn't match shape {:?} (expected {})",
                data.len(),
                shape,
                expected_len
            )));
        }
        Ok(Self { data, shape })
    }

    /// Create a zero-filled tensor with the given shape
    ///
    /// # Arguments
    /// * `shape` - Dimensions of the tensor
    ///
    /// # Returns
    /// A new zero-filled `Tensor`
    ///
    /// # Errors
    /// Returns `GnnError::InvalidShape` if shape is empty or contains zero
    pub fn zeros(shape: &[usize]) -> Result<Self> {
        if shape.is_empty() || shape.iter().any(|&d| d == 0) {
            return Err(GnnError::invalid_shape(format!(
                "Invalid shape: {:?}",
                shape
            )));
        }
        let size: usize = shape.iter().product();
        Ok(Self {
            data: vec![0.0; size],
            shape: shape.to_vec(),
        })
    }

    /// Create a 1D tensor from a vector
    ///
    /// # Arguments
    /// * `data` - Vector data
    ///
    /// # Returns
    /// A new 1D `Tensor`
    pub fn from_vec(data: Vec<f32>) -> Self {
        let len = data.len();
        Self {
            data,
            shape: vec![len],
        }
    }

    /// Compute dot product with another tensor (both must be 1D)
    ///
    /// # Arguments
    /// * `other` - Another tensor to compute dot product with
    ///
    /// # Returns
    /// The dot product as a scalar
    ///
    /// # Errors
    /// Returns `GnnError::DimensionMismatch` if tensors are not 1D or have different lengths
    pub fn dot(&self, other: &Tensor) -> Result<f32> {
        if self.shape.len() != 1 || other.shape.len() != 1 {
            return Err(GnnError::dimension_mismatch(
                "1D tensors",
                format!("{}D and {}D", self.shape.len(), other.shape.len()),
            ));
        }
        if self.shape[0] != other.shape[0] {
            return Err(GnnError::dimension_mismatch(
                format!("length {}", self.shape[0]),
                format!("length {}", other.shape[0]),
            ));
        }

        let result = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .sum();
        Ok(result)
    }

    /// Matrix multiplication
    ///
    /// # Arguments
    /// * `other` - Another tensor to multiply with
    ///
    /// # Returns
    /// The result of matrix multiplication
    ///
    /// # Errors
    /// Returns `GnnError::DimensionMismatch` if dimensions are incompatible
    pub fn matmul(&self, other: &Tensor) -> Result<Tensor> {
        // Support 1D x 1D (dot product), 2D x 1D, 2D x 2D
        match (self.shape.len(), other.shape.len()) {
            (1, 1) => {
                let dot = self.dot(other)?;
                Ok(Tensor::from_vec(vec![dot]))
            }
            (2, 1) => {
                // Matrix-vector multiplication
                let m = self.shape[0];
                let n = self.shape[1];
                if n != other.shape[0] {
                    return Err(GnnError::dimension_mismatch(
                        format!("{}x{}", m, n),
                        format!("vector of length {}", other.shape[0]),
                    ));
                }

                let mut result = vec![0.0; m];
                for i in 0..m {
                    for j in 0..n {
                        result[i] += self.data[i * n + j] * other.data[j];
                    }
                }
                Ok(Tensor::from_vec(result))
            }
            (2, 2) => {
                // Matrix-matrix multiplication
                let m = self.shape[0];
                let n = self.shape[1];
                let p = other.shape[1];

                if n != other.shape[0] {
                    return Err(GnnError::dimension_mismatch(
                        format!("{}x{}", m, n),
                        format!("{}x{}", other.shape[0], p),
                    ));
                }

                let mut result = vec![0.0; m * p];
                for i in 0..m {
                    for j in 0..p {
                        for k in 0..n {
                            result[i * p + j] += self.data[i * n + k] * other.data[k * p + j];
                        }
                    }
                }
                Tensor::new(result, vec![m, p])
            }
            _ => Err(GnnError::dimension_mismatch(
                "1D or 2D tensors",
                format!("{}D and {}D", self.shape.len(), other.shape.len()),
            )),
        }
    }

    /// Element-wise addition
    ///
    /// # Arguments
    /// * `other` - Another tensor to add
    ///
    /// # Returns
    /// The sum of the two tensors
    ///
    /// # Errors
    /// Returns `GnnError::DimensionMismatch` if shapes don't match
    pub fn add(&self, other: &Tensor) -> Result<Tensor> {
        if self.shape != other.shape {
            return Err(GnnError::dimension_mismatch(
                format!("{:?}", self.shape),
                format!("{:?}", other.shape),
            ));
        }

        let result: Vec<f32> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a + b)
            .collect();

        Tensor::new(result, self.shape.clone())
    }

    /// Scalar multiplication
    ///
    /// # Arguments
    /// * `scalar` - Scalar value to multiply by
    ///
    /// # Returns
    /// A new tensor with all elements scaled
    pub fn scale(&self, scalar: f32) -> Tensor {
        let result: Vec<f32> = self.data.iter().map(|&x| x * scalar).collect();
        Tensor {
            data: result,
            shape: self.shape.clone(),
        }
    }

    /// ReLU activation function (max(0, x))
    ///
    /// # Returns
    /// A new tensor with ReLU applied element-wise
    pub fn relu(&self) -> Tensor {
        let result: Vec<f32> = self.data.iter().map(|&x| x.max(0.0)).collect();
        Tensor {
            data: result,
            shape: self.shape.clone(),
        }
    }

    /// Sigmoid activation function (1 / (1 + e^(-x))) with numerical stability
    ///
    /// # Returns
    /// A new tensor with sigmoid applied element-wise
    pub fn sigmoid(&self) -> Tensor {
        let result: Vec<f32> = self
            .data
            .iter()
            .map(|&x| {
                if x > 0.0 {
                    1.0 / (1.0 + (-x).exp())
                } else {
                    let ex = x.exp();
                    ex / (1.0 + ex)
                }
            })
            .collect();
        Tensor {
            data: result,
            shape: self.shape.clone(),
        }
    }

    /// Tanh activation function
    ///
    /// # Returns
    /// A new tensor with tanh applied element-wise
    pub fn tanh(&self) -> Tensor {
        let result: Vec<f32> = self.data.iter().map(|&x| x.tanh()).collect();
        Tensor {
            data: result,
            shape: self.shape.clone(),
        }
    }

    /// Compute L2 norm (Euclidean norm) with improved precision
    ///
    /// # Returns
    /// The L2 norm of the tensor
    pub fn l2_norm(&self) -> f32 {
        // Use f64 accumulator for better numerical precision
        let sum_squares: f64 = self.data.iter().map(|&x| (x as f64) * (x as f64)).sum();
        (sum_squares.sqrt()) as f32
    }

    /// Normalize the tensor to unit L2 norm
    ///
    /// # Returns
    /// A normalized tensor
    ///
    /// # Errors
    /// Returns `GnnError::InvalidInput` if norm is zero
    pub fn normalize(&self) -> Result<Tensor> {
        let norm = self.l2_norm();
        if norm == 0.0 {
            return Err(GnnError::invalid_input(
                "Cannot normalize zero vector".to_string(),
            ));
        }
        Ok(self.scale(1.0 / norm))
    }

    /// Get a slice view of the tensor data
    ///
    /// # Returns
    /// A slice reference to the underlying data
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Consume the tensor and return the underlying vector
    ///
    /// # Returns
    /// The vector containing the tensor data
    pub fn into_vec(self) -> Vec<f32> {
        self.data
    }

    /// Get the number of elements in the tensor
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the tensor is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Xavier/Glorot initialization for neural network weights
///
/// Samples from uniform distribution U(-a, a) where a = sqrt(6 / (fan_in + fan_out))
///
/// # Arguments
/// * `fan_in` - Number of input units
/// * `fan_out` - Number of output units
///
/// # Returns
/// A vector of initialized weights
///
/// # Panics
/// Panics if fan_in or fan_out is 0
pub fn xavier_init(fan_in: usize, fan_out: usize) -> Vec<f32> {
    assert!(
        fan_in > 0 && fan_out > 0,
        "fan_in and fan_out must be positive"
    );

    let limit = (6.0 / (fan_in + fan_out) as f32).sqrt();
    let uniform = Uniform::new(-limit, limit);
    let mut rng = rand::thread_rng();

    (0..fan_in * fan_out)
        .map(|_| uniform.sample(&mut rng))
        .collect()
}

/// He initialization for ReLU networks
///
/// Samples from normal distribution N(0, sqrt(2 / fan_in))
///
/// # Arguments
/// * `fan_in` - Number of input units
///
/// # Returns
/// A vector of initialized weights
///
/// # Panics
/// Panics if fan_in is 0
pub fn he_init(fan_in: usize) -> Vec<f32> {
    assert!(fan_in > 0, "fan_in must be positive");

    let std_dev = (2.0 / fan_in as f32).sqrt();
    let normal = Normal::new(0.0, std_dev).expect("Invalid normal distribution parameters");
    let mut rng = rand::thread_rng();

    (0..fan_in).map(|_| normal.sample(&mut rng)).collect()
}

/// Element-wise (Hadamard) product
///
/// # Arguments
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
/// Element-wise product of the two vectors
///
/// # Panics
/// Panics if vectors have different lengths
pub fn hadamard_product(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), b.len(), "Vectors must have the same length");
    a.iter().zip(b.iter()).map(|(x, y)| x * y).collect()
}

/// Element-wise vector addition
///
/// # Arguments
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
/// Element-wise sum of the two vectors
///
/// # Panics
/// Panics if vectors have different lengths
pub fn vector_add(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), b.len(), "Vectors must have the same length");
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// Scalar multiplication of a vector
///
/// # Arguments
/// * `v` - Input vector
/// * `scalar` - Scalar multiplier
///
/// # Returns
/// Vector with all elements multiplied by scalar
pub fn vector_scale(v: &[f32], scalar: f32) -> Vec<f32> {
    v.iter().map(|&x| x * scalar).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn assert_vec_approx_eq(a: &[f32], b: &[f32], epsilon: f32) {
        assert_eq!(a.len(), b.len(), "Vectors have different lengths");
        for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
            assert!(
                (x - y).abs() < epsilon,
                "Values at index {} differ: {} vs {} (diff: {})",
                i,
                x,
                y,
                (x - y).abs()
            );
        }
    }

    #[test]
    fn test_tensor_new() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let tensor = Tensor::new(data.clone(), vec![2, 2]).unwrap();
        assert_eq!(tensor.data, data);
        assert_eq!(tensor.shape, vec![2, 2]);
    }

    #[test]
    fn test_tensor_new_invalid_shape() {
        let data = vec![1.0, 2.0, 3.0];
        let result = Tensor::new(data, vec![2, 2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_zeros() {
        let tensor = Tensor::zeros(&[3, 2]).unwrap();
        assert_eq!(tensor.data, vec![0.0; 6]);
        assert_eq!(tensor.shape, vec![3, 2]);
    }

    #[test]
    fn test_tensor_zeros_invalid_shape() {
        let result = Tensor::zeros(&[0, 2]);
        assert!(result.is_err());

        let result = Tensor::zeros(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_from_vec() {
        let data = vec![1.0, 2.0, 3.0];
        let tensor = Tensor::from_vec(data.clone());
        assert_eq!(tensor.data, data);
        assert_eq!(tensor.shape, vec![3]);
    }

    #[test]
    fn test_dot_product() {
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let b = Tensor::from_vec(vec![4.0, 5.0, 6.0]);
        let result = a.dot(&b).unwrap();
        assert!((result - 32.0).abs() < EPSILON); // 1*4 + 2*5 + 3*6 = 32
    }

    #[test]
    fn test_dot_product_dimension_mismatch() {
        let a = Tensor::from_vec(vec![1.0, 2.0]);
        let b = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let result = a.dot(&b);
        assert!(result.is_err());
    }

    #[test]
    fn test_matmul_1d() {
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let b = Tensor::from_vec(vec![4.0, 5.0, 6.0]);
        let result = a.matmul(&b).unwrap();
        assert_eq!(result.shape, vec![1]);
        assert!((result.data[0] - 32.0).abs() < EPSILON);
    }

    #[test]
    fn test_matmul_2d_1d() {
        // Matrix-vector multiplication
        let mat = Tensor::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]).unwrap();
        let vec = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let result = mat.matmul(&vec).unwrap();

        assert_eq!(result.shape, vec![2]);
        // [1,2,3] * [1,2,3]' = 14
        // [4,5,6] * [1,2,3]' = 32
        assert_vec_approx_eq(&result.data, &[14.0, 32.0], EPSILON);
    }

    #[test]
    fn test_matmul_2d_2d() {
        // Matrix-matrix multiplication
        let a = Tensor::new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).unwrap();
        let b = Tensor::new(vec![5.0, 6.0, 7.0, 8.0], vec![2, 2]).unwrap();
        let result = a.matmul(&b).unwrap();

        assert_eq!(result.shape, vec![2, 2]);
        // [[1,2], [3,4]] * [[5,6], [7,8]] = [[19,22], [43,50]]
        assert_vec_approx_eq(&result.data, &[19.0, 22.0, 43.0, 50.0], EPSILON);
    }

    #[test]
    fn test_matmul_dimension_mismatch() {
        let a = Tensor::new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).unwrap();
        let b = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let result = a.matmul(&b);
        assert!(result.is_err());
    }

    #[test]
    fn test_add() {
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let b = Tensor::from_vec(vec![4.0, 5.0, 6.0]);
        let result = a.add(&b).unwrap();
        assert_eq!(result.data, vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn test_add_dimension_mismatch() {
        let a = Tensor::from_vec(vec![1.0, 2.0]);
        let b = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let result = a.add(&b);
        assert!(result.is_err());
    }

    #[test]
    fn test_scale() {
        let tensor = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let result = tensor.scale(2.0);
        assert_eq!(result.data, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_relu() {
        let tensor = Tensor::from_vec(vec![-1.0, 0.0, 1.0, 2.0]);
        let result = tensor.relu();
        assert_eq!(result.data, vec![0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn test_sigmoid() {
        let tensor = Tensor::from_vec(vec![0.0, 1.0, -1.0]);
        let result = tensor.sigmoid();

        assert!((result.data[0] - 0.5).abs() < EPSILON);
        assert!((result.data[1] - 0.7310586).abs() < EPSILON);
        assert!((result.data[2] - 0.26894143).abs() < EPSILON);
    }

    #[test]
    fn test_tanh() {
        let tensor = Tensor::from_vec(vec![0.0, 1.0, -1.0]);
        let result = tensor.tanh();

        assert!((result.data[0] - 0.0).abs() < EPSILON);
        assert!((result.data[1] - 0.7615942).abs() < EPSILON);
        assert!((result.data[2] - (-0.7615942)).abs() < EPSILON);
    }

    #[test]
    fn test_l2_norm() {
        let tensor = Tensor::from_vec(vec![3.0, 4.0]);
        let norm = tensor.l2_norm();
        assert!((norm - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_normalize() {
        let tensor = Tensor::from_vec(vec![3.0, 4.0]);
        let result = tensor.normalize().unwrap();
        assert_vec_approx_eq(&result.data, &[0.6, 0.8], EPSILON);
        assert!((result.l2_norm() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let tensor = Tensor::from_vec(vec![0.0, 0.0]);
        let result = tensor.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_as_slice() {
        let data = vec![1.0, 2.0, 3.0];
        let tensor = Tensor::from_vec(data.clone());
        assert_eq!(tensor.as_slice(), &data[..]);
    }

    #[test]
    fn test_into_vec() {
        let data = vec![1.0, 2.0, 3.0];
        let tensor = Tensor::from_vec(data.clone());
        assert_eq!(tensor.into_vec(), data);
    }

    #[test]
    fn test_len() {
        let tensor = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        assert_eq!(tensor.len(), 3);
    }

    #[test]
    fn test_is_empty() {
        let tensor = Tensor::from_vec(vec![]);
        assert!(tensor.is_empty());

        let tensor = Tensor::from_vec(vec![1.0]);
        assert!(!tensor.is_empty());
    }

    #[test]
    fn test_xavier_init() {
        let weights = xavier_init(100, 50);
        assert_eq!(weights.len(), 5000);

        // Check that values are in expected range
        let limit = (6.0 / 150.0_f32).sqrt();
        for &w in &weights {
            assert!(w >= -limit && w <= limit);
        }

        // Check distribution properties
        let mean: f32 = weights.iter().sum::<f32>() / weights.len() as f32;
        assert!(mean.abs() < 0.1); // Mean should be close to 0
    }

    #[test]
    #[should_panic(expected = "fan_in and fan_out must be positive")]
    fn test_xavier_init_zero_fan() {
        xavier_init(0, 10);
    }

    #[test]
    fn test_he_init() {
        let weights = he_init(100);
        assert_eq!(weights.len(), 100);

        // Check distribution properties
        let mean: f32 = weights.iter().sum::<f32>() / weights.len() as f32;
        assert!(mean.abs() < 0.2); // Mean should be close to 0
    }

    #[test]
    #[should_panic(expected = "fan_in must be positive")]
    fn test_he_init_zero_fan() {
        he_init(0);
    }

    #[test]
    fn test_hadamard_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let result = hadamard_product(&a, &b);
        assert_eq!(result, vec![4.0, 10.0, 18.0]);
    }

    #[test]
    #[should_panic(expected = "Vectors must have the same length")]
    fn test_hadamard_product_length_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        hadamard_product(&a, &b);
    }

    #[test]
    fn test_vector_add() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let result = vector_add(&a, &b);
        assert_eq!(result, vec![5.0, 7.0, 9.0]);
    }

    #[test]
    #[should_panic(expected = "Vectors must have the same length")]
    fn test_vector_add_length_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        vector_add(&a, &b);
    }

    #[test]
    fn test_vector_scale() {
        let v = vec![1.0, 2.0, 3.0];
        let result = vector_scale(&v, 2.5);
        assert_vec_approx_eq(&result, &[2.5, 5.0, 7.5], EPSILON);
    }

    #[test]
    fn test_complex_operations() {
        // Test chaining operations
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0]);
        let b = Tensor::from_vec(vec![0.5, 1.0, 1.5]);

        let sum = a.add(&b).unwrap();
        let scaled = sum.scale(2.0);
        let activated = scaled.relu();
        let normalized = activated.normalize().unwrap();

        assert!((normalized.l2_norm() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_edge_case_single_element() {
        let tensor = Tensor::from_vec(vec![5.0]);
        assert_eq!(tensor.len(), 1);
        assert_eq!(tensor.l2_norm(), 5.0);

        let normalized = tensor.normalize().unwrap();
        assert_vec_approx_eq(&normalized.data, &[1.0], EPSILON);
    }

    #[test]
    fn test_edge_case_negative_values() {
        let tensor = Tensor::from_vec(vec![-3.0, -4.0]);
        assert!((tensor.l2_norm() - 5.0).abs() < EPSILON);

        let relu_result = tensor.relu();
        assert_eq!(relu_result.data, vec![0.0, 0.0]);
    }

    #[test]
    fn test_large_matrix_multiplication() {
        // 10x10 matrix multiplication
        let size = 10;
        let a_data: Vec<f32> = (0..size * size).map(|i| i as f32).collect();
        let b_data: Vec<f32> = (0..size * size).map(|i| (i % 2) as f32).collect();

        let a = Tensor::new(a_data, vec![size, size]).unwrap();
        let b = Tensor::new(b_data, vec![size, size]).unwrap();

        let result = a.matmul(&b).unwrap();
        assert_eq!(result.shape, vec![size, size]);
        assert_eq!(result.len(), size * size);
    }

    #[test]
    fn test_activation_functions_range() {
        let tensor = Tensor::from_vec(vec![-10.0, -1.0, 0.0, 1.0, 10.0]);

        // Sigmoid should be in (0, 1)
        let sigmoid = tensor.sigmoid();
        for &val in &sigmoid.data {
            assert!(val > 0.0 && val < 1.0);
        }

        // Tanh should be in [-1, 1]
        let tanh = tensor.tanh();
        for &val in &tanh.data {
            assert!(val >= -1.0 && val <= 1.0);
        }

        // ReLU should be non-negative
        let relu = tensor.relu();
        for &val in &relu.data {
            assert!(val >= 0.0);
        }
    }
}
