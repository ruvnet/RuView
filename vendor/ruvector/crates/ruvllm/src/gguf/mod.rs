//! GGUF Model Format Loader for RuvLLM
//!
//! This module provides support for loading llama.cpp compatible GGUF model files.
//! GGUF (GGML Universal File) is a binary format that stores model weights along
//! with metadata and tokenizer information.
//!
//! ## Features
//!
//! - **Parser**: Complete GGUF v3 format parsing with memory-mapped file support
//! - **Quantization**: All llama.cpp quantization types (Q4_0, Q4_K, Q8_0, etc.)
//! - **Streaming**: Chunk-based tensor loading for large models
//! - **Metadata**: Automatic extraction of model architecture parameters
//!
//! ## Supported Quantization Types
//!
//! | Type | Bits | Block Size | Memory (7B) | Quality |
//! |------|------|------------|-------------|---------|
//! | F32 | 32 | 1 | 28 GB | Best |
//! | F16 | 16 | 1 | 14 GB | Excellent |
//! | Q8_0 | 8.5 | 32 | 7.5 GB | Very Good |
//! | Q4_K | 4.5 | 256 | 4 GB | Good |
//! | Q4_0 | 4.5 | 32 | 4 GB | Acceptable |
//! | Q2_K | 2.6 | 256 | 2.3 GB | Experimental |
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::gguf::{GgufFile, GgufModelLoader};
//! use std::path::Path;
//!
//! // Load a GGUF file
//! let file = GgufFile::open(Path::new("model.gguf"))?;
//!
//! // Read model metadata
//! println!("Architecture: {:?}", file.architecture());
//! println!("Context length: {:?}", file.context_length());
//! println!("Layers: {:?}", file.layer_count());
//!
//! // Load a specific tensor
//! let weights = file.load_tensor_f32("model.layers.0.attention.wq.weight")?;
//!
//! // Or use memory-mapped loading for efficiency
//! let mmap_file = GgufFile::open_mmap(Path::new("model.gguf"))?;
//! let tensor_info = mmap_file.get_tensor("model.embed_tokens.weight").unwrap();
//! let data = mmap_file.tensor_data(&tensor_info);
//! ```
//!
//! ## Backend Integration
//!
//! The GGUF loader integrates seamlessly with RuvLLM backends:
//!
//! ```rust,ignore
//! use ruvllm::backends::LlmBackend;
//! use std::path::Path;
//!
//! // Load from GGUF file directly
//! let backend = LlmBackend::from_gguf(
//!     Path::new("model-Q4_K_M.gguf"),
//!     BackendConfig::default()
//! )?;
//! ```

pub mod loader;
pub mod model_init;
pub mod parser;
pub mod quantization;
pub mod tensors;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::FileExt;

use crate::backends::ModelArchitecture;
use crate::error::{Result, RuvLLMError};

pub use loader::{
    GgufLoader, LoadConfig, LoadProgress, LoadedTensor, LoadedWeights, ProgressCallback,
    StreamingLoader, TensorCategory, TensorNameMapper,
};
pub use model_init::{
    LayerWeights, ModelInitializer, ModelWeights, ProgressModelBuilder, QuantizedWeight,
    WeightTensor,
};
pub use parser::{parse_header, parse_metadata, parse_tensor_infos, GgufHeader, GgufValue};
pub use quantization::{dequantize_block, GgufQuantType, QuantizedTensor};
pub use tensors::TensorInfo;

// ============================================================================
// GGUF File Magic and Constants
// ============================================================================

/// GGUF magic number (little-endian: "GGUF")
pub const GGUF_MAGIC: u32 = 0x46554747;

/// Current GGUF version supported
pub const GGUF_VERSION: u32 = 3;

/// Default alignment for tensor data
pub const DEFAULT_ALIGNMENT: usize = 32;

// ============================================================================
// GgufFile - Main Interface
// ============================================================================

