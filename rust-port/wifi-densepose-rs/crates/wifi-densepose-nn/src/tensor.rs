//! Tensor types and operations for neural network inference.
//!
//! This module provides a unified tensor abstraction that works across
//! different backends (ONNX, tch, Candle).

use crate::error::{NnError, NnResult};
use ndarray::{Array1, Array2, Array3, Array4, ArrayD};
// num_traits is available if needed for advanced tensor operations
use serde::{Deserialize, Serialize};
use std::fmt;

/// Shape of a tensor
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TensorShape(Vec<usize>);

impl TensorShape {
    /// Create a new tensor shape
    pub fn new(dims: Vec<usize>) -> Self {
        Self(dims)
    }

    /// Create a shape from a slice
    pub fn from_slice(dims: &[usize]) -> Self {
        Self(dims.to_vec())
    }

    /// Get the number of dimensions
    pub fn ndim(&self) -> usize {
        self.0.len()
    }

    /// Get the dimensions
    pub fn dims(&self) -> &[usize] {
        &self.0
    }

    /// Get the total number of elements
    pub fn numel(&self) -> usize {
        self.0.iter().product()
    }

    /// Get dimension at index
    pub fn dim(&self, idx: usize) -> Option<usize> {
        self.0.get(idx).copied()
    }

    /// Check if shapes are compatible for broadcasting
    pub fn is_broadcast_compatible(&self, other: &TensorShape) -> bool {
        let max_dims = self.ndim().max(other.ndim());
        for i in 0..max_dims {
            let d1 = self.0.get(self.ndim().saturating_sub(i + 1)).unwrap_or(&1);
            let d2 = other.0.get(other.ndim().saturating_sub(i + 1)).unwrap_or(&1);
            if *d1 != *d2 && *d1 != 1 && *d2 != 1 {
                return false;
            }
        }
        true
    }
}

impl fmt::Display for TensorShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, d) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", d)?;
        }
        write!(f, "]")
    }
}

impl From<Vec<usize>> for TensorShape {
    fn from(dims: Vec<usize>) -> Self {
        Self::new(dims)
    }
}

impl From<&[usize]> for TensorShape {
    fn from(dims: &[usize]) -> Self {
        Self::from_slice(dims)
    }
}

impl<const N: usize> From<[usize; N]> for TensorShape {
    fn from(dims: [usize; N]) -> Self {
        Self::new(dims.to_vec())
    }
}

/// Data type for tensor elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// 32-bit floating point
    Float32,
    /// 64-bit floating point
    Float64,
    /// 32-bit integer
    Int32,
    /// 64-bit integer
    Int64,
    /// 8-bit unsigned integer
    Uint8,
    /// Boolean
    Bool,
}

impl DataType {
    /// Get the size of this data type in bytes
    pub fn size_bytes(&self) -> usize {
        match self {
            DataType::Float32 => 4,
            DataType::Float64 => 8,
            DataType::Int32 => 4,
            DataType::Int64 => 8,
            DataType::Uint8 => 1,
            DataType::Bool => 1,
        }
    }
}

/// A tensor wrapper that abstracts over different array types
#[derive(Debug, Clone)]
pub enum Tensor {
    /// 1D float tensor
    Float1D(Array1<f32>),
    /// 2D float tensor
    Float2D(Array2<f32>),
    /// 3D float tensor
    Float3D(Array3<f32>),
    /// 4D float tensor (batch, channels, height, width)
    Float4D(Array4<f32>),
    /// Dynamic dimension float tensor
    FloatND(ArrayD<f32>),
    /// 1D integer tensor
    Int1D(Array1<i64>),
    /// 2D integer tensor
    Int2D(Array2<i64>),
    /// Dynamic dimension integer tensor
    IntND(ArrayD<i64>),
}

impl Tensor {
    /// Create a new 4D float tensor filled with zeros
    pub fn zeros_4d(shape: [usize; 4]) -> Self {
        Tensor::Float4D(Array4::zeros(shape))
    }

    /// Create a new 4D float tensor filled with ones
    pub fn ones_4d(shape: [usize; 4]) -> Self {
        Tensor::Float4D(Array4::ones(shape))
    }

    /// Create a tensor from a 4D ndarray
    pub fn from_array4(array: Array4<f32>) -> Self {
        Tensor::Float4D(array)
    }

    /// Create a tensor from a dynamic ndarray
    pub fn from_arrayd(array: ArrayD<f32>) -> Self {
        Tensor::FloatND(array)
    }

    /// Get the shape of the tensor
    pub fn shape(&self) -> TensorShape {
        match self {
            Tensor::Float1D(a) => TensorShape::from_slice(a.shape()),
            Tensor::Float2D(a) => TensorShape::from_slice(a.shape()),
            Tensor::Float3D(a) => TensorShape::from_slice(a.shape()),
            Tensor::Float4D(a) => TensorShape::from_slice(a.shape()),
            Tensor::FloatND(a) => TensorShape::from_slice(a.shape()),
            Tensor::Int1D(a) => TensorShape::from_slice(a.shape()),
            Tensor::Int2D(a) => TensorShape::from_slice(a.shape()),
            Tensor::IntND(a) => TensorShape::from_slice(a.shape()),
        }
    }

