//! Native Rust simulator backend
//!
//! Provides a pure-Rust implementation of the transformer inference
//! for testing, development, and fallback when no FPGA is available.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::artifact::ModelArtifact;
use crate::backend::{
    compute_topk, read_lock, validate_tokens, write_lock, BackendStats, TransformerBackend,
};
use crate::error::{Error, Result};
use crate::gating::CoherenceGate;
use crate::quant::{dequantize_i8, quantize_i16, softmax_lut};
use crate::types::{
    BackendKind, FixedShape, GateDecision, GateHint, InferenceRequest, InferenceResult, ModelId,
    QuantSpec, SkipReason, WitnessLog,
};

/// Loaded model data for native simulation
struct LoadedModel {
    /// Model artifact (contains weights and config)
    artifact: ModelArtifact,
    /// Precomputed embedding matrix (dequantized for sim)
    embeddings: Vec<f32>,
    /// Layer weights (simplified for simulation)
    layers: Vec<LayerWeights>,
    /// Output projection
    output_proj: Vec<f32>,
}

/// Simplified layer weights for simulation
struct LayerWeights {
    /// Attention Q projection
    wq: Vec<f32>,
    /// Attention K projection
    wk: Vec<f32>,
    /// Attention V projection
    wv: Vec<f32>,
    /// Attention output projection
    wo: Vec<f32>,
    /// FFN up projection
    w1: Vec<f32>,
    /// FFN down projection
    w2: Vec<f32>,
    /// Layer norm weights
    ln1_weight: Vec<f32>,
    ln2_weight: Vec<f32>,
}

/// Native simulator backend
pub struct NativeSimBackend {
    /// Loaded models
    models: RwLock<HashMap<ModelId, Arc<LoadedModel>>>,
    /// Coherence gate
    gate: Arc<dyn CoherenceGate>,
    /// Statistics
    stats: RwLock<BackendStats>,
    /// Configuration
    config: NativeSimConfig,
}

/// Configuration for native simulator
#[derive(Debug, Clone)]
pub struct NativeSimConfig {
    /// Maximum models to keep loaded
    pub max_models: usize,
    /// Enable detailed tracing
    pub trace: bool,
    /// Use LUT-based softmax
    pub lut_softmax: bool,
    /// Number of layers to simulate (0 = all)
    pub max_layers: usize,
}

impl Default for NativeSimConfig {
    fn default() -> Self {
        Self {
            max_models: 8,
            trace: false,
            lut_softmax: true,
            max_layers: 0,
        }
    }
}

