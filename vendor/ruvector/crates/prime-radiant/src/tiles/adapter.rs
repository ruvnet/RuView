//! Tile adapter wrapping a single cognitum-gate-kernel tile.

use super::error::{TilesError, TilesResult};
use cognitum_gate_kernel::{
    delta::{Delta, Observation},
    report::{TileReport, TileStatus, WitnessFragment},
    TileState, MAX_DELTA_BUFFER,
};
use serde::{Deserialize, Serialize};

/// Configuration for a tile adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileAdapterConfig {
    /// Maximum deltas to buffer before processing.
    pub max_buffer_size: usize,
    /// Whether to auto-flush on buffer full.
    pub auto_flush: bool,
    /// Enable diagnostic logging.
    pub enable_diagnostics: bool,
}

impl Default for TileAdapterConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: MAX_DELTA_BUFFER,
            auto_flush: true,
            enable_diagnostics: false,
        }
    }
}

/// Adapter wrapping a single `cognitum_gate_kernel::TileState`.
///
/// This adapter provides a domain-specific interface for coherence computation,
/// translating between the coherence engine's concepts and the tile kernel's API.
pub struct TileAdapter {
    /// The underlying tile state.
    tile: TileState,
    /// Configuration.
    config: TileAdapterConfig,
    /// Count of processed ticks.
    ticks_processed: u64,
    /// Total deltas ingested.
    total_deltas: u64,
    /// Last report from the tile.
    last_report: Option<TileReport>,
}

impl TileAdapter {
    /// Create a new tile adapter with the given ID.
    ///
    /// # Arguments
    ///
    /// * `tile_id` - The tile identifier (0-255)
    /// * `config` - Configuration for the adapter
    ///
    /// # Errors
    ///
    /// Returns an error if the tile ID is out of range.
    pub fn new(tile_id: u8, config: TileAdapterConfig) -> TilesResult<Self> {
        Ok(Self {
            tile: TileState::new(tile_id),
            config,
            ticks_processed: 0,
            total_deltas: 0,
            last_report: None,
        })
    }

    /// Create a new tile adapter with default configuration.
    pub fn with_id(tile_id: u8) -> TilesResult<Self> {
        Self::new(tile_id, TileAdapterConfig::default())
    }

    /// Get the tile ID.
    #[inline]
    pub fn tile_id(&self) -> u8 {
        self.tile.tile_id
    }

    /// Get the current tick number.
    #[inline]
    pub fn current_tick(&self) -> u32 {
        self.tile.tick
    }

    /// Get the generation number (incremented on structural changes).
    #[inline]
    pub fn generation(&self) -> u16 {
        self.tile.generation
    }

    /// Check if the tile is initialized.
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.tile.status & TileState::STATUS_INITIALIZED != 0
    }

    /// Check if the tile has pending deltas.
    #[inline]
    pub fn has_pending_deltas(&self) -> bool {
        self.tile.has_pending_deltas()
    }

    /// Check if the tile is in error state.
    #[inline]
    pub fn is_error(&self) -> bool {
        self.tile.is_error()
    }

    /// Get the number of pending deltas.
    #[inline]
    pub fn pending_delta_count(&self) -> u16 {
        self.tile.delta_count
    }

    /// Ingest a state update for a node.
    ///
    /// This translates a coherence engine state update into a tile observation.
    /// Uses cut membership observation with confidence proportional to energy.
    pub fn ingest_state_update(&mut self, node_id: u64, energy: f32) -> TilesResult<()> {
        // Convert to cut membership observation with energy as confidence
        let confidence = ((energy.clamp(0.0, 1.0) * 65535.0) as u16).min(65535);
        let obs = Observation::cut_membership(node_id as u16, 0, confidence);
        let delta = Delta::observation(obs);
        self.ingest_delta(&delta)
    }

    /// Ingest an edge addition.
    pub fn ingest_edge_add(&mut self, source: u16, target: u16, weight: u16) -> TilesResult<()> {
        let delta = Delta::edge_add(source, target, weight);
        self.ingest_delta(&delta)
    }

    /// Ingest an edge removal.
    pub fn ingest_edge_remove(&mut self, source: u16, target: u16) -> TilesResult<()> {
        let delta = Delta::edge_remove(source, target);
        self.ingest_delta(&delta)
    }

    /// Ingest a weight update.
    pub fn ingest_weight_update(
        &mut self,
        source: u16,
        target: u16,
        new_weight: u16,
    ) -> TilesResult<()> {
        let delta = Delta::weight_update(source, target, new_weight);
        self.ingest_delta(&delta)
    }

    /// Ingest a connectivity observation.
    pub fn ingest_connectivity(&mut self, vertex: u16, connected: bool) -> TilesResult<()> {
        let obs = Observation::connectivity(vertex, connected);
        let delta = Delta::observation(obs);
        self.ingest_delta(&delta)
    }

    /// Ingest a raw delta.
    fn ingest_delta(&mut self, delta: &Delta) -> TilesResult<()> {
        if self.tile.delta_count as usize >= self.config.max_buffer_size {
            if self.config.auto_flush {
                // Auto-flush by running a tick
                self.tick(self.tile.tick)?;
            } else {
                return Err(TilesError::buffer_full(
                    self.tile.tile_id,
                    self.config.max_buffer_size,
                ));
            }
        }

        if !self.tile.ingest_delta(delta) {
            return Err(TilesError::buffer_full(self.tile.tile_id, MAX_DELTA_BUFFER));
        }

        self.total_deltas += 1;
        Ok(())
    }

    /// Execute one tick of the tile.
    ///
    /// This processes all buffered deltas, updates the evidence accumulator,
    /// recomputes graph connectivity if needed, and produces a report.
    pub fn tick(&mut self, tick_number: u32) -> TilesResult<TileReport> {
        if self.is_error() {
            return Err(TilesError::tile_error(
                self.tile.tile_id,
                "tile is in error state",
            ));
        }

        let report = self.tile.tick(tick_number);
        self.ticks_processed += 1;
        self.last_report = Some(report);

        if report.status == TileStatus::Error {
            return Err(TilesError::tile_error(
                self.tile.tile_id,
                "tick returned error status",
            ));
        }

        Ok(report)
    }

    /// Get the current witness fragment.
    #[inline]
    pub fn witness_fragment(&self) -> WitnessFragment {
        self.tile.get_witness_fragment()
    }

    /// Get the last report, if any.
    #[inline]
    pub fn last_report(&self) -> Option<&TileReport> {
        self.last_report.as_ref()
    }

    /// Get the log e-value from the evidence accumulator.
    #[inline]
    pub fn log_e_value(&self) -> f32 {
        self.tile.evidence.global_log_e as f32
    }

    /// Get the global e-value (exponentiated).
    #[inline]
    pub fn e_value(&self) -> f64 {
        self.tile.evidence.global_e_value() as f64
    }

    /// Get graph statistics.
    pub fn graph_stats(&self) -> GraphStats {
        GraphStats {
            num_vertices: self.tile.graph.num_vertices,
            num_edges: self.tile.graph.num_edges,
            num_components: self.tile.graph.num_components,
            is_connected: self.tile.graph.is_connected(),
        }
    }

    /// Get adapter statistics.
    pub fn adapter_stats(&self) -> AdapterStats {
        AdapterStats {
            tile_id: self.tile.tile_id,
            ticks_processed: self.ticks_processed,
            total_deltas: self.total_deltas,
            pending_deltas: self.tile.delta_count as u64,
            generation: self.tile.generation,
            log_e_value: self.log_e_value(),
        }
    }

    /// Reset the tile to initial state.
    pub fn reset(&mut self) {
        self.tile.reset();
        self.ticks_processed = 0;
        self.total_deltas = 0;
        self.last_report = None;
    }

    /// Mark a batch end to trigger recomputation.
    pub fn mark_batch_end(&mut self) -> TilesResult<()> {
        let delta = Delta::batch_end();
        self.ingest_delta(&delta)
    }
}

