//! Memory pools for BFS and graph traversal allocations
//!
//! Provides reusable memory pools to reduce allocation overhead during
//! repeated BFS/DFS operations in minimum cut algorithms.
//!
//! # Overview
//!
//! Graph algorithms like BFS perform many allocations:
//! - Queue for vertices to visit
//! - HashSet/BitSet for visited vertices
//! - Vec for collecting results
//!
//! By reusing these structures, we avoid repeated allocation/deallocation
//! overhead, which can be significant for algorithms that perform many
//! traversals.
//!
//! # Thread Safety
//!
//! The pools use thread-local storage for zero-contention access.
//! Each thread gets its own pool of resources.
//!
//! # Example
//!
//! ```
//! use ruvector_mincut::pool::BfsPool;
//!
//! // Acquire resources from the pool
//! let mut resources = BfsPool::acquire(256);
//!
//! // Use resources for BFS
//! resources.queue.push_back(0);
//! resources.visited.insert(0);
//!
//! // Clear and return to pool automatically when dropped
//! drop(resources);
//! ```

use crate::graph::VertexId;
use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};

/// Thread-local pool for BFS resources
thread_local! {
    static BFS_POOL: RefCell<BfsPoolInner> = RefCell::new(BfsPoolInner::new());
}

/// Inner pool state
struct BfsPoolInner {
    /// Pool of reusable queues
    queues: Vec<VecDeque<VertexId>>,
    /// Pool of reusable visited sets
    visited_sets: Vec<HashSet<VertexId>>,
    /// Pool of reusable result vectors
    result_vecs: Vec<Vec<VertexId>>,
    /// Statistics: number of acquires
    acquires: usize,
    /// Statistics: number of hits (reused from pool)
    hits: usize,
}

impl BfsPoolInner {
    fn new() -> Self {
        Self {
            queues: Vec::new(),
            visited_sets: Vec::new(),
            result_vecs: Vec::new(),
            acquires: 0,
            hits: 0,
        }
    }

    fn acquire_queue(&mut self, capacity: usize) -> VecDeque<VertexId> {
        self.acquires += 1;
        if let Some(mut queue) = self.queues.pop() {
            self.hits += 1;
            queue.clear();
            // Reserve if needed
            if queue.capacity() < capacity {
                queue.reserve(capacity - queue.len());
            }
            queue
        } else {
            VecDeque::with_capacity(capacity)
        }
    }

    fn acquire_visited(&mut self, capacity: usize) -> HashSet<VertexId> {
        self.acquires += 1;
        if let Some(mut set) = self.visited_sets.pop() {
            self.hits += 1;
            set.clear();
            if set.capacity() < capacity {
                set.reserve(capacity - set.len());
            }
            set
        } else {
            HashSet::with_capacity(capacity)
        }
    }

    fn acquire_vec(&mut self, capacity: usize) -> Vec<VertexId> {
        self.acquires += 1;
        if let Some(mut v) = self.result_vecs.pop() {
            self.hits += 1;
            v.clear();
            if v.capacity() < capacity {
                v.reserve(capacity - v.len());
            }
            v
        } else {
            Vec::with_capacity(capacity)
        }
    }

    fn return_queue(&mut self, queue: VecDeque<VertexId>) {
        // Keep at most 8 pooled queues
        if self.queues.len() < 8 {
            self.queues.push(queue);
        }
    }

    fn return_visited(&mut self, set: HashSet<VertexId>) {
        if self.visited_sets.len() < 8 {
            self.visited_sets.push(set);
        }
    }

    fn return_vec(&mut self, v: Vec<VertexId>) {
        if self.result_vecs.len() < 8 {
            self.result_vecs.push(v);
        }
    }
}

/// BFS resources acquired from the pool
///
/// Automatically returns resources to the pool when dropped.
pub struct BfsResources {
    /// Queue for BFS traversal
    pub queue: VecDeque<VertexId>,
    /// Set of visited vertices
    pub visited: HashSet<VertexId>,
    /// Vector for collecting results
    pub results: Vec<VertexId>,
}

impl Drop for BfsResources {
    fn drop(&mut self) {
        // Return resources to pool
        BFS_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();

            // Take ownership via swap
            let queue = std::mem::take(&mut self.queue);
            let visited = std::mem::take(&mut self.visited);
            let results = std::mem::take(&mut self.results);

            pool.return_queue(queue);
            pool.return_visited(visited);
            pool.return_vec(results);
        });
    }
}

