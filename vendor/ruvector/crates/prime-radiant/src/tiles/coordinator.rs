//! Tile coordinator for managing communication and aggregation across tiles.

use super::adapter::TileAdapter;
use super::error::TilesResult;
use cognitum_gate_kernel::report::WitnessFragment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the tile coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// Hash function for shard mapping.
    pub hash_seed: u64,
    /// Number of shards (typically 256 for tile count).
    pub num_shards: u16,
    /// Enable parallel aggregation.
    pub parallel_aggregation: bool,
    /// Witness hash algorithm (blake3).
    pub witness_hash_algo: String,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            hash_seed: 0x5851F42D4C957F2D, // FNV offset basis
            num_shards: 256,
            parallel_aggregation: true,
            witness_hash_algo: "blake3".to_string(),
        }
    }
}

/// Maps node IDs to tile shards.
#[derive(Debug, Clone)]
pub struct ShardMap {
    /// Hash seed for consistent hashing.
    hash_seed: u64,
    /// Number of shards.
    num_shards: u16,
}

impl ShardMap {
    /// Create a new shard map.
    pub fn new(hash_seed: u64, num_shards: u16) -> Self {
        Self {
            hash_seed,
            num_shards,
        }
    }

    /// Create with default configuration.
    pub fn default_256() -> Self {
        Self::new(0x5851F42D4C957F2D, 256)
    }

    /// Get the tile ID for a given node ID.
    ///
    /// Uses FNV-1a hash for consistent distribution.
    #[inline]
    pub fn tile_for_node(&self, node_id: u64) -> u8 {
        let hash = self.fnv1a_hash(node_id);
        (hash % self.num_shards as u64) as u8
    }

    /// FNV-1a hash function.
    fn fnv1a_hash(&self, data: u64) -> u64 {
        const FNV_PRIME: u64 = 0x00000100000001B3;
        let mut hash = self.hash_seed;
        let bytes = data.to_le_bytes();
        for byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    /// Get all node IDs that map to a specific tile.
    /// Note: This is expensive and should only be used for debugging.
    pub fn nodes_for_tile(&self, tile_id: u8, node_ids: &[u64]) -> Vec<u64> {
        node_ids
            .iter()
            .filter(|&&id| self.tile_for_node(id) == tile_id)
            .copied()
            .collect()
    }
}

impl Default for ShardMap {
    fn default() -> Self {
        Self::default_256()
    }
}

/// Aggregated witness from all tiles.
#[derive(Debug, Clone)]
pub struct AggregatedWitness {
    /// Combined hash of all witness fragments.
    pub combined_hash: [u8; 32],
    /// Total cardinality across all tiles.
    pub total_cardinality: u32,
    /// Total boundary vertices.
    pub total_boundary: u32,
    /// Estimated global min-cut value.
    pub estimated_min_cut: f64,
    /// Number of tiles contributing.
    pub contributing_tiles: u16,
    /// Per-tile fragments (for debugging).
    pub fragments: Vec<(u8, WitnessFragment)>,
}

impl AggregatedWitness {
    /// Create an empty aggregated witness.
    pub fn empty() -> Self {
        Self {
            combined_hash: [0u8; 32],
            total_cardinality: 0,
            total_boundary: 0,
            estimated_min_cut: 0.0,
            contributing_tiles: 0,
            fragments: Vec::new(),
        }
    }

    /// Check if the witness is empty.
    pub fn is_empty(&self) -> bool {
        self.contributing_tiles == 0
    }
}

/// Coordinator for tile communication and aggregation.
pub struct TileCoordinator {
    /// Configuration.
    config: CoordinatorConfig,
    /// Shard mapping.
    shard_map: ShardMap,
    /// Cached fragment hashes for change detection.
    cached_hashes: HashMap<u8, u16>,
    /// Last aggregated witness.
    last_witness: Option<AggregatedWitness>,
}

impl TileCoordinator {
    /// Create a new tile coordinator.
    pub fn new(config: CoordinatorConfig) -> Self {
        let shard_map = ShardMap::new(config.hash_seed, config.num_shards);
        Self {
            config,
            shard_map,
            cached_hashes: HashMap::with_capacity(256),
            last_witness: None,
        }
    }

    /// Create with default configuration.
    pub fn default_coordinator() -> Self {
        Self::new(CoordinatorConfig::default())
    }

    /// Get the shard map.
    pub fn shard_map(&self) -> &ShardMap {
        &self.shard_map
    }

    /// Get the tile ID for a node.
    #[inline]
    pub fn tile_for_node(&self, node_id: u64) -> u8 {
        self.shard_map.tile_for_node(node_id)
    }