/// GGUF file reader with optional memory-mapping support.
///
/// This struct provides the main interface for reading GGUF model files.
/// It supports both traditional file I/O and memory-mapped access for
/// improved performance with large models.
///
/// # Memory Mapping
///
/// Memory mapping is recommended for large models as it allows the OS
/// to manage memory efficiently and enables lazy loading of tensor data.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::gguf::GgufFile;
/// use std::path::Path;
///
/// // Standard file access
/// let file = GgufFile::open(Path::new("model.gguf"))?;
///
/// // Memory-mapped access (recommended for large models)
/// let mmap_file = GgufFile::open_mmap(Path::new("model.gguf"))?;
/// ```
pub struct GgufFile {
    /// GGUF header information
    pub header: GgufHeader,
    /// Key-value metadata
    pub metadata: HashMap<String, GgufValue>,
    /// Tensor information array
    pub tensors: Vec<TensorInfo>,
    /// File path
    path: std::path::PathBuf,
    /// Optional memory-mapped data
    mmap: Option<MmapData>,
    /// Data section offset in file
    data_offset: u64,
    /// Alignment for tensor data
    alignment: usize,
}

/// Memory-mapped file data
struct MmapData {
    /// Memory-mapped region
    #[cfg(feature = "mmap")]
    mmap: memmap2::Mmap,
    #[cfg(not(feature = "mmap"))]
    data: Vec<u8>,
}

impl GgufFile {
    /// Open a GGUF file for reading.
    ///
    /// This method reads the file header, metadata, and tensor information
    /// but does not load tensor data into memory.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the GGUF file
    ///
    /// # Returns
    ///
    /// A `GgufFile` instance ready for tensor loading
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened
    /// - The file is not a valid GGUF file
    /// - The GGUF version is not supported
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .map_err(|e| RuvLLMError::Model(format!("Failed to open GGUF file: {}", e)))?;
        let mut reader = BufReader::new(file);

        // Parse header
        let header = parse_header(&mut reader)?;

        // Validate magic and version
        if header.magic != GGUF_MAGIC {
            return Err(RuvLLMError::Model(format!(
                "Invalid GGUF magic: expected 0x{:08X}, got 0x{:08X}",
                GGUF_MAGIC, header.magic
            )));
        }

        if header.version != GGUF_VERSION && header.version != 2 {
            return Err(RuvLLMError::Model(format!(
                "Unsupported GGUF version: {} (supported: 2, 3)",
                header.version
            )));
        }

        // Parse metadata
        let metadata = parse_metadata(&mut reader, header.metadata_kv_count)?;

