//! # ruQu - Classical Nervous System for Quantum Machines
//!
//! Real-time syndrome processing and coherence assessment for quantum systems.
//!
//! This crate provides high-throughput, low-latency data pipelines for ingesting,
//! buffering, and transforming quantum error syndromes into coherence-relevant signals.
//!
//! ## Architecture
//!
//! ruQu is organized into several bounded contexts following Domain-Driven Design:
//!
//! - **Syndrome Processing** (Supporting Domain): High-throughput data acquisition
//! - **Coherence Gate** (Core Domain): Real-time structural assessment
//! - **Tile Architecture**: 256-tile WASM fabric for parallel processing
//!
//! The system uses a two-layer classical control approach:
//! 1. **RuVector Memory Layer**: Pattern recognition and historical mitigation retrieval
//! 2. **Dynamic Min-Cut Gate**: Real El-Hayek/Henzinger/Li O(n^{o(1)}) algorithm
//!
//! ## Quick Start
//!
//! ```rust
//! use ruqu::syndrome::{DetectorBitmap, SyndromeRound, SyndromeBuffer};
//!
//! // Create a detector bitmap for 64 detectors
//! let mut bitmap = DetectorBitmap::new(64);
//! bitmap.set(0, true);
//! bitmap.set(5, true);
//! bitmap.set(63, true);
//!
//! assert_eq!(bitmap.fired_count(), 3);
//!
//! // Create a syndrome round
//! let round = SyndromeRound {
//!     round_id: 1,
//!     cycle: 1000,
//!     timestamp: 1705500000000,
//!     detectors: bitmap,
//!     source_tile: 0,
//! };
//!
//! // Buffer rounds for analysis
//! let mut buffer = SyndromeBuffer::new(1024);
//! buffer.push(round);
//! ```
//!
//! ## Three-Filter Decision Logic
//!
//! The coherence gate uses three stacked filters:
//! 1. **Structural Filter**: Min-cut based stability assessment
//! 2. **Shift Filter**: Drift detection from baseline patterns
//! 3. **Evidence Filter**: Anytime-valid e-value accumulation
//!
//! All three must pass for PERMIT. Any one can trigger DENY or DEFER.
//!
//! ## Performance Targets
//!
//! - Gate decision latency: < 4 microseconds p99
//! - Syndrome ingestion: 1M rounds/second
//! - Memory per tile: 64KB
//! - Total latency budget: ~2,350ns
//!
//! ## Feature Flags
//!
//! - `structural` - Enable min-cut based structural filter (requires ruvector-mincut)
//! - `tilezero` - Enable TileZero arbiter integration (requires cognitum-gate-tilezero)
//! - `simd` - Enable SIMD acceleration for bitmap operations
//! - `wasm` - WASM-compatible mode (disables native SIMD)
//! - `full` - Enable all features

#![deny(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

// Core modules
pub mod attention;
pub mod decoder;
pub mod error;
pub mod fabric;
pub mod filters;
pub mod mincut;
pub mod syndrome;
pub mod tile;
pub mod types;

// Advanced features
pub mod adaptive;
pub mod metrics;
pub mod parallel;
pub mod stim;

// Production interfaces
pub mod schema;
pub mod traits;

