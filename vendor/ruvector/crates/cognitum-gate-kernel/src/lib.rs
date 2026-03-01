//! Cognitum Gate Kernel
//!
//! A no_std WASM kernel for worker tiles in a 256-tile coherence gate fabric.
//! Each tile maintains a local graph shard, accumulates evidence for sequential
//! testing, and produces witness fragments for aggregation.
//!
//! # Architecture
//!
//! The coherence gate consists of 256 worker tiles, each running this kernel.
//! Tiles receive delta updates (edge additions, removals, weight changes) and
//! observations, process them through a deterministic tick loop, and produce
//! reports containing:
//!
//! - Local graph state (vertices, edges, components)
//! - Evidence accumulation (e-values for hypothesis testing)
//! - Witness fragments (for global min-cut aggregation)
//!
//! # Memory Budget
//!
//! Each tile operates within a ~64KB memory budget:
//! - CompactGraph: ~42KB (vertices, edges, adjacency)
//! - EvidenceAccumulator: ~2KB (hypotheses, sliding window)
//! - TileState: ~1KB (configuration, buffers)
//! - Stack/Control: ~19KB (remaining)
//!
//! # WASM Exports
//!
//! The kernel exports three main functions for the WASM interface:
//!
//! - `ingest_delta`: Process incoming delta updates
//! - `tick`: Execute one step of the deterministic tick loop
//! - `get_witness_fragment`: Retrieve the current witness fragment
//!
//! # Example
//!
//! ```ignore
//! // Initialize tile
//! let tile = TileState::new(42);  // Tile ID 42
//!
//! // Ingest deltas
//! tile.ingest_delta(&Delta::edge_add(0, 1, 100));
//! tile.ingest_delta(&Delta::edge_add(1, 2, 100));
//!
//! // Process tick
//! let report = tile.tick(1);
//!
//! // Get witness
//! let witness = tile.get_witness_fragment();
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![allow(clippy::missing_safety_doc)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Global allocator for no_std builds
#[cfg(all(not(feature = "std"), not(test)))]
mod allocator {
    use core::alloc::{GlobalAlloc, Layout};

    /// A simple bump allocator for no_std WASM builds
    /// In production, this would be replaced with wee_alloc or similar
    struct BumpAllocator;

    // 64KB heap for each tile
    const HEAP_SIZE: usize = 65536;
    static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    static mut HEAP_PTR: usize = 0;

    unsafe impl GlobalAlloc for BumpAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let size = layout.size();
            let align = layout.align();

            unsafe {
                // Align the heap pointer
                let aligned = (HEAP_PTR + align - 1) & !(align - 1);

                if aligned + size > HEAP_SIZE {
                    core::ptr::null_mut()
                } else {
                    HEAP_PTR = aligned + size;
                    HEAP.as_mut_ptr().add(aligned)
                }
            }
        }

        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
            // Bump allocator doesn't deallocate
            // This is fine for short-lived WASM kernels
        }
    }

    #[global_allocator]
    static ALLOCATOR: BumpAllocator = BumpAllocator;
}

// Panic handler for no_std builds (not needed for tests or std builds)
#[cfg(all(not(feature = "std"), not(test), target_arch = "wasm32"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // In WASM, we can use unreachable to trap
    core::arch::wasm32::unreachable()
}

// For non-wasm no_std builds without test
#[cfg(all(not(feature = "std"), not(test), not(target_arch = "wasm32")))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

pub mod delta;
pub mod evidence;
pub mod report;
pub mod shard;

#[cfg(feature = "canonical-witness")]
pub mod canonical_witness;

#[cfg(feature = "canonical-witness")]
pub use canonical_witness::{
    ArenaCactus, CactusNode, CanonicalPartition, CanonicalWitnessFragment, FixedPointWeight,
};

use crate::delta::{Delta, DeltaTag};
use crate::evidence::EvidenceAccumulator;
use crate::report::{TileReport, TileStatus, WitnessFragment};
use crate::shard::CompactGraph;
use core::mem::size_of;

/// Maximum deltas in ingestion buffer
pub const MAX_DELTA_BUFFER: usize = 64;

