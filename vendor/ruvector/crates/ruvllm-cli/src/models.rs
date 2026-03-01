//! Model definitions and aliases for RuvLLM CLI
//!
//! This module defines the recommended models for different use cases,
//! optimized for Mac M4 Pro with 36GB unified memory.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Recommended models for RuvLLM on Mac M4 Pro
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    /// HuggingFace model ID
    pub hf_id: String,
    /// Short alias for CLI
    pub alias: String,
    /// Display name
    pub name: String,
    /// Model architecture (mistral, llama, phi, qwen)
    pub architecture: String,
    /// Parameter count in billions
    pub params_b: f32,
    /// Primary use case
    pub use_case: String,
    /// Recommended quantization
    pub recommended_quant: String,
    /// Estimated memory usage in GB (for recommended quant)
    pub memory_gb: f32,
    /// Context length
    pub context_length: usize,
    /// Notes about the model
    pub notes: String,
}

/// Get all recommended models
pub fn get_recommended_models() -> Vec<ModelDefinition> {
    vec![
        // Primary reasoning model
        ModelDefinition {
            hf_id: "Qwen/Qwen2.5-14B-Instruct-GGUF".to_string(),
            alias: "qwen".to_string(),
            name: "Qwen2.5-14B-Instruct".to_string(),
            architecture: "qwen".to_string(),
            params_b: 14.0,
            use_case: "Primary reasoning, code generation, complex tasks".to_string(),
            recommended_quant: "Q4_K_M".to_string(),
            memory_gb: 9.5,
            context_length: 32768,
            notes: "Best overall performance for reasoning tasks on M4 Pro".to_string(),
        },
        // Fast instruction following
        ModelDefinition {
            hf_id: "mistralai/Mistral-7B-Instruct-v0.3".to_string(),
            alias: "mistral".to_string(),
            name: "Mistral-7B-Instruct-v0.3".to_string(),
            architecture: "mistral".to_string(),
            params_b: 7.0,
            use_case: "Fast instruction following, general chat".to_string(),
            recommended_quant: "Q4_K_M".to_string(),
            memory_gb: 4.5,
            context_length: 32768,
            notes: "Excellent speed/quality tradeoff with sliding window attention".to_string(),
        },
        // Tiny/testing model
        ModelDefinition {
            hf_id: "microsoft/Phi-4-mini-instruct".to_string(),
            alias: "phi".to_string(),
            name: "Phi-4-mini".to_string(),
            architecture: "phi".to_string(),
            params_b: 3.8,
            use_case: "Testing, quick prototyping, resource-constrained".to_string(),
            recommended_quant: "Q4_K_M".to_string(),
            memory_gb: 2.5,
            context_length: 16384,
            notes: "Surprisingly capable for its size, fast inference".to_string(),
        },
        // Tool use model
        ModelDefinition {
            hf_id: "meta-llama/Llama-3.2-3B-Instruct".to_string(),
            alias: "llama".to_string(),
            name: "Llama-3.2-3B-Instruct".to_string(),
            architecture: "llama".to_string(),
            params_b: 3.2,
            use_case: "Tool use, function calling, structured output".to_string(),
            recommended_quant: "Q4_K_M".to_string(),
            memory_gb: 2.2,
            context_length: 131072,
            notes: "Optimized for tool use and function calling".to_string(),
        },
        // Code-specific model
        ModelDefinition {
            hf_id: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF".to_string(),
            alias: "qwen-coder".to_string(),
            name: "Qwen2.5-Coder-7B-Instruct".to_string(),
            architecture: "qwen".to_string(),
            params_b: 7.0,
            use_case: "Code generation, code review, debugging".to_string(),
            recommended_quant: "Q4_K_M".to_string(),
            memory_gb: 4.8,
            context_length: 32768,
            notes: "Specialized for coding tasks, excellent at code completion".to_string(),
        },
        // Large reasoning model (for when you have the memory)
        ModelDefinition {
            hf_id: "Qwen/Qwen2.5-32B-Instruct-GGUF".to_string(),
            alias: "qwen-large".to_string(),
            name: "Qwen2.5-32B-Instruct".to_string(),
            architecture: "qwen".to_string(),
            params_b: 32.0,
            use_case: "Complex reasoning, research, highest quality output".to_string(),
            recommended_quant: "Q4_K_M".to_string(),
            memory_gb: 20.0,
            context_length: 32768,
            notes: "Requires significant memory, but provides best quality".to_string(),
        },
    ]
}

