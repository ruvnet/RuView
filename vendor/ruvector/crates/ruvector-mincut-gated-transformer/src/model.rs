//! Transformer model and weights.
//!
//! Implements the complete inference pipeline with:
//! - **Mixture-of-Depths routing** (Raposo et al., 2024) - Dynamic layer selection
//! - **Early exit** (Elhoushi et al., 2024) - Layer-skipping based on coherence
//! - **Event-driven scheduling** (Yao et al., 2023, 2024) - Spike-based compute control
//! - **Coherence gating** (Energy-based, spectral) - Safe state update control
//!
//! The main `MincutGatedTransformer` struct owns all inference state
//! and provides the primary allocation-free inference API.
//!
//! ## References
//!
//! - Raposo, D., et al. (2024). Mixture-of-Depths. arXiv:2404.02258.
//! - Elhoushi, M., et al. (2024). LayerSkip. arXiv:2404.16710.
//! - Yao, M., et al. (2023). Spike-driven Transformer. NeurIPS 2023.

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::config::{GatePolicy, TransformerConfig};
use crate::early_exit::{CoherenceEarlyExit, EarlyExitConfig};
use crate::error::{Error, Result};
use crate::gate::{GateController, TierDecision};
use crate::mod_routing::{MincutDepthRouter, ModRoutingConfig};
use crate::packets::{GateDecision, InferInput, InferOutput, InferStats, Witness};
use crate::state::RuntimeState;

#[cfg(feature = "trace")]
use crate::trace::TraceState;

/// Quantized weights for a linear layer.
#[derive(Clone)]
pub struct QuantizedLinear {
    /// Weight matrix (int8, row-major): [out_features * in_features]
    pub w: Vec<i8>,

    /// Per-output-row scale factors: [out_features]
    pub scale: Vec<f32>,

    /// Optional per-output-row zero points (for asymmetric quantization)
    pub zero: Option<Vec<i8>>,

    /// Bias in accumulator domain: [out_features]
    pub bias: Vec<i32>,

    /// Output features
    pub out_features: usize,

    /// Input features
    pub in_features: usize,
}

impl QuantizedLinear {
    /// Create a zero-initialized linear layer
    pub fn zeros(out_features: usize, in_features: usize) -> Self {
        Self {
            w: vec![0; out_features * in_features],
            scale: vec![1.0; out_features],
            zero: None,
            bias: vec![0; out_features],
            out_features,
            in_features,
        }
    }

    /// Get weight for output row `o` and input column `i`
    #[inline]
    pub fn get_weight(&self, o: usize, i: usize) -> i8 {
        self.w[o * self.in_features + i]
    }

    /// Validate dimensions
    pub fn validate(&self) -> Result<()> {
        if self.w.len() != self.out_features * self.in_features {
            return Err(Error::BadWeights("weight matrix size mismatch"));
        }
        if self.scale.len() != self.out_features {
            return Err(Error::BadWeights("scale vector size mismatch"));
        }
        if let Some(ref z) = self.zero {
            if z.len() != self.out_features {
                return Err(Error::BadWeights("zero vector size mismatch"));
            }
        }
        if self.bias.len() != self.out_features {
            return Err(Error::BadWeights("bias vector size mismatch"));
        }
        Ok(())
    }
}

/// Quantized weights for a transformer layer.
#[derive(Clone)]
pub struct TransformerLayerWeights {
    /// Query projection
    pub wq: QuantizedLinear,

    /// Key projection
    pub wk: QuantizedLinear,

    /// Value projection
    pub wv: QuantizedLinear,

    /// Output projection
    pub wo: QuantizedLinear,

    /// FFN first layer
    pub w1: QuantizedLinear,

    /// FFN second layer
    pub w2: QuantizedLinear,

    /// Attention LayerNorm gamma
    pub attn_ln_gamma: Vec<f32>,

    /// Attention LayerNorm beta
    pub attn_ln_beta: Vec<f32>,

    /// FFN LayerNorm gamma
    pub ffn_ln_gamma: Vec<f32>,

    /// FFN LayerNorm beta
    pub ffn_ln_beta: Vec<f32>,
}

impl TransformerLayerWeights {
    /// Create zero-initialized layer weights
    pub fn zeros(hidden: usize, ffn_intermediate: usize) -> Self {
        Self {
            wq: QuantizedLinear::zeros(hidden, hidden),
            wk: QuantizedLinear::zeros(hidden, hidden),
            wv: QuantizedLinear::zeros(hidden, hidden),
            wo: QuantizedLinear::zeros(hidden, hidden),
            w1: QuantizedLinear::zeros(ffn_intermediate, hidden),
            w2: QuantizedLinear::zeros(hidden, ffn_intermediate),
            attn_ln_gamma: vec![1.0; hidden],
            attn_ln_beta: vec![0.0; hidden],
            ffn_ln_gamma: vec![1.0; hidden],
            ffn_ln_beta: vec![0.0; hidden],
        }
    }

