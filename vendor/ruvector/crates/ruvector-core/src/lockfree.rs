//! Lock-free data structures for high-concurrency operations
//!
//! This module provides lock-free implementations of common data structures
//! to minimize contention and improve scalability.
//!
//! Note: This module requires the `parallel` feature and is not available on WASM.

#![cfg(all(feature = "parallel", not(target_arch = "wasm32")))]

use crossbeam::queue::{ArrayQueue, SegQueue};
use crossbeam::utils::CachePadded;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// Lock-free counter with cache padding to prevent false sharing
#[repr(align(64))]
pub struct LockFreeCounter {
    value: CachePadded<AtomicU64>,
}

impl LockFreeCounter {
    pub fn new(initial: u64) -> Self {
        Self {
            value: CachePadded::new(AtomicU64::new(initial)),
        }
    }

    #[inline]
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn add(&self, delta: u64) -> u64 {
        self.value.fetch_add(delta, Ordering::Relaxed)
    }
}

/// Lock-free statistics collector
pub struct LockFreeStats {
    queries: CachePadded<AtomicU64>,
    inserts: CachePadded<AtomicU64>,
    deletes: CachePadded<AtomicU64>,
    total_latency_ns: CachePadded<AtomicU64>,
}

impl LockFreeStats {
    pub fn new() -> Self {
        Self {
            queries: CachePadded::new(AtomicU64::new(0)),
            inserts: CachePadded::new(AtomicU64::new(0)),
            deletes: CachePadded::new(AtomicU64::new(0)),
            total_latency_ns: CachePadded::new(AtomicU64::new(0)),
        }
    }

    #[inline]
    pub fn record_query(&self, latency_ns: u64) {
        self.queries.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_insert(&self) {
        self.inserts.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_delete(&self) {
        self.deletes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        let queries = self.queries.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ns.load(Ordering::Relaxed);

        StatsSnapshot {
            queries,
            inserts: self.inserts.load(Ordering::Relaxed),
            deletes: self.deletes.load(Ordering::Relaxed),
            avg_latency_ns: if queries > 0 {
                total_latency / queries
            } else {
                0
            },
        }
    }
}

impl Default for LockFreeStats {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    pub queries: u64,
    pub inserts: u64,
    pub deletes: u64,
    pub avg_latency_ns: u64,
}

/// Lock-free object pool for reducing allocations
pub struct ObjectPool<T> {
    queue: Arc<SegQueue<T>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    capacity: usize,
    allocated: AtomicUsize,
}

impl<T> ObjectPool<T> {
    pub fn new<F>(capacity: usize, factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            queue: Arc::new(SegQueue::new()),
            factory: Arc::new(factory),
            capacity,
            allocated: AtomicUsize::new(0),
        }
    }

    /// Get an object from the pool or create a new one
    pub fn acquire(&self) -> PooledObject<T> {
        let object = self.queue.pop().unwrap_or_else(|| {
            let current = self.allocated.fetch_add(1, Ordering::Relaxed);
            if current < self.capacity {
                (self.factory)()
            } else {
                self.allocated.fetch_sub(1, Ordering::Relaxed);
                // Wait for an object to be returned
                loop {
                    if let Some(obj) = self.queue.pop() {
                        break obj;
                    }
                    std::hint::spin_loop();
                }
            }
        });

        PooledObject {
            object: Some(object),
            pool: Arc::clone(&self.queue),
        }
    }
}

/// RAII wrapper for pooled objects
pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<SegQueue<T>>,
}

impl<T> PooledObject<T> {
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            self.pool.push(object);
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}

/// Lock-free ring buffer for work distribution
pub struct LockFreeWorkQueue<T> {
    queue: ArrayQueue<T>,
}