/// Pool for BFS memory allocation
///
/// Provides thread-local pools for reusing BFS data structures.
pub struct BfsPool;

impl BfsPool {
    /// Acquire BFS resources from the pool
    ///
    /// # Arguments
    ///
    /// * `expected_size` - Expected number of vertices to visit
    ///
    /// # Returns
    ///
    /// BfsResources that will be returned to the pool when dropped
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_mincut::pool::BfsPool;
    ///
    /// let mut res = BfsPool::acquire(100);
    ///
    /// // Perform BFS
    /// res.queue.push_back(0);
    /// while let Some(v) = res.queue.pop_front() {
    ///     if res.visited.insert(v) {
    ///         res.results.push(v);
    ///         // Push neighbors...
    ///     }
    /// }
    ///
    /// // Resources automatically returned when res is dropped
    /// ```
    pub fn acquire(expected_size: usize) -> BfsResources {
        BFS_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            BfsResources {
                queue: pool.acquire_queue(expected_size),
                visited: pool.acquire_visited(expected_size),
                results: pool.acquire_vec(expected_size),
            }
        })
    }

    /// Get pool statistics for the current thread
    ///
    /// Returns (acquires, hits, hit_rate)
    pub fn stats() -> (usize, usize, f64) {
        BFS_POOL.with(|pool| {
            let pool = pool.borrow();
            let rate = if pool.acquires > 0 {
                pool.hits as f64 / pool.acquires as f64
            } else {
                0.0
            };
            (pool.acquires, pool.hits, rate)
        })
    }

    /// Clear the pool (useful for testing or memory pressure)
    pub fn clear() {
        BFS_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            pool.queues.clear();
            pool.visited_sets.clear();
            pool.result_vecs.clear();
        });
    }
}

/// Pool for distance-annotated BFS
pub struct DistanceBfsResources {
    /// Queue with (vertex, distance) pairs
    pub queue: VecDeque<(VertexId, usize)>,
    /// Set of visited vertices
    pub visited: HashSet<VertexId>,
    /// Distance map
    pub distances: std::collections::HashMap<VertexId, usize>,
}

impl Default for DistanceBfsResources {
    fn default() -> Self {
        Self::new()
    }
}

impl DistanceBfsResources {
    /// Create new distance BFS resources
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            visited: HashSet::new(),
            distances: std::collections::HashMap::new(),
        }
    }

    /// Create with capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
            visited: HashSet::with_capacity(capacity),
            distances: std::collections::HashMap::with_capacity(capacity),
        }
    }

    /// Clear all resources for reuse
    pub fn clear(&mut self) {
        self.queue.clear();
        self.visited.clear();
        self.distances.clear();
    }

    /// Perform BFS from a source vertex
    ///
    /// Returns the set of vertices reachable within the given radius.
    ///
    /// # Arguments
    ///
    /// * `source` - Starting vertex
    /// * `radius` - Maximum distance to traverse
    /// * `adjacency` - Function to get neighbors of a vertex
    pub fn bfs_within_radius<F>(
        &mut self,
        source: VertexId,
        radius: usize,
        adjacency: F,
    ) -> &HashSet<VertexId>
    where
        F: Fn(VertexId) -> Vec<VertexId>,
    {
        self.clear();

        self.queue.push_back((source, 0));
        self.visited.insert(source);
        self.distances.insert(source, 0);

        while let Some((vertex, dist)) = self.queue.pop_front() {
            if dist >= radius {
                continue;
            }

            for neighbor in adjacency(vertex) {
                if self.visited.insert(neighbor) {
                    let new_dist = dist + 1;
                    self.distances.insert(neighbor, new_dist);
                    self.queue.push_back((neighbor, new_dist));
                }
            }
        }

        &self.visited
    }
}

/// Compact bitset pool for small graphs
///
/// Uses fixed-size bitsets instead of HashSets for graphs with <= 256 vertices.
pub struct CompactBfsResources {
    /// Queue for BFS traversal
    pub queue: VecDeque<VertexId>,
    /// Visited bitmap (256 bits = 32 bytes)
    pub visited: [u64; 4],
    /// Results vector
    pub results: Vec<VertexId>,
}

impl Default for CompactBfsResources {
    fn default() -> Self {
        Self::new()
    }
}

