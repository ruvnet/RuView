//! Kernel Pack Manifest (kernels.json)
//!
//! Defines the manifest schema for kernel packs, including kernel metadata,
//! resource limits, platform requirements, and versioning.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kernel pack manifest (kernels.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelManifest {
    /// JSON schema URL
    #[serde(rename = "$schema", default)]
    pub schema: String,

    /// Manifest version (semver)
    pub version: String,

    /// Pack name
    pub name: String,

    /// Pack description
    pub description: String,

    /// Minimum runtime version required
    pub min_runtime_version: String,

    /// Maximum runtime version supported
    pub max_runtime_version: String,

    /// Creation timestamp (ISO 8601)
    pub created_at: String,

    /// Author information
    pub author: AuthorInfo,

    /// List of kernels in the pack
    pub kernels: Vec<KernelInfo>,

    /// Fallback mappings (kernel_id -> fallback_kernel_id)
    #[serde(default)]
    pub fallbacks: HashMap<String, String>,
}

/// Author information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorInfo {
    /// Author name
    pub name: String,

    /// Contact email
    pub email: String,

    /// Ed25519 public signing key (base64 or hex encoded)
    pub signing_key: String,
}

/// Individual kernel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelInfo {
    /// Unique kernel identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Kernel category
    pub category: KernelCategory,

    /// Path to WASM file relative to pack root
    pub path: String,

    /// SHA256 hash of the WASM file (format: "sha256:...")
    pub hash: String,

    /// Entry point function name
    pub entry_point: String,

    /// Input tensor specifications
    pub inputs: Vec<TensorSpec>,

    /// Output tensor specifications
    pub outputs: Vec<TensorSpec>,

    /// Kernel-specific parameters
    #[serde(default)]
    pub params: HashMap<String, KernelParam>,

    /// Resource limits
    pub resource_limits: ResourceLimits,

    /// Platform-specific configurations
    #[serde(default)]
    pub platforms: HashMap<String, PlatformConfig>,

    /// Benchmark results
    #[serde(default)]
    pub benchmarks: HashMap<String, BenchmarkResult>,
}

/// Kernel categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelCategory {
    /// Positional encoding (RoPE, etc.)
    PositionalEncoding,
    /// Normalization (RMSNorm, LayerNorm, etc.)
    Normalization,
    /// Activation functions (SwiGLU, GELU, etc.)
    Activation,
    /// KV cache operations (quantize, dequantize)
    KvCache,
    /// Adapter operations (LoRA, etc.)
    Adapter,
    /// Attention mechanisms
    Attention,
    /// Custom/other operations
    Custom,
}

impl Default for KernelCategory {
    fn default() -> Self {
        KernelCategory::Custom
    }
}

/// Tensor specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorSpec {
    /// Tensor name
    pub name: String,

    /// Data type
    pub dtype: DataType,

    /// Shape specification (symbolic dimensions like "batch", "seq", numeric for fixed)
    pub shape: Vec<ShapeDim>,
}

/// Shape dimension (can be symbolic or numeric)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ShapeDim {
    /// Symbolic dimension (e.g., "batch", "seq", "heads")
    Symbolic(String),
    /// Fixed numeric dimension
    Fixed(usize),
}

/// Data types supported by kernels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    /// 32-bit float
    F32,
    /// 16-bit float (half precision)
    F16,
    /// Brain float 16
    Bf16,
    /// 8-bit integer (signed)
    I8,
    /// 8-bit unsigned integer
    U8,
    /// 32-bit integer
    I32,
    /// Quantized 4-bit
    Q4,
    /// Quantized 8-bit
    Q8,
}

impl DataType {
    /// Get size in bytes for this data type
    pub fn size_bytes(&self) -> usize {
        match self {
            DataType::F32 | DataType::I32 => 4,
            DataType::F16 | DataType::Bf16 => 2,
            DataType::I8 | DataType::U8 | DataType::Q8 => 1,
            DataType::Q4 => 1, // Packed, 2 values per byte
        }
    }
}

