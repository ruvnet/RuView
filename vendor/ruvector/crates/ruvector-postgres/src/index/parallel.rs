//! Parallel query execution for vector indexes
//!
//! Implements PostgreSQL parallel query support for HNSW and IVFFlat indexes.
//! Enables multi-worker parallel scans with result merging for k-NN queries.

use pgrx::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

use parking_lot::RwLock;

use super::hnsw::{HnswIndex, NodeId};
use crate::distance::DistanceMetric;

// ============================================================================
// Parallel Scan State
// ============================================================================

/// Shared state for parallel HNSW scan
///
/// This structure is allocated in shared memory and accessed by all parallel workers.
#[repr(C)]
pub struct RuHnswSharedState {
    /// Total number of parallel workers
    pub num_workers: u32,
    /// Next list/partition to scan
    pub next_partition: AtomicU32,
    /// Total partitions to scan
    pub total_partitions: u32,
    /// Query vector dimensions
    pub dimensions: u32,
    /// Number of nearest neighbors to find
    pub k: usize,
    /// ef_search parameter
    pub ef_search: usize,
    /// Distance metric
    pub metric: DistanceMetric,
    /// Completed workers count
    pub completed_workers: AtomicU32,
    /// Total results found across all workers
    pub total_results: AtomicUsize,
}

impl RuHnswSharedState {
    /// Create new shared state for parallel scan
    pub fn new(
        num_workers: u32,
        total_partitions: u32,
        dimensions: u32,
        k: usize,
        ef_search: usize,
        metric: DistanceMetric,
    ) -> Self {
        Self {
            num_workers,
            next_partition: AtomicU32::new(0),
            total_partitions,
            dimensions,
            k,
            ef_search,
            metric,
            completed_workers: AtomicU32::new(0),
            total_results: AtomicUsize::new(0),
        }
    }

    /// Get next partition to scan (work-stealing)
    pub fn get_next_partition(&self) -> Option<u32> {
        let partition = self.next_partition.fetch_add(1, AtomicOrdering::SeqCst);
        if partition < self.total_partitions {
            Some(partition)
        } else {
            None
        }
    }

    /// Mark worker as completed
    pub fn mark_completed(&self) {
        self.completed_workers.fetch_add(1, AtomicOrdering::SeqCst);
    }

    /// Check if all workers completed
    pub fn all_completed(&self) -> bool {
        self.completed_workers.load(AtomicOrdering::SeqCst) >= self.num_workers
    }

    /// Add results count
    pub fn add_results(&self, count: usize) {
        self.total_results.fetch_add(count, AtomicOrdering::SeqCst);
    }
}

/// Parallel scan descriptor for worker
pub struct RuHnswParallelScanDesc {
    /// Shared state across all workers
    pub shared: Arc<RwLock<RuHnswSharedState>>,
    /// Worker ID
    pub worker_id: u32,
    /// Local results buffer
    pub local_results: Vec<(f32, ItemPointer)>,
    /// Query vector (copied per worker)
    pub query: Vec<f32>,
}

impl RuHnswParallelScanDesc {
    /// Create new parallel scan descriptor
    pub fn new(
        shared: Arc<RwLock<RuHnswSharedState>>,
        worker_id: u32,
        query: Vec<f32>,
    ) -> Self {
        Self {
            shared,
            worker_id,
            local_results: Vec::new(),
            query,
        }
    }

    /// Execute parallel scan for this worker
    pub fn execute_scan(&mut self, index: &HnswIndex) {
        // Get partitions using work-stealing
        while let Some(partition_id) = {
            let shared = self.shared.read();
            shared.get_next_partition()
        } {
            // Scan this partition
            let partition_results = self.scan_partition(index, partition_id);
            self.local_results.extend(partition_results);
        }

        // Sort local results by distance
        self.local_results.sort_by(|a, b| {
            a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal)
        });

        // Keep only top k locally
        let shared = self.shared.read();
        let k = shared.k;
        drop(shared);

        if self.local_results.len() > k {
            self.local_results.truncate(k);
        }