/// Tile state containing all local state for a worker tile
#[repr(C)]
pub struct TileState {
    /// Tile identifier (0-255)
    pub tile_id: u8,
    /// Status flags
    pub status: u8,
    /// Current tick number
    pub tick: u32,
    /// Generation number (incremented on structural changes)
    pub generation: u16,
    /// Reserved padding
    pub _reserved: [u8; 2],
    /// Local graph shard
    pub graph: CompactGraph,
    /// Evidence accumulator
    pub evidence: EvidenceAccumulator,
    /// Delta ingestion buffer
    pub delta_buffer: [Delta; MAX_DELTA_BUFFER],
    /// Number of deltas in buffer
    pub delta_count: u16,
    /// Buffer head pointer
    pub delta_head: u16,
    /// Last report produced
    pub last_report: TileReport,
}

impl TileState {
    /// Status: tile is initialized
    pub const STATUS_INITIALIZED: u8 = 0x01;
    /// Status: tile has pending deltas
    pub const STATUS_HAS_DELTAS: u8 = 0x02;
    /// Status: tile needs recomputation
    pub const STATUS_DIRTY: u8 = 0x04;
    /// Status: tile is in error state
    pub const STATUS_ERROR: u8 = 0x80;

    /// Create a new tile state
    pub fn new(tile_id: u8) -> Self {
        Self {
            tile_id,
            status: Self::STATUS_INITIALIZED,
            tick: 0,
            generation: 0,
            _reserved: [0; 2],
            graph: CompactGraph::new(),
            evidence: EvidenceAccumulator::new(),
            delta_buffer: [Delta::nop(); MAX_DELTA_BUFFER],
            delta_count: 0,
            delta_head: 0,
            last_report: TileReport::new(tile_id),
        }
    }

    /// Ingest a delta into the buffer
    ///
    /// Returns true if the delta was successfully buffered.
    /// Returns false if the buffer is full.
    pub fn ingest_delta(&mut self, delta: &Delta) -> bool {
        if self.delta_count as usize >= MAX_DELTA_BUFFER {
            return false;
        }

        let idx = (self.delta_head as usize + self.delta_count as usize) % MAX_DELTA_BUFFER;
        self.delta_buffer[idx] = *delta;
        self.delta_count += 1;
        self.status |= Self::STATUS_HAS_DELTAS;
        true
    }