    /// Aggregate witness fragments from multiple tiles.
    ///
    /// This combines the witness fragments into a global witness that represents
    /// the coherence state across all tiles.
    pub fn aggregate_witnesses(&mut self, tiles: &[TileAdapter]) -> TilesResult<AggregatedWitness> {
        if tiles.is_empty() {
            return Ok(AggregatedWitness::empty());
        }

        let mut hasher = blake3::Hasher::new();
        let mut total_cardinality: u32 = 0;
        let mut total_boundary: u32 = 0;
        let mut min_cut_sum: f64 = 0.0;
        let mut contributing_tiles: u16 = 0;
        let mut fragments = Vec::with_capacity(tiles.len());

        for tile in tiles {
            let fragment = tile.witness_fragment();

            // Skip empty fragments
            if fragment.is_empty() {
                continue;
            }

            // Update hash
            hasher.update(&fragment.hash.to_le_bytes());

            // Aggregate metrics
            total_cardinality += fragment.cardinality as u32;
            total_boundary += fragment.boundary_size as u32;
            min_cut_sum += fragment.local_min_cut as f64;
            contributing_tiles += 1;

            // Cache for change detection
            self.cached_hashes.insert(tile.tile_id(), fragment.hash);

            fragments.push((tile.tile_id(), fragment));
        }

        let combined_hash = *hasher.finalize().as_bytes();

        let witness = AggregatedWitness {
            combined_hash,
            total_cardinality,
            total_boundary,
            estimated_min_cut: min_cut_sum,
            contributing_tiles,
            fragments,
        };

        self.last_witness = Some(witness.clone());
        Ok(witness)
    }

    /// Check if any tile's witness has changed since last aggregation.
    pub fn has_witness_changed(&self, tiles: &[TileAdapter]) -> bool {
        for tile in tiles {
            let fragment = tile.witness_fragment();
            if let Some(&cached) = self.cached_hashes.get(&tile.tile_id()) {
                if cached != fragment.hash {
                    return true;
                }
            } else if !fragment.is_empty() {
                return true;
            }
        }
        false
    }

    /// Get the last aggregated witness, if any.
    pub fn last_witness(&self) -> Option<&AggregatedWitness> {
        self.last_witness.as_ref()
    }

    /// Compute global energy from all tiles.
    ///
    /// This sums the log e-values from all tiles to get a global coherence measure.
    pub fn compute_global_energy(&self, tiles: &[TileAdapter]) -> f64 {
        tiles.iter().map(|t| t.log_e_value() as f64).sum()
    }

    /// Get coherence summary across all tiles.
    pub fn coherence_summary(&self, tiles: &[TileAdapter]) -> CoherenceSummary {
        let mut total_vertices = 0u32;
        let mut total_edges = 0u32;
        let mut total_components = 0u32;
        let mut total_energy = 0.0f64;
        let mut active_tiles = 0u16;

        for tile in tiles {
            let stats = tile.graph_stats();
            if stats.num_vertices > 0 {
                total_vertices += stats.num_vertices as u32;
                total_edges += stats.num_edges as u32;
                total_components += stats.num_components as u32;
                total_energy += tile.log_e_value() as f64;
                active_tiles += 1;
            }
        }

        CoherenceSummary {
            total_vertices,
            total_edges,
            total_components,
            total_energy,
            active_tiles,
            average_energy: if active_tiles > 0 {
                total_energy / active_tiles as f64
            } else {
                0.0
            },
        }
    }

    /// Clear cached state.
    pub fn clear_cache(&mut self) {
        self.cached_hashes.clear();
        self.last_witness = None;
    }
}

impl Default for TileCoordinator {
    fn default() -> Self {
        Self::default_coordinator()
    }
}

impl std::fmt::Debug for TileCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TileCoordinator")
            .field("num_shards", &self.config.num_shards)
            .field("cached_tiles", &self.cached_hashes.len())
            .field("has_witness", &self.last_witness.is_some())
            .finish()
    }
}

/// Summary of coherence state across all tiles.
#[derive(Debug, Clone, Copy)]
pub struct CoherenceSummary {
    /// Total vertices across all tiles.
    pub total_vertices: u32,
    /// Total edges across all tiles.
    pub total_edges: u32,
    /// Total connected components.
    pub total_components: u32,
    /// Total energy (sum of log e-values).
    pub total_energy: f64,
    /// Number of active tiles.
    pub active_tiles: u16,
    /// Average energy per active tile.
    pub average_energy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_map_distribution() {
        let map = ShardMap::default_256();

        // Test that different node IDs get distributed
        let mut tile_counts = [0u32; 256];
        for i in 0..10000u64 {
            let tile = map.tile_for_node(i);
            tile_counts[tile as usize] += 1;
        }

        // Check reasonable distribution (each tile should have some nodes)
        let non_empty = tile_counts.iter().filter(|&&c| c > 0).count();
        assert!(
            non_empty > 200,
            "Distribution too sparse: {non_empty} tiles used"
        );
    }

    #[test]
    fn test_shard_map_consistency() {
        let map = ShardMap::default_256();

        // Same node ID should always map to same tile
        let node = 12345u64;
        let tile1 = map.tile_for_node(node);
        let tile2 = map.tile_for_node(node);
        assert_eq!(tile1, tile2);
    }

    #[test]
    fn test_coordinator_aggregate_empty() {
        let mut coordinator = TileCoordinator::default();
        let witness = coordinator.aggregate_witnesses(&[]).unwrap();
        assert!(witness.is_empty());
    }

    #[test]
    fn test_coordinator_coherence_summary() {
        let coordinator = TileCoordinator::default();
        let tiles: Vec<TileAdapter> = vec![];
        let summary = coordinator.coherence_summary(&tiles);
        assert_eq!(summary.active_tiles, 0);
        assert_eq!(summary.total_vertices, 0);
    }
}
