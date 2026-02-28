//! Parallel Level Updates with Work-Stealing
//!
//! Provides efficient parallel computation for j-tree levels:
//! - Rayon-based parallel iteration
//! - Work-stealing for load balancing
//! - Lock-free result aggregation
//! - Adaptive parallelism based on workload
//!
//! Target: Near-linear speedup for independent level updates

use crate::graph::VertexId;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

/// Configuration for parallel level updates
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Minimum workload to use parallelism
    pub min_parallel_size: usize,
    /// Number of threads (0 = auto-detect)
    pub num_threads: usize,
    /// Enable work-stealing
    pub work_stealing: bool,
    /// Chunk size for parallel iteration
    pub chunk_size: usize,
    /// Enable adaptive parallelism
    pub adaptive: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            min_parallel_size: 100,
            num_threads: 0, // Auto-detect
            work_stealing: true,
            chunk_size: 64,
            adaptive: true,
        }
    }
}

/// Work item for parallel processing
#[derive(Debug, Clone)]
pub struct WorkItem {
    /// Level index
    pub level: usize,
    /// Vertices to process
    pub vertices: Vec<VertexId>,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Estimated work units
    pub estimated_work: usize,
}

/// Result from parallel level update
#[derive(Debug, Clone)]
pub struct LevelUpdateResult {
    /// Level index
    pub level: usize,
    /// Computed cut value
    pub cut_value: f64,
    /// Partition (vertices on one side)
    pub partition: HashSet<VertexId>,
    /// Time taken in microseconds
    pub time_us: u64,
}

/// Work-stealing scheduler for parallel level processing
pub struct WorkStealingScheduler {
    config: ParallelConfig,
    /// Work queue
    work_queue: RwLock<Vec<WorkItem>>,
    /// Completed results
    results: RwLock<HashMap<usize, LevelUpdateResult>>,
    /// Active workers count
    active_workers: AtomicUsize,
    /// Total work processed
    total_work: AtomicU64,
    /// Steal count
    steals: AtomicU64,
}

impl WorkStealingScheduler {
    /// Create new scheduler with default config
    pub fn new() -> Self {
        Self::with_config(ParallelConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: ParallelConfig) -> Self {
        Self {
            config,
            work_queue: RwLock::new(Vec::new()),
            results: RwLock::new(HashMap::new()),
            active_workers: AtomicUsize::new(0),
            total_work: AtomicU64::new(0),
            steals: AtomicU64::new(0),
        }
    }

    /// Submit work item
    pub fn submit(&self, item: WorkItem) {
        let mut queue = self.work_queue.write().unwrap();
        let estimated_work = item.estimated_work;
        queue.push(item);

        // Sort by priority (ascending)
        queue.sort_by_key(|w| w.priority);

        self.total_work
            .fetch_add(estimated_work as u64, Ordering::Relaxed);
    }

    /// Submit multiple work items
    pub fn submit_batch(&self, items: Vec<WorkItem>) {
        let mut queue = self.work_queue.write().unwrap();

        for item in items {
            self.total_work
                .fetch_add(item.estimated_work as u64, Ordering::Relaxed);
            queue.push(item);
        }

        // Sort by priority (ascending)
        queue.sort_by_key(|w| w.priority);
    }

    /// Try to steal work from queue
    pub fn steal(&self) -> Option<WorkItem> {
        let mut queue = self.work_queue.write().unwrap();

        if queue.is_empty() {
            return None;
        }

        self.steals.fetch_add(1, Ordering::Relaxed);

        // Steal from front (highest priority)
        Some(queue.remove(0))
    }

    /// Record result
    pub fn complete(&self, result: LevelUpdateResult) {
        let mut results = self.results.write().unwrap();
        results.insert(result.level, result);
    }

    /// Get all results
    pub fn get_results(&self) -> HashMap<usize, LevelUpdateResult> {
        self.results.read().unwrap().clone()
    }

    /// Clear results
    pub fn clear_results(&self) {
        self.results.write().unwrap().clear();
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.work_queue.read().unwrap().is_empty()
    }

    /// Get queue size
    pub fn queue_size(&self) -> usize {
        self.work_queue.read().unwrap().len()
    }

    /// Get total steals
    pub fn steal_count(&self) -> u64 {
        self.steals.load(Ordering::Relaxed)
    }
}

impl Default for WorkStealingScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Parallel level updater using Rayon
pub struct ParallelLevelUpdater {
    config: ParallelConfig,
    /// Scheduler for work-stealing
    scheduler: Arc<WorkStealingScheduler>,
    /// Global minimum cut found
    global_min: AtomicU64,
    /// Level with global minimum
    best_level: AtomicUsize,
}

impl ParallelLevelUpdater {
    /// Create new parallel updater with default config
    pub fn new() -> Self {
        Self::with_config(ParallelConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: ParallelConfig) -> Self {
        Self {
            scheduler: Arc::new(WorkStealingScheduler::with_config(config.clone())),
            config,
            global_min: AtomicU64::new(f64::INFINITY.to_bits()),
            best_level: AtomicUsize::new(usize::MAX),
        }
    }

    /// Update global minimum atomically
    pub fn try_update_min(&self, value: f64, level: usize) -> bool {
        let value_bits = value.to_bits();
        let mut current = self.global_min.load(Ordering::Acquire);

        loop {
            let current_value = f64::from_bits(current);
            if value >= current_value {
                return false;
            }

            match self.global_min.compare_exchange_weak(
                current,
                value_bits,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.best_level.store(level, Ordering::Release);
                    return true;
                }
                Err(c) => current = c,
            }
        }
    }

