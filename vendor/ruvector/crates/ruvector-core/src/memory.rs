//! Memory management utilities for ruvector-core
//!
//! This module provides memory-efficient data structures and utilities
//! for vector storage operations.

/// Memory pool for vector allocations.
#[derive(Debug, Default)]
pub struct MemoryPool {
    /// Total allocated bytes.
    allocated: usize,
    /// Maximum allocation limit.
    limit: Option<usize>,
}

impl MemoryPool {
    /// Create a new memory pool.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a memory pool with a limit.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            allocated: 0,
            limit: Some(limit),
        }
    }

    /// Get currently allocated bytes.
    pub fn allocated(&self) -> usize {
        self.allocated
    }

    /// Get the allocation limit, if any.
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }
}
