//! Universal model loader trait and metadata

use crate::error::{ModelError, SparseInferenceError};
use crate::model::gguf::{GgufModel, GgufParser, GgufValue};

type Result<T> = std::result::Result<T, SparseInferenceError>;
use std::collections::HashMap;
use std::path::Path;

/// Universal model loader trait
pub trait ModelLoader {
    type Model;
    type Error: std::error::Error;

    /// Load model from bytes
    fn load(data: &[u8]) -> Result<Self::Model>;

    /// Load model from file path (native only)
    #[cfg(not(target_arch = "wasm32"))]
    fn load_file(path: &Path) -> Result<Self::Model> {
        let data = std::fs::read(path).map_err(|e| {
            SparseInferenceError::Model(ModelError::LoadFailed(format!(
                "Failed to read file: {}",
                e
            )))
        })?;
        Self::load(&data)
    }

    /// Get model metadata
    fn metadata(&self) -> &ModelMetadata;
}

/// Model metadata extracted from GGUF or other formats
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    pub architecture: ModelArchitecture,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_layers: usize,
    pub num_heads: usize,
    pub num_key_value_heads: Option<usize>,
    pub vocab_size: usize,
    pub max_position_embeddings: usize,
    pub quantization: Option<QuantizationType>,
    pub rope_theta: Option<f32>,
    pub rope_scaling: Option<RopeScaling>,
}

impl ModelMetadata {
    /// Extract metadata from GGUF model
    pub fn from_gguf(model: &GgufModel) -> Result<Self> {
        let arch_name = Self::get_string(&model.metadata, "general.architecture")
            .map_err(|e| SparseInferenceError::Model(ModelError::InvalidConfig(e)))?;
        let architecture = ModelArchitecture::from_str(&arch_name)
            .map_err(|e| SparseInferenceError::Model(ModelError::InvalidConfig(e)))?;

        let prefix = format!("{}", arch_name);

        Ok(Self {
            architecture,
            hidden_size: Self::get_u32(&model.metadata, &format!("{}.embedding_length", prefix))?
                as usize,
            intermediate_size: Self::get_u32(
                &model.metadata,
                &format!("{}.feed_forward_length", prefix),
            )
            .unwrap_or(0) as usize,
            num_layers: Self::get_u32(&model.metadata, &format!("{}.block_count", prefix))?
                as usize,
            num_heads: Self::get_u32(&model.metadata, &format!("{}.attention.head_count", prefix))?
                as usize,
            num_key_value_heads: Self::get_u32(
                &model.metadata,
                &format!("{}.attention.head_count_kv", prefix),
            )
            .ok()
            .map(|v| v as usize),
            vocab_size: Self::get_u32(&model.metadata, "tokenizer.ggml.tokens")
                .or_else(|_| Self::get_array_len(&model.metadata, "tokenizer.ggml.tokens"))
                .unwrap_or(32000) as usize,
            max_position_embeddings: Self::get_u32(
                &model.metadata,
                &format!("{}.context_length", prefix),
            )
            .unwrap_or(2048) as usize,
            quantization: None, // Determined from tensor types
            rope_theta: Self::get_f32(&model.metadata, &format!("{}.rope.freq_base", prefix)).ok(),
            rope_scaling: None,
        })
    }

    fn get_string(
        metadata: &HashMap<String, GgufValue>,
        key: &str,
    ) -> std::result::Result<String, String> {
        match metadata.get(key) {
            Some(GgufValue::String(s)) => Ok(s.clone()),
            _ => Err(format!("Missing metadata: {}", key)),
        }
    }

    fn get_u32(
        metadata: &HashMap<String, GgufValue>,
        key: &str,
    ) -> std::result::Result<u32, String> {
        match metadata.get(key) {
            Some(GgufValue::Uint32(v)) => Ok(*v),
            Some(GgufValue::Uint64(v)) => Ok(*v as u32),
            Some(GgufValue::Int32(v)) => Ok(*v as u32),
            _ => Err(format!("Missing metadata: {}", key)),
        }
    }

    fn get_f32(
        metadata: &HashMap<String, GgufValue>,
        key: &str,
    ) -> std::result::Result<f32, String> {
        match metadata.get(key) {
            Some(GgufValue::Float32(v)) => Ok(*v),
            Some(GgufValue::Float64(v)) => Ok(*v as f32),
            _ => Err(format!("Missing metadata: {}", key)),
        }
    }

    fn get_array_len(
        metadata: &HashMap<String, GgufValue>,
        key: &str,
    ) -> std::result::Result<u32, String> {
        match metadata.get(key) {
            Some(GgufValue::Array(arr)) => Ok(arr.len() as u32),
            _ => Err(format!("Missing metadata: {}", key)),
        }
    }
}

/// Model architecture type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArchitecture {
    Llama,
    LFM2,
    Bert,
    Mistral,
    Qwen,
    Phi,
    Gemma,
}

impl ModelArchitecture {
    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_lowercase().as_str() {
            "llama" => Ok(Self::Llama),
            "lfm" | "lfm2" => Ok(Self::LFM2),
            "bert" => Ok(Self::Bert),
            "mistral" => Ok(Self::Mistral),
            "qwen" | "qwen2" => Ok(Self::Qwen),
            "phi" | "phi2" | "phi3" => Ok(Self::Phi),
            "gemma" | "gemma2" => Ok(Self::Gemma),
            _ => Err(format!("Unsupported architecture: {}", s)),
        }
    }
}

/// Quantization type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizationType {
    F32,
    F16,
    Q4_0,
    Q4_1,
    Q5_0,
    Q5_1,
    Q8_0,
    Q8_1,
    Q4_K,
    Q5_K,
    Q6_K,
}

/// RoPE scaling configuration
#[derive(Debug, Clone)]
pub struct RopeScaling {
    pub scaling_type: String,
    pub factor: f32,
}

impl Default for ModelMetadata {
    fn default() -> Self {
        Self {
            architecture: ModelArchitecture::Llama,
            hidden_size: 4096,
            intermediate_size: 11008,
            num_layers: 32,
            num_heads: 32,
            num_key_value_heads: None,
            vocab_size: 32000,
            max_position_embeddings: 2048,
            quantization: None,
            rope_theta: Some(10000.0),
            rope_scaling: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_parsing() {
        assert_eq!(
            ModelArchitecture::from_str("llama").unwrap(),
            ModelArchitecture::Llama
        );
        assert_eq!(
            ModelArchitecture::from_str("BERT").unwrap(),
            ModelArchitecture::Bert
        );
    }

    #[test]
    fn test_default_metadata() {
        let metadata = ModelMetadata::default();
        assert_eq!(metadata.architecture, ModelArchitecture::Llama);
        assert_eq!(metadata.hidden_size, 4096);
    }
}
