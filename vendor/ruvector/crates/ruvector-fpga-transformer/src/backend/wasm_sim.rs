//! WASM Simulator backend
//!
//! Pure Rust implementation that runs in WASM environments.
//! Uses RefCell for interior mutability since WASM is single-threaded.

#![cfg(feature = "wasm")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::artifact::ModelArtifact;
use crate::backend::{compute_topk, validate_tokens, BackendStats, TransformerBackend};
use crate::error::{Error, Result};
use crate::gating::CoherenceGate;
use crate::quant::{dequantize_i8, quantize_i16};
use crate::types::{
    BackendKind, FixedShape, GateDecision, GateHint, InferenceRequest, InferenceResult, ModelId,
    QuantSpec, WitnessLog,
};

/// Loaded model for WASM simulation
struct WasmModel {
    /// Model artifact
    artifact: ModelArtifact,
    /// Prepacked embedding table (dequantized to f32 for computation)
    embeddings: Vec<f32>,
    /// Number of layers
    num_layers: usize,
    /// Shape info
    shape: FixedShape,
}

/// WASM simulator backend state (interior mutable for single-threaded WASM)
struct WasmState {
    /// Loaded models
    models: HashMap<ModelId, WasmModel>,
    /// Statistics
    stats: BackendStats,
}

/// WASM simulator backend
///
/// Uses RefCell for interior mutability since WASM is inherently single-threaded.
/// This allows the TransformerBackend trait to be implemented with &self methods.
pub struct WasmSimBackend {
    /// Interior mutable state
    state: RefCell<WasmState>,
    /// Coherence gate (immutable, shared)
    gate: Rc<dyn CoherenceGate>,
}

impl WasmSimBackend {
    /// Create a new WASM simulator backend
    pub fn new(gate: Rc<dyn CoherenceGate>) -> Self {
        Self {
            state: RefCell::new(WasmState {
                models: HashMap::new(),
                stats: BackendStats::default(),
            }),
            gate,
        }
    }

    /// Prepare model from artifact
    fn prepare_model(&self, artifact: &ModelArtifact) -> Result<WasmModel> {
        let shape = artifact.manifest.shape;
        let quant = &artifact.manifest.quant;
        let d_model = shape.d_model as usize;
        let vocab = shape.vocab as usize;

        // Dequantize embeddings
        let embedding_size = vocab * d_model;
        let embeddings = if artifact.weights.len() >= embedding_size {
            dequantize_i8(&artifact.weights[..embedding_size], quant)
        } else {
            // Generate deterministic embeddings for testing
            (0..embedding_size)
                .map(|i| ((i as f32 * 0.001).sin() * 0.1))
                .collect()
        };

        // Determine number of layers from artifact or default
        let num_layers = if artifact.manifest.backend.options.early_exit {
            6
        } else {
            4
        };

        Ok(WasmModel {
            artifact: artifact.clone(),
            embeddings,
            num_layers,
            shape,
        })
    }

    /// Run inference for WASM
    fn run_inference(
        &self,
        model: &WasmModel,
        tokens: &[u16],
        gate_hint: &GateHint,
    ) -> (Vec<i16>, GateDecision) {
        let shape = &model.shape;

        // Check preflight
        let preflight = self.gate.preflight(gate_hint);
        if !preflight.did_run() {
            return (vec![0i16; shape.vocab as usize], preflight);
        }

        let vocab = shape.vocab as usize;
        let d_model = shape.d_model as usize;

        // Initialize hidden state from embeddings
        let seq_len = tokens.len();
        let mut hidden = vec![0.0f32; seq_len * d_model];

        // Lookup embeddings with bounds checking
        for (i, &token) in tokens.iter().enumerate() {
            let offset = (token as usize).min(vocab.saturating_sub(1)) * d_model;
            if offset + d_model <= model.embeddings.len() {
                hidden[i * d_model..(i + 1) * d_model]
                    .copy_from_slice(&model.embeddings[offset..offset + d_model]);
            }
        }

        // Run through simplified layers with early exit support
        for layer in 0..model.num_layers {
            // Simple layer computation (for WASM we keep it lightweight)
            // Apply simple transformation
            for t in 0..seq_len {
                let start = t * d_model;
                // Simple ReLU-like activation
                for i in 0..d_model {
                    hidden[start + i] =
                        hidden[start + i].max(0.0) * 0.99 + hidden[start + i] * 0.01;
                }
            }

            // Check for early exit
            let coherence_signal = compute_coherence(&hidden);
            if let Some(decision) = self.gate.checkpoint(layer as u8, coherence_signal) {
                let logits = self.compute_output(&hidden, model);
                return (logits, decision);
            }
        }

        // Compute output logits
        let logits = self.compute_output(&hidden, model);
        (logits, GateDecision::RanFull)
    }

