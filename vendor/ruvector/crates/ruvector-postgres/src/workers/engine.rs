//! Engine Worker - Main Coordination Worker
//!
//! The Engine Worker is the core RuVector instance that handles all vector
//! operations. It maintains in-memory indexes and processes queries/mutations
//! submitted via shared memory.
//!
//! # Responsibilities
//!
//! - Query routing and processing
//! - Load balancing across indexes
//! - In-memory index management
//! - Persistence coordination
//!
//! # Architecture
//!
//! ```text
//! +------------------------------------------------------------------+
//! |                      ENGINE WORKER                               |
//! +------------------------------------------------------------------+
//! |                                                                  |
//! |  +------------------+     +-------------------+                  |
//! |  | Work Queue       | --> | Query Router      |                  |
//! |  | (from backends)  |     | (load balancing)  |                  |
//! |  +------------------+     +-------------------+                  |
//! |                                   |                              |
//! |           +-------------------+---+---+-------------------+      |
//! |           v                   v       v                   v      |
//! |  +---------------+   +---------------+   +---------------+       |
//! |  | Collection 1  |   | Collection 2  |   | Collection N  |       |
//! |  | HNSW Index    |   | IVFFlat Index |   | HNSW Index    |       |
//! |  +---------------+   +---------------+   +---------------+       |
//! |                                                                  |
//! |  +------------------+     +-------------------+                  |
//! |  | Result Queue     | <-- | Result Aggregator |                  |
//! |  | (to backends)    |     |                   |                  |
//! |  +------------------+     +-------------------+                  |
//! |                                                                  |
//! +------------------------------------------------------------------+
//! ```

use parking_lot::RwLock;
use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::ipc::{
    get_shared_memory, BuildIndexRequest, DeleteRequest, InsertRequest, Operation, ResultStatus,
    SearchRequest, UpdateIndexRequest, WorkItem, WorkResult,
};
use super::lifecycle::{get_lifecycle_manager, WorkerStatus};

// Re-export for external use
pub use super::ipc::SearchRequest as SearchReq;

// ============================================================================
// Engine Worker Configuration
// ============================================================================

/// Engine worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineWorkerConfig {
    /// Maximum memory for indexes (bytes)
    pub max_index_memory: usize,
    /// Maximum concurrent search operations
    pub max_concurrent_searches: usize,
    /// Work queue depth
    pub work_queue_size: usize,
    /// Shutdown timeout (seconds)
    pub shutdown_timeout_secs: u64,
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Prefetch distance for search
    pub prefetch_distance: usize,
    /// Batch size for insert operations
    pub insert_batch_size: usize,
    /// Enable query caching
    pub enable_query_cache: bool,
    /// Query cache size (number of entries)
    pub query_cache_size: usize,
    /// Query cache TTL (seconds)
    pub query_cache_ttl_secs: u64,
}

impl Default for EngineWorkerConfig {
    fn default() -> Self {
        Self {
            max_index_memory: 4 * 1024 * 1024 * 1024, // 4GB
            max_concurrent_searches: 64,
            work_queue_size: 1024,
            shutdown_timeout_secs: 30,
            enable_simd: true,
            prefetch_distance: 4,
            insert_batch_size: 1000,
            enable_query_cache: true,
            query_cache_size: 10000,
            query_cache_ttl_secs: 60,
        }
    }
}

// Global engine configuration
static ENGINE_CONFIG: OnceLock<RwLock<EngineWorkerConfig>> = OnceLock::new();

/// Get engine configuration
pub fn get_engine_config() -> EngineWorkerConfig {
    ENGINE_CONFIG
        .get_or_init(|| RwLock::new(EngineWorkerConfig::default()))
        .read()
        .clone()
}

/// Set engine configuration
pub fn set_engine_config(config: EngineWorkerConfig) {
    if let Some(cfg) = ENGINE_CONFIG.get() {
        *cfg.write() = config;
    }
}

// ============================================================================
// Search Result
// ============================================================================

