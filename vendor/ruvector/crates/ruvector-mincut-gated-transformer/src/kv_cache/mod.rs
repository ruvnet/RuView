//! Three-Tier Adaptive KV Cache Management System
//!
//! Implements ADR-004: KV Cache Management Strategy for RuvLLM.
//!
//! This module provides a hierarchical KV cache architecture combining:
//! 1. **Hot Buffer** (Tier 1): Recent tokens in FP16/BF16 - full precision
//! 2. **Warm Cache** (Tier 2): Intermediate tokens in 4-bit KIVI quantization
//! 3. **Archive** (Tier 3): Stale tokens in 2-bit KIVI/SQuat/KVQuant
//!
//! # Architecture
//!
//! ```text
//! +---------------------------------------------------------------------+
//! |                      TOKEN SEQUENCE (left=old, right=new)          |
//! |  [0]...[N-1024]...[N-512]...[N-256]...[N-64]...[N-16]...[N-1]...[N] |
//! +---------------------------------------------------------------------+
//!          |              |               |              |
//!          v              v               v              v
//! +----------------+  +----------------+  +----------------+
//! |    TIER 3:     |  |    TIER 2:     |  |    TIER 1:     |
//! |  DEEP ARCHIVE  |  |   WARM CACHE   |  |   HOT BUFFER   |
//! |                |  |                |  |                |
//! |  * 2-bit KIVI  |  |  * 4-bit KIVI  |  |  * FP16/BF16   |
//! |  * SQuat for   |  |  * Per-channel |  |  * Full        |
//! |    extreme     |  |    keys, per-  |  |    precision   |
//! |    contexts    |  |    token vals  |  |  * No quant    |
//! |  * KVQuant for |  |                |  |    overhead    |
//! |    quality-    |  |                |  |                |
//! |    critical    |  |                |  |                |
//! +----------------+  +----------------+  +----------------+
//! ```
//!
//! # Performance
//!
//! | Compression Ratio | Strategy | PPL Degradation |
//! |-------------------|----------|-----------------|
//! | 8x                | 2-bit KIVI | < 0.3 |
//! | 15-22x            | KIVI + SQuat | < 0.3 |
//! | 5.3x              | 3-bit KVQuant | < 0.1 |
//!
//! # Example
//!
//! ```rust,no_run
//! use ruvector_mincut_gated_transformer::kv_cache::{
//!     AdaptiveKVCache, AdaptiveKVCacheConfig, ArchiveQuantizer,
//! };
//!
//! let config = AdaptiveKVCacheConfig {
//!     num_layers: 12,
//!     num_heads: 8,
//!     head_dim: 64,
//!     max_seq_len: 4096,
//!     tail_length: 64,
//!     warm_length: 448,
//!     archive_quantizer: ArchiveQuantizer::Kivi2Bit,
//!     quality_target: 0.97,
//!     enable_rematerialization: false,
//! };
//!
//! let mut cache = AdaptiveKVCache::new(config);
//! ```

#[cfg(feature = "no_std_gateway")]
extern crate alloc;

// Legacy module for backward compatibility
pub mod legacy;

// New three-tier KV cache modules
pub mod hot_buffer;
pub mod kivi;
pub mod kvquant;
pub mod manager;
pub mod metrics;
pub mod policy;
pub mod quantized_store;
pub mod squat;
pub mod tier;

// Re-export legacy types for backward compatibility
pub use legacy::{HadamardTransform, QuantBits, QuantizedKVCache};

// Re-export new three-tier types
pub use hot_buffer::{HotBuffer, HotBufferConfig};
pub use kivi::{KiviQuantizer, QuantScheme, QuantizedKV};
pub use kvquant::{
    CalibrationData, KVQuantKeyMode, KVQuantQuantizer, KVQuantValueMode, PreRoPEKey, QuantizedValue,
};
pub use manager::{AdaptiveKVCache, AdaptiveKVCacheConfig, ArchiveQuantizer};
pub use metrics::{MemoryStats, QualityFeedback, QualityMetric, QualityTracker, TierMetrics};
pub use policy::{
    EvictionDecision, MemoryTracker, RematerializationCostModel, RematerializationPolicy,
    TierPolicy,
};
pub use quantized_store::{DequantizedKV, QuantizedEntry, QuantizedStore, QuantizedStoreConfig};
pub use squat::{QuantizedSubspace, SQuatCompressed, SQuatQuantizer};
pub use tier::{CacheTier, TierBoundary, TierConfig, TierCounts};
