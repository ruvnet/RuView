//! Bloom filters for fast negative lookups
//!
//! Bloom filters provide O(1) membership tests with false positives
//! but no false negatives, perfect for quickly eliminating non-existent keys.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Standard bloom filter with configurable size and hash functions
pub struct BloomFilter {
    /// Bit array
    bits: Vec<u64>,
    /// Number of hash functions
    num_hashes: usize,
    /// Number of bits
    num_bits: usize,
}

impl BloomFilter {
    /// Create a new bloom filter
    ///
    /// # Arguments
    /// * `expected_items` - Expected number of items to be inserted
    /// * `false_positive_rate` - Desired false positive rate (e.g., 0.01 for 1%)
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        let num_bits = Self::optimal_num_bits(expected_items, false_positive_rate);
        let num_hashes = Self::optimal_num_hashes(expected_items, num_bits);

        let num_u64s = (num_bits + 63) / 64;

        Self {
            bits: vec![0; num_u64s],
            num_hashes,
            num_bits,
        }
    }

    /// Calculate optimal number of bits
    fn optimal_num_bits(n: usize, p: f64) -> usize {
        let ln2 = std::f64::consts::LN_2;
        (-(n as f64) * p.ln() / (ln2 * ln2)).ceil() as usize
    }

    /// Calculate optimal number of hash functions
    fn optimal_num_hashes(n: usize, m: usize) -> usize {
        let ln2 = std::f64::consts::LN_2;
        ((m as f64 / n as f64) * ln2).ceil() as usize
    }

    /// Insert an item into the bloom filter
    pub fn insert<T: Hash>(&mut self, item: &T) {
        for i in 0..self.num_hashes {
            let hash = self.hash(item, i);
            let bit_index = hash % self.num_bits;
            let array_index = bit_index / 64;
            let bit_offset = bit_index % 64;

            self.bits[array_index] |= 1u64 << bit_offset;
        }
    }

    /// Check if an item might be in the set
    ///
    /// Returns true if the item might be present (with possible false positive)
    /// Returns false if the item is definitely not present
    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        for i in 0..self.num_hashes {
            let hash = self.hash(item, i);
            let bit_index = hash % self.num_bits;
            let array_index = bit_index / 64;
            let bit_offset = bit_index % 64;

            if (self.bits[array_index] & (1u64 << bit_offset)) == 0 {
                return false;
            }
        }
        true
    }

    /// Hash function for bloom filter
    fn hash<T: Hash>(&self, item: &T, i: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        i.hash(&mut hasher);
        hasher.finish() as usize
    }

    /// Clear the bloom filter
    pub fn clear(&mut self) {
        self.bits.fill(0);
    }

    /// Get approximate number of items (based on bit saturation)
    pub fn approximate_count(&self) -> usize {
        let set_bits: u32 = self.bits.iter().map(|&word| word.count_ones()).sum();

        let m = self.num_bits as f64;
        let k = self.num_hashes as f64;
        let x = set_bits as f64;

        // Formula: n ≈ -(m/k) * ln(1 - x/m)
        let n = -(m / k) * (1.0 - x / m).ln();
        n as usize
    }

    /// Get current false positive rate estimate
    pub fn current_false_positive_rate(&self) -> f64 {
        let set_bits: u32 = self.bits.iter().map(|&word| word.count_ones()).sum();

        let p = set_bits as f64 / self.num_bits as f64;
        p.powi(self.num_hashes as i32)
    }
}

/// Scalable bloom filter that grows as needed
pub struct ScalableBloomFilter {
    /// Current active filter
    filters: Vec<BloomFilter>,
    /// Items per filter
    items_per_filter: usize,
    /// Target false positive rate
    false_positive_rate: f64,
    /// Growth factor
    growth_factor: f64,
    /// Current item count
    item_count: usize,
}

impl ScalableBloomFilter {
    /// Create a new scalable bloom filter
    pub fn new(initial_capacity: usize, false_positive_rate: f64) -> Self {
        let initial_filter = BloomFilter::new(initial_capacity, false_positive_rate);

        Self {
            filters: vec![initial_filter],
            items_per_filter: initial_capacity,
            false_positive_rate,
            growth_factor: 2.0,
            item_count: 0,
        }
    }