/// Search result returned from engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result vector IDs
    pub ids: Vec<i64>,
    /// Distances to query
    pub distances: Vec<f32>,
    /// Search time in microseconds
    pub search_time_us: u64,
    /// Number of vectors scanned
    pub vectors_scanned: u64,
    /// Cache hit
    pub cache_hit: bool,
}

impl SearchResult {
    /// Create empty result
    pub fn empty() -> Self {
        Self {
            ids: Vec::new(),
            distances: Vec::new(),
            search_time_us: 0,
            vectors_scanned: 0,
            cache_hit: false,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        bincode::deserialize(data).ok()
    }
}

// ============================================================================
// Collection Index State
// ============================================================================

/// In-memory index for a collection
#[derive(Debug)]
pub struct CollectionIndex {
    /// Collection ID
    pub collection_id: i32,
    /// Index type (hnsw, ivfflat)
    pub index_type: String,
    /// Number of vectors
    pub vector_count: u64,
    /// Dimensions
    pub dimensions: usize,
    /// Memory usage in bytes
    pub memory_bytes: usize,
    /// Last access time
    pub last_access: u64,
    /// Query count
    pub query_count: AtomicU64,
    /// Is index loaded
    pub loaded: AtomicBool,
    // In production, this would contain the actual index structures
    // pub hnsw: Option<HnswIndex>,
    // pub ivfflat: Option<IvfFlatIndex>,
}

impl CollectionIndex {
    /// Create a new collection index
    pub fn new(collection_id: i32, index_type: &str, dimensions: usize) -> Self {
        Self {
            collection_id,
            index_type: index_type.to_string(),
            vector_count: 0,
            dimensions,
            memory_bytes: 0,
            last_access: current_epoch_secs(),
            query_count: AtomicU64::new(0),
            loaded: AtomicBool::new(false),
        }
    }

    /// Record a query
    pub fn record_query(&self) {
        self.query_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get query count
    pub fn get_query_count(&self) -> u64 {
        self.query_count.load(Ordering::Relaxed)
    }

    /// Check if index is loaded
    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }
}

// ============================================================================
// Query Cache
// ============================================================================

/// Query cache entry
#[derive(Clone)]
struct CacheEntry {
    result: SearchResult,
    created_at: u64,
}

/// LRU query cache
struct QueryCache {
    entries: RwLock<HashMap<u64, CacheEntry>>,
    max_size: usize,
    ttl_secs: u64,
}

impl QueryCache {
    fn new(max_size: usize, ttl_secs: u64) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_size,
            ttl_secs,
        }
    }

    fn get(&self, key: u64) -> Option<SearchResult> {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(&key) {
            let now = current_epoch_secs();
            if now - entry.created_at < self.ttl_secs {
                return Some(entry.result.clone());
            }
        }
        None
    }

    fn put(&self, key: u64, result: SearchResult) {
        let mut entries = self.entries.write();

        // Evict old entries if at capacity
        if entries.len() >= self.max_size {
            let now = current_epoch_secs();
            entries.retain(|_, v| now - v.created_at < self.ttl_secs);

            // If still too large, remove oldest
            if entries.len() >= self.max_size {
                if let Some(oldest_key) = entries
                    .iter()
                    .min_by_key(|(_, v)| v.created_at)
                    .map(|(k, _)| *k)
                {
                    entries.remove(&oldest_key);
                }
            }
        }

        entries.insert(
            key,
            CacheEntry {
                result,
                created_at: current_epoch_secs(),
            },
        );
    }

    fn clear(&self) {
        self.entries.write().clear();
    }
}

// ============================================================================
// RuVector Engine
// ============================================================================

/// Main RuVector engine instance
pub struct RuVectorEngine {
    /// Configuration
    config: EngineWorkerConfig,
    /// Collection indexes
    indexes: RwLock<HashMap<i32, CollectionIndex>>,
    /// Query cache
    cache: QueryCache,
    /// Statistics
    stats: EngineStats,
    /// Running flag
    running: AtomicBool,
}

