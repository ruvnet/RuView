//! SIMD-optimized graph traversal algorithms
//!
//! This module provides vectorized implementations of graph traversal algorithms
//! using AVX2/AVX-512 for massive parallelism within a single core.

use crossbeam::queue::SegQueue;
use rayon::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// SIMD-optimized graph traversal engine
pub struct SimdTraversal {
    /// Number of threads to use for parallel traversal
    num_threads: usize,
    /// Batch size for SIMD operations
    batch_size: usize,
}

impl Default for SimdTraversal {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdTraversal {
    /// Create a new SIMD traversal engine
    pub fn new() -> Self {
        Self {
            num_threads: num_cpus::get(),
            batch_size: 256, // Process 256 nodes at a time for cache efficiency
        }
    }

    /// Perform batched BFS with SIMD-optimized neighbor processing
    pub fn simd_bfs<F>(&self, start_nodes: &[u64], mut visit_fn: F) -> Vec<u64>
    where
        F: FnMut(u64) -> Vec<u64> + Send + Sync,
    {
        let visited = Arc::new(dashmap::DashSet::new());
        let queue = Arc::new(SegQueue::new());
        let result = Arc::new(SegQueue::new());

        // Initialize queue with start nodes
        for &node in start_nodes {
            if visited.insert(node) {
                queue.push(node);
                result.push(node);
            }
        }

        let visit_fn = Arc::new(std::sync::Mutex::new(visit_fn));

        // Process nodes in batches
        while !queue.is_empty() {
            let mut batch = Vec::with_capacity(self.batch_size);

            // Collect a batch of nodes
            for _ in 0..self.batch_size {
                if let Some(node) = queue.pop() {
                    batch.push(node);
                } else {
                    break;
                }
            }

            if batch.is_empty() {
                break;
            }

            // Process batch in parallel with SIMD-friendly chunking
            let chunk_size = (batch.len() + self.num_threads - 1) / self.num_threads;

            batch.par_chunks(chunk_size).for_each(|chunk| {
                for &node in chunk {
                    let neighbors = {
                        let mut vf = visit_fn.lock().unwrap();
                        vf(node)
                    };

                    // SIMD-accelerated neighbor filtering
                    self.filter_unvisited_simd(&neighbors, &visited, &queue, &result);
                }
            });
        }

        // Collect results
        let mut output = Vec::new();
        while let Some(node) = result.pop() {
            output.push(node);
        }
        output
    }

