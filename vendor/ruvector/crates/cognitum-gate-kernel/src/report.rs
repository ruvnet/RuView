//! Tile report structures for coherence gate coordination
//!
//! Defines the 64-byte cache-line aligned report structure that tiles
//! produce after each tick. These reports are aggregated by the coordinator
//! to form witness fragments for the coherence gate.

#![allow(missing_docs)]

use crate::delta::TileVertexId;
use crate::evidence::LogEValue;
use core::mem::size_of;

/// Tile status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TileStatus {
    /// Tile is idle (no work)
    Idle = 0,
    /// Tile is processing deltas
    Processing = 1,
    /// Tile completed tick successfully
    Complete = 2,
    /// Tile encountered an error
    Error = 3,
    /// Tile is waiting for synchronization
    Waiting = 4,
    /// Tile is checkpointing
    Checkpointing = 5,
    /// Tile is recovering from checkpoint
    Recovering = 6,
    /// Tile is shutting down
    Shutdown = 7,
}

impl From<u8> for TileStatus {
    fn from(v: u8) -> Self {
        match v {
            0 => TileStatus::Idle,
            1 => TileStatus::Processing,
            2 => TileStatus::Complete,
            3 => TileStatus::Error,
            4 => TileStatus::Waiting,
            5 => TileStatus::Checkpointing,
            6 => TileStatus::Recovering,
            7 => TileStatus::Shutdown,
            _ => TileStatus::Error,
        }
    }
}

/// Witness fragment for aggregation
///
/// Compact representation of local cut/partition information
/// that can be merged across tiles.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(8))]
pub struct WitnessFragment {
    /// Seed vertex for this fragment
    pub seed: TileVertexId,
    /// Boundary size (cut edges crossing fragment)
    pub boundary_size: u16,
    /// Cardinality (vertices in fragment)
    pub cardinality: u16,
    /// Fragment hash for consistency checking
    pub hash: u16,
    /// Local minimum cut value (fixed-point)
    pub local_min_cut: u16,
    /// Component ID this fragment belongs to
    pub component: u16,
    /// Reserved padding
    pub _reserved: u16,
}

impl WitnessFragment {
    /// Create a new witness fragment
    #[inline]
    pub const fn new(
        seed: TileVertexId,
        boundary_size: u16,
        cardinality: u16,
        local_min_cut: u16,
    ) -> Self {
        Self {
            seed,
            boundary_size,
            cardinality,
            hash: 0,
            local_min_cut,
            component: 0,
            _reserved: 0,
        }
    }

    /// Compute fragment hash
    pub fn compute_hash(&mut self) {
        let mut h = self.seed as u32;
        h = h.wrapping_mul(31).wrapping_add(self.boundary_size as u32);
        h = h.wrapping_mul(31).wrapping_add(self.cardinality as u32);
        h = h.wrapping_mul(31).wrapping_add(self.local_min_cut as u32);
        self.hash = (h & 0xFFFF) as u16;
    }

    /// Check if fragment is empty
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.cardinality == 0
    }
}

/// Tile report produced after each tick (64 bytes, cache-line aligned)
///
/// This structure is designed to fit exactly in one cache line for
/// efficient memory access patterns in the coordinator.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct TileReport {
    // --- Header (8 bytes) ---
    /// Tile ID (0-255)
    pub tile_id: u8,
    /// Tile status
    pub status: TileStatus,
    /// Generation/epoch number
    pub generation: u16,
    /// Current tick number
    pub tick: u32,

    // --- Graph state (8 bytes) ---
    /// Number of active vertices
    pub num_vertices: u16,
    /// Number of active edges
    pub num_edges: u16,
    /// Number of connected components
    pub num_components: u16,
    /// Graph flags
    pub graph_flags: u16,

    // --- Evidence state (8 bytes) ---
    /// Global log e-value (tile-local)
    pub log_e_value: LogEValue,
    /// Number of observations processed
    pub obs_count: u16,
    /// Number of rejected hypotheses
    pub rejected_count: u16,

    // --- Witness fragment (16 bytes) ---
    /// Primary witness fragment
    pub witness: WitnessFragment,

    // --- Performance metrics (8 bytes) ---
    /// Delta processing time (microseconds)
    pub delta_time_us: u16,
    /// Tick processing time (microseconds)
    pub tick_time_us: u16,
    /// Deltas processed this tick
    pub deltas_processed: u16,
    /// Memory usage (KB)
    pub memory_kb: u16,

    // --- Cross-tile coordination (8 bytes) ---
    /// Number of ghost vertices
    pub ghost_vertices: u16,
    /// Number of ghost edges
    pub ghost_edges: u16,
    /// Boundary vertices (shared with other tiles)
    pub boundary_vertices: u16,
    /// Pending sync messages
    pub pending_sync: u16,

    // --- Reserved for future use (8 bytes) ---
    /// Reserved fields
    pub _reserved: [u8; 8],
}

