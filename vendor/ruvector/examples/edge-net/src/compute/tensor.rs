//! Tensor abstraction layer for unified compute operations
//!
//! Provides a minimal tensor abstraction that works across all compute backends
//! (WebGPU, WebGL2, SIMD, WebWorkers, and naive fallback).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Data type for tensor elements
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DType {
    /// 32-bit floating point
    F32,
    /// 16-bit floating point (for WebGPU)
    F16,
    /// 8-bit integer (for quantized models)
    I8,
    /// Unsigned 8-bit (for embeddings)
    U8,
    /// Binary (for HDC hypervectors)
    Binary,
}

impl DType {
    /// Size in bytes for this data type
    pub fn size_bytes(&self) -> usize {
        match self {
            DType::F32 => 4,
            DType::F16 => 2,
            DType::I8 | DType::U8 => 1,
            DType::Binary => 1, // 8 bits per byte
        }
    }
}

impl Default for DType {
    fn default() -> Self {
        DType::F32
    }
}

/// Tensor shape with up to 4 dimensions
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape {
    dims: Vec<usize>,
}

impl Shape {
    /// Create a new shape from dimensions
    pub fn new(dims: &[usize]) -> Self {
        Self { dims: dims.to_vec() }
    }

    /// 1D shape (vector)
    pub fn d1(n: usize) -> Self {
        Self { dims: vec![n] }
    }

    /// 2D shape (matrix)
    pub fn d2(rows: usize, cols: usize) -> Self {
        Self { dims: vec![rows, cols] }
    }

    /// 3D shape (batch of matrices)
    pub fn d3(batch: usize, rows: usize, cols: usize) -> Self {
        Self { dims: vec![batch, rows, cols] }
    }

    /// 4D shape (e.g., attention tensors)
    pub fn d4(b: usize, h: usize, s: usize, d: usize) -> Self {
        Self { dims: vec![b, h, s, d] }
    }

    /// Total number of elements
    pub fn numel(&self) -> usize {
        self.dims.iter().product()
    }

    /// Number of dimensions
    pub fn ndim(&self) -> usize {
        self.dims.len()
    }

    /// Get dimension at index
    pub fn dim(&self, idx: usize) -> usize {
        self.dims.get(idx).copied().unwrap_or(1)
    }

    /// Get all dimensions
    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    /// Check if shape is compatible for matrix multiplication with another
    pub fn matmul_compatible(&self, other: &Shape) -> bool {
        if self.ndim() < 1 || other.ndim() < 1 {
            return false;
        }
        // Last dim of self must match second-to-last of other (or last if 1D)
        let self_k = self.dim(self.ndim() - 1);
        let other_k = if other.ndim() >= 2 {
            other.dim(other.ndim() - 2)
        } else {
            other.dim(0)
        };
        self_k == other_k
    }

    /// Compute strides for row-major layout
    pub fn strides(&self) -> Vec<usize> {
        let mut strides = vec![1; self.dims.len()];
        for i in (0..self.dims.len() - 1).rev() {
            strides[i] = strides[i + 1] * self.dims[i + 1];
        }
        strides
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, d) in self.dims.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", d)?;
        }
        write!(f, ")")
    }
}

/// Memory layout for tensors
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    /// Row-major (C-style), most common
    RowMajor,
    /// Column-major (Fortran-style)
    ColMajor,
    /// Strided (non-contiguous)
    Strided,
}

impl Default for Layout {
    fn default() -> Self {
        Layout::RowMajor
    }
}

/// Tensor storage - holds the actual data
#[derive(Clone, Debug)]
pub enum TensorStorage {
    /// CPU storage (Vec<f32>)
    Cpu(Vec<f32>),
    /// Quantized storage (Vec<i8>)
    Quantized(Vec<i8>, f32), // (data, scale)
    /// Binary storage for HDC
    Binary(Vec<u64>), // 64 bits per element
    /// GPU buffer reference (opaque handle)
    GpuBuffer(u32), // WebGPU buffer ID
    /// Shared memory reference for WebWorkers
    SharedBuffer(u32), // SharedArrayBuffer ID
}