        // Update shared state
        let shared = self.shared.read();
        shared.add_results(self.local_results.len());
        shared.mark_completed();
    }

    /// Scan a single partition
    fn scan_partition(
        &self,
        index: &HnswIndex,
        partition_id: u32,
    ) -> Vec<(f32, ItemPointer)> {
        let shared = self.shared.read();
        let k = shared.k;
        let ef_search = shared.ef_search;
        drop(shared);

        // Get partition bounds
        let total_nodes = index.len();
        let shared = self.shared.read();
        let partitions = shared.total_partitions as usize;
        drop(shared);

        let partition_size = (total_nodes + partitions - 1) / partitions;
        let start_idx = partition_id as usize * partition_size;
        let end_idx = ((partition_id as usize + 1) * partition_size).min(total_nodes);

        if start_idx >= total_nodes {
            return Vec::new();
        }

        // Search within partition
        // Note: This is a simplified partition-based approach
        // In production, you'd use graph partitioning or other methods
        let results = index.search(&self.query, k, Some(ef_search));

        // Convert results to ItemPointer format
        results
            .into_iter()
            .map(|(node_id, distance)| {
                // In real implementation, map node_id to ItemPointer (TID)
                let item_pointer = create_item_pointer(node_id);
                (distance, item_pointer)
            })
            .collect()
    }
}

/// PostgreSQL ItemPointer (tuple ID)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ItemPointer {
    pub block_number: u32,
    pub offset_number: u16,
}

impl ItemPointer {
    pub fn new(block_number: u32, offset_number: u16) -> Self {
        Self {
            block_number,
            offset_number,
        }
    }
}

/// Create ItemPointer from NodeId (simplified mapping)
fn create_item_pointer(node_id: NodeId) -> ItemPointer {
    // In production, maintain a node_id -> TID mapping
    let block = (node_id / 8191) as u32; // Max tuples per page
    let offset = (node_id % 8191) as u16 + 1;
    ItemPointer::new(block, offset)
}

// ============================================================================
// Parallel Worker Estimation
// ============================================================================

/// Estimate optimal number of parallel workers for HNSW index
///
/// Based on:
/// - Index size (number of pages)
/// - Available parallel workers
/// - Query complexity (k, ef_search)
///
/// # Arguments
/// * `index_pages` - Number of pages in the index
/// * `index_tuples` - Number of tuples (vectors) in the index
/// * `k` - Number of nearest neighbors to find
/// * `ef_search` - HNSW search parameter
///
/// # Returns
/// Recommended number of parallel workers (0 = no parallelism)
pub fn ruhnsw_estimate_parallel_workers(
    index_pages: i32,
    index_tuples: i64,
    k: i32,
    ef_search: i32,
) -> i32 {
    // Don't parallelize small indexes
    if index_pages < 100 || index_tuples < 10000 {
        return 0;
    }

    // Get max parallel workers from GUC
    let max_workers = get_max_parallel_workers();

    // Estimate based on index size
    // 1 worker per 1000 pages, up to max
    let workers_by_size = (index_pages / 1000).min(max_workers);

    // Adjust based on query complexity
    let complexity_factor = if ef_search > 100 || k > 100 {
        2.0 // More complex queries benefit more from parallelism
    } else if ef_search > 50 || k > 50 {
        1.5
    } else {
        1.0
    };

    let recommended = ((workers_by_size as f32 * complexity_factor) as i32)
        .min(max_workers)
        .max(0);

    recommended
}

/// Get max parallel workers from PostgreSQL GUC
fn get_max_parallel_workers() -> i32 {
    // Query max_parallel_workers_per_gather GUC
    // In production, use: current_setting('max_parallel_workers_per_gather')::int
    // For now, return a reasonable default
    4
}

/// Estimate number of partitions for parallel scan
///
/// More partitions allow better work distribution but increase overhead.
pub fn estimate_partitions(num_workers: i32, total_tuples: i64) -> u32 {
    // Use 2-4x more partitions than workers for better load balancing
    let base_partitions = num_workers * 3;

    // Adjust based on total tuples
    let tuples_per_partition = 10000;
    let partitions_by_size = (total_tuples / tuples_per_partition) as i32;

    base_partitions.min(partitions_by_size).max(1) as u32
}

// ============================================================================
// Parallel Result Merging
// ============================================================================

/// Neighbor entry for k-NN result merging
#[derive(Debug, Clone, Copy)]
pub struct KnnNeighbor {
    pub distance: f32,
    pub item_pointer: ItemPointer,
}

impl PartialEq for KnnNeighbor {
    fn eq(&self, other: &Self) -> bool {
        self.item_pointer == other.item_pointer
    }
}

impl Eq for KnnNeighbor {}

impl PartialOrd for KnnNeighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KnnNeighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for max-heap (we want smallest distances)
        other.distance.partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Merge k-NN results from multiple parallel workers
