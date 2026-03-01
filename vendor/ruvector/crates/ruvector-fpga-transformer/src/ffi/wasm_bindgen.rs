//! WASM bindings via wasm-bindgen
//!
//! Provides the same API shape for browser and Node.js environments.

#![cfg(feature = "wasm")]

use js_sys::{Array, Int16Array, Object, Reflect, Uint16Array, Uint8Array};
use wasm_bindgen::prelude::*;

use crate::artifact::{unpack_artifact, ModelArtifact};
use crate::backend::native_sim::{NativeSimBackend, NativeSimConfig};
use crate::backend::TransformerBackend;
use crate::gating::DefaultCoherenceGate;
use crate::types::{ComputeClass, FixedShape, GateHint, InferenceRequest, ModelId};
use std::sync::Arc;

/// WASM Engine for transformer inference
#[wasm_bindgen]
pub struct WasmEngine {
    backend: NativeSimBackend,
    loaded_models: Vec<ModelId>,
    last_witness: Option<crate::types::WitnessLog>,
}

#[wasm_bindgen]
impl WasmEngine {
    /// Create a new WASM engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Use permissive config for WASM
        let config = NativeSimConfig {
            max_models: 4,
            trace: false,
            lut_softmax: true,
            max_layers: 0,
        };

        let gate = Arc::new(DefaultCoherenceGate::new());
        let backend = NativeSimBackend::with_config(gate, config);

