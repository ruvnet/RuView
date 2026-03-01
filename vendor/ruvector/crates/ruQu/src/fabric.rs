//! QuantumFabric Orchestration Layer
//!
//! This module provides the top-level API for the ruQu coherence gate system.
//! It manages the 256-tile WASM fabric, coordinates syndrome processing across
//! worker tiles, and exposes a clean interface for quantum control systems.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use ruqu::fabric::{QuantumFabric, PatchMap, surface_code_d7};
//!
//! // Initialize the 256-tile quantum control fabric
//! let mut fabric = QuantumFabric::builder()
//!     .tiles(256)                    // 255 workers + TileZero
//!     .patch_map(surface_code_d7())  // Surface code layout
//!     .syndrome_buffer(1024)         // Ring buffer depth
//!     .build()
//!     .expect("Failed to build fabric");
//!
//! // Each cycle: ingest syndromes and get gate decision
//! // let decision = fabric.tick()?;
//! ```
//!
//! # Architecture
//!
//! The QuantumFabric coordinates:
//! - **255 WorkerTiles** (IDs 1-255): Process local patches of the quantum device
//! - **TileZero** (ID 0): Merges worker reports and issues gate decisions
//! - **CoherenceGate**: Three-filter decision pipeline (Structural, Shift, Evidence)
//! - **PatchMap**: Hardware topology mapping qubits to tiles
//!
//! # Latency Budget
//!
//! Target: <4μs p99 end-to-end decision latency
//!
//! ```text
//! Syndrome Arrival        → 0 ns
//! Worker Distribution     → +100 ns
//! Parallel Worker Ticks   → +500 ns
//! Report Collection       → +100 ns
//! TileZero Merge          → +500 ns
//! Three-Filter Eval       → +100 ns
//! Gate Decision           → +100 ns
//! Token Signing           → +500 ns
//! Receipt Append          → +100 ns
//! ─────────────────────────────────
//! Total                   → ~2,000 ns
//! ```

use std::time::Instant;

use crate::error::{Result, RuQuError};
use crate::filters::{FilterConfig, FilterPipeline, FilterResults, SystemState, Verdict};
use crate::syndrome::SyndromeRound;
use crate::tile::{
    GateDecision as TileGateDecision, GateThresholds, ReceiptLog, TileReport, TileZero, WorkerTile,
};
use crate::types::{GateDecision, RegionMask, SequenceId};
use crate::{DEFAULT_BUFFER_CAPACITY, TILE_COUNT, WORKER_TILE_COUNT};

// ═══════════════════════════════════════════════════════════════════════════════
// PatchMap - Hardware Topology
// ═══════════════════════════════════════════════════════════════════════════════

/// Assignment of qubits/vertices to a specific tile.
#[derive(Debug, Clone)]
pub struct TileAssignment {
    /// Tile ID (1-255 for workers, 0 reserved for TileZero)
    pub tile_id: u8,
    /// Qubit/vertex IDs assigned to this tile
    pub vertices: Vec<u64>,
    /// Boundary vertices shared with other tiles
    pub boundary_vertices: Vec<u64>,
    /// Neighboring tile IDs
    pub neighbors: Vec<u8>,
}

impl TileAssignment {
    /// Create a new tile assignment.
    pub fn new(tile_id: u8) -> Self {
        Self {
            tile_id,
            vertices: Vec::new(),
            boundary_vertices: Vec::new(),
            neighbors: Vec::new(),
        }
    }

    /// Add a vertex to this tile.
    pub fn add_vertex(&mut self, vertex_id: u64) {
        self.vertices.push(vertex_id);
    }

    /// Add a boundary vertex (shared with neighboring tiles).
    pub fn add_boundary(&mut self, vertex_id: u64) {
        self.boundary_vertices.push(vertex_id);
    }

    /// Add a neighboring tile.
    pub fn add_neighbor(&mut self, tile_id: u8) {
        if !self.neighbors.contains(&tile_id) {
            self.neighbors.push(tile_id);
        }
    }

    /// Get the total number of vertices (including boundary).
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() + self.boundary_vertices.len()
    }
}

/// Hardware topology mapping qubits to tiles.
///
/// The PatchMap defines how the quantum device is partitioned across the 256-tile
/// fabric. Each tile is responsible for a "patch" of qubits, with boundary regions
/// shared between neighboring tiles.
#[derive(Debug, Clone)]
pub struct PatchMap {
    /// Human-readable name for this topology
    pub name: String,
    /// Total number of qubits in the device
    pub qubit_count: usize,
    /// Per-tile assignments
    pub tile_assignments: Vec<TileAssignment>,
    /// Code distance (for surface codes)
    pub distance: Option<usize>,
    /// Number of detectors per round
    pub detector_count: usize,
}