    /// Get current global minimum
    pub fn global_min(&self) -> f64 {
        f64::from_bits(self.global_min.load(Ordering::Acquire))
    }

    /// Get best level
    pub fn best_level(&self) -> Option<usize> {
        let level = self.best_level.load(Ordering::Acquire);
        if level == usize::MAX {
            None
        } else {
            Some(level)
        }
    }

    /// Reset global minimum
    pub fn reset_min(&self) {
        self.global_min
            .store(f64::INFINITY.to_bits(), Ordering::Release);
        self.best_level.store(usize::MAX, Ordering::Release);
    }

    /// Process levels in parallel using Rayon
    #[cfg(feature = "rayon")]
    pub fn process_parallel<F>(&self, levels: &[usize], mut process_fn: F) -> Vec<LevelUpdateResult>
    where
        F: FnMut(usize) -> LevelUpdateResult + Send + Sync + Clone,
    {
        let size = levels.len();

        if size < self.config.min_parallel_size {
            // Sequential processing for small workloads
            return levels
                .iter()
                .map(|&level| {
                    let result = process_fn.clone()(level);
                    self.try_update_min(result.cut_value, level);
                    result
                })
                .collect();
        }

        // Parallel processing with Rayon
        levels
            .par_iter()
            .map(|&level| {
                let result = process_fn.clone()(level);
                self.try_update_min(result.cut_value, level);
                result
            })
            .collect()
    }

    /// Process levels in parallel (scalar fallback)
    #[cfg(not(feature = "rayon"))]
    pub fn process_parallel<F>(&self, levels: &[usize], mut process_fn: F) -> Vec<LevelUpdateResult>
    where
        F: FnMut(usize) -> LevelUpdateResult + Clone,
    {
        levels
            .iter()
            .map(|&level| {
                let result = process_fn.clone()(level);
                self.try_update_min(result.cut_value, level);
                result
            })
            .collect()
    }

    /// Process work items with work-stealing
    #[cfg(feature = "rayon")]
    pub fn process_with_stealing<F>(
        &self,
        work_items: Vec<WorkItem>,
        process_fn: F,
    ) -> Vec<LevelUpdateResult>
    where
        F: Fn(&WorkItem) -> LevelUpdateResult + Send + Sync,
    {
        if work_items.len() < self.config.min_parallel_size {
            // Sequential
            return work_items
                .iter()
                .map(|item| {
                    let result = process_fn(item);
                    self.try_update_min(result.cut_value, item.level);
                    result
                })
                .collect();
        }

        // Parallel with work-stealing
        work_items
            .par_iter()
            .map(|item| {
                let result = process_fn(item);
                self.try_update_min(result.cut_value, item.level);
                result
            })
            .collect()
    }

    /// Process work items (scalar fallback)
    #[cfg(not(feature = "rayon"))]
    pub fn process_with_stealing<F>(
        &self,
        work_items: Vec<WorkItem>,
        process_fn: F,
    ) -> Vec<LevelUpdateResult>
    where
        F: Fn(&WorkItem) -> LevelUpdateResult,
    {
        work_items
            .iter()
            .map(|item| {
                let result = process_fn(item);
                self.try_update_min(result.cut_value, item.level);
                result
            })
            .collect()
    }

    /// Batch vertex processing within a level
    #[cfg(feature = "rayon")]
    pub fn process_vertices_parallel<F, R>(&self, vertices: &[VertexId], process_fn: F) -> Vec<R>
    where
        F: Fn(VertexId) -> R + Send + Sync,
        R: Send,
    {
        if vertices.len() < self.config.min_parallel_size {
            return vertices.iter().map(|&v| process_fn(v)).collect();
        }

        vertices.par_iter().map(|&v| process_fn(v)).collect()
    }