impl TensorStorage {
    /// Get storage size in bytes
    pub fn size_bytes(&self) -> usize {
        match self {
            TensorStorage::Cpu(v) => v.len() * 4,
            TensorStorage::Quantized(v, _) => v.len(),
            TensorStorage::Binary(v) => v.len() * 8,
            TensorStorage::GpuBuffer(_) => 0, // Unknown
            TensorStorage::SharedBuffer(_) => 0, // Unknown
        }
    }

    /// Check if storage is on CPU
    pub fn is_cpu(&self) -> bool {
        matches!(self, TensorStorage::Cpu(_) | TensorStorage::Quantized(_, _))
    }

    /// Check if storage is on GPU
    pub fn is_gpu(&self) -> bool {
        matches!(self, TensorStorage::GpuBuffer(_))
    }
}

/// Main tensor type for all compute operations
#[derive(Clone, Debug)]
pub struct Tensor {
    /// Shape of the tensor
    shape: Shape,
    /// Data type
    dtype: DType,
    /// Memory layout
    layout: Layout,
    /// Underlying storage
    storage: TensorStorage,
    /// Offset into storage (for views)
    offset: usize,
    /// Custom strides (for non-contiguous tensors)
    strides: Option<Vec<usize>>,
}

impl Tensor {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a new tensor with zeros
    pub fn zeros(shape: Shape, dtype: DType) -> Self {
        let numel = shape.numel();
        let storage = match dtype {
            DType::F32 | DType::F16 => TensorStorage::Cpu(vec![0.0; numel]),
            DType::I8 | DType::U8 => TensorStorage::Quantized(vec![0; numel], 1.0),
            DType::Binary => TensorStorage::Binary(vec![0; (numel + 63) / 64]),
        };
        Self {
            shape,
            dtype,
            layout: Layout::RowMajor,
            storage,
            offset: 0,
            strides: None,
        }
    }

    /// Create a new tensor with ones
    pub fn ones(shape: Shape, dtype: DType) -> Self {
        let numel = shape.numel();
        let storage = match dtype {
            DType::F32 | DType::F16 => TensorStorage::Cpu(vec![1.0; numel]),
            DType::I8 | DType::U8 => TensorStorage::Quantized(vec![1; numel], 1.0),
            DType::Binary => TensorStorage::Binary(vec![u64::MAX; (numel + 63) / 64]),
        };
        Self {
            shape,
            dtype,
            layout: Layout::RowMajor,
            storage,
            offset: 0,
            strides: None,
        }
    }

    /// Create a tensor from raw f32 data
    pub fn from_slice(data: &[f32], shape: Shape) -> Self {
        assert_eq!(
            data.len(),
            shape.numel(),
            "Data length {} doesn't match shape {}",
            data.len(),
            shape
        );
        Self {
            shape,
            dtype: DType::F32,
            layout: Layout::RowMajor,
            storage: TensorStorage::Cpu(data.to_vec()),
            offset: 0,
            strides: None,
        }
    }

    /// Create a tensor from a Vec<f32>
    pub fn from_vec(data: Vec<f32>, shape: Shape) -> Self {
        assert_eq!(
            data.len(),
            shape.numel(),
            "Data length {} doesn't match shape {}",
            data.len(),
            shape
        );
        Self {
            shape,
            dtype: DType::F32,
            layout: Layout::RowMajor,
            storage: TensorStorage::Cpu(data),
            offset: 0,
            strides: None,
        }
    }

