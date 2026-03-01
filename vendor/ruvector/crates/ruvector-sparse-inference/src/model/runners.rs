//! Model runners for different architectures with sparse inference support

use crate::error::SparseInferenceError;
use crate::model::loader::{ModelLoader, ModelMetadata};
use crate::model::types::{CalibrationStats, InferenceConfig, ModelInput, ModelOutput, Tensor};
use crate::ops::{silu, Embedding, LayerNorm, Linear, RMSNorm};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, SparseInferenceError>;

/// Trait for running inference on models
pub trait ModelRunner {
    /// Forward pass with optional sparse computation
    fn forward(&self, input: &ModelInput, config: &InferenceConfig) -> Result<ModelOutput>;

    /// Get predictor for a specific layer (if available)
    fn get_predictor(&self, layer_idx: usize) -> Option<&LowRankPredictor>;

    /// Calibrate predictors with sample data
    fn calibrate(&mut self, samples: &[ModelInput]) -> Result<CalibrationStats>;

    /// Get model metadata
    fn metadata(&self) -> &ModelMetadata;
}

/// Low-rank predictor for neuron activation prediction
#[derive(Debug, Clone)]
pub struct LowRankPredictor {
    pub u: Vec<Vec<f32>>, // U matrix (d x r)
    pub v: Vec<Vec<f32>>, // V matrix (r x m)
    pub rank: usize,
}

impl LowRankPredictor {
    pub fn new(input_dim: usize, output_dim: usize, rank: usize) -> Self {
        Self {
            u: vec![vec![0.0; rank]; input_dim],
            v: vec![vec![0.0; output_dim]; rank],
            rank,
        }
    }

    /// Predict top-k active neurons
    pub fn predict_active(&self, input: &[f32], k: usize) -> Vec<usize> {
        let scores = self.forward(input);
        let mut indices: Vec<usize> = (0..scores.len()).collect();
        indices.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap());
        indices.truncate(k);
        indices
    }

    fn forward(&self, input: &[f32]) -> Vec<f32> {
        // Compute UV^T · input in two steps
        // First: U^T · input (r-dimensional)
        let mut hidden = vec![0.0; self.rank];
        for i in 0..self.rank {
            for (j, u_ji) in self.u.iter().enumerate() {
                if j < input.len() && i < u_ji.len() {
                    hidden[i] += u_ji[i] * input[j];
                }
            }
        }

        // Second: V · hidden (m-dimensional)
        let output_dim = self.v.first().map(|v| v.len()).unwrap_or(0);
        let mut output = vec![0.0; output_dim];
        for i in 0..output_dim {
            for (j, &h) in hidden.iter().enumerate() {
                if j < self.v.len() && i < self.v[j].len() {
                    output[i] += self.v[j][i] * h;
                }
            }
        }

        output
    }
}

// ============================================================================
// Llama Model
// ============================================================================

/// Llama model for sparse inference
pub struct LlamaModel {
    pub metadata: ModelMetadata,
    pub layers: Vec<LlamaLayer>,
    pub embed_tokens: Embedding,
    pub norm: RMSNorm,
    pub lm_head: Option<Linear>,
}

pub struct LlamaLayer {
    pub input_layernorm: RMSNorm,
    pub self_attn: LlamaAttention,
    pub post_attention_layernorm: RMSNorm,
    pub mlp: LlamaMLP,
    pub predictor: Option<LowRankPredictor>,
}

pub struct LlamaAttention {
    pub q_proj: Linear,
    pub k_proj: Linear,
    pub v_proj: Linear,
    pub o_proj: Linear,
    pub num_heads: usize,
    pub head_dim: usize,
}

pub struct LlamaMLP {
    pub gate_proj: Linear, // W1 for SwiGLU gate
    pub up_proj: Linear,   // W3 for SwiGLU up
    pub down_proj: Linear, // W2 for down projection
}

impl LlamaMLP {
    /// Standard forward pass (dense)
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        let gate = self.gate_proj.forward(x);
        let up = self.up_proj.forward(x);

