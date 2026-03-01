//! Core types for compute operations
//!
//! These types work without the WebGPU feature and provide
//! the interface for compute operations.

use serde::{Serialize, Deserialize};

/// Matrix storage format
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatrixLayout {
    /// Row-major storage (C-style)
    RowMajor,
    /// Column-major storage (Fortran-style)
    ColMajor,
}

impl Default for MatrixLayout {
    fn default() -> Self {
        Self::RowMajor
    }
}

/// Data type for compute operations
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// 32-bit floating point
    F32,
    /// 16-bit floating point
    F16,
    /// 16-bit brain floating point
    BF16,
    /// 8-bit signed integer
    I8,
    /// 8-bit unsigned integer
    U8,
    /// 4-bit integer (packed, 2 per byte)
    I4,
}

impl DataType {
    /// Get size in bytes
    pub fn size_bytes(&self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 | Self::BF16 => 2,
            Self::I8 | Self::U8 => 1,
            Self::I4 => 1, // 2 values per byte, but minimum addressable is 1
        }
    }

    /// Check if this is a floating point type
    pub fn is_float(&self) -> bool {
        matches!(self, Self::F32 | Self::F16 | Self::BF16)
    }

    /// Check if this is a quantized type
    pub fn is_quantized(&self) -> bool {
        matches!(self, Self::I8 | Self::U8 | Self::I4)
    }
}

/// Tensor descriptor for GPU buffers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TensorDescriptor {
    /// Shape of the tensor
    pub shape: Vec<usize>,
    /// Data type
    pub dtype: DataType,
    /// Storage layout
    pub layout: MatrixLayout,
    /// Stride between elements (None = contiguous)
    pub strides: Option<Vec<usize>>,
}

impl TensorDescriptor {
    /// Create a new contiguous tensor descriptor
    pub fn new(shape: Vec<usize>, dtype: DataType) -> Self {
        Self {
            shape,
            dtype,
            layout: MatrixLayout::RowMajor,
            strides: None,
        }
    }

    /// Total number of elements
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Size in bytes
    pub fn size_bytes(&self) -> usize {
        self.numel() * self.dtype.size_bytes()
    }

    /// Check if tensor is contiguous in memory
    pub fn is_contiguous(&self) -> bool {
        self.strides.is_none()
    }

    /// Get number of dimensions
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// Create 2D matrix descriptor
    pub fn matrix(rows: usize, cols: usize, dtype: DataType) -> Self {
        Self::new(vec![rows, cols], dtype)
    }

    /// Create 3D tensor descriptor (batch, seq, hidden)
    pub fn tensor3d(batch: usize, seq: usize, hidden: usize, dtype: DataType) -> Self {
        Self::new(vec![batch, seq, hidden], dtype)
    }
}

/// LoRA adapter configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoraConfig {
    /// Rank of the adaptation (typically 2-64)
    pub rank: usize,
    /// Alpha scaling factor
    pub alpha: f32,
    /// Input dimension
    pub in_dim: usize,
    /// Output dimension
    pub out_dim: usize,
    /// Dropout rate (0.0 = no dropout)
    pub dropout: f32,
}

impl LoraConfig {
    /// Create new LoRA config
    pub fn new(rank: usize, in_dim: usize, out_dim: usize) -> Self {
        Self {
            rank,
            alpha: rank as f32, // Default alpha = rank
            in_dim,
            out_dim,
            dropout: 0.0,
        }
    }

    /// Scaling factor for LoRA output
    pub fn scaling(&self) -> f32 {
        self.alpha / self.rank as f32
    }

    /// Size of A matrix (in_dim x rank)
    pub fn a_size(&self) -> usize {
        self.in_dim * self.rank
    }

    /// Size of B matrix (rank x out_dim)
    pub fn b_size(&self) -> usize {
        self.rank * self.out_dim
    }
}

/// Attention configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttentionConfig {
    /// Number of attention heads
    pub num_heads: usize,
    /// Dimension per head
    pub head_dim: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
    /// Use causal (autoregressive) masking
    pub causal: bool,
    /// Attention dropout rate
    pub dropout: f32,
    /// Scale factor (None = 1/sqrt(head_dim))
    pub scale: Option<f32>,
    /// Use flash attention algorithm
    pub flash: bool,
}

