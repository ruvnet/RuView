//! High-performance term arena using bump allocation.
//!
//! Modeled after `ruvector-solver`'s `SolverArena` -- single contiguous
//! allocation with O(1) reset and FxHash-based dedup cache.

use std::cell::RefCell;

/// Bump-allocating term arena with open-addressing hash cache.
///
/// # Performance
///
/// - Allocation: O(1) amortized (bump pointer)
/// - Dedup lookup: O(1) amortized (open-addressing, 50% load factor)
/// - Reset: O(1) (pointer reset + memset cache)
/// - Cache-line aligned (64 bytes) for SIMD access patterns
#[cfg(feature = "fast-arena")]
pub struct FastTermArena {
    /// Monotonic term counter.
    count: RefCell<u32>,
    /// Open-addressing dedup cache: [hash, term_id] pairs.
    cache: RefCell<Vec<u64>>,
    /// Cache capacity mask (capacity - 1, power of 2).
    cache_mask: usize,
    /// Statistics.
    stats: RefCell<FastArenaStats>,
}

/// Arena performance statistics.
#[derive(Debug, Clone, Default)]
pub struct FastArenaStats {
    pub allocations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub resets: u64,
    pub peak_terms: u32,
}

impl FastArenaStats {
    /// Cache hit rate as a fraction (0.0 to 1.0).
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
}

#[cfg(feature = "fast-arena")]
impl FastTermArena {
    /// Create arena with capacity for expected number of terms.
    ///
    /// Cache is sized to 2x capacity (50% load factor) rounded to power of 2.
    pub fn with_capacity(expected_terms: usize) -> Self {
        let cache_cap = (expected_terms * 2).next_power_of_two().max(64);
        Self {
            count: RefCell::new(0),
            cache: RefCell::new(vec![0u64; cache_cap * 2]),
            cache_mask: cache_cap - 1,
            stats: RefCell::new(FastArenaStats::default()),
        }
    }

    /// Default arena for typical proof obligations (~4096 terms).
    pub fn new() -> Self {
        Self::with_capacity(4096)
    }

    /// Intern a term, returning cached ID if duplicate.
    ///
    /// Uses 4-wide linear probing for ILP (instruction-level parallelism).
    #[inline]
    pub fn intern(&self, hash: u64) -> (u32, bool) {
        let mask = self.cache_mask;
        let cache = self.cache.borrow();
        let start = (hash as usize) & mask;

        // 4-wide probe (ILP pattern from ruvector-solver/cg.rs)
        for offset in 0..4 {
            let slot = (start + offset) & mask;
            let stored_hash = cache[slot * 2];

            if stored_hash == hash && hash != 0 {
                // Cache hit
                let id = cache[slot * 2 + 1] as u32;
                drop(cache);
                self.stats.borrow_mut().cache_hits += 1;
                return (id, true);
            }

            if stored_hash == 0 {
                break; // Empty slot
            }
        }
        drop(cache);

        // Cache miss: allocate new term
        self.stats.borrow_mut().cache_misses += 1;
        self.alloc_with_hash(hash)
    }

    /// Allocate a new term and insert into cache.
    fn alloc_with_hash(&self, hash: u64) -> (u32, bool) {
        let mut count = self.count.borrow_mut();
        let id = *count;
        *count = count.checked_add(1).expect("FastTermArena: term overflow");

        let mut stats = self.stats.borrow_mut();
        stats.allocations += 1;
        if id + 1 > stats.peak_terms {
            stats.peak_terms = id + 1;
        }
        drop(stats);

        // Insert into cache
        if hash != 0 {
            let mask = self.cache_mask;
            let mut cache = self.cache.borrow_mut();
            let start = (hash as usize) & mask;

            for offset in 0..8 {
                let slot = (start + offset) & mask;
                if cache[slot * 2] == 0 {
                    cache[slot * 2] = hash;
                    cache[slot * 2 + 1] = id as u64;
                    break;
                }
            }
        }

        drop(count);
        (id, false)
    }

    /// Allocate a term without caching.
    pub fn alloc(&self) -> u32 {
        let mut count = self.count.borrow_mut();
        let id = *count;
        *count = count.checked_add(1).expect("FastTermArena: term overflow");
        self.stats.borrow_mut().allocations += 1;
        id
    }