        // SwiGLU: silu(gate) ⊙ up
        let hidden: Vec<f32> = gate
            .iter()
            .zip(up.iter())
            .map(|(&g, &u)| silu(g) * u)
            .collect();

        self.down_proj.forward(&hidden)
    }

    /// Sparse forward pass using predictor
    pub fn forward_sparse(&self, x: &[f32], active_neurons: &[usize]) -> Vec<f32> {
        // Only compute for active neurons in intermediate layer
        let gate = sparse_matmul(&self.gate_proj, x, active_neurons);
        let up = sparse_matmul(&self.up_proj, x, active_neurons);

        // SwiGLU on active neurons only
        let hidden: Vec<f32> = gate
            .iter()
            .zip(up.iter())
            .map(|(&g, &u)| silu(g) * u)
            .collect();

        // Sparse down projection
        sparse_matmul_full(&self.down_proj, &hidden, active_neurons)
    }
}

impl ModelRunner for LlamaModel {
    fn forward(&self, input: &ModelInput, config: &InferenceConfig) -> Result<ModelOutput> {
        // Embed tokens
        let mut hidden_states = self.embed_tokens.forward(&input.input_ids);

        let mut all_hidden_states = if config.output_hidden_states {
            Some(Vec::new())
        } else {
            None
        };

        // Process each layer
        for (idx, layer) in self.layers.iter().enumerate() {
            if let Some(ref mut states) = all_hidden_states {
                states.push(hidden_states.clone());
            }

            // Layer norm
            let normed = layer.input_layernorm.forward(&hidden_states);

            // Self-attention (simplified, no KV cache)
            let attn_output = layer.self_attn.forward(&normed);

            // Residual
            hidden_states = add_vectors(&hidden_states, &attn_output);

            // Post-attention norm
            let normed = layer.post_attention_layernorm.forward(&hidden_states);

            // MLP with optional sparsity
            let mlp_output = if config.use_sparse_ffn {
                if let Some(ref predictor) = layer.predictor {
                    let k = config.active_neurons_per_layer.unwrap_or(
                        (self.metadata.intermediate_size as f32 * (1.0 - config.sparsity)) as usize,
                    );
                    let active = predictor.predict_active(&normed, k);
                    layer.mlp.forward_sparse(&normed, &active)
                } else {
                    layer.mlp.forward(&normed)
                }
            } else {
                layer.mlp.forward(&normed)
            };

            // Residual
            hidden_states = add_vectors(&hidden_states, &mlp_output);
        }

        // Final norm
        hidden_states = self.norm.forward(&hidden_states);

        // LM head
        let logits = if let Some(ref lm_head) = self.lm_head {
            lm_head.forward(&hidden_states)
        } else {
            hidden_states
        };

        Ok(ModelOutput::new(logits).with_hidden_states(all_hidden_states.unwrap_or_default()))
    }

    fn get_predictor(&self, layer_idx: usize) -> Option<&LowRankPredictor> {
        self.layers.get(layer_idx)?.predictor.as_ref()
    }

    fn calibrate(&mut self, samples: &[ModelInput]) -> Result<CalibrationStats> {
        // Placeholder: would collect activation statistics
        Ok(CalibrationStats {
            num_samples: samples.len(),
            average_sparsity: 0.9,
            layer_stats: HashMap::new(),
        })
    }

    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }
}

impl LlamaAttention {
    pub fn forward(&self, hidden_states: &[f32]) -> Vec<f32> {
        // Simplified: full attention without KV cache
        let q = self.q_proj.forward(hidden_states);
        let k = self.k_proj.forward(hidden_states);
        let v = self.v_proj.forward(hidden_states);

        // Placeholder: would do scaled dot-product attention
        self.o_proj.forward(&q)
    }
}

// ============================================================================
// LFM2 Model (Liquid AI)
// ============================================================================

