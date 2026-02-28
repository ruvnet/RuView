//! Backend implementations for FPGA Transformer
//!
//! All backends implement the `TransformerBackend` trait for uniform API.

use crate::artifact::ModelArtifact;
use crate::error::Result;
use crate::types::{InferenceRequest, InferenceResult, ModelId};

#[cfg(feature = "native_sim")]
pub mod native_sim;

#[cfg(feature = "daemon")]
pub mod fpga_daemon;

#[cfg(feature = "pcie")]
pub mod fpga_pcie;

#[cfg(feature = "wasm")]
pub mod wasm_sim;

/// Trait for transformer inference backends
///
/// All backends must be thread-safe and implement the same API.
pub trait TransformerBackend: Send + Sync {
    /// Load a model artifact and return its ID
    ///
    /// The artifact is validated, test vectors are run, and
    /// the model is prepared for inference.
    fn load(&self, artifact: &ModelArtifact) -> Result<ModelId>;

    /// Run inference on the given request
    ///
    /// The request must specify a model that has been loaded.
    /// Returns the inference result with witness log.
    fn infer(&self, req: InferenceRequest) -> Result<InferenceResult>;

    /// Unload a model to free resources
    fn unload(&self, model: ModelId) -> Result<()>;

    /// Check if a model is loaded
    fn is_loaded(&self, model: ModelId) -> bool;

    /// Get the backend kind
    fn kind(&self) -> crate::types::BackendKind;

    /// Get backend-specific statistics
    fn stats(&self) -> BackendStats {
        BackendStats::default()
    }
}

/// Backend statistics
#[derive(Debug, Clone, Default)]
pub struct BackendStats {
    /// Number of models currently loaded
    pub models_loaded: usize,
    /// Total inferences performed
    pub total_inferences: u64,
    /// Total cycles consumed (FPGA only)
    pub total_cycles: u64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: u64,
    /// P99 latency in nanoseconds
    pub p99_latency_ns: u64,
    /// Number of early exits
    pub early_exits: u64,
    /// Number of skipped inferences
    pub skipped: u64,
}

/// Protocol constants for daemon/PCIe communication
pub mod protocol {
    /// Magic number for frame validation
    pub const MAGIC: u32 = 0x5256_5846; // "RVXF" - RuVector FPGA

    /// Current protocol version
    pub const VERSION: u16 = 1;

    /// Frame header size in bytes
    pub const HEADER_SIZE: usize = 24;

    /// Maximum payload size
    pub const MAX_PAYLOAD: usize = 1024 * 1024; // 1MB

    /// Request flags
    pub mod flags {
        /// Return only top-K predictions
        pub const TOPK_ONLY: u16 = 0x0001;
        /// Use LUT-based softmax
        pub const LUT_SOFTMAX: u16 = 0x0002;
        /// Enable early exit
        pub const EARLY_EXIT: u16 = 0x0004;
        /// Return detailed witness
        pub const WITNESS_DETAIL: u16 = 0x0008;
    }

    /// Response status codes
    pub mod status {
        /// Success
        pub const OK: u16 = 0;
        /// Model not found
        pub const MODEL_NOT_FOUND: u16 = 1;
        /// Shape mismatch
        pub const SHAPE_MISMATCH: u16 = 2;
        /// Gate blocked
        pub const GATE_BLOCKED: u16 = 3;
        /// Internal error
        pub const INTERNAL_ERROR: u16 = 0xFFFF;
    }
}

/// Request frame for wire protocol
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RequestFrame {
    /// Magic number (MAGIC)
    pub magic: u32,
    /// Protocol version
    pub protocol: u16,
    /// Sequence length
    pub seq_len: u16,
    /// Model dimension
    pub d_model: u16,
    /// Vocabulary size
    pub vocab: u16,
    /// Model ID (lower 32 bits)
    pub model_id_low: u32,
    /// Model ID (upper 32 bits)
    pub model_id_high: u32,
    /// Request flags
    pub flags: u16,
    /// Top-K count (if TOPK_ONLY flag set)
    pub topk: u16,
}

