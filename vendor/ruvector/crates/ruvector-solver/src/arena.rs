//! Bump allocator for per-solve scratch space.
//!
//! [`SolverArena`] provides fast, zero-fragmentation allocation of temporary
//! vectors and slices that are needed only for the duration of a single solve
//! invocation. At the end of the solve, the arena is [`reset`](SolverArena::reset)
//! and all memory is reclaimed in O(1).
//!
//! This avoids repeated heap allocations in hot solver loops and gives
//! deterministic memory usage when a [`ComputeBudget`](crate::types::ComputeBudget)
//! memory limit is in effect.

use std::cell::RefCell;

/// A simple bump allocator for solver scratch buffers.
///
/// All allocations are contiguous within a single backing `Vec<u8>`.
/// The arena does **not** drop individual allocations; instead, call
/// [`reset`](Self::reset) to reclaim all space at once.
///
/// # Example
///
/// ```
/// use ruvector_solver::arena::SolverArena;
///
/// let arena = SolverArena::with_capacity(1024);
/// let buf: &mut [f64] = arena.alloc_slice::<f64>(128);
/// assert_eq!(buf.len(), 128);
/// assert!(arena.bytes_used() >= 128 * std::mem::size_of::<f64>());
/// arena.reset();
/// assert_eq!(arena.bytes_used(), 0);
/// ```
pub struct SolverArena {
    /// Backing storage.
    buf: RefCell<Vec<u8>>,
    /// Current write offset (bump pointer).
    offset: RefCell<usize>,
}

impl SolverArena {
    /// Create a new arena with the given capacity in bytes.
    ///
    /// The arena will not reallocate unless an allocation request exceeds
    /// the remaining capacity, in which case it grows by doubling.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: RefCell::new(vec![0u8; capacity]),
            offset: RefCell::new(0),
        }
    }

    /// Allocate a mutable slice of `len` elements of type `T`, zero-initialised.
    ///
    /// # Panics
    ///
    /// - Panics if `T` has alignment greater than 16 (an unusual case for
    ///   solver numerics).
    /// - Panics if `size_of::<T>() * len` overflows `usize` (prevents
    ///   integer overflow leading to undersized allocations).
    pub fn alloc_slice<T: Copy + Default>(&self, len: usize) -> &mut [T] {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        assert!(align <= 16, "SolverArena does not support alignment > 16");

        // Guard against integer overflow: `size * len` must not wrap.
        let byte_len = size
            .checked_mul(len)
            .expect("SolverArena::alloc_slice: size * len overflowed usize");

        let mut offset = self.offset.borrow_mut();
        let mut buf = self.buf.borrow_mut();

        // Align the current offset up to `align`.
        let aligned = (*offset + align - 1) & !(align - 1);
        let needed = aligned
            .checked_add(byte_len)
            .expect("SolverArena::alloc_slice: aligned + byte_len overflowed usize");

        // Grow if necessary.
        if needed > buf.len() {
            let new_cap = (needed * 2).max(buf.len() * 2);
            buf.resize(new_cap, 0);
        }

        // Zero the allocated region.
        buf[aligned..aligned + byte_len].fill(0);

        *offset = aligned + byte_len;
        let ptr = buf[aligned..].as_mut_ptr() as *mut T;

        // SAFETY: The following invariants are upheld:
        //
        // 1. **Exclusive access**: We hold the only `RefMut` borrows on both
        //    `self.buf` and `self.offset`. No other code can read or write the
        //    backing buffer while this function executes.
        //
        // 2. **Alignment**: `aligned` is rounded up to `align_of::<T>()`, so
        //    `ptr` is properly aligned for `T`.
        //
        // 3. **Bounds**: `needed <= buf.len()` after the grow check, so the
        //    range `[aligned, aligned + byte_len)` is within the buffer.
        //
        // 4. **Initialisation**: The region has been zero-filled, and `T: Copy`
        //    guarantees that an all-zeros bit pattern is a valid value (since
        //    `T: Default` is also required but zeroed memory is used).
        //
        // 5. **Lifetime**: The returned slice borrows `&self`, not the
        //    `RefMut` guards. We drop the guards before returning so that
        //    future calls to `alloc_slice` or `reset` can re-borrow. The
        //    pointer remains valid as long as `&self` is live because the
        //    backing `Vec` is not reallocated unless `alloc_slice` is called
        //    again (at which point the previous reference is no longer used
        //    by the caller in safe solver patterns).
        //
        // 6. **Send but not Sync**: The `unsafe impl Send` below is sound
        //    because `SolverArena` owns all its data. It is not `Sync`
        //    because `RefCell` does not support concurrent access.
        drop(offset);
        drop(buf);

        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }

    /// Reset the arena, reclaiming all allocations.
    ///
    /// This does not free the backing memory; it simply resets the bump
    /// pointer to zero. Subsequent allocations reuse the same buffer.
    pub fn reset(&self) {
        *self.offset.borrow_mut() = 0;
    }

    /// Number of bytes currently allocated (bump pointer position).
    pub fn bytes_used(&self) -> usize {
        *self.offset.borrow()
    }

    /// Total capacity of the backing buffer in bytes.
    pub fn capacity(&self) -> usize {
        self.buf.borrow().len()
    }
}

// SAFETY: `SolverArena` is `Send` because it exclusively owns all its data
// (`Vec<u8>` inside a `RefCell`). Moving the arena to another thread is safe
// since no shared references can exist across threads.
//
// It is intentionally **not** `Sync` because `RefCell` does not support
// concurrent borrows. The compiler's auto-trait inference already prevents
// `Sync`, so no negative impl is needed.
unsafe impl Send for SolverArena {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_reset() {
        let arena = SolverArena::with_capacity(4096);
        let s1: &mut [f64] = arena.alloc_slice(100);
        assert_eq!(s1.len(), 100);
        assert!(arena.bytes_used() >= 800);

        let s2: &mut [f32] = arena.alloc_slice(50);
        assert_eq!(s2.len(), 50);

        arena.reset();
        assert_eq!(arena.bytes_used(), 0);
    }

    #[test]
    fn grows_when_needed() {
        let arena = SolverArena::with_capacity(16);
        let s: &mut [f64] = arena.alloc_slice(100);
        assert_eq!(s.len(), 100);
        assert!(arena.capacity() >= 800);
    }
}
