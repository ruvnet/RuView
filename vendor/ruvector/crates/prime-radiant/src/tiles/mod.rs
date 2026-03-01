//! Tiles Integration - Adapter for cognitum-gate-kernel (256-tile WASM fabric)
//!
//! This module provides the coherence fabric adapter that wraps the `cognitum-gate-kernel`
//! crate, enabling distributed coherence computation across 256 WASM tiles.
//!
//! # Architecture
//!
//! The coherence fabric consists of 256 worker tiles, each running a lightweight kernel.
//! Tiles receive delta updates and observations, process them through a deterministic tick
//! loop, and produce witness fragments for global aggregation.
//!
//! # Key Types
//!
//! - [`CoherenceFabric`]: Main coordinator for all 256 tiles
//! - [`TileAdapter`]: Adapter wrapping a single `cognitum_gate_kernel::TileState`
//! - [`TileCoordinator`]: Coordinates tile communication and aggregation
//! - [`FabricReport`]: Aggregated report from all tiles after a tick
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::tiles::{CoherenceFabric, FabricConfig};
//!
//! // Create fabric with default configuration
//! let mut fabric = CoherenceFabric::new(FabricConfig::default());
//!
//! // Distribute a node update
//! fabric.distribute_update(node_id, &new_state);
//!
//! // Execute one tick across all tiles
//! let report = fabric.tick(1);
//!
//! // Check global coherence
//! println!("Global energy: {}", report.global_energy);
//! ```

mod adapter;
mod coordinator;
mod error;
mod fabric;

pub use adapter::{TileAdapter, TileAdapterConfig};
pub use coordinator::{AggregatedWitness, CoordinatorConfig, ShardMap, TileCoordinator};
pub use error::{TilesError, TilesResult};
pub use fabric::{CoherenceFabric, FabricConfig, FabricReport, FabricState};