    /// Insert an item
    pub fn insert<T: Hash>(&mut self, item: &T) {
        // Check if we need to add a new filter
        if self.item_count >= self.items_per_filter * self.filters.len() {
            let new_capacity = (self.items_per_filter as f64 * self.growth_factor) as usize;
            let new_filter = BloomFilter::new(new_capacity, self.false_positive_rate);
            self.filters.push(new_filter);
        }

        // Insert into the most recent filter
        if let Some(filter) = self.filters.last_mut() {
            filter.insert(item);
        }

        self.item_count += 1;
    }

    /// Check if item might be present
    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        // Check all filters (item could be in any of them)
        self.filters.iter().any(|filter| filter.contains(item))
    }

    /// Clear all filters
    pub fn clear(&mut self) {
        for filter in &mut self.filters {
            filter.clear();
        }
        self.item_count = 0;
    }

    /// Get number of filters
    pub fn num_filters(&self) -> usize {
        self.filters.len()
    }

    /// Get total memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.filters.iter().map(|f| f.bits.len() * 8).sum()
    }
}

impl Default for ScalableBloomFilter {
    fn default() -> Self {
        Self::new(1000, 0.01)
    }
}

/// Counting bloom filter (supports deletion)
pub struct CountingBloomFilter {
    /// Counter array (4-bit counters)
    counters: Vec<u8>,
    /// Number of hash functions
    num_hashes: usize,
    /// Number of counters
    num_counters: usize,
}

impl CountingBloomFilter {
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        let num_counters = BloomFilter::optimal_num_bits(expected_items, false_positive_rate);
        let num_hashes = BloomFilter::optimal_num_hashes(expected_items, num_counters);

        Self {
            counters: vec![0; num_counters],
            num_hashes,
            num_counters,
        }
    }

    pub fn insert<T: Hash>(&mut self, item: &T) {
        for i in 0..self.num_hashes {
            let hash = self.hash(item, i);
            let index = hash % self.num_counters;

            // Increment counter (saturate at 15)
            if self.counters[index] < 15 {
                self.counters[index] += 1;
            }
        }
    }

    pub fn remove<T: Hash>(&mut self, item: &T) {
        for i in 0..self.num_hashes {
            let hash = self.hash(item, i);
            let index = hash % self.num_counters;

            // Decrement counter
            if self.counters[index] > 0 {
                self.counters[index] -= 1;
            }
        }
    }

    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        for i in 0..self.num_hashes {
            let hash = self.hash(item, i);
            let index = hash % self.num_counters;

            if self.counters[index] == 0 {
                return false;
            }
        }
        true
    }

    fn hash<T: Hash>(&self, item: &T, i: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        i.hash(&mut hasher);
        hasher.finish() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter() {
        let mut filter = BloomFilter::new(1000, 0.01);

        filter.insert(&"hello");
        filter.insert(&"world");
        filter.insert(&12345);

        assert!(filter.contains(&"hello"));
        assert!(filter.contains(&"world"));
        assert!(filter.contains(&12345));
        assert!(!filter.contains(&"nonexistent"));
    }

    #[test]
    fn test_bloom_filter_false_positive_rate() {
        let mut filter = BloomFilter::new(100, 0.01);

        // Insert 100 items
        for i in 0..100 {
            filter.insert(&i);
        }

        // Check false positive rate
        let mut false_positives = 0;
        let test_items = 1000;

        for i in 100..(100 + test_items) {
            if filter.contains(&i) {
                false_positives += 1;
            }
        }

        let rate = false_positives as f64 / test_items as f64;
        assert!(rate < 0.05, "False positive rate too high: {}", rate);
    }

    #[test]
    fn test_scalable_bloom_filter() {
        let mut filter = ScalableBloomFilter::new(10, 0.01);

        // Insert many items (more than initial capacity)
        for i in 0..100 {
            filter.insert(&i);
        }

        assert!(filter.num_filters() > 1);

        // All items should be found
        for i in 0..100 {
            assert!(filter.contains(&i));
        }
    }

    #[test]
    fn test_counting_bloom_filter() {
        let mut filter = CountingBloomFilter::new(100, 0.01);

        filter.insert(&"test");
        assert!(filter.contains(&"test"));

        filter.remove(&"test");
        assert!(!filter.contains(&"test"));
    }

    #[test]
    fn test_bloom_clear() {
        let mut filter = BloomFilter::new(100, 0.01);

        filter.insert(&"test");
        assert!(filter.contains(&"test"));

        filter.clear();
        assert!(!filter.contains(&"test"));
    }
}