        // Get alignment from metadata or use default
        let alignment = metadata
            .get("general.alignment")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_ALIGNMENT);

        // Parse tensor infos
        let tensors = parse_tensor_infos(&mut reader, header.tensor_count)?;

        // Calculate data offset (aligned)
        let current_pos = reader
            .stream_position()
            .map_err(|e| RuvLLMError::Model(format!("Failed to get stream position: {}", e)))?;
        let data_offset = align_offset(current_pos, alignment as u64);

        Ok(Self {
            header,
            metadata,
            tensors,
            path: path.to_path_buf(),
            mmap: None,
            data_offset,
            alignment,
        })
    }

    /// Open a GGUF file with memory mapping.
    ///
    /// Memory mapping provides efficient access to tensor data for large
    /// models by letting the operating system manage memory paging.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the GGUF file
    ///
    /// # Returns
    ///
    /// A `GgufFile` instance with memory-mapped tensor data access
    ///
    /// # Errors
    ///
    /// Returns an error if memory mapping fails or the file is invalid
    #[cfg(feature = "mmap")]
    pub fn open_mmap(path: &Path) -> Result<Self> {
        let mut gguf = Self::open(path)?;

        let file = File::open(path)
            .map_err(|e| RuvLLMError::Model(format!("Failed to open file for mmap: {}", e)))?;

        let mmap = unsafe {
            memmap2::Mmap::map(&file)
                .map_err(|e| RuvLLMError::Model(format!("Failed to memory map file: {}", e)))?
        };

        gguf.mmap = Some(MmapData { mmap });
        Ok(gguf)
    }

    /// Open with memory mapping (fallback when mmap feature is disabled)
    #[cfg(not(feature = "mmap"))]
    pub fn open_mmap(path: &Path) -> Result<Self> {
        let mut gguf = Self::open(path)?;

        // Read entire file into memory as fallback
        let data = std::fs::read(path)
            .map_err(|e| RuvLLMError::Model(format!("Failed to read file: {}", e)))?;

        gguf.mmap = Some(MmapData { data });
        Ok(gguf)
    }

    /// Get tensor information by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The tensor name (e.g., "model.layers.0.attention.wq.weight")
    ///
    /// # Returns
    ///
    /// Reference to the tensor info if found
    pub fn get_tensor(&self, name: &str) -> Option<&TensorInfo> {
        self.tensors.iter().find(|t| t.name == name)
    }

    /// Load tensor data as FP32 (dequantizing if necessary).
    ///
    /// This method reads the tensor from disk and converts it to FP32
    /// format, dequantizing quantized data as needed.
    ///
    /// # Arguments
    ///
    /// * `name` - The tensor name
    ///
    /// # Returns
    ///
    /// Vector of FP32 values
    ///
    /// # Errors
    ///
    /// Returns an error if the tensor is not found or cannot be read
    pub fn load_tensor_f32(&self, name: &str) -> Result<Vec<f32>> {
        let info = self
            .get_tensor(name)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Tensor not found: {}", name)))?;

        let raw_data = self.read_tensor_bytes(info)?;
        let num_elements: usize = info.shape.iter().product();

        // Dequantize based on type
        let output = quantization::dequantize_tensor(&raw_data, info.dtype, num_elements)?;
        Ok(output)
    }

    /// Load tensor as a quantized tensor (preserving quantization).
    ///
    /// This method reads the tensor without dequantizing, preserving
    /// the original quantization format for efficient inference.
    ///
    /// # Arguments
    ///
    /// * `name` - The tensor name
    ///
    /// # Returns
    ///
    /// A `QuantizedTensor` containing the raw quantized data
    pub fn load_tensor_quantized(&self, name: &str) -> Result<QuantizedTensor> {
        let info = self
            .get_tensor(name)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Tensor not found: {}", name)))?;

        let data = self.read_tensor_bytes(info)?;
        let num_elements: usize = info.shape.iter().product();

        Ok(QuantizedTensor {
            data,
            dtype: info.dtype,
            shape: info.shape.clone(),
            num_elements,
        })
    }

    /// Get direct access to tensor data bytes (for memory-mapped files).
    ///
    /// This method returns a slice to the raw tensor data without copying.
    /// Only available when the file was opened with `open_mmap`.
    ///
    /// # Arguments
    ///
    /// * `info` - Tensor information
    ///
    /// # Returns
    ///
    /// Slice of raw bytes for the tensor
    ///
    /// # Panics
    ///
    /// Panics if the file was not opened with memory mapping
    pub fn tensor_data(&self, info: &TensorInfo) -> &[u8] {
        let mmap = self.mmap.as_ref().expect("File not memory-mapped");
        let start = (self.data_offset + info.offset) as usize;
        let end = start + info.byte_size();

        #[cfg(feature = "mmap")]
        {
            &mmap.mmap[start..end]
        }
        #[cfg(not(feature = "mmap"))]
        {
            &mmap.data[start..end]
        }
    }

    /// Stream tensor data in chunks for memory-efficient processing.
    ///
    /// This method processes the tensor in chunks, calling the provided
    /// callback for each chunk. Useful for very large tensors.
    ///
    /// # Arguments
    ///
    /// * `name` - The tensor name
    /// * `chunk_size` - Number of FP32 elements per chunk
    /// * `f` - Callback function receiving each chunk
    ///
    /// # Returns
    ///
    /// Ok(()) if all chunks were processed successfully
    pub fn stream_tensor<F>(&self, name: &str, chunk_size: usize, mut f: F) -> Result<()>
    where
        F: FnMut(&[f32]) -> Result<()>,
    {
        let info = self
            .get_tensor(name)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Tensor not found: {}", name)))?;

        let _num_elements: usize = info.shape.iter().product();

        // For simple types (F32, F16), we can stream directly
        match info.dtype {
            GgufQuantType::F32 => {
                self.stream_f32_tensor(info, chunk_size, &mut f)?;
            }
            GgufQuantType::F16 => {
                self.stream_f16_tensor(info, chunk_size, &mut f)?;
            }
            _ => {
                // For quantized types, load and dequantize in block-aligned chunks
                let block_size = info.dtype.block_size();
                let aligned_chunk = ((chunk_size + block_size - 1) / block_size) * block_size;
                let full_data = self.load_tensor_f32(name)?;

                for chunk in full_data.chunks(aligned_chunk) {
                    f(chunk)?;
                }
            }
        }

        Ok(())
    }

    // ========================================================================
    // Metadata Extraction Methods
    // ========================================================================

    /// Get the model architecture (llama, mistral, phi, etc.).
    pub fn architecture(&self) -> Option<&str> {
        self.metadata
            .get("general.architecture")
            .and_then(|v| v.as_str())
    }

    /// Get the model architecture as enum.
    pub fn architecture_type(&self) -> Option<ModelArchitecture> {
        self.architecture()
            .and_then(|arch| match arch.to_lowercase().as_str() {
                "llama" => Some(ModelArchitecture::Llama),
                "mistral" => Some(ModelArchitecture::Mistral),
                "phi" | "phi2" | "phi3" => Some(ModelArchitecture::Phi),
                "qwen" | "qwen2" => Some(ModelArchitecture::Qwen),
                "gemma" => Some(ModelArchitecture::Gemma),
                _ => None,
            })
    }

    /// Get the context length (max sequence length).
    pub fn context_length(&self) -> Option<usize> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.context_length", arch))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }

    /// Get the embedding dimension (hidden size).
    pub fn embedding_length(&self) -> Option<usize> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.embedding_length", arch))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }

    /// Get the number of attention heads.
    pub fn head_count(&self) -> Option<usize> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.attention.head_count", arch))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }

    /// Get the number of key-value heads (for GQA/MQA).
    pub fn head_count_kv(&self) -> Option<usize> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.attention.head_count_kv", arch))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .or_else(|| self.head_count()) // Default to head_count if not specified
    }

    /// Get the number of layers.
    pub fn layer_count(&self) -> Option<usize> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.block_count", arch))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }

    /// Get the vocabulary size.
    pub fn vocab_size(&self) -> Option<usize> {
        // Try tokenizer.ggml.model first
        self.metadata
            .get("tokenizer.ggml.tokens")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .or_else(|| {
                let arch = self.architecture()?;
                self.metadata
                    .get(&format!("{}.vocab_size", arch))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
            })
    }

    /// Get the RoPE frequency base.
    pub fn rope_freq_base(&self) -> Option<f32> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.rope.freq_base", arch))
            .and_then(|v| v.as_f32())
    }

    /// Get the RoPE dimension count.
    pub fn rope_dimension_count(&self) -> Option<usize> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.rope.dimension_count", arch))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }

    /// Get the feed-forward hidden dimension.
    pub fn feed_forward_length(&self) -> Option<usize> {
        let arch = self.architecture()?;
        self.metadata
            .get(&format!("{}.feed_forward_length", arch))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }

    /// Get the model name.
    pub fn model_name(&self) -> Option<&str> {
        self.metadata.get("general.name").and_then(|v| v.as_str())
    }

    /// Get the model author.
    pub fn author(&self) -> Option<&str> {
        self.metadata.get("general.author").and_then(|v| v.as_str())
    }

    /// Get the quantization type description.
    pub fn quantization_version(&self) -> Option<&str> {
        self.metadata
            .get("general.quantization_version")
            .and_then(|v| v.as_str())
    }

    /// Get all tensor names.
    pub fn tensor_names(&self) -> impl Iterator<Item = &str> {
        self.tensors.iter().map(|t| t.name.as_str())
    }

    /// Get the total size of all tensors in bytes.
    pub fn total_tensor_size(&self) -> usize {
        self.tensors.iter().map(|t| t.byte_size()).sum()
    }

    // ========================================================================
    // Private Helper Methods
    // ========================================================================

    fn read_tensor_bytes(&self, info: &TensorInfo) -> Result<Vec<u8>> {
        if let Some(ref mmap) = self.mmap {
            let start = (self.data_offset + info.offset) as usize;
            let end = start + info.byte_size();

            #[cfg(feature = "mmap")]
            let data = mmap.mmap[start..end].to_vec();
            #[cfg(not(feature = "mmap"))]
            let data = mmap.data[start..end].to_vec();

            return Ok(data);
        }

        // Read from file
        let mut file = File::open(&self.path)
            .map_err(|e| RuvLLMError::Model(format!("Failed to open file: {}", e)))?;

        file.seek(SeekFrom::Start(self.data_offset + info.offset))
            .map_err(|e| RuvLLMError::Model(format!("Failed to seek: {}", e)))?;

        let mut data = vec![0u8; info.byte_size()];
        file.read_exact(&mut data)
            .map_err(|e| RuvLLMError::Model(format!("Failed to read tensor: {}", e)))?;

        Ok(data)
    }

    fn stream_f32_tensor<F>(&self, info: &TensorInfo, chunk_size: usize, f: &mut F) -> Result<()>
    where
        F: FnMut(&[f32]) -> Result<()>,
    {
        let num_elements: usize = info.shape.iter().product();
        let mut file = File::open(&self.path)
            .map_err(|e| RuvLLMError::Model(format!("Failed to open file: {}", e)))?;

        file.seek(SeekFrom::Start(self.data_offset + info.offset))
            .map_err(|e| RuvLLMError::Model(format!("Failed to seek: {}", e)))?;

        let mut processed = 0;
        let mut buffer = vec![0u8; chunk_size * 4];

        while processed < num_elements {
            let remaining = num_elements - processed;
            let this_chunk = remaining.min(chunk_size);
            let byte_count = this_chunk * 4;

            file.read_exact(&mut buffer[..byte_count])
                .map_err(|e| RuvLLMError::Model(format!("Failed to read: {}", e)))?;

            let floats: Vec<f32> = buffer[..byte_count]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();

            f(&floats)?;
            processed += this_chunk;
        }

        Ok(())
    }

    fn stream_f16_tensor<F>(&self, info: &TensorInfo, chunk_size: usize, f: &mut F) -> Result<()>
    where
        F: FnMut(&[f32]) -> Result<()>,
    {
        let num_elements: usize = info.shape.iter().product();
        let mut file = File::open(&self.path)
            .map_err(|e| RuvLLMError::Model(format!("Failed to open file: {}", e)))?;

        file.seek(SeekFrom::Start(self.data_offset + info.offset))
            .map_err(|e| RuvLLMError::Model(format!("Failed to seek: {}", e)))?;

        let mut processed = 0;
        let mut buffer = vec![0u8; chunk_size * 2];

        while processed < num_elements {
            let remaining = num_elements - processed;
            let this_chunk = remaining.min(chunk_size);
            let byte_count = this_chunk * 2;

            file.read_exact(&mut buffer[..byte_count])
                .map_err(|e| RuvLLMError::Model(format!("Failed to read: {}", e)))?;

            let floats: Vec<f32> = buffer[..byte_count]
                .chunks_exact(2)
                .map(|b| {
                    let bits = u16::from_le_bytes([b[0], b[1]]);
                    half::f16::from_bits(bits).to_f32()
                })
                .collect();

            f(&floats)?;
            processed += this_chunk;
        }

        Ok(())
    }
}

