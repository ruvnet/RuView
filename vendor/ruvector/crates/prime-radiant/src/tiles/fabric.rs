//! Coherence fabric managing all 256 tiles.

use super::adapter::{TileAdapter, TileAdapterConfig};
use super::coordinator::{AggregatedWitness, CoherenceSummary, CoordinatorConfig, TileCoordinator};
use super::error::{TilesError, TilesResult};
use cognitum_gate_kernel::report::TileReport;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Number of tiles in the fabric.
pub const NUM_TILES: usize = 256;

/// Configuration for the coherence fabric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricConfig {
    /// Tile adapter configuration.
    pub tile_config: TileAdapterConfig,
    /// Coordinator configuration.
    pub coordinator_config: CoordinatorConfig,
    /// Enable parallel tick processing.
    pub parallel_ticks: bool,
    /// Auto-aggregate witnesses after each tick.
    pub auto_aggregate: bool,
    /// Target tick rate (ticks per second, 0 = unlimited).
    pub target_tick_rate: u32,
}

impl Default for FabricConfig {
    fn default() -> Self {
        Self {
            tile_config: TileAdapterConfig::default(),
            coordinator_config: CoordinatorConfig::default(),
            parallel_ticks: true,
            auto_aggregate: true,
            target_tick_rate: 10000, // 10K ticks/sec target
        }
    }
}

/// State of the coherence fabric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabricState {
    /// Fabric is uninitialized.
    Uninitialized,
    /// Fabric is initialized and ready.
    Ready,
    /// Fabric is running (processing ticks).
    Running,
    /// Fabric is paused.
    Paused,
    /// Fabric is in error state.
    Error,
}

/// Report from a fabric tick.
#[derive(Debug, Clone)]
pub struct FabricReport {
    /// Tick number.
    pub tick: u32,
    /// Global energy (sum of tile energies).
    pub global_energy: f64,
    /// Aggregated witness from all tiles.
    pub global_witness: AggregatedWitness,
    /// Per-tile reports.
    pub tile_reports: Vec<TileReport>,
    /// Processing time in microseconds.
    pub processing_time_us: u64,
    /// Number of tiles that processed deltas.
    pub active_tiles: u16,
    /// Total deltas processed this tick.
    pub total_deltas: u32,
}

/// Coherence fabric using 256 WASM-style tiles.
///
/// This is the main entry point for distributed coherence computation.
/// It manages all 256 tiles, distributes updates, and aggregates results.
pub struct CoherenceFabric {
    /// All tiles.
    tiles: Vec<TileAdapter>,
    /// Coordinator for tile communication.
    coordinator: TileCoordinator,
    /// Configuration.
    config: FabricConfig,
    /// Current state.
    state: FabricState,
    /// Current tick number.
    current_tick: u32,
    /// Total ticks processed.
    total_ticks: u64,
}

impl CoherenceFabric {
    /// Create a new coherence fabric with the given configuration.
    pub fn new(config: FabricConfig) -> TilesResult<Self> {
        let mut tiles = Vec::with_capacity(NUM_TILES);

        for i in 0..NUM_TILES {
            let adapter = TileAdapter::new(i as u8, config.tile_config.clone())?;
            tiles.push(adapter);
        }

        let coordinator = TileCoordinator::new(config.coordinator_config.clone());

        Ok(Self {
            tiles,
            coordinator,
            config,
            state: FabricState::Ready,
            current_tick: 0,
            total_ticks: 0,
        })
    }

    /// Create with default configuration.
    pub fn default_fabric() -> TilesResult<Self> {
        Self::new(FabricConfig::default())
    }

    /// Get the current fabric state.
    #[inline]
    pub fn state(&self) -> FabricState {
        self.state
    }

    /// Get the current tick number.
    #[inline]
    pub fn current_tick(&self) -> u32 {
        self.current_tick
    }

    /// Get total ticks processed.
    #[inline]
    pub fn total_ticks(&self) -> u64 {
        self.total_ticks
    }

    /// Get the coordinator.
    pub fn coordinator(&self) -> &TileCoordinator {
        &self.coordinator
    }

    /// Get a tile by ID.
    pub fn tile(&self, tile_id: u8) -> Option<&TileAdapter> {
        self.tiles.get(tile_id as usize)
    }

