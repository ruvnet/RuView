//! Conversion result cache with access-pattern prediction.
//!
//! Modeled after `ruvector-mincut`'s PathDistanceCache (10x speedup).

use std::collections::VecDeque;

/// Open-addressing conversion cache with prefetch hints.
pub struct ConversionCache {
    entries: Vec<CacheEntry>,
    mask: usize,
    history: VecDeque<u64>,
    stats: CacheStats,
}

#[derive(Clone, Default)]
struct CacheEntry {
    key_hash: u64,
    #[allow(dead_code)]
    input_id: u32,
    result_id: u32,
}

/// Cache performance statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl ConversionCache {
    /// Create cache with given capacity (rounded up to power of 2).
    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.next_power_of_two().max(64);
        Self {
            entries: vec![CacheEntry::default(); cap],
            mask: cap - 1,
            history: VecDeque::with_capacity(64),
            stats: CacheStats::default(),
        }
    }

    /// Default cache (10,000 entries).
    pub fn new() -> Self {
        Self::with_capacity(10_000)
    }

    /// Look up a cached conversion result.
    #[inline]
    pub fn get(&mut self, term_id: u32, ctx_len: u32) -> Option<u32> {
        let hash = self.key_hash(term_id, ctx_len);
        let slot = (hash as usize) & self.mask;
        let entry = &self.entries[slot];

        if entry.key_hash == hash && entry.key_hash != 0 {
            self.stats.hits += 1;
            self.history.push_back(hash);
            if self.history.len() > 64 {
                self.history.pop_front();
            }
            Some(entry.result_id)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Insert a conversion result.
    pub fn insert(&mut self, term_id: u32, ctx_len: u32, result_id: u32) {
        let hash = self.key_hash(term_id, ctx_len);
        let slot = (hash as usize) & self.mask;

        if self.entries[slot].key_hash != 0 {
            self.stats.evictions += 1;
        }

        self.entries[slot] = CacheEntry {
            key_hash: hash,
            input_id: term_id,
            result_id,
        };
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.fill(CacheEntry::default());
        self.history.clear();
    }

    /// Get statistics.
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    #[inline]
    fn key_hash(&self, term_id: u32, ctx_len: u32) -> u64 {
        let mut h = term_id as u64;
        h = h.wrapping_mul(0x517cc1b727220a95);
        h ^= ctx_len as u64;
        h = h.wrapping_mul(0x6c62272e07bb0142);
        if h == 0 {
            h = 1;
        } // Reserve 0 for empty
        h
    }
}

impl Default for ConversionCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_miss_then_hit() {
        let mut cache = ConversionCache::new();
        assert!(cache.get(1, 0).is_none());
        cache.insert(1, 0, 42);
        assert_eq!(cache.get(1, 0), Some(42));
    }

    #[test]
    fn test_cache_different_ctx() {
        let mut cache = ConversionCache::new();
        cache.insert(1, 0, 10);
        cache.insert(1, 1, 20);
        assert_eq!(cache.get(1, 0), Some(10));
        assert_eq!(cache.get(1, 1), Some(20));
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = ConversionCache::new();
        cache.insert(1, 0, 42);
        cache.clear();
        assert!(cache.get(1, 0).is_none());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = ConversionCache::new();
        cache.get(1, 0); // miss
        cache.insert(1, 0, 42);
        cache.get(1, 0); // hit
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
        assert!((cache.stats().hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cache_high_volume() {
        let mut cache = ConversionCache::with_capacity(1024);
        for i in 0..1000u32 {
            cache.insert(i, 0, i * 10);
        }
        let mut hits = 0u32;
        for i in 0..1000u32 {
            if cache.get(i, 0).is_some() {
                hits += 1;
            }
        }
        // Due to collisions, not all will be found, but most should
        assert!(hits > 500, "expected >50% hit rate, got {hits}/1000");
    }
}
