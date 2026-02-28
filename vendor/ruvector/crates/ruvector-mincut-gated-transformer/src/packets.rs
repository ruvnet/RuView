//! Packet types for gate and spike signaling.
//!
//! These types define the coherence control interface between the mincut
//! engine, spike scheduler, and transformer kernel.

use serde::{Deserialize, Serialize};

/// Gate packet from the mincut coherence controller.
///
/// This is the only required coherence input. It carries lambda (coherence metric)
/// and boundary statistics from the dynamic minimum cut computation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatePacket {
    /// Current lambda (minimum cut value / coherence metric)
    pub lambda: u32,

    /// Previous lambda for trend detection
    pub lambda_prev: u32,

    /// Number of edges crossing partition boundaries
    pub boundary_edges: u16,

    /// Boundary edge concentration (Q15: 0-32767)
    /// Higher means edges are concentrated into fewer boundaries
    pub boundary_concentration_q15: u16,

    /// Number of partitions in current graph state
    pub partition_count: u16,

    /// Policy flags (force safe mode, etc.)
    pub flags: u16,
}

impl GatePacket {
    /// Flag: Force safe mode regardless of metrics
    pub const FLAG_FORCE_SAFE: u16 = 1 << 0;

    /// Flag: Skip inference entirely
    pub const FLAG_SKIP: u16 = 1 << 1;

    /// Flag: Boundary edge IDs available in side channel
    pub const FLAG_BOUNDARY_IDS_AVAILABLE: u16 = 1 << 2;

    /// Check if force safe mode is set
    #[inline]
    pub fn force_safe(&self) -> bool {
        self.flags & Self::FLAG_FORCE_SAFE != 0
    }

    /// Check if skip is requested
    #[inline]
    pub fn skip_requested(&self) -> bool {
        self.flags & Self::FLAG_SKIP != 0
    }

    /// Calculate lambda delta
    #[inline]
    pub fn lambda_delta(&self) -> i32 {
        (self.lambda as i32) - (self.lambda_prev as i32)
    }

    /// Calculate drop ratio in Q15 fixed point
    #[inline]
    pub fn drop_ratio_q15(&self) -> u16 {
        if self.lambda_prev == 0 || self.lambda >= self.lambda_prev {
            return 0;
        }

        let drop = self.lambda_prev - self.lambda;
        // (drop / lambda_prev) * 32768
        ((drop as u64 * 32768) / (self.lambda_prev as u64)) as u16
    }
}

/// Spike packet for event-driven scheduling.
///
/// Used by the optional spike scheduler to determine whether to run inference
/// and at what compute tier.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpikePacket {
    /// Spike fired indicator (0 = skip or cheap path)
    pub fired: u8,

    /// Spike rate (Q15: 0-32767)
    pub rate_q15: u16,

    /// Novelty metric (Q15: 0-32767)
    pub novelty_q15: u16,

    /// Number of valid entries in top_idx/top_w
    pub top_len: u8,

    /// Top-k indices for sparse attention/context
    pub top_idx: [u16; 16],

    /// Top-k weights (Q15)
    pub top_w_q15: [u16; 16],

    /// Flags
    pub flags: u16,
}

impl SpikePacket {
    /// Flag: Use top-k as sparse attention mask
    pub const FLAG_SPARSE_MASK: u16 = 1 << 0;

    /// Flag: Use top-k as sparse context builder
    pub const FLAG_SPARSE_CONTEXT: u16 = 1 << 1;

    /// Check if spike indicates activity
    #[inline]
    pub fn is_active(&self) -> bool {
        self.fired != 0
    }

    /// Check if sparse mask mode is enabled
    #[inline]
    pub fn use_sparse_mask(&self) -> bool {
        self.flags & Self::FLAG_SPARSE_MASK != 0
    }

    /// Get top indices slice
    #[inline]
    pub fn top_indices(&self) -> &[u16] {
        &self.top_idx[..(self.top_len as usize).min(16)]
    }

    /// Get top weights slice
    #[inline]
    pub fn top_weights(&self) -> &[u16] {
        &self.top_w_q15[..(self.top_len as usize).min(16)]
    }
}

/// Gate decision output.
///
/// Determines what the transformer kernel is allowed to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GateDecision {
    /// Proceed normally
    #[default]
    Allow = 0,

    /// Reduce sequence length and window size
    ReduceScope = 1,

    /// Flush KV cache before proceeding
    FlushKv = 2,

    /// Freeze KV writes (read-only mode)
    FreezeWrites = 3,

    /// Run compute but discard all state changes
    QuarantineUpdates = 4,
}