impl Default for TileReport {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TileReport {
    /// Graph flag: graph is connected
    pub const GRAPH_CONNECTED: u16 = 0x0001;
    /// Graph flag: graph is dirty (needs recomputation)
    pub const GRAPH_DIRTY: u16 = 0x0002;
    /// Graph flag: graph is at capacity
    pub const GRAPH_FULL: u16 = 0x0004;
    /// Graph flag: graph has ghost edges
    pub const GRAPH_HAS_GHOSTS: u16 = 0x0008;

    /// Create a new report for a tile
    #[inline]
    pub const fn new(tile_id: u8) -> Self {
        Self {
            tile_id,
            status: TileStatus::Idle,
            generation: 0,
            tick: 0,
            num_vertices: 0,
            num_edges: 0,
            num_components: 0,
            graph_flags: 0,
            log_e_value: 0,
            obs_count: 0,
            rejected_count: 0,
            witness: WitnessFragment {
                seed: 0,
                boundary_size: 0,
                cardinality: 0,
                hash: 0,
                local_min_cut: 0,
                component: 0,
                _reserved: 0,
            },
            delta_time_us: 0,
            tick_time_us: 0,
            deltas_processed: 0,
            memory_kb: 0,
            ghost_vertices: 0,
            ghost_edges: 0,
            boundary_vertices: 0,
            pending_sync: 0,
            _reserved: [0; 8],
        }
    }

    /// Mark report as complete
    #[inline]
    pub fn set_complete(&mut self) {
        self.status = TileStatus::Complete;
    }

    /// Mark report as error
    #[inline]
    pub fn set_error(&mut self) {
        self.status = TileStatus::Error;
    }

    /// Set connected flag
    #[inline]
    pub fn set_connected(&mut self, connected: bool) {
        if connected {
            self.graph_flags |= Self::GRAPH_CONNECTED;
        } else {
            self.graph_flags &= !Self::GRAPH_CONNECTED;
        }
    }

    /// Check if graph is connected
    #[inline]
    pub const fn is_connected(&self) -> bool {
        self.graph_flags & Self::GRAPH_CONNECTED != 0
    }

    /// Check if graph is dirty
    #[inline]
    pub const fn is_dirty(&self) -> bool {
        self.graph_flags & Self::GRAPH_DIRTY != 0
    }

    /// Get e-value as approximate f32
    pub fn e_value_approx(&self) -> f32 {
        let log2_val = (self.log_e_value as f32) / 65536.0;
        libm::exp2f(log2_val)
    }

    /// Update witness fragment
    pub fn set_witness(&mut self, witness: WitnessFragment) {
        self.witness = witness;
    }

    /// Get the witness fragment
    #[inline]
    pub const fn get_witness(&self) -> &WitnessFragment {
        &self.witness
    }

    /// Check if tile has any rejections
    #[inline]
    pub const fn has_rejections(&self) -> bool {
        self.rejected_count > 0
    }

    /// Get processing rate (deltas per microsecond)
    pub fn processing_rate(&self) -> f32 {
        if self.tick_time_us == 0 {
            0.0
        } else {
            (self.deltas_processed as f32) / (self.tick_time_us as f32)
        }
    }
}

/// Report aggregator for combining multiple tile reports
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AggregatedReport {
    /// Total vertices across all tiles
    pub total_vertices: u32,
    /// Total edges across all tiles
    pub total_edges: u32,
    /// Total components across all tiles
    pub total_components: u16,
    /// Number of tiles reporting
    pub tiles_reporting: u16,
    /// Tiles with errors
    pub tiles_with_errors: u16,
    /// Tiles with rejections
    pub tiles_with_rejections: u16,
    /// Global log e-value (sum of tile e-values)
    pub global_log_e: i64,
    /// Minimum local cut across tiles
    pub global_min_cut: u16,
    /// Tile with minimum cut
    pub min_cut_tile: u8,
    /// Reserved padding
    pub _reserved: u8,
    /// Total processing time (microseconds)
    pub total_time_us: u32,
    /// Tick number
    pub tick: u32,
}

impl AggregatedReport {
    /// Create a new aggregated report
    pub const fn new(tick: u32) -> Self {
        Self {
            total_vertices: 0,
            total_edges: 0,
            total_components: 0,
            tiles_reporting: 0,
            tiles_with_errors: 0,
            tiles_with_rejections: 0,
            global_log_e: 0,
            global_min_cut: u16::MAX,
            min_cut_tile: 0,
            _reserved: 0,
            total_time_us: 0,
            tick,
        }
    }

