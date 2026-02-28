//! Arena allocator for efficient weight storage.
//!
//! Provides a single contiguous allocation for all model weights,
//! reducing memory fragmentation and improving cache locality.
//!
//! ## Benefits
//!
//! - Single allocation: O(1) allocations vs O(n) for per-layer alloc
//! - Cache locality: Contiguous memory improves prefetch efficiency
//! - Deterministic: No runtime allocator overhead during inference
//! - Alignment: 64-byte aligned for SIMD operations
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ruvector_mincut_gated_transformer::arena::WeightArena;
//!
//! // Calculate total size needed
//! let total_bytes = 1024 * 1024; // 1MB for example
//!
//! // Create arena
//! let mut arena = WeightArena::new(total_bytes);
//!
//! // Allocate slices from arena
//! let w1 = arena.alloc_i8(256 * 1024).unwrap();
//! let w2 = arena.alloc_i8(256 * 1024).unwrap();
//! let scales = arena.alloc_f32(256).unwrap();
//! ```

extern crate alloc;
use alloc::vec::Vec;

/// Cache line size for alignment (64 bytes on most architectures).
const CACHE_LINE: usize = 64;

/// Arena allocator for model weights.
///
/// Provides a single contiguous allocation with bump-pointer allocation.
/// All allocations are aligned to cache line boundaries for optimal access.
#[derive(Debug)]
pub struct WeightArena {
    /// Backing storage
    buffer: Vec<u8>,
    /// Current allocation offset
    offset: usize,
    /// Total capacity
    capacity: usize,
}

impl WeightArena {
    /// Create a new arena with the specified capacity.
    ///
    /// The capacity is rounded up to the nearest cache line boundary.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Minimum capacity in bytes
    pub fn new(capacity: usize) -> Self {
        // Round up to cache line boundary
        let aligned_capacity = (capacity + CACHE_LINE - 1) & !(CACHE_LINE - 1);

        // Allocate with cache line alignment
        // We over-allocate to ensure proper alignment
        let buffer = vec![0u8; aligned_capacity + CACHE_LINE];

        Self {
            buffer,
            offset: 0,
            capacity: aligned_capacity,
        }
    }

    /// Get the current allocation offset.
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Get the total capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get remaining capacity.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.offset)
    }

    /// Check if the arena is empty (no allocations made).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.offset == 0
    }

    /// Reset the arena, allowing reuse of the buffer.
    ///
    /// This does not deallocate memory, just resets the offset.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Get the aligned start of the buffer.
    #[inline]
    fn aligned_start(&self) -> usize {
        let ptr = self.buffer.as_ptr() as usize;
        (ptr + CACHE_LINE - 1) & !(CACHE_LINE - 1)
    }

    /// Allocate a slice of i8 values.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of i8 elements
    ///
    /// # Returns
    ///
    /// Mutable slice of i8, or None if out of capacity
    pub fn alloc_i8(&mut self, count: usize) -> Option<&mut [i8]> {
        let bytes = count;
        let aligned_size = (bytes + CACHE_LINE - 1) & !(CACHE_LINE - 1);

        if self.offset + aligned_size > self.capacity {
            return None;
        }

        let start = self.aligned_start() + self.offset;
        self.offset += aligned_size;

        // SAFETY: We've verified the allocation fits within our buffer,
        // and the alignment is correct. The slice is within bounds.
        unsafe {
            let ptr = start as *mut i8;
            Some(core::slice::from_raw_parts_mut(ptr, count))
        }
    }

    /// Allocate a slice of f32 values.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of f32 elements
    ///
    /// # Returns
    ///
    /// Mutable slice of f32, or None if out of capacity
    pub fn alloc_f32(&mut self, count: usize) -> Option<&mut [f32]> {
        let bytes = count * 4;
        let aligned_size = (bytes + CACHE_LINE - 1) & !(CACHE_LINE - 1);

        if self.offset + aligned_size > self.capacity {
            return None;
        }

        let start = self.aligned_start() + self.offset;
        self.offset += aligned_size;

        // SAFETY: We've verified the allocation fits within our buffer.
        // The start address is aligned to 64 bytes, which exceeds f32's 4-byte requirement.
        unsafe {
            let ptr = start as *mut f32;
            Some(core::slice::from_raw_parts_mut(ptr, count))
        }
    }

    /// Allocate a slice of i32 values.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of i32 elements
    ///
    /// # Returns
    ///
    /// Mutable slice of i32, or None if out of capacity
    pub fn alloc_i32(&mut self, count: usize) -> Option<&mut [i32]> {
        let bytes = count * 4;
        let aligned_size = (bytes + CACHE_LINE - 1) & !(CACHE_LINE - 1);

        if self.offset + aligned_size > self.capacity {
            return None;
        }

        let start = self.aligned_start() + self.offset;
        self.offset += aligned_size;

        // SAFETY: We've verified the allocation fits within our buffer.
        // The start address is aligned to 64 bytes, which exceeds i32's 4-byte requirement.
        unsafe {
            let ptr = start as *mut i32;
            Some(core::slice::from_raw_parts_mut(ptr, count))
        }
    }

    /// Allocate raw bytes with custom alignment.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes
    /// * `align` - Alignment requirement (must be power of 2)
    ///
    /// # Returns
    ///
    /// Mutable byte slice, or None if out of capacity
    pub fn alloc_bytes(&mut self, bytes: usize, align: usize) -> Option<&mut [u8]> {
        debug_assert!(align.is_power_of_two());

        let align = align.max(CACHE_LINE);
        let aligned_size = (bytes + align - 1) & !(align - 1);

        if self.offset + aligned_size > self.capacity {
            return None;
        }

        let start = self.aligned_start() + self.offset;
        self.offset += aligned_size;

        // SAFETY: We've verified the allocation fits within our buffer.
        unsafe {
            let ptr = start as *mut u8;
            Some(core::slice::from_raw_parts_mut(ptr, bytes))
        }
    }
}