///
/// Uses a max-heap to efficiently find the top-k results across all workers.
///
/// # Arguments
/// * `worker_results` - Results from each worker (already sorted by distance)
/// * `k` - Number of nearest neighbors to return
///
/// # Returns
/// Top k results sorted by distance (ascending)
pub fn merge_knn_results(
    worker_results: &[Vec<(f32, ItemPointer)>],
    k: usize,
) -> Vec<(f32, ItemPointer)> {
    if worker_results.is_empty() {
        return Vec::new();
    }

    // Use max-heap to track top k results
    let mut heap: BinaryHeap<KnnNeighbor> = BinaryHeap::new();

    // Merge results from all workers
    for results in worker_results {
        for &(distance, item_pointer) in results {
            let neighbor = KnnNeighbor {
                distance,
                item_pointer,
            };

            if heap.len() < k {
                heap.push(neighbor);
            } else if let Some(worst) = heap.peek() {
                if neighbor.distance < worst.distance {
                    heap.pop();
                    heap.push(neighbor);
                }
            }
        }
    }

    // Convert heap to sorted vector
    let mut results: Vec<(f32, ItemPointer)> = heap
        .into_iter()
        .map(|n| (n.distance, n.item_pointer))
        .collect();

    // Sort by distance ascending
    results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

    results
}

/// Parallel merge using tournament tree for large result sets
///
/// More efficient than heap-based merge for many workers.
pub fn merge_knn_results_tournament(
    worker_results: &[Vec<(f32, ItemPointer)>],
    k: usize,
) -> Vec<(f32, ItemPointer)> {
    if worker_results.is_empty() {
        return Vec::new();
    }

    if worker_results.len() == 1 {
        return worker_results[0].iter().take(k).copied().collect();
    }

    // Initialize cursors for each worker's results
    let mut cursors: Vec<usize> = vec![0; worker_results.len()];
    let mut merged = Vec::with_capacity(k);

    // K-way merge
    for _ in 0..k {
        let mut best_worker = None;
        let mut best_distance = f32::MAX;

        // Find worker with smallest next distance
        for (worker_id, cursor) in cursors.iter_mut().enumerate() {
            if *cursor < worker_results[worker_id].len() {
                let (distance, _) = worker_results[worker_id][*cursor];
                if distance < best_distance {
                    best_distance = distance;
                    best_worker = Some(worker_id);
                }
            }
        }

        // Add best result and advance cursor
        if let Some(worker_id) = best_worker {
            let cursor = &mut cursors[worker_id];
            merged.push(worker_results[worker_id][*cursor]);
            *cursor += 1;
        } else {
            break; // No more results
        }
    }

    merged
}

// ============================================================================
// Parallel Scan Coordinator
// ============================================================================

/// Coordinator for parallel k-NN scan
pub struct ParallelScanCoordinator {
    /// Shared state
    pub shared_state: Arc<RwLock<RuHnswSharedState>>,
    /// Worker results
    pub worker_results: Vec<Vec<(f32, ItemPointer)>>,
}

impl ParallelScanCoordinator {
    /// Create new parallel scan coordinator
    pub fn new(
        num_workers: u32,
        total_partitions: u32,
        dimensions: u32,
        k: usize,
        ef_search: usize,
        metric: DistanceMetric,
    ) -> Self {
        let shared_state = Arc::new(RwLock::new(RuHnswSharedState::new(
            num_workers,
            total_partitions,
            dimensions,
            k,
            ef_search,
            metric,
        )));

        Self {
            shared_state,
            worker_results: Vec::with_capacity(num_workers as usize),
        }
    }

    /// Spawn parallel workers and collect results
    pub fn execute_parallel_scan(
        &mut self,
        index: &HnswIndex,
        query: Vec<f32>,
    ) -> Vec<(f32, ItemPointer)> {
        let num_workers = {
            let shared = self.shared_state.read();
            shared.num_workers
        };

        // In production, spawn actual PostgreSQL parallel workers
        // For now, simulate with thread pool
        use rayon::prelude::*;

        let results: Vec<Vec<(f32, ItemPointer)>> = (0..num_workers)
            .into_par_iter()
            .map(|worker_id| {
                let mut scan_desc = RuHnswParallelScanDesc::new(
                    Arc::clone(&self.shared_state),
                    worker_id,
                    query.clone(),
                );
                scan_desc.execute_scan(index);
                scan_desc.local_results
            })
            .collect();

        self.worker_results = results;

        // Merge results
        let k = {
            let shared = self.shared_state.read();
            shared.k
        };

        merge_knn_results_tournament(&self.worker_results, k)
    }