/// Get model by alias or HF ID
pub fn get_model(identifier: &str) -> Option<ModelDefinition> {
    let models = get_recommended_models();

    // First try exact alias match
    if let Some(model) = models.iter().find(|m| m.alias == identifier) {
        return Some(model.clone());
    }

    // Try HF ID match
    if let Some(model) = models.iter().find(|m| m.hf_id == identifier) {
        return Some(model.clone());
    }

    // Try partial HF ID match
    if let Some(model) = models.iter().find(|m| m.hf_id.contains(identifier)) {
        return Some(model.clone());
    }

    None
}

/// Resolve model identifier to HuggingFace ID
pub fn resolve_model_id(identifier: &str) -> String {
    if let Some(model) = get_model(identifier) {
        model.hf_id
    } else {
        // Assume it's a direct HF model ID
        identifier.to_string()
    }
}

/// Get model aliases map
pub fn get_aliases() -> HashMap<String, String> {
    get_recommended_models()
        .into_iter()
        .map(|m| (m.alias, m.hf_id))
        .collect()
}

/// Quantization presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantPreset {
    /// 4-bit K-quants (best quality/size tradeoff)
    Q4K,
    /// 8-bit quantization (higher quality, more memory)
    Q8,
    /// 16-bit floating point (high quality, most memory)
    F16,
    /// No quantization (full precision)
    None,
}

impl QuantPreset {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "q4k" | "q4_k" | "q4_k_m" | "q4" => Some(Self::Q4K),
            "q8" | "q8_0" => Some(Self::Q8),
            "f16" | "fp16" => Some(Self::F16),
            "none" | "f32" | "fp32" => Some(Self::None),
            _ => None,
        }
    }

    /// Get GGUF file suffix
    pub fn gguf_suffix(&self) -> &'static str {
        match self {
            Self::Q4K => "Q4_K_M.gguf",
            Self::Q8 => "Q8_0.gguf",
            Self::F16 => "F16.gguf",
            Self::None => "F32.gguf",
        }
    }

    /// Get bytes per weight
    pub fn bytes_per_weight(&self) -> f32 {
        match self {
            Self::Q4K => 0.5,
            Self::Q8 => 1.0,
            Self::F16 => 2.0,
            Self::None => 4.0,
        }
    }

    /// Estimate memory usage in GB for given parameter count
    pub fn estimate_memory_gb(&self, params_b: f32) -> f32 {
        // Base memory for weights
        let weight_memory = params_b * self.bytes_per_weight();
        // Add overhead for KV cache, activations, etc. (roughly 20%)
        weight_memory * 1.2
    }
}

impl std::fmt::Display for QuantPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Q4K => write!(f, "Q4_K_M"),
            Self::Q8 => write!(f, "Q8_0"),
            Self::F16 => write!(f, "F16"),
            Self::None => write!(f, "F32"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_by_alias() {
        let model = get_model("qwen").unwrap();
        assert!(model.hf_id.contains("Qwen2.5-14B"));
    }

    #[test]
    fn test_resolve_model_id() {
        assert!(resolve_model_id("mistral").contains("Mistral-7B"));
        assert_eq!(resolve_model_id("custom/model"), "custom/model");
    }

    #[test]
    fn test_quant_preset() {
        assert_eq!(QuantPreset::from_str("q4k"), Some(QuantPreset::Q4K));
        assert_eq!(QuantPreset::Q4K.bytes_per_weight(), 0.5);
    }
}
