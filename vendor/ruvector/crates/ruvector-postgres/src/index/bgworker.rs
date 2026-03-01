//! Background worker for index maintenance and optimization
//!
//! Implements PostgreSQL background worker for:
//! - Periodic index optimization
//! - Index statistics collection
//! - Vacuum and cleanup operations
//! - Automatic reindexing for heavily updated indexes

use pgrx::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;

// ============================================================================
// Background Worker Configuration
// ============================================================================

/// Configuration for RuVector background worker
#[derive(Debug, Clone)]
pub struct BgWorkerConfig {
    /// Maintenance interval in seconds
    pub maintenance_interval_secs: u64,
    /// Whether to perform automatic optimization
    pub auto_optimize: bool,
    /// Whether to collect statistics
    pub collect_stats: bool,
    /// Whether to perform automatic vacuum
    pub auto_vacuum: bool,
    /// Minimum age (in seconds) before vacuuming an index
    pub vacuum_min_age_secs: u64,
    /// Maximum number of indexes to process per cycle
    pub max_indexes_per_cycle: usize,
    /// Optimization threshold (e.g., 10% deleted tuples)
    pub optimize_threshold: f32,
}

impl Default for BgWorkerConfig {
    fn default() -> Self {
        Self {
            maintenance_interval_secs: 300, // 5 minutes
            auto_optimize: true,
            collect_stats: true,
            auto_vacuum: true,
            vacuum_min_age_secs: 3600, // 1 hour
            max_indexes_per_cycle: 10,
            optimize_threshold: 0.10, // 10%
        }
    }
}

/// Global background worker state
pub struct BgWorkerState {
    /// Configuration
    config: RwLock<BgWorkerConfig>,
    /// Whether worker is running
    running: AtomicBool,
    /// Last maintenance timestamp
    last_maintenance: AtomicU64,
    /// Total maintenance cycles completed
    cycles_completed: AtomicU64,
    /// Total indexes maintained
    indexes_maintained: AtomicU64,
}

impl BgWorkerState {
    /// Create new background worker state
    pub fn new(config: BgWorkerConfig) -> Self {
        Self {
            config: RwLock::new(config),
            running: AtomicBool::new(false),
            last_maintenance: AtomicU64::new(0),
            cycles_completed: AtomicU64::new(0),
            indexes_maintained: AtomicU64::new(0),
        }
    }

    /// Check if worker is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Start worker
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Stop worker
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Get statistics
    pub fn get_stats(&self) -> BgWorkerStats {
        BgWorkerStats {
            running: self.running.load(Ordering::SeqCst),
            last_maintenance: self.last_maintenance.load(Ordering::SeqCst),
            cycles_completed: self.cycles_completed.load(Ordering::SeqCst),
            indexes_maintained: self.indexes_maintained.load(Ordering::SeqCst),
        }
    }

    /// Record maintenance cycle
    fn record_cycle(&self, indexes_count: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.last_maintenance.store(now, Ordering::SeqCst);
        self.cycles_completed.fetch_add(1, Ordering::SeqCst);
        self.indexes_maintained.fetch_add(indexes_count, Ordering::SeqCst);
    }
}

/// Background worker statistics
#[derive(Debug, Clone)]
pub struct BgWorkerStats {
    pub running: bool,
    pub last_maintenance: u64,
    pub cycles_completed: u64,
    pub indexes_maintained: u64,
}

// Global worker state
static WORKER_STATE: std::sync::OnceLock<Arc<BgWorkerState>> = std::sync::OnceLock::new();

fn get_worker_state() -> &'static Arc<BgWorkerState> {
    WORKER_STATE.get_or_init(|| {
        Arc::new(BgWorkerState::new(BgWorkerConfig::default()))
    })
}

// ============================================================================
// Background Worker Entry Point
// ============================================================================

/// Main background worker function
///
/// This is registered with PostgreSQL and runs in a separate background process.
#[pg_guard]
pub extern "C" fn ruvector_bgworker_main(_arg: pg_sys::Datum) {
    // Initialize worker
    pgrx::log!("RuVector background worker starting");

    let worker_state = get_worker_state();
    worker_state.start();

    // Main loop
    while worker_state.is_running() {
        // Perform maintenance cycle
        if let Err(e) = perform_maintenance_cycle() {
            pgrx::warning!("Background worker maintenance failed: {}", e);
        }

        // Sleep until next cycle
        let interval = {
            let config = worker_state.config.read();
            config.maintenance_interval_secs
        };

        // Use PostgreSQL's WaitLatch for interruptible sleep
        unsafe {
            pg_sys::WaitLatch(
                pg_sys::MyLatch,
                pg_sys::WL_LATCH_SET as i32 | pg_sys::WL_TIMEOUT as i32,
                (interval * 1000) as i64, // Convert to milliseconds
                pg_sys::PG_WAIT_EXTENSION as u32,
            );
            pg_sys::ResetLatch(pg_sys::MyLatch);
        }

        // Check for shutdown signal
        if unsafe { pg_sys::ShutdownRequestPending } {
            break;
        }
    }

    worker_state.stop();
    pgrx::log!("RuVector background worker stopped");
}