// ============================================================================
// Model Loader for Backend Integration
// ============================================================================

/// GGUF model loader for backend integration.
///
/// This struct wraps a `GgufFile` and provides higher-level methods
/// for model loading and configuration extraction.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::gguf::GgufModelLoader;
/// use std::path::Path;
///
/// let loader = GgufModelLoader::load(Path::new("model.gguf"))?;
///
/// println!("Architecture: {:?}", loader.architecture());
/// println!("Config: {:?}", loader.config());
///
/// // Convert to Candle model
/// let model = loader.to_candle_model(&device)?;
/// ```
pub struct GgufModelLoader {
    file: GgufFile,
}

impl GgufModelLoader {
    /// Load a GGUF file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the GGUF file
    pub fn load(path: &Path) -> Result<Self> {
        let file = GgufFile::open_mmap(path)?;
        Ok(Self { file })
    }

    /// Get the underlying GGUF file.
    pub fn file(&self) -> &GgufFile {
        &self.file
    }

    /// Get the model architecture.
    pub fn architecture(&self) -> Option<ModelArchitecture> {
        self.file.architecture_type()
    }

    /// Get the model configuration.
    pub fn config(&self) -> ModelConfig {
        ModelConfig {
            architecture: self.file.architecture().map(|s| s.to_string()),
            context_length: self.file.context_length(),
            embedding_length: self.file.embedding_length(),
            head_count: self.file.head_count(),
            head_count_kv: self.file.head_count_kv(),
            layer_count: self.file.layer_count(),
            vocab_size: self.file.vocab_size(),
            rope_freq_base: self.file.rope_freq_base(),
            feed_forward_length: self.file.feed_forward_length(),
        }
    }

