//! SafeTensors Export - PEFT-compatible LoRA weight serialization
//!
//! Exports SONA's learned LoRA weights in SafeTensors format for use with
//! HuggingFace's PEFT library and transformers ecosystem.

use super::{ExportConfig, ExportError, ExportResult, ExportType};
use crate::engine::SonaEngine;
use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "serde-support")]
use serde::{Deserialize, Serialize};

/// SafeTensors exporter for LoRA weights
pub struct SafeTensorsExporter<'a> {
    _config: &'a ExportConfig,
}

impl<'a> SafeTensorsExporter<'a> {
    /// Create new SafeTensors exporter
    pub fn new(config: &'a ExportConfig) -> Self {
        Self { _config: config }
    }

    /// Export engine's LoRA weights to SafeTensors format
    pub fn export_engine<P: AsRef<Path>>(
        &self,
        engine: &SonaEngine,
        output_dir: P,
    ) -> Result<ExportResult, ExportError> {
        let output_dir = output_dir.as_ref();
        std::fs::create_dir_all(output_dir).map_err(ExportError::Io)?;

        // Get LoRA state from engine
        let lora_state = engine.export_lora_state();

        // Build tensor data map
        let mut tensors: HashMap<String, TensorData> = HashMap::new();

        // Export MicroLoRA weights (rank 1-2)
        for (i, layer) in lora_state.micro_lora_layers.iter().enumerate() {
            let a_key = format!(
                "base_model.model.layers.{}.self_attn.micro_lora_A.weight",
                i
            );
            let b_key = format!(
                "base_model.model.layers.{}.self_attn.micro_lora_B.weight",
                i
            );

            tensors.insert(
                a_key,
                TensorData {
                    data: layer.lora_a.clone(),
                    shape: vec![layer.rank, layer.input_dim],
                    dtype: "F32".to_string(),
                },
            );

            tensors.insert(
                b_key,
                TensorData {
                    data: layer.lora_b.clone(),
                    shape: vec![layer.output_dim, layer.rank],
                    dtype: "F32".to_string(),
                },
            );
        }

        // Export BaseLoRA weights (rank 4-16)
        for (i, layer) in lora_state.base_lora_layers.iter().enumerate() {
            // Q projection
            let q_a_key = format!(
                "base_model.model.layers.{}.self_attn.q_proj.lora_A.weight",
                i
            );
            let q_b_key = format!(
                "base_model.model.layers.{}.self_attn.q_proj.lora_B.weight",
                i
            );

            tensors.insert(
                q_a_key,
                TensorData {
                    data: layer.lora_a.clone(),
                    shape: vec![layer.rank, layer.input_dim],
                    dtype: "F32".to_string(),
                },
            );

            tensors.insert(
                q_b_key,
                TensorData {
                    data: layer.lora_b.clone(),
                    shape: vec![layer.output_dim, layer.rank],
                    dtype: "F32".to_string(),
                },
            );

            // K projection
            let k_a_key = format!(
                "base_model.model.layers.{}.self_attn.k_proj.lora_A.weight",
                i
            );
            let k_b_key = format!(
                "base_model.model.layers.{}.self_attn.k_proj.lora_B.weight",
                i
            );

            tensors.insert(
                k_a_key,
                TensorData {
                    data: layer.lora_a.clone(),
                    shape: vec![layer.rank, layer.input_dim],
                    dtype: "F32".to_string(),
                },
            );

            tensors.insert(
                k_b_key,
                TensorData {
                    data: layer.lora_b.clone(),
                    shape: vec![layer.output_dim, layer.rank],
                    dtype: "F32".to_string(),
                },
            );

            // V projection
            let v_a_key = format!(
                "base_model.model.layers.{}.self_attn.v_proj.lora_A.weight",
                i
            );
            let v_b_key = format!(
                "base_model.model.layers.{}.self_attn.v_proj.lora_B.weight",
                i
            );

            tensors.insert(
                v_a_key,
                TensorData {
                    data: layer.lora_a.clone(),
                    shape: vec![layer.rank, layer.input_dim],
                    dtype: "F32".to_string(),
                },
            );

            tensors.insert(
                v_b_key,
                TensorData {
                    data: layer.lora_b.clone(),
                    shape: vec![layer.output_dim, layer.rank],
                    dtype: "F32".to_string(),
                },
            );

            // O projection
            let o_a_key = format!(
                "base_model.model.layers.{}.self_attn.o_proj.lora_A.weight",
                i
            );
            let o_b_key = format!(
                "base_model.model.layers.{}.self_attn.o_proj.lora_B.weight",
                i
            );

            tensors.insert(
                o_a_key,
                TensorData {
                    data: layer.lora_a.clone(),
                    shape: vec![layer.rank, layer.input_dim],
                    dtype: "F32".to_string(),
                },
            );

            tensors.insert(
                o_b_key,
                TensorData {
                    data: layer.lora_b.clone(),
                    shape: vec![layer.output_dim, layer.rank],
                    dtype: "F32".to_string(),
                },
            );
        }

        // Serialize to SafeTensors format
        let safetensors_path = output_dir.join("adapter_model.safetensors");
        let bytes = self.serialize_safetensors(&tensors)?;
        std::fs::write(&safetensors_path, &bytes).map_err(ExportError::Io)?;

        let size_bytes = bytes.len() as u64;

        Ok(ExportResult {
            export_type: ExportType::SafeTensors,
            items_exported: tensors.len(),
            output_path: safetensors_path.to_string_lossy().to_string(),
            size_bytes,
        })
    }