/// Calculate total arena size needed for a model.
///
/// # Arguments
///
/// * `layers` - Number of transformer layers
/// * `hidden` - Hidden dimension
/// * `ffn_mult` - FFN intermediate size multiplier (typically 4)
/// * `heads` - Number of attention heads
///
/// # Returns
///
/// Total bytes needed for all weights
pub fn calculate_arena_size(layers: usize, hidden: usize, ffn_mult: usize, _heads: usize) -> usize {
    // Per-layer weights (all i8):
    // - Q, K, V projections: 3 * hidden * hidden
    // - Output projection: hidden * hidden
    // - FFN W1: hidden * (hidden * ffn_mult)
    // - FFN W2: (hidden * ffn_mult) * hidden
    let qkv_size = 3 * hidden * hidden;
    let out_proj_size = hidden * hidden;
    let ffn_w1_size = hidden * (hidden * ffn_mult);
    let ffn_w2_size = (hidden * ffn_mult) * hidden;

    let per_layer_i8 = qkv_size + out_proj_size + ffn_w1_size + ffn_w2_size;

    // Per-layer scales (f32):
    // - Q, K, V scales: 3 * hidden
    // - Output scale: hidden
    // - FFN W1 scales: hidden * ffn_mult
    // - FFN W2 scales: hidden
    let per_layer_f32 = 3 * hidden + hidden + (hidden * ffn_mult) + hidden;

    // Per-layer biases (i32):
    // - Q, K, V bias: 3 * hidden (optional)
    // - Output bias: hidden (optional)
    // - FFN biases: hidden * ffn_mult + hidden (optional)
    let per_layer_i32 = 3 * hidden + hidden + (hidden * ffn_mult) + hidden;

    // Total per layer
    let per_layer = per_layer_i8 + per_layer_f32 * 4 + per_layer_i32 * 4;

    // Multiply by layers, add padding for alignment
    let total = layers * per_layer;
    (total + CACHE_LINE - 1) & !(CACHE_LINE - 1)
}

/// Weight reference into an arena.
///
/// Stores offsets rather than pointers for serialization compatibility.
#[derive(Clone, Copy, Debug)]
pub struct WeightRef {
    /// Offset in arena
    pub offset: u32,
    /// Size in bytes
    pub size: u32,
}

impl WeightRef {
    /// Create a new weight reference.
    pub const fn new(offset: u32, size: u32) -> Self {
        Self { offset, size }
    }

    /// Check if reference is valid (non-zero size).
    pub const fn is_valid(&self) -> bool {
        self.size > 0
    }
}

/// Layer weight references for efficient lookup.
#[derive(Clone, Debug)]
pub struct LayerWeights {
    /// Q projection weights
    pub w_q: WeightRef,
    /// K projection weights
    pub w_k: WeightRef,
    /// V projection weights
    pub w_v: WeightRef,
    /// Output projection weights
    pub w_o: WeightRef,
    /// FFN first layer weights
    pub w_ffn1: WeightRef,
    /// FFN second layer weights
    pub w_ffn2: WeightRef,
    /// Q projection scales
    pub s_q: WeightRef,
    /// K projection scales
    pub s_k: WeightRef,
    /// V projection scales
    pub s_v: WeightRef,
    /// Output projection scales
    pub s_o: WeightRef,
    /// FFN first layer scales
    pub s_ffn1: WeightRef,
    /// FFN second layer scales
    pub s_ffn2: WeightRef,
}

