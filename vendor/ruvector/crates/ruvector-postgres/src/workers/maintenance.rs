//! Maintenance Worker - Index Maintenance and Optimization
//!
//! The Maintenance Worker performs periodic operations including:
//! - Index optimization and compaction
//! - Tier management (promote/demote vectors)
//! - Statistics collection
//! - Dead tuple cleanup
//!
//! # Architecture
//!
//! ```text
//! +------------------------------------------------------------------+
//! |                   MAINTENANCE WORKER                             |
//! +------------------------------------------------------------------+
//! |                                                                  |
//! |  +-----------------+     +-------------------+                   |
//! |  | Scheduler       | --> | Operation Router  |                   |
//! |  | (periodic/cron) |     |                   |                   |
//! |  +-----------------+     +-------------------+                   |
//! |                                  |                               |
//! |          +---------------+-------+-------+---------------+       |
//! |          v               v               v               v       |
//! |  +------------+  +------------+  +------------+  +------------+  |
//! |  | Compaction |  | Statistics |  | Tiering    |  | Cleanup    |  |
//! |  | Engine     |  | Collector  |  | Manager    |  | Handler    |  |
//! |  +------------+  +------------+  +------------+  +------------+  |
//! |                                                                  |
//! +------------------------------------------------------------------+
//! ```

use parking_lot::RwLock;
use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::lifecycle::{get_lifecycle_manager, WorkerStatus};

// ============================================================================
// Maintenance Configuration
// ============================================================================

/// Maintenance worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceConfig {
    /// Interval between maintenance cycles (seconds)
    pub interval_secs: u64,
    /// Maximum indexes to process per cycle
    pub max_indexes_per_cycle: usize,
    /// Enable automatic tier management
    pub enable_tiering: bool,
    /// Enable automatic compaction
    pub enable_compaction: bool,
    /// Enable statistics collection
    pub enable_stats: bool,
    /// Enable dead tuple cleanup
    pub enable_cleanup: bool,
    /// Compaction threshold (fragmentation ratio)
    pub compaction_threshold: f32,
    /// Tier check interval (separate from main interval)
    pub tier_check_interval_secs: u64,
    /// Cleanup age threshold (seconds)
    pub cleanup_age_threshold_secs: u64,
    /// Maximum time per maintenance cycle (seconds)
    pub max_cycle_duration_secs: u64,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300, // 5 minutes
            max_indexes_per_cycle: 10,
            enable_tiering: true,
            enable_compaction: true,
            enable_stats: true,
            enable_cleanup: true,
            compaction_threshold: 0.15,        // 15% fragmentation
            tier_check_interval_secs: 3600,    // 1 hour
            cleanup_age_threshold_secs: 86400, // 24 hours
            max_cycle_duration_secs: 60,
        }
    }
}

// Global maintenance configuration
static MAINTENANCE_CONFIG: OnceLock<RwLock<MaintenanceConfig>> = OnceLock::new();

/// Get maintenance configuration
pub fn get_maintenance_config() -> MaintenanceConfig {
    MAINTENANCE_CONFIG
        .get_or_init(|| RwLock::new(MaintenanceConfig::default()))
        .read()
        .clone()
}

/// Set maintenance configuration
pub fn set_maintenance_config(config: MaintenanceConfig) {
    if let Some(cfg) = MAINTENANCE_CONFIG.get() {
        *cfg.write() = config;
    }
}

// ============================================================================
// Index Information
// ============================================================================

/// Information about a RuVector index
#[derive(Debug, Clone)]
pub struct IndexInfo {
    /// Index name
    pub name: String,
    /// Index OID
    pub oid: u32,
    /// Parent relation OID
    pub relation_oid: u32,
    /// Index type (ruhnsw, ruivfflat)
    pub index_type: String,
    /// Size in bytes
    pub size_bytes: i64,
    /// Tuple count
    pub tuple_count: i64,
    /// Last vacuum timestamp
    pub last_vacuum: Option<u64>,
    /// Last analyze timestamp
    pub last_analyze: Option<u64>,
    /// Fragmentation ratio
    pub fragmentation: f32,
    /// Collection ID
    pub collection_id: Option<i32>,
}

// ============================================================================
// Tier Management
// ============================================================================