impl GateDecision {
    /// Check if this decision allows KV cache writes
    #[inline]
    pub fn allows_kv_writes(&self) -> bool {
        matches!(self, GateDecision::Allow | GateDecision::ReduceScope)
    }

    /// Check if this decision allows external writes
    #[inline]
    pub fn allows_external_writes(&self) -> bool {
        matches!(self, GateDecision::Allow)
    }

    /// Check if this is an intervention (not Allow)
    #[inline]
    pub fn is_intervention(&self) -> bool {
        !matches!(self, GateDecision::Allow)
    }
}

/// Reason for a gate decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GateReason {
    /// No intervention needed
    #[default]
    None = 0,

    /// Lambda below minimum threshold
    LambdaBelowMin = 1,

    /// Lambda dropped too fast
    LambdaDroppedFast = 2,

    /// Boundary edge count exceeded threshold
    BoundarySpike = 3,

    /// Boundary concentration exceeded threshold
    BoundaryConcentrationSpike = 4,

    /// Partition count indicates drift
    PartitionDrift = 5,

    /// Spike rate indicates overload
    SpikeStorm = 6,

    /// Forced by flag in GatePacket
    ForcedByFlag = 7,
}

/// Witness record for a gate decision.
///
/// Every inference call produces a witness. Minimal for Allow decisions,
/// richer for interventions.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Witness {
    /// The gate decision made
    pub decision: GateDecision,

    /// Reason for the decision
    pub reason: GateReason,

    /// Current lambda value
    pub lambda: u32,

    /// Previous lambda value
    pub lambda_prev: u32,

    /// Lambda delta (signed)
    pub lambda_delta: i32,

    /// Effective sequence length used
    pub effective_seq_len: u16,

    /// Effective window size used
    pub effective_window: u16,

    /// Whether KV writes were enabled (0 or 1)
    pub kv_writes_enabled: u8,

    /// Whether external writes are enabled (0 or 1)
    pub external_writes_enabled: u8,

    /// Boundary edges from gate packet
    pub boundary_edges: u16,

    /// Boundary concentration from gate packet
    pub boundary_concentration_q15: u16,

    /// Partition count from gate packet
    pub partition_count: u16,

    /// Top boundary edge IDs (optional, from side channel)
    pub top_boundary_edge_ids: [u32; 8],
}

impl Witness {
    /// Create a witness for an Allow decision
    pub fn allow(gate: &GatePacket, seq_len: u16, window: u16) -> Self {
        Self {
            decision: GateDecision::Allow,
            reason: GateReason::None,
            lambda: gate.lambda,
            lambda_prev: gate.lambda_prev,
            lambda_delta: gate.lambda_delta(),
            effective_seq_len: seq_len,
            effective_window: window,
            kv_writes_enabled: 1,
            external_writes_enabled: 1,
            boundary_edges: gate.boundary_edges,
            boundary_concentration_q15: gate.boundary_concentration_q15,
            partition_count: gate.partition_count,
            top_boundary_edge_ids: [0; 8],
        }
    }

    /// Create a witness for an intervention
    pub fn intervention(
        decision: GateDecision,
        reason: GateReason,
        gate: &GatePacket,
        seq_len: u16,
        window: u16,
    ) -> Self {
        Self {
            decision,
            reason,
            lambda: gate.lambda,
            lambda_prev: gate.lambda_prev,
            lambda_delta: gate.lambda_delta(),
            effective_seq_len: seq_len,
            effective_window: window,
            kv_writes_enabled: if decision.allows_kv_writes() { 1 } else { 0 },
            external_writes_enabled: if decision.allows_external_writes() {
                1
            } else {
                0
            },
            boundary_edges: gate.boundary_edges,
            boundary_concentration_q15: gate.boundary_concentration_q15,
            partition_count: gate.partition_count,
            top_boundary_edge_ids: [0; 8],
        }
    }
}