/// Kernel parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelParam {
    /// Parameter data type
    #[serde(rename = "type")]
    pub param_type: ParamType,

    /// Default value
    pub default: serde_json::Value,

    /// Optional minimum value
    #[serde(default)]
    pub min: Option<serde_json::Value>,

    /// Optional maximum value
    #[serde(default)]
    pub max: Option<serde_json::Value>,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
}

/// Parameter types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    F32,
    F64,
    I32,
    I64,
    U32,
    U64,
    Bool,
}

/// Resource limits for kernel execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum WASM memory pages (64KB each)
    pub max_memory_pages: u32,

    /// Maximum epoch ticks before interruption
    pub max_epoch_ticks: u64,

    /// Maximum table elements
    pub max_table_elements: u32,

    /// Optional: Maximum stack size in bytes
    #[serde(default)]
    pub max_stack_size: Option<usize>,

    /// Optional: Maximum globals
    #[serde(default)]
    pub max_globals: Option<u32>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        ResourceLimits {
            max_memory_pages: 256,    // 16MB
            max_epoch_ticks: 1000,    // ~10 seconds at 10ms/tick
            max_table_elements: 1024, // Function pointers
            max_stack_size: None,
            max_globals: None,
        }
    }
}

/// Platform-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    /// Minimum version of the runtime
    pub min_version: String,

    /// Required WASM features
    #[serde(default)]
    pub features: Vec<String>,

    /// Whether AOT compilation is available
    #[serde(default)]
    pub aot_available: bool,
}

/// Benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Latency in microseconds
    pub latency_us: u64,

    /// Throughput in GFLOPS
    pub throughput_gflops: f64,
}

/// Kernel invocation descriptor passed to WASM
///
/// This is the C-compatible struct passed to kernels to describe
/// memory layout and tensor locations.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KernelDescriptor {
    /// Input tensor A offset in linear memory
    pub input_a_offset: u32,
    /// Input tensor A size in bytes
    pub input_a_size: u32,
    /// Input tensor B offset (0 if unused)
    pub input_b_offset: u32,
    /// Input tensor B size in bytes
    pub input_b_size: u32,
    /// Output tensor offset
    pub output_offset: u32,
    /// Output tensor size in bytes
    pub output_size: u32,
    /// Scratch space offset
    pub scratch_offset: u32,
    /// Scratch space size in bytes
    pub scratch_size: u32,
    /// Kernel-specific parameters offset
    pub params_offset: u32,
    /// Kernel-specific parameters size
    pub params_size: u32,
}

impl KernelDescriptor {
    /// Create a new kernel descriptor
    pub fn new() -> Self {
        KernelDescriptor {
            input_a_offset: 0,
            input_a_size: 0,
            input_b_offset: 0,
            input_b_size: 0,
            output_offset: 0,
            output_size: 0,
            scratch_offset: 0,
            scratch_size: 0,
            params_offset: 0,
            params_size: 0,
        }
    }

    /// Calculate total memory required
    pub fn total_memory_required(&self) -> usize {
        let max_end = [
            self.input_a_offset + self.input_a_size,
            self.input_b_offset + self.input_b_size,
            self.output_offset + self.output_size,
            self.scratch_offset + self.scratch_size,
            self.params_offset + self.params_size,
        ]
        .into_iter()
        .max()
        .unwrap_or(0);

        max_end as usize
    }

    /// Serialize to bytes for passing to WASM
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(40);
        bytes.extend_from_slice(&self.input_a_offset.to_le_bytes());
        bytes.extend_from_slice(&self.input_a_size.to_le_bytes());
        bytes.extend_from_slice(&self.input_b_offset.to_le_bytes());
        bytes.extend_from_slice(&self.input_b_size.to_le_bytes());
        bytes.extend_from_slice(&self.output_offset.to_le_bytes());
        bytes.extend_from_slice(&self.output_size.to_le_bytes());
        bytes.extend_from_slice(&self.scratch_offset.to_le_bytes());
        bytes.extend_from_slice(&self.scratch_size.to_le_bytes());
        bytes.extend_from_slice(&self.params_offset.to_le_bytes());
        bytes.extend_from_slice(&self.params_size.to_le_bytes());
        bytes
    }
}

