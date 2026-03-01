//! WGSL Shader Module Definitions
//!
//! This module contains the embedded WGSL shader source code for all
//! compute operations. Shaders are embedded at compile time for efficient
//! loading in WASM.

/// Matrix multiplication shader (tiled with shared memory)
pub const MATMUL_SHADER: &str = include_str!("shaders/matmul.wgsl");

/// Flash attention shader (online softmax, causal masking)
pub const ATTENTION_SHADER: &str = include_str!("shaders/attention.wgsl");

/// RMSNorm and LayerNorm shader
pub const NORM_SHADER: &str = include_str!("shaders/norm.wgsl");

/// Softmax shader (numerically stable)
pub const SOFTMAX_SHADER: &str = include_str!("shaders/softmax.wgsl");

/// Shader entry points for matrix multiplication
pub mod matmul {
    /// Standard tiled matrix multiply
    pub const MAIN: &str = "main";
    /// Batched matrix multiply for attention projections
    pub const BATCHED: &str = "main_batched";
    /// Vector-matrix multiply for single token generation
    pub const GEMV: &str = "main_gemv";
}

/// Shader entry points for attention
pub mod attention {
    /// Standard multi-head attention
    pub const MAIN: &str = "main";
    /// Grouped query attention (GQA)
    pub const GQA: &str = "main_gqa";
    /// Single token decode attention
    pub const DECODE: &str = "main_decode";
}

/// Shader entry points for normalization
pub mod norm {
    /// RMSNorm (Llama-style)
    pub const RMS_NORM: &str = "rms_norm";
    /// RMSNorm with fused residual connection
    pub const RMS_NORM_RESIDUAL: &str = "rms_norm_residual";
    /// Standard LayerNorm
    pub const LAYER_NORM: &str = "layer_norm";
    /// Fast RMSNorm for small dimensions
    pub const RMS_NORM_SMALL: &str = "rms_norm_small";
}

/// Shader entry points for softmax
pub mod softmax {
    /// Standard row-wise softmax
    pub const MAIN: &str = "softmax";
    /// In-place softmax
    pub const INPLACE: &str = "softmax_inplace";
    /// Small dimension softmax
    pub const SMALL: &str = "softmax_small";
    /// Log softmax for loss computation
    pub const LOG_SOFTMAX: &str = "log_softmax";
}

/// Shader module wrapper for wasm-bindgen
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct ShaderModule {
    name: String,
    source: String,
    entry_points: Vec<String>,
}

#[wasm_bindgen]
impl ShaderModule {
    /// Get the matrix multiplication shader module
    #[wasm_bindgen(js_name = matmul)]
    pub fn get_matmul() -> ShaderModule {
        ShaderModule {
            name: "matmul".to_string(),
            source: MATMUL_SHADER.to_string(),
            entry_points: vec![
                matmul::MAIN.to_string(),
                matmul::BATCHED.to_string(),
                matmul::GEMV.to_string(),
            ],
        }
    }

    /// Get the attention shader module
    #[wasm_bindgen(js_name = attention)]
    pub fn get_attention() -> ShaderModule {
        ShaderModule {
            name: "attention".to_string(),
            source: ATTENTION_SHADER.to_string(),
            entry_points: vec![
                attention::MAIN.to_string(),
                attention::GQA.to_string(),
                attention::DECODE.to_string(),
            ],
        }
    }

    /// Get the normalization shader module
    #[wasm_bindgen(js_name = norm)]
    pub fn get_norm() -> ShaderModule {
        ShaderModule {
            name: "norm".to_string(),
            source: NORM_SHADER.to_string(),
            entry_points: vec![
                norm::RMS_NORM.to_string(),
                norm::RMS_NORM_RESIDUAL.to_string(),
                norm::LAYER_NORM.to_string(),
                norm::RMS_NORM_SMALL.to_string(),
            ],
        }
    }

    /// Get the softmax shader module
    #[wasm_bindgen(js_name = softmax)]
    pub fn get_softmax() -> ShaderModule {
        ShaderModule {
            name: "softmax".to_string(),
            source: SOFTMAX_SHADER.to_string(),
            entry_points: vec![
                softmax::MAIN.to_string(),
                softmax::INPLACE.to_string(),
                softmax::SMALL.to_string(),
                softmax::LOG_SOFTMAX.to_string(),
            ],
        }
    }

    /// Get shader name
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get shader source code
    #[wasm_bindgen(getter)]
    pub fn source(&self) -> String {
        self.source.clone()
    }

    /// Get available entry points
    #[wasm_bindgen(getter, js_name = entryPoints)]
    pub fn entry_points(&self) -> Vec<String> {
        self.entry_points.clone()
    }

    /// Check if an entry point exists
    #[wasm_bindgen(js_name = hasEntryPoint)]
    pub fn has_entry_point(&self, name: &str) -> bool {
        self.entry_points.iter().any(|ep| ep == name)
    }
}

/// Get all available shader modules
#[wasm_bindgen(js_name = getAllShaderModules)]
pub fn get_all_shader_modules() -> Vec<ShaderModule> {
    vec![
        ShaderModule::get_matmul(),
        ShaderModule::get_attention(),
        ShaderModule::get_norm(),
        ShaderModule::get_softmax(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_sources_not_empty() {
        assert!(!MATMUL_SHADER.is_empty());
        assert!(!ATTENTION_SHADER.is_empty());
        assert!(!NORM_SHADER.is_empty());
        assert!(!SOFTMAX_SHADER.is_empty());
    }

    #[test]
    fn test_shader_module_creation() {
        let matmul = ShaderModule::get_matmul();
        assert_eq!(matmul.name(), "matmul");
        assert!(matmul.has_entry_point("main"));
        assert!(matmul.has_entry_point("main_batched"));
    }

    #[test]
    fn test_all_shader_modules() {
        let modules = get_all_shader_modules();
        assert_eq!(modules.len(), 4);
    }
}