/// Inference input structure.
pub struct InferInput<'a> {
    /// Token IDs (optional, use either tokens or embedding)
    pub tokens: Option<&'a [u32]>,

    /// Quantized embedding input (int8)
    pub embedding_q: Option<&'a [i8]>,

    /// Scale factor for quantized embedding
    pub embedding_scale: f32,

    /// Optional input signature for cache hits
    pub input_signature: Option<u64>,

    /// Gate packet from mincut controller
    pub gate: GatePacket,

    /// Optional spike packet from scheduler
    pub spikes: Option<SpikePacket>,
}

impl<'a> InferInput<'a> {
    /// Create input from tokens
    pub fn from_tokens(tokens: &'a [u32], gate: GatePacket) -> Self {
        Self {
            tokens: Some(tokens),
            embedding_q: None,
            embedding_scale: 1.0,
            input_signature: None,
            gate,
            spikes: None,
        }
    }

    /// Create input from quantized embeddings
    pub fn from_embedding(embedding_q: &'a [i8], scale: f32, gate: GatePacket) -> Self {
        Self {
            tokens: None,
            embedding_q: Some(embedding_q),
            embedding_scale: scale,
            input_signature: None,
            gate,
            spikes: None,
        }
    }

    /// Set input signature for caching
    pub fn with_signature(mut self, sig: u64) -> Self {
        self.input_signature = Some(sig);
        self
    }

    /// Set spike packet
    pub fn with_spikes(mut self, spikes: SpikePacket) -> Self {
        self.spikes = Some(spikes);
        self
    }
}

/// Inference output structure.
pub struct InferOutput<'a> {
    /// Output logits buffer (i32 accumulator domain)
    pub logits_i32: &'a mut [i32],

    /// Witness for this inference call
    pub witness: Witness,

    /// Statistics for this inference call
    pub stats: InferStats,
}

impl<'a> InferOutput<'a> {
    /// Create output with buffer
    pub fn new(logits_i32: &'a mut [i32]) -> Self {
        Self {
            logits_i32,
            witness: Witness::default(),
            stats: InferStats::default(),
        }
    }
}

/// Inference statistics.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct InferStats {
    /// Effective sequence length used
    pub effective_seq_len: u16,

    /// Effective window size used
    pub effective_window: u16,

    /// Number of layers executed
    pub layers_executed: u16,

    /// Compute tier used (0-3)
    pub tier: u8,

    /// Number of quantized GEMM calls
    pub qgemm_calls: u32,

    /// Attention dot product operations
    pub attn_dot_ops: u64,

    /// FFN operations
    pub ffn_ops: u64,

    /// KV cache bytes touched
    pub kv_bytes_touched: u64,

    /// Whether inference was skipped
    pub skipped: u8,

    /// Number of tokens skipped via MoD routing
    pub tokens_skipped: u32,

    /// Layer at which early exit occurred (0 = no early exit)
    pub early_exit_layer: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_packet_delta() {
        let gate = GatePacket {
            lambda: 80,
            lambda_prev: 100,
            ..Default::default()
        };
        assert_eq!(gate.lambda_delta(), -20);
        assert!(gate.drop_ratio_q15() > 0);
    }

    #[test]
    fn test_gate_packet_flags() {
        let mut gate = GatePacket::default();
        gate.flags = GatePacket::FLAG_FORCE_SAFE;
        assert!(gate.force_safe());
        assert!(!gate.skip_requested());
    }

    #[test]
    fn test_gate_decision_permissions() {
        assert!(GateDecision::Allow.allows_kv_writes());
        assert!(GateDecision::Allow.allows_external_writes());

        assert!(GateDecision::ReduceScope.allows_kv_writes());
        assert!(!GateDecision::ReduceScope.allows_external_writes());

        assert!(!GateDecision::FreezeWrites.allows_kv_writes());
        assert!(!GateDecision::QuarantineUpdates.allows_external_writes());
    }

    #[test]
    fn test_witness_creation() {
        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let witness = Witness::allow(&gate, 64, 16);
        assert_eq!(witness.decision, GateDecision::Allow);
        assert_eq!(witness.lambda, 100);
        assert_eq!(witness.kv_writes_enabled, 1);
        assert_eq!(witness.external_writes_enabled, 1);
    }

    #[test]
    fn test_spike_packet() {
        let mut spike = SpikePacket::default();
        spike.fired = 1;
        spike.top_len = 3;
        spike.top_idx[0] = 10;
        spike.top_idx[1] = 20;
        spike.top_idx[2] = 30;

        assert!(spike.is_active());
        assert_eq!(spike.top_indices().len(), 3);
    }
}