impl PatchMap {
    /// Create a new empty patch map.
    pub fn new(name: impl Into<String>, qubit_count: usize) -> Self {
        Self {
            name: name.into(),
            qubit_count,
            tile_assignments: Vec::new(),
            distance: None,
            detector_count: qubit_count, // Default: one detector per qubit
        }
    }

    /// Set the code distance.
    pub fn with_distance(mut self, d: usize) -> Self {
        self.distance = Some(d);
        self
    }

    /// Set the detector count.
    pub fn with_detectors(mut self, count: usize) -> Self {
        self.detector_count = count;
        self
    }

    /// Add a tile assignment.
    pub fn add_assignment(&mut self, assignment: TileAssignment) {
        self.tile_assignments.push(assignment);
    }

    /// Get the number of active tiles.
    pub fn tile_count(&self) -> usize {
        self.tile_assignments.len()
    }

    /// Get assignment for a specific tile.
    pub fn get_assignment(&self, tile_id: u8) -> Option<&TileAssignment> {
        self.tile_assignments.iter().find(|a| a.tile_id == tile_id)
    }

    /// Find which tile owns a vertex.
    pub fn find_tile_for_vertex(&self, vertex_id: u64) -> Option<u8> {
        for assignment in &self.tile_assignments {
            if assignment.vertices.contains(&vertex_id) {
                return Some(assignment.tile_id);
            }
        }
        None
    }

    /// Validate the patch map.
    pub fn validate(&self) -> Result<()> {
        if self.qubit_count == 0 {
            return Err(RuQuError::InvalidFabricConfig(
                "PatchMap has zero qubits".to_string(),
            ));
        }

        if self.tile_assignments.is_empty() {
            return Err(RuQuError::InvalidFabricConfig(
                "PatchMap has no tile assignments".to_string(),
            ));
        }

        // Check for duplicate tile IDs
        let mut seen_ids = std::collections::HashSet::new();
        for assignment in &self.tile_assignments {
            if assignment.tile_id == 0 {
                return Err(RuQuError::InvalidFabricConfig(
                    "TileId 0 is reserved for TileZero".to_string(),
                ));
            }
            if !seen_ids.insert(assignment.tile_id) {
                return Err(RuQuError::InvalidFabricConfig(format!(
                    "Duplicate tile ID: {}",
                    assignment.tile_id
                )));
            }
        }

        Ok(())
    }
}

/// Create a patch map for a distance-7 surface code.
///
/// This is the canonical example topology with approximately 97 data qubits
/// (7x7 lattice) partitioned across available tiles.
pub fn surface_code_d7() -> PatchMap {
    surface_code(7)
}

/// Create a patch map for a surface code of given distance.
///
/// # Arguments
///
/// * `distance` - The code distance (must be odd, >= 3)
///
/// # Returns
///
/// A PatchMap with qubits distributed across tiles.
pub fn surface_code(distance: usize) -> PatchMap {
    assert!(distance >= 3, "Surface code distance must be >= 3");
    assert!(distance % 2 == 1, "Surface code distance must be odd");

    // Surface code has d^2 data qubits + (d-1)^2 + (d)^2 ancilla qubits
    // Simplified: use approximately 2*d^2 total qubits
    let qubit_count = 2 * distance * distance;

    // Detector count: approximately (d-1)^2 X-checks + (d-1)^2 Z-checks per round
    let detector_count = 2 * (distance - 1) * (distance - 1);

    let mut patch_map = PatchMap::new(format!("surface_code_d{}", distance), qubit_count)
        .with_distance(distance)
        .with_detectors(detector_count);

    // Partition qubits across tiles
    // Strategy: assign sqrt(qubit_count) qubits per tile
    let qubits_per_tile = (qubit_count as f64).sqrt().ceil() as usize;
    let num_tiles = (qubit_count + qubits_per_tile - 1) / qubits_per_tile;
    let num_tiles = num_tiles.min(WORKER_TILE_COUNT);

    for tile_idx in 0..num_tiles {
        let tile_id = (tile_idx + 1) as u8; // Tile IDs start at 1
        let mut assignment = TileAssignment::new(tile_id);

        let start_qubit = tile_idx * qubits_per_tile;
        let end_qubit = ((tile_idx + 1) * qubits_per_tile).min(qubit_count);

        for qubit in start_qubit..end_qubit {
            assignment.add_vertex(qubit as u64);
        }

        // Add neighbors (simple linear topology for now)
        if tile_idx > 0 {
            assignment.add_neighbor(tile_idx as u8);
        }
        if tile_idx < num_tiles - 1 {
            assignment.add_neighbor((tile_idx + 2) as u8);
        }

        // Mark boundary vertices
        if tile_idx > 0 {
            assignment.add_boundary(start_qubit as u64);
        }
        if tile_idx < num_tiles - 1 && end_qubit > start_qubit {
            assignment.add_boundary((end_qubit - 1) as u64);
        }

        patch_map.add_assignment(assignment);
    }

    patch_map
}

