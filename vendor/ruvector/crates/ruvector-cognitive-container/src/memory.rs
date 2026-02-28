use serde::{Deserialize, Serialize};

use crate::error::{ContainerError, Result};

/// Configuration for memory slab layout.
///
/// Each budget defines the byte size of a sub-arena within the slab.
/// The total slab size is the sum of all budgets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Total slab size in bytes (must equal sum of budgets).
    pub slab_size: usize,
    /// Bytes reserved for graph adjacency data.
    pub graph_budget: usize,
    /// Bytes reserved for feature / embedding storage.
    pub feature_budget: usize,
    /// Bytes reserved for solver scratch space.
    pub solver_budget: usize,
    /// Bytes reserved for witness receipt storage.
    pub witness_budget: usize,
    /// Bytes reserved for evidence accumulation.
    pub evidence_budget: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            slab_size: 4 * 1024 * 1024,   // 4 MB total
            graph_budget: 1024 * 1024,    // 1 MB
            feature_budget: 1024 * 1024,  // 1 MB
            solver_budget: 512 * 1024,    // 512 KB
            witness_budget: 512 * 1024,   // 512 KB
            evidence_budget: 1024 * 1024, // 1 MB
        }
    }
}

impl MemoryConfig {
    /// Validate that budget components sum to `slab_size`.
    pub fn validate(&self) -> Result<()> {
        let sum = self.graph_budget
            + self.feature_budget
            + self.solver_budget
            + self.witness_budget
            + self.evidence_budget;
        if sum != self.slab_size {
            return Err(ContainerError::InvalidConfig {
                reason: format!(
                    "budget sum ({sum}) does not equal slab_size ({})",
                    self.slab_size
                ),
            });
        }
        Ok(())
    }
}

/// A contiguous block of memory backing all container arenas.
pub struct MemorySlab {
    data: Vec<u8>,
    config: MemoryConfig,
}

impl MemorySlab {
    /// Allocate a new slab according to `config`.
    pub fn new(config: MemoryConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            data: vec![0u8; config.slab_size],
            config,
        })
    }

    /// Total slab size in bytes.
    pub fn total_size(&self) -> usize {
        self.data.len()
    }

    /// Immutable view of the raw slab bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Reference to the underlying config.
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }
}

/// A bump-allocator arena within a `MemorySlab`.
///
/// `base_offset` is the starting position inside the slab.
/// Allocations grow upward; `reset()` reclaims all space.
pub struct Arena {
    base_offset: usize,
    size: usize,
    offset: usize,
}

impl Arena {
    /// Create a new arena starting at `base_offset` with the given `size`.
    pub fn new(base_offset: usize, size: usize) -> Self {
        Self {
            base_offset,
            size,
            offset: 0,
        }
    }

    /// Bump-allocate `size` bytes with the given `align`ment.
    ///
    /// Returns the absolute offset within the slab on success.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<usize> {
        let align = align.max(1);
        let current = self.base_offset + self.offset;
        let aligned = (current + align - 1) & !(align - 1);
        let padding = aligned - current;
        let total = padding + size;

        if self.offset + total > self.size {
            return Err(ContainerError::AllocationFailed {
                requested: size,
                available: self.remaining(),
            });
        }

        self.offset += total;
        Ok(aligned)
    }

    /// Reset the arena, reclaiming all allocated space.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Number of bytes currently consumed (including alignment padding).
    pub fn used(&self) -> usize {
        self.offset
    }

    /// Number of bytes still available.
    pub fn remaining(&self) -> usize {
        self.size.saturating_sub(self.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_slab_creation() {
        let config = MemoryConfig::default();
        let slab = MemorySlab::new(config).expect("slab should allocate");
        assert_eq!(slab.total_size(), 4 * 1024 * 1024);
        assert_eq!(slab.as_bytes().len(), slab.total_size());
        // Fresh slab is zero-filled.
        assert!(slab.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_memory_config_validation_fails_on_mismatch() {
        let config = MemoryConfig {
            slab_size: 100,
            graph_budget: 10,
            feature_budget: 10,
            solver_budget: 10,
            witness_budget: 10,
            evidence_budget: 10,
        };
        assert!(MemorySlab::new(config).is_err());
    }

    #[test]
    fn test_arena_allocation() {
        let mut arena = Arena::new(0, 256);
        assert_eq!(arena.remaining(), 256);
        assert_eq!(arena.used(), 0);

        let off1 = arena.alloc(64, 8).expect("alloc 64");
        assert_eq!(off1, 0); // base 0, align 8 => 0
        assert_eq!(arena.used(), 64);
        assert_eq!(arena.remaining(), 192);

        let off2 = arena.alloc(32, 16).expect("alloc 32");
        // 64 already used, align to 16 => 64 (already aligned)
        assert_eq!(off2, 64);
        assert_eq!(arena.used(), 96);

        arena.reset();
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.remaining(), 256);
    }

    #[test]
    fn test_arena_allocation_overflow() {
        let mut arena = Arena::new(0, 64);
        assert!(arena.alloc(128, 1).is_err());
    }

    #[test]
    fn test_arena_alignment_padding() {
        let mut arena = Arena::new(0, 256);
        // Allocate 1 byte at alignment 1
        let _ = arena.alloc(1, 1).unwrap();
        assert_eq!(arena.used(), 1);
        // Next allocation with align 16: from offset 1, aligned to 16 => 16
        let off = arena.alloc(8, 16).unwrap();
        assert_eq!(off, 16);
        // used = 1 (first) + 15 (padding) + 8 = 24
        assert_eq!(arena.used(), 24);
    }
}
