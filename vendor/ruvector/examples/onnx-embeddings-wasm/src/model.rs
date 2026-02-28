//! Tract-based ONNX model for WASM inference

use crate::error::{Result, WasmEmbeddingError};
use crate::tokenizer::EncodedInput;
use tract_onnx::prelude::*;

/// Tract ONNX model wrapper for WASM
pub struct TractModel {
    model: SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>,
    hidden_size: usize,
}

impl TractModel {
    /// Load model from ONNX bytes
    pub fn from_bytes(bytes: &[u8], max_seq_length: usize) -> Result<Self> {
        // Parse ONNX model
        let model = tract_onnx::onnx()
            .model_for_read(&mut std::io::Cursor::new(bytes))
            .map_err(|e| WasmEmbeddingError::model(format!("Failed to parse ONNX: {}", e)))?;

        // Set input shapes for optimization
        // Standard transformer inputs: [batch, seq_len]
        let batch = 1usize;
        let seq_len = max_seq_length;

        let model = model
            .with_input_fact(
                0,
                InferenceFact::dt_shape(i64::datum_type(), tvec![batch, seq_len]),
            )?
            .with_input_fact(
                1,
                InferenceFact::dt_shape(i64::datum_type(), tvec![batch, seq_len]),
            )?
            .with_input_fact(
                2,
                InferenceFact::dt_shape(i64::datum_type(), tvec![batch, seq_len]),
            )?;

        // Optimize the model
        let model = model
            .into_optimized()
            .map_err(|e| WasmEmbeddingError::model(format!("Failed to optimize: {}", e)))?;

        let model = model
            .into_runnable()
            .map_err(|e| WasmEmbeddingError::model(format!("Failed to make runnable: {}", e)))?;

        // Default hidden size (will be determined from output)
        let hidden_size = 384;

        Ok(Self { model, hidden_size })
    }

    /// Run inference on encoded input
    pub fn run(&self, input: &EncodedInput) -> Result<Vec<f32>> {
        let seq_len = input.input_ids.len();

        // Create input tensors
        let input_ids: Tensor = tract_ndarray::Array2::from_shape_vec(
            (1, seq_len),
            input.input_ids.clone(),
        )
        .map_err(|e| WasmEmbeddingError::inference(e.to_string()))?
        .into();

        let attention_mask: Tensor = tract_ndarray::Array2::from_shape_vec(
            (1, seq_len),
            input.attention_mask.clone(),
        )
        .map_err(|e| WasmEmbeddingError::inference(e.to_string()))?
        .into();

        let token_type_ids: Tensor = tract_ndarray::Array2::from_shape_vec(
            (1, seq_len),
            input.token_type_ids.clone(),
        )
        .map_err(|e| WasmEmbeddingError::inference(e.to_string()))?
        .into();

        // Run inference
        let inputs = tvec![
            input_ids.into(),
            attention_mask.into(),
            token_type_ids.into()
        ];

        let outputs = self
            .model
            .run(inputs)
            .map_err(|e| WasmEmbeddingError::inference(format!("Inference failed: {}", e)))?;

        // Extract output tensor
        // Output is typically [batch, seq_len, hidden_size] or [batch, hidden_size]
        let output = outputs
            .first()
            .ok_or_else(|| WasmEmbeddingError::inference("No output tensor"))?;

        let output_array = output
            .to_array_view::<f32>()
            .map_err(|e| WasmEmbeddingError::inference(format!("Failed to extract output: {}", e)))?;

        // Flatten and return
        Ok(output_array.iter().copied().collect())
    }

    /// Get the hidden size
    pub fn hidden_size(&self) -> usize {
        self.hidden_size
    }

    /// Update hidden size (called after first inference)
    pub fn set_hidden_size(&mut self, size: usize) {
        self.hidden_size = size;
    }
}