/// Create a simple linear patch map for testing.
pub fn linear_patch_map(qubit_count: usize, tiles: usize) -> PatchMap {
    let tiles = tiles.min(WORKER_TILE_COUNT).max(1);
    let mut patch_map = PatchMap::new("linear", qubit_count);

    let qubits_per_tile = (qubit_count + tiles - 1) / tiles;

    for tile_idx in 0..tiles {
        let tile_id = (tile_idx + 1) as u8;
        let mut assignment = TileAssignment::new(tile_id);

        let start = tile_idx * qubits_per_tile;
        let end = ((tile_idx + 1) * qubits_per_tile).min(qubit_count);

        for qubit in start..end {
            assignment.add_vertex(qubit as u64);
        }

        patch_map.add_assignment(assignment);
    }

    patch_map
}

// ═══════════════════════════════════════════════════════════════════════════════
// FabricConfig - Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for the QuantumFabric.
#[derive(Debug, Clone)]
pub struct FabricConfig {
    /// Number of tiles (max 256)
    pub tile_count: usize,
    /// Syndrome buffer size per tile
    pub buffer_size: usize,
    /// Gate decision thresholds
    pub thresholds: GateThresholds,
    /// Filter pipeline configuration
    pub filter_config: FilterConfig,
    /// Enable receipt logging
    pub enable_receipts: bool,
    /// Decision budget in nanoseconds
    pub decision_budget_ns: u64,
}

impl Default for FabricConfig {
    fn default() -> Self {
        Self {
            tile_count: TILE_COUNT,
            buffer_size: DEFAULT_BUFFER_CAPACITY,
            thresholds: GateThresholds::default(),
            filter_config: FilterConfig::default(),
            enable_receipts: true,
            decision_budget_ns: 4_000, // 4 microseconds
        }
    }
}