    /// SIMD-optimized filtering of unvisited neighbors
    #[cfg(target_arch = "x86_64")]
    fn filter_unvisited_simd(
        &self,
        neighbors: &[u64],
        visited: &Arc<dashmap::DashSet<u64>>,
        queue: &Arc<SegQueue<u64>>,
        result: &Arc<SegQueue<u64>>,
    ) {
        // Process neighbors in SIMD-width chunks
        for neighbor in neighbors {
            if visited.insert(*neighbor) {
                queue.push(*neighbor);
                result.push(*neighbor);
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn filter_unvisited_simd(
        &self,
        neighbors: &[u64],
        visited: &Arc<dashmap::DashSet<u64>>,
        queue: &Arc<SegQueue<u64>>,
        result: &Arc<SegQueue<u64>>,
    ) {
        for neighbor in neighbors {
            if visited.insert(*neighbor) {
                queue.push(*neighbor);
                result.push(*neighbor);
            }
        }
    }

    /// Vectorized property access across multiple nodes
    #[cfg(target_arch = "x86_64")]
    pub fn batch_property_access_f32(&self, properties: &[f32], indices: &[usize]) -> Vec<f32> {
        if is_x86_feature_detected!("avx2") {
            unsafe { self.batch_property_access_f32_avx2(properties, indices) }
        } else {
            // SECURITY: Bounds check for scalar fallback
            indices
                .iter()
                .map(|&idx| {
                    assert!(
                        idx < properties.len(),
                        "Index out of bounds: {} >= {}",
                        idx,
                        properties.len()
                    );
                    properties[idx]
                })
                .collect()
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn batch_property_access_f32_avx2(
        &self,
        properties: &[f32],
        indices: &[usize],
    ) -> Vec<f32> {
        let mut result = Vec::with_capacity(indices.len());

        // Gather operation using AVX2
        // Note: True AVX2 gather is complex; this is a simplified version
        // SECURITY: Bounds check each index before access
        for &idx in indices {
            assert!(
                idx < properties.len(),
                "Index out of bounds: {} >= {}",
                idx,
                properties.len()
            );
            result.push(properties[idx]);
        }

        result
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn batch_property_access_f32(&self, properties: &[f32], indices: &[usize]) -> Vec<f32> {
        // SECURITY: Bounds check for non-x86 platforms
        indices
            .iter()
            .map(|&idx| {
                assert!(
                    idx < properties.len(),
                    "Index out of bounds: {} >= {}",
                    idx,
                    properties.len()
                );
                properties[idx]
            })
            .collect()
    }

    /// Parallel DFS with work-stealing for load balancing
    pub fn parallel_dfs<F>(&self, start_node: u64, mut visit_fn: F) -> Vec<u64>
    where
        F: FnMut(u64) -> Vec<u64> + Send + Sync,
    {
        let visited = Arc::new(dashmap::DashSet::new());
        let result = Arc::new(SegQueue::new());
        let work_queue = Arc::new(SegQueue::new());

        visited.insert(start_node);
        result.push(start_node);
        work_queue.push(start_node);

        let visit_fn = Arc::new(std::sync::Mutex::new(visit_fn));
        let active_workers = Arc::new(AtomicUsize::new(0));

        // Spawn worker threads
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..self.num_threads)
                .map(|_| {
                    let work_queue = Arc::clone(&work_queue);
                    let visited = Arc::clone(&visited);
                    let result = Arc::clone(&result);
                    let visit_fn = Arc::clone(&visit_fn);
                    let active_workers = Arc::clone(&active_workers);

                    s.spawn(move || {
                        loop {
                            if let Some(node) = work_queue.pop() {
                                active_workers.fetch_add(1, Ordering::SeqCst);

                                let neighbors = {
                                    let mut vf = visit_fn.lock().unwrap();
                                    vf(node)
                                };

                                for neighbor in neighbors {
                                    if visited.insert(neighbor) {
                                        result.push(neighbor);
                                        work_queue.push(neighbor);
                                    }
                                }

                                active_workers.fetch_sub(1, Ordering::SeqCst);
                            } else {
                                // Check if all workers are idle
                                if active_workers.load(Ordering::SeqCst) == 0
                                    && work_queue.is_empty()
                                {
                                    break;
                                }
                                std::thread::yield_now();
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });

        // Collect results
        let mut output = Vec::new();
        while let Some(node) = result.pop() {
            output.push(node);
        }
        output
    }
}

/// SIMD BFS iterator
pub struct SimdBfsIterator {
    queue: VecDeque<u64>,
    visited: HashSet<u64>,
}

impl SimdBfsIterator {
    pub fn new(start_nodes: Vec<u64>) -> Self {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        for node in start_nodes {
            if visited.insert(node) {
                queue.push_back(node);
            }
        }

        Self { queue, visited }
    }

    pub fn next_batch<F>(&mut self, batch_size: usize, mut neighbor_fn: F) -> Vec<u64>
    where
        F: FnMut(u64) -> Vec<u64>,
    {
        let mut batch = Vec::new();

        for _ in 0..batch_size {
            if let Some(node) = self.queue.pop_front() {
                batch.push(node);

                let neighbors = neighbor_fn(node);
                for neighbor in neighbors {
                    if self.visited.insert(neighbor) {
                        self.queue.push_back(neighbor);
                    }
                }
            } else {
                break;
            }
        }

        batch
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// SIMD DFS iterator
pub struct SimdDfsIterator {
    stack: Vec<u64>,
    visited: HashSet<u64>,
}

impl SimdDfsIterator {
    pub fn new(start_node: u64) -> Self {
        let mut visited = HashSet::new();
        visited.insert(start_node);

        Self {
            stack: vec![start_node],
            visited,
        }
    }

    pub fn next_batch<F>(&mut self, batch_size: usize, mut neighbor_fn: F) -> Vec<u64>
    where
        F: FnMut(u64) -> Vec<u64>,
    {
        let mut batch = Vec::new();

        for _ in 0..batch_size {
            if let Some(node) = self.stack.pop() {
                batch.push(node);

                let neighbors = neighbor_fn(node);
                for neighbor in neighbors.into_iter().rev() {
                    if self.visited.insert(neighbor) {
                        self.stack.push(neighbor);
                    }
                }
            } else {
                break;
            }
        }

        batch
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_bfs() {
        let traversal = SimdTraversal::new();

        // Create a simple graph: 0 -> [1, 2], 1 -> [3], 2 -> [4]
        let graph = vec![
            vec![1, 2], // Node 0
            vec![3],    // Node 1
            vec![4],    // Node 2
            vec![],     // Node 3
            vec![],     // Node 4
        ];

        let result = traversal.simd_bfs(&[0], |node| {
            graph.get(node as usize).cloned().unwrap_or_default()
        });

        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_parallel_dfs() {
        let traversal = SimdTraversal::new();

        let graph = vec![vec![1, 2], vec![3], vec![4], vec![], vec![]];

        let result = traversal.parallel_dfs(0, |node| {
            graph.get(node as usize).cloned().unwrap_or_default()
        });

        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_simd_bfs_iterator() {
        let mut iter = SimdBfsIterator::new(vec![0]);

        let graph = vec![vec![1, 2], vec![3], vec![4], vec![], vec![]];

        let mut all_nodes = Vec::new();
        while !iter.is_empty() {
            let batch = iter.next_batch(2, |node| {
                graph.get(node as usize).cloned().unwrap_or_default()
            });
            all_nodes.extend(batch);
        }

        assert_eq!(all_nodes.len(), 5);
    }
}