impl<T> LockFreeWorkQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: ArrayQueue::new(capacity),
        }
    }

    #[inline]
    pub fn try_push(&self, item: T) -> Result<(), T> {
        self.queue.push(item)
    }

    #[inline]
    pub fn try_pop(&self) -> Option<T> {
        self.queue.pop()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Atomic vector pool for lock-free vector operations (ADR-001)
///
/// Provides a pool of pre-allocated vectors that can be acquired and released
/// without locking, ideal for high-throughput batch operations.
pub struct AtomicVectorPool {
    /// Pool of available vectors
    pool: SegQueue<Vec<f32>>,
    /// Dimensions per vector
    dimensions: usize,
    /// Maximum pool size
    max_size: usize,
    /// Current pool size
    size: AtomicUsize,
    /// Total allocations
    total_allocations: AtomicU64,
    /// Pool hits (reused vectors)
    pool_hits: AtomicU64,
}

impl AtomicVectorPool {
    /// Create a new atomic vector pool
    pub fn new(dimensions: usize, initial_size: usize, max_size: usize) -> Self {
        let pool = SegQueue::new();

        // Pre-allocate vectors
        for _ in 0..initial_size {
            pool.push(vec![0.0; dimensions]);
        }

        Self {
            pool,
            dimensions,
            max_size,
            size: AtomicUsize::new(initial_size),
            total_allocations: AtomicU64::new(0),
            pool_hits: AtomicU64::new(0),
        }
    }

    /// Acquire a vector from the pool (or allocate new one)
    pub fn acquire(&self) -> PooledVector<'_> {
        self.total_allocations.fetch_add(1, Ordering::Relaxed);

        let vec = if let Some(mut v) = self.pool.pop() {
            self.pool_hits.fetch_add(1, Ordering::Relaxed);
            // Clear the vector for reuse
            v.fill(0.0);
            v
        } else {
            // Allocate new vector
            vec![0.0; self.dimensions]
        };

        PooledVector {
            vec: Some(vec),
            pool: self,
        }
    }

    /// Return a vector to the pool
    fn return_to_pool(&self, vec: Vec<f32>) {
        let current_size = self.size.load(Ordering::Relaxed);
        if current_size < self.max_size {
            self.pool.push(vec);
            self.size.fetch_add(1, Ordering::Relaxed);
        }
        // If pool is full, vector is dropped
    }

    /// Get pool statistics
    pub fn stats(&self) -> VectorPoolStats {
        let total = self.total_allocations.load(Ordering::Relaxed);
        let hits = self.pool_hits.load(Ordering::Relaxed);
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        VectorPoolStats {
            total_allocations: total,
            pool_hits: hits,
            hit_rate,
            current_size: self.size.load(Ordering::Relaxed),
            max_size: self.max_size,
        }
    }

    /// Get dimensions
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Statistics for the vector pool
#[derive(Debug, Clone)]
pub struct VectorPoolStats {
    pub total_allocations: u64,
    pub pool_hits: u64,
    pub hit_rate: f64,
    pub current_size: usize,
    pub max_size: usize,
}

/// RAII wrapper for pooled vectors
pub struct PooledVector<'a> {
    vec: Option<Vec<f32>>,
    pool: &'a AtomicVectorPool,
}

impl<'a> PooledVector<'a> {
    /// Get as slice
    pub fn as_slice(&self) -> &[f32] {
        self.vec.as_ref().unwrap()
    }

    /// Get as mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        self.vec.as_mut().unwrap()
    }

    /// Copy from source slice
    pub fn copy_from(&mut self, src: &[f32]) {
        let vec = self.vec.as_mut().unwrap();
        assert_eq!(vec.len(), src.len(), "Dimension mismatch");
        vec.copy_from_slice(src);
    }

    /// Detach the vector from the pool (it won't be returned)
    pub fn detach(mut self) -> Vec<f32> {
        self.vec.take().unwrap()
    }
}

impl<'a> Drop for PooledVector<'a> {
    fn drop(&mut self) {
        if let Some(vec) = self.vec.take() {
            self.pool.return_to_pool(vec);
        }
    }
}

impl<'a> std::ops::Deref for PooledVector<'a> {
    type Target = [f32];

    fn deref(&self) -> &[f32] {
        self.as_slice()
    }
}

impl<'a> std::ops::DerefMut for PooledVector<'a> {
    fn deref_mut(&mut self) -> &mut [f32] {
        self.as_mut_slice()
    }
}

/// Lock-free batch processor for parallel vector operations (ADR-001)
///
/// Distributes work across multiple workers without contention.
pub struct LockFreeBatchProcessor {
    /// Work queue for pending items
    work_queue: ArrayQueue<BatchItem>,
    /// Results queue
    results_queue: SegQueue<BatchResult>,
    /// Pending count
    pending: AtomicUsize,
    /// Completed count
    completed: AtomicUsize,
}

