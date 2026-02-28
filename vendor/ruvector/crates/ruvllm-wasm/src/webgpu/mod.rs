//! WebGPU Compute Module for WASM-based GPU Acceleration
//!
//! This module provides WebGPU compute shader support for LLM inference
//! operations in the browser. It includes:
//!
//! - Matrix multiplication (tiled, batched, GEMV)
//! - Flash Attention (causal, GQA, decode)
//! - RMSNorm and LayerNorm
//! - Softmax (standard, temperature-scaled, log-softmax)
//!
//! ## Feature Detection
//!
//! WebGPU availability is checked at runtime with graceful fallback:
//!
//! ```javascript
//! if (await WebGpuInference.isAvailable()) {
//!     const gpu = await WebGpuInference.init();
//!     const result = await gpu.matmul(a, b, m, n, k);
//! } else {
//!     // Fall back to CPU implementation
//! }
//! ```
//!
//! ## Performance Targets
//!
//! - Matrix multiply: ~1 TFLOP on integrated GPUs, ~10 TFLOPS on discrete
//! - Attention: 2ms for 4K context on discrete GPU
//! - Normalization: <0.5ms for typical hidden dimensions

pub mod buffers;
pub mod compute;
pub mod shaders;

use wasm_bindgen::prelude::*;

pub use buffers::{GpuBuffer, GpuBufferUsage};
pub use compute::{ComputePipeline, WebGpuContext};
pub use shaders::ShaderModule;

/// GPU adapter information
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    /// GPU vendor name
    #[wasm_bindgen(skip)]
    pub vendor: String,
    /// GPU architecture/device name
    #[wasm_bindgen(skip)]
    pub architecture: String,
    /// Device type (integrated, discrete, etc.)
    #[wasm_bindgen(skip)]
    pub device_type: String,
    /// Backend API (WebGPU, etc.)
    #[wasm_bindgen(skip)]
    pub backend: String,
    /// Maximum buffer size in bytes
    #[wasm_bindgen(skip)]
    pub max_buffer_size: u64,
    /// Maximum compute workgroup size
    #[wasm_bindgen(skip)]
    pub max_workgroup_size: u32,
}

#[wasm_bindgen]
impl AdapterInfo {
    /// Get GPU vendor name
    #[wasm_bindgen(getter)]
    pub fn vendor(&self) -> String {
        self.vendor.clone()
    }

    /// Get GPU architecture
    #[wasm_bindgen(getter)]
    pub fn architecture(&self) -> String {
        self.architecture.clone()
    }

    /// Get device type
    #[wasm_bindgen(getter, js_name = deviceType)]
    pub fn device_type(&self) -> String {
        self.device_type.clone()
    }

    /// Get backend API
    #[wasm_bindgen(getter)]
    pub fn backend(&self) -> String {
        self.backend.clone()
    }

    /// Get maximum buffer size
    #[wasm_bindgen(getter, js_name = maxBufferSize)]
    pub fn max_buffer_size(&self) -> u64 {
        self.max_buffer_size
    }

    /// Get maximum workgroup size
    #[wasm_bindgen(getter, js_name = maxWorkgroupSize)]
    pub fn max_workgroup_size(&self) -> u32 {
        self.max_workgroup_size
    }

    /// Convert to JSON string
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        let json = serde_json::json!({
            "vendor": self.vendor,
            "architecture": self.architecture,
            "deviceType": self.device_type,
            "backend": self.backend,
            "maxBufferSize": self.max_buffer_size,
            "maxWorkgroupSize": self.max_workgroup_size,
        });
        serde_json::to_string(&json).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Attention configuration for compute shaders
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct AttentionConfig {
    /// Sequence length for queries
    #[wasm_bindgen(skip)]
    pub seq_len: u32,
    /// Key/Value sequence length (can differ for encoder-decoder)
    #[wasm_bindgen(skip)]
    pub kv_seq_len: u32,
    /// Number of attention heads
    #[wasm_bindgen(skip)]
    pub num_heads: u32,
    /// Dimension per head
    #[wasm_bindgen(skip)]
    pub head_dim: u32,
    /// Whether to apply causal masking
    #[wasm_bindgen(skip)]
    pub causal: bool,
}

#[wasm_bindgen]
impl AttentionConfig {
    /// Create new attention configuration
    #[wasm_bindgen(constructor)]
    pub fn new(seq_len: u32, num_heads: u32, head_dim: u32, causal: bool) -> Self {
        Self {
            seq_len,
            kv_seq_len: seq_len,
            num_heads,
            head_dim,
            causal,
        }
    }

    /// Create for encoder-decoder models with different KV length
    #[wasm_bindgen(js_name = forEncoderDecoder)]
    pub fn for_encoder_decoder(
        seq_len: u32,
        kv_seq_len: u32,
        num_heads: u32,
        head_dim: u32,
    ) -> Self {
        Self {
            seq_len,
            kv_seq_len,
            num_heads,
            head_dim,
            causal: false,
        }
    }

    /// Get the scaling factor (1/sqrt(head_dim))
    pub fn scale(&self) -> f32 {
        1.0 / (self.head_dim as f32).sqrt()
    }

