//! Parallel distribution for 256-core agentic chip
//!
//! Distributes minimum cut computation across WASM cores.

// Internal optimization module - docs on public API in lib.rs
#![allow(missing_docs)]

use crate::compact::{
    BitSet256, CompactCoreState, CompactEdge, CompactVertexId, CompactWitness, CoreResult,
    MAX_EDGES_PER_CORE,
};
use core::sync::atomic::{AtomicU16, AtomicU8, Ordering};

// SIMD functions (inlined for non-wasm, uses wasm::simd when available)
#[cfg(feature = "wasm")]
use crate::wasm::simd::{simd_boundary_size, simd_popcount};

#[cfg(not(feature = "wasm"))]
#[inline]
fn simd_popcount(bits: &[u64; 4]) -> u32 {
    bits.iter().map(|b| b.count_ones()).sum()
}

#[cfg(not(feature = "wasm"))]
#[inline]
fn simd_boundary_size(set_a: &BitSet256, edges: &[(CompactVertexId, CompactVertexId)]) -> u16 {
    let mut count = 0u16;
    for &(src, tgt) in edges {
        let src_in = set_a.contains(src);
        let tgt_in = set_a.contains(tgt);
        if src_in != tgt_in {
            count += 1;
        }
    }
    count
}

/// Number of WASM cores
pub const NUM_CORES: usize = 256;

/// Number of geometric ranges per core
pub const RANGES_PER_CORE: usize = 1;

/// Total ranges = NUM_CORES × RANGES_PER_CORE
pub const TOTAL_RANGES: usize = NUM_CORES * RANGES_PER_CORE;

/// Range factor (1.2 from paper)
pub const RANGE_FACTOR: f32 = 1.2;

/// Core assignment strategy
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CoreStrategy {
    /// Each core handles one geometric range [1.2^i, 1.2^(i+1)]
    GeometricRanges = 0,
    /// Cores handle graph partitions (for very large graphs)
    GraphPartition = 1,
    /// Work stealing with dynamic assignment
    WorkStealing = 2,
}

/// Message types for inter-core communication (4 bytes)
#[derive(Clone, Copy)]
#[repr(C)]
pub struct CoreMessage {
    pub msg_type: u8,
    pub src_core: u8,
    pub payload: u16,
}

impl CoreMessage {
    pub const TYPE_IDLE: u8 = 0;
    pub const TYPE_WORK_REQUEST: u8 = 1;
    pub const TYPE_WORK_AVAILABLE: u8 = 2;
    pub const TYPE_RESULT: u8 = 3;
    pub const TYPE_SYNC: u8 = 4;
    pub const TYPE_STEAL_REQUEST: u8 = 5;
    pub const TYPE_STEAL_RESPONSE: u8 = 6;
}

/// Lock-free work queue entry
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct WorkItem {
    /// Range index to process
    pub range_idx: u16,
    /// Priority (lower = higher priority)
    pub priority: u8,
    /// Status
    pub status: u8,
}

impl WorkItem {
    pub const STATUS_PENDING: u8 = 0;
    pub const STATUS_IN_PROGRESS: u8 = 1;
    pub const STATUS_COMPLETE: u8 = 2;
}

/// Shared state for coordination (fits in shared memory)
#[repr(C, align(64))]
pub struct SharedCoordinator {
    /// Global minimum cut found so far
    pub global_min_cut: AtomicU16,
    /// Number of cores that have completed (u16 to support NUM_CORES=256)
    pub completed_cores: AtomicU16,
    /// Current phase
    pub phase: AtomicU8,
    /// Work queue head (for work stealing)
    pub queue_head: AtomicU16,
    /// Work queue tail
    pub queue_tail: AtomicU16,
    /// Best result core ID
    pub best_core: AtomicU8,
    /// Padding for alignment
    _pad: [u8; 52],
}

impl SharedCoordinator {
    pub const PHASE_INIT: u8 = 0;
    pub const PHASE_DISTRIBUTE: u8 = 1;
    pub const PHASE_COMPUTE: u8 = 2;
    pub const PHASE_COLLECT: u8 = 3;
    pub const PHASE_DONE: u8 = 4;

    pub fn new() -> Self {
        Self {
            global_min_cut: AtomicU16::new(u16::MAX),
            completed_cores: AtomicU16::new(0),
            phase: AtomicU8::new(Self::PHASE_INIT),
            queue_head: AtomicU16::new(0),
            queue_tail: AtomicU16::new(0),
            best_core: AtomicU8::new(0),
            _pad: [0; 52],
        }
    }