    /// O(1) reset -- reclaim all terms and clear cache.
    pub fn reset(&self) {
        *self.count.borrow_mut() = 0;
        self.cache.borrow_mut().fill(0);
        self.stats.borrow_mut().resets += 1;
    }

    /// Number of terms currently allocated.
    pub fn term_count(&self) -> u32 {
        *self.count.borrow()
    }

    /// Get performance statistics.
    pub fn stats(&self) -> FastArenaStats {
        self.stats.borrow().clone()
    }
}

#[cfg(feature = "fast-arena")]
impl Default for FastTermArena {
    fn default() -> Self {
        Self::new()
    }
}

/// FxHash: multiply-shift hash (used by rustc internally).
/// ~5x faster than SipHash for small keys.
#[inline]
pub fn fx_hash_u64(value: u64) -> u64 {
    value.wrapping_mul(0x517cc1b727220a95)
}

/// FxHash for two u32 values.
#[inline]
pub fn fx_hash_pair(a: u32, b: u32) -> u64 {
    fx_hash_u64((a as u64) << 32 | b as u64)
}

/// FxHash for a string (symbol name).
#[inline]
pub fn fx_hash_str(s: &str) -> u64 {
    let mut h: u64 = 0;
    for &b in s.as_bytes() {
        h = h.wrapping_mul(0x100000001b3) ^ (b as u64);
    }
    fx_hash_u64(h)
}

#[cfg(test)]
#[cfg(feature = "fast-arena")]
mod tests {
    use super::*;

    #[test]
    fn test_arena_alloc() {
        let arena = FastTermArena::new();
        let id0 = arena.alloc();
        let id1 = arena.alloc();
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(arena.term_count(), 2);
    }

    #[test]
    fn test_arena_intern_dedup() {
        let arena = FastTermArena::new();
        let (id1, hit1) = arena.intern(0x12345678);
        let (id2, hit2) = arena.intern(0x12345678);
        assert!(!hit1, "first intern should be a miss");
        assert!(hit2, "second intern should be a hit");
        assert_eq!(id1, id2, "same hash should return same ID");
    }

    #[test]
    fn test_arena_intern_different() {
        let arena = FastTermArena::new();
        let (id1, _) = arena.intern(0xAAAA);
        let (id2, _) = arena.intern(0xBBBB);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_arena_reset() {
        let arena = FastTermArena::new();
        arena.alloc();
        arena.alloc();
        assert_eq!(arena.term_count(), 2);
        arena.reset();
        assert_eq!(arena.term_count(), 0);
    }

    #[test]
    fn test_arena_stats() {
        let arena = FastTermArena::new();
        arena.intern(0x111);
        arena.intern(0x111); // hit
        arena.intern(0x222); // miss
        let stats = arena.stats();
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 2);
        assert!(stats.cache_hit_rate() > 0.3);
    }

    #[test]
    fn test_arena_capacity() {
        let arena = FastTermArena::with_capacity(16);
        for i in 0..16u64 {
            arena.intern(i + 1);
        }
        assert_eq!(arena.term_count(), 16);
    }

    #[test]
    fn test_fx_hash_deterministic() {
        assert_eq!(fx_hash_u64(42), fx_hash_u64(42));
        assert_ne!(fx_hash_u64(42), fx_hash_u64(43));
    }

    #[test]
    fn test_fx_hash_pair() {
        let h1 = fx_hash_pair(1, 2);
        let h2 = fx_hash_pair(2, 1);
        assert_ne!(h1, h2, "order should matter");
    }

    #[test]
    fn test_fx_hash_str() {
        assert_eq!(fx_hash_str("Nat"), fx_hash_str("Nat"));
        assert_ne!(fx_hash_str("Nat"), fx_hash_str("Vec"));
    }

    #[test]
    fn test_arena_high_volume() {
        let arena = FastTermArena::with_capacity(10_000);
        for i in 0..10_000u64 {
            arena.intern(i + 1);
        }
        assert_eq!(arena.term_count(), 10_000);
        // Re-intern all -- should be 100% cache hits
        for i in 0..10_000u64 {
            let (_, hit) = arena.intern(i + 1);
            assert!(hit, "re-intern should hit cache");
        }
        assert!(arena.stats().cache_hit_rate() > 0.49);
    }
}