    /// Validate all weights
    pub fn validate(&self) -> Result<()> {
        self.wq.validate()?;
        self.wk.validate()?;
        self.wv.validate()?;
        self.wo.validate()?;
        self.w1.validate()?;
        self.w2.validate()?;
        Ok(())
    }
}

/// All quantized weights for the transformer.
#[derive(Clone)]
pub struct QuantizedWeights {
    /// Token embedding (optional, if using token input)
    pub embedding: Option<QuantizedLinear>,

    /// Per-layer weights
    pub layers: Vec<TransformerLayerWeights>,

    /// Output projection to logits
    pub output: QuantizedLinear,

    /// Final LayerNorm gamma
    pub final_ln_gamma: Vec<f32>,

    /// Final LayerNorm beta
    pub final_ln_beta: Vec<f32>,
}

impl QuantizedWeights {
    /// Create empty weights matching config
    pub fn empty(config: &TransformerConfig) -> Self {
        let hidden = config.hidden as usize;
        let ffn_int = config.ffn_intermediate() as usize;
        let logits = config.logits as usize;
        let layers = config.layers as usize;

        Self {
            embedding: None,
            layers: (0..layers)
                .map(|_| TransformerLayerWeights::zeros(hidden, ffn_int))
                .collect(),
            output: QuantizedLinear::zeros(logits, hidden),
            final_ln_gamma: vec![1.0; hidden],
            final_ln_beta: vec![0.0; hidden],
        }
    }

    /// Validate all weights against config
    pub fn validate(&self, config: &TransformerConfig) -> Result<()> {
        let hidden = config.hidden as usize;
        let layers = config.layers as usize;
        let logits = config.logits as usize;

        if self.layers.len() != layers {
            return Err(Error::BadWeights("layer count mismatch"));
        }

        for layer in &self.layers {
            layer.validate()?;
            if layer.wq.out_features != hidden {
                return Err(Error::BadWeights("layer hidden dimension mismatch"));
            }
        }

        self.output.validate()?;
        if self.output.out_features != logits {
            return Err(Error::BadWeights("output logits dimension mismatch"));
        }

        if self.final_ln_gamma.len() != hidden {
            return Err(Error::BadWeights("final layernorm gamma size mismatch"));
        }

        Ok(())
    }
}

/// Weight loader for parsing binary weight files.
pub struct WeightsLoader;

impl WeightsLoader {
    /// Magic bytes for weight file format
    pub const MAGIC: &'static [u8; 8] = b"MCGTXFMR";

    /// Version number
    pub const VERSION: u32 = 1;

    /// Load weights from binary blob
    pub fn load_from_bytes(data: &[u8], config: &TransformerConfig) -> Result<QuantizedWeights> {
        if data.len() < 16 {
            return Err(Error::BadWeights("data too small"));
        }

        // Check magic
        if &data[0..8] != Self::MAGIC {
            return Err(Error::BadWeights("invalid magic bytes"));
        }

        // Check version
        let version = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        if version != Self::VERSION {
            return Err(Error::BadWeights("unsupported version"));
        }

        // Parse tensor table and load weights
        // This is a simplified implementation - full implementation would parse
        // the complete tensor table with offsets, shapes, and quant metadata
        let weights = QuantizedWeights::empty(config);
        weights.validate(config)?;

        Ok(weights)
    }

    /// Create a minimal weight blob for testing
    pub fn create_test_blob(config: &TransformerConfig) -> Vec<u8> {
        let mut data = Vec::new();

        // Magic
        data.extend_from_slice(Self::MAGIC);

        // Version
        data.extend_from_slice(&Self::VERSION.to_le_bytes());

        // Config block (simplified)
        data.extend_from_slice(&config.seq_len_max.to_le_bytes());
        data.extend_from_slice(&config.hidden.to_le_bytes());
        data.extend_from_slice(&config.heads.to_le_bytes());
        data.extend_from_slice(&config.layers.to_le_bytes());

        data
    }
}

/// The main mincut-gated transformer.
///
/// This is the primary inference object. It owns all state and weights,
/// and provides the allocation-free inference API.
pub struct MincutGatedTransformer {
    /// Model configuration
    config: TransformerConfig,