    /// Get the data type
    pub fn dtype(&self) -> DataType {
        match self {
            Tensor::Float1D(_)
            | Tensor::Float2D(_)
            | Tensor::Float3D(_)
            | Tensor::Float4D(_)
            | Tensor::FloatND(_) => DataType::Float32,
            Tensor::Int1D(_) | Tensor::Int2D(_) | Tensor::IntND(_) => DataType::Int64,
        }
    }

    /// Get the number of elements
    pub fn numel(&self) -> usize {
        self.shape().numel()
    }

    /// Get the number of dimensions
    pub fn ndim(&self) -> usize {
        self.shape().ndim()
    }

    /// Try to convert to a 4D float array
    pub fn as_array4(&self) -> NnResult<&Array4<f32>> {
        match self {
            Tensor::Float4D(a) => Ok(a),
            _ => Err(NnError::tensor_op("Cannot convert to 4D array")),
        }
    }

    /// Try to convert to a mutable 4D float array
    pub fn as_array4_mut(&mut self) -> NnResult<&mut Array4<f32>> {
        match self {
            Tensor::Float4D(a) => Ok(a),
            _ => Err(NnError::tensor_op("Cannot convert to mutable 4D array")),
        }
    }

    /// Get the underlying data as a slice
    pub fn as_slice(&self) -> NnResult<&[f32]> {
        match self {
            Tensor::Float1D(a) => a.as_slice().ok_or_else(|| NnError::tensor_op("Non-contiguous array")),
            Tensor::Float2D(a) => a.as_slice().ok_or_else(|| NnError::tensor_op("Non-contiguous array")),
            Tensor::Float3D(a) => a.as_slice().ok_or_else(|| NnError::tensor_op("Non-contiguous array")),
            Tensor::Float4D(a) => a.as_slice().ok_or_else(|| NnError::tensor_op("Non-contiguous array")),
            Tensor::FloatND(a) => a.as_slice().ok_or_else(|| NnError::tensor_op("Non-contiguous array")),
            _ => Err(NnError::tensor_op("Cannot get float slice from integer tensor")),
        }
    }

    /// Convert tensor to owned Vec
    pub fn to_vec(&self) -> NnResult<Vec<f32>> {
        match self {
            Tensor::Float1D(a) => Ok(a.iter().copied().collect()),
            Tensor::Float2D(a) => Ok(a.iter().copied().collect()),
            Tensor::Float3D(a) => Ok(a.iter().copied().collect()),
            Tensor::Float4D(a) => Ok(a.iter().copied().collect()),
            Tensor::FloatND(a) => Ok(a.iter().copied().collect()),
            _ => Err(NnError::tensor_op("Cannot convert integer tensor to float vec")),
        }
    }

    /// Apply ReLU activation
    pub fn relu(&self) -> NnResult<Tensor> {
        match self {
            Tensor::Float4D(a) => Ok(Tensor::Float4D(a.mapv(|x| x.max(0.0)))),
            Tensor::FloatND(a) => Ok(Tensor::FloatND(a.mapv(|x| x.max(0.0)))),
            _ => Err(NnError::tensor_op("ReLU not supported for this tensor type")),
        }
    }

    /// Apply sigmoid activation
    pub fn sigmoid(&self) -> NnResult<Tensor> {
        match self {
            Tensor::Float4D(a) => Ok(Tensor::Float4D(a.mapv(|x| 1.0 / (1.0 + (-x).exp())))),
            Tensor::FloatND(a) => Ok(Tensor::FloatND(a.mapv(|x| 1.0 / (1.0 + (-x).exp())))),
            _ => Err(NnError::tensor_op("Sigmoid not supported for this tensor type")),
        }
    }

    /// Apply tanh activation
    pub fn tanh(&self) -> NnResult<Tensor> {
        match self {
            Tensor::Float4D(a) => Ok(Tensor::Float4D(a.mapv(|x| x.tanh()))),
            Tensor::FloatND(a) => Ok(Tensor::FloatND(a.mapv(|x| x.tanh()))),
            _ => Err(NnError::tensor_op("Tanh not supported for this tensor type")),
        }
    }

    /// Apply softmax along axis
    pub fn softmax(&self, axis: usize) -> NnResult<Tensor> {
        match self {
            Tensor::Float4D(a) => {
                let max = a.fold(f32::NEG_INFINITY, |acc, &x| acc.max(x));
                let exp = a.mapv(|x| (x - max).exp());
                let sum = exp.sum();
                Ok(Tensor::Float4D(exp / sum))
            }
            _ => Err(NnError::tensor_op("Softmax not supported for this tensor type")),
        }
    }

    /// Get argmax along axis
    pub fn argmax(&self, axis: usize) -> NnResult<Tensor> {
        match self {
            Tensor::Float4D(a) => {
                let result = a.map_axis(ndarray::Axis(axis), |row| {
                    row.iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                        .map(|(i, _)| i as i64)
                        .unwrap_or(0)
                });
                Ok(Tensor::IntND(result.into_dyn()))
            }
            _ => Err(NnError::tensor_op("Argmax not supported for this tensor type")),
        }
    }