pub struct LFM2Model {
    pub metadata: ModelMetadata,
    pub embedding: Embedding,
    pub layers: Vec<LFM2Layer>,
    pub pooler: Option<Pooler>,
}

pub struct LFM2Layer {
    pub gated_conv: GatedConv1d,
    pub attention: GroupedQueryAttention,
    pub ffn: SparseFfn,
    pub norm: LayerNorm,
}

pub struct GatedConv1d {
    pub weight: Vec<Vec<f32>>,
    pub gate: Linear,
}

pub struct GroupedQueryAttention {
    pub q_proj: Linear,
    pub k_proj: Linear,
    pub v_proj: Linear,
    pub o_proj: Linear,
    pub num_groups: usize,
}

pub struct SparseFfn {
    pub w1: Linear,
    pub w2: Linear,
    pub predictor: Option<LowRankPredictor>,
}

impl ModelRunner for LFM2Model {
    fn forward(&self, input: &ModelInput, config: &InferenceConfig) -> Result<ModelOutput> {
        let mut hidden = self.embedding.forward(&input.input_ids);

        for layer in &self.layers {
            // Gated convolution for local context
            hidden = layer.gated_conv.forward(&hidden);

            // Grouped query attention
            let attn_out = layer.attention.forward(&hidden);
            hidden = add_vectors(&hidden, &attn_out);

            // Sparse FFN
            let ffn_out = layer.ffn.forward(&hidden, config);
            hidden = add_vectors(&hidden, &ffn_out);

            hidden = layer.norm.forward(&hidden);
        }

        Ok(ModelOutput::new(hidden))
    }

    fn get_predictor(&self, layer_idx: usize) -> Option<&LowRankPredictor> {
        self.layers.get(layer_idx)?.ffn.predictor.as_ref()
    }

    fn calibrate(&mut self, _samples: &[ModelInput]) -> Result<CalibrationStats> {
        Ok(CalibrationStats {
            num_samples: 0,
            average_sparsity: 0.9,
            layer_stats: HashMap::new(),
        })
    }

    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }
}

impl GatedConv1d {
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        // Simplified convolution
        x.to_vec()
    }
}

impl GroupedQueryAttention {
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        self.o_proj.forward(x)
    }
}

impl SparseFfn {
    pub fn forward(&self, x: &[f32], config: &InferenceConfig) -> Vec<f32> {
        if config.use_sparse_ffn {
            if let Some(ref predictor) = self.predictor {
                let k = (self.w1.out_features as f32 * (1.0 - config.sparsity)) as usize;
                let active = predictor.predict_active(x, k);
                return sparse_matmul_full(&self.w2, &self.w1.forward(x), &active);
            }
        }
        self.w2.forward(&self.w1.forward(x))
    }
}

// ============================================================================
// BERT Model
// ============================================================================

pub struct BertModel {
    pub metadata: ModelMetadata,
    pub embeddings: BertEmbeddings,
    pub encoder: Vec<BertLayer>,
    pub pooler: Option<Pooler>,
}

pub struct BertEmbeddings {
    pub word_embeddings: Embedding,
    pub position_embeddings: Embedding,
    pub token_type_embeddings: Embedding,
    pub layer_norm: LayerNorm,
}

pub struct BertLayer {
    pub attention: MultiHeadAttention,
    pub intermediate: Linear,
    pub output: Linear,
    pub layer_norm1: LayerNorm,
    pub layer_norm2: LayerNorm,
}

pub struct MultiHeadAttention {
    pub q_proj: Linear,
    pub k_proj: Linear,
    pub v_proj: Linear,
    pub o_proj: Linear,
    pub num_heads: usize,
}

pub struct Pooler {
    pub dense: Linear,
}