    /// Try to update global minimum (atomic compare-and-swap)
    pub fn try_update_min(&self, new_min: u16, core_id: u8) -> bool {
        let mut current = self.global_min_cut.load(Ordering::Acquire);
        loop {
            if new_min >= current {
                return false;
            }
            match self.global_min_cut.compare_exchange_weak(
                current,
                new_min,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.best_core.store(core_id, Ordering::Release);
                    return true;
                }
                Err(c) => current = c,
            }
        }
    }

    /// Mark core as completed
    pub fn mark_completed(&self) -> u16 {
        self.completed_cores.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Check if all cores completed
    pub fn all_completed(&self) -> bool {
        self.completed_cores.load(Ordering::Acquire) >= NUM_CORES as u16
    }
}

/// Compute range bounds for a core
#[inline]
pub fn compute_core_range(core_id: u8) -> (u16, u16) {
    let i = core_id as u32;
    let lambda_min = (RANGE_FACTOR.powi(i as i32)).floor() as u16;
    let lambda_max = (RANGE_FACTOR.powi((i + 1) as i32)).floor() as u16;
    (lambda_min.max(1), lambda_max.max(1))
}

/// Distribute graph across cores based on strategy
pub struct CoreDistributor {
    pub strategy: CoreStrategy,
    pub num_vertices: u16,
    pub num_edges: u16,
}

impl CoreDistributor {
    pub fn new(strategy: CoreStrategy, num_vertices: u16, num_edges: u16) -> Self {
        Self {
            strategy,
            num_vertices,
            num_edges,
        }
    }

    /// Determine which core should handle a vertex
    #[inline]
    pub fn vertex_to_core(&self, v: CompactVertexId) -> u8 {
        match self.strategy {
            CoreStrategy::GeometricRanges => {
                // All vertices go to all cores (replicated)
                0
            }
            CoreStrategy::GraphPartition => {
                // Partition by vertex ID
                ((v as u32 * NUM_CORES as u32) / self.num_vertices as u32) as u8
            }
            CoreStrategy::WorkStealing => {
                // Dynamic assignment
                0
            }
        }
    }

    /// Get the range of vertices for a core
    pub fn core_vertex_range(&self, core_id: u8) -> (CompactVertexId, CompactVertexId) {
        match self.strategy {
            CoreStrategy::GeometricRanges => (0, self.num_vertices),
            CoreStrategy::GraphPartition => {
                let n = self.num_vertices as u32;
                let start = (core_id as u32 * n) / NUM_CORES as u32;
                let end = ((core_id as u32 + 1) * n) / NUM_CORES as u32;
                (start as u16, end as u16)
            }
            CoreStrategy::WorkStealing => (0, self.num_vertices),
        }
    }
}

/// Per-core execution context
pub struct CoreExecutor<'a> {
    /// Core identifier (0-255)
    pub core_id: u8,
    /// Core state containing graph and witness data
    pub state: CompactCoreState,
    /// Reference to shared coordinator for cross-core synchronization
    pub coordinator: Option<&'a SharedCoordinator>,
}

impl<'a> CoreExecutor<'a> {
    /// Initialize core with its assigned range
    pub fn init(core_id: u8, coordinator: Option<&'a SharedCoordinator>) -> Self {
        let (lambda_min, lambda_max) = compute_core_range(core_id);

        let state = CompactCoreState {
            adjacency: Default::default(),
            edges: [CompactEdge::default(); MAX_EDGES_PER_CORE],
            num_vertices: 0,
            num_edges: 0,
            min_cut: u16::MAX,
            best_witness: CompactWitness::default(),
            lambda_min,
            lambda_max,
            core_id,
            status: CompactCoreState::STATUS_IDLE,
        };

        Self {
            core_id,
            state,
            coordinator,
        }
    }

    /// Add edge to this core's local graph
    pub fn add_edge(&mut self, src: CompactVertexId, tgt: CompactVertexId, weight: u16) {
        if self.state.num_edges as usize >= 512 {
            return; // Full
        }

        let idx = self.state.num_edges as usize;
        self.state.edges[idx] = CompactEdge {
            source: src,
            target: tgt,
            weight,
            flags: CompactEdge::FLAG_ACTIVE,
        };
        self.state.num_edges += 1;

        // Track vertices
        self.state.num_vertices = self.state.num_vertices.max(src + 1).max(tgt + 1);
    }

