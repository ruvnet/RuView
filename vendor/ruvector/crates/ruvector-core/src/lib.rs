//! # Ruvector Core
//!
//! High-performance Rust-native vector database with HNSW indexing and SIMD-optimized operations.
//!
//! ## Working Features (Tested & Benchmarked)
//!
//! - **HNSW Indexing**: Approximate nearest neighbor search with O(log n) complexity
//! - **SIMD Distance**: SimSIMD-powered distance calculations (~16M ops/sec for 512-dim)
//! - **Quantization**: Scalar (4x), Int4 (8x), Product (8-16x), and binary (32x) compression with distance support
//! - **Persistence**: REDB-based storage with config persistence
//! - **Search**: ~2.5K queries/sec on 10K vectors (benchmarked)
//!
//! ## ⚠️ Experimental/Incomplete Features - READ BEFORE USE
//!
//! - **AgenticDB**: ⚠️⚠️⚠️ **CRITICAL WARNING** ⚠️⚠️⚠️
//!   - Uses PLACEHOLDER hash-based embeddings, NOT real semantic embeddings
//!   - "dog" and "cat" will NOT be similar (different characters)
//!   - "dog" and "god" WILL be similar (same characters) - **This is wrong!**
//!   - **MUST integrate real embedding model for production** (ONNX, Candle, or API)
//!   - See [`agenticdb`] module docs and `/examples/onnx-embeddings` for integration
//! - **Advanced Features**: Conformal prediction, hybrid search - functional but less tested
//!
//! ## What This Is NOT
//!
//! - This is NOT a complete RAG solution - you need external embedding models
//! - Examples use mock embeddings for demonstration only

#![allow(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::incompatible_msrv)]

pub mod advanced_features;

// AgenticDB requires storage feature
#[cfg(feature = "storage")]
pub mod agenticdb;

pub mod distance;
pub mod embeddings;
pub mod error;
pub mod index;
pub mod quantization;

// Storage backends - conditional compilation based on features
#[cfg(feature = "storage")]
pub mod storage;

#[cfg(not(feature = "storage"))]
pub mod storage_memory;

#[cfg(not(feature = "storage"))]
pub use storage_memory as storage;

pub mod types;
pub mod vector_db;

// Performance optimization modules
pub mod arena;
pub mod cache_optimized;
#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
pub mod lockfree;
pub mod simd_intrinsics;

/// Unified Memory Pool and Paging System (ADR-006)
///
/// High-performance paged memory management for LLM inference:
/// - 2MB page-granular allocation with best-fit strategy
/// - Reference-counted pinning with RAII guards
/// - LRU eviction with hysteresis for thrash prevention
/// - Multi-tenant isolation with Hot/Warm/Cold residency tiers
pub mod memory;

/// Advanced techniques: hypergraphs, learned indexes, neural hashing, TDA (Phase 6)
pub mod advanced;

// Re-exports
pub use advanced_features::{
    ConformalConfig, ConformalPredictor, EnhancedPQ, FilterExpression, FilterStrategy,
    FilteredSearch, HybridConfig, HybridSearch, MMRConfig, MMRSearch, PQConfig, PredictionSet,
    BM25,
};

#[cfg(feature = "storage")]
pub use agenticdb::{
    AgenticDB, PolicyAction, PolicyEntry, PolicyMemoryStore, SessionStateIndex, SessionTurn,
    WitnessEntry, WitnessLog,
};

#[cfg(feature = "api-embeddings")]
pub use embeddings::ApiEmbedding;
pub use embeddings::{BoxedEmbeddingProvider, EmbeddingProvider, HashEmbedding};

#[cfg(feature = "real-embeddings")]
pub use embeddings::CandleEmbedding;

// Compile-time warning about AgenticDB limitations
#[cfg(feature = "storage")]
#[allow(deprecated, clippy::let_unit_value)]
const _: () = {
    #[deprecated(
        since = "0.1.0",
        note = "AgenticDB uses placeholder hash-based embeddings. For semantic search, integrate a real embedding model (ONNX, Candle, or API). See /examples/onnx-embeddings for production setup."
    )]
    const AGENTICDB_EMBEDDING_WARNING: () = ();
    let _ = AGENTICDB_EMBEDDING_WARNING;
};

pub use error::{Result, RuvectorError};
pub use types::{DistanceMetric, SearchQuery, SearchResult, VectorEntry, VectorId};
pub use vector_db::VectorDB;

// Quantization types (ADR-001)
pub use quantization::{
    BinaryQuantized, Int4Quantized, ProductQuantized, QuantizedVector, ScalarQuantized,
};

// Memory management types (ADR-001)
pub use arena::{Arena, ArenaVec, BatchVectorAllocator, CacheAlignedVec, CACHE_LINE_SIZE};

// Lock-free structures (requires parallel feature)
#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
pub use lockfree::{
    AtomicVectorPool, BatchItem, BatchResult, LockFreeBatchProcessor, LockFreeCounter,
    LockFreeStats, LockFreeWorkQueue, ObjectPool, PooledObject, PooledVector, StatsSnapshot,
    VectorPoolStats,
};

// Cache-optimized storage
pub use cache_optimized::SoAVectorStorage;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        // Verify version matches workspace - use dynamic check instead of hardcoded value
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty(), "Version should not be empty");
        assert!(version.starts_with("0.1."), "Version should be 0.1.x");
    }
}