impl CompactBfsResources {
    /// Create new compact BFS resources
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(32),
            visited: [0; 4],
            results: Vec::with_capacity(32),
        }
    }

    /// Clear for reuse
    pub fn clear(&mut self) {
        self.queue.clear();
        self.visited = [0; 4];
        self.results.clear();
    }

    /// Check if vertex is visited
    #[inline]
    pub fn is_visited(&self, v: VertexId) -> bool {
        if v >= 256 {
            return false;
        }
        let idx = (v / 64) as usize;
        let bit = v % 64;
        (self.visited[idx] & (1u64 << bit)) != 0
    }

    /// Mark vertex as visited
    #[inline]
    pub fn mark_visited(&mut self, v: VertexId) -> bool {
        if v >= 256 {
            return false;
        }
        let idx = (v / 64) as usize;
        let bit = v % 64;
        let was_visited = (self.visited[idx] & (1u64 << bit)) != 0;
        self.visited[idx] |= 1u64 << bit;
        !was_visited
    }

    /// Count visited vertices
    pub fn visited_count(&self) -> usize {
        self.visited.iter().map(|w| w.count_ones() as usize).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bfs_pool_acquire() {
        let res = BfsPool::acquire(100);
        assert!(res.queue.is_empty());
        assert!(res.visited.is_empty());
        assert!(res.results.is_empty());
    }

    #[test]
    fn test_bfs_pool_reuse() {
        // First acquire
        {
            let mut res = BfsPool::acquire(100);
            res.queue.push_back(1);
            res.queue.push_back(2);
            res.visited.insert(1);
            res.visited.insert(2);
        } // Returned to pool

        // Second acquire should get cleared resources
        let res = BfsPool::acquire(100);
        assert!(res.queue.is_empty());
        assert!(res.visited.is_empty());
    }

    #[test]
    fn test_bfs_pool_stats() {
        BfsPool::clear(); // Reset stats

        // Multiple acquires
        let _r1 = BfsPool::acquire(10);
        let _r2 = BfsPool::acquire(10);
        drop(_r1);
        drop(_r2);

        // Third acquire should hit cache
        let _r3 = BfsPool::acquire(10);

        let (acquires, hits, _rate) = BfsPool::stats();
        assert!(acquires >= 3);
        assert!(hits >= 1); // At least one hit
    }

    #[test]
    fn test_distance_bfs() {
        let mut res = DistanceBfsResources::with_capacity(10);

        // Linear graph: 0 - 1 - 2 - 3 - 4
        let adjacency = |v: VertexId| -> Vec<VertexId> {
            match v {
                0 => vec![1],
                1 => vec![0, 2],
                2 => vec![1, 3],
                3 => vec![2, 4],
                4 => vec![3],
                _ => vec![],
            }
        };

        let visited = res.bfs_within_radius(0, 2, adjacency);

        // Should reach 0, 1, 2 (radius 2 from 0)
        assert!(visited.contains(&0));
        assert!(visited.contains(&1));
        assert!(visited.contains(&2));
        assert!(!visited.contains(&3)); // Beyond radius
        assert!(!visited.contains(&4));

        // Check distances
        assert_eq!(res.distances.get(&0), Some(&0));
        assert_eq!(res.distances.get(&1), Some(&1));
        assert_eq!(res.distances.get(&2), Some(&2));
    }

    #[test]
    fn test_compact_bfs() {
        let mut res = CompactBfsResources::new();

        assert!(!res.is_visited(0));
        assert!(res.mark_visited(0)); // First visit returns true
        assert!(res.is_visited(0));
        assert!(!res.mark_visited(0)); // Second visit returns false

        res.mark_visited(100);
        res.mark_visited(255);

        assert_eq!(res.visited_count(), 3);

        res.clear();
        assert_eq!(res.visited_count(), 0);
    }

    #[test]
    fn test_compact_bfs_boundary() {
        let mut res = CompactBfsResources::new();

        // Test boundary vertices
        assert!(res.mark_visited(0));
        assert!(res.mark_visited(63));
        assert!(res.mark_visited(64));
        assert!(res.mark_visited(127));
        assert!(res.mark_visited(128));
        assert!(res.mark_visited(191));
        assert!(res.mark_visited(192));
        assert!(res.mark_visited(255));

        assert!(res.is_visited(0));
        assert!(res.is_visited(255));

        // Out of range
        assert!(!res.is_visited(256));
        assert!(!res.mark_visited(256));
    }
}
