//! # RuVector-Postgres
//!
//! High-performance PostgreSQL extension for vector similarity search.
//! A drop-in replacement for pgvector with SIMD optimizations.

// Allow development-stage lints for work-in-progress code
#![allow(unexpected_cfgs)] // pgrx macros (pg12/pg13) and optional features (tokio)
#![allow(dead_code)] // Stub implementations and future features
#![allow(unused_variables)] // WIP function signatures
#![allow(unused_mut)]
// Variables prepared for future mutation
// Allow clippy lints common in pgrx extensions and WIP code
#![allow(clippy::all)] // Allow all clippy warnings for development
#![allow(for_loops_over_fallibles)] // pgrx derive macro generates this pattern

use pgrx::prelude::*;
use pgrx::{GucContext, GucFlags, GucRegistry, GucSetting};

// Initialize the extension
::pgrx::pg_module_magic!();

// Module declarations
pub mod attention;
pub mod dag;
pub mod distance;
pub mod gnn;
pub mod graph;
pub mod healing;
pub mod hybrid;
pub mod hyperbolic;
pub mod index;
pub mod integrity;
pub mod learning;
pub mod operators;
pub mod quantization;
pub mod routing;
pub mod sparse;
pub mod tenancy;
pub mod types;
pub mod workers;

// Optional: Local embedding generation (requires 'embeddings' feature)
#[cfg(feature = "embeddings")]
pub mod embeddings;

// Optional: Mincut-gated transformer (requires 'gated-transformer' feature)
#[cfg(feature = "gated-transformer")]
pub mod gated_transformer;

// v0.3 feature-gated modules
#[cfg(feature = "solver")]
pub mod solver;

#[cfg(feature = "math-distances")]
pub mod math;

#[cfg(feature = "tda")]
pub mod tda;

#[cfg(feature = "sona-learning")]
pub mod sona;

#[cfg(feature = "domain-expansion")]
pub mod domain_expansion;

// Re-exports for convenience
pub use distance::{cosine_distance, euclidean_distance, inner_product_distance, DistanceMetric};
pub use types::RuVector;

/// Extension version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum supported vector dimensions
pub const MAX_DIMENSIONS: usize = 16_000;

/// Default HNSW parameters
pub const DEFAULT_HNSW_M: usize = 16;
pub const DEFAULT_HNSW_EF_CONSTRUCTION: usize = 64;
pub const DEFAULT_HNSW_EF_SEARCH: usize = 40;

/// Default IVFFlat parameters
pub const DEFAULT_IVFFLAT_LISTS: usize = 100;
pub const DEFAULT_IVFFLAT_PROBES: usize = 1;

// GUC variables
static EF_SEARCH: GucSetting<i32> = GucSetting::<i32>::new(DEFAULT_HNSW_EF_SEARCH as i32);
static PROBES: GucSetting<i32> = GucSetting::<i32>::new(DEFAULT_IVFFLAT_PROBES as i32);

// Hybrid search GUC variables
static HYBRID_ALPHA: GucSetting<f64> = GucSetting::<f64>::new(0.5);
static HYBRID_RRF_K: GucSetting<i32> = GucSetting::<i32>::new(60);
static HYBRID_PREFETCH_K: GucSetting<i32> = GucSetting::<i32>::new(100);

// ============================================================================
// Extension Initialization
// ============================================================================

/// Called when the extension is loaded
#[pg_guard]
pub extern "C" fn _PG_init() {
    // Initialize SIMD dispatch
    distance::init_simd_dispatch();

    // Register GUCs
    GucRegistry::define_int_guc(
        "ruvector.ef_search",
        "HNSW ef_search parameter for query time",
        "Higher values improve recall at the cost of speed",
        &EF_SEARCH,
        1,
        1000,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_int_guc(
        "ruvector.probes",
        "IVFFlat number of lists to probe",
        "Higher values improve recall at the cost of speed",
        &PROBES,
        1,
        10000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // Hybrid search GUCs
    GucRegistry::define_float_guc(
        "ruvector.hybrid_alpha",
        "Default alpha for hybrid linear fusion (0=keyword only, 1=vector only)",
        "Controls the blend between vector and keyword search",
        &HYBRID_ALPHA,
        0.0,
        1.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_int_guc(
        "ruvector.hybrid_rrf_k",
        "RRF constant for hybrid search (default 60)",
        "Lower values give more weight to top-ranked results",
        &HYBRID_RRF_K,
        1,
        1000,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_int_guc(
        "ruvector.hybrid_prefetch_k",
        "Number of results to prefetch from each branch",
        "Higher values improve recall but increase latency",
        &HYBRID_PREFETCH_K,
        1,
        10000,
        GucContext::Userset,
        GucFlags::default(),
    );

    // Initialize tenant GUCs for multi-tenancy
    tenancy::init_tenant_gucs();

    // Note: DAG neural learning is initialized on first use
    // No explicit initialization required - state is lazy-loaded

    // Log initialization
    pgrx::log!(
        "RuVector {} initialized with {} SIMD support",
        VERSION,
        distance::simd_info()
    );
}

// ============================================================================
// SQL Functions
// ============================================================================

/// Returns the extension version
#[pg_extern]
fn ruvector_version() -> &'static str {
    VERSION
}

/// Returns SIMD capability information
#[pg_extern]
fn ruvector_simd_info() -> String {
    distance::simd_info_detailed()
}

/// Returns memory statistics for the extension
#[pg_extern]
fn ruvector_memory_stats() -> pgrx::JsonB {
    let stats = serde_json::json!({
        "index_memory_mb": index::get_total_index_memory_mb(),
        "vector_cache_mb": types::get_vector_cache_memory_mb(),
        "quantization_tables_mb": quantization::get_table_memory_mb(),
        "total_extension_mb": index::get_total_index_memory_mb() +
                              types::get_vector_cache_memory_mb() +
                              quantization::get_table_memory_mb(),
    });
    pgrx::JsonB(stats)
}

/// Perform index maintenance
#[pg_extern]
fn ruvector_index_maintenance(index_name: &str) -> String {
    match index::perform_maintenance(index_name) {
        Ok(stats) => format!("Maintenance completed: {:?}", stats),
        Err(e) => format!("Maintenance failed: {}", e),
    }
}

// ============================================================================
// Quantization Functions (Array-based)
// ============================================================================

/// Binary quantize a vector (array-based)
#[pg_extern(immutable, parallel_safe)]
fn binary_quantize_arr(v: Vec<f32>) -> Vec<u8> {
    quantization::binary::quantize(&v)
}

/// Scalar quantize a vector (SQ8) (array-based)
#[pg_extern(immutable, parallel_safe)]
fn scalar_quantize_arr(v: Vec<f32>) -> pgrx::JsonB {
    let (quantized, scale, offset) = quantization::scalar::quantize(&v);
    pgrx::JsonB(serde_json::json!({
        "data": quantized,
        "scale": scale,
        "offset": offset,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_version() {
        assert!(!ruvector_version().is_empty());
    }

    #[pg_test]
    fn test_simd_info() {
        let info = ruvector_simd_info();
        assert!(
            info.contains("avx512")
                || info.contains("avx2")
                || info.contains("neon")
                || info.contains("scalar")
        );
    }
}

/// Bootstrap the extension (called by pgrx)
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {}
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        vec![]
    }
}