    /// Create a random tensor (uniform [0, 1))
    pub fn rand(shape: Shape) -> Self {
        let numel = shape.numel();
        let mut data = vec![0.0f32; numel];
        // Simple LCG PRNG for reproducibility
        let mut seed = 0xDEADBEEFu64;
        for x in data.iter_mut() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *x = (seed >> 33) as f32 / (1u64 << 31) as f32;
        }
        Self::from_vec(data, shape)
    }

    /// Create a random normal tensor (mean=0, std=1)
    pub fn randn(shape: Shape) -> Self {
        let numel = shape.numel();
        let mut data = vec![0.0f32; numel];
        // Box-Muller transform for normal distribution
        let mut seed = 0xCAFEBABEu64;
        for i in (0..numel).step_by(2) {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u1 = (seed >> 33) as f32 / (1u64 << 31) as f32;
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u2 = (seed >> 33) as f32 / (1u64 << 31) as f32;

            let r = (-2.0 * u1.max(1e-10).ln()).sqrt();
            let theta = 2.0 * std::f32::consts::PI * u2;

            data[i] = r * theta.cos();
            if i + 1 < numel {
                data[i + 1] = r * theta.sin();
            }
        }
        Self::from_vec(data, shape)
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get tensor shape
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// Get data type
    pub fn dtype(&self) -> DType {
        self.dtype
    }

    /// Get number of elements
    pub fn numel(&self) -> usize {
        self.shape.numel()
    }

    /// Get memory layout
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Check if tensor is contiguous
    pub fn is_contiguous(&self) -> bool {
        self.strides.is_none() && self.offset == 0
    }

    /// Get underlying storage reference
    pub fn storage(&self) -> &TensorStorage {
        &self.storage
    }

    /// Get underlying data as f32 slice (if CPU storage)
    pub fn as_slice(&self) -> Option<&[f32]> {
        match &self.storage {
            TensorStorage::Cpu(data) => {
                if self.is_contiguous() {
                    Some(data.as_slice())
                } else {
                    Some(&data[self.offset..self.offset + self.numel()])
                }
            }
            _ => None,
        }
    }

    /// Get mutable underlying data (if CPU storage)
    pub fn as_mut_slice(&mut self) -> Option<&mut [f32]> {
        match &mut self.storage {
            TensorStorage::Cpu(data) => {
                if self.is_contiguous() {
                    Some(data.as_mut_slice())
                } else {
                    let start = self.offset;
                    let end = start + self.numel();
                    Some(&mut data[start..end])
                }
            }
            _ => None,
        }
    }

    /// Convert to Vec<f32> (copies data)
    pub fn to_vec(&self) -> Vec<f32> {
        match &self.storage {
            TensorStorage::Cpu(data) => {
                if self.is_contiguous() {
                    data.clone()
                } else {
                    data[self.offset..self.offset + self.numel()].to_vec()
                }
            }
            TensorStorage::Quantized(data, scale) => {
                data.iter().map(|&x| x as f32 * scale).collect()
            }
            _ => vec![0.0; self.numel()],
        }
    }

    // ========================================================================
    // Transformations
    // ========================================================================

    /// Reshape tensor (must have same numel)
    pub fn reshape(&self, new_shape: Shape) -> Self {
        assert_eq!(
            self.numel(),
            new_shape.numel(),
            "Cannot reshape {} to {}",
            self.shape,
            new_shape
        );
        Self {
            shape: new_shape,
            dtype: self.dtype,
            layout: self.layout,
            storage: self.storage.clone(),
            offset: self.offset,
            strides: None, // Reshaping makes it contiguous
        }
    }

    /// Transpose 2D tensor
    pub fn transpose(&self) -> Self {
        assert_eq!(self.shape.ndim(), 2, "Transpose only supports 2D tensors");
        let rows = self.shape.dim(0);
        let cols = self.shape.dim(1);

        // For non-contiguous transpose, we'd use strides
        // For simplicity, we copy and transpose
        if let TensorStorage::Cpu(data) = &self.storage {
            let mut new_data = vec![0.0f32; self.numel()];
            for i in 0..rows {
                for j in 0..cols {
                    new_data[j * rows + i] = data[i * cols + j];
                }
            }
            Self::from_vec(new_data, Shape::d2(cols, rows))
        } else {
            // For GPU tensors, return a strided view
            Self {
                shape: Shape::d2(cols, rows),
                dtype: self.dtype,
                layout: Layout::Strided,
                storage: self.storage.clone(),
                offset: self.offset,
                strides: Some(vec![1, rows]),
            }
        }
    }

    /// Convert to contiguous layout
    pub fn contiguous(&self) -> Self {
        if self.is_contiguous() {
            self.clone()
        } else {
            // Copy to new contiguous storage
            Self::from_vec(self.to_vec(), self.shape.clone())
        }
    }

    /// Quantize to i8
    pub fn quantize(&self) -> Self {
        let data = self.to_vec();
        let max_abs = data.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let scale = max_abs / 127.0;

        let quantized: Vec<i8> = data
            .iter()
            .map(|&x| (x / scale).clamp(-127.0, 127.0) as i8)
            .collect();

        Self {
            shape: self.shape.clone(),
            dtype: DType::I8,
            layout: Layout::RowMajor,
            storage: TensorStorage::Quantized(quantized, scale),
            offset: 0,
            strides: None,
        }
    }

    /// Dequantize to f32
    pub fn dequantize(&self) -> Self {
        Self::from_vec(self.to_vec(), self.shape.clone())
    }

    // ========================================================================
    // Size estimation
    // ========================================================================

    /// Estimate memory usage in bytes
    pub fn size_bytes(&self) -> usize {
        self.storage.size_bytes()
    }
}