    /// Get statistics about the parallel scan
    pub fn get_stats(&self) -> ParallelScanStats {
        let shared = self.shared_state.read();
        ParallelScanStats {
            num_workers: shared.num_workers,
            total_partitions: shared.total_partitions,
            completed_workers: shared.completed_workers.load(AtomicOrdering::SeqCst),
            total_results: shared.total_results.load(AtomicOrdering::SeqCst),
        }
    }
}

/// Statistics from parallel scan
#[derive(Debug, Clone)]
pub struct ParallelScanStats {
    pub num_workers: u32,
    pub total_partitions: u32,
    pub completed_workers: u32,
    pub total_results: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_state_partitioning() {
        let state = RuHnswSharedState::new(
            4,  // 4 workers
            16, // 16 partitions
            128, // 128 dimensions
            10, // k=10
            40, // ef_search=40
            DistanceMetric::Euclidean,
        );

        // Workers claim partitions
        assert_eq!(state.get_next_partition(), Some(0));
        assert_eq!(state.get_next_partition(), Some(1));
        assert_eq!(state.get_next_partition(), Some(2));

        // Simulate all partitions claimed
        for _ in 3..16 {
            state.get_next_partition();
        }

        // No more partitions
        assert_eq!(state.get_next_partition(), None);
    }

    #[test]
    fn test_worker_estimation() {
        // Small index - no parallelism
        assert_eq!(ruhnsw_estimate_parallel_workers(50, 5000, 10, 40), 0);

        // Medium index - some parallelism
        let workers = ruhnsw_estimate_parallel_workers(2000, 100000, 10, 40);
        assert!(workers > 0 && workers <= 4);

        // Large complex query - more workers
        let workers_complex = ruhnsw_estimate_parallel_workers(5000, 500000, 100, 200);
        let workers_simple = ruhnsw_estimate_parallel_workers(5000, 500000, 10, 40);
        assert!(workers_complex >= workers_simple);
    }

    #[test]
    fn test_merge_knn_results() {
        let worker1 = vec![
            (0.1, ItemPointer::new(1, 1)),
            (0.3, ItemPointer::new(1, 3)),
            (0.5, ItemPointer::new(1, 5)),
        ];

        let worker2 = vec![
            (0.2, ItemPointer::new(2, 2)),
            (0.4, ItemPointer::new(2, 4)),
            (0.6, ItemPointer::new(2, 6)),
        ];

        let worker3 = vec![
            (0.15, ItemPointer::new(3, 1)),
            (0.35, ItemPointer::new(3, 3)),
        ];

        let results = merge_knn_results(&[worker1, worker2, worker3], 5);

        assert_eq!(results.len(), 5);

        // Should be sorted by distance
        assert_eq!(results[0].0, 0.1);
        assert_eq!(results[1].0, 0.15);
        assert_eq!(results[2].0, 0.2);
        assert_eq!(results[3].0, 0.3);
        assert_eq!(results[4].0, 0.35);
    }

    #[test]
    fn test_merge_tournament() {
        let worker1 = vec![
            (0.1, ItemPointer::new(1, 1)),
            (0.4, ItemPointer::new(1, 4)),
        ];

        let worker2 = vec![
            (0.2, ItemPointer::new(2, 2)),
            (0.5, ItemPointer::new(2, 5)),
        ];

        let worker3 = vec![
            (0.3, ItemPointer::new(3, 3)),
            (0.6, ItemPointer::new(3, 6)),
        ];

        let results = merge_knn_results_tournament(&[worker1, worker2, worker3], 4);

        assert_eq!(results.len(), 4);
        assert_eq!(results[0].0, 0.1);
        assert_eq!(results[1].0, 0.2);
        assert_eq!(results[2].0, 0.3);
        assert_eq!(results[3].0, 0.4);
    }

    #[test]
    fn test_partition_estimation() {
        // Small dataset - few partitions
        let partitions = estimate_partitions(2, 15000);
        assert!(partitions >= 2 && partitions <= 6);

        // Large dataset - more partitions
        let partitions_large = estimate_partitions(4, 500000);
        assert!(partitions_large > partitions);
    }

    #[test]
    fn test_item_pointer_creation() {
        let ip1 = create_item_pointer(0);
        assert_eq!(ip1.block_number, 0);
        assert_eq!(ip1.offset_number, 1);

        let ip2 = create_item_pointer(8191);
        assert_eq!(ip2.block_number, 1);
        assert_eq!(ip2.offset_number, 1);

        let ip3 = create_item_pointer(100);
        assert_eq!(ip3.block_number, 0);
        assert_eq!(ip3.offset_number, 101);
    }
}