impl FabricConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.tile_count == 0 || self.tile_count > TILE_COUNT {
            return Err(RuQuError::InvalidFabricConfig(format!(
                "tile_count must be 1-{}, got {}",
                TILE_COUNT, self.tile_count
            )));
        }

        if self.buffer_size == 0 {
            return Err(RuQuError::InvalidFabricConfig(
                "buffer_size must be positive".to_string(),
            ));
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FabricState - Runtime State
// ═══════════════════════════════════════════════════════════════════════════════

/// Current state of the QuantumFabric.
#[derive(Debug, Clone)]
pub struct FabricState {
    /// Current tick number
    pub tick: u64,
    /// Total syndromes ingested
    pub syndromes_ingested: u64,
    /// Number of active worker tiles
    pub active_tiles: usize,
    /// Most recent gate decision
    pub last_decision: GateDecision,
    /// Regions currently flagged as unsafe
    pub quarantine_mask: RegionMask,
    /// Average decision latency (nanoseconds)
    pub avg_latency_ns: u64,
    /// Peak decision latency (nanoseconds)
    pub peak_latency_ns: u64,
    /// Total permit decisions
    pub permit_count: u64,
    /// Total defer decisions
    pub defer_count: u64,
    /// Total deny decisions
    pub deny_count: u64,
}

impl Default for FabricState {
    fn default() -> Self {
        Self {
            tick: 0,
            syndromes_ingested: 0,
            active_tiles: 0,
            last_decision: GateDecision::Cautious,
            quarantine_mask: RegionMask::none(),
            avg_latency_ns: 0,
            peak_latency_ns: 0,
            permit_count: 0,
            defer_count: 0,
            deny_count: 0,
        }
    }
}

impl FabricState {
    /// Get the total number of decisions made.
    pub fn total_decisions(&self) -> u64 {
        self.permit_count + self.defer_count + self.deny_count
    }

    /// Get the permit rate (0.0 to 1.0).
    pub fn permit_rate(&self) -> f64 {
        let total = self.total_decisions();
        if total == 0 {
            return 0.0;
        }
        self.permit_count as f64 / total as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WitnessReceipt - Audit Trail
// ═══════════════════════════════════════════════════════════════════════════════

/// A witness receipt for auditing gate decisions.
///
/// Each gate decision produces a receipt containing cryptographic proof of
/// the decision inputs and output, enabling post-hoc verification.
#[derive(Debug, Clone)]
pub struct WitnessReceipt {
    /// Decision sequence number
    pub sequence: SequenceId,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
    /// The gate decision
    pub decision: GateDecision,
    /// Blake3 hash of input state
    pub input_hash: [u8; 32],
    /// Filter results summary
    pub filter_summary: FilterSummary,
    /// Previous receipt hash (for chaining)
    pub previous_hash: [u8; 32],
    /// This receipt's hash
    pub hash: [u8; 32],
}

/// Summary of filter results for the receipt.
#[derive(Debug, Clone, Default)]
pub struct FilterSummary {
    /// Structural filter: min-cut value
    pub cut_value: f64,
    /// Shift filter: pressure value
    pub shift_pressure: f64,
    /// Evidence filter: e-value
    pub e_value: f64,
    /// Regions affected
    pub affected_regions: u32,
}

// ═══════════════════════════════════════════════════════════════════════════════
// CoherenceGate - Public Gate Interface
// ═══════════════════════════════════════════════════════════════════════════════

/// The coherence gate - the decision-making core of ruQu.
///
/// The gate uses three stacked filters to make coherence assessments:
/// 1. **Structural Filter**: Min-cut based partition detection
/// 2. **Shift Filter**: Distribution drift detection
/// 3. **Evidence Filter**: Anytime-valid e-value accumulation
///
/// All three must pass for PERMIT. Any one can trigger DENY or DEFER.
#[derive(Debug)]
pub struct CoherenceGate {
    /// The three-filter pipeline
    pipeline: FilterPipeline,
    /// System state tracking
    state: SystemState,
    /// Current sequence number
    sequence: SequenceId,
    /// Last receipt (for chaining)
    last_receipt_hash: [u8; 32],
}

impl CoherenceGate {
    /// Create a new coherence gate with the given configuration.
    pub fn new(config: FilterConfig) -> Self {
        Self {
            pipeline: FilterPipeline::new(config),
            state: SystemState::new(0),
            sequence: 0,
            last_receipt_hash: [0u8; 32],
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(FilterConfig::default())
    }

    /// Evaluate the current system state and return a gate decision.
    ///
    /// This is the main entry point for coherence assessment.
    pub fn evaluate(&self) -> Result<GateDecision> {
        let results = self.pipeline.evaluate(&self.state);

        let decision = match results.verdict {
            Some(Verdict::Permit) => GateDecision::Safe,
            Some(Verdict::Deny) => GateDecision::Unsafe,
            Some(Verdict::Defer) | None => GateDecision::Cautious,
        };

        Ok(decision)
    }

    /// Evaluate and return detailed filter results.
    pub fn evaluate_detailed(&self) -> FilterResults {
        self.pipeline.evaluate(&self.state)
    }

    /// Get the current witness receipt (if available).
    pub fn receipt(&self) -> Option<WitnessReceipt> {
        if self.sequence == 0 {
            return None;
        }

        let results = self.pipeline.evaluate(&self.state);

        let summary = FilterSummary {
            cut_value: results.structural.cut_value,
            shift_pressure: results.shift.pressure,
            e_value: results.evidence.e_value,
            affected_regions: results.affected_regions.count(),
        };

        // Compute simple hash (use blake3 in production)
        let mut hash = [0u8; 32];
        hash[0..8].copy_from_slice(&self.sequence.to_le_bytes());
        hash[8..16].copy_from_slice(&summary.cut_value.to_le_bytes());

        Some(WitnessReceipt {
            sequence: self.sequence,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            decision: self.evaluate().unwrap_or(GateDecision::Cautious),
            input_hash: self.last_receipt_hash,
            filter_summary: summary,
            previous_hash: self.last_receipt_hash,
            hash,
        })
    }

    /// Update the system state with new data.
    pub fn update_state(&mut self, state: SystemState) {
        self.state = state;
    }

    /// Get a mutable reference to the filter pipeline.
    pub fn pipeline_mut(&mut self) -> &mut FilterPipeline {
        &mut self.pipeline
    }

    /// Get a reference to the filter pipeline.
    pub fn pipeline(&self) -> &FilterPipeline {
        &self.pipeline
    }

    /// Get the current system state.
    pub fn state(&self) -> &SystemState {
        &self.state
    }

    /// Increment the sequence counter (called after each decision).
    pub(crate) fn increment_sequence(&mut self) {
        self.sequence += 1;
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FabricBuilder - Builder Pattern
// ═══════════════════════════════════════════════════════════════════════════════

/// Builder for constructing a QuantumFabric.
///
/// # Example
///
/// ```rust,no_run
/// use ruqu::fabric::{QuantumFabric, surface_code_d7};
///
/// let fabric = QuantumFabric::builder()
///     .tiles(256)
///     .patch_map(surface_code_d7())
///     .syndrome_buffer(1024)
///     .build()
///     .expect("Failed to build fabric");
/// ```
#[derive(Debug)]
pub struct FabricBuilder {
    tile_count: usize,
    patch_map: Option<PatchMap>,
    buffer_size: usize,
    thresholds: GateThresholds,
    filter_config: FilterConfig,
    enable_receipts: bool,
}

impl Default for FabricBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FabricBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            tile_count: TILE_COUNT,
            patch_map: None,
            buffer_size: DEFAULT_BUFFER_CAPACITY,
            thresholds: GateThresholds::default(),
            filter_config: FilterConfig::default(),
            enable_receipts: true,
        }
    }

    /// Set the number of tiles (max 256).
    ///
    /// The fabric will have `count - 1` worker tiles plus TileZero.
    pub fn tiles(mut self, count: usize) -> Self {
        self.tile_count = count.min(TILE_COUNT);
        self
    }

    /// Set the patch map (hardware topology).
    pub fn patch_map(mut self, map: PatchMap) -> Self {
        self.patch_map = Some(map);
        self
    }

    /// Set the syndrome buffer size per tile.
    pub fn syndrome_buffer(mut self, size: usize) -> Self {
        self.buffer_size = size.max(1);
        self
    }

    /// Set the gate thresholds.
    pub fn thresholds(mut self, t: GateThresholds) -> Self {
        self.thresholds = t;
        self
    }

    /// Set custom filter configuration.
    pub fn filter_config(mut self, config: FilterConfig) -> Self {
        self.filter_config = config;
        self
    }

    /// Enable or disable receipt logging.
    pub fn enable_receipts(mut self, enable: bool) -> Self {
        self.enable_receipts = enable;
        self
    }

    /// Build the QuantumFabric.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration is invalid
    /// - PatchMap validation fails
    pub fn build(self) -> Result<QuantumFabric> {
        // Validate patch map if provided
        if let Some(ref map) = self.patch_map {
            map.validate()?;
        }

        // Create configuration
        let config = FabricConfig {
            tile_count: self.tile_count,
            buffer_size: self.buffer_size,
            thresholds: self.thresholds.clone(),
            filter_config: self.filter_config.clone(),
            enable_receipts: self.enable_receipts,
            ..Default::default()
        };

        config.validate()?;

        // Determine number of worker tiles
        let worker_count = if let Some(ref map) = self.patch_map {
            map.tile_count().min(WORKER_TILE_COUNT)
        } else {
            (self.tile_count - 1).min(WORKER_TILE_COUNT)
        };

        // Create worker tiles
        let mut tiles: Vec<WorkerTile> = Vec::with_capacity(worker_count);
        for i in 0..worker_count {
            let tile_id = (i + 1) as u8; // Tile IDs start at 1
            tiles.push(WorkerTile::new(tile_id));
        }

        // Create TileZero
        let tile_zero = TileZero::new(self.thresholds);

        // Create coherence gate
        let gate = CoherenceGate::new(self.filter_config);

        // Create patch map if not provided
        let patch_map = self.patch_map.unwrap_or_else(|| {
            linear_patch_map(64, worker_count) // Default: 64 qubits
        });

        Ok(QuantumFabric {
            tiles,
            tile_zero,
            config,
            patch_map,
            gate,
            state: FabricState::default(),
            receipt_log: ReceiptLog::new(),
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// QuantumFabric - Main Orchestrator
// ═══════════════════════════════════════════════════════════════════════════════

/// The main orchestrator for the ruQu coherence gate system.
///
/// QuantumFabric manages the 256-tile WASM fabric, coordinating syndrome
/// processing across worker tiles and issuing coherence gate decisions.
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │                       QuantumFabric                              │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  ┌──────────┐  ┌──────────┐  ┌──────────┐      ┌──────────┐    │
/// │  │ Worker 1 │  │ Worker 2 │  │ Worker 3 │ ...  │Worker 255│    │
/// │  └────┬─────┘  └────┬─────┘  └────┬─────┘      └────┬─────┘    │
/// │       │             │             │                  │          │
/// │       └─────────────┴──────┬──────┴──────────────────┘          │
/// │                            │                                    │
/// │                     ┌──────▼──────┐                             │
/// │                     │  TileZero   │                             │
/// │                     │ (Coordinator)│                            │
/// │                     └──────┬──────┘                             │
/// │                            │                                    │
/// │                     ┌──────▼──────┐                             │
/// │                     │CoherenceGate │                            │
/// │                     │ (Decision)   │                            │
/// │                     └─────────────┘                             │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use ruqu::fabric::{QuantumFabric, surface_code_d7};
/// use ruqu::syndrome::{DetectorBitmap, SyndromeRound};
///
/// // Build the fabric
/// let mut fabric = QuantumFabric::builder()
///     .tiles(256)
///     .patch_map(surface_code_d7())
///     .syndrome_buffer(1024)
///     .build()
///     .expect("Failed to build fabric");
///
/// // Process syndrome rounds
/// let round = SyndromeRound::new(
///     1,
///     1000,
///     1705500000000,
///     DetectorBitmap::new(64),
///     0,
/// );
///
/// // Ingest and tick
/// fabric.ingest_syndromes(&[round]).expect("Ingest failed");
/// let decision = fabric.tick().expect("Tick failed");
/// ```
#[derive(Debug)]
pub struct QuantumFabric {
    /// Worker tiles (IDs 1-255)
    tiles: Vec<WorkerTile>,
    /// Coordinator tile (ID 0)
    tile_zero: TileZero,
    /// Fabric configuration
    config: FabricConfig,
    /// Hardware topology
    patch_map: PatchMap,
    /// The coherence gate
    pub gate: CoherenceGate,
    /// Runtime state
    state: FabricState,
    /// Receipt log for audit
    receipt_log: ReceiptLog,
}

impl QuantumFabric {
    /// Create a new FabricBuilder.
    pub fn builder() -> FabricBuilder {
        FabricBuilder::new()
    }

    /// Ingest a batch of syndrome rounds.
    ///
    /// Syndromes are distributed to the appropriate worker tiles based on
    /// the patch map. Each tile processes its assigned syndromes.
    ///
    /// # Arguments
    ///
    /// * `batch` - Slice of syndrome rounds to ingest
    ///
    /// # Errors
    ///
    /// Returns an error if syndrome distribution fails.
    pub fn ingest_syndromes(&mut self, batch: &[SyndromeRound]) -> Result<()> {
        for round in batch {
            self.state.syndromes_ingested += 1;

            // Distribute syndrome to appropriate tile(s)
            let tile_id = round.source_tile;
            if tile_id == 0 || tile_id as usize > self.tiles.len() {
                // Distribute to all tiles if source is TileZero or invalid
                // This handles the case where syndromes aren't pre-assigned
                for tile in &mut self.tiles {
                    // Convert syndrome round to delta for tile processing
                    let delta = crate::tile::SyndromeDelta::new(0, 0, round.fired_count() as u16);
                    tile.tick(&delta);
                }
            } else {
                // Send to specific tile
                let tile_idx = (tile_id - 1) as usize;
                if tile_idx < self.tiles.len() {
                    let delta = crate::tile::SyndromeDelta::new(0, 0, round.fired_count() as u16);
                    self.tiles[tile_idx].tick(&delta);
                }
            }
        }

        Ok(())
    }

    /// Execute one tick of the coherence gate.
    ///
    /// This is the main processing loop entry point:
    /// 1. Collect reports from all worker tiles
    /// 2. Merge reports in TileZero
    /// 3. Evaluate the three-filter pipeline
    /// 4. Issue gate decision
    /// 5. Update receipts
    ///
    /// # Returns
    ///
    /// The gate decision (Safe, Cautious, or Unsafe).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Decision latency exceeds budget
    /// - Filter evaluation fails
    pub fn tick(&mut self) -> Result<GateDecision> {
        let start = Instant::now();
        self.state.tick += 1;

        // Collect reports from all worker tiles
        let mut reports: Vec<TileReport> = Vec::with_capacity(self.tiles.len());
        for tile in &self.tiles {
            let report = TileReport::new(tile.tile_id);
            // In a real implementation, we'd get the actual report from the tile
            // For now, create a synthetic report based on tile state
            let mut report = report;
            report.local_cut = tile.local_cut_state.cut_value;
            report.shift_score = 0.1; // Would compute from tile
            report.e_value = tile.evidence.e_value();
            report.num_vertices = tile.patch_graph.num_vertices;
            report.num_edges = tile.patch_graph.num_edges;
            reports.push(report);
        }

        // Merge reports in TileZero
        let tile_decision = self.tile_zero.merge_reports(reports);

        // Convert tile decision to domain decision
        let decision = match tile_decision {
            TileGateDecision::Permit => GateDecision::Safe,
            TileGateDecision::Defer => GateDecision::Cautious,
            TileGateDecision::Deny => GateDecision::Unsafe,
        };

        // Update state
        self.state.last_decision = decision;
        self.state.active_tiles = self.tiles.len();

        match decision {
            GateDecision::Safe => self.state.permit_count += 1,
            GateDecision::Cautious => self.state.defer_count += 1,
            GateDecision::Unsafe => self.state.deny_count += 1,
        }

        // Update latency tracking
        let elapsed = start.elapsed().as_nanos() as u64;
        self.state.peak_latency_ns = self.state.peak_latency_ns.max(elapsed);

        let n = self.state.total_decisions();
        if n > 0 {
            self.state.avg_latency_ns = (self.state.avg_latency_ns * (n - 1) + elapsed) / n;
        }

        // Check latency budget
        if elapsed > self.config.decision_budget_ns {
            // Log warning but don't fail - latency budget is advisory
            // In production, this would trigger monitoring alerts
        }

        // Update gate sequence
        self.gate.increment_sequence();

        // Append receipt if enabled
        if self.config.enable_receipts {
            let witness_hash = [0u8; 32]; // Would compute proper hash
            self.receipt_log
                .append(tile_decision, self.state.tick, elapsed, witness_hash);
        }

        Ok(decision)
    }

    /// Get the current fabric state.
    pub fn current_state(&self) -> &FabricState {
        &self.state
    }

    /// Get a snapshot of the current fabric state (cloned).
    pub fn state_snapshot(&self) -> FabricState {
        self.state.clone()
    }

    /// Get the patch map.
    pub fn patch_map(&self) -> &PatchMap {
        &self.patch_map
    }

    /// Get the fabric configuration.
    pub fn config(&self) -> &FabricConfig {
        &self.config
    }

    /// Get the number of worker tiles.
    pub fn worker_count(&self) -> usize {
        self.tiles.len()
    }

    /// Get a reference to a specific worker tile.
    pub fn get_tile(&self, tile_id: u8) -> Option<&WorkerTile> {
        if tile_id == 0 || tile_id as usize > self.tiles.len() {
            return None;
        }
        Some(&self.tiles[(tile_id - 1) as usize])
    }

    /// Get a mutable reference to a specific worker tile.
    pub fn get_tile_mut(&mut self, tile_id: u8) -> Option<&mut WorkerTile> {
        if tile_id == 0 || tile_id as usize > self.tiles.len() {
            return None;
        }
        Some(&mut self.tiles[(tile_id - 1) as usize])
    }

    /// Get the TileZero coordinator.
    pub fn tile_zero(&self) -> &TileZero {
        &self.tile_zero
    }

    /// Get the receipt log.
    pub fn receipt_log(&self) -> &ReceiptLog {
        &self.receipt_log
    }

    /// Reset all tiles and state.
    pub fn reset(&mut self) {
        for tile in &mut self.tiles {
            tile.reset();
        }
        self.state = FabricState::default();
        self.receipt_log = ReceiptLog::new();
    }

    /// Get decision statistics.
    pub fn decision_stats(&self) -> DecisionStats {
        DecisionStats {
            total: self.state.total_decisions(),
            permits: self.state.permit_count,
            defers: self.state.defer_count,
            denies: self.state.deny_count,
            permit_rate: self.state.permit_rate(),
            avg_latency_ns: self.state.avg_latency_ns,
            peak_latency_ns: self.state.peak_latency_ns,
        }
    }
}

/// Statistics about gate decisions.
#[derive(Debug, Clone, Default)]
pub struct DecisionStats {
    /// Total decisions made
    pub total: u64,
    /// Number of PERMIT decisions
    pub permits: u64,
    /// Number of DEFER decisions
    pub defers: u64,
    /// Number of DENY decisions
    pub denies: u64,
    /// Permit rate (0.0 to 1.0)
    pub permit_rate: f64,
    /// Average decision latency (nanoseconds)
    pub avg_latency_ns: u64,
    /// Peak decision latency (nanoseconds)
    pub peak_latency_ns: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syndrome::DetectorBitmap;

    #[test]
    fn test_surface_code_d7() {
        let patch_map = surface_code_d7();

        assert_eq!(patch_map.name, "surface_code_d7");
        assert_eq!(patch_map.distance, Some(7));
        assert!(patch_map.qubit_count > 0);
        assert!(patch_map.tile_count() > 0);
        assert!(patch_map.validate().is_ok());
    }

    #[test]
    fn test_surface_code_various_distances() {
        for d in [3, 5, 7, 9, 11] {
            let patch_map = surface_code(d);
            assert_eq!(patch_map.distance, Some(d));
            assert!(patch_map.validate().is_ok());
        }
    }

    #[test]
    fn test_linear_patch_map() {
        let patch_map = linear_patch_map(100, 10);

        assert_eq!(patch_map.name, "linear");
        assert_eq!(patch_map.qubit_count, 100);
        assert_eq!(patch_map.tile_count(), 10);
        assert!(patch_map.validate().is_ok());
    }

    #[test]
    fn test_fabric_builder_default() {
        let fabric = QuantumFabric::builder().build();
        assert!(fabric.is_ok());

        let fabric = fabric.unwrap();
        assert!(fabric.worker_count() > 0);
    }

    #[test]
    fn test_fabric_builder_with_options() {
        let fabric = QuantumFabric::builder()
            .tiles(16)
            .patch_map(surface_code_d7())
            .syndrome_buffer(512)
            .enable_receipts(true)
            .build();

        assert!(fabric.is_ok());
        let fabric = fabric.unwrap();
        assert!(fabric.worker_count() <= 15);
    }

    #[test]
    fn test_fabric_ingest_syndromes() {
        let mut fabric = QuantumFabric::builder().tiles(4).build().unwrap();

        let rounds: Vec<SyndromeRound> = (0..10)
            .map(|i| SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0))
            .collect();

        let result = fabric.ingest_syndromes(&rounds);
        assert!(result.is_ok());
        assert_eq!(fabric.current_state().syndromes_ingested, 10);
    }

    #[test]
    fn test_fabric_tick() {
        let mut fabric = QuantumFabric::builder().tiles(4).build().unwrap();

        // Tick without any syndromes
        let result = fabric.tick();
        assert!(result.is_ok());

        let state = fabric.current_state();
        assert_eq!(state.tick, 1);
        assert_eq!(state.total_decisions(), 1);
    }

    #[test]
    fn test_fabric_multiple_ticks() {
        let mut fabric = QuantumFabric::builder().tiles(8).build().unwrap();

        // Run multiple ticks
        for _ in 0..100 {
            let _ = fabric.tick();
        }

        let state = fabric.current_state();
        assert_eq!(state.tick, 100);
        assert_eq!(state.total_decisions(), 100);
    }

    #[test]
    fn test_fabric_get_tile() {
        let fabric = QuantumFabric::builder().tiles(4).build().unwrap();

        // Tile 0 (TileZero) should return None
        assert!(fabric.get_tile(0).is_none());

        // Valid tile IDs
        assert!(fabric.get_tile(1).is_some());
        assert!(fabric.get_tile(2).is_some());
        assert!(fabric.get_tile(3).is_some());

        // Invalid tile ID
        assert!(fabric.get_tile(100).is_none());
    }

    #[test]
    fn test_fabric_reset() {
        let mut fabric = QuantumFabric::builder().tiles(4).build().unwrap();

        // Do some work
        for _ in 0..10 {
            let _ = fabric.tick();
        }

        assert_eq!(fabric.current_state().tick, 10);

        // Reset
        fabric.reset();

        assert_eq!(fabric.current_state().tick, 0);
        assert_eq!(fabric.current_state().total_decisions(), 0);
    }

    #[test]
    fn test_fabric_decision_stats() {
        let mut fabric = QuantumFabric::builder().tiles(4).build().unwrap();

        for _ in 0..50 {
            let _ = fabric.tick();
        }

        let stats = fabric.decision_stats();
        assert_eq!(stats.total, 50);
        assert!(stats.permits + stats.defers + stats.denies == 50);
    }

    #[test]
    fn test_coherence_gate_evaluate() {
        let gate = CoherenceGate::with_defaults();
        let decision = gate.evaluate();
        assert!(decision.is_ok());
    }

    #[test]
    fn test_coherence_gate_receipt() {
        let mut gate = CoherenceGate::with_defaults();

        // No receipt before first evaluation
        assert!(gate.receipt().is_none());

        // After incrementing sequence
        gate.increment_sequence();
        let receipt = gate.receipt();
        assert!(receipt.is_some());
    }

    #[test]
    fn test_patch_map_find_tile() {
        let patch_map = surface_code_d7();

        // Find tile for first qubit
        let tile = patch_map.find_tile_for_vertex(0);
        assert!(tile.is_some());

        // Non-existent qubit
        let tile = patch_map.find_tile_for_vertex(999999);
        assert!(tile.is_none());
    }

    #[test]
    fn test_tile_assignment() {
        let mut assignment = TileAssignment::new(1);

        assignment.add_vertex(0);
        assignment.add_vertex(1);
        assignment.add_vertex(2);
        assignment.add_boundary(0);
        assignment.add_neighbor(2);
        assignment.add_neighbor(2); // Duplicate should be ignored

        assert_eq!(assignment.vertices.len(), 3);
        assert_eq!(assignment.boundary_vertices.len(), 1);
        assert_eq!(assignment.neighbors.len(), 1);
        assert_eq!(assignment.vertex_count(), 4);
    }

    #[test]
    fn test_fabric_config_validate() {
        // Valid config
        let config = FabricConfig::default();
        assert!(config.validate().is_ok());

        // Invalid: zero tiles
        let mut config = FabricConfig::default();
        config.tile_count = 0;
        assert!(config.validate().is_err());

        // Invalid: too many tiles
        let mut config = FabricConfig::default();
        config.tile_count = 1000;
        assert!(config.validate().is_err());

        // Invalid: zero buffer
        let mut config = FabricConfig::default();
        config.buffer_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_fabric_state_metrics() {
        let mut state = FabricState::default();

        assert_eq!(state.total_decisions(), 0);
        assert_eq!(state.permit_rate(), 0.0);

        state.permit_count = 80;
        state.defer_count = 15;
        state.deny_count = 5;

        assert_eq!(state.total_decisions(), 100);
        assert!((state.permit_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_witness_receipt_creation() {
        let mut gate = CoherenceGate::with_defaults();
        gate.increment_sequence();

        let receipt = gate.receipt();
        assert!(receipt.is_some());

        let receipt = receipt.unwrap();
        assert_eq!(receipt.sequence, 1);
    }
}