        Self {
            backend,
            loaded_models: Vec::new(),
            last_witness: None,
        }
    }

    /// Load a model artifact from bytes
    ///
    /// Returns the model ID as a Uint8Array on success
    #[wasm_bindgen(js_name = loadArtifact)]
    pub fn load_artifact(&mut self, artifact_bytes: &[u8]) -> Result<Uint8Array, JsValue> {
        let artifact = unpack_artifact(artifact_bytes)
            .map_err(|e| JsValue::from_str(&format!("Failed to unpack artifact: {}", e)))?;

        let model_id = self
            .backend
            .load(&artifact)
            .map_err(|e| JsValue::from_str(&format!("Failed to load model: {}", e)))?;

        self.loaded_models.push(model_id);

        // Return model ID as Uint8Array
        let id_array = Uint8Array::new_with_length(32);
        id_array.copy_from(model_id.as_bytes());
        Ok(id_array)
    }

    /// Run inference
    ///
    /// Returns an object with logits, topk, and witness
    #[wasm_bindgen]
    pub fn infer(
        &mut self,
        model_id: &[u8],
        tokens: &[u16],
        mask: &[u8],
        coherence_score_q: i16,
        boundary_crossed: bool,
        max_compute_class: u8,
    ) -> Result<JsValue, JsValue> {
        // Parse model ID
        if model_id.len() != 32 {
            return Err(JsValue::from_str("Model ID must be 32 bytes"));
        }
        let mut id_bytes = [0u8; 32];
        id_bytes.copy_from_slice(model_id);
        let model = ModelId::new(id_bytes);

        // Get shape from loaded model
        // For WASM, we use micro shape by default
        let shape = FixedShape::micro();

        // Validate input length
        if tokens.len() != shape.seq_len as usize {
            return Err(JsValue::from_str(&format!(
                "Token length mismatch: expected {}, got {}",
                shape.seq_len,
                tokens.len()
            )));
        }

        // Build gate hint
        let compute_class =
            ComputeClass::from_u8(max_compute_class).unwrap_or(ComputeClass::Deliberative);
        let gate_hint = GateHint::new(coherence_score_q, boundary_crossed, compute_class);

        // Create request
        let req = InferenceRequest::new(model, shape, tokens, mask, gate_hint);

        // Run inference
        let result = self
            .backend
            .infer(req)
            .map_err(|e| JsValue::from_str(&format!("Inference failed: {}", e)))?;

        // Store witness
        self.last_witness = Some(result.witness.clone());

        // Build result object
        let obj = Object::new();

        // Add logits
        let logits = Int16Array::new_with_length(result.logits_q.len() as u32);
        logits.copy_from(&result.logits_q);
        Reflect::set(&obj, &"logits".into(), &logits)?;

        // Add top-K if available
        if let Some(topk) = &result.topk {
            let topk_array = Array::new();
            for (token, logit) in topk {
                let pair = Array::new();
                pair.push(&JsValue::from(*token));
                pair.push(&JsValue::from(*logit));
                topk_array.push(&pair);
            }
            Reflect::set(&obj, &"topk".into(), &topk_array)?;
        }

        // Add witness info
        let witness = Object::new();
        Reflect::set(
            &witness,
            &"backend".into(),
            &format!("{:?}", result.witness.backend).into(),
        )?;
        Reflect::set(
            &witness,
            &"cycles".into(),
            &JsValue::from(result.witness.cycles),
        )?;
        Reflect::set(
            &witness,
            &"latency_ns".into(),
            &JsValue::from(result.witness.latency_ns),
        )?;
        Reflect::set(
            &witness,
            &"gate_decision".into(),
            &format!("{:?}", result.witness.gate_decision).into(),
        )?;
        Reflect::set(&obj, &"witness".into(), &witness)?;

        Ok(obj.into())
    }

    /// Get the last witness log as JSON
    #[wasm_bindgen(js_name = getWitness)]
    pub fn get_witness(&self) -> Result<JsValue, JsValue> {
        match &self.last_witness {
            Some(witness) => {
                let json = serde_json::to_string(witness)
                    .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))?;
                Ok(JsValue::from_str(&json))
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Get list of loaded model IDs
    #[wasm_bindgen(js_name = getLoadedModels)]
    pub fn get_loaded_models(&self) -> Array {
        let arr = Array::new();
        for id in &self.loaded_models {
            let id_array = Uint8Array::new_with_length(32);
            id_array.copy_from(id.as_bytes());
            arr.push(&id_array);
        }
        arr
    }

    /// Unload a model
    #[wasm_bindgen]
    pub fn unload(&mut self, model_id: &[u8]) -> Result<(), JsValue> {
        if model_id.len() != 32 {
            return Err(JsValue::from_str("Model ID must be 32 bytes"));
        }
        let mut id_bytes = [0u8; 32];
        id_bytes.copy_from_slice(model_id);
        let model = ModelId::new(id_bytes);

        self.backend
            .unload(model)
            .map_err(|e| JsValue::from_str(&format!("Unload failed: {}", e)))?;

        self.loaded_models.retain(|id| *id != model);
        Ok(())
    }

    /// Get backend statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> Result<JsValue, JsValue> {
        let stats = self.backend.stats();
        let obj = Object::new();

        Reflect::set(
            &obj,
            &"models_loaded".into(),
            &JsValue::from(stats.models_loaded as u32),
        )?;
        Reflect::set(
            &obj,
            &"total_inferences".into(),
            &JsValue::from(stats.total_inferences as f64),
        )?;
        Reflect::set(
            &obj,
            &"avg_latency_ns".into(),
            &JsValue::from(stats.avg_latency_ns as f64),
        )?;
        Reflect::set(
            &obj,
            &"early_exits".into(),
            &JsValue::from(stats.early_exits as f64),
        )?;
        Reflect::set(
            &obj,
            &"skipped".into(),
            &JsValue::from(stats.skipped as f64),
        )?;

        Ok(obj.into())
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility function to create a micro shape configuration
#[wasm_bindgen(js_name = microShape)]
pub fn micro_shape() -> Result<JsValue, JsValue> {
    let shape = FixedShape::micro();
    let obj = Object::new();

    Reflect::set(&obj, &"seq_len".into(), &JsValue::from(shape.seq_len))?;
    Reflect::set(&obj, &"d_model".into(), &JsValue::from(shape.d_model))?;
    Reflect::set(&obj, &"heads".into(), &JsValue::from(shape.heads))?;
    Reflect::set(&obj, &"d_head".into(), &JsValue::from(shape.d_head))?;
    Reflect::set(&obj, &"vocab".into(), &JsValue::from(shape.vocab))?;

    Ok(obj.into())
}

/// Utility function to validate an artifact without loading
#[wasm_bindgen(js_name = validateArtifact)]
pub fn validate_artifact(artifact_bytes: &[u8]) -> Result<JsValue, JsValue> {
    let artifact = unpack_artifact(artifact_bytes)
        .map_err(|e| JsValue::from_str(&format!("Invalid artifact: {}", e)))?;

    artifact
        .validate()
        .map_err(|e| JsValue::from_str(&format!("Validation failed: {}", e)))?;

    let obj = Object::new();
    Reflect::set(&obj, &"name".into(), &artifact.manifest.name.into())?;
    Reflect::set(&obj, &"valid".into(), &JsValue::TRUE)?;

    Ok(obj.into())
}