// ============================================================================
// Maintenance Operations
// ============================================================================

/// Perform one maintenance cycle
fn perform_maintenance_cycle() -> Result<(), String> {
    let worker_state = get_worker_state();
    let config = worker_state.config.read().clone();
    drop(worker_state.config.read());

    // Find all RuVector indexes
    let indexes = find_ruvector_indexes(config.max_indexes_per_cycle)?;

    let mut maintained_count = 0u64;

    for index_info in indexes {
        // Perform maintenance operations
        if config.collect_stats {
            if let Err(e) = collect_index_stats(&index_info) {
                pgrx::warning!("Failed to collect stats for index {}: {}", index_info.name, e);
            }
        }

        if config.auto_optimize {
            if let Err(e) = optimize_index_if_needed(&index_info, config.optimize_threshold) {
                pgrx::warning!("Failed to optimize index {}: {}", index_info.name, e);
            } else {
                maintained_count += 1;
            }
        }

        if config.auto_vacuum {
            if let Err(e) = vacuum_index_if_needed(&index_info, config.vacuum_min_age_secs) {
                pgrx::warning!("Failed to vacuum index {}: {}", index_info.name, e);
            }
        }
    }

    worker_state.record_cycle(maintained_count);

    Ok(())
}

/// Index information
#[derive(Debug, Clone)]
struct IndexInfo {
    name: String,
    oid: pg_sys::Oid,
    relation_oid: pg_sys::Oid,
    index_type: String, // "ruhnsw" or "ruivfflat"
    size_bytes: i64,
    tuple_count: i64,
    last_vacuum: Option<u64>,
}

/// Find all RuVector indexes in the database
fn find_ruvector_indexes(max_count: usize) -> Result<Vec<IndexInfo>, String> {
    let mut indexes = Vec::new();

    // Query pg_class for indexes using our access methods
    // This is a simplified version - in production, use SPI to query system catalogs

    // For now, return empty list (would be populated via SPI query in production)
    // Example query:
    // SELECT c.relname, c.oid, c.relfilenode, am.amname, pg_relation_size(c.oid)
    // FROM pg_class c
    // JOIN pg_am am ON c.relam = am.oid
    // WHERE am.amname IN ('ruhnsw', 'ruivfflat')
    // LIMIT $max_count

    Ok(indexes)
}

/// Collect statistics for an index
fn collect_index_stats(index: &IndexInfo) -> Result<(), String> {
    pgrx::debug1!("Collecting stats for index: {}", index.name);

    // In production, collect:
    // - Index size
    // - Number of tuples
    // - Number of deleted tuples
    // - Fragmentation level
    // - Average search depth
    // - Distribution statistics

    Ok(())
}

/// Optimize index if it exceeds threshold
fn optimize_index_if_needed(index: &IndexInfo, threshold: f32) -> Result<(), String> {
    // Check if optimization is needed
    let fragmentation = calculate_fragmentation(index)?;

    if fragmentation > threshold {
        pgrx::log!(
            "Optimizing index {} (fragmentation: {:.2}%)",
            index.name,
            fragmentation * 100.0
        );

        optimize_index(index)?;
    }

    Ok(())
}

/// Calculate index fragmentation ratio
fn calculate_fragmentation(_index: &IndexInfo) -> Result<f32, String> {
    // In production:
    // - Count deleted/obsolete tuples
    // - Measure graph connectivity (for HNSW)
    // - Check for unbalanced partitions

    // For now, return low fragmentation
    Ok(0.05)
}

/// Perform index optimization
fn optimize_index(index: &IndexInfo) -> Result<(), String> {
    match index.index_type.as_str() {
        "ruhnsw" => optimize_hnsw_index(index),
        "ruivfflat" => optimize_ivfflat_index(index),
        _ => Err(format!("Unknown index type: {}", index.index_type)),
    }
}

/// Optimize HNSW index
fn optimize_hnsw_index(index: &IndexInfo) -> Result<(), String> {
    pgrx::log!("Optimizing HNSW index: {}", index.name);

    // HNSW optimization operations:
    // 1. Remove deleted nodes
    // 2. Rebuild edges for improved connectivity
    // 3. Rebalance layers
    // 4. Compact memory

    Ok(())
}

/// Optimize IVFFlat index
fn optimize_ivfflat_index(index: &IndexInfo) -> Result<(), String> {
    pgrx::log!("Optimizing IVFFlat index: {}", index.name);

    // IVFFlat optimization operations:
    // 1. Recompute centroids
    // 2. Rebalance lists
    // 3. Remove deleted vectors
    // 4. Update statistics

    Ok(())
}

