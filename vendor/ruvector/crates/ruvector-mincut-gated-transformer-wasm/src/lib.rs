//! WASM bindings for Mincut-Gated Transformer.
//!
//! Provides JavaScript-friendly API for ultra-low-latency inference with
//! coherence control via dynamic minimum cut signals.
//!
//! ## Features
//!
//! - **Zero-copy inference**: Direct memory access from JavaScript
//! - **Deterministic bounds**: Predictable latency guarantees
//! - **Explainable decisions**: Every inference produces a witness
//! - **Coherence control**: Integration with mincut gate signals
//!
//! ## Example (JavaScript)
//!
//! ```javascript
//! import { WasmTransformer, WasmGatePacket } from './pkg';
//!
//! // Create transformer with micro config (optimized for WASM)
//! const transformer = new WasmTransformer();
//!
//! // Create gate packet from coherence signals
//! const gate = new WasmGatePacket();
//! gate.lambda = 100;
//! gate.lambda_prev = 95;
//! gate.boundary_edges = 5;
//! gate.boundary_concentration_q15 = 8192;
//! gate.partition_count = 3;
//!
//! // Run inference
//! const tokens = new Uint32Array([1, 2, 3, 4]);
//! const result = transformer.infer(tokens, gate);
//!
//! console.log('Decision:', result.decision);
//! console.log('Reason:', result.reason);
//! console.log('Logits:', result.logits);
//! ```

