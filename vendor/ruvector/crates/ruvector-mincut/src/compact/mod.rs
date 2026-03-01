//! Compact data structures for 8KB WASM cores
//!
//! Optimized for agentic chip with 256 cores × 8KB each.
//! Total budget: ~8KB per core for complete min-cut state.

// Internal optimization module - docs on public API in lib.rs
#![allow(missing_docs)]
#![cfg_attr(not(test), no_std)]
extern crate alloc;

use core::mem::size_of;

/// Maximum vertices per core (fits in 8KB budget)
pub const MAX_VERTICES_PER_CORE: usize = 256;

/// Maximum edges per core (reduced to fit in 8KB total)
/// Budget breakdown:
/// - Adjacency offsets: 257 * 2 = 514 bytes
/// - Adjacency neighbors: 384 * 2 * 4 = 3072 bytes
/// - Edges: 384 * 8 = 3072 bytes
/// - Other fields: ~50 bytes
/// Total: ~6.7KB (fits comfortably in 8KB)
pub const MAX_EDGES_PER_CORE: usize = 384;

/// Compact vertex ID (u16 saves 6 bytes vs u64)
pub type CompactVertexId = u16;

/// Compact edge ID (u16)
pub type CompactEdgeId = u16;

/// Bit-packed membership set (256 vertices = 32 bytes)
/// Much smaller than RoaringBitmap for small vertex counts
#[derive(Clone, Copy, Default)]
#[repr(C, align(32))]
pub struct BitSet256 {
    /// Raw bits storage - public for SIMD access
    pub bits: [u64; 4], // 256 bits = 32 bytes
}

impl BitSet256 {
    pub const fn new() -> Self {
        Self { bits: [0; 4] }
    }

    #[inline(always)]
    pub fn insert(&mut self, v: CompactVertexId) {
        let idx = (v / 64) as usize;
        let bit = v % 64;
        if idx < 4 {
            self.bits[idx] |= 1u64 << bit;
        }
    }

    #[inline(always)]
    pub fn contains(&self, v: CompactVertexId) -> bool {
        let idx = (v / 64) as usize;
        let bit = v % 64;
        idx < 4 && (self.bits[idx] & (1u64 << bit)) != 0
    }

    #[inline(always)]
    pub fn remove(&mut self, v: CompactVertexId) {
        let idx = (v / 64) as usize;
        let bit = v % 64;
        if idx < 4 {
            self.bits[idx] &= !(1u64 << bit);
        }
    }

    #[inline(always)]
    pub fn count(&self) -> u32 {
        self.bits.iter().map(|b| b.count_ones()).sum()
    }

    #[inline(always)]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            bits: [
                self.bits[0] | other.bits[0],
                self.bits[1] | other.bits[1],
                self.bits[2] | other.bits[2],
                self.bits[3] | other.bits[3],
            ],
        }
    }

    #[inline(always)]
    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            bits: [
                self.bits[0] & other.bits[0],
                self.bits[1] & other.bits[1],
                self.bits[2] & other.bits[2],
                self.bits[3] & other.bits[3],
            ],
        }
    }

    #[inline(always)]
    pub fn xor(&self, other: &Self) -> Self {
        Self {
            bits: [
                self.bits[0] ^ other.bits[0],
                self.bits[1] ^ other.bits[1],
                self.bits[2] ^ other.bits[2],
                self.bits[3] ^ other.bits[3],
            ],
        }
    }

    pub fn iter(&self) -> BitSet256Iter {
        // Initialize with the first word's value
        BitSet256Iter {
            set: self,
            current: self.bits[0],
            word_idx: 0,
        }
    }
}

pub struct BitSet256Iter<'a> {
    set: &'a BitSet256,
    current: u64,
    word_idx: usize,
}

impl<'a> Iterator for BitSet256Iter<'a> {
    type Item = CompactVertexId;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current != 0 {
                let bit = self.current.trailing_zeros();
                self.current &= self.current - 1; // Clear lowest bit
                return Some((self.word_idx as u16 * 64) + bit as u16);
            }
            self.word_idx += 1;
            if self.word_idx >= 4 {
                return None;
            }
            self.current = self.set.bits[self.word_idx];
        }
    }
}

/// Compact edge representation (8 bytes)
#[derive(Clone, Copy, Default)]
#[repr(C, packed)]
pub struct CompactEdge {
    pub source: CompactVertexId, // 2 bytes
    pub target: CompactVertexId, // 2 bytes
    pub weight: u16,             // 2 bytes (fixed-point 0.01 precision)
    pub flags: u16,              // 2 bytes (active, in_cut, etc.)
}

impl CompactEdge {
    pub const FLAG_ACTIVE: u16 = 0x0001;
    pub const FLAG_IN_CUT: u16 = 0x0002;
    pub const FLAG_TREE_EDGE: u16 = 0x0004;

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.flags & Self::FLAG_ACTIVE != 0
    }

    #[inline(always)]
    pub fn is_in_cut(&self) -> bool {
        self.flags & Self::FLAG_IN_CUT != 0
    }
}

/// Compact witness (40 bytes total)
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct CompactWitness {
    pub membership: BitSet256, // 32 bytes
    pub seed: CompactVertexId, // 2 bytes
    pub boundary_size: u16,    // 2 bytes
    pub cardinality: u16,      // 2 bytes
    pub hash: u16,             // 2 bytes
}

