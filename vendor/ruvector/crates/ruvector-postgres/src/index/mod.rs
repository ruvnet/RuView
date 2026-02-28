//! Index implementations for vector similarity search
//!
//! Provides HNSW and IVFFlat index types compatible with pgvector.
//!
//! ## Index Types
//!
//! - **HNSW**: Hierarchical Navigable Small World graphs for fast ANN search
//! - **IVFFlat**: Inverted File with Flat quantization for scalable search
//!
//! ## Access Methods (PostgreSQL Integration)
//!
//! - `ruhnsw`: HNSW index access method
//! - `ruivfflat`: IVFFlat index access method (v2 with quantization support)
//!
//! ## SQL Usage
//!
//! ```sql
//! -- Create HNSW index
//! CREATE INDEX idx ON items USING ruhnsw (embedding vector_l2_ops)
//!     WITH (m=16, ef_construction=64);
//!
//! -- Create IVFFlat index with quantization
//! CREATE INDEX idx ON items USING ruivfflat (embedding vector_l2_ops)
//!     WITH (lists=100, quantization='sq8');
//!
//! -- Runtime configuration
//! SET ruvector.ivfflat_probes = 10;
//! SELECT ruivfflat_set_adaptive_probes(true);
//! ```

mod hnsw;
mod ivfflat;
mod scan;

// Access Method implementations (PostgreSQL Index AM)
mod hnsw_am;
mod ivfflat_am;
mod ivfflat_storage;

// Parallel execution support
// pub mod parallel;
// pub mod bgworker;
// pub mod parallel_ops;

pub use hnsw::*;
pub use ivfflat::*;
pub use scan::*;

use std::sync::atomic::{AtomicUsize, Ordering};

/// Global index memory tracking
static INDEX_MEMORY_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Get total index memory in MB
pub fn get_total_index_memory_mb() -> f64 {
    INDEX_MEMORY_BYTES.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

/// Track index memory allocation
pub fn track_index_allocation(bytes: usize) {
    INDEX_MEMORY_BYTES.fetch_add(bytes, Ordering::Relaxed);
}

/// Track index memory deallocation
pub fn track_index_deallocation(bytes: usize) {
    INDEX_MEMORY_BYTES.fetch_sub(bytes, Ordering::Relaxed);
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub name: String,
    pub index_type: String,
    pub vector_count: i64,
    pub dimensions: i32,
    pub index_size_mb: f64,
    pub fragmentation_pct: f64,
}

/// Get statistics for all indexes
pub fn get_all_index_stats() -> Vec<IndexStats> {
    // This would query PostgreSQL's system catalogs
    // For now, return empty
    Vec::new()
}

/// Maintenance result
#[derive(Debug)]
pub struct MaintenanceStats {
    pub nodes_updated: usize,
    pub connections_optimized: usize,
    pub memory_reclaimed_bytes: usize,
    pub duration_ms: u64,
}

/// Perform index maintenance
pub fn perform_maintenance(_index_name: &str) -> Result<MaintenanceStats, String> {
    // Would perform actual maintenance operations
    Ok(MaintenanceStats {
        nodes_updated: 0,
        connections_optimized: 0,
        memory_reclaimed_bytes: 0,
        duration_ms: 0,
    })
}