impl ModelRunner for BertModel {
    fn forward(&self, input: &ModelInput, config: &InferenceConfig) -> Result<ModelOutput> {
        let mut hidden = self.embeddings.forward(&input.input_ids);

        for layer in &self.encoder {
            let attn_out = layer.attention.forward(&hidden);
            hidden = layer.layer_norm1.forward(&add_vectors(&hidden, &attn_out));

            let intermediate = layer.intermediate.forward(&hidden);
            let output = layer.output.forward(&intermediate);
            hidden = layer.layer_norm2.forward(&add_vectors(&hidden, &output));
        }

        Ok(ModelOutput::new(hidden))
    }

    fn get_predictor(&self, _layer_idx: usize) -> Option<&LowRankPredictor> {
        None
    }

    fn calibrate(&mut self, _samples: &[ModelInput]) -> Result<CalibrationStats> {
        Ok(CalibrationStats {
            num_samples: 0,
            average_sparsity: 0.0,
            layer_stats: HashMap::new(),
        })
    }

    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }
}

impl BertEmbeddings {
    pub fn forward(&self, input_ids: &[u64]) -> Vec<f32> {
        self.word_embeddings.forward(input_ids)
    }
}

impl MultiHeadAttention {
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        self.o_proj.forward(x)
    }
}

// ============================================================================
// Unified Model Wrapper
// ============================================================================

pub enum SparseModel {
    Llama(LlamaModel),
    LFM2(LFM2Model),
    Bert(BertModel),
}

impl ModelRunner for SparseModel {
    fn forward(&self, input: &ModelInput, config: &InferenceConfig) -> Result<ModelOutput> {
        match self {
            Self::Llama(m) => m.forward(input, config),
            Self::LFM2(m) => m.forward(input, config),
            Self::Bert(m) => m.forward(input, config),
        }
    }

    fn get_predictor(&self, layer_idx: usize) -> Option<&LowRankPredictor> {
        match self {
            Self::Llama(m) => m.get_predictor(layer_idx),
            Self::LFM2(m) => m.get_predictor(layer_idx),
            Self::Bert(m) => m.get_predictor(layer_idx),
        }
    }

    fn calibrate(&mut self, samples: &[ModelInput]) -> Result<CalibrationStats> {
        match self {
            Self::Llama(m) => m.calibrate(samples),
            Self::LFM2(m) => m.calibrate(samples),
            Self::Bert(m) => m.calibrate(samples),
        }
    }

    fn metadata(&self) -> &ModelMetadata {
        match self {
            Self::Llama(m) => m.metadata(),
            Self::LFM2(m) => m.metadata(),
            Self::Bert(m) => m.metadata(),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn sparse_matmul(linear: &Linear, input: &[f32], active_cols: &[usize]) -> Vec<f32> {
    let mut output = vec![0.0; active_cols.len()];

    for (out_idx, &col_idx) in active_cols.iter().enumerate() {
        if col_idx < linear.out_features {
            for (in_idx, &x) in input.iter().enumerate() {
                if in_idx < linear.in_features {
                    output[out_idx] += linear.weight[col_idx][in_idx] * x;
                }
            }
            if let Some(ref bias) = linear.bias {
                output[out_idx] += bias[col_idx];
            }
        }
    }

    output
}

fn sparse_matmul_full(linear: &Linear, input: &[f32], active_input_cols: &[usize]) -> Vec<f32> {
    let mut output = vec![0.0; linear.out_features];

    for out_idx in 0..linear.out_features {
        for &in_idx in active_input_cols {
            if in_idx < input.len() && in_idx < linear.in_features {
                output[out_idx] += linear.weight[out_idx][in_idx] * input[in_idx];
            }
        }
        if let Some(ref bias) = linear.bias {
            output[out_idx] += bias[out_idx];
        }
    }

    output
}

fn add_vectors(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_low_rank_predictor() {
        let predictor = LowRankPredictor::new(128, 512, 16);
        let input = vec![1.0; 128];
        let active = predictor.predict_active(&input, 10);
        assert_eq!(active.len(), 10);
    }

    #[test]
    fn test_add_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let result = add_vectors(&a, &b);
        assert_eq!(result, vec![5.0, 7.0, 9.0]);
    }
}