    /// Get list of tensor names that match a pattern.
    pub fn find_tensors(&self, pattern: &str) -> Vec<&str> {
        self.file
            .tensor_names()
            .filter(|name| name.contains(pattern))
            .collect()
    }

    /// Check if this is a quantized model.
    pub fn is_quantized(&self) -> bool {
        self.file.tensors.iter().any(|t| t.dtype.is_quantized())
    }

    /// Get the primary quantization type.
    pub fn quantization_type(&self) -> Option<GgufQuantType> {
        // Find the most common quantization type among weight tensors
        let mut counts: HashMap<GgufQuantType, usize> = HashMap::new();

        for tensor in &self.file.tensors {
            if tensor.name.contains("weight") {
                *counts.entry(tensor.dtype).or_insert(0) += 1;
            }
        }

        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(dtype, _)| dtype)
    }

    /// Convert to a Candle-compatible model (stub for integration).
    #[cfg(feature = "candle")]
    pub fn to_candle_model(&self, _device: &candle_core::Device) -> Result<()> {
        // This would be implemented based on the specific Candle model architecture
        Err(RuvLLMError::Model(
            "Candle model conversion not yet implemented".to_string(),
        ))
    }
}

/// Model configuration extracted from GGUF metadata.
#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    /// Model architecture name
    pub architecture: Option<String>,
    /// Maximum context/sequence length
    pub context_length: Option<usize>,
    /// Hidden/embedding dimension
    pub embedding_length: Option<usize>,
    /// Number of attention heads
    pub head_count: Option<usize>,
    /// Number of key-value heads (for GQA)
    pub head_count_kv: Option<usize>,
    /// Number of transformer layers
    pub layer_count: Option<usize>,
    /// Vocabulary size
    pub vocab_size: Option<usize>,
    /// RoPE frequency base
    pub rope_freq_base: Option<f32>,
    /// Feed-forward hidden dimension
    pub feed_forward_length: Option<usize>,
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Align an offset to the specified alignment.
#[inline]
fn align_offset(offset: u64, alignment: u64) -> u64 {
    (offset + alignment - 1) / alignment * alignment
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_offset() {
        assert_eq!(align_offset(0, 32), 0);
        assert_eq!(align_offset(1, 32), 32);
        assert_eq!(align_offset(31, 32), 32);
        assert_eq!(align_offset(32, 32), 32);
        assert_eq!(align_offset(33, 32), 64);
    }

    #[test]
    fn test_gguf_magic() {
        // "GGUF" in little-endian
        assert_eq!(GGUF_MAGIC, 0x46554747);
        let bytes = GGUF_MAGIC.to_le_bytes();
        assert_eq!(&bytes, b"GGUF");
    }

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert!(config.architecture.is_none());
        assert!(config.context_length.is_none());
    }
}