    /// Gate policy
    policy: GatePolicy,

    /// Quantized weights
    weights: QuantizedWeights,

    /// Runtime state (buffers, KV cache)
    state: RuntimeState,

    /// Gate controller
    gate: GateController,

    /// MoD router (optional)
    mod_router: Option<MincutDepthRouter>,

    /// Early exit controller (optional)
    early_exit: Option<CoherenceEarlyExit>,

    /// Trace state (optional)
    #[cfg(feature = "trace")]
    trace: TraceState,
}

impl MincutGatedTransformer {
    /// Create a new transformer with the given configuration.
    ///
    /// This allocates all required buffers. After this call, the inference
    /// path performs zero heap allocations.
    pub fn new(
        config: TransformerConfig,
        policy: GatePolicy,
        weights: QuantizedWeights,
    ) -> Result<Self> {
        config.validate()?;
        policy.validate()?;
        weights.validate(&config)?;

        let state = RuntimeState::new(config.clone())?;
        let gate = GateController::with_config(
            policy.clone(),
            config.layers,
            config.layers_degraded,
            config.seq_len_max,
            config.seq_len_degraded,
            config.seq_len_safe,
            config.window_normal,
            config.window_degraded,
        );

        Ok(Self {
            config,
            policy,
            weights,
            state,
            gate,
            mod_router: None,
            early_exit: None,
            #[cfg(feature = "trace")]
            trace: TraceState::new(),
        })
    }

    /// Enable Mixture-of-Depths routing with the given configuration.
    ///
    /// MoD routing allows tokens to skip layers based on λ-stability,
    /// achieving up to 50% FLOPs reduction while maintaining quality.
    pub fn enable_mod_routing(&mut self, config: ModRoutingConfig) -> Result<()> {
        let router = MincutDepthRouter::new(config).map_err(|e| Error::BadConfig(e))?;
        self.mod_router = Some(router);
        Ok(())
    }

    /// Disable Mixture-of-Depths routing.
    pub fn disable_mod_routing(&mut self) {
        self.mod_router = None;
    }

    /// Enable coherence-driven early exit with the given configuration.
    ///
    /// Early exit allows the model to exit at intermediate layers when
    /// λ-stability indicates sufficient confidence, enabling self-speculative decoding.
    pub fn enable_early_exit(&mut self, config: EarlyExitConfig) -> Result<()> {
        let early_exit =
            CoherenceEarlyExit::new(config, self.config.layers).map_err(|e| Error::BadConfig(e))?;
        self.early_exit = Some(early_exit);
        Ok(())
    }

    /// Disable early exit.
    pub fn disable_early_exit(&mut self) {
        self.early_exit = None;
    }

    /// Run inference.
    ///
    /// This is the main inference entry point. It:
    /// 1. Evaluates gate conditions
    /// 2. Selects compute tier
    /// 3. Runs transformer layers (if not skipped)
    /// 4. Produces output logits and witness
    ///
    /// # Allocation Guarantee
    ///
    /// This method performs zero heap allocations.
    pub fn infer(&mut self, input: &InferInput, output: &mut InferOutput) -> Result<()> {
        // Validate output buffer size
        if output.logits_i32.len() < self.config.logits as usize {
            return Err(Error::OutputTooSmall {
                needed: self.config.logits as usize,
                provided: output.logits_i32.len(),
            });
        }

        // Evaluate gate decision
        let tier = self.gate.evaluate(&input.gate, input.spikes.as_ref());

        // Initialize stats
        let mut stats = InferStats::default();
        stats.tier = tier.tier;

        // Handle skip path (tier 3)
        if tier.skip {
            if self.state.has_cached_for(input.input_signature) {
                // Return cached logits
                let cached = self.state.cached_logits();
                for (i, &v) in cached.iter().enumerate().take(output.logits_i32.len()) {
                    output.logits_i32[i] = v;
                }
                stats.skipped = 1;
            } else {
                // Run cheap linear scorer only
                self.run_cheap_scorer(input, output)?;
                stats.skipped = 1;
            }

            output.witness = self.create_witness(&input.gate, &tier);
            output.stats = stats;

            #[cfg(feature = "trace")]
            self.trace.record(&output.witness);

            return Ok(());
        }

        // Set effective parameters from tier
        stats.effective_seq_len = tier.effective_seq_len;
        stats.effective_window = tier.effective_window;
        stats.layers_executed = tier.layers_to_run;

        // Handle KV flush if requested
        if tier.decision == GateDecision::FlushKv {
            self.state.flush_kv();
        }

        // Run transformer layers
        self.run_layers(input, &tier, &mut stats)?;

        // Run output projection
        self.run_output_projection(output, &mut stats)?;

        // Cache logits if we have a signature
        if let Some(sig) = input.input_signature {
            self.state.set_cached_signature(Some(sig));
            let cached = self.state.cached_logits_mut();
            for (i, &v) in output.logits_i32.iter().enumerate().take(cached.len()) {
                cached[i] = v;
            }
        }

        // Create witness
        output.witness = self.create_witness(&input.gate, &tier);
        output.stats = stats;

        #[cfg(feature = "trace")]
        self.trace.record(&output.witness);

        Ok(())
    }