/// Item in the batch work queue
#[derive(Debug)]
pub struct BatchItem {
    pub id: u64,
    pub data: Vec<f32>,
}

/// Result from batch processing
pub struct BatchResult {
    pub id: u64,
    pub result: Vec<f32>,
}

impl LockFreeBatchProcessor {
    /// Create a new batch processor with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            work_queue: ArrayQueue::new(capacity),
            results_queue: SegQueue::new(),
            pending: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
        }
    }

    /// Submit a batch item for processing
    pub fn submit(&self, item: BatchItem) -> Result<(), BatchItem> {
        self.pending.fetch_add(1, Ordering::Relaxed);
        self.work_queue.push(item)
    }

    /// Try to get a work item (for workers)
    pub fn try_get_work(&self) -> Option<BatchItem> {
        self.work_queue.pop()
    }

    /// Submit a result (from workers)
    pub fn submit_result(&self, result: BatchResult) {
        self.completed.fetch_add(1, Ordering::Relaxed);
        self.results_queue.push(result);
    }

    /// Collect all available results
    pub fn collect_results(&self) -> Vec<BatchResult> {
        let mut results = Vec::new();
        while let Some(result) = self.results_queue.pop() {
            results.push(result);
        }
        results
    }

    /// Get pending count
    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::Relaxed)
    }

    /// Get completed count
    pub fn completed(&self) -> usize {
        self.completed.load(Ordering::Relaxed)
    }

    /// Check if all work is done
    pub fn is_done(&self) -> bool {
        self.pending() == self.completed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_lockfree_counter() {
        let counter = Arc::new(LockFreeCounter::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    counter_clone.increment();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.get(), 10000);
    }

    #[test]
    fn test_object_pool() {
        let pool = ObjectPool::new(4, || Vec::<u8>::with_capacity(1024));

        let mut obj1 = pool.acquire();
        obj1.push(1);
        assert_eq!(obj1.len(), 1);

        drop(obj1);

        let obj2 = pool.acquire();
        // Object should be reused (but cleared state is not guaranteed)
        assert!(obj2.capacity() >= 1024);
    }

    #[test]
    fn test_stats_collector() {
        let stats = LockFreeStats::new();

        stats.record_query(1000);
        stats.record_query(2000);
        stats.record_insert();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.queries, 2);
        assert_eq!(snapshot.inserts, 1);
        assert_eq!(snapshot.avg_latency_ns, 1500);
    }

    #[test]
    fn test_atomic_vector_pool() {
        let pool = AtomicVectorPool::new(4, 2, 10);

        // Acquire first vector
        let mut v1 = pool.acquire();
        v1.copy_from(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(v1.as_slice(), &[1.0, 2.0, 3.0, 4.0]);

        // Acquire second vector
        let mut v2 = pool.acquire();
        v2.copy_from(&[5.0, 6.0, 7.0, 8.0]);

        // Stats should show allocations
        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 2);
    }

    #[test]
    fn test_vector_pool_reuse() {
        let pool = AtomicVectorPool::new(3, 1, 5);

        // Acquire and release
        {
            let mut v = pool.acquire();
            v.copy_from(&[1.0, 2.0, 3.0]);
        } // v is returned to pool here

        // Acquire again - should be a pool hit
        let _v2 = pool.acquire();

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 2);
        assert!(stats.pool_hits >= 1, "Should have at least one pool hit");
    }

    #[test]
    fn test_batch_processor() {
        let processor = LockFreeBatchProcessor::new(10);

        // Submit work items
        processor
            .submit(BatchItem {
                id: 1,
                data: vec![1.0, 2.0],
            })
            .unwrap();
        processor
            .submit(BatchItem {
                id: 2,
                data: vec![3.0, 4.0],
            })
            .unwrap();

        assert_eq!(processor.pending(), 2);

        // Process work
        while let Some(item) = processor.try_get_work() {
            let result = BatchResult {
                id: item.id,
                result: item.data.iter().map(|x| x * 2.0).collect(),
            };
            processor.submit_result(result);
        }

        assert!(processor.is_done());
        assert_eq!(processor.completed(), 2);

        // Collect results
        let results = processor.collect_results();
        assert_eq!(results.len(), 2);
    }
}