/// LoRA adapter for efficient fine-tuning
#[derive(Clone, Debug)]
pub struct LoraAdapter {
    /// Low-rank A matrix (d x r)
    pub a: Tensor,
    /// Low-rank B matrix (r x d)
    pub b: Tensor,
    /// Scaling factor (alpha / rank)
    pub scaling: f32,
    /// Target layer name
    pub target: String,
}

impl LoraAdapter {
    /// Create a new LoRA adapter
    pub fn new(input_dim: usize, output_dim: usize, rank: usize, alpha: f32, target: &str) -> Self {
        // Initialize A with random normal, B with zeros (as per LoRA paper)
        let a = Tensor::randn(Shape::d2(input_dim, rank));
        let b = Tensor::zeros(Shape::d2(rank, output_dim), DType::F32);

        Self {
            a,
            b,
            scaling: alpha / rank as f32,
            target: target.to_string(),
        }
    }

    /// Get rank of this adapter
    pub fn rank(&self) -> usize {
        self.a.shape().dim(1)
    }

    /// Get input dimension
    pub fn input_dim(&self) -> usize {
        self.a.shape().dim(0)
    }

    /// Get output dimension
    pub fn output_dim(&self) -> usize {
        self.b.shape().dim(1)
    }
}

/// Workload classification for backend selection
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkloadType {
    /// Small matmul (< 1K elements)
    SmallMatmul,
    /// Medium matmul (1K - 100K elements)
    MediumMatmul,
    /// Large matmul (> 100K elements)
    LargeMatmul,
    /// Attention mechanism
    Attention,
    /// Element-wise operation
    Elementwise,
    /// Reduction (sum, mean, etc.)
    Reduction,
    /// Sparse operation (> 50% zeros)
    Sparse,
    /// Batch inference
    BatchInference,
    /// LoRA forward pass
    LoraForward,
}

impl WorkloadType {
    /// Classify a workload from tensor shapes
    pub fn classify(a: &Tensor, b: Option<&Tensor>) -> Self {
        let numel_a = a.numel();

        match b {
            Some(b_tensor) => {
                let numel_b = b_tensor.numel();
                let total = numel_a + numel_b;

                if a.shape().ndim() >= 3 && a.shape().dim(a.shape().ndim() - 2) == a.shape().dim(a.shape().ndim() - 1) {
                    // Likely attention (square inner dimensions)
                    WorkloadType::Attention
                } else if total < 1_000 {
                    WorkloadType::SmallMatmul
                } else if total < 100_000 {
                    WorkloadType::MediumMatmul
                } else {
                    WorkloadType::LargeMatmul
                }
            }
            None => {
                if numel_a < 1_000 {
                    WorkloadType::Elementwise
                } else {
                    WorkloadType::Reduction
                }
            }
        }
    }

    /// Get estimated FLOP count for this workload
    pub fn estimated_flops(&self, numel: usize) -> u64 {
        match self {
            WorkloadType::SmallMatmul => numel as u64 * 2,
            WorkloadType::MediumMatmul => numel as u64 * 2,
            WorkloadType::LargeMatmul => numel as u64 * 2,
            WorkloadType::Attention => numel as u64 * 4, // Q*K + softmax + *V
            WorkloadType::Elementwise => numel as u64,
            WorkloadType::Reduction => numel as u64,
            WorkloadType::Sparse => numel as u64 / 2, // Assumes 50% sparsity
            WorkloadType::BatchInference => numel as u64 * 10,
            WorkloadType::LoraForward => numel as u64 * 4, // A*x + B*(A*x)
        }
    }
}