    /// Ingest a delta from raw bytes
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` points to a valid `Delta` structure
    /// and that the pointer is properly aligned.
    #[inline]
    pub unsafe fn ingest_delta_raw(&mut self, ptr: *const u8) -> bool {
        let delta = unsafe { &*(ptr as *const Delta) };
        self.ingest_delta(delta)
    }

    /// Process one tick of the kernel
    ///
    /// This is the main entry point for the tick loop. It:
    /// 1. Processes all buffered deltas
    /// 2. Updates the evidence accumulator
    /// 3. Recomputes graph connectivity if needed
    /// 4. Produces a tile report
    pub fn tick(&mut self, tick_number: u32) -> TileReport {
        self.tick = tick_number;
        let tick_start = self.current_time_us();

        // Process buffered deltas
        let deltas_processed = self.process_deltas();

        // Recompute connectivity if graph is dirty
        if self.graph.status & CompactGraph::STATUS_DIRTY != 0 {
            self.graph.recompute_components();
        }

        // Build report
        let mut report = TileReport::new(self.tile_id);
        report.tick = tick_number;
        report.generation = self.generation;
        report.status = TileStatus::Complete;

        // Graph state
        report.num_vertices = self.graph.num_vertices;
        report.num_edges = self.graph.num_edges;
        report.num_components = self.graph.num_components;
        report.set_connected(self.graph.is_connected());

        if self.graph.status & CompactGraph::STATUS_DIRTY != 0 {
            report.graph_flags |= TileReport::GRAPH_DIRTY;
        }

        // Evidence state
        report.log_e_value = self.evidence.global_log_e;
        report.obs_count = self.evidence.total_obs as u16;
        report.rejected_count = self.evidence.rejected_count;

        // Witness fragment
        report.witness = self.compute_witness_fragment();

        // Performance metrics
        let tick_end = self.current_time_us();
        report.tick_time_us = (tick_end - tick_start) as u16;
        report.deltas_processed = deltas_processed as u16;
        report.memory_kb = (Self::memory_size() / 1024) as u16;

        self.last_report = report;
        report
    }

    /// Get the current witness fragment
    pub fn get_witness_fragment(&self) -> WitnessFragment {
        self.last_report.witness
    }

    /// Process all buffered deltas
    fn process_deltas(&mut self) -> usize {
        let mut processed = 0;

        while self.delta_count > 0 {
            let delta = self.delta_buffer[self.delta_head as usize];
            self.delta_head = ((self.delta_head as usize + 1) % MAX_DELTA_BUFFER) as u16;
            self.delta_count -= 1;

            self.apply_delta(&delta);
            processed += 1;
        }

        self.status &= !Self::STATUS_HAS_DELTAS;
        processed
    }

    /// Apply a single delta to the tile state
    fn apply_delta(&mut self, delta: &Delta) {
        match delta.tag {
            DeltaTag::Nop => {}
            DeltaTag::EdgeAdd => {
                let ea = unsafe { delta.get_edge_add() };
                self.graph.add_edge(ea.source, ea.target, ea.weight);
                self.generation = self.generation.wrapping_add(1);
            }
            DeltaTag::EdgeRemove => {
                let er = unsafe { delta.get_edge_remove() };
                self.graph.remove_edge(er.source, er.target);
                self.generation = self.generation.wrapping_add(1);
            }
            DeltaTag::WeightUpdate => {
                let wu = unsafe { delta.get_weight_update() };
                self.graph
                    .update_weight(wu.source, wu.target, wu.new_weight);
            }
            DeltaTag::Observation => {
                let obs = unsafe { *delta.get_observation() };
                self.evidence.process_observation(obs, self.tick);
            }
            DeltaTag::BatchEnd => {
                // Trigger recomputation
                self.status |= Self::STATUS_DIRTY;
            }
            DeltaTag::Checkpoint => {
                // TODO: Implement checkpointing
            }
            DeltaTag::Reset => {
                self.graph.clear();
                self.evidence.reset();
                self.generation = 0;
            }
        }
    }

    /// Compute the witness fragment for the current state
    fn compute_witness_fragment(&self) -> WitnessFragment {
        // Find the vertex with minimum degree (likely on cut boundary)
        let mut min_degree = u8::MAX;
        let mut seed = 0u16;

        for v in 0..shard::MAX_SHARD_VERTICES {
            if self.graph.vertices[v].is_active() {
                let degree = self.graph.vertices[v].degree;
                if degree < min_degree && degree > 0 {
                    min_degree = degree;
                    seed = v as u16;
                }
            }
        }

        // Count boundary vertices (vertices with edges to other tiles would be marked ghost)
        let mut boundary = 0u16;
        for v in 0..shard::MAX_SHARD_VERTICES {
            if self.graph.vertices[v].is_active()
                && (self.graph.vertices[v].flags & shard::VertexEntry::FLAG_BOUNDARY) != 0
            {
                boundary += 1;
            }
        }

        // Estimate local min cut as minimum vertex degree * average edge weight
        // This is a heuristic; actual min-cut requires more computation
        let local_min_cut = if min_degree == u8::MAX {
            0
        } else {
            // Average weight (assuming uniform for simplicity)
            min_degree as u16 * 100 // weight scale factor
        };

        let mut fragment =
            WitnessFragment::new(seed, boundary, self.graph.num_vertices, local_min_cut);
        fragment.component = self.graph.num_components;
        fragment.compute_hash();

        fragment
    }

    /// Get current time in microseconds (stub for no_std)
    #[inline]
    fn current_time_us(&self) -> u32 {
        // In actual WASM, this would call a host function
        // For now, return tick-based time
        self.tick * 1000
    }

    /// Get total memory size of tile state
    pub const fn memory_size() -> usize {
        size_of::<Self>()
    }

    /// Reset the tile to initial state
    pub fn reset(&mut self) {
        self.graph.clear();
        self.evidence.reset();
        self.delta_count = 0;
        self.delta_head = 0;
        self.tick = 0;
        self.generation = 0;
        self.status = Self::STATUS_INITIALIZED;
    }

    /// Check if tile has pending deltas
    #[inline]
    pub fn has_pending_deltas(&self) -> bool {
        self.delta_count > 0
    }

    /// Check if tile is in error state
    #[inline]
    pub fn is_error(&self) -> bool {
        self.status & Self::STATUS_ERROR != 0
    }

    /// Compute a canonical witness fragment for the current tile state.
    ///
    /// This produces a reproducible, hash-stable 16-byte witness by:
    /// 1. Building a cactus tree from the `CompactGraph`
    /// 2. Deriving a canonical (lex-smallest) min-cut partition
    /// 3. Packing the result into a `CanonicalWitnessFragment`
    ///
    /// Temporary stack usage: ~2.1KB (fits in the 14.5KB remaining headroom).
    #[cfg(feature = "canonical-witness")]
    pub fn canonical_witness(&self) -> canonical_witness::CanonicalWitnessFragment {
        let cactus = canonical_witness::ArenaCactus::build_from_compact_graph(&self.graph);
        let partition = cactus.canonical_partition();

        canonical_witness::CanonicalWitnessFragment {
            tile_id: self.tile_id,
            epoch: (self.tick & 0xFF) as u8,
            cardinality_a: partition.cardinality_a,
            cardinality_b: partition.cardinality_b,
            cut_value: cactus.min_cut_value.to_u16(),
            canonical_hash: partition.canonical_hash,
            boundary_edges: self.graph.num_edges,
            cactus_digest: cactus.digest(),
        }
    }
}

// ============================================================================
// WASM Exports
// ============================================================================

/// Global tile state (single tile per WASM instance)
static mut TILE_STATE: Option<TileState> = None;

/// Initialize the tile with the given ID
///
/// # Safety
///
/// This function modifies global state. It should only be called once
/// during module initialization.
#[no_mangle]
pub unsafe extern "C" fn init_tile(tile_id: u8) {
    unsafe {
        TILE_STATE = Some(TileState::new(tile_id));
    }
}

/// Ingest a delta from raw memory
///
/// # Safety
///
/// - `ptr` must point to a valid `Delta` structure
/// - The tile must be initialized
///
/// Returns 1 on success, 0 if buffer is full or tile not initialized.
#[no_mangle]
pub unsafe extern "C" fn ingest_delta(ptr: *const u8) -> i32 {
    unsafe {
        match TILE_STATE.as_mut() {
            Some(tile) => {
                if tile.ingest_delta_raw(ptr) {
                    1
                } else {
                    0
                }
            }
            None => 0,
        }
    }
}

/// Execute one tick of the kernel
///
/// # Safety
///
/// - `report_ptr` must point to a buffer of at least 64 bytes
/// - The tile must be initialized
///
/// Returns 1 on success, 0 if tile not initialized.
#[no_mangle]
pub unsafe extern "C" fn tick(tick_number: u32, report_ptr: *mut u8) -> i32 {
    unsafe {
        match TILE_STATE.as_mut() {
            Some(tile) => {
                let report = tile.tick(tick_number);
                // Copy report to output buffer
                let report_bytes =
                    core::slice::from_raw_parts(&report as *const TileReport as *const u8, 64);
                core::ptr::copy_nonoverlapping(report_bytes.as_ptr(), report_ptr, 64);
                1
            }
            None => 0,
        }
    }
}

/// Get the current witness fragment
///
/// # Safety
///
/// - `fragment_ptr` must point to a buffer of at least 16 bytes
/// - The tile must be initialized
///
/// Returns 1 on success, 0 if tile not initialized.
#[no_mangle]
pub unsafe extern "C" fn get_witness_fragment(fragment_ptr: *mut u8) -> i32 {
    unsafe {
        match TILE_STATE.as_ref() {
            Some(tile) => {
                let fragment = tile.get_witness_fragment();
                let fragment_bytes = core::slice::from_raw_parts(
                    &fragment as *const WitnessFragment as *const u8,
                    16,
                );
                core::ptr::copy_nonoverlapping(fragment_bytes.as_ptr(), fragment_ptr, 16);
                1
            }
            None => 0,
        }
    }
}

/// Get tile status
///
/// # Safety
///
/// The tile must be initialized.
///
/// Returns status byte, or 0xFF if not initialized.
#[no_mangle]
pub unsafe extern "C" fn get_status() -> u8 {
    unsafe {
        match TILE_STATE.as_ref() {
            Some(tile) => tile.status,
            None => 0xFF,
        }
    }
}

/// Reset the tile state
///
/// # Safety
///
/// The tile must be initialized.
#[no_mangle]
pub unsafe extern "C" fn reset_tile() {
    unsafe {
        if let Some(tile) = TILE_STATE.as_mut() {
            tile.reset();
        }
    }
}

/// Get memory usage in bytes
#[no_mangle]
pub extern "C" fn get_memory_usage() -> u32 {
    TileState::memory_size() as u32
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::Observation;

    #[test]
    fn test_tile_state_new() {
        let tile = TileState::new(42);
        assert_eq!(tile.tile_id, 42);
        assert_eq!(tile.tick, 0);
        assert_eq!(tile.delta_count, 0);
    }

    #[test]
    fn test_ingest_delta() {
        let mut tile = TileState::new(0);

        let delta = Delta::edge_add(1, 2, 100);
        assert!(tile.ingest_delta(&delta));
        assert_eq!(tile.delta_count, 1);
        assert!(tile.has_pending_deltas());
    }

    #[test]
    fn test_ingest_buffer_full() {
        let mut tile = TileState::new(0);

        // Fill buffer
        for i in 0..MAX_DELTA_BUFFER {
            let delta = Delta::edge_add(i as u16, (i + 1) as u16, 100);
            assert!(tile.ingest_delta(&delta));
        }

        // Should fail when full
        let delta = Delta::edge_add(100, 101, 100);
        assert!(!tile.ingest_delta(&delta));
    }

    #[test]
    fn test_tick_processes_deltas() {
        let mut tile = TileState::new(0);

        // Add some edges
        tile.ingest_delta(&Delta::edge_add(0, 1, 100));
        tile.ingest_delta(&Delta::edge_add(1, 2, 100));
        tile.ingest_delta(&Delta::edge_add(2, 0, 100));

        // Process tick
        let report = tile.tick(1);

        assert_eq!(report.tile_id, 0);
        assert_eq!(report.tick, 1);
        assert_eq!(report.status, TileStatus::Complete);
        assert_eq!(report.num_vertices, 3);
        assert_eq!(report.num_edges, 3);
        assert_eq!(report.deltas_processed, 3);
        assert!(!tile.has_pending_deltas());
    }

    #[test]
    fn test_tick_connectivity() {
        let mut tile = TileState::new(0);

        // Create a connected graph
        tile.ingest_delta(&Delta::edge_add(0, 1, 100));
        tile.ingest_delta(&Delta::edge_add(1, 2, 100));

        let report = tile.tick(1);
        assert!(report.is_connected());
        assert_eq!(report.num_components, 1);
    }

    #[test]
    fn test_tick_disconnected() {
        let mut tile = TileState::new(0);

        // Create two disconnected components
        tile.ingest_delta(&Delta::edge_add(0, 1, 100));
        tile.ingest_delta(&Delta::edge_add(2, 3, 100));

        let report = tile.tick(1);
        assert!(!report.is_connected());
        assert_eq!(report.num_components, 2);
    }

    #[test]
    fn test_observation_processing() {
        let mut tile = TileState::new(0);

        // Add hypothesis
        tile.evidence.add_connectivity_hypothesis(5);

        // Process observations
        for i in 0..5 {
            let obs = Observation::connectivity(5, true);
            tile.ingest_delta(&Delta::observation(obs));
            tile.tick(i);
        }

        assert!(tile.evidence.global_e_value() > 1.0);
    }

    #[test]
    fn test_witness_fragment() {
        let mut tile = TileState::new(0);

        tile.ingest_delta(&Delta::edge_add(0, 1, 100));
        tile.ingest_delta(&Delta::edge_add(1, 2, 100));
        tile.ingest_delta(&Delta::edge_add(2, 0, 100));

        tile.tick(1);
        let witness = tile.get_witness_fragment();

        assert!(!witness.is_empty());
        assert_eq!(witness.cardinality, 3);
        assert_ne!(witness.hash, 0);
    }

    #[test]
    fn test_reset() {
        let mut tile = TileState::new(0);

        tile.ingest_delta(&Delta::edge_add(0, 1, 100));
        tile.tick(1);

        assert_eq!(tile.graph.num_edges, 1);

        tile.reset();

        assert_eq!(tile.graph.num_edges, 0);
        assert_eq!(tile.graph.num_vertices, 0);
        assert_eq!(tile.tick, 0);
    }

    #[test]
    fn test_memory_size() {
        let size = TileState::memory_size();
        // Should fit in 64KB tile budget
        assert!(size <= 65536, "TileState exceeds 64KB: {} bytes", size);
    }

    #[test]
    fn test_edge_removal() {
        let mut tile = TileState::new(0);

        tile.ingest_delta(&Delta::edge_add(0, 1, 100));
        tile.ingest_delta(&Delta::edge_add(1, 2, 100));
        tile.tick(1);

        assert_eq!(tile.graph.num_edges, 2);

        tile.ingest_delta(&Delta::edge_remove(0, 1));
        tile.tick(2);

        assert_eq!(tile.graph.num_edges, 1);
    }

    #[test]
    fn test_weight_update() {
        let mut tile = TileState::new(0);

        tile.ingest_delta(&Delta::edge_add(0, 1, 100));
        tile.tick(1);

        assert_eq!(tile.graph.edge_weight(0, 1), Some(100));

        tile.ingest_delta(&Delta::weight_update(0, 1, 200));
        tile.tick(2);

        assert_eq!(tile.graph.edge_weight(0, 1), Some(200));
    }
}