    /// Batch vertex processing (scalar fallback)
    #[cfg(not(feature = "rayon"))]
    pub fn process_vertices_parallel<F, R>(&self, vertices: &[VertexId], process_fn: F) -> Vec<R>
    where
        F: Fn(VertexId) -> R,
    {
        vertices.iter().map(|&v| process_fn(v)).collect()
    }

    /// Parallel reduction for aggregating results
    #[cfg(feature = "rayon")]
    pub fn parallel_reduce<T, F, R>(
        &self,
        items: &[T],
        identity: R,
        map_fn: F,
        reduce_fn: fn(R, R) -> R,
    ) -> R
    where
        T: Sync,
        F: Fn(&T) -> R + Send + Sync,
        R: Send + Clone,
    {
        if items.len() < self.config.min_parallel_size {
            return items
                .iter()
                .map(|item| map_fn(item))
                .fold(identity.clone(), reduce_fn);
        }

        items
            .par_iter()
            .map(|item| map_fn(item))
            .reduce(|| identity.clone(), reduce_fn)
    }

    /// Parallel reduction (scalar fallback)
    #[cfg(not(feature = "rayon"))]
    pub fn parallel_reduce<T, F, R>(
        &self,
        items: &[T],
        identity: R,
        map_fn: F,
        reduce_fn: fn(R, R) -> R,
    ) -> R
    where
        F: Fn(&T) -> R,
        R: Clone,
    {
        items
            .iter()
            .map(|item| map_fn(item))
            .fold(identity, reduce_fn)
    }

    /// Get scheduler reference
    pub fn scheduler(&self) -> &Arc<WorkStealingScheduler> {
        &self.scheduler
    }
}

impl Default for ParallelLevelUpdater {
    fn default() -> Self {
        Self::new()
    }
}

/// Parallel cut computation helpers
pub struct ParallelCutOps;

impl ParallelCutOps {
    /// Compute boundary size in parallel
    #[cfg(feature = "rayon")]
    pub fn boundary_size_parallel(
        partition: &HashSet<VertexId>,
        adjacency: &HashMap<VertexId, Vec<(VertexId, f64)>>,
    ) -> f64 {
        let partition_vec: Vec<_> = partition.iter().copied().collect();

        if partition_vec.len() < 100 {
            return Self::boundary_size_sequential(partition, adjacency);
        }

        partition_vec
            .par_iter()
            .map(|&v| {
                adjacency
                    .get(&v)
                    .map(|neighbors| {
                        neighbors
                            .iter()
                            .filter(|(n, _)| !partition.contains(n))
                            .map(|(_, w)| w)
                            .sum::<f64>()
                    })
                    .unwrap_or(0.0)
            })
            .sum()
    }

    /// Compute boundary size sequentially
    #[cfg(not(feature = "rayon"))]
    pub fn boundary_size_parallel(
        partition: &HashSet<VertexId>,
        adjacency: &HashMap<VertexId, Vec<(VertexId, f64)>>,
    ) -> f64 {
        Self::boundary_size_sequential(partition, adjacency)
    }

    /// Sequential boundary computation
    pub fn boundary_size_sequential(
        partition: &HashSet<VertexId>,
        adjacency: &HashMap<VertexId, Vec<(VertexId, f64)>>,
    ) -> f64 {
        partition
            .iter()
            .map(|&v| {
                adjacency
                    .get(&v)
                    .map(|neighbors| {
                        neighbors
                            .iter()
                            .filter(|(n, _)| !partition.contains(n))
                            .map(|(_, w)| w)
                            .sum::<f64>()
                    })
                    .unwrap_or(0.0)
            })
            .sum()
    }

    /// Find minimum degree vertex in parallel
    #[cfg(feature = "rayon")]
    pub fn min_degree_vertex_parallel(
        vertices: &[VertexId],
        adjacency: &HashMap<VertexId, Vec<(VertexId, f64)>>,
    ) -> Option<(VertexId, usize)> {
        if vertices.len() < 100 {
            return Self::min_degree_vertex_sequential(vertices, adjacency);
        }

        vertices
            .par_iter()
            .map(|&v| {
                let degree = adjacency.get(&v).map(|n| n.len()).unwrap_or(0);
                (v, degree)
            })
            .filter(|(_, d)| *d > 0)
            .min_by_key(|(_, d)| *d)
    }

    /// Find minimum degree vertex sequentially
    #[cfg(not(feature = "rayon"))]
    pub fn min_degree_vertex_parallel(
        vertices: &[VertexId],
        adjacency: &HashMap<VertexId, Vec<(VertexId, f64)>>,
    ) -> Option<(VertexId, usize)> {
        Self::min_degree_vertex_sequential(vertices, adjacency)
    }