impl std::fmt::Debug for TileAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TileAdapter")
            .field("tile_id", &self.tile.tile_id)
            .field("tick", &self.tile.tick)
            .field("generation", &self.tile.generation)
            .field("ticks_processed", &self.ticks_processed)
            .field("total_deltas", &self.total_deltas)
            .finish()
    }
}

/// Graph statistics from a tile.
#[derive(Debug, Clone, Copy)]
pub struct GraphStats {
    /// Number of active vertices.
    pub num_vertices: u16,
    /// Number of edges.
    pub num_edges: u16,
    /// Number of connected components.
    pub num_components: u16,
    /// Whether the graph is fully connected.
    pub is_connected: bool,
}

/// Adapter statistics.
#[derive(Debug, Clone, Copy)]
pub struct AdapterStats {
    /// Tile ID.
    pub tile_id: u8,
    /// Total ticks processed.
    pub ticks_processed: u64,
    /// Total deltas ingested.
    pub total_deltas: u64,
    /// Currently pending deltas.
    pub pending_deltas: u64,
    /// Generation number.
    pub generation: u16,
    /// Current log e-value.
    pub log_e_value: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_adapter_creation() {
        let adapter = TileAdapter::with_id(42).unwrap();
        assert_eq!(adapter.tile_id(), 42);
        assert!(adapter.is_initialized());
        assert!(!adapter.is_error());
        assert!(!adapter.has_pending_deltas());
    }

    #[test]
    fn test_ingest_edge_and_tick() {
        let mut adapter = TileAdapter::with_id(0).unwrap();

        adapter.ingest_edge_add(0, 1, 100).unwrap();
        adapter.ingest_edge_add(1, 2, 100).unwrap();
        adapter.ingest_edge_add(2, 0, 100).unwrap();

        assert!(adapter.has_pending_deltas());

        let report = adapter.tick(1).unwrap();
        assert_eq!(report.tick, 1);
        assert_eq!(report.num_vertices, 3);
        assert_eq!(report.num_edges, 3);
        assert!(report.is_connected());
    }

    #[test]
    fn test_graph_stats() {
        let mut adapter = TileAdapter::with_id(0).unwrap();

        adapter.ingest_edge_add(0, 1, 100).unwrap();
        adapter.ingest_edge_add(2, 3, 100).unwrap();
        adapter.tick(1).unwrap();

        let stats = adapter.graph_stats();
        assert_eq!(stats.num_vertices, 4);
        assert_eq!(stats.num_edges, 2);
        assert_eq!(stats.num_components, 2);
        assert!(!stats.is_connected);
    }

    #[test]
    fn test_adapter_reset() {
        let mut adapter = TileAdapter::with_id(0).unwrap();

        adapter.ingest_edge_add(0, 1, 100).unwrap();
        adapter.tick(1).unwrap();

        adapter.reset();

        assert_eq!(adapter.current_tick(), 0);
        assert_eq!(adapter.generation(), 0);
        let stats = adapter.graph_stats();
        assert_eq!(stats.num_edges, 0);
    }
}
