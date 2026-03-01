//! Delta types for incremental graph updates
//!
//! Defines the message types that tiles receive from the coordinator.
//! All types are `#[repr(C)]` for FFI compatibility and fixed-size
//! for deterministic memory allocation.

#![allow(missing_docs)]

use core::mem::size_of;

/// Compact vertex identifier (16-bit for tile-local addressing)
pub type TileVertexId = u16;

/// Compact edge identifier (16-bit for tile-local addressing)
pub type TileEdgeId = u16;

/// Fixed-point weight (16-bit, 0.01 precision)
/// Actual weight = raw_weight / 100.0
pub type FixedWeight = u16;

/// Convert fixed-point weight to f32
#[inline(always)]
pub const fn weight_to_f32(w: FixedWeight) -> f32 {
    (w as f32) / 100.0
}

/// Convert f32 weight to fixed-point (saturating)
#[inline(always)]
pub const fn f32_to_weight(w: f32) -> FixedWeight {
    let scaled = (w * 100.0) as i32;
    if scaled < 0 {
        0
    } else if scaled > 65535 {
        65535
    } else {
        scaled as u16
    }
}

/// Delta operation tag
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeltaTag {
    /// No operation (padding/sentinel)
    Nop = 0,
    /// Add an edge to the graph
    EdgeAdd = 1,
    /// Remove an edge from the graph
    EdgeRemove = 2,
    /// Update the weight of an existing edge
    WeightUpdate = 3,
    /// Observation for evidence accumulation
    Observation = 4,
    /// Batch boundary marker
    BatchEnd = 5,
    /// Checkpoint request
    Checkpoint = 6,
    /// Reset tile state
    Reset = 7,
}

impl From<u8> for DeltaTag {
    fn from(v: u8) -> Self {
        match v {
            1 => DeltaTag::EdgeAdd,
            2 => DeltaTag::EdgeRemove,
            3 => DeltaTag::WeightUpdate,
            4 => DeltaTag::Observation,
            5 => DeltaTag::BatchEnd,
            6 => DeltaTag::Checkpoint,
            7 => DeltaTag::Reset,
            _ => DeltaTag::Nop,
        }
    }
}

/// Edge addition delta
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct EdgeAdd {
    /// Source vertex (tile-local ID)
    pub source: TileVertexId,
    /// Target vertex (tile-local ID)
    pub target: TileVertexId,
    /// Edge weight (fixed-point)
    pub weight: FixedWeight,
    /// Edge flags (reserved for future use)
    pub flags: u16,
}

impl EdgeAdd {
    /// Create a new edge addition
    #[inline]
    pub const fn new(source: TileVertexId, target: TileVertexId, weight: FixedWeight) -> Self {
        Self {
            source,
            target,
            weight,
            flags: 0,
        }
    }

    /// Create from f32 weight
    #[inline]
    pub const fn with_f32_weight(source: TileVertexId, target: TileVertexId, weight: f32) -> Self {
        Self::new(source, target, f32_to_weight(weight))
    }
}

/// Edge removal delta
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct EdgeRemove {
    /// Source vertex (tile-local ID)
    pub source: TileVertexId,
    /// Target vertex (tile-local ID)
    pub target: TileVertexId,
    /// Reserved padding for alignment
    pub _reserved: u32,
}

impl EdgeRemove {
    /// Create a new edge removal
    #[inline]
    pub const fn new(source: TileVertexId, target: TileVertexId) -> Self {
        Self {
            source,
            target,
            _reserved: 0,
        }
    }
}

/// Weight update delta
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct WeightUpdate {
    /// Source vertex (tile-local ID)
    pub source: TileVertexId,
    /// Target vertex (tile-local ID)
    pub target: TileVertexId,
    /// New weight (fixed-point)
    pub new_weight: FixedWeight,
    /// Delta mode: 0 = absolute, 1 = relative add, 2 = relative multiply
    pub mode: u8,
    /// Reserved padding
    pub _reserved: u8,
}

impl WeightUpdate {
    /// Absolute weight update mode
    pub const MODE_ABSOLUTE: u8 = 0;
    /// Relative addition mode
    pub const MODE_ADD: u8 = 1;
    /// Relative multiply mode (fixed-point: value/100)
    pub const MODE_MULTIPLY: u8 = 2;