impl LayerWeights {
    /// Create empty layer weights (all zero refs).
    pub const fn empty() -> Self {
        Self {
            w_q: WeightRef::new(0, 0),
            w_k: WeightRef::new(0, 0),
            w_v: WeightRef::new(0, 0),
            w_o: WeightRef::new(0, 0),
            w_ffn1: WeightRef::new(0, 0),
            w_ffn2: WeightRef::new(0, 0),
            s_q: WeightRef::new(0, 0),
            s_k: WeightRef::new(0, 0),
            s_v: WeightRef::new(0, 0),
            s_o: WeightRef::new(0, 0),
            s_ffn1: WeightRef::new(0, 0),
            s_ffn2: WeightRef::new(0, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_basic() {
        let mut arena = WeightArena::new(1024);
        assert!(arena.is_empty());
        assert_eq!(arena.capacity(), 1024);
        assert_eq!(arena.remaining(), 1024);
    }

    #[test]
    fn test_arena_alloc_i8() {
        let mut arena = WeightArena::new(1024);

        let slice = arena.alloc_i8(100).unwrap();
        assert_eq!(slice.len(), 100);

        // Fill and verify
        for (i, v) in slice.iter_mut().enumerate() {
            *v = i as i8;
        }
        assert_eq!(slice[0], 0);
        assert_eq!(slice[99], 99);
    }

    #[test]
    fn test_arena_alloc_f32() {
        let mut arena = WeightArena::new(1024);

        let slice = arena.alloc_f32(50).unwrap();
        assert_eq!(slice.len(), 50);

        // Fill and verify
        for (i, v) in slice.iter_mut().enumerate() {
            *v = i as f32;
        }
        assert_eq!(slice[0], 0.0);
        assert_eq!(slice[49], 49.0);
    }

    #[test]
    fn test_arena_alloc_i32() {
        let mut arena = WeightArena::new(1024);

        let slice = arena.alloc_i32(50).unwrap();
        assert_eq!(slice.len(), 50);

        for (i, v) in slice.iter_mut().enumerate() {
            *v = i as i32;
        }
        assert_eq!(slice[49], 49);
    }

    #[test]
    fn test_arena_out_of_capacity() {
        let mut arena = WeightArena::new(256);

        // First allocation should succeed
        assert!(arena.alloc_i8(100).is_some());

        // Second allocation should succeed (rounds up to 128)
        assert!(arena.alloc_i8(100).is_some());

        // Third should fail (no space left)
        assert!(arena.alloc_i8(100).is_none());
    }

    #[test]
    fn test_arena_reset() {
        let mut arena = WeightArena::new(256);

        arena.alloc_i8(100).unwrap();
        assert!(!arena.is_empty());

        arena.reset();
        assert!(arena.is_empty());
        assert_eq!(arena.remaining(), 256);

        // Can allocate again after reset
        assert!(arena.alloc_i8(100).is_some());
    }

    #[test]
    fn test_calculate_arena_size() {
        // Small model: 4 layers, 128 hidden, 4x FFN, 4 heads
        let size = calculate_arena_size(4, 128, 4, 4);
        assert!(size > 0);
        assert_eq!(size % CACHE_LINE, 0); // Should be aligned

        // Medium model: 12 layers, 768 hidden, 4x FFN, 12 heads
        let size = calculate_arena_size(12, 768, 4, 12);
        assert!(size > 80_000_000); // Should be > 80MB (approx 85MB for this config)
        assert!(size < 200_000_000); // Sanity check upper bound
    }

    #[test]
    fn test_weight_ref() {
        let ref1 = WeightRef::new(0, 100);
        assert!(ref1.is_valid());

        let ref2 = WeightRef::new(100, 0);
        assert!(!ref2.is_valid());
    }

    #[test]
    fn test_layer_weights_empty() {
        let weights = LayerWeights::empty();
        assert!(!weights.w_q.is_valid());
        assert!(!weights.w_k.is_valid());
    }

    #[test]
    fn test_arena_alignment() {
        let mut arena = WeightArena::new(1024);

        // Allocate f32 - should be aligned
        let f32_slice = arena.alloc_f32(10).unwrap();
        let ptr = f32_slice.as_ptr() as usize;
        assert_eq!(ptr % 4, 0, "f32 should be 4-byte aligned");

        // Allocate i32 - should be aligned
        let i32_slice = arena.alloc_i32(10).unwrap();
        let ptr = i32_slice.as_ptr() as usize;
        assert_eq!(ptr % 4, 0, "i32 should be 4-byte aligned");
    }
}