impl RuVectorEngine {
    /// Create a new engine instance
    pub fn new(config: EngineWorkerConfig) -> Self {
        Self {
            cache: QueryCache::new(config.query_cache_size, config.query_cache_ttl_secs),
            config,
            indexes: RwLock::new(HashMap::new()),
            stats: EngineStats::new(),
            running: AtomicBool::new(false),
        }
    }

    /// Start the engine
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Stop the engine
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if engine is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Load indexes from storage
    pub fn load_from_storage(&mut self) -> Result<(), String> {
        pgrx::log!("Loading indexes from storage");

        // In production, query pg_class for RuVector indexes and load them
        // For now, return OK
        Ok(())
    }

    /// Persist indexes to storage
    pub fn persist_to_storage(&self) {
        pgrx::log!("Persisting indexes to storage");

        // In production, flush dirty indexes to disk
    }

    /// Process a search request
    pub fn search(&self, request: &SearchRequest) -> Result<SearchResult, String> {
        let start = Instant::now();

        // Check cache
        if self.config.enable_query_cache {
            let cache_key = compute_cache_key(request);
            if let Some(mut result) = self.cache.get(cache_key) {
                result.cache_hit = true;
                self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(result);
            }
            self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
        }

        // Get index
        let indexes = self.indexes.read();
        let index = indexes
            .get(&request.collection_id)
            .ok_or_else(|| format!("Collection {} not found", request.collection_id))?;

        if !index.is_loaded() {
            return Err(format!(
                "Index for collection {} not loaded",
                request.collection_id
            ));
        }

        index.record_query();

        // Perform search (mock implementation)
        let result = self.perform_search(index, request)?;

        // Update stats
        let elapsed = start.elapsed();
        self.stats.total_searches.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_search_time_us
            .fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);

        // Cache result
        if self.config.enable_query_cache {
            let cache_key = compute_cache_key(request);
            self.cache.put(cache_key, result.clone());
        }

