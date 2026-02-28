//! Count-Min Sketch for temperature tracking.
//!
//! Tracks per-block access frequency to drive tier promotion/demotion.
//! Spec defaults: width=1024, depth=4, 8-bit saturating counters.

use alloc::vec;
use alloc::vec::Vec;

/// Count-Min Sketch for access frequency estimation.
#[derive(Clone, Debug)]
pub struct CountMinSketch {
    /// Counter matrix: `counters[row][col]`, each row uses a different hash.
    pub counters: Vec<Vec<u8>>,
    /// Number of counters per row.
    pub width: usize,
    /// Number of hash functions (rows).
    pub depth: usize,
    /// Total number of increment operations (for aging decisions).
    pub total_accesses: u64,
}

/// Default width (counters per row).
pub const DEFAULT_WIDTH: usize = 1024;

/// Default depth (hash functions / rows).
pub const DEFAULT_DEPTH: usize = 4;

/// Aging trigger: halve all counters every 2^16 accesses.
const AGING_INTERVAL: u64 = 1 << 16;

impl CountMinSketch {
    /// Create a new sketch with the given width and depth.
    pub fn new(width: usize, depth: usize) -> Self {
        Self {
            counters: vec![vec![0u8; width]; depth],
            width,
            depth,
            total_accesses: 0,
        }
    }

    /// Create a sketch with default parameters (w=1024, d=4).
    pub fn default_sketch() -> Self {
        Self::new(DEFAULT_WIDTH, DEFAULT_DEPTH)
    }

    /// Increment the count for `block_id` using saturating addition.
    ///
    /// Updates all `depth` hash rows with `min(counter + 1, 255)`.
    pub fn increment(&mut self, block_id: u64) {
        for row in 0..self.depth {
            let idx = self.hash(block_id, row) % self.width;
            self.counters[row][idx] = self.counters[row][idx].saturating_add(1);
        }
        self.total_accesses = self.total_accesses.wrapping_add(1);
    }

    /// Estimate the access count for `block_id`.
    ///
    /// Returns the minimum across all hash rows (Count-Min guarantee:
    /// estimate >= true count, with bounded overestimation).
    pub fn estimate(&self, block_id: u64) -> u8 {
        let mut min_val = u8::MAX;
        for row in 0..self.depth {
            let idx = self.hash(block_id, row) % self.width;
            min_val = min_val.min(self.counters[row][idx]);
        }
        min_val
    }

    /// Age (decay) all counters by right-shifting by 1 (halving).
    ///
    /// This ensures the sketch tracks *recent* access patterns rather
    /// than cumulative history.
    pub fn age(&mut self) {
        for row in &mut self.counters {
            for counter in row.iter_mut() {
                *counter >>= 1;
            }
        }
    }

    /// Returns true if aging should be triggered (every 2^16 accesses).
    pub fn should_age(&self) -> bool {
        self.total_accesses > 0 && self.total_accesses.is_multiple_of(AGING_INTERVAL)
    }

    /// Memory footprint in bytes (counters only, excluding struct overhead).
    pub fn memory_bytes(&self) -> usize {
        self.width * self.depth
    }

    /// Hash function using FNV-1a style multiplicative hashing.
    ///
    /// Each row uses a different seed to produce independent hash values.
    fn hash(&self, block_id: u64, row: usize) -> usize {
        // FNV-1a inspired: mix block_id with row-dependent seed.
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let seed = HASH_SEEDS[row % HASH_SEEDS.len()];
        let mut h = FNV_OFFSET ^ seed;
        let bytes = block_id.to_le_bytes();
        for &b in &bytes {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
        h as usize
    }
}

/// Seeds for hash functions (one per row).
const HASH_SEEDS: [u64; 8] = [
    0x517cc1b727220a95,
    0x6c62272e07bb0142,
    0x44c6b90e0f294e41,
    0x3b9f7a3e2d8f1c5b,
    0x7e4a1b3c5d6f8a9e,
    0x1a2b3c4d5e6f7089,
    0x9f8e7d6c5b4a3210,
    0xdeadbeefcafebabe,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sketch_all_zeros() {
        let s = CountMinSketch::new(64, 4);
        for row in &s.counters {
            for &c in row {
                assert_eq!(c, 0);
            }
        }
        assert_eq!(s.total_accesses, 0);
    }

    #[test]
    fn estimate_ge_true_count() {
        let mut s = CountMinSketch::new(256, 4);
        let block = 42u64;

        for _ in 0..10 {
            s.increment(block);
        }
        let est = s.estimate(block);
        assert!(est >= 10, "estimate {est} should be >= true count 10");
    }

    #[test]
    fn increment_saturates_at_255() {
        let mut s = CountMinSketch::new(64, 2);
        let block = 1u64;

        for _ in 0..300 {
            s.increment(block);
        }
        let est = s.estimate(block);
        assert_eq!(est, 255);
    }

    #[test]
    fn aging_halves_counters() {
        let mut s = CountMinSketch::new(64, 2);
        let block = 7u64;

        for _ in 0..100 {
            s.increment(block);
        }
        let before = s.estimate(block);
        s.age();
        let after = s.estimate(block);

        // After aging, counts should be approximately halved.
        assert!(after <= before, "aging should not increase count");
        assert!(
            after >= before / 2 - 1,
            "aging should halve: before={before}, after={after}"
        );
    }

    #[test]
    fn should_age_at_power_of_two() {
        let mut s = CountMinSketch::new(64, 2);
        // Not at boundary
        s.total_accesses = 100;
        assert!(!s.should_age());

        // At 2^16
        s.total_accesses = 1 << 16;
        assert!(s.should_age());

        // At 2 * 2^16
        s.total_accesses = 2 << 16;
        assert!(s.should_age());
    }

    #[test]
    fn different_blocks_independent() {
        let mut s = CountMinSketch::new(1024, 4);

        for _ in 0..50 {
            s.increment(100);
        }
        for _ in 0..10 {
            s.increment(200);
        }

        let est_100 = s.estimate(100);
        let est_200 = s.estimate(200);
        assert!(est_100 >= 50);
        assert!(est_200 >= 10);
        assert!(est_100 > est_200);
    }

    #[test]
    fn memory_bytes() {
        let s = CountMinSketch::new(1024, 4);
        assert_eq!(s.memory_bytes(), 4096);
    }

    #[test]
    fn unseen_block_is_zero() {
        let s = CountMinSketch::new(1024, 4);
        assert_eq!(s.estimate(999), 0);
    }
}