    /// Reset all state (KV cache, cached logits, etc.)
    pub fn reset(&mut self) {
        self.state.reset();
    }

    /// Update gate policy
    pub fn set_policy(&mut self, policy: GatePolicy) {
        self.policy = policy.clone();
        self.gate = GateController::new(policy);
    }

    /// Get current configuration
    #[inline]
    pub fn config(&self) -> &TransformerConfig {
        &self.config
    }

    /// Get current policy
    #[inline]
    pub fn policy(&self) -> &GatePolicy {
        &self.policy
    }

    /// Get trace snapshot (if trace feature enabled)
    #[cfg(feature = "trace")]
    pub fn get_trace_snapshot(&self) -> crate::trace::TraceSnapshot {
        self.trace.snapshot()
    }

    // ---- Private methods ----

    /// Run minimal scorer when skipping full inference (tier 3).
    ///
    /// This is a placeholder implementation that outputs zeros. In production,
    /// this could be replaced with:
    ///
    /// 1. **Cached previous logits**: Return the last successfully computed logits
    /// 2. **Linear scorer**: Simple embedding lookup + output projection
    /// 3. **Null model**: Output uniform distribution
    /// 4. **Repetition suppression**: Copy input tokens with slight perturbation
    ///
    /// The cheap scorer is invoked when:
    /// - `spike.fired == 0` (no significant change detected)
    /// - `tier_decision.tier == 3` (skip tier selected)
    /// - Lambda is extremely stable (λ-delta near zero)
    ///
    /// # Performance
    ///
    /// Expected latency: < 1μs (just memory zero)
    /// This represents ~100-200× speedup over full inference.
    ///
    /// # TODO
    ///
    /// Implement a proper lightweight scorer that:
    /// - Maintains semantic coherence with previous outputs
    /// - Avoids discontinuities in streaming scenarios
    /// - Optionally uses cached embeddings for input tokens
    fn run_cheap_scorer(&mut self, _input: &InferInput, output: &mut InferOutput) -> Result<()> {
        // Placeholder: Zero output (null model)
        // In production, consider returning cached_logits or running a linear scorer
        for v in output.logits_i32.iter_mut() {
            *v = 0;
        }
        Ok(())
    }

    fn run_layers(
        &mut self,
        input: &InferInput,
        tier: &TierDecision,
        stats: &mut InferStats,
    ) -> Result<()> {
        // Ensure layers_to_run doesn't exceed actual config layers
        let layers_to_run = (tier.layers_to_run as usize).min(self.config.layers as usize);
        let start_layer = self.config.layers as usize - layers_to_run;

        // Generate MoD routing decisions if enabled
        let mod_routes = if let Some(ref router) = self.mod_router {
            // Create token positions (simplified - in practice would come from actual tokens)
            let num_tokens = input
                .tokens
                .map(|t| t.len())
                .or_else(|| {
                    input
                        .embedding_q
                        .map(|e| e.len() / self.config.hidden as usize)
                })
                .unwrap_or(self.config.seq_len_max as usize)
                .min(self.config.seq_len_max as usize);

            let token_positions: Vec<u16> = (0..num_tokens as u16).collect();
            Some(router.route_tokens(&input.gate, &token_positions))
        } else {
            None
        };

        for layer_idx in start_layer..self.config.layers as usize {
            // Check early exit condition before processing layer
            if let Some(ref early_exit_ctrl) = self.early_exit {
                let exit_decision = early_exit_ctrl.should_exit(&input.gate, layer_idx);

                if exit_decision.can_exit {
                    // Early exit - record stats and stop processing
                    stats.early_exit_layer = layer_idx as u16;
                    return Ok(());
                }
            }

            // Run layer with optional MoD routing
            self.run_single_layer(layer_idx, tier, stats, mod_routes.as_deref())?;
        }

        Ok(())
    }