// Re-exports for convenient access
pub use adaptive::{
    AdaptiveStats, AdaptiveThresholds, DriftConfig, DriftDetector, DriftDirection, DriftProfile,
    LearningConfig,
};
pub use attention::{AttentionConfig, AttentionStats, CoherenceAttention, GatePacketBridge};
pub use decoder::{Correction, DecoderConfig, MWPMDecoder, StreamingDecoder};
pub use error::{Result, RuQuError};
pub use fabric::{
    linear_patch_map, surface_code, surface_code_d7, CoherenceGate, DecisionStats, FabricBuilder,
    FabricConfig, FabricState, FilterSummary, PatchMap, QuantumFabric, TileAssignment,
    WitnessReceipt,
};
pub use filters::{
    EdgeId as FilterEdgeId, EvidenceAccumulator, EvidenceFilter, EvidenceResult, FilterConfig,
    FilterPipeline, FilterResults, RegionMask, ShiftFilter, ShiftResult, StructuralFilter,
    StructuralResult, SystemState, Verdict,
};
pub use metrics::{Counter, Gauge, Histogram, MetricsCollector, MetricsConfig, MetricsSnapshot};
pub use mincut::{DynamicMinCutEngine, MinCutResult};
pub use parallel::{parallel_aggregate, ParallelConfig, ParallelFabric, ParallelStats};
pub use stim::{ErrorPatternGenerator, StimSyndromeSource, SurfaceCodeConfig, SyndromeStats};
pub use syndrome::{
    BufferStatistics, DetectorBitmap, SyndromeBuffer, SyndromeDelta, SyndromeRound,
};
pub use tile::{
    GateDecision, GateThresholds, LocalCutState, PatchGraph, PermitToken, ReceiptLog, TileReport,
    TileZero, WorkerTile,
};
pub use types::{
    ActionId, CycleId, GateDecision as DomainGateDecision, RegionMask as DomainRegionMask, RoundId,
    SequenceId, TileId as DomainTileId,
};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Maximum number of detectors supported (1024 = 16 * 64 bits)
pub const MAX_DETECTORS: usize = 1024;

/// Default buffer capacity in rounds
pub const DEFAULT_BUFFER_CAPACITY: usize = 1024;

/// Total number of tiles in the fabric
pub const TILE_COUNT: usize = 256;

/// Number of worker tiles (excluding TileZero)
pub const WORKER_TILE_COUNT: usize = 255;

/// Memory budget per tile in bytes (64KB)
pub const TILE_MEMORY_BUDGET: usize = 65536;

/// Prelude module for convenient imports
pub mod prelude {
    //! Commonly used types for syndrome processing, filters, and tile architecture.
    pub use crate::adaptive::{
        AdaptiveStats, AdaptiveThresholds, DriftConfig, DriftDetector, DriftProfile, LearningConfig,
    };
    pub use crate::error::{Result, RuQuError};
    pub use crate::fabric::{
        linear_patch_map, surface_code, surface_code_d7, CoherenceGate, DecisionStats,
        FabricBuilder, FabricConfig, FabricState, PatchMap, QuantumFabric, TileAssignment,
        WitnessReceipt,
    };
    pub use crate::filters::{
        EvidenceAccumulator, EvidenceFilter, EvidenceResult, FilterConfig, FilterPipeline,
        FilterResults, RegionMask, ShiftFilter, ShiftResult, StructuralFilter, StructuralResult,
        SystemState, Verdict,
    };
    pub use crate::metrics::{MetricsCollector, MetricsConfig, MetricsSnapshot};
    pub use crate::parallel::{ParallelConfig, ParallelFabric, ParallelStats};
    pub use crate::stim::{StimSyndromeSource, SurfaceCodeConfig, SyndromeStats};
    pub use crate::syndrome::{
        BufferStatistics, DetectorBitmap, SyndromeBuffer, SyndromeDelta, SyndromeRound,
    };
    pub use crate::tile::{
        GateDecision, GateThresholds, LocalCutState, PatchGraph, PermitToken, ReceiptLog,
        TileReport, TileZero, WorkerTile,
    };
    pub use crate::types::{
        ActionId, CycleId, GateDecision as DomainGateDecision, RegionMask as DomainRegionMask,
        RoundId, SequenceId,
    };
    pub use crate::{
        DEFAULT_BUFFER_CAPACITY, MAX_DETECTORS, TILE_COUNT, TILE_MEMORY_BUDGET, WORKER_TILE_COUNT,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant() {
        assert!(!VERSION.is_empty());
        assert!(!NAME.is_empty());
        assert_eq!(NAME, "ruqu");
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_DETECTORS, 1024);
        assert_eq!(DEFAULT_BUFFER_CAPACITY, 1024);
    }
}
