//! Progressive layer model (Layer A / B / C) for RVF indexing.
//!
//! Each layer is independently useful and stores a different granularity
//! of the HNSW graph, enabling progressive availability.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::hnsw::HnswLayer;

/// Which index layer a piece of data belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndexLayer {
    /// Entry points + coarse routing. Always present, loaded first (< 5ms).
    A = 0,
    /// Partial adjacency for the hot region. Loaded second (100ms-1s).
    B = 1,
    /// Full adjacency for every node. Loaded last (seconds to minutes).
    C = 2,
}

impl TryFrom<u8> for IndexLayer {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::A),
            1 => Ok(Self::B),
            2 => Ok(Self::C),
            other => Err(other),
        }
    }
}

/// Entry in the centroid-to-partition map.
#[derive(Clone, Debug)]
pub struct PartitionEntry {
    /// Which centroid owns this partition.
    pub centroid_id: u32,
    /// First vector ID in this partition.
    pub vector_id_start: u64,
    /// Last vector ID in this partition (exclusive).
    pub vector_id_end: u64,
    /// Segment ID containing the vector data.
    pub segment_ref: u64,
    /// Block offset within the segment.
    pub block_ref: u32,
}

/// Layer A: Entry Points + Coarse Routing.
///
/// Contains:
/// - HNSW entry points (node ID + layer)
/// - Top-layer adjacency lists (layers >= threshold)
/// - Cluster centroids for IVF-style partition routing
/// - Centroid-to-partition map
#[derive(Clone, Debug)]
pub struct LayerA {
    /// Entry points: `(node_id, max_layer)`.
    pub entry_points: Vec<(u64, u32)>,
    /// Top-layer adjacency: HNSW layers at the highest levels.
    /// Index 0 = the highest layer, etc.
    pub top_layers: Vec<HnswLayer>,
    /// The HNSW layer index where top_layers[0] starts.
    pub top_layer_start: usize,
    /// Cluster centroids for partition routing.
    pub centroids: Vec<Vec<f32>>,
    /// Map from centroid to vector ID ranges.
    pub partition_map: Vec<PartitionEntry>,
}

/// Layer B: Partial Adjacency for the hot working set.
///
/// Contains neighbor lists for the most-accessed nodes (determined by
/// temperature sketch). Typically covers 10-20% of total nodes.
#[derive(Clone, Debug)]
pub struct LayerB {
    /// Partial adjacency: node_id -> neighbor list.
    /// Only nodes in the hot region are present.
    pub partial_adjacency: BTreeMap<u64, Vec<u64>>,
    /// Ranges of node IDs covered by this layer.
    pub covered_ranges: Vec<(u64, u64)>,
}

impl LayerB {
    /// Returns true if the given node has adjacency data in this layer.
    #[inline]
    pub fn has_node(&self, id: u64) -> bool {
        self.partial_adjacency.contains_key(&id)
    }

    /// Returns neighbors for a node, or `None` if not in the hot region.
    #[inline]
    pub fn neighbors(&self, id: u64) -> Option<&[u64]> {
        self.partial_adjacency.get(&id).map(|v| v.as_slice())
    }
}

/// Layer C: Full Adjacency.
///
/// Complete neighbor lists for every node at every HNSW level.
/// This is the traditional full HNSW graph.
#[derive(Clone, Debug)]
pub struct LayerC {
    /// Full adjacency at every HNSW layer. Index 0 = layer 0 (bottom).
    pub full_adjacency: Vec<HnswLayer>,
}

/// Aggregated state of all loaded index layers.
#[derive(Clone, Debug)]
pub struct IndexState {
    pub layer_a: Option<LayerA>,
    pub layer_b: Option<LayerB>,
    pub layer_c: Option<LayerC>,
    /// Total number of nodes in the full graph (known from metadata).
    pub total_nodes: u64,
}

/// Estimate recall@10 based on which layers are currently loaded.
///
/// These are approximate lower-bound estimates based on the spec:
/// - A only: 0.65-0.75
/// - A + B: 0.85-0.92
/// - A + B + C: 0.95-0.99
pub fn available_recall(state: &IndexState) -> f32 {
    match (&state.layer_a, &state.layer_b, &state.layer_c) {
        (None, _, _) => 0.0,
        (Some(_), None, None) => 0.70,
        (Some(_), Some(b), None) => {
            // Recall scales with coverage of partial adjacency.
            let covered_nodes: u64 = b
                .covered_ranges
                .iter()
                .map(|(start, end)| end.saturating_sub(*start))
                .sum();
            let coverage = if state.total_nodes > 0 {
                covered_nodes as f32 / state.total_nodes as f32
            } else {
                0.0
            };
            // Scale between 0.70 (no B coverage) and 0.92 (full B coverage).
            0.70 + coverage * 0.22
        }
        (Some(_), _, Some(_)) => 0.97,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_layer_round_trip() {
        assert_eq!(IndexLayer::try_from(0), Ok(IndexLayer::A));
        assert_eq!(IndexLayer::try_from(1), Ok(IndexLayer::B));
        assert_eq!(IndexLayer::try_from(2), Ok(IndexLayer::C));
        assert_eq!(IndexLayer::try_from(3), Err(3));
    }

    #[test]
    fn recall_no_layers() {
        let state = IndexState {
            layer_a: None,
            layer_b: None,
            layer_c: None,
            total_nodes: 1000,
        };
        assert!((available_recall(&state) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn recall_a_only() {
        let state = IndexState {
            layer_a: Some(LayerA {
                entry_points: vec![(0, 5)],
                top_layers: vec![],
                top_layer_start: 5,
                centroids: vec![],
                partition_map: vec![],
            }),
            layer_b: None,
            layer_c: None,
            total_nodes: 1000,
        };
        assert!((available_recall(&state) - 0.70).abs() < 0.01);
    }

    #[test]
    fn recall_a_plus_b() {
        let state = IndexState {
            layer_a: Some(LayerA {
                entry_points: vec![(0, 5)],
                top_layers: vec![],
                top_layer_start: 5,
                centroids: vec![],
                partition_map: vec![],
            }),
            layer_b: Some(LayerB {
                partial_adjacency: BTreeMap::new(),
                covered_ranges: vec![(0, 500)],
            }),
            layer_c: None,
            total_nodes: 1000,
        };
        let recall = available_recall(&state);
        assert!(recall > 0.70);
        assert!(recall < 0.93);
    }

    #[test]
    fn recall_full() {
        let state = IndexState {
            layer_a: Some(LayerA {
                entry_points: vec![(0, 5)],
                top_layers: vec![],
                top_layer_start: 5,
                centroids: vec![],
                partition_map: vec![],
            }),
            layer_b: Some(LayerB {
                partial_adjacency: BTreeMap::new(),
                covered_ranges: vec![(0, 1000)],
            }),
            layer_c: Some(LayerC {
                full_adjacency: vec![],
            }),
            total_nodes: 1000,
        };
        assert!(available_recall(&state) >= 0.95);
    }

    #[test]
    fn layer_b_has_node() {
        let mut adj = BTreeMap::new();
        adj.insert(42, vec![1, 2, 3]);
        let b = LayerB {
            partial_adjacency: adj,
            covered_ranges: vec![(0, 100)],
        };
        assert!(b.has_node(42));
        assert!(!b.has_node(99));
        assert_eq!(b.neighbors(42), Some([1u64, 2, 3].as_slice()));
        assert_eq!(b.neighbors(99), None);
    }
}