/// Tier policy for a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierPolicy {
    /// Collection ID
    pub collection_id: i32,
    /// Promotion threshold (access count)
    pub promotion_threshold: u64,
    /// Demotion threshold (days since last access)
    pub demotion_threshold_days: u32,
    /// Hot tier compression (none, sq4, sq8)
    pub hot_compression: String,
    /// Cold tier compression (sq8, pq, binary)
    pub cold_compression: String,
    /// Archive tier compression (pq, binary)
    pub archive_compression: String,
}

/// Tier candidate for promotion/demotion
#[derive(Debug, Clone)]
pub struct TierCandidate {
    /// Vector TID
    pub vector_tid: (u32, u16),
    /// Current tier
    pub current_tier: String,
    /// Target tier
    pub target_tier: String,
    /// Needs promotion
    pub needs_promotion: bool,
    /// Needs demotion
    pub needs_demotion: bool,
    /// Access count
    pub access_count: u64,
    /// Last access timestamp
    pub last_access: u64,
}

// ============================================================================
// Maintenance Statistics
// ============================================================================

/// Maintenance operation statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaintenanceStats {
    /// Total cycles completed
    pub cycles_completed: u64,
    /// Total indexes maintained
    pub indexes_maintained: u64,
    /// Total compactions performed
    pub compactions_performed: u64,
    /// Total bytes reclaimed
    pub bytes_reclaimed: u64,
    /// Total tier promotions
    pub tier_promotions: u64,
    /// Total tier demotions
    pub tier_demotions: u64,
    /// Total stats collections
    pub stats_collections: u64,
    /// Total cleanup operations
    pub cleanup_operations: u64,
    /// Total time spent (microseconds)
    pub total_time_us: u64,
    /// Last cycle duration (microseconds)
    pub last_cycle_duration_us: u64,
    /// Last cycle timestamp
    pub last_cycle_at: u64,
}

/// Atomic maintenance statistics
pub struct MaintenanceStatsAtomic {
    cycles_completed: AtomicU64,
    indexes_maintained: AtomicU64,
    compactions_performed: AtomicU64,
    bytes_reclaimed: AtomicU64,
    tier_promotions: AtomicU64,
    tier_demotions: AtomicU64,
    stats_collections: AtomicU64,
    cleanup_operations: AtomicU64,
    total_time_us: AtomicU64,
    last_cycle_duration_us: AtomicU64,
    last_cycle_at: AtomicU64,
}