/// Vacuum index if needed
fn vacuum_index_if_needed(index: &IndexInfo, min_age_secs: u64) -> Result<(), String> {
    // Check if vacuum is needed based on age
    if let Some(last_vacuum) = index.last_vacuum {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now - last_vacuum < min_age_secs {
            return Ok(()); // Too soon
        }
    }

    pgrx::log!("Vacuuming index: {}", index.name);

    // Perform vacuum
    // In production, use PostgreSQL's vacuum infrastructure

    Ok(())
}

// ============================================================================
// SQL Functions for Background Worker Control
// ============================================================================

/// Start the background worker
#[pg_extern]
pub fn ruvector_bgworker_start() -> bool {
    let worker_state = get_worker_state();
    if worker_state.is_running() {
        pgrx::warning!("Background worker is already running");
        return false;
    }

    // In production, register and launch the background worker
    // For now, just mark as started
    worker_state.start();
    pgrx::log!("Background worker started");
    true
}

/// Stop the background worker
#[pg_extern]
pub fn ruvector_bgworker_stop() -> bool {
    let worker_state = get_worker_state();
    if !worker_state.is_running() {
        pgrx::warning!("Background worker is not running");
        return false;
    }

    worker_state.stop();
    pgrx::log!("Background worker stopped");
    true
}

/// Get background worker status and statistics
#[pg_extern]
pub fn ruvector_bgworker_status() -> pgrx::JsonB {
    let worker_state = get_worker_state();
    let stats = worker_state.get_stats();
    let config = worker_state.config.read().clone();

    let status = serde_json::json!({
        "running": stats.running,
        "last_maintenance": stats.last_maintenance,
        "cycles_completed": stats.cycles_completed,
        "indexes_maintained": stats.indexes_maintained,
        "config": {
            "maintenance_interval_secs": config.maintenance_interval_secs,
            "auto_optimize": config.auto_optimize,
            "collect_stats": config.collect_stats,
            "auto_vacuum": config.auto_vacuum,
            "vacuum_min_age_secs": config.vacuum_min_age_secs,
            "max_indexes_per_cycle": config.max_indexes_per_cycle,
            "optimize_threshold": config.optimize_threshold,
        }
    });

    pgrx::JsonB(status)
}

/// Update background worker configuration
#[pg_extern]
pub fn ruvector_bgworker_config(
    maintenance_interval_secs: Option<i32>,
    auto_optimize: Option<bool>,
    collect_stats: Option<bool>,
    auto_vacuum: Option<bool>,
) -> pgrx::JsonB {
    let worker_state = get_worker_state();
    let mut config = worker_state.config.write();

    if let Some(interval) = maintenance_interval_secs {
        if interval > 0 {
            config.maintenance_interval_secs = interval as u64;
        }
    }

    if let Some(optimize) = auto_optimize {
        config.auto_optimize = optimize;
    }

    if let Some(stats) = collect_stats {
        config.collect_stats = stats;
    }

    if let Some(vacuum) = auto_vacuum {
        config.auto_vacuum = vacuum;
    }

    let result = serde_json::json!({
        "status": "updated",
        "config": {
            "maintenance_interval_secs": config.maintenance_interval_secs,
            "auto_optimize": config.auto_optimize,
            "collect_stats": config.collect_stats,
            "auto_vacuum": config.auto_vacuum,
        }
    });

    pgrx::JsonB(result)
}

// ============================================================================
// Worker Registration
// ============================================================================

/// Register background worker with PostgreSQL
///
/// This should be called from _PG_init()
pub fn register_background_worker() {
    // In production, use pg_sys::RegisterBackgroundWorker
    // For now, just log
    pgrx::log!("RuVector background worker registration placeholder");

    // Example registration (pseudo-code):
    // unsafe {
    //     let mut worker = pg_sys::BackgroundWorker::default();
    //     worker.bgw_name = "ruvector maintenance worker";
    //     worker.bgw_type = "ruvector worker";
    //     worker.bgw_flags = BGW_NEVER_RESTART;
    //     worker.bgw_start_time = BgWorkerStartTime::BgWorkerStart_RecoveryFinished;
    //     worker.bgw_main = Some(ruvector_bgworker_main);
    //     pg_sys::RegisterBackgroundWorker(&mut worker);
    // }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_state() {
        let state = BgWorkerState::new(BgWorkerConfig::default());

        assert!(!state.is_running());

        state.start();
        assert!(state.is_running());

        state.stop();
        assert!(!state.is_running());
    }

    #[test]
    fn test_stats_recording() {
        let state = BgWorkerState::new(BgWorkerConfig::default());

        state.record_cycle(5);
        state.record_cycle(3);

        let stats = state.get_stats();
        assert_eq!(stats.cycles_completed, 2);
        assert_eq!(stats.indexes_maintained, 8);
        assert!(stats.last_maintenance > 0);
    }

    #[test]
    fn test_default_config() {
        let config = BgWorkerConfig::default();

        assert_eq!(config.maintenance_interval_secs, 300);
        assert!(config.auto_optimize);
        assert!(config.collect_stats);
        assert!(config.auto_vacuum);
        assert_eq!(config.optimize_threshold, 0.10);
    }
}