impl Default for KernelDescriptor {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelManifest {
    /// Parse manifest from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize manifest to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get kernel by ID
    pub fn get_kernel(&self, id: &str) -> Option<&KernelInfo> {
        self.kernels.iter().find(|k| k.id == id)
    }

    /// Get fallback kernel for a given kernel ID
    pub fn get_fallback(&self, id: &str) -> Option<&str> {
        self.fallbacks.get(id).map(|s| s.as_str())
    }

    /// List all kernel IDs
    pub fn kernel_ids(&self) -> Vec<&str> {
        self.kernels.iter().map(|k| k.id.as_str()).collect()
    }

    /// List kernels by category
    pub fn kernels_by_category(&self, category: KernelCategory) -> Vec<&KernelInfo> {
        self.kernels
            .iter()
            .filter(|k| k.category == category)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest_json() -> &'static str {
        r#"{
            "$schema": "https://ruvllm.dev/schemas/kernel-pack-v1.json",
            "version": "1.0.0",
            "name": "test-kernels",
            "description": "Test kernel pack",
            "min_runtime_version": "0.5.0",
            "max_runtime_version": "1.0.0",
            "created_at": "2026-01-18T00:00:00Z",
            "author": {
                "name": "Test Author",
                "email": "test@example.com",
                "signing_key": "ed25519:AAAA..."
            },
            "kernels": [
                {
                    "id": "rope_f32",
                    "name": "Rotary Position Embedding (FP32)",
                    "category": "positional_encoding",
                    "path": "rope/rope_f32.wasm",
                    "hash": "sha256:abc123",
                    "entry_point": "rope_forward",
                    "inputs": [
                        {"name": "x", "dtype": "f32", "shape": ["batch", "seq", "heads", "dim"]},
                        {"name": "freqs", "dtype": "f32", "shape": ["seq", 64]}
                    ],
                    "outputs": [
                        {"name": "y", "dtype": "f32", "shape": ["batch", "seq", "heads", "dim"]}
                    ],
                    "params": {
                        "theta": {"type": "f32", "default": 10000.0}
                    },
                    "resource_limits": {
                        "max_memory_pages": 256,
                        "max_epoch_ticks": 1000,
                        "max_table_elements": 1024
                    },
                    "platforms": {
                        "wasmtime": {
                            "min_version": "15.0.0",
                            "features": ["simd", "bulk-memory"]
                        }
                    },
                    "benchmarks": {
                        "seq_512_dim_128": {
                            "latency_us": 45,
                            "throughput_gflops": 2.1
                        }
                    }
                }
            ],
            "fallbacks": {
                "rope_f32": "rope_reference"
            }
        }"#
    }

    #[test]
    fn test_manifest_parsing() {
        let manifest = KernelManifest::from_json(sample_manifest_json()).unwrap();
        assert_eq!(manifest.name, "test-kernels");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.kernels.len(), 1);
    }

    #[test]
    fn test_kernel_lookup() {
        let manifest = KernelManifest::from_json(sample_manifest_json()).unwrap();
        let kernel = manifest.get_kernel("rope_f32").unwrap();
        assert_eq!(kernel.name, "Rotary Position Embedding (FP32)");
        assert_eq!(kernel.category, KernelCategory::PositionalEncoding);
    }

    #[test]
    fn test_fallback_lookup() {
        let manifest = KernelManifest::from_json(sample_manifest_json()).unwrap();
        assert_eq!(manifest.get_fallback("rope_f32"), Some("rope_reference"));
        assert_eq!(manifest.get_fallback("unknown"), None);
    }

    #[test]
    fn test_kernel_descriptor() {
        let mut desc = KernelDescriptor::new();
        desc.input_a_offset = 0;
        desc.input_a_size = 1024;
        desc.output_offset = 1024;
        desc.output_size = 1024;

        assert_eq!(desc.total_memory_required(), 2048);
        assert_eq!(desc.to_bytes().len(), 40);
    }

    #[test]
    fn test_data_type_sizes() {
        assert_eq!(DataType::F32.size_bytes(), 4);
        assert_eq!(DataType::F16.size_bytes(), 2);
        assert_eq!(DataType::I8.size_bytes(), 1);
    }
}