    /// Process this core's assigned range
    pub fn process(&mut self) -> CoreResult {
        self.state.status = CompactCoreState::STATUS_PROCESSING;

        // Simple minimum cut via minimum degree heuristic
        // (Full algorithm would use LocalKCut here)
        let mut min_degree = u16::MAX;
        let mut min_vertex = 0u16;

        for v in 0..self.state.num_vertices {
            let degree = self.compute_degree(v);
            if degree > 0 && degree < min_degree {
                min_degree = degree;
                min_vertex = v;
            }
        }

        // Check if in our range
        if min_degree >= self.state.lambda_min && min_degree <= self.state.lambda_max {
            self.state.min_cut = min_degree;

            // Create witness
            let mut membership = BitSet256::new();
            membership.insert(min_vertex);
            self.state.best_witness = CompactWitness::new(min_vertex, membership, min_degree);

            // Try to update global minimum
            if let Some(coord) = self.coordinator {
                coord.try_update_min(min_degree, self.core_id);
            }
        }

        self.state.status = CompactCoreState::STATUS_DONE;

        // Report result
        if let Some(coord) = self.coordinator {
            coord.mark_completed();
        }

        CoreResult {
            core_id: self.core_id,
            status: self.state.status,
            min_cut: self.state.min_cut,
            witness_hash: self.state.best_witness.hash,
            witness_seed: self.state.best_witness.seed,
            witness_cardinality: self.state.best_witness.cardinality,
            witness_boundary: self.state.best_witness.boundary_size,
            padding: [0; 4],
        }
    }

    /// Compute degree of a vertex
    fn compute_degree(&self, v: CompactVertexId) -> u16 {
        let mut degree = 0u16;
        for i in 0..self.state.num_edges as usize {
            let edge = &self.state.edges[i];
            if edge.is_active() && (edge.source == v || edge.target == v) {
                // Sum weights for weighted min-cut (not edge count)
                degree = degree.saturating_add(edge.weight);
            }
        }
        degree
    }

    /// SIMD-accelerated boundary computation for a vertex set
    ///
    /// Uses WASM SIMD128 when available for parallel edge checking
    #[inline]
    pub fn compute_boundary_simd(&self, set: &BitSet256) -> u16 {
        // Collect active edges as (source, target) pairs
        let edges: Vec<(CompactVertexId, CompactVertexId)> = self.state.edges
            [..self.state.num_edges as usize]
            .iter()
            .filter(|e| e.is_active())
            .map(|e| (e.source, e.target))
            .collect();

        // Use SIMD-accelerated boundary computation
        simd_boundary_size(set, &edges)
    }

    /// SIMD-accelerated population count for membership sets
    #[inline]
    pub fn membership_count_simd(&self, set: &BitSet256) -> u32 {
        simd_popcount(&set.bits)
    }
}

/// Result aggregator for collecting results from all cores
pub struct ResultAggregator {
    /// Results from each core
    pub results: [CoreResult; NUM_CORES],
    /// Index of the best result
    pub best_idx: usize,
    /// Global minimum cut value found
    pub global_min: u16,
}

impl ResultAggregator {
    /// Create a new result aggregator
    pub fn new() -> Self {
        Self {
            results: [CoreResult::default(); NUM_CORES],
            best_idx: 0,
            global_min: u16::MAX,
        }
    }

    /// Add a result from a core and update the best if needed
    pub fn add_result(&mut self, result: CoreResult) {
        let idx = result.core_id as usize;
        self.results[idx] = result;

        if result.min_cut < self.global_min {
            self.global_min = result.min_cut;
            self.best_idx = idx;
        }
    }

    /// Get the best result (lowest minimum cut)
    pub fn best_result(&self) -> &CoreResult {
        &self.results[self.best_idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_core_range() {
        let (min0, max0) = compute_core_range(0);
        assert_eq!(min0, 1);
        assert_eq!(max0, 1);

        let (min10, max10) = compute_core_range(10);
        assert_eq!(min10, 6);
        assert_eq!(max10, 7);
    }

    #[test]
    fn test_shared_coordinator() {
        let coord = SharedCoordinator::new();

        assert!(coord.try_update_min(100, 0));
        assert_eq!(coord.global_min_cut.load(Ordering::Acquire), 100);

        assert!(coord.try_update_min(50, 1));
        assert_eq!(coord.global_min_cut.load(Ordering::Acquire), 50);

        assert!(!coord.try_update_min(60, 2)); // 60 > 50
        assert_eq!(coord.global_min_cut.load(Ordering::Acquire), 50);
    }

    #[test]
    fn test_core_executor() {
        let coord = SharedCoordinator::new();
        let mut exec = CoreExecutor::init(0, Some(&coord));

        exec.add_edge(0, 1, 1);
        exec.add_edge(1, 2, 1);

        let result = exec.process();
        assert_eq!(result.core_id, 0);
    }

    #[test]
    fn test_result_aggregator() {
        let mut agg = ResultAggregator::new();

        agg.add_result(CoreResult {
            core_id: 0,
            min_cut: 100,
            ..Default::default()
        });

        agg.add_result(CoreResult {
            core_id: 1,
            min_cut: 50,
            ..Default::default()
        });

        assert_eq!(agg.global_min, 50);
        assert_eq!(agg.best_idx, 1);
    }
}