    /// Create an absolute weight update
    #[inline]
    pub const fn absolute(source: TileVertexId, target: TileVertexId, weight: FixedWeight) -> Self {
        Self {
            source,
            target,
            new_weight: weight,
            mode: Self::MODE_ABSOLUTE,
            _reserved: 0,
        }
    }

    /// Create a relative weight addition
    #[inline]
    pub const fn add(source: TileVertexId, target: TileVertexId, delta: FixedWeight) -> Self {
        Self {
            source,
            target,
            new_weight: delta,
            mode: Self::MODE_ADD,
            _reserved: 0,
        }
    }
}

/// Observation for evidence accumulation
///
/// Represents a measurement or event that affects the e-value calculation.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Observation {
    /// Vertex or region this observation applies to
    pub vertex: TileVertexId,
    /// Observation type/category
    pub obs_type: u8,
    /// Observation flags
    pub flags: u8,
    /// Observation value (interpretation depends on obs_type)
    pub value: u32,
}

impl Observation {
    /// Observation type: connectivity evidence
    pub const TYPE_CONNECTIVITY: u8 = 0;
    /// Observation type: cut membership evidence
    pub const TYPE_CUT_MEMBERSHIP: u8 = 1;
    /// Observation type: flow evidence
    pub const TYPE_FLOW: u8 = 2;
    /// Observation type: witness evidence
    pub const TYPE_WITNESS: u8 = 3;

    /// Create a connectivity observation
    #[inline]
    pub const fn connectivity(vertex: TileVertexId, connected: bool) -> Self {
        Self {
            vertex,
            obs_type: Self::TYPE_CONNECTIVITY,
            flags: if connected { 1 } else { 0 },
            value: 0,
        }
    }

    /// Create a cut membership observation
    #[inline]
    pub const fn cut_membership(vertex: TileVertexId, side: u8, confidence: u16) -> Self {
        Self {
            vertex,
            obs_type: Self::TYPE_CUT_MEMBERSHIP,
            flags: side,
            value: confidence as u32,
        }
    }
}

/// Unified delta message (8 bytes, cache-aligned for batching)
///
/// Tagged union for all delta types. The layout is optimized for
/// WASM memory access patterns.
#[derive(Clone, Copy)]
#[repr(C)]
pub union DeltaPayload {
    /// Edge addition payload
    pub edge_add: EdgeAdd,
    /// Edge removal payload
    pub edge_remove: EdgeRemove,
    /// Weight update payload
    pub weight_update: WeightUpdate,
    /// Observation payload
    pub observation: Observation,
    /// Raw bytes for custom payloads
    pub raw: [u8; 8],
}

impl Default for DeltaPayload {
    fn default() -> Self {
        Self { raw: [0u8; 8] }
    }
}

/// Complete delta message with tag
#[derive(Clone, Copy)]
#[repr(C, align(16))]
pub struct Delta {
    /// Delta operation tag
    pub tag: DeltaTag,
    /// Sequence number for ordering
    pub sequence: u8,
    /// Source tile ID (for cross-tile deltas)
    pub source_tile: u8,
    /// Reserved for future use
    pub _reserved: u8,
    /// Timestamp (lower 32 bits of tick counter)
    pub timestamp: u32,
    /// Delta payload
    pub payload: DeltaPayload,
}

impl Default for Delta {
    fn default() -> Self {
        Self {
            tag: DeltaTag::Nop,
            sequence: 0,
            source_tile: 0,
            _reserved: 0,
            timestamp: 0,
            payload: DeltaPayload::default(),
        }
    }
}

impl Delta {
    /// Create a NOP delta
    #[inline]
    pub const fn nop() -> Self {
        Self {
            tag: DeltaTag::Nop,
            sequence: 0,
            source_tile: 0,
            _reserved: 0,
            timestamp: 0,
            payload: DeltaPayload { raw: [0u8; 8] },
        }
    }

    /// Create an edge add delta
    #[inline]
    pub fn edge_add(source: TileVertexId, target: TileVertexId, weight: FixedWeight) -> Self {
        Self {
            tag: DeltaTag::EdgeAdd,
            sequence: 0,
            source_tile: 0,
            _reserved: 0,
            timestamp: 0,
            payload: DeltaPayload {
                edge_add: EdgeAdd::new(source, target, weight),
            },
        }
    }