    /// Serialize tensors to SafeTensors binary format
    fn serialize_safetensors(
        &self,
        tensors: &HashMap<String, TensorData>,
    ) -> Result<Vec<u8>, ExportError> {
        // SafeTensors format:
        // 8 bytes: header size (little endian u64)
        // N bytes: JSON header with tensor metadata
        // ... tensor data (aligned to 8 bytes)

        let mut header_data: HashMap<String, TensorMetadata> = HashMap::new();
        let mut tensor_bytes: Vec<u8> = Vec::new();

        // Sort keys for deterministic output
        let mut keys: Vec<_> = tensors.keys().collect();
        keys.sort();

        for key in keys {
            let tensor = &tensors[key];

            // Align to 8 bytes
            let padding = (8 - (tensor_bytes.len() % 8)) % 8;
            tensor_bytes.extend(vec![0u8; padding]);

            let start_offset = tensor_bytes.len();

            // Write tensor data
            for &val in &tensor.data {
                tensor_bytes.extend_from_slice(&val.to_le_bytes());
            }

            let end_offset = tensor_bytes.len();

            header_data.insert(
                key.clone(),
                TensorMetadata {
                    dtype: tensor.dtype.clone(),
                    shape: tensor.shape.clone(),
                    data_offsets: [start_offset, end_offset],
                },
            );
        }

        // Serialize header to JSON
        let header_json =
            serde_json::to_string(&header_data).map_err(ExportError::Serialization)?;
        let header_bytes = header_json.as_bytes();

        // Build final buffer
        let mut result = Vec::new();

        // Header size (8 bytes, little endian)
        result.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());

        // Header JSON
        result.extend_from_slice(header_bytes);

        // Tensor data
        result.extend(tensor_bytes);

        Ok(result)
    }
}

/// Tensor data for export
#[derive(Clone, Debug)]
pub struct TensorData {
    /// Flattened tensor values
    pub data: Vec<f32>,
    /// Tensor shape
    pub shape: Vec<usize>,
    /// Data type (F32, F16, BF16, etc.)
    pub dtype: String,
}

/// Tensor metadata for SafeTensors header
#[cfg(feature = "serde-support")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TensorMetadata {
    dtype: String,
    shape: Vec<usize>,
    data_offsets: [usize; 2],
}

/// LoRA layer state for export
#[derive(Clone, Debug)]
pub struct LoRALayerState {
    /// LoRA A matrix (rank x input_dim)
    pub lora_a: Vec<f32>,
    /// LoRA B matrix (output_dim x rank)
    pub lora_b: Vec<f32>,
    /// LoRA rank
    pub rank: usize,
    /// Input dimension
    pub input_dim: usize,
    /// Output dimension
    pub output_dim: usize,
}

/// Complete LoRA state for export
#[derive(Clone, Debug, Default)]
pub struct LoRAState {
    /// MicroLoRA layers (instant adaptation)
    pub micro_lora_layers: Vec<LoRALayerState>,
    /// BaseLoRA layers (background learning)
    pub base_lora_layers: Vec<LoRALayerState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_data_creation() {
        let tensor = TensorData {
            data: vec![1.0, 2.0, 3.0, 4.0],
            shape: vec![2, 2],
            dtype: "F32".to_string(),
        };

        assert_eq!(tensor.data.len(), 4);
        assert_eq!(tensor.shape, vec![2, 2]);
    }

    #[test]
    fn test_lora_layer_state() {
        let state = LoRALayerState {
            lora_a: vec![0.1, 0.2, 0.3, 0.4],
            lora_b: vec![0.5, 0.6, 0.7, 0.8],
            rank: 2,
            input_dim: 2,
            output_dim: 2,
        };

        assert_eq!(state.rank, 2);
        assert_eq!(state.lora_a.len(), 4);
    }
}