impl CompactWitness {
    pub fn new(seed: CompactVertexId, membership: BitSet256, boundary: u16) -> Self {
        let cardinality = membership.count() as u16;
        let hash = Self::compute_hash(seed, &membership);
        Self {
            membership,
            seed,
            boundary_size: boundary,
            cardinality,
            hash,
        }
    }

    fn compute_hash(seed: CompactVertexId, membership: &BitSet256) -> u16 {
        let mut h = seed as u32;
        for &word in &membership.bits {
            h = h.wrapping_mul(31).wrapping_add(word as u32);
        }
        (h & 0xFFFF) as u16
    }

    #[inline(always)]
    pub fn contains(&self, v: CompactVertexId) -> bool {
        self.membership.contains(v)
    }
}

/// Compact adjacency list (fixed size, ~2KB)
#[derive(Clone)]
#[repr(C)]
pub struct CompactAdjacency {
    /// Offset into neighbors array for each vertex
    pub offsets: [u16; MAX_VERTICES_PER_CORE + 1], // 514 bytes
    /// Packed neighbor list (vertex, edge_id)
    pub neighbors: [(CompactVertexId, CompactEdgeId); MAX_EDGES_PER_CORE * 2], // 2048 bytes
}

impl Default for CompactAdjacency {
    fn default() -> Self {
        Self {
            offsets: [0; MAX_VERTICES_PER_CORE + 1],
            neighbors: [(0, 0); MAX_EDGES_PER_CORE * 2],
        }
    }
}

impl CompactAdjacency {
    pub fn neighbors(&self, v: CompactVertexId) -> &[(CompactVertexId, CompactEdgeId)] {
        let start = self.offsets[v as usize] as usize;
        let end = self.offsets[v as usize + 1] as usize;
        &self.neighbors[start..end]
    }

    pub fn degree(&self, v: CompactVertexId) -> u16 {
        self.offsets[v as usize + 1] - self.offsets[v as usize]
    }
}

/// Memory budget breakdown for 8KB core:
/// - CompactAdjacency: ~3.5KB (514 + 3072 bytes)
/// - Edge array: 384 × 8 = 3KB
/// - CompactWitness: 40 bytes
/// - Other fields: ~12 bytes
/// - Stack/control: ~1.4KB
/// Total: ~6.7KB (fits comfortably in 8KB)

/// Core state for minimum cut (fits in 8KB)
#[repr(C)]
pub struct CompactCoreState {
    /// Adjacency structure (~2.5KB)
    pub adjacency: CompactAdjacency,
    /// Edge storage (4KB)
    pub edges: [CompactEdge; MAX_EDGES_PER_CORE],
    /// Number of active vertices
    pub num_vertices: u16,
    /// Number of active edges
    pub num_edges: u16,
    /// Current minimum cut value
    pub min_cut: u16,
    /// Best witness found
    pub best_witness: CompactWitness,
    /// Instance range [lambda_min, lambda_max]
    pub lambda_min: u16,
    pub lambda_max: u16,
    /// Core ID (0-255)
    pub core_id: u8,
    /// Status flags
    pub status: u8,
}

impl CompactCoreState {
    pub const STATUS_IDLE: u8 = 0;
    pub const STATUS_PROCESSING: u8 = 1;
    pub const STATUS_DONE: u8 = 2;
    pub const STATUS_ERROR: u8 = 3;

    pub const fn size() -> usize {
        size_of::<Self>()
    }
}

// Verify size fits in 8KB
const _: () = assert!(
    CompactCoreState::size() <= 8192,
    "CompactCoreState exceeds 8KB"
);

/// Result communicated back from core (16 bytes)
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct CoreResult {
    pub core_id: u8,
    pub status: u8,
    pub min_cut: u16,
    pub witness_hash: u16,
    pub witness_seed: CompactVertexId,
    pub witness_cardinality: u16,
    pub witness_boundary: u16,
    pub padding: [u8; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitset256_basic() {
        let mut bs = BitSet256::new();
        assert!(!bs.contains(0));

        bs.insert(0);
        bs.insert(100);
        bs.insert(255);

        assert!(bs.contains(0));
        assert!(bs.contains(100));
        assert!(bs.contains(255));
        assert!(!bs.contains(50));

        assert_eq!(bs.count(), 3);
    }

    #[test]
    fn test_bitset256_iter() {
        let mut bs = BitSet256::new();
        bs.insert(1);
        bs.insert(64);
        bs.insert(200);

        let collected: alloc::vec::Vec<_> = bs.iter().collect();
        assert_eq!(collected, alloc::vec![1, 64, 200]);
    }

    #[test]
    fn test_compact_witness_size() {
        // CompactWitness is 64 bytes due to BitSet256's 32-byte alignment
        assert_eq!(size_of::<CompactWitness>(), 64);
    }

    #[test]
    fn test_compact_edge_size() {
        assert_eq!(size_of::<CompactEdge>(), 8);
    }

    #[test]
    fn test_core_state_fits_8kb() {
        assert!(CompactCoreState::size() <= 8192);
        // Print actual size for debugging
        // println!("CompactCoreState size: {} bytes", CompactCoreState::size());
    }

    #[test]
    fn test_bitset_operations() {
        let mut a = BitSet256::new();
        let mut b = BitSet256::new();

        a.insert(1);
        a.insert(2);
        b.insert(2);
        b.insert(3);

        let union = a.union(&b);
        assert!(union.contains(1));
        assert!(union.contains(2));
        assert!(union.contains(3));

        let inter = a.intersection(&b);
        assert!(!inter.contains(1));
        assert!(inter.contains(2));
        assert!(!inter.contains(3));
    }
}
