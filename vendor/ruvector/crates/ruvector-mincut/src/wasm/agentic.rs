//! Agentic chip interface for minimum cut
//!
//! Provides the external API for the 256-core WASM chip.

use crate::compact::*;
use crate::parallel::*;

/// External interface for agentic chip
/// This is the main entry point called from the chip's control plane
#[repr(C)]
pub struct AgenticMinCut {
    /// Shared coordinator for all cores
    pub coordinator: SharedCoordinator,
    /// Result aggregator
    pub aggregator: ResultAggregator,
    /// Distribution strategy
    pub strategy: CoreStrategy,
    /// Number of vertices in graph
    pub num_vertices: u16,
    /// Number of edges in graph
    pub num_edges: u16,
}

impl AgenticMinCut {
    /// Create new instance
    pub fn new() -> Self {
        Self {
            coordinator: SharedCoordinator::new(),
            aggregator: ResultAggregator::new(),
            strategy: CoreStrategy::GeometricRanges,
            num_vertices: 0,
            num_edges: 0,
        }
    }

    /// Initialize with graph size
    pub fn init(&mut self, num_vertices: u16, num_edges: u16, strategy: CoreStrategy) {
        self.num_vertices = num_vertices;
        self.num_edges = num_edges;
        self.strategy = strategy;
        self.coordinator = SharedCoordinator::new();
        self.aggregator = ResultAggregator::new();
    }

    /// Get coordinator pointer for cores
    pub fn coordinator_ptr(&self) -> *const SharedCoordinator {
        &self.coordinator
    }

    /// Add result from a core
    pub fn add_result(&mut self, result: CoreResult) {
        self.aggregator.add_result(result);
    }

    /// Get global minimum cut
    pub fn min_cut(&self) -> u16 {
        self.aggregator.global_min
    }

    /// Get best witness
    pub fn best_witness(&self) -> CompactWitness {
        let best = self.aggregator.best_result();
        CompactWitness {
            membership: BitSet256::new(), // Would need to fetch from core
            seed: best.witness_seed,
            boundary_size: best.witness_boundary,
            cardinality: best.witness_cardinality,
            hash: best.witness_hash,
        }
    }

    /// Check if computation is complete
    pub fn is_complete(&self) -> bool {
        self.coordinator.all_completed()
    }
}

impl Default for AgenticMinCut {
    fn default() -> Self {
        Self::new()
    }
}

/// FFI exports for WASM
#[cfg(target_arch = "wasm32")]
#[allow(missing_docs)]
pub mod ffi {
    use super::*;

    static mut INSTANCE: Option<AgenticMinCut> = None;

    /// Initialize the minimum cut computation with graph parameters.
    #[no_mangle]
    pub extern "C" fn mincut_init(num_vertices: u16, num_edges: u16, strategy: u8) {
        unsafe {
            let s = match strategy {
                0 => CoreStrategy::GeometricRanges,
                1 => CoreStrategy::GraphPartition,
                _ => CoreStrategy::WorkStealing,
            };

            let mut instance = AgenticMinCut::new();
            instance.init(num_vertices, num_edges, s);
            INSTANCE = Some(instance);
        }
    }

    /// Get a pointer to the shared coordinator for multi-core coordination.
    #[no_mangle]
    pub extern "C" fn mincut_get_coordinator() -> *const SharedCoordinator {
        unsafe {
            INSTANCE
                .as_ref()
                .map(|i| i.coordinator_ptr())
                .unwrap_or(core::ptr::null())
        }
    }

    /// Add a result from a completed core computation.
    #[no_mangle]
    pub extern "C" fn mincut_add_result(
        core_id: u8,
        status: u8,
        min_cut: u16,
        witness_hash: u16,
        witness_seed: u16,
        witness_cardinality: u16,
        witness_boundary: u16,
    ) {
        unsafe {
            if let Some(ref mut instance) = INSTANCE {
                instance.add_result(CoreResult {
                    core_id,
                    status,
                    min_cut,
                    witness_hash,
                    witness_seed,
                    witness_cardinality,
                    witness_boundary,
                    padding: [0; 4],
                });
            }
        }
    }