    /// Get a mutable tile by ID.
    pub fn tile_mut(&mut self, tile_id: u8) -> Option<&mut TileAdapter> {
        self.tiles.get_mut(tile_id as usize)
    }

    /// Distribute a node state update to the appropriate tile.
    pub fn distribute_state_update(&mut self, node_id: u64, energy: f32) -> TilesResult<()> {
        let tile_id = self.coordinator.tile_for_node(node_id);
        let tile = self
            .tiles
            .get_mut(tile_id as usize)
            .ok_or(TilesError::TileIdOutOfRange(tile_id as u16))?;
        tile.ingest_state_update(node_id, energy)
    }

    /// Distribute an edge addition.
    pub fn distribute_edge_add(
        &mut self,
        source_node: u64,
        target_node: u64,
        weight: u16,
    ) -> TilesResult<()> {
        // Edges go to the tile of the source node
        let tile_id = self.coordinator.tile_for_node(source_node);
        let tile = self
            .tiles
            .get_mut(tile_id as usize)
            .ok_or(TilesError::TileIdOutOfRange(tile_id as u16))?;

        // Convert node IDs to local vertex IDs (truncate for now)
        let source_local = (source_node % 65536) as u16;
        let target_local = (target_node % 65536) as u16;

        tile.ingest_edge_add(source_local, target_local, weight)
    }

    /// Distribute an edge removal.
    pub fn distribute_edge_remove(
        &mut self,
        source_node: u64,
        target_node: u64,
    ) -> TilesResult<()> {
        let tile_id = self.coordinator.tile_for_node(source_node);
        let tile = self
            .tiles
            .get_mut(tile_id as usize)
            .ok_or(TilesError::TileIdOutOfRange(tile_id as u16))?;

        let source_local = (source_node % 65536) as u16;
        let target_local = (target_node % 65536) as u16;

        tile.ingest_edge_remove(source_local, target_local)
    }

    /// Execute one tick across all tiles.
    ///
    /// This is the main processing function that:
    /// 1. Processes all buffered deltas in each tile
    /// 2. Updates evidence accumulators
    /// 3. Recomputes graph connectivity
    /// 4. Aggregates witness fragments
    pub fn tick(&mut self, tick_number: u32) -> TilesResult<FabricReport> {
        if self.state == FabricState::Uninitialized {
            return Err(TilesError::FabricNotStarted);
        }

        let start = Instant::now();
        self.state = FabricState::Running;
        self.current_tick = tick_number;

        // Process all tiles (sequential for now, parallel later)
        let mut tile_reports = Vec::with_capacity(NUM_TILES);
        let mut active_tiles = 0u16;
        let mut total_deltas = 0u32;

        for tile in &mut self.tiles {
            let report = tile.tick(tick_number)?;
            if report.deltas_processed > 0 {
                active_tiles += 1;
                total_deltas += report.deltas_processed as u32;
            }
            tile_reports.push(report);
        }

        // Aggregate witnesses
        let global_witness = if self.config.auto_aggregate {
            self.coordinator.aggregate_witnesses(&self.tiles)?
        } else {
            AggregatedWitness::empty()
        };

        // Compute global energy
        let global_energy = self.coordinator.compute_global_energy(&self.tiles);

        let processing_time_us = start.elapsed().as_micros() as u64;
        self.total_ticks += 1;
        self.state = FabricState::Ready;

        Ok(FabricReport {
            tick: tick_number,
            global_energy,
            global_witness,
            tile_reports,
            processing_time_us,
            active_tiles,
            total_deltas,
        })
    }

    /// Execute multiple ticks in sequence.
    pub fn tick_n(&mut self, count: u32) -> TilesResult<Vec<FabricReport>> {
        let mut reports = Vec::with_capacity(count as usize);
        for i in 0..count {
            let report = self.tick(self.current_tick + i)?;
            reports.push(report);
        }
        Ok(reports)
    }

    /// Get coherence summary across all tiles.
    pub fn coherence_summary(&self) -> CoherenceSummary {
        self.coordinator.coherence_summary(&self.tiles)
    }

    /// Get the last aggregated witness.
    pub fn last_witness(&self) -> Option<&AggregatedWitness> {
        self.coordinator.last_witness()
    }