impl NativeSimBackend {
    /// Create a new native simulator backend
    pub fn new(gate: Arc<dyn CoherenceGate>) -> Self {
        Self::with_config(gate, NativeSimConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(gate: Arc<dyn CoherenceGate>, config: NativeSimConfig) -> Self {
        Self {
            models: RwLock::new(HashMap::new()),
            gate,
            stats: RwLock::new(BackendStats::default()),
            config,
        }
    }

    /// Run the core transformer inference
    fn run_inference(
        &self,
        model: &LoadedModel,
        tokens: &[u16],
        _attn_mask: &[u8],
        gate_hint: &GateHint,
    ) -> Result<(Vec<i16>, GateDecision)> {
        let shape = &model.artifact.manifest.shape;
        let num_layers = model.layers.len();

        // Check preflight gate
        let preflight = self.gate.preflight(gate_hint);
        if let GateDecision::Skipped { reason } = preflight {
            return Ok((
                vec![0i16; shape.vocab as usize],
                GateDecision::Skipped { reason },
            ));
        }

        // Initialize hidden states from embeddings
        let d_model = shape.d_model as usize;
        let seq_len = tokens.len();
        let mut hidden = vec![0.0f32; seq_len * d_model];

        // Lookup embeddings
        for (i, &token) in tokens.iter().enumerate() {
            let offset = (token as usize) * d_model;
            if offset + d_model <= model.embeddings.len() {
                hidden[i * d_model..(i + 1) * d_model]
                    .copy_from_slice(&model.embeddings[offset..offset + d_model]);
            }
        }

        // Run through layers
        let max_layers = if self.config.max_layers > 0 {
            self.config.max_layers.min(num_layers)
        } else {
            num_layers
        };

        for layer_idx in 0..max_layers {
            let layer = &model.layers[layer_idx];

            // Check layer checkpoint for early exit
            let coherence_signal = self.compute_coherence_signal(&hidden);
            if let Some(decision) = self.gate.checkpoint(layer_idx as u8, coherence_signal) {
                if let GateDecision::EarlyExit { layer } = decision {
                    // Early exit - compute output from current hidden state
                    let logits = self.compute_output(&hidden, &model.output_proj, shape);
                    return Ok((logits, GateDecision::EarlyExit { layer }));
                }
            }

            // Simplified attention + FFN (for simulation purposes)
            hidden = self.run_layer(&hidden, layer, shape);
        }

        // Compute output logits
        let logits = self.compute_output(&hidden, &model.output_proj, shape);

        Ok((logits, GateDecision::RanFull))
    }

    /// Run a single transformer layer
    fn run_layer(&self, hidden: &[f32], layer: &LayerWeights, shape: &FixedShape) -> Vec<f32> {
        let d_model = shape.d_model as usize;
        let seq_len = hidden.len() / d_model;

        // Simplified layer computation
        // In a real implementation, this would do full attention + FFN

        let mut output = hidden.to_vec();

        // Layer norm 1
        for t in 0..seq_len {
            let start = t * d_model;
            let end = start + d_model;
            layer_norm_inplace(&mut output[start..end], &layer.ln1_weight);
        }

        // Simplified attention (just apply output projection as placeholder)
        // Real implementation would compute Q, K, V, attention scores, etc.
        if !layer.wo.is_empty() {
            let mut attn_out = vec![0.0f32; output.len()];
            for t in 0..seq_len {
                for i in 0..d_model {
                    let mut sum = 0.0f32;
                    for j in 0..d_model.min(layer.wo.len() / d_model) {
                        sum += output[t * d_model + j] * layer.wo[j * d_model + i];
                    }
                    attn_out[t * d_model + i] = sum;
                }
            }
            // Residual connection
            for i in 0..output.len() {
                output[i] += attn_out[i];
            }
        }

        // Layer norm 2
        for t in 0..seq_len {
            let start = t * d_model;
            let end = start + d_model;
            layer_norm_inplace(&mut output[start..end], &layer.ln2_weight);
        }

        // Simplified FFN (SwiGLU-like)
        if !layer.w1.is_empty() && !layer.w2.is_empty() {
            let ffn_dim = layer.w1.len() / d_model;
            let mut ffn_out = vec![0.0f32; output.len()];

            for t in 0..seq_len {
                // Up projection
                let mut up = vec![0.0f32; ffn_dim];
                for i in 0..ffn_dim {
                    for j in 0..d_model {
                        up[i] += output[t * d_model + j] * layer.w1[j * ffn_dim + i];
                    }
                    // SiLU activation
                    up[i] = up[i] * sigmoid(up[i]);
                }

                // Down projection
                for i in 0..d_model {
                    for j in 0..ffn_dim.min(layer.w2.len() / d_model) {
                        ffn_out[t * d_model + i] += up[j] * layer.w2[j * d_model + i];
                    }
                }
            }

            // Residual connection
            for i in 0..output.len() {
                output[i] += ffn_out[i];
            }
        }

        output
    }

    /// Compute output logits from hidden state
    fn compute_output(&self, hidden: &[f32], output_proj: &[f32], shape: &FixedShape) -> Vec<i16> {
        let d_model = shape.d_model as usize;
        let vocab = shape.vocab as usize;
        let seq_len = hidden.len() / d_model;

        // Take last token's hidden state
        let last_hidden = &hidden[(seq_len - 1) * d_model..];

        // Compute logits
        let mut logits = vec![0.0f32; vocab];
        if output_proj.len() >= d_model * vocab {
            for v in 0..vocab {
                for d in 0..d_model {
                    logits[v] += last_hidden[d] * output_proj[d * vocab + v];
                }
            }
        } else {
            // Fallback: random logits for simulation when weights not available
            for v in 0..vocab {
                logits[v] = (v as f32 * 0.01).sin();
            }
        }

        // Apply softmax (optional) and quantize
        if self.config.lut_softmax {
            softmax_lut(&mut logits);
        } else {
            softmax_f32(&mut logits);
        }

        // Quantize to i16
        quantize_i16(&logits)
    }

    /// Compute coherence signal for early exit decision
    fn compute_coherence_signal(&self, hidden: &[f32]) -> i16 {
        // Simple coherence metric: variance of hidden states
        let mean = hidden.iter().sum::<f32>() / hidden.len() as f32;
        let variance = hidden.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / hidden.len() as f32;

        // Scale to Q8.8 fixed point
        ((variance * 256.0).clamp(-32768.0, 32767.0)) as i16
    }

    /// Prepare model from artifact (dequantize weights for simulation)
    fn prepare_model(&self, artifact: &ModelArtifact) -> Result<LoadedModel> {
        let shape = &artifact.manifest.shape;
        let quant = &artifact.manifest.quant;
        let d_model = shape.d_model as usize;
        let vocab = shape.vocab as usize;

        // Dequantize embeddings
        let embedding_size = vocab * d_model;
        let embeddings = if artifact.weights.len() >= embedding_size {
            dequantize_i8(&artifact.weights[..embedding_size], quant)
        } else {
            // Generate random embeddings for testing
            (0..embedding_size)
                .map(|i| ((i as f32 * 0.001).sin() * 0.1))
                .collect()
        };

        // Create simplified layer weights
        let num_layers = 4; // Default for simulation
        let layers: Vec<LayerWeights> = (0..num_layers)
            .map(|_| LayerWeights {
                wq: vec![0.01; d_model * d_model],
                wk: vec![0.01; d_model * d_model],
                wv: vec![0.01; d_model * d_model],
                wo: vec![0.01; d_model * d_model],
                w1: vec![0.01; d_model * 4 * d_model],
                w2: vec![0.01; 4 * d_model * d_model],
                ln1_weight: vec![1.0; d_model],
                ln2_weight: vec![1.0; d_model],
            })
            .collect();

        // Output projection
        let output_proj = vec![0.01; d_model * vocab];

        Ok(LoadedModel {
            artifact: artifact.clone(),
            embeddings,
            layers,
            output_proj,
        })
    }
}

impl TransformerBackend for NativeSimBackend {
    fn load(&self, artifact: &ModelArtifact) -> Result<ModelId> {
        // Validate artifact
        artifact.validate()?;

        // Prepare model
        let model = self.prepare_model(artifact)?;
        let model_id = artifact.model_id();

        // Check capacity (with poison handling)
        let at_capacity = read_lock(&self.models, |models| {
            models.len() >= self.config.max_models && !models.contains_key(&model_id)
        })?;

        if at_capacity {
            return Err(Error::ResourceExhausted("Max models reached".into()));
        }

        // Store model
        write_lock(&self.models, |models| {
            models.insert(model_id, Arc::new(model));
        })?;

        // Update stats
        write_lock(&self.stats, |stats| {
            stats.models_loaded += 1;
        })?;

        Ok(model_id)
    }

    fn infer(&self, req: InferenceRequest) -> Result<InferenceResult> {
        let start = Instant::now();

        // Validate request
        req.validate()?;

        // Get model (with poison handling)
        let model = read_lock(&self.models, |models| models.get(&req.model).cloned())?
            .ok_or_else(|| Error::ModelNotFound(req.model))?;

        // Validate shape
        if model.artifact.manifest.shape != req.shape {
            return Err(Error::ShapeMismatch {
                expected: model.artifact.manifest.shape,
                actual: req.shape,
            });
        }

        // Validate tokens against vocabulary
        validate_tokens(req.tokens, model.artifact.manifest.shape.vocab)?;

        // Run inference
        let (logits_q, gate_decision) =
            self.run_inference(&model, req.tokens, req.attn_mask, &req.gate_hint)?;

        let latency_ns = start.elapsed().as_nanos() as u32;

        // Compute top-K using common utility
        let topk = compute_topk(&logits_q, 16);

        // Create witness
        let witness = WitnessLog::new(
            model.artifact.model_hash(),
            model.artifact.quant_hash(),
            BackendKind::NativeSim,
            0, // No cycles for simulator
            latency_ns,
            gate_decision,
        );

        // Update stats (with poison handling)
        write_lock(&self.stats, |stats| {
            stats.total_inferences += 1;
            let n = stats.total_inferences;
            stats.avg_latency_ns = (stats.avg_latency_ns * (n - 1) + latency_ns as u64) / n;
            match gate_decision {
                GateDecision::EarlyExit { .. } => stats.early_exits += 1,
                GateDecision::Skipped { .. } => stats.skipped += 1,
                _ => {}
            }
        })?;

        Ok(InferenceResult::new(logits_q, Some(topk), witness))
    }

    fn unload(&self, model: ModelId) -> Result<()> {
        let removed = write_lock(&self.models, |models| models.remove(&model).is_some())?;

        if removed {
            write_lock(&self.stats, |stats| {
                stats.models_loaded = stats.models_loaded.saturating_sub(1);
            })?;
            Ok(())
        } else {
            Err(Error::ModelNotFound(model))
        }
    }

    fn is_loaded(&self, model: ModelId) -> bool {
        read_lock(&self.models, |m| m.contains_key(&model)).unwrap_or(false)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::NativeSim
    }

    fn stats(&self) -> BackendStats {
        read_lock(&self.stats, |s| s.clone()).unwrap_or_default()
    }
}

// Helper functions

fn layer_norm_inplace(x: &mut [f32], weight: &[f32]) {
    let mean = x.iter().sum::<f32>() / x.len() as f32;
    let variance = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / x.len() as f32;
    let std = (variance + 1e-5).sqrt();

    for (i, v) in x.iter_mut().enumerate() {
        *v = (*v - mean) / std * weight.get(i).copied().unwrap_or(1.0);
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn softmax_f32(x: &mut [f32]) {
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
            name: "test_model".into(),
            model_hash: "0".repeat(64),
            shape: FixedShape::micro(),
            quant: QuantSpec::int8(),
            io: Default::default(),
            backend: Default::default(),
            tests: Default::default(),
        };

        ModelArtifact {
            manifest,
            weights: vec![0u8; 4096 * 64], // Minimal embedding weights
            bitstream: None,
            calibration: None,
            test_vectors: vec![],
            signature: [0u8; 64],
            pubkey: [0u8; 32],
        }
    }

    #[test]
    fn test_native_sim_load_unload() {
        let gate = Arc::new(DefaultCoherenceGate::new());
        let backend = NativeSimBackend::new(gate);

        let artifact = create_test_artifact();
        let model_id = backend.load(&artifact).unwrap();

        assert!(backend.is_loaded(model_id));

        backend.unload(model_id).unwrap();
        assert!(!backend.is_loaded(model_id));
    }

    #[test]
    fn test_native_sim_inference() {
        let gate = Arc::new(DefaultCoherenceGate::new());
        let backend = NativeSimBackend::new(gate);

        let artifact = create_test_artifact();
        let model_id = backend.load(&artifact).unwrap();

        let tokens: Vec<u16> = (0..32).collect();
        let mask = vec![1u8; 32];

        let req = InferenceRequest::new(
            model_id,
            FixedShape::micro(),
            &tokens,
            &mask,
            GateHint::allow_all(),
        );

        let result = backend.infer(req).unwrap();

        assert!(!result.logits_q.is_empty());
        assert!(result.topk.is_some());
        assert_eq!(result.witness.backend, BackendKind::NativeSim);
    }
}