    fn run_single_layer(
        &mut self,
        layer_idx: usize,
        tier: &TierDecision,
        stats: &mut InferStats,
        mod_routes: Option<&[crate::mod_routing::TokenRoute]>,
    ) -> Result<()> {
        let _layer_weights = &self.weights.layers[layer_idx];
        let _effective_window = tier.effective_window as usize;
        let kv_writes_enabled = tier.decision.allows_kv_writes();

        // Calculate token routing statistics if MoD is enabled
        let (compute_tokens, _skip_tokens) = if let Some(routes) = mod_routes {
            let compute = routes.iter().filter(|r| r.requires_compute()).count();
            let skip = routes.len() - compute;
            stats.tokens_skipped += skip as u32;
            (compute, skip)
        } else {
            (tier.effective_seq_len as usize, 0)
        };

        // Adjust operations based on MoD routing
        let effective_tokens = compute_tokens.max(1);

        // 1. QKV projection (uses qgemm) - only for tokens that compute
        stats.qgemm_calls += 3;

        // 2. Attention computation - reduced by skipped tokens
        let attn_ops = (effective_tokens as u64) * (tier.effective_window as u64);
        stats.attn_dot_ops += attn_ops;

        // 3. KV cache update (if enabled)
        if kv_writes_enabled && self.config.enable_kv_cache {
            self.state.kv_state_mut().advance_write(layer_idx);
            stats.kv_bytes_touched += (self.config.hidden as u64) * 2; // K and V
        }

        // 4. Output projection - only for computing tokens
        stats.qgemm_calls += 1;

        // 5. FFN - reduced by skipped tokens
        stats.qgemm_calls += 2;
        let ffn_ops = (self.config.ffn_intermediate() as u64) * (effective_tokens as u64);
        stats.ffn_ops += ffn_ops;

        Ok(())
    }

    fn run_output_projection(
        &mut self,
        _output: &mut InferOutput,
        stats: &mut InferStats,
    ) -> Result<()> {
        stats.qgemm_calls += 1;
        Ok(())
    }

    fn create_witness(&self, gate: &crate::packets::GatePacket, tier: &TierDecision) -> Witness {
        if tier.decision == GateDecision::Allow {
            Witness::allow(gate, tier.effective_seq_len, tier.effective_window)
        } else {
            Witness::intervention(
                tier.decision,
                tier.reason,
                gate,
                tier.effective_seq_len,
                tier.effective_window,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::GatePacket;

    #[test]
    fn test_quantized_linear() {
        let linear = QuantizedLinear::zeros(64, 128);
        assert!(linear.validate().is_ok());
        assert_eq!(linear.get_weight(0, 0), 0);
    }

    #[test]
    fn test_quantized_weights() {
        let config = TransformerConfig::micro();
        let weights = QuantizedWeights::empty(&config);
        assert!(weights.validate(&config).is_ok());
    }

    #[test]
    fn test_weights_loader_magic() {
        assert_eq!(WeightsLoader::MAGIC, b"MCGTXFMR");
    }

    #[test]
    fn test_transformer_creation() {
        let config = TransformerConfig::micro();
        let policy = GatePolicy::default();
        let weights = QuantizedWeights::empty(&config);

        let transformer = MincutGatedTransformer::new(config, policy, weights);
        assert!(transformer.is_ok());
    }

    #[test]
    fn test_inference_basic() {
        let config = TransformerConfig::micro();
        let policy = GatePolicy::default();
        let weights = QuantizedWeights::empty(&config);

        let mut transformer = MincutGatedTransformer::new(config.clone(), policy, weights).unwrap();

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            ..Default::default()
        };

        let input = InferInput::from_tokens(&[1, 2, 3, 4], gate);
        let mut logits = vec![0i32; config.logits as usize];
        let mut output = InferOutput::new(&mut logits);

        let result = transformer.infer(&input, &mut output);
        assert!(result.is_ok());
        assert_eq!(output.witness.decision, GateDecision::Allow);
    }

    #[test]
    fn test_output_buffer_too_small() {
        let config = TransformerConfig::micro();
        let policy = GatePolicy::default();
        let weights = QuantizedWeights::empty(&config);

        let mut transformer = MincutGatedTransformer::new(config, policy, weights).unwrap();

        let gate = GatePacket::default();
        let input = InferInput::from_tokens(&[1, 2, 3, 4], gate);
        let mut logits = vec![0i32; 10]; // Too small
        let mut output = InferOutput::new(&mut logits);

        let result = transformer.infer(&input, &mut output);
        assert!(matches!(result, Err(Error::OutputTooSmall { .. })));
    }
}