    /// Get total hidden dimension
    pub fn hidden_dim(&self) -> u32 {
        self.num_heads * self.head_dim
    }

    #[wasm_bindgen(getter, js_name = seqLen)]
    pub fn get_seq_len(&self) -> u32 {
        self.seq_len
    }

    #[wasm_bindgen(setter, js_name = seqLen)]
    pub fn set_seq_len(&mut self, value: u32) {
        self.seq_len = value;
    }

    #[wasm_bindgen(getter, js_name = kvSeqLen)]
    pub fn get_kv_seq_len(&self) -> u32 {
        self.kv_seq_len
    }

    #[wasm_bindgen(setter, js_name = kvSeqLen)]
    pub fn set_kv_seq_len(&mut self, value: u32) {
        self.kv_seq_len = value;
    }

    #[wasm_bindgen(getter, js_name = numHeads)]
    pub fn get_num_heads(&self) -> u32 {
        self.num_heads
    }

    #[wasm_bindgen(setter, js_name = numHeads)]
    pub fn set_num_heads(&mut self, value: u32) {
        self.num_heads = value;
    }

    #[wasm_bindgen(getter, js_name = headDim)]
    pub fn get_head_dim(&self) -> u32 {
        self.head_dim
    }

    #[wasm_bindgen(setter, js_name = headDim)]
    pub fn set_head_dim(&mut self, value: u32) {
        self.head_dim = value;
    }

    #[wasm_bindgen(getter)]
    pub fn get_causal(&self) -> bool {
        self.causal
    }

    #[wasm_bindgen(setter)]
    pub fn set_causal(&mut self, value: bool) {
        self.causal = value;
    }
}

/// Check if WebGPU is available in this browser
#[wasm_bindgen(js_name = isWebGpuAvailable)]
pub async fn is_webgpu_available() -> bool {
    compute::is_webgpu_available().await
}

/// Get GPU information if available
#[wasm_bindgen(js_name = getGpuInfo)]
pub async fn get_gpu_info() -> Result<JsValue, JsValue> {
    match compute::get_gpu_info().await {
        Some(info) => {
            let js_obj = js_sys::Object::new();
            js_sys::Reflect::set(&js_obj, &"vendor".into(), &info.vendor.into())?;
            js_sys::Reflect::set(&js_obj, &"architecture".into(), &info.architecture.into())?;
            js_sys::Reflect::set(&js_obj, &"deviceType".into(), &info.device_type.into())?;
            js_sys::Reflect::set(&js_obj, &"backend".into(), &info.backend.into())?;
            js_sys::Reflect::set(
                &js_obj,
                &"maxBufferSize".into(),
                &JsValue::from_f64(info.max_buffer_size as f64),
            )?;
            js_sys::Reflect::set(
                &js_obj,
                &"maxWorkgroupSize".into(),
                &JsValue::from_f64(info.max_workgroup_size as f64),
            )?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::NULL),
    }
}

/// WebGPU error types
#[derive(Debug)]
pub enum WebGpuError {
    /// WebGPU not available in this browser
    NotAvailable,
    /// Failed to get GPU adapter
    AdapterNotFound,
    /// Failed to create device
    DeviceCreationFailed(String),
    /// Buffer allocation failed
    BufferAllocationFailed { requested: usize, available: usize },
    /// Shader compilation failed
    ShaderCompilationFailed(String),
    /// Invalid dimensions for operation
    DimensionMismatch { expected: String, actual: String },
    /// Operation timed out
    Timeout,
    /// Generic GPU error
    GpuError(String),
}

impl std::fmt::Display for WebGpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "WebGPU is not available in this browser"),
            Self::AdapterNotFound => write!(f, "No suitable GPU adapter found"),
            Self::DeviceCreationFailed(msg) => write!(f, "Failed to create GPU device: {}", msg),
            Self::BufferAllocationFailed {
                requested,
                available,
            } => {
                write!(
                    f,
                    "Buffer allocation failed: requested {} bytes, {} available",
                    requested, available
                )
            }
            Self::ShaderCompilationFailed(msg) => write!(f, "Shader compilation failed: {}", msg),
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "Dimension mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::Timeout => write!(f, "GPU operation timed out"),
            Self::GpuError(msg) => write!(f, "GPU error: {}", msg),
        }
    }
}

impl std::error::Error for WebGpuError {}

impl From<WebGpuError> for JsValue {
    fn from(error: WebGpuError) -> Self {
        JsValue::from_str(&error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_config() {
        let config = AttentionConfig::new(512, 8, 64, true);
        assert_eq!(config.hidden_dim(), 512);
        assert!((config.scale() - 0.125).abs() < 0.001); // 1/sqrt(64) = 0.125
    }

    #[test]
    fn test_adapter_info_json() {
        let info = AdapterInfo {
            vendor: "TestVendor".to_string(),
            architecture: "TestArch".to_string(),
            device_type: "integrated".to_string(),
            backend: "WebGPU".to_string(),
            max_buffer_size: 1024 * 1024 * 256,
            max_workgroup_size: 256,
        };
        let json = info.to_json().unwrap();
        assert!(json.contains("TestVendor"));
    }
}