    /// Compute output logits from hidden state
    fn compute_output(&self, hidden: &[f32], model: &WasmModel) -> Vec<i16> {
        let shape = &model.shape;
        let d_model = shape.d_model as usize;
        let vocab = shape.vocab as usize;
        let seq_len = hidden.len() / d_model;

        // Take last token's hidden state
        let last_hidden = &hidden[(seq_len.saturating_sub(1)) * d_model..];

        // Compute logits via dot product with embedding matrix (transposed)
        let mut logits_f32 = vec![0.0f32; vocab];
        for v in 0..vocab.min(model.embeddings.len() / d_model) {
            let v_offset = v * d_model;
            let mut dot = 0.0f32;
            for d in 0..d_model.min(last_hidden.len()) {
                if v_offset + d < model.embeddings.len() {
                    dot += last_hidden[d] * model.embeddings[v_offset + d];
                }
            }
            logits_f32[v] = dot;
        }

        // Apply softmax and quantize
        softmax_inplace(&mut logits_f32);
        quantize_i16(&logits_f32)
    }
}

// Note: WASM is single-threaded, so these trait bounds are satisfied trivially
// by never actually being used across threads
unsafe impl Send for WasmSimBackend {}
unsafe impl Sync for WasmSimBackend {}

impl TransformerBackend for WasmSimBackend {
    fn load(&self, artifact: &ModelArtifact) -> Result<ModelId> {
        // Validate artifact
        artifact.validate()?;

        // Prepare model
        let model = self.prepare_model(artifact)?;
        let model_id = artifact.model_id();

        // Store in state
        let mut state = self.state.borrow_mut();
        state.models.insert(model_id, model);
        state.stats.models_loaded += 1;

        Ok(model_id)
    }

    fn infer(&self, req: InferenceRequest) -> Result<InferenceResult> {
        let start = js_sys::Date::now();

        // Validate request
        req.validate()?;

        // Get model (immutable borrow)
        let state = self.state.borrow();
        let model = state
            .models
            .get(&req.model)
            .ok_or_else(|| Error::ModelNotFound(req.model))?;

        // Validate tokens
        validate_tokens(req.tokens, model.shape.vocab)?;

        // Run inference
        let (logits, gate_decision) = self.run_inference(model, req.tokens, &req.gate_hint);

        let latency_ns = ((js_sys::Date::now() - start) * 1_000_000.0) as u32;

        // Compute top-K
        let topk = compute_topk(&logits, 16);

        // Build witness
        let witness = WitnessLog::new(
            model.artifact.model_hash(),
            model.artifact.quant_hash(),
            BackendKind::WasmSim,
            0, // No cycles for WASM sim
            latency_ns,
            gate_decision,
        );

        drop(state); // Release borrow before mutable borrow

        // Update stats
        {
            let mut state = self.state.borrow_mut();
            state.stats.total_inferences += 1;
            let n = state.stats.total_inferences;
            state.stats.avg_latency_ns =
                (state.stats.avg_latency_ns * (n - 1) + latency_ns as u64) / n;
            match gate_decision {
                GateDecision::EarlyExit { .. } => state.stats.early_exits += 1,
                GateDecision::Skipped { .. } => state.stats.skipped += 1,
                _ => {}
            }
        }

        Ok(InferenceResult::new(logits, Some(topk), witness))
    }

    fn unload(&self, model: ModelId) -> Result<()> {
        let mut state = self.state.borrow_mut();
        if state.models.remove(&model).is_some() {
            state.stats.models_loaded = state.stats.models_loaded.saturating_sub(1);
            Ok(())
        } else {
            Err(Error::ModelNotFound(model))
        }
    }

    fn is_loaded(&self, model: ModelId) -> bool {
        self.state.borrow().models.contains_key(&model)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::WasmSim
    }

    fn stats(&self) -> BackendStats {
        self.state.borrow().stats.clone()
    }
}

/// Compute coherence signal from hidden state
fn compute_coherence(hidden: &[f32]) -> i16 {
    if hidden.is_empty() {
        return 0;
    }
    let mean = hidden.iter().sum::<f32>() / hidden.len() as f32;
    let variance = hidden.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / hidden.len() as f32;
    ((variance * 256.0).clamp(-32768.0, 32767.0)) as i16
}

/// In-place softmax
fn softmax_inplace(x: &mut [f32]) {
    if x.is_empty() {
        return;
    }
    let max = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0f32;
    for v in x.iter_mut() {
        *v = (*v - max).exp();
        sum += *v;
    }
    if sum > 0.0 {
        for v in x.iter_mut() {
            *v /= sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Manifest;
    use crate::gating::DefaultCoherenceGate;

    fn create_test_artifact() -> ModelArtifact {
        let manifest = Manifest {
            name: "wasm_test".into(),
            model_hash: "0".repeat(64),
            shape: FixedShape::micro(),
            quant: QuantSpec::int8(),
            io: Default::default(),
            backend: Default::default(),
            tests: Default::default(),
        };

        ModelArtifact {
            manifest,
            weights: vec![0u8; 4096 * 64],
            bitstream: None,
            calibration: None,
            test_vectors: vec![],
            signature: [0u8; 64],
            pubkey: [0u8; 32],
        }
    }

    #[test]
    fn test_wasm_sim_prepare_model() {
        let gate = Rc::new(DefaultCoherenceGate::new());
        let backend = WasmSimBackend::new(gate);
        let artifact = create_test_artifact();

        let model = backend.prepare_model(&artifact).unwrap();
        assert_eq!(model.shape.seq_len, 32);
        assert!(!model.embeddings.is_empty());
    }
}