impl RequestFrame {
    /// Create a new request frame
    pub fn new(
        seq_len: u16,
        d_model: u16,
        vocab: u32,
        model_id: &ModelId,
        flags: u16,
        topk: u16,
    ) -> Self {
        let id_bytes = model_id.as_bytes();
        let model_id_low = u32::from_le_bytes([id_bytes[0], id_bytes[1], id_bytes[2], id_bytes[3]]);
        let model_id_high =
            u32::from_le_bytes([id_bytes[4], id_bytes[5], id_bytes[6], id_bytes[7]]);

        Self {
            magic: protocol::MAGIC,
            protocol: protocol::VERSION,
            seq_len,
            d_model,
            vocab: (vocab & 0xFFFF) as u16,
            model_id_low,
            model_id_high,
            flags,
            topk,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> [u8; protocol::HEADER_SIZE] {
        let mut bytes = [0u8; protocol::HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.protocol.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.seq_len.to_le_bytes());
        bytes[8..10].copy_from_slice(&self.d_model.to_le_bytes());
        bytes[10..12].copy_from_slice(&self.vocab.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.model_id_low.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.model_id_high.to_le_bytes());
        bytes[20..22].copy_from_slice(&self.flags.to_le_bytes());
        bytes[22..24].copy_from_slice(&self.topk.to_le_bytes());
        bytes
    }
}

/// Response frame from wire protocol
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ResponseFrame {
    /// Status code
    pub status: u16,
    /// Latency in nanoseconds
    pub latency_ns: u32,
    /// Compute cycles
    pub cycles: u32,
    /// Gate decision (packed)
    pub gate_decision: u8,
    /// Exit layer (if early exit)
    pub exit_layer: u8,
    /// Skip reason (if skipped)
    pub skip_reason: u8,
    /// Reserved
    pub reserved: u8,
}

impl ResponseFrame {
    /// Parse from bytes
    pub fn from_bytes(bytes: &[u8; 14]) -> Self {
        Self {
            status: u16::from_le_bytes([bytes[0], bytes[1]]),
            latency_ns: u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            cycles: u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
            gate_decision: bytes[10],
            exit_layer: bytes[11],
            skip_reason: bytes[12],
            reserved: bytes[13],
        }
    }

    /// Convert gate decision to enum
    pub fn to_gate_decision(&self) -> crate::types::GateDecision {
        match self.gate_decision {
            0 => crate::types::GateDecision::RanFull,
            1 => crate::types::GateDecision::EarlyExit {
                layer: self.exit_layer,
            },
            2 => crate::types::GateDecision::Skipped {
                reason: match self.skip_reason {
                    0 => crate::types::SkipReason::LowCoherence,
                    1 => crate::types::SkipReason::PolicyDenied,
                    _ => crate::types::SkipReason::BudgetExceeded,
                },
            },
            _ => crate::types::GateDecision::RanFull,
        }
    }
}

/// Calculate CRC32 checksum for frame validation
pub fn crc32(data: &[u8]) -> u32 {
    // Simple CRC32 implementation (could use crc32fast crate in production)
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB88320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

// ============================================================================
// Common utilities shared across backends
// ============================================================================

/// Compute top-K predictions from logits
/// Returns sorted (token_id, logit) pairs, descending by logit value
#[inline]
pub fn compute_topk(logits: &[i16], k: usize) -> Vec<(u16, i16)> {
    if logits.is_empty() {
        return vec![];
    }

    // For small K, partial sort is faster
    if k <= 32 && logits.len() > 100 {
        // Use partial heap-based selection
        let mut heap: Vec<(i16, u16)> = Vec::with_capacity(k + 1);
        for (i, &v) in logits.iter().enumerate() {
            if heap.len() < k {
                heap.push((v, i as u16));
                if heap.len() == k {
                    // Heapify
                    heap.sort_by(|a, b| a.0.cmp(&b.0));
                }
            } else if v > heap[0].0 {
                heap[0] = (v, i as u16);
                // Maintain min-heap property
                let mut idx = 0;
                while idx * 2 + 1 < heap.len() {
                    let left = idx * 2 + 1;
                    let right = idx * 2 + 2;
                    let mut smallest = idx;
                    if heap[left].0 < heap[smallest].0 {
                        smallest = left;
                    }
                    if right < heap.len() && heap[right].0 < heap[smallest].0 {
                        smallest = right;
                    }
                    if smallest == idx {
                        break;
                    }
                    heap.swap(idx, smallest);
                    idx = smallest;
                }
            }
        }
        heap.sort_by(|a, b| b.0.cmp(&a.0));
        heap.into_iter().map(|(v, i)| (i, v)).collect()
    } else {
        // Full sort for small arrays
        let mut indexed: Vec<(usize, i16)> = logits.iter().cloned().enumerate().collect();
        indexed.sort_by(|a, b| b.1.cmp(&a.1));
        indexed
            .into_iter()
            .take(k)
            .map(|(i, v)| (i as u16, v))
            .collect()
    }
}

/// Helper to safely read from RwLock, returning error on poison
pub fn read_lock<T, R>(
    lock: &std::sync::RwLock<T>,
    f: impl FnOnce(&T) -> R,
) -> crate::error::Result<R> {
    lock.read()
        .map(|guard| f(&*guard))
        .map_err(|_| crate::error::Error::BackendError("Lock poisoned (read)".into()))
}

/// Helper to safely write to RwLock, returning error on poison
pub fn write_lock<T, R>(
    lock: &std::sync::RwLock<T>,
    f: impl FnOnce(&mut T) -> R,
) -> crate::error::Result<R> {
    lock.write()
        .map(|mut guard| f(&mut *guard))
        .map_err(|_| crate::error::Error::BackendError("Lock poisoned (write)".into()))
}

/// Validate token indices against vocabulary size
#[inline]
pub fn validate_tokens(tokens: &[u16], vocab_size: u32) -> crate::error::Result<()> {
    for (i, &token) in tokens.iter().enumerate() {
        if token as u32 >= vocab_size {
            return Err(crate::error::Error::InvalidConfig(format!(
                "Token {} at index {} exceeds vocabulary size {}",
                token, i, vocab_size
            )));
        }
    }
    Ok(())
}

/// Build witness log from inference metadata
pub fn build_witness(
    model_hash: [u8; 32],
    quant_hash: [u8; 32],
    backend: crate::types::BackendKind,
    cycles: u32,
    latency_ns: u32,
    gate_decision: crate::types::GateDecision,
) -> crate::types::WitnessLog {
    crate::types::WitnessLog::new(
        model_hash,
        quant_hash,
        backend,
        cycles,
        latency_ns,
        gate_decision,
    )
}

/// Command types for daemon protocol
pub mod commands {
    /// Load model command
    pub const LOAD_MODEL: u8 = 0x01;
    /// Unload model command
    pub const UNLOAD_MODEL: u8 = 0x02;
    /// Inference request command
    pub const INFER: u8 = 0x03;
    /// Ping/health check command
    pub const PING: u8 = 0x04;
    /// Get status command
    pub const STATUS: u8 = 0x05;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_frame_roundtrip() {
        let model_id = ModelId::new([0x42u8; 32]);
        let frame = RequestFrame::new(64, 256, 32000, &model_id, 0, 16);
        let bytes = frame.to_bytes();

        assert_eq!(bytes.len(), protocol::HEADER_SIZE);
        assert_eq!(&bytes[0..4], &protocol::MAGIC.to_le_bytes());
    }

    #[test]
    fn test_crc32() {
        let data = b"test data";
        let crc = crc32(data);
        // CRC should be consistent
        assert_eq!(crc, crc32(data));
    }

    #[test]
    fn test_compute_topk() {
        let logits: Vec<i16> = vec![100, 50, 300, 200, 150];
        let topk = compute_topk(&logits, 3);

        assert_eq!(topk.len(), 3);
        assert_eq!(topk[0], (2, 300)); // Index 2, value 300
        assert_eq!(topk[1], (3, 200)); // Index 3, value 200
        assert_eq!(topk[2], (4, 150)); // Index 4, value 150
    }

    #[test]
    fn test_compute_topk_large() {
        let logits: Vec<i16> = (0..1000).map(|i| (i * 7 % 500) as i16).collect();
        let topk = compute_topk(&logits, 10);

        assert_eq!(topk.len(), 10);
        // Should be sorted descending
        for i in 1..topk.len() {
            assert!(topk[i - 1].1 >= topk[i].1);
        }
    }

    #[test]
    fn test_validate_tokens() {
        assert!(validate_tokens(&[0, 1, 2], 100).is_ok());
        assert!(validate_tokens(&[99], 100).is_ok());
        assert!(validate_tokens(&[100], 100).is_err());
        assert!(validate_tokens(&[0, 50, 101], 100).is_err());
    }
}