/// Sparsity analysis for tensors
#[derive(Clone, Debug)]
pub struct SparsityInfo {
    /// Fraction of zero elements
    pub sparsity: f32,
    /// Is structured sparsity (blocks of zeros)?
    pub is_structured: bool,
    /// Block size if structured
    pub block_size: Option<usize>,
}

impl SparsityInfo {
    /// Analyze sparsity of a tensor
    pub fn analyze(tensor: &Tensor) -> Self {
        let data = tensor.to_vec();
        let total = data.len();
        let zeros = data.iter().filter(|&&x| x == 0.0).count();
        let sparsity = zeros as f32 / total as f32;

        // Check for structured sparsity (simple block check)
        let block_sizes = [4, 8, 16, 32];
        let mut is_structured = false;
        let mut detected_block = None;

        for &block in &block_sizes {
            if total >= block * 4 {
                let mut block_zeros = 0;
                let mut total_blocks = 0;

                for chunk in data.chunks(block) {
                    total_blocks += 1;
                    if chunk.iter().all(|&x| x == 0.0) {
                        block_zeros += 1;
                    }
                }

                // If > 30% of blocks are all zeros, consider structured
                if block_zeros as f32 / total_blocks as f32 > 0.3 {
                    is_structured = true;
                    detected_block = Some(block);
                    break;
                }
            }
        }

        Self {
            sparsity,
            is_structured,
            block_size: detected_block,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_creation() {
        let s = Shape::d2(3, 4);
        assert_eq!(s.numel(), 12);
        assert_eq!(s.ndim(), 2);
        assert_eq!(s.dim(0), 3);
        assert_eq!(s.dim(1), 4);
    }

    #[test]
    fn test_tensor_zeros() {
        let t = Tensor::zeros(Shape::d2(2, 3), DType::F32);
        assert_eq!(t.numel(), 6);
        let data = t.to_vec();
        assert!(data.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_tensor_from_slice() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let t = Tensor::from_slice(&data, Shape::d2(2, 3));
        assert_eq!(t.to_vec(), data);
    }

    #[test]
    fn test_matmul_compatible() {
        let s1 = Shape::d2(3, 4);
        let s2 = Shape::d2(4, 5);
        let s3 = Shape::d2(3, 5);

        assert!(s1.matmul_compatible(&s2));
        assert!(!s1.matmul_compatible(&s3));
    }

    #[test]
    fn test_transpose() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let t = Tensor::from_slice(&data, Shape::d2(2, 3));
        let t_t = t.transpose();

        assert_eq!(t_t.shape().dims(), &[3, 2]);
        assert_eq!(t_t.to_vec(), vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn test_workload_classification() {
        let small = Tensor::zeros(Shape::d2(10, 10), DType::F32);
        let large = Tensor::zeros(Shape::d2(1000, 1000), DType::F32);

        assert_eq!(
            WorkloadType::classify(&small, Some(&small)),
            WorkloadType::SmallMatmul
        );
        assert_eq!(
            WorkloadType::classify(&large, Some(&large)),
            WorkloadType::LargeMatmul
        );
    }

    #[test]
    fn test_quantization() {
        let data = vec![0.5, -0.5, 1.0, -1.0];
        let t = Tensor::from_slice(&data, Shape::d1(4));
        let q = t.quantize();

        assert_eq!(q.dtype(), DType::I8);

        // Dequantize and check approximate equality
        let dq = q.dequantize();
        let dq_data = dq.to_vec();
        for (a, b) in data.iter().zip(dq_data.iter()) {
            assert!((a - b).abs() < 0.01);
        }
    }

    #[test]
    fn test_lora_adapter() {
        let lora = LoraAdapter::new(128, 128, 4, 1.0, "attention.q");
        assert_eq!(lora.rank(), 4);
        assert_eq!(lora.input_dim(), 128);
        assert_eq!(lora.output_dim(), 128);
    }
}