impl MaintenanceStatsAtomic {
    pub fn new() -> Self {
        Self {
            cycles_completed: AtomicU64::new(0),
            indexes_maintained: AtomicU64::new(0),
            compactions_performed: AtomicU64::new(0),
            bytes_reclaimed: AtomicU64::new(0),
            tier_promotions: AtomicU64::new(0),
            tier_demotions: AtomicU64::new(0),
            stats_collections: AtomicU64::new(0),
            cleanup_operations: AtomicU64::new(0),
            total_time_us: AtomicU64::new(0),
            last_cycle_duration_us: AtomicU64::new(0),
            last_cycle_at: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> MaintenanceStats {
        MaintenanceStats {
            cycles_completed: self.cycles_completed.load(Ordering::Relaxed),
            indexes_maintained: self.indexes_maintained.load(Ordering::Relaxed),
            compactions_performed: self.compactions_performed.load(Ordering::Relaxed),
            bytes_reclaimed: self.bytes_reclaimed.load(Ordering::Relaxed),
            tier_promotions: self.tier_promotions.load(Ordering::Relaxed),
            tier_demotions: self.tier_demotions.load(Ordering::Relaxed),
            stats_collections: self.stats_collections.load(Ordering::Relaxed),
            cleanup_operations: self.cleanup_operations.load(Ordering::Relaxed),
            total_time_us: self.total_time_us.load(Ordering::Relaxed),
            last_cycle_duration_us: self.last_cycle_duration_us.load(Ordering::Relaxed),
            last_cycle_at: self.last_cycle_at.load(Ordering::Relaxed),
        }
    }
}

impl Default for MaintenanceStatsAtomic {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Maintenance Worker
// ============================================================================

/// Maintenance worker implementation
pub struct MaintenanceWorker {
    /// Worker ID
    id: u64,
    /// Configuration
    config: MaintenanceConfig,
    /// Statistics
    stats: MaintenanceStatsAtomic,
    /// Last tier check time
    last_tier_check: u64,
    /// Running flag
    running: AtomicBool,
}

impl MaintenanceWorker {
    /// Create a new maintenance worker
    pub fn new(id: u64) -> Self {
        Self {
            id,
            config: get_maintenance_config(),
            stats: MaintenanceStatsAtomic::new(),
            last_tier_check: 0,
            running: AtomicBool::new(false),
        }
    }

    /// Run the maintenance worker
    pub fn run(&mut self) {
        pgrx::log!("Maintenance worker {} starting", self.id);

        self.running.store(true, Ordering::SeqCst);

        loop {
            // Check for shutdown
            if get_lifecycle_manager().is_shutdown_requested() {
                break;
            }

            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            // Perform maintenance cycle
            let cycle_start = Instant::now();
            if let Err(e) = self.perform_maintenance_cycle() {
                pgrx::warning!("Maintenance cycle failed: {}", e);
            }
            let cycle_duration = cycle_start.elapsed();

            // Update stats
            let now = current_epoch_secs();
            self.stats.cycles_completed.fetch_add(1, Ordering::Relaxed);
            self.stats
                .total_time_us
                .fetch_add(cycle_duration.as_micros() as u64, Ordering::Relaxed);
            self.stats
                .last_cycle_duration_us
                .store(cycle_duration.as_micros() as u64, Ordering::Relaxed);
            self.stats.last_cycle_at.store(now, Ordering::Relaxed);

            // Sleep until next cycle
            self.sleep_interruptible(self.config.interval_secs);
        }

        self.running.store(false, Ordering::SeqCst);
        pgrx::log!("Maintenance worker {} stopped", self.id);
    }

    /// Perform a single maintenance cycle
    fn perform_maintenance_cycle(&mut self) -> Result<(), String> {
        let cycle_deadline =
            Instant::now() + Duration::from_secs(self.config.max_cycle_duration_secs);

        // Find all RuVector indexes
        let indexes = self.find_ruvector_indexes()?;

        let mut maintained_count = 0u64;

        for index in indexes.iter().take(self.config.max_indexes_per_cycle) {
            // Check cycle deadline
            if Instant::now() > cycle_deadline {
                pgrx::log!("Maintenance cycle deadline reached, stopping early");
                break;
            }

            // Statistics collection
            if self.config.enable_stats {
                if let Err(e) = self.collect_index_stats(index) {
                    pgrx::warning!("Stats collection failed for {}: {}", index.name, e);
                } else {
                    self.stats.stats_collections.fetch_add(1, Ordering::Relaxed);
                }
            }

            // Compaction
            if self.config.enable_compaction {
                if index.fragmentation > self.config.compaction_threshold {
                    pgrx::log!(
                        "Compacting index {} (fragmentation: {:.1}%)",
                        index.name,
                        index.fragmentation * 100.0
                    );

                    match self.compact_index(index) {
                        Ok(bytes) => {
                            self.stats
                                .compactions_performed
                                .fetch_add(1, Ordering::Relaxed);
                            self.stats
                                .bytes_reclaimed
                                .fetch_add(bytes, Ordering::Relaxed);
                            maintained_count += 1;
                        }
                        Err(e) => {
                            pgrx::warning!("Compaction failed for {}: {}", index.name, e);
                        }
                    }
                }
            }

            // Cleanup
            if self.config.enable_cleanup {
                if let Err(e) = self.cleanup_index(index) {
                    pgrx::warning!("Cleanup failed for {}: {}", index.name, e);
                } else {
                    self.stats
                        .cleanup_operations
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        // Tier management (less frequent)
        let now = current_epoch_secs();
        if self.config.enable_tiering
            && now - self.last_tier_check > self.config.tier_check_interval_secs
        {
            if let Err(e) = self.perform_tier_management() {
                pgrx::warning!("Tier management failed: {}", e);
            }
            self.last_tier_check = now;
        }

        self.stats
            .indexes_maintained
            .fetch_add(maintained_count, Ordering::Relaxed);

        Ok(())
    }

    /// Find all RuVector indexes in the database
    fn find_ruvector_indexes(&self) -> Result<Vec<IndexInfo>, String> {
        // In production, use SPI to query pg_class:
        //
        // SELECT c.relname, c.oid, c.relfilenode, am.amname,
        //        pg_relation_size(c.oid), c.reltuples
        // FROM pg_class c
        // JOIN pg_am am ON c.relam = am.oid
        // WHERE am.amname IN ('ruhnsw', 'ruivfflat')
        // LIMIT $max_count

        // Mock implementation
        Ok(vec![])
    }

    /// Collect statistics for an index
    fn collect_index_stats(&self, index: &IndexInfo) -> Result<(), String> {
        pgrx::debug1!("Collecting stats for index: {}", index.name);

        // In production:
        // - Calculate index size
        // - Count tuples
        // - Measure fragmentation
        // - Collect search depth statistics
        // - Update pg_statistic if needed

        Ok(())
    }

    /// Compact an index
    fn compact_index(&self, index: &IndexInfo) -> Result<u64, String> {
        pgrx::log!("Compacting index: {}", index.name);

        // In production:
        // - For HNSW: Rebuild edges, remove deleted nodes, rebalance layers
        // - For IVFFlat: Recompute centroids, rebalance lists

        // Return bytes reclaimed (mock)
        Ok(0)
    }

    /// Cleanup an index
    fn cleanup_index(&self, index: &IndexInfo) -> Result<(), String> {
        pgrx::debug1!("Cleaning up index: {}", index.name);

        // In production:
        // - Remove dead tuples
        // - Clean up transaction metadata
        // - Update visibility map

        Ok(())
    }

    /// Perform tier management for all collections
    fn perform_tier_management(&mut self) -> Result<(), String> {
        pgrx::log!("Performing tier management");

        // Get collections with tiering enabled
        let collections = self.get_tiered_collections()?;

        for collection in collections {
            // Get tier policies
            let policies = self.get_tier_policies(collection)?;

            // Get candidates for promotion/demotion
            let candidates = self.get_tier_candidates(collection, &policies)?;

            for candidate in candidates {
                if candidate.needs_promotion {
                    if let Err(e) = self.promote_vector(&candidate) {
                        pgrx::warning!(
                            "Promotion failed for vector {:?}: {}",
                            candidate.vector_tid,
                            e
                        );
                    } else {
                        self.stats.tier_promotions.fetch_add(1, Ordering::Relaxed);
                    }
                } else if candidate.needs_demotion {
                    if let Err(e) = self.demote_vector(&candidate) {
                        pgrx::warning!(
                            "Demotion failed for vector {:?}: {}",
                            candidate.vector_tid,
                            e
                        );
                    } else {
                        self.stats.tier_demotions.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get collections with tiering enabled
    fn get_tiered_collections(&self) -> Result<Vec<i32>, String> {
        // In production, query ruvector.tier_policies
        Ok(vec![])
    }

    /// Get tier policies for a collection
    fn get_tier_policies(&self, _collection_id: i32) -> Result<Vec<TierPolicy>, String> {
        // In production, query ruvector.tier_policies
        Ok(vec![])
    }

    /// Get tier candidates for promotion/demotion
    fn get_tier_candidates(
        &self,
        _collection_id: i32,
        _policies: &[TierPolicy],
    ) -> Result<Vec<TierCandidate>, String> {
        // In production:
        // - Query access counters
        // - Compare against thresholds
        // - Identify candidates

        Ok(vec![])
    }

    /// Promote a vector to a hotter tier
    fn promote_vector(&self, candidate: &TierCandidate) -> Result<(), String> {
        pgrx::debug1!(
            "Promoting vector {:?} from {} to {}",
            candidate.vector_tid,
            candidate.current_tier,
            candidate.target_tier
        );

        // In production:
        // 1. Decompress if needed
        // 2. Move to hot storage
        // 3. Update access counter tier
        // 4. Log promotion event

        Ok(())
    }

    /// Demote a vector to a colder tier
    fn demote_vector(&self, candidate: &TierCandidate) -> Result<(), String> {
        pgrx::debug1!(
            "Demoting vector {:?} from {} to {}",
            candidate.vector_tid,
            candidate.current_tier,
            candidate.target_tier
        );

        // In production:
        // 1. Apply compression (SQ8, PQ, etc.)
        // 2. Move to cold storage
        // 3. Update access counter tier
        // 4. Log demotion event

        Ok(())
    }

    /// Sleep for a duration but wake up early if shutdown is requested
    fn sleep_interruptible(&self, secs: u64) {
        let deadline = Instant::now() + Duration::from_secs(secs);

        while Instant::now() < deadline {
            if get_lifecycle_manager().is_shutdown_requested() {
                return;
            }

            // Sleep in small increments
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Get worker statistics
    pub fn stats(&self) -> MaintenanceStats {
        self.stats.snapshot()
    }

    /// Stop the worker
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

// ============================================================================
// Maintenance Worker Entry Point
// ============================================================================

/// Main background worker function for maintenance
#[pg_guard]
pub extern "C" fn ruvector_maintenance_worker_main(arg: pg_sys::Datum) {
    let worker_id = arg.value() as u64;

    pgrx::log!("RuVector maintenance worker {} starting", worker_id);

    let mut worker = MaintenanceWorker::new(worker_id);

    // Update lifecycle status
    get_lifecycle_manager().update_status(worker_id, WorkerStatus::Running);

    // Run main loop
    worker.run();

    // Update lifecycle status
    get_lifecycle_manager().update_status(worker_id, WorkerStatus::Stopped);

    pgrx::log!("RuVector maintenance worker {} stopped", worker_id);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current epoch time in seconds
fn current_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Calculate fragmentation for an index
pub fn calculate_fragmentation(index: &IndexInfo) -> f32 {
    // In production:
    // - Count deleted/obsolete tuples
    // - Measure graph connectivity (for HNSW)
    // - Check for unbalanced partitions

    // Mock: return provided value or default
    index.fragmentation
}

// ============================================================================
// SQL Functions
// ============================================================================

/// Get maintenance statistics
#[pg_extern]
pub fn ruvector_maintenance_stats() -> pgrx::JsonB {
    let config = get_maintenance_config();
    let stats = MaintenanceStats::default(); // In production, get from active worker

    pgrx::JsonB(serde_json::json!({
        "config": {
            "interval_secs": config.interval_secs,
            "max_indexes_per_cycle": config.max_indexes_per_cycle,
            "enable_tiering": config.enable_tiering,
            "enable_compaction": config.enable_compaction,
            "enable_stats": config.enable_stats,
            "compaction_threshold": config.compaction_threshold,
        },
        "stats": {
            "cycles_completed": stats.cycles_completed,
            "indexes_maintained": stats.indexes_maintained,
            "compactions_performed": stats.compactions_performed,
            "bytes_reclaimed": stats.bytes_reclaimed,
            "tier_promotions": stats.tier_promotions,
            "tier_demotions": stats.tier_demotions,
            "total_time_us": stats.total_time_us,
            "last_cycle_at": stats.last_cycle_at,
        }
    }))
}

/// Force a maintenance cycle
#[pg_extern]
pub fn ruvector_force_maintenance() -> bool {
    // In production, signal the maintenance worker to run a cycle
    pgrx::log!("Force maintenance requested");
    true
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maintenance_config_default() {
        let config = MaintenanceConfig::default();
        assert_eq!(config.interval_secs, 300);
        assert!(config.enable_tiering);
        assert!(config.enable_compaction);
        assert_eq!(config.compaction_threshold, 0.15);
    }

    #[test]
    fn test_maintenance_stats() {
        let stats = MaintenanceStatsAtomic::new();

        stats.cycles_completed.fetch_add(5, Ordering::Relaxed);
        stats.compactions_performed.fetch_add(3, Ordering::Relaxed);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.cycles_completed, 5);
        assert_eq!(snapshot.compactions_performed, 3);
    }

    #[test]
    fn test_tier_candidate() {
        let candidate = TierCandidate {
            vector_tid: (100, 5),
            current_tier: "hot".to_string(),
            target_tier: "cold".to_string(),
            needs_promotion: false,
            needs_demotion: true,
            access_count: 10,
            last_access: 1000000,
        };

        assert!(candidate.needs_demotion);
        assert!(!candidate.needs_promotion);
    }

    #[test]
    fn test_index_info() {
        let index = IndexInfo {
            name: "test_idx".to_string(),
            oid: 12345,
            relation_oid: 12344,
            index_type: "ruhnsw".to_string(),
            size_bytes: 1024 * 1024,
            tuple_count: 10000,
            last_vacuum: Some(1000000),
            last_analyze: Some(1000000),
            fragmentation: 0.05,
            collection_id: Some(1),
        };

        assert_eq!(index.index_type, "ruhnsw");
        assert_eq!(index.fragmentation, 0.05);
    }
}