    /// Merge a tile report into the aggregate
    pub fn merge(&mut self, report: &TileReport) {
        self.total_vertices += report.num_vertices as u32;
        self.total_edges += report.num_edges as u32;
        self.total_components += report.num_components;
        self.tiles_reporting += 1;

        if report.status == TileStatus::Error {
            self.tiles_with_errors += 1;
        }

        if report.rejected_count > 0 {
            self.tiles_with_rejections += 1;
        }

        self.global_log_e += report.log_e_value as i64;

        if report.witness.local_min_cut < self.global_min_cut {
            self.global_min_cut = report.witness.local_min_cut;
            self.min_cut_tile = report.tile_id;
        }

        self.total_time_us = self.total_time_us.max(report.tick_time_us as u32);
    }

    /// Check if all tiles completed successfully
    pub fn all_complete(&self, expected_tiles: u16) -> bool {
        self.tiles_reporting == expected_tiles && self.tiles_with_errors == 0
    }

    /// Get global e-value as approximate f64
    pub fn global_e_value(&self) -> f64 {
        let log2_val = (self.global_log_e as f64) / 65536.0;
        libm::exp2(log2_val)
    }
}

// Compile-time size assertions
const _: () = assert!(
    size_of::<TileReport>() == 64,
    "TileReport must be exactly 64 bytes"
);
const _: () = assert!(
    size_of::<WitnessFragment>() == 16,
    "WitnessFragment must be 16 bytes"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_report_size() {
        assert_eq!(size_of::<TileReport>(), 64);
    }

    #[test]
    fn test_tile_report_alignment() {
        assert_eq!(core::mem::align_of::<TileReport>(), 64);
    }

    #[test]
    fn test_witness_fragment_size() {
        assert_eq!(size_of::<WitnessFragment>(), 16);
    }

    #[test]
    fn test_new_report() {
        let report = TileReport::new(5);
        assert_eq!(report.tile_id, 5);
        assert_eq!(report.status, TileStatus::Idle);
        assert_eq!(report.tick, 0);
    }

    #[test]
    fn test_set_status() {
        let mut report = TileReport::new(0);
        report.set_complete();
        assert_eq!(report.status, TileStatus::Complete);

        report.set_error();
        assert_eq!(report.status, TileStatus::Error);
    }

    #[test]
    fn test_connected_flag() {
        let mut report = TileReport::new(0);
        assert!(!report.is_connected());

        report.set_connected(true);
        assert!(report.is_connected());

        report.set_connected(false);
        assert!(!report.is_connected());
    }

    #[test]
    fn test_witness_fragment() {
        let mut frag = WitnessFragment::new(10, 5, 20, 100);
        assert_eq!(frag.seed, 10);
        assert_eq!(frag.boundary_size, 5);
        assert_eq!(frag.cardinality, 20);
        assert_eq!(frag.local_min_cut, 100);

        frag.compute_hash();
        assert_ne!(frag.hash, 0);
    }

    #[test]
    fn test_aggregated_report() {
        let mut agg = AggregatedReport::new(1);

        let mut report1 = TileReport::new(0);
        report1.num_vertices = 50;
        report1.num_edges = 100;
        report1.witness.local_min_cut = 200;

        let mut report2 = TileReport::new(1);
        report2.num_vertices = 75;
        report2.num_edges = 150;
        report2.witness.local_min_cut = 150;

        agg.merge(&report1);
        agg.merge(&report2);

        assert_eq!(agg.tiles_reporting, 2);
        assert_eq!(agg.total_vertices, 125);
        assert_eq!(agg.total_edges, 250);
        assert_eq!(agg.global_min_cut, 150);
        assert_eq!(agg.min_cut_tile, 1);
    }

    #[test]
    fn test_tile_status_roundtrip() {
        for i in 0..=7 {
            let status = TileStatus::from(i);
            assert_eq!(status as u8, i);
        }
    }

    #[test]
    fn test_processing_rate() {
        let mut report = TileReport::new(0);
        report.deltas_processed = 100;
        report.tick_time_us = 50;

        assert!((report.processing_rate() - 2.0).abs() < 0.01);
    }
}