    /// Get the current minimum cut value.
    #[no_mangle]
    pub extern "C" fn mincut_get_result() -> u16 {
        unsafe { INSTANCE.as_ref().map(|i| i.min_cut()).unwrap_or(u16::MAX) }
    }

    /// Check if the computation is complete (returns 1 if complete, 0 otherwise).
    #[no_mangle]
    pub extern "C" fn mincut_is_complete() -> u8 {
        unsafe {
            INSTANCE
                .as_ref()
                .map(|i| i.is_complete() as u8)
                .unwrap_or(0)
        }
    }
}

/// RuVector integration
pub mod ruvector {
    use super::*;
    use std::collections::BTreeMap;

    /// Convert RuVector graph to compact representation
    pub fn from_ruvector_graph(
        vertices: &[u64],
        edges: &[(u64, u64, f32)],
    ) -> (
        Vec<CompactVertexId>,
        Vec<(CompactVertexId, CompactVertexId, u16)>,
    ) {
        // Create vertex ID mapping
        let mut vertex_map = BTreeMap::new();
        for (i, &v) in vertices.iter().enumerate() {
            vertex_map.insert(v, i as CompactVertexId);
        }

        // Convert edges
        let compact_edges: Vec<_> = edges
            .iter()
            .filter_map(|&(src, tgt, weight)| {
                let cs = vertex_map.get(&src)?;
                let ct = vertex_map.get(&tgt)?;
                let w = (weight * 100.0) as u16; // Fixed-point
                Some((*cs, *ct, w))
            })
            .collect();

        let compact_vertices: Vec<_> = (0..vertices.len() as u16).collect();

        (compact_vertices, compact_edges)
    }

    /// Compute minimum cut for RuVector graph
    pub fn compute_mincut(vertices: &[u64], edges: &[(u64, u64, f32)]) -> Option<(u16, Vec<u64>)> {
        let (compact_v, compact_e) = from_ruvector_graph(vertices, edges);

        if compact_v.len() > MAX_VERTICES_PER_CORE || compact_e.len() > 512 {
            // Too large for single core - would need multi-core
            return None;
        }

        // Create executor
        let coord = SharedCoordinator::new();
        let mut exec = CoreExecutor::init(0, Some(&coord));

        // Add edges
        for (src, tgt, weight) in compact_e {
            exec.add_edge(src, tgt, weight);
        }

        // Process
        let result = exec.process();

        if result.min_cut < u16::MAX {
            // Convert witness back to original vertex IDs
            let witness_vertices: Vec<u64> = vertices
                .iter()
                .enumerate()
                .filter(|(i, _)| exec.state.best_witness.contains(*i as u16))
                .map(|(_, &v)| v)
                .collect();

            Some((result.min_cut, witness_vertices))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agentic_mincut() {
        let mut amc = AgenticMinCut::new();
        amc.init(100, 200, CoreStrategy::GeometricRanges);

        assert_eq!(amc.num_vertices, 100);
        assert_eq!(amc.num_edges, 200);
        assert!(!amc.is_complete());
    }

    #[test]
    fn test_ruvector_conversion() {
        let vertices = vec![100u64, 200, 300];
        let edges = vec![(100u64, 200u64, 1.0f32), (200, 300, 1.0)];

        let (cv, ce) = ruvector::from_ruvector_graph(&vertices, &edges);

        assert_eq!(cv.len(), 3);
        assert_eq!(ce.len(), 2);
        assert_eq!(ce[0], (0, 1, 100)); // Fixed-point weight
    }

    #[test]
    fn test_ruvector_mincut() {
        let vertices: Vec<u64> = (0..5).collect();
        let edges = vec![(0u64, 1, 1.0f32), (1, 2, 1.0), (2, 3, 1.0), (3, 4, 1.0)];

        let result = ruvector::compute_mincut(&vertices, &edges);

        if let Some((min_cut, _witness)) = result {
            assert_eq!(min_cut, 100); // 1.0 * 100 fixed-point
        }
    }
}
