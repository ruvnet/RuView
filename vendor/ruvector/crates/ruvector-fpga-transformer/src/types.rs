//! Core types for FPGA Transformer backend
//!
//! All types are designed for deterministic, allocation-free inference
//! with explicit quantization metadata.

use serde::{Deserialize, Serialize};

/// Unique identifier for a loaded model (SHA-256 hash)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(pub [u8; 32]);

impl ModelId {
    /// Create a new ModelId from bytes
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Create a zero ModelId (for testing)
    pub const fn zero() -> Self {
        Self([0u8; 32])
    }

    /// Get the bytes of the ModelId
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Option<Self> {
        if s.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let hex_str = std::str::from_utf8(chunk).ok()?;
            bytes[i] = u8::from_str_radix(hex_str, 16).ok()?;
        }
        Some(Self(bytes))
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Fixed shape specification for transformer inference
///
/// All dimensions are compile-time or model-time constants.
/// This enables zero-allocation inference paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FixedShape {
    /// Maximum sequence length
    pub seq_len: u16,
    /// Model/hidden dimension
    pub d_model: u16,
    /// Number of attention heads
    pub heads: u8,
    /// Dimension per head
    pub d_head: u16,
    /// Vocabulary size
    pub vocab: u32,
}

impl FixedShape {
    /// Create a new FixedShape
    pub const fn new(seq_len: u16, d_model: u16, heads: u8, d_head: u16, vocab: u32) -> Self {
        Self {
            seq_len,
            d_model,
            heads,
            d_head,
            vocab,
        }
    }

    /// Micro configuration for edge/WASM deployment
    pub const fn micro() -> Self {
        Self {
            seq_len: 32,
            d_model: 64,
            heads: 4,
            d_head: 16,
            vocab: 4096,
        }
    }

    /// Small configuration for embedded
    pub const fn small() -> Self {
        Self {
            seq_len: 64,
            d_model: 128,
            heads: 4,
            d_head: 32,
            vocab: 8192,
        }
    }

    /// Baseline configuration
    pub const fn baseline() -> Self {
        Self {
            seq_len: 128,
            d_model: 256,
            heads: 8,
            d_head: 32,
            vocab: 32000,
        }
    }

    /// Calculate total parameters for embedding layer
    pub const fn embedding_params(&self) -> usize {
        self.vocab as usize * self.d_model as usize
    }

    /// Calculate parameters per attention layer
    pub const fn attention_params(&self) -> usize {
        // Q, K, V projections + output projection
        4 * (self.d_model as usize * self.d_model as usize)
    }

    /// Calculate parameters per FFN layer (assuming 4x expansion)
    pub const fn ffn_params(&self) -> usize {
        2 * (self.d_model as usize * 4 * self.d_model as usize)
    }

    /// Validate shape consistency
    pub fn validate(&self) -> Result<(), String> {
        if self.d_model as usize != self.heads as usize * self.d_head as usize {
            return Err(format!(
                "d_model ({}) must equal heads ({}) * d_head ({})",
                self.d_model, self.heads, self.d_head
            ));
        }
        if self.seq_len == 0 {
            return Err("seq_len must be > 0".into());
        }
        if self.vocab == 0 {
            return Err("vocab must be > 0".into());
        }
        Ok(())
    }
}

impl Default for FixedShape {
    fn default() -> Self {
        Self::baseline()
    }
}

/// Quantization specification
///
/// Explicit quantization metadata ensuring reproducible inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantSpec {
    /// Weight bit width (1, 2, 4, 8)
    pub w_bits: u8,
    /// Activation bit width (4, 8, 16)
    pub a_bits: u8,
    /// Scale factor (Q16.16 fixed point)
    pub scale_q: i32,
    /// Zero point (Q16.16 fixed point)
    pub zero_q: i32,
    /// Memory layout
    pub layout: Layout,
}

impl QuantSpec {
    /// Create a new QuantSpec
    pub const fn new(w_bits: u8, a_bits: u8, scale_q: i32, zero_q: i32, layout: Layout) -> Self {
        Self {
            w_bits,
            a_bits,
            scale_q,
            zero_q,
            layout,
        }
    }

    /// INT4 weights, INT8 activations (common for edge)
    pub const fn int4_int8() -> Self {
        Self {
            w_bits: 4,
            a_bits: 8,
            scale_q: 1 << 16, // 1.0 in Q16.16
            zero_q: 0,
            layout: Layout::Blocked { block: 32 },
        }
    }

    /// INT8 weights and activations
    pub const fn int8() -> Self {
        Self {
            w_bits: 8,
            a_bits: 8,
            scale_q: 1 << 16,
            zero_q: 0,
            layout: Layout::RowMajor,
        }
    }

    /// Bytes per weight element
    pub const fn bytes_per_weight(&self) -> usize {
        match self.w_bits {
            1 => 1, // Packed 8 per byte, but minimum 1 byte
            2 => 1, // Packed 4 per byte
            4 => 1, // Packed 2 per byte
            8 => 1,
            16 => 2,
            _ => 4,
        }
    }