        Ok(result)
    }

    /// Perform the actual search (implementation would use real index)
    fn perform_search(
        &self,
        index: &CollectionIndex,
        request: &SearchRequest,
    ) -> Result<SearchResult, String> {
        // In production, this would:
        // 1. Validate query dimensions match index
        // 2. Apply ef_search parameter
        // 3. Use SIMD-optimized distance computation
        // 4. Apply filter if present
        // 5. Use GNN routing if enabled

        // Mock result
        Ok(SearchResult {
            ids: (0..request.k as i64).collect(),
            distances: (0..request.k).map(|i| i as f32 * 0.1).collect(),
            search_time_us: 100,
            vectors_scanned: 1000,
            cache_hit: false,
        })
    }

    /// Process an insert request
    pub fn insert(&mut self, request: &InsertRequest) -> Result<(), String> {
        let start = Instant::now();

        // Get or create index
        let mut indexes = self.indexes.write();
        let index = indexes.entry(request.collection_id).or_insert_with(|| {
            // Create new index (in production, would load from catalog)
            CollectionIndex::new(request.collection_id, "hnsw", 0)
        });

        // In production, insert vectors into the index
        let count = request.vectors.len() as u64;

        // Update stats
        self.stats.total_inserts.fetch_add(count, Ordering::Relaxed);
        self.stats
            .total_insert_time_us
            .fetch_add(start.elapsed().as_micros() as u64, Ordering::Relaxed);

        // Invalidate cache for this collection
        self.cache.clear();

        Ok(())
    }

    /// Process a delete request
    pub fn delete(&mut self, request: &DeleteRequest) -> Result<(), String> {
        let start = Instant::now();

        let indexes = self.indexes.read();
        let _index = indexes
            .get(&request.collection_id)
            .ok_or_else(|| format!("Collection {} not found", request.collection_id))?;

        // In production, mark vectors as deleted in the index
        let count = request.ids.len() as u64;

        // Update stats
        self.stats.total_deletes.fetch_add(count, Ordering::Relaxed);
        self.stats
            .total_delete_time_us
            .fetch_add(start.elapsed().as_micros() as u64, Ordering::Relaxed);

        // Invalidate cache for this collection
        self.cache.clear();

        Ok(())
    }

    /// Process a build index request
    pub fn build_index(&mut self, request: &BuildIndexRequest) -> Result<(), String> {
        let start = Instant::now();

        pgrx::log!(
            "Building {} index for collection {}",
            request.index_type,
            request.collection_id
        );

        // In production:
        // 1. Parse index parameters
        // 2. Load vectors from table
        // 3. Build the index structure
        // 4. Register in memory

        let dimensions = 128; // Would be determined from vectors
        let index = CollectionIndex::new(request.collection_id, &request.index_type, dimensions);

        let mut indexes = self.indexes.write();
        indexes.insert(request.collection_id, index);

        self.stats.indexes_built.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_build_time_us
            .fetch_add(start.elapsed().as_micros() as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Process an update index request
    pub fn update_index(&mut self, request: &UpdateIndexRequest) -> Result<(), String> {
        let start = Instant::now();

        let mut indexes = self.indexes.write();
        let _index = indexes
            .get_mut(&request.collection_id)
            .ok_or_else(|| format!("Collection {} not found", request.collection_id))?;

        // In production, incrementally add vectors to the index
        let count = request.vectors.len() as u64;

        self.stats.total_updates.fetch_add(count, Ordering::Relaxed);
        self.stats
            .total_update_time_us
            .fetch_add(start.elapsed().as_micros() as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Get engine statistics
    pub fn stats(&self) -> EngineStatsSnapshot {
        self.stats.snapshot()
    }

    /// Get index count
    pub fn index_count(&self) -> usize {
        self.indexes.read().len()
    }

    /// Get total memory usage
    pub fn memory_usage(&self) -> usize {
        self.indexes
            .read()
            .values()
            .map(|idx| idx.memory_bytes)
            .sum()
    }
}

/// Engine statistics
pub struct EngineStats {
    pub total_searches: AtomicU64,
    pub total_search_time_us: AtomicU64,
    pub total_inserts: AtomicU64,
    pub total_insert_time_us: AtomicU64,
    pub total_deletes: AtomicU64,
    pub total_delete_time_us: AtomicU64,
    pub total_updates: AtomicU64,
    pub total_update_time_us: AtomicU64,
    pub indexes_built: AtomicU64,
    pub total_build_time_us: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
}

impl EngineStats {
    pub fn new() -> Self {
        Self {
            total_searches: AtomicU64::new(0),
            total_search_time_us: AtomicU64::new(0),
            total_inserts: AtomicU64::new(0),
            total_insert_time_us: AtomicU64::new(0),
            total_deletes: AtomicU64::new(0),
            total_delete_time_us: AtomicU64::new(0),
            total_updates: AtomicU64::new(0),
            total_update_time_us: AtomicU64::new(0),
            indexes_built: AtomicU64::new(0),
            total_build_time_us: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> EngineStatsSnapshot {
        let searches = self.total_searches.load(Ordering::Relaxed);
        let search_time = self.total_search_time_us.load(Ordering::Relaxed);
        let inserts = self.total_inserts.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);

        EngineStatsSnapshot {
            total_searches: searches,
            total_search_time_us: search_time,
            avg_search_time_us: if searches > 0 {
                search_time / searches
            } else {
                0
            },
            total_inserts: inserts,
            total_insert_time_us: self.total_insert_time_us.load(Ordering::Relaxed),
            total_deletes: self.total_deletes.load(Ordering::Relaxed),
            total_updates: self.total_updates.load(Ordering::Relaxed),
            indexes_built: self.indexes_built.load(Ordering::Relaxed),
            cache_hits,
            cache_misses,
            cache_hit_rate: if cache_hits + cache_misses > 0 {
                cache_hits as f64 / (cache_hits + cache_misses) as f64
            } else {
                0.0
            },
        }
    }
}

impl Default for EngineStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Engine statistics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatsSnapshot {
    pub total_searches: u64,
    pub total_search_time_us: u64,
    pub avg_search_time_us: u64,
    pub total_inserts: u64,
    pub total_insert_time_us: u64,
    pub total_deletes: u64,
    pub total_updates: u64,
    pub indexes_built: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
}

// ============================================================================
// Engine Worker
// ============================================================================

/// Engine worker wrapper
pub struct EngineWorker {
    /// Worker ID
    id: u64,
    /// Engine instance
    engine: RuVectorEngine,
}

impl EngineWorker {
    /// Create a new engine worker
    pub fn new(id: u64) -> Self {
        let config = get_engine_config();
        Self {
            id,
            engine: RuVectorEngine::new(config),
        }
    }

    /// Run the worker main loop
    pub fn run(&mut self) {
        pgrx::log!("Engine worker {} starting", self.id);

        self.engine.start();

        // Load persisted indexes
        if let Err(e) = self.engine.load_from_storage() {
            pgrx::warning!("Failed to load indexes: {}", e);
        }

        // Main loop
        while self.engine.is_running() {
            // Check for shutdown
            if get_lifecycle_manager().is_shutdown_requested() {
                break;
            }

            // Process work queue
            self.process_work_queue();

            // Yield
            std::thread::sleep(Duration::from_millis(1));
        }

        // Graceful shutdown
        self.engine.persist_to_storage();
        self.engine.stop();

        pgrx::log!("Engine worker {} stopped", self.id);
    }

    /// Process items from the work queue
    fn process_work_queue(&mut self) {
        let shmem = get_shared_memory();

        while let Some(work_item) = shmem.work_queue.try_pop() {
            // Check if cancelled
            if shmem.is_cancelled(work_item.request_id) {
                continue;
            }

            // Check deadline
            if work_item.deadline_ms > 0 && current_epoch_ms() > work_item.deadline_ms {
                let result = WorkResult {
                    request_id: work_item.request_id,
                    status: ResultStatus::Timeout,
                    data: Vec::new(),
                    processing_time_us: 0,
                };
                shmem.result_queue.push(result);
                continue;
            }

            // Process the operation
            let start = Instant::now();
            let result = self.process_operation(&work_item);
            let processing_time_us = start.elapsed().as_micros() as u64;

            // Send result
            let work_result = match result {
                Ok(data) => {
                    shmem
                        .stats
                        .record_success(processing_time_us, data.len() as u64);
                    WorkResult {
                        request_id: work_item.request_id,
                        status: ResultStatus::Success,
                        data,
                        processing_time_us,
                    }
                }
                Err(e) => {
                    shmem.stats.record_failure();
                    WorkResult {
                        request_id: work_item.request_id,
                        status: ResultStatus::Error,
                        data: e.into_bytes(),
                        processing_time_us,
                    }
                }
            };

            shmem.result_queue.push(work_result);
        }
    }

    /// Process a single operation
    fn process_operation(&mut self, work_item: &WorkItem) -> Result<Vec<u8>, String> {
        match &work_item.operation {
            Operation::Search(req) => {
                let result = self.engine.search(req)?;
                Ok(result.to_bytes())
            }
            Operation::Insert(req) => {
                self.engine.insert(req)?;
                Ok(Vec::new())
            }
            Operation::Delete(req) => {
                self.engine.delete(req)?;
                Ok(Vec::new())
            }
            Operation::BuildIndex(req) => {
                self.engine.build_index(req)?;
                Ok(Vec::new())
            }
            Operation::UpdateIndex(req) => {
                self.engine.update_index(req)?;
                Ok(Vec::new())
            }
            Operation::LargePayloadRef(payload_ref) => {
                // Read from shared segment and decode operation
                let shmem = get_shared_memory();
                let data = shmem
                    .large_payload_segment
                    .read(payload_ref.offset as usize, payload_ref.length as usize)?;

                let operation: Operation = bincode::deserialize(&data)
                    .map_err(|e| format!("Failed to decode operation: {}", e))?;

                // Recursive call with decoded operation
                let decoded_item = WorkItem {
                    operation,
                    ..work_item.clone()
                };
                self.process_operation(&decoded_item)
            }
            Operation::Ping => Ok(b"pong".to_vec()),
        }
    }
}

// ============================================================================
// Engine Worker Entry Point
// ============================================================================

/// Main background worker function for engine
#[pg_guard]
pub extern "C" fn ruvector_engine_worker_main(arg: pg_sys::Datum) {
    let worker_id = arg.value() as u64;

    pgrx::log!("RuVector engine worker {} starting", worker_id);

    let mut worker = EngineWorker::new(worker_id);

    // Update lifecycle status
    get_lifecycle_manager().update_status(worker_id, WorkerStatus::Running);

    // Run main loop
    worker.run();

    // Update lifecycle status
    get_lifecycle_manager().update_status(worker_id, WorkerStatus::Stopped);

    pgrx::log!("RuVector engine worker {} stopped", worker_id);
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

/// Get current epoch time in milliseconds
fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Compute cache key for a search request
fn compute_cache_key(request: &SearchRequest) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    request.collection_id.hash(&mut hasher);
    request.k.hash(&mut hasher);
    request.ef_search.hash(&mut hasher);
    request.use_gnn.hash(&mut hasher);
    request.filter.hash(&mut hasher);

    // Hash query vector
    for &v in &request.query {
        v.to_bits().hash(&mut hasher);
    }

    hasher.finish()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_config_default() {
        let config = EngineWorkerConfig::default();
        assert_eq!(config.max_concurrent_searches, 64);
        assert!(config.enable_simd);
        assert!(config.enable_query_cache);
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            ids: vec![1, 2, 3],
            distances: vec![0.1, 0.2, 0.3],
            search_time_us: 100,
            vectors_scanned: 1000,
            cache_hit: false,
        };

        let bytes = result.to_bytes();
        let decoded = SearchResult::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.ids, result.ids);
        assert_eq!(decoded.distances, result.distances);
    }

    #[test]
    fn test_collection_index() {
        let index = CollectionIndex::new(1, "hnsw", 128);

        assert_eq!(index.collection_id, 1);
        assert_eq!(index.index_type, "hnsw");
        assert!(!index.is_loaded());

        index.record_query();
        assert_eq!(index.get_query_count(), 1);
    }

    #[test]
    fn test_query_cache() {
        let cache = QueryCache::new(10, 60);

        let result = SearchResult {
            ids: vec![1],
            distances: vec![0.1],
            search_time_us: 100,
            vectors_scanned: 10,
            cache_hit: false,
        };

        cache.put(123, result.clone());

        let cached = cache.get(123).unwrap();
        assert_eq!(cached.ids, result.ids);

        assert!(cache.get(456).is_none());
    }

    #[test]
    fn test_engine_basic() {
        let config = EngineWorkerConfig::default();
        let mut engine = RuVectorEngine::new(config);

        engine.start();
        assert!(engine.is_running());

        engine.stop();
        assert!(!engine.is_running());
    }

    #[test]
    fn test_cache_key_computation() {
        let req1 = SearchRequest {
            collection_id: 1,
            query: vec![1.0, 2.0, 3.0],
            k: 10,
            ef_search: Some(50),
            filter: None,
            use_gnn: false,
        };

        let req2 = SearchRequest {
            collection_id: 1,
            query: vec![1.0, 2.0, 3.0],
            k: 10,
            ef_search: Some(50),
            filter: None,
            use_gnn: false,
        };

        let req3 = SearchRequest {
            collection_id: 2, // Different collection
            query: vec![1.0, 2.0, 3.0],
            k: 10,
            ef_search: Some(50),
            filter: None,
            use_gnn: false,
        };

        assert_eq!(compute_cache_key(&req1), compute_cache_key(&req2));
        assert_ne!(compute_cache_key(&req1), compute_cache_key(&req3));
    }
}