    /// Create an edge remove delta
    #[inline]
    pub fn edge_remove(source: TileVertexId, target: TileVertexId) -> Self {
        Self {
            tag: DeltaTag::EdgeRemove,
            sequence: 0,
            source_tile: 0,
            _reserved: 0,
            timestamp: 0,
            payload: DeltaPayload {
                edge_remove: EdgeRemove::new(source, target),
            },
        }
    }

    /// Create a weight update delta
    #[inline]
    pub fn weight_update(source: TileVertexId, target: TileVertexId, weight: FixedWeight) -> Self {
        Self {
            tag: DeltaTag::WeightUpdate,
            sequence: 0,
            source_tile: 0,
            _reserved: 0,
            timestamp: 0,
            payload: DeltaPayload {
                weight_update: WeightUpdate::absolute(source, target, weight),
            },
        }
    }

    /// Create an observation delta
    #[inline]
    pub fn observation(obs: Observation) -> Self {
        Self {
            tag: DeltaTag::Observation,
            sequence: 0,
            source_tile: 0,
            _reserved: 0,
            timestamp: 0,
            payload: DeltaPayload { observation: obs },
        }
    }

    /// Create a batch end marker
    #[inline]
    pub const fn batch_end() -> Self {
        Self {
            tag: DeltaTag::BatchEnd,
            sequence: 0,
            source_tile: 0,
            _reserved: 0,
            timestamp: 0,
            payload: DeltaPayload { raw: [0u8; 8] },
        }
    }

    /// Check if this is a NOP
    #[inline]
    pub const fn is_nop(&self) -> bool {
        matches!(self.tag, DeltaTag::Nop)
    }

    /// Get the edge add payload (unsafe: caller must verify tag)
    #[inline]
    pub unsafe fn get_edge_add(&self) -> &EdgeAdd {
        unsafe { &self.payload.edge_add }
    }

    /// Get the edge remove payload (unsafe: caller must verify tag)
    #[inline]
    pub unsafe fn get_edge_remove(&self) -> &EdgeRemove {
        unsafe { &self.payload.edge_remove }
    }

    /// Get the weight update payload (unsafe: caller must verify tag)
    #[inline]
    pub unsafe fn get_weight_update(&self) -> &WeightUpdate {
        unsafe { &self.payload.weight_update }
    }

    /// Get the observation payload (unsafe: caller must verify tag)
    #[inline]
    pub unsafe fn get_observation(&self) -> &Observation {
        unsafe { &self.payload.observation }
    }
}

// Compile-time size assertions
const _: () = assert!(size_of::<EdgeAdd>() == 8, "EdgeAdd must be 8 bytes");
const _: () = assert!(size_of::<EdgeRemove>() == 8, "EdgeRemove must be 8 bytes");
const _: () = assert!(
    size_of::<WeightUpdate>() == 8,
    "WeightUpdate must be 8 bytes"
);
const _: () = assert!(size_of::<Observation>() == 8, "Observation must be 8 bytes");
const _: () = assert!(size_of::<Delta>() == 16, "Delta must be 16 bytes");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_conversion() {
        assert_eq!(weight_to_f32(100), 1.0);
        assert_eq!(weight_to_f32(50), 0.5);
        assert_eq!(weight_to_f32(0), 0.0);

        assert_eq!(f32_to_weight(1.0), 100);
        assert_eq!(f32_to_weight(0.5), 50);
        assert_eq!(f32_to_weight(0.0), 0);
    }

    #[test]
    fn test_delta_tag_roundtrip() {
        for i in 0..=7 {
            let tag = DeltaTag::from(i);
            assert_eq!(tag as u8, i);
        }
    }

    #[test]
    fn test_edge_add_creation() {
        let ea = EdgeAdd::new(1, 2, 150);
        assert_eq!(ea.source, 1);
        assert_eq!(ea.target, 2);
        assert_eq!(ea.weight, 150);
    }

    #[test]
    fn test_delta_edge_add() {
        let delta = Delta::edge_add(5, 10, 200);
        assert_eq!(delta.tag, DeltaTag::EdgeAdd);
        unsafe {
            let ea = delta.get_edge_add();
            assert_eq!(ea.source, 5);
            assert_eq!(ea.target, 10);
            assert_eq!(ea.weight, 200);
        }
    }

    #[test]
    fn test_observation_creation() {
        let obs = Observation::connectivity(42, true);
        assert_eq!(obs.vertex, 42);
        assert_eq!(obs.obs_type, Observation::TYPE_CONNECTIVITY);
        assert_eq!(obs.flags, 1);
    }
}