    /// Weights packed per byte
    pub const fn weights_per_byte(&self) -> usize {
        match self.w_bits {
            1 => 8,
            2 => 4,
            4 => 2,
            _ => 1,
        }
    }
}

impl Default for QuantSpec {
    fn default() -> Self {
        Self::int8()
    }
}

/// Memory layout for quantized tensors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    /// Standard row-major layout
    RowMajor,
    /// Blocked layout for SIMD/hardware efficiency
    Blocked { block: u16 },
    /// Heads interleaved for attention computation
    InterleavedHeads,
}

impl Default for Layout {
    fn default() -> Self {
        Self::RowMajor
    }
}

/// Hint for gating decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateHint {
    /// Coherence score (Q8.8 fixed point, higher = more coherent)
    pub coherence_score_q: i16,
    /// Whether a boundary was crossed in the input
    pub boundary_crossed: bool,
    /// Maximum compute class allowed
    pub max_compute_class: ComputeClass,
}

impl GateHint {
    /// Create a new GateHint
    pub const fn new(
        coherence_score_q: i16,
        boundary_crossed: bool,
        max_compute_class: ComputeClass,
    ) -> Self {
        Self {
            coherence_score_q,
            boundary_crossed,
            max_compute_class,
        }
    }

    /// Default hint allowing full computation
    pub const fn allow_all() -> Self {
        Self {
            coherence_score_q: i16::MAX,
            boundary_crossed: false,
            max_compute_class: ComputeClass::Deliberative,
        }
    }

    /// Reflex-only hint for fast path
    pub const fn reflex_only() -> Self {
        Self {
            coherence_score_q: 0,
            boundary_crossed: false,
            max_compute_class: ComputeClass::Reflex,
        }
    }
}

impl Default for GateHint {
    fn default() -> Self {
        Self::allow_all()
    }
}

/// Compute class for tiered inference
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ComputeClass {
    /// Fastest path, minimal computation (1-2 layers)
    Reflex = 0,
    /// Medium path, associative memory (4-6 layers)
    Associative = 1,
    /// Full deliberative computation (all layers)
    Deliberative = 2,
}

impl ComputeClass {
    /// Convert from u8
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Reflex),
            1 => Some(Self::Associative),
            2 => Some(Self::Deliberative),
            _ => None,
        }
    }
}

impl Default for ComputeClass {
    fn default() -> Self {
        Self::Deliberative
    }
}

/// Inference request
#[derive(Debug, Clone)]
pub struct InferenceRequest<'a> {
    /// Model to use
    pub model: ModelId,
    /// Expected shape
    pub shape: FixedShape,
    /// Input token IDs (length = seq_len)
    pub tokens: &'a [u16],
    /// Attention mask (length = seq_len or seq_len^2)
    pub attn_mask: &'a [u8],
    /// Gating hint for coherence control
    pub gate_hint: GateHint,
}

impl<'a> InferenceRequest<'a> {
    /// Create a new InferenceRequest
    pub fn new(
        model: ModelId,
        shape: FixedShape,
        tokens: &'a [u16],
        attn_mask: &'a [u8],
        gate_hint: GateHint,
    ) -> Self {
        Self {
            model,
            shape,
            tokens,
            attn_mask,
            gate_hint,
        }
    }

    /// Validate the request
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.tokens.len() != self.shape.seq_len as usize {
            return Err(crate::error::Error::InputLengthMismatch {
                expected: self.shape.seq_len as usize,
                actual: self.tokens.len(),
            });
        }
        if self.attn_mask.len() != self.shape.seq_len as usize
            && self.attn_mask.len() != (self.shape.seq_len as usize).pow(2)
        {
            return Err(crate::error::Error::InputLengthMismatch {
                expected: self.shape.seq_len as usize,
                actual: self.attn_mask.len(),
            });
        }
        Ok(())
    }
}

/// Inference result
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// Full logits (quantized, length = vocab) or empty if topk_only
    pub logits_q: Vec<i16>,
    /// Top-K predictions (token_id, logit_q)
    pub topk: Option<Vec<(u16, i16)>>,
    /// Witness log for audit trail
    pub witness: WitnessLog,
}

impl InferenceResult {
    /// Create a new InferenceResult
    pub fn new(logits_q: Vec<i16>, topk: Option<Vec<(u16, i16)>>, witness: WitnessLog) -> Self {
        Self {
            logits_q,
            topk,
            witness,
        }
    }

    /// Get the argmax token
    pub fn argmax(&self) -> Option<u16> {
        if let Some(ref topk) = self.topk {
            topk.first().map(|(token, _)| *token)
        } else if !self.logits_q.is_empty() {
            self.logits_q
                .iter()
                .enumerate()
                .max_by_key(|(_, &v)| v)
                .map(|(i, _)| i as u16)
        } else {
            None
        }
    }
}