impl AttentionConfig {
    /// Create new attention config
    pub fn new(num_heads: usize, head_dim: usize, max_seq_len: usize) -> Self {
        Self {
            num_heads,
            head_dim,
            max_seq_len,
            causal: true,
            dropout: 0.0,
            scale: None,
            flash: true,
        }
    }

    /// Total hidden dimension (num_heads * head_dim)
    pub fn hidden_dim(&self) -> usize {
        self.num_heads * self.head_dim
    }

    /// Get attention scale factor
    pub fn get_scale(&self) -> f32 {
        self.scale.unwrap_or_else(|| 1.0 / (self.head_dim as f32).sqrt())
    }
}

/// Quantization configuration for int8/int4 operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantConfig {
    /// Target data type
    pub dtype: DataType,
    /// Per-channel vs per-tensor quantization
    pub per_channel: bool,
    /// Symmetric quantization (zero_point = 0)
    pub symmetric: bool,
    /// Group size for group quantization (0 = no grouping)
    pub group_size: usize,
}

impl Default for QuantConfig {
    fn default() -> Self {
        Self {
            dtype: DataType::I8,
            per_channel: true,
            symmetric: true,
            group_size: 0,
        }
    }
}

impl QuantConfig {
    /// Create int8 quantization config
    pub fn int8() -> Self {
        Self::default()
    }

    /// Create int4 quantization config with grouping
    pub fn int4_grouped(group_size: usize) -> Self {
        Self {
            dtype: DataType::I4,
            per_channel: false,
            symmetric: true,
            group_size,
        }
    }
}

/// Buffer usage flags for GPU memory
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferUsage {
    pub map_read: bool,
    pub map_write: bool,
    pub copy_src: bool,
    pub copy_dst: bool,
    pub storage: bool,
    pub uniform: bool,
}

impl Default for BufferUsage {
    fn default() -> Self {
        Self {
            map_read: false,
            map_write: false,
            copy_src: false,
            copy_dst: true,
            storage: true,
            uniform: false,
        }
    }
}

impl BufferUsage {
    /// Buffer for staging CPU->GPU transfers
    pub fn staging_upload() -> Self {
        Self {
            map_read: false,
            map_write: true,
            copy_src: true,
            copy_dst: false,
            storage: false,
            uniform: false,
        }
    }

    /// Buffer for staging GPU->CPU transfers
    pub fn staging_download() -> Self {
        Self {
            map_read: true,
            map_write: false,
            copy_src: false,
            copy_dst: true,
            storage: false,
            uniform: false,
        }
    }

    /// Buffer for compute shader storage
    pub fn storage() -> Self {
        Self {
            map_read: false,
            map_write: false,
            copy_src: true,
            copy_dst: true,
            storage: true,
            uniform: false,
        }
    }

    /// Buffer for uniform data (small, read-only)
    pub fn uniform() -> Self {
        Self {
            map_read: false,
            map_write: false,
            copy_src: false,
            copy_dst: true,
            storage: false,
            uniform: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_size() {
        assert_eq!(DataType::F32.size_bytes(), 4);
        assert_eq!(DataType::F16.size_bytes(), 2);
        assert_eq!(DataType::I8.size_bytes(), 1);
    }

    #[test]
    fn test_tensor_descriptor() {
        let desc = TensorDescriptor::matrix(1024, 768, DataType::F32);
        assert_eq!(desc.numel(), 1024 * 768);
        assert_eq!(desc.size_bytes(), 1024 * 768 * 4);
        assert_eq!(desc.ndim(), 2);
    }

    #[test]
    fn test_lora_config() {
        let config = LoraConfig::new(4, 768, 768);
        assert_eq!(config.rank, 4);
        assert!((config.scaling() - 1.0).abs() < 0.001);
        assert_eq!(config.a_size(), 768 * 4);
        assert_eq!(config.b_size(), 4 * 768);
    }

    #[test]
    fn test_attention_config() {
        let config = AttentionConfig::new(12, 64, 4096);
        assert_eq!(config.hidden_dim(), 768);
        assert!((config.get_scale() - 0.125).abs() < 0.001);
    }
}