use ruvector_mincut_gated_transformer::{
    GateDecision, GatePacket, GatePolicy, GateReason, InferInput, InferOutput,
    MincutGatedTransformer, QuantizedWeights, SpikePacket, TransformerConfig,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// JavaScript-friendly transformer wrapper.
///
/// This wraps the core `MincutGatedTransformer` and provides a JavaScript-friendly API.
#[wasm_bindgen]
pub struct WasmTransformer {
    inner: MincutGatedTransformer,
    logits_buffer: Vec<i32>,
}

#[wasm_bindgen]
impl WasmTransformer {
    /// Create with micro config (optimized for WASM).
    ///
    /// Micro config:
    /// - Sequence length: 32
    /// - Hidden size: 128
    /// - Heads: 4
    /// - Layers: 2
    /// - Logits: 256
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmTransformer, JsValue> {
        let config = TransformerConfig::micro();
        let policy = GatePolicy::default();
        let weights = QuantizedWeights::empty(&config);

        let inner = MincutGatedTransformer::new(config.clone(), policy, weights)
            .map_err(|e| JsValue::from_str(&format!("Failed to create transformer: {}", e)))?;

        let logits_buffer = vec![0i32; config.logits as usize];

        Ok(WasmTransformer {
            inner,
            logits_buffer,
        })
    }

    /// Create with baseline config (larger model).
    ///
    /// Baseline config:
    /// - Sequence length: 64
    /// - Hidden size: 256
    /// - Heads: 4
    /// - Layers: 4
    /// - Logits: 1024
    #[wasm_bindgen]
    pub fn new_baseline() -> Result<WasmTransformer, JsValue> {
        let config = TransformerConfig::baseline();
        let policy = GatePolicy::default();
        let weights = QuantizedWeights::empty(&config);

        let inner = MincutGatedTransformer::new(config.clone(), policy, weights)
            .map_err(|e| JsValue::from_str(&format!("Failed to create transformer: {}", e)))?;

        let logits_buffer = vec![0i32; config.logits as usize];

        Ok(WasmTransformer {
            inner,
            logits_buffer,
        })
    }

    /// Create with custom config from JavaScript object.
    ///
    /// Example:
    /// ```javascript
    /// const config = {
    ///   seq_len_max: 32,
    ///   hidden: 128,
    ///   heads: 4,
    ///   layers: 2,
    ///   window_normal: 8,
    ///   window_degraded: 4,
    ///   ffn_mult: 4,
    ///   logits: 256
    /// };
    /// const transformer = WasmTransformer.with_config(config);
    /// ```
    #[wasm_bindgen]
    pub fn with_config(config_js: JsValue) -> Result<WasmTransformer, JsValue> {
        let config: TransformerConfig = serde_wasm_bindgen::from_value(config_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid config: {}", e)))?;

        let policy = GatePolicy::default();
        let weights = QuantizedWeights::empty(&config);

        let inner = MincutGatedTransformer::new(config.clone(), policy, weights)
            .map_err(|e| JsValue::from_str(&format!("Failed to create transformer: {}", e)))?;

        let logits_buffer = vec![0i32; config.logits as usize];

        Ok(WasmTransformer {
            inner,
            logits_buffer,
        })
    }

    /// Run inference with gate packet.
    ///
    /// Returns a `WasmInferResult` containing logits, decision, and witness information.
    #[wasm_bindgen]
    pub fn infer(&mut self, tokens: &[u32], gate_js: JsValue) -> Result<WasmInferResult, JsValue> {
        let gate: WasmGatePacket = serde_wasm_bindgen::from_value(gate_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid gate packet: {}", e)))?;

        let gate_packet = gate.to_native();
        let input = InferInput::from_tokens(tokens, gate_packet);

        let mut output = InferOutput::new(&mut self.logits_buffer);

        self.inner
            .infer(&input, &mut output)
            .map_err(|e| JsValue::from_str(&format!("Inference failed: {}", e)))?;

        Ok(WasmInferResult::from_output(&output))
    }

    /// Run inference with gate and spike packets.
    ///
    /// This enables event-driven scheduling with spike signals.
    #[wasm_bindgen]
    pub fn infer_with_spikes(
        &mut self,
        tokens: &[u32],
        gate_js: JsValue,
        spikes_js: JsValue,
    ) -> Result<WasmInferResult, JsValue> {
        let gate: WasmGatePacket = serde_wasm_bindgen::from_value(gate_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid gate packet: {}", e)))?;

        let spikes: WasmSpikePacket = serde_wasm_bindgen::from_value(spikes_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid spike packet: {}", e)))?;

        let gate_packet = gate.to_native();
        let spike_packet = spikes.to_native();

        let input = InferInput::from_tokens(tokens, gate_packet).with_spikes(spike_packet);

        let mut output = InferOutput::new(&mut self.logits_buffer);

        self.inner
            .infer(&input, &mut output)
            .map_err(|e| JsValue::from_str(&format!("Inference failed: {}", e)))?;

        Ok(WasmInferResult::from_output(&output))
    }

    /// Reset all state (KV cache, cached logits, etc.).
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get the logits buffer size.
    #[wasm_bindgen]
    pub fn buffer_size(&self) -> usize {
        self.logits_buffer.len()
    }

    /// Update gate policy from JavaScript object.
    #[wasm_bindgen]
    pub fn set_policy(&mut self, policy_js: JsValue) -> Result<(), JsValue> {
        let policy: GatePolicy = serde_wasm_bindgen::from_value(policy_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid policy: {}", e)))?;

        self.inner.set_policy(policy);
        Ok(())
    }
}

/// JavaScript-friendly gate packet.
///
/// This carries coherence control signals from the mincut engine.
#[wasm_bindgen]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WasmGatePacket {
    /// Current lambda (minimum cut value / coherence metric)
    pub lambda: u32,

    /// Previous lambda for trend detection
    pub lambda_prev: u32,

    /// Number of edges crossing partition boundaries
    pub boundary_edges: u16,

    /// Boundary edge concentration (Q15: 0-32767)
    pub boundary_concentration_q15: u16,

    /// Number of partitions in current graph state
    pub partition_count: u16,

    /// Policy flags (force safe mode, etc.)
    pub flags: u16,
}

#[wasm_bindgen]
impl WasmGatePacket {
    /// Create a new gate packet with default values.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmGatePacket {
        WasmGatePacket {
            lambda: 100,
            lambda_prev: 100,
            boundary_edges: 0,
            boundary_concentration_q15: 0,
            partition_count: 1,
            flags: 0,
        }
    }

    /// Create from JavaScript object.
    #[wasm_bindgen]
    pub fn from_js(js: JsValue) -> Result<WasmGatePacket, JsValue> {
        serde_wasm_bindgen::from_value(js)
            .map_err(|e| JsValue::from_str(&format!("Invalid gate packet: {}", e)))
    }
}

impl WasmGatePacket {
    fn to_native(&self) -> GatePacket {
        GatePacket {
            lambda: self.lambda,
            lambda_prev: self.lambda_prev,
            boundary_edges: self.boundary_edges,
            boundary_concentration_q15: self.boundary_concentration_q15,
            partition_count: self.partition_count,
            flags: self.flags,
        }
    }
}

impl Default for WasmGatePacket {
    fn default() -> Self {
        Self::new()
    }
}

/// JavaScript-friendly spike packet.
///
/// Used for event-driven scheduling to determine whether to run inference.
#[wasm_bindgen]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WasmSpikePacket {
    /// Spike fired indicator (0 = skip or cheap path)
    pub fired: u8,

    /// Spike rate (Q15: 0-32767)
    pub rate_q15: u16,

    /// Novelty metric (Q15: 0-32767)
    pub novelty_q15: u16,

    /// Flags
    pub flags: u16,
}

#[wasm_bindgen]
impl WasmSpikePacket {
    /// Create a new spike packet with default values.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmSpikePacket {
        WasmSpikePacket {
            fired: 1,
            rate_q15: 0,
            novelty_q15: 0,
            flags: 0,
        }
    }
}

impl WasmSpikePacket {
    fn to_native(&self) -> SpikePacket {
        SpikePacket {
            fired: self.fired,
            rate_q15: self.rate_q15,
            novelty_q15: self.novelty_q15,
            top_len: 0,
            top_idx: [0; 16],
            top_w_q15: [0; 16],
            flags: self.flags,
        }
    }
}

impl Default for WasmSpikePacket {
    fn default() -> Self {
        Self::new()
    }
}

/// JavaScript-friendly inference result.
///
/// Contains output logits and witness information about the inference decision.
#[wasm_bindgen]
pub struct WasmInferResult {
    logits: Vec<i32>,
    decision: String,
    reason: String,
    tier: u8,
    kv_writes_enabled: bool,
    external_writes_enabled: bool,
    effective_seq_len: u16,
    effective_window: u16,
    lambda: u32,
    lambda_prev: u32,
    boundary_edges: u16,
    partition_count: u16,
}

#[wasm_bindgen]
impl WasmInferResult {
    /// Get output logits as Int32Array.
    #[wasm_bindgen(getter)]
    pub fn logits(&self) -> Vec<i32> {
        self.logits.clone()
    }

    /// Get gate decision as string.
    #[wasm_bindgen(getter)]
    pub fn decision(&self) -> String {
        self.decision.clone()
    }

    /// Get decision reason as string.
    #[wasm_bindgen(getter)]
    pub fn reason(&self) -> String {
        self.reason.clone()
    }

    /// Get compute tier (0-3).
    #[wasm_bindgen(getter)]
    pub fn tier(&self) -> u8 {
        self.tier
    }

    /// Check if KV writes were enabled.
    #[wasm_bindgen(getter)]
    pub fn kv_writes_enabled(&self) -> bool {
        self.kv_writes_enabled
    }

    /// Check if external writes are enabled.
    #[wasm_bindgen(getter)]
    pub fn external_writes_enabled(&self) -> bool {
        self.external_writes_enabled
    }

    /// Get effective sequence length used.
    #[wasm_bindgen(getter)]
    pub fn effective_seq_len(&self) -> u16 {
        self.effective_seq_len
    }

    /// Get effective window size used.
    #[wasm_bindgen(getter)]
    pub fn effective_window(&self) -> u16 {
        self.effective_window
    }

    /// Get current lambda value.
    #[wasm_bindgen(getter)]
    pub fn lambda(&self) -> u32 {
        self.lambda
    }

    /// Get previous lambda value.
    #[wasm_bindgen(getter)]
    pub fn lambda_prev(&self) -> u32 {
        self.lambda_prev
    }

    /// Get boundary edges count.
    #[wasm_bindgen(getter)]
    pub fn boundary_edges(&self) -> u16 {
        self.boundary_edges
    }

    /// Get partition count.
    #[wasm_bindgen(getter)]
    pub fn partition_count(&self) -> u16 {
        self.partition_count
    }
}

impl WasmInferResult {
    fn from_output(output: &InferOutput) -> Self {
        let decision = match output.witness.decision {
            GateDecision::Allow => "Allow",
            GateDecision::ReduceScope => "ReduceScope",
            GateDecision::FlushKv => "FlushKv",
            GateDecision::FreezeWrites => "FreezeWrites",
            GateDecision::QuarantineUpdates => "QuarantineUpdates",
        };

        let reason = match output.witness.reason {
            GateReason::None => "None",
            GateReason::LambdaBelowMin => "LambdaBelowMin",
            GateReason::LambdaDroppedFast => "LambdaDroppedFast",
            GateReason::BoundarySpike => "BoundarySpike",
            GateReason::BoundaryConcentrationSpike => "BoundaryConcentrationSpike",
            GateReason::PartitionDrift => "PartitionDrift",
            GateReason::SpikeStorm => "SpikeStorm",
            GateReason::ForcedByFlag => "ForcedByFlag",
        };

        WasmInferResult {
            logits: output.logits_i32.to_vec(),
            decision: decision.to_string(),
            reason: reason.to_string(),
            tier: output.stats.tier,
            kv_writes_enabled: output.witness.kv_writes_enabled != 0,
            external_writes_enabled: output.witness.external_writes_enabled != 0,
            effective_seq_len: output.witness.effective_seq_len,
            effective_window: output.witness.effective_window,
            lambda: output.witness.lambda,
            lambda_prev: output.witness.lambda_prev,
            boundary_edges: output.witness.boundary_edges,
            partition_count: output.witness.partition_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_transformer_creation() {
        let transformer = WasmTransformer::new();
        assert!(transformer.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_gate_packet() {
        let gate = WasmGatePacket::new();
        assert_eq!(gate.lambda, 100);
        assert_eq!(gate.lambda_prev, 100);
    }
}