    /// Check if any tile has pending deltas.
    pub fn has_pending_deltas(&self) -> bool {
        self.tiles.iter().any(|t| t.has_pending_deltas())
    }

    /// Get the number of tiles with pending deltas.
    pub fn pending_delta_count(&self) -> usize {
        self.tiles.iter().filter(|t| t.has_pending_deltas()).count()
    }

    /// Reset all tiles to initial state.
    pub fn reset(&mut self) {
        for tile in &mut self.tiles {
            tile.reset();
        }
        self.coordinator.clear_cache();
        self.current_tick = 0;
        self.total_ticks = 0;
        self.state = FabricState::Ready;
    }

    /// Pause the fabric.
    pub fn pause(&mut self) {
        self.state = FabricState::Paused;
    }

    /// Resume the fabric.
    pub fn resume(&mut self) {
        if self.state == FabricState::Paused {
            self.state = FabricState::Ready;
        }
    }

    /// Get fabric statistics.
    pub fn stats(&self) -> FabricStats {
        let mut total_vertices = 0u32;
        let mut total_edges = 0u32;
        let mut tiles_with_data = 0u16;

        for tile in &self.tiles {
            let graph_stats = tile.graph_stats();
            if graph_stats.num_vertices > 0 {
                total_vertices += graph_stats.num_vertices as u32;
                total_edges += graph_stats.num_edges as u32;
                tiles_with_data += 1;
            }
        }

        FabricStats {
            total_tiles: NUM_TILES as u16,
            tiles_with_data,
            total_vertices,
            total_edges,
            total_ticks: self.total_ticks,
            current_tick: self.current_tick,
            state: self.state,
        }
    }
}

impl std::fmt::Debug for CoherenceFabric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoherenceFabric")
            .field("state", &self.state)
            .field("current_tick", &self.current_tick)
            .field("total_ticks", &self.total_ticks)
            .field("pending_tiles", &self.pending_delta_count())
            .finish()
    }
}

/// Fabric statistics.
#[derive(Debug, Clone, Copy)]
pub struct FabricStats {
    /// Total number of tiles.
    pub total_tiles: u16,
    /// Tiles with graph data.
    pub tiles_with_data: u16,
    /// Total vertices across all tiles.
    pub total_vertices: u32,
    /// Total edges across all tiles.
    pub total_edges: u32,
    /// Total ticks processed.
    pub total_ticks: u64,
    /// Current tick number.
    pub current_tick: u32,
    /// Current fabric state.
    pub state: FabricState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fabric_creation() {
        let fabric = CoherenceFabric::default_fabric().unwrap();
        assert_eq!(fabric.state(), FabricState::Ready);
        assert_eq!(fabric.current_tick(), 0);
    }

    #[test]
    fn test_fabric_tick_empty() {
        let mut fabric = CoherenceFabric::default_fabric().unwrap();
        let report = fabric.tick(1).unwrap();
        assert_eq!(report.tick, 1);
        assert_eq!(report.active_tiles, 0);
    }

    #[test]
    fn test_fabric_distribute_and_tick() {
        let mut fabric = CoherenceFabric::default_fabric().unwrap();

        // Add some edges
        fabric.distribute_edge_add(0, 1, 100).unwrap();
        fabric.distribute_edge_add(1, 2, 100).unwrap();

        assert!(fabric.has_pending_deltas());

        let report = fabric.tick(1).unwrap();
        assert!(report.active_tiles > 0);
        assert!(report.total_deltas > 0);
    }

    #[test]
    fn test_fabric_reset() {
        let mut fabric = CoherenceFabric::default_fabric().unwrap();

        fabric.distribute_edge_add(0, 1, 100).unwrap();
        fabric.tick(1).unwrap();

        fabric.reset();

        assert_eq!(fabric.current_tick(), 0);
        assert_eq!(fabric.total_ticks(), 0);
        assert!(!fabric.has_pending_deltas());
    }

    #[test]
    fn test_fabric_stats() {
        let fabric = CoherenceFabric::default_fabric().unwrap();
        let stats = fabric.stats();

        assert_eq!(stats.total_tiles, 256);
        assert_eq!(stats.state, FabricState::Ready);
    }
}