    /// Compute mean
    pub fn mean(&self) -> NnResult<f32> {
        match self {
            Tensor::Float4D(a) => Ok(a.mean().unwrap_or(0.0)),
            Tensor::FloatND(a) => Ok(a.mean().unwrap_or(0.0)),
            _ => Err(NnError::tensor_op("Mean not supported for this tensor type")),
        }
    }

    /// Compute standard deviation
    pub fn std(&self) -> NnResult<f32> {
        match self {
            Tensor::Float4D(a) => {
                let mean = a.mean().unwrap_or(0.0);
                let variance = a.mapv(|x| (x - mean).powi(2)).mean().unwrap_or(0.0);
                Ok(variance.sqrt())
            }
            Tensor::FloatND(a) => {
                let mean = a.mean().unwrap_or(0.0);
                let variance = a.mapv(|x| (x - mean).powi(2)).mean().unwrap_or(0.0);
                Ok(variance.sqrt())
            }
            _ => Err(NnError::tensor_op("Std not supported for this tensor type")),
        }
    }

    /// Get min value
    pub fn min(&self) -> NnResult<f32> {
        match self {
            Tensor::Float4D(a) => Ok(a.fold(f32::INFINITY, |acc, &x| acc.min(x))),
            Tensor::FloatND(a) => Ok(a.fold(f32::INFINITY, |acc, &x| acc.min(x))),
            _ => Err(NnError::tensor_op("Min not supported for this tensor type")),
        }
    }

    /// Get max value
    pub fn max(&self) -> NnResult<f32> {
        match self {
            Tensor::Float4D(a) => Ok(a.fold(f32::NEG_INFINITY, |acc, &x| acc.max(x))),
            Tensor::FloatND(a) => Ok(a.fold(f32::NEG_INFINITY, |acc, &x| acc.max(x))),
            _ => Err(NnError::tensor_op("Max not supported for this tensor type")),
        }
    }
}

/// Statistics about a tensor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorStats {
    /// Mean value
    pub mean: f32,
    /// Standard deviation
    pub std: f32,
    /// Minimum value
    pub min: f32,
    /// Maximum value
    pub max: f32,
    /// Sparsity (fraction of zeros)
    pub sparsity: f32,
}

impl TensorStats {
    /// Compute statistics for a tensor
    pub fn from_tensor(tensor: &Tensor) -> NnResult<Self> {
        let mean = tensor.mean()?;
        let std = tensor.std()?;
        let min = tensor.min()?;
        let max = tensor.max()?;

        // Compute sparsity
        let sparsity = match tensor {
            Tensor::Float4D(a) => {
                let zeros = a.iter().filter(|&&x| x == 0.0).count();
                zeros as f32 / a.len() as f32
            }
            Tensor::FloatND(a) => {
                let zeros = a.iter().filter(|&&x| x == 0.0).count();
                zeros as f32 / a.len() as f32
            }
            _ => 0.0,
        };

        Ok(TensorStats {
            mean,
            std,
            min,
            max,
            sparsity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_shape() {
        let shape = TensorShape::new(vec![1, 3, 224, 224]);
        assert_eq!(shape.ndim(), 4);
        assert_eq!(shape.numel(), 1 * 3 * 224 * 224);
        assert_eq!(shape.dim(0), Some(1));
        assert_eq!(shape.dim(1), Some(3));
    }

    #[test]
    fn test_tensor_zeros() {
        let tensor = Tensor::zeros_4d([1, 256, 64, 64]);
        assert_eq!(tensor.shape().dims(), &[1, 256, 64, 64]);
        assert_eq!(tensor.dtype(), DataType::Float32);
    }

    #[test]
    fn test_tensor_activations() {
        let arr = Array4::from_elem([1, 2, 2, 2], -1.0f32);
        let tensor = Tensor::Float4D(arr);

        let relu = tensor.relu().unwrap();
        assert_eq!(relu.max().unwrap(), 0.0);

        let sigmoid = tensor.sigmoid().unwrap();
        assert!(sigmoid.min().unwrap() > 0.0);
        assert!(sigmoid.max().unwrap() < 1.0);
    }

    #[test]
    fn test_broadcast_compatible() {
        let a = TensorShape::new(vec![1, 3, 224, 224]);
        let b = TensorShape::new(vec![1, 1, 224, 224]);
        assert!(a.is_broadcast_compatible(&b));

        // [1, 3, 224, 224] and [2, 3, 224, 224] ARE broadcast compatible (1 broadcasts to 2)
        let c = TensorShape::new(vec![2, 3, 224, 224]);
        assert!(a.is_broadcast_compatible(&c));

        // [2, 3, 224, 224] and [3, 3, 224, 224] are NOT compatible (2 != 3, neither is 1)
        let d = TensorShape::new(vec![3, 3, 224, 224]);
        assert!(!c.is_broadcast_compatible(&d));
    }
}