/// Witness log for audit trail and ReasoningBank integration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessLog {
    /// Hash of the model used
    pub model_hash: [u8; 32],
    /// Hash of quantization parameters used
    pub quant_hash: [u8; 32],
    /// Backend that executed the inference
    pub backend: BackendKind,
    /// Compute cycles used (FPGA) or 0 (sim)
    pub cycles: u32,
    /// Latency in nanoseconds
    pub latency_ns: u32,
    /// Gate decision made
    pub gate_decision: GateDecision,
}

impl WitnessLog {
    /// Create a new WitnessLog
    pub fn new(
        model_hash: [u8; 32],
        quant_hash: [u8; 32],
        backend: BackendKind,
        cycles: u32,
        latency_ns: u32,
        gate_decision: GateDecision,
    ) -> Self {
        Self {
            model_hash,
            quant_hash,
            backend,
            cycles,
            latency_ns,
            gate_decision,
        }
    }

    /// Create an empty witness (for testing)
    pub fn empty() -> Self {
        Self {
            model_hash: [0u8; 32],
            quant_hash: [0u8; 32],
            backend: BackendKind::NativeSim,
            cycles: 0,
            latency_ns: 0,
            gate_decision: GateDecision::RanFull,
        }
    }
}

/// Backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendKind {
    /// PCIe-connected FPGA
    FpgaPcie,
    /// FPGA via local daemon
    FpgaDaemon,
    /// WASM simulator
    WasmSim,
    /// Native Rust simulator
    NativeSim,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FpgaPcie => write!(f, "fpga_pcie"),
            Self::FpgaDaemon => write!(f, "fpga_daemon"),
            Self::WasmSim => write!(f, "wasm_sim"),
            Self::NativeSim => write!(f, "native_sim"),
        }
    }
}

/// Gate decision outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateDecision {
    /// Full inference completed
    RanFull,
    /// Early exit at specified layer
    EarlyExit { layer: u8 },
    /// Inference was skipped
    Skipped { reason: SkipReason },
}

impl GateDecision {
    /// Check if inference actually ran
    pub const fn did_run(&self) -> bool {
        !matches!(self, Self::Skipped { .. })
    }

    /// Get the exit layer (full = max layers)
    pub const fn exit_layer(&self, max_layers: u8) -> u8 {
        match self {
            Self::RanFull => max_layers,
            Self::EarlyExit { layer } => *layer,
            Self::Skipped { .. } => 0,
        }
    }
}

/// Reason for skipping inference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// Coherence score too low
    LowCoherence,
    /// Policy denied the inference
    PolicyDenied,
    /// Compute budget exceeded
    BudgetExceeded,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LowCoherence => write!(f, "low_coherence"),
            Self::PolicyDenied => write!(f, "policy_denied"),
            Self::BudgetExceeded => write!(f, "budget_exceeded"),
        }
    }
}

/// Quantized tensor wrapper
#[derive(Debug, Clone)]
pub struct QuantizedTensor {
    /// Raw quantized data
    pub data: Vec<u8>,
    /// Quantization specification
    pub spec: QuantSpec,
    /// Tensor shape (row-major)
    pub shape: Vec<usize>,
}

impl QuantizedTensor {
    /// Create a new quantized tensor
    pub fn new(data: Vec<u8>, spec: QuantSpec, shape: Vec<usize>) -> Self {
        Self { data, spec, shape }
    }

    /// Total number of elements
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Expected data size in bytes
    pub fn expected_bytes(&self) -> usize {
        let numel = self.numel();
        (numel + self.spec.weights_per_byte() - 1) / self.spec.weights_per_byte()
    }

    /// Validate tensor integrity
    pub fn validate(&self) -> crate::error::Result<()> {
        let expected = self.expected_bytes();
        if self.data.len() != expected {
            return Err(crate::error::Error::QuantizationError(format!(
                "Data size mismatch: expected {} bytes, got {}",
                expected,
                self.data.len()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_id_hex_roundtrip() {
        let bytes = [0x12u8; 32];
        let id = ModelId::new(bytes);
        let hex = id.to_hex();
        let parsed = ModelId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_fixed_shape_validate() {
        let valid = FixedShape::new(64, 256, 8, 32, 32000);
        assert!(valid.validate().is_ok());

        let invalid = FixedShape::new(64, 256, 8, 16, 32000); // 8*16 != 256
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_quant_spec_bytes() {
        assert_eq!(QuantSpec::int8().weights_per_byte(), 1);
        assert_eq!(QuantSpec::int4_int8().weights_per_byte(), 2);
    }

    #[test]
    fn test_gate_decision() {
        assert!(GateDecision::RanFull.did_run());
        assert!(GateDecision::EarlyExit { layer: 3 }.did_run());
        assert!(!GateDecision::Skipped {
            reason: SkipReason::LowCoherence
        }
        .did_run());
    }
}