    /// Sequential minimum degree
    pub fn min_degree_vertex_sequential(
        vertices: &[VertexId],
        adjacency: &HashMap<VertexId, Vec<(VertexId, f64)>>,
    ) -> Option<(VertexId, usize)> {
        vertices
            .iter()
            .map(|&v| {
                let degree = adjacency.get(&v).map(|n| n.len()).unwrap_or(0);
                (v, degree)
            })
            .filter(|(_, d)| *d > 0)
            .min_by_key(|(_, d)| *d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_item_submission() {
        let scheduler = WorkStealingScheduler::new();

        scheduler.submit(WorkItem {
            level: 0,
            vertices: vec![1, 2, 3],
            priority: 1,
            estimated_work: 100,
        });

        scheduler.submit(WorkItem {
            level: 1,
            vertices: vec![4, 5, 6],
            priority: 0, // Higher priority
            estimated_work: 50,
        });

        assert_eq!(scheduler.queue_size(), 2);

        // Should steal highest priority first
        let stolen = scheduler.steal().unwrap();
        assert_eq!(stolen.level, 1); // Priority 0 comes first
    }

    #[test]
    fn test_parallel_updater_min() {
        let updater = ParallelLevelUpdater::new();

        assert!(updater.global_min().is_infinite());

        assert!(updater.try_update_min(10.0, 0));
        assert_eq!(updater.global_min(), 10.0);
        assert_eq!(updater.best_level(), Some(0));

        assert!(updater.try_update_min(5.0, 1));
        assert_eq!(updater.global_min(), 5.0);
        assert_eq!(updater.best_level(), Some(1));

        // Should not update with higher value
        assert!(!updater.try_update_min(7.0, 2));
        assert_eq!(updater.global_min(), 5.0);
    }

    #[test]
    fn test_process_parallel() {
        let updater = ParallelLevelUpdater::new();

        let levels = vec![0, 1, 2, 3, 4];

        let results = updater.process_parallel(&levels, |level| LevelUpdateResult {
            level,
            cut_value: level as f64 * 2.0,
            partition: HashSet::new(),
            time_us: 0,
        });

        assert_eq!(results.len(), 5);
        assert_eq!(updater.global_min(), 0.0);
        assert_eq!(updater.best_level(), Some(0));
    }

    #[test]
    fn test_boundary_size() {
        let partition: HashSet<_> = vec![1, 2].into_iter().collect();

        let mut adjacency: HashMap<VertexId, Vec<(VertexId, f64)>> = HashMap::new();
        adjacency.insert(1, vec![(2, 1.0), (3, 2.0)]);
        adjacency.insert(2, vec![(1, 1.0), (4, 3.0)]);
        adjacency.insert(3, vec![(1, 2.0)]);
        adjacency.insert(4, vec![(2, 3.0)]);

        let boundary = ParallelCutOps::boundary_size_sequential(&partition, &adjacency);

        // Edges crossing: 1-3 (2.0) + 2-4 (3.0) = 5.0
        assert_eq!(boundary, 5.0);
    }

    #[test]
    fn test_min_degree_vertex() {
        let vertices: Vec<_> = vec![1, 2, 3, 4];

        let mut adjacency: HashMap<VertexId, Vec<(VertexId, f64)>> = HashMap::new();
        adjacency.insert(1, vec![(2, 1.0), (3, 1.0), (4, 1.0)]);
        adjacency.insert(2, vec![(1, 1.0)]);
        adjacency.insert(3, vec![(1, 1.0), (4, 1.0)]);
        adjacency.insert(4, vec![(1, 1.0), (3, 1.0)]);

        let (min_v, min_deg) =
            ParallelCutOps::min_degree_vertex_sequential(&vertices, &adjacency).unwrap();

        assert_eq!(min_v, 2);
        assert_eq!(min_deg, 1);
    }

    #[test]
    fn test_scheduler_steal_count() {
        let scheduler = WorkStealingScheduler::new();

        scheduler.submit(WorkItem {
            level: 0,
            vertices: vec![1],
            priority: 0,
            estimated_work: 10,
        });

        assert_eq!(scheduler.steal_count(), 0);
        let _ = scheduler.steal();
        assert_eq!(scheduler.steal_count(), 1);
    }

    #[test]
    fn test_batch_submit() {
        let scheduler = WorkStealingScheduler::new();

        let items = vec![
            WorkItem {
                level: 0,
                vertices: vec![],
                priority: 2,
                estimated_work: 100,
            },
            WorkItem {
                level: 1,
                vertices: vec![],
                priority: 0,
                estimated_work: 50,
            },
            WorkItem {
                level: 2,
                vertices: vec![],
                priority: 1,
                estimated_work: 75,
            },
        ];

        scheduler.submit_batch(items);

        assert_eq!(scheduler.queue_size(), 3);

        // Should be sorted by priority
        let first = scheduler.steal().unwrap();
        assert_eq!(first.level, 1); // Priority 0
    }
}
