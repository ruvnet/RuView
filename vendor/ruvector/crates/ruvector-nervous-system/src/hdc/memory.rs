//! Associative memory for hyperdimensional computing
//!
//! Provides high-capacity storage and retrieval of hypervector patterns
//! with 10^40 representational capacity.

use super::vector::Hypervector;
use std::collections::HashMap;

/// Associative memory for storing and retrieving hypervectors
///
/// # Capacity
///
/// - Theoretical: 10^40 distinct patterns
/// - Practical: Limited by available memory (~1.2KB per entry)
///
/// # Performance
///
/// - Store: O(1)
/// - Retrieve: O(N) where N is number of stored items
/// - Can be optimized to O(log N) with spatial indexing
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
///
/// let mut memory = HdcMemory::new();
///
/// // Store concepts
/// let concept_a = Hypervector::random();
/// let concept_b = Hypervector::random();
///
/// memory.store("animal", concept_a.clone());
/// memory.store("plant", concept_b);
///
/// // Retrieve similar concepts
/// let results = memory.retrieve(&concept_a, 0.8);
/// assert_eq!(results[0].0, "animal");
/// assert!(results[0].1 > 0.99);
/// ```
#[derive(Clone, Debug)]
pub struct HdcMemory {
    items: HashMap<String, Hypervector>,
}

impl HdcMemory {
    /// Creates a new empty associative memory
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Creates a memory with pre-allocated capacity
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::HdcMemory;
    ///
    /// let memory = HdcMemory::with_capacity(1000);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: HashMap::with_capacity(capacity),
        }
    }

    /// Stores a hypervector with a key
    ///
    /// If the key already exists, the value is overwritten.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// let vector = Hypervector::random();
    ///
    /// memory.store("my_key", vector);
    /// ```
    pub fn store(&mut self, key: impl Into<String>, value: Hypervector) {
        self.items.insert(key.into(), value);
    }

    /// Retrieves vectors similar to the query above a threshold
    ///
    /// Returns a vector of (key, similarity) pairs sorted by similarity (descending).
    ///
    /// # Arguments
    ///
    /// * `query` - The query hypervector
    /// * `threshold` - Minimum similarity (0.0 to 1.0) to include in results
    ///
    /// # Performance
    ///
    /// O(N) where N is the number of stored items. Each comparison is <100ns.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// let v1 = Hypervector::random();
    ///
    /// memory.store("item1", v1.clone());
    /// memory.store("item2", Hypervector::random());
    ///
    /// let results = memory.retrieve(&v1, 0.9);
    /// assert!(!results.is_empty());
    /// assert_eq!(results[0].0, "item1");
    /// ```
    pub fn retrieve(&self, query: &Hypervector, threshold: f32) -> Vec<(String, f32)> {
        let mut results: Vec<_> = self
            .items
            .iter()
            .map(|(key, vector)| (key.clone(), query.similarity(vector)))
            .filter(|(_, sim)| *sim >= threshold)
            .collect();

        // Sort by similarity descending (NaN-safe: treat NaN as less than any value)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Less));

        results
    }

    /// Retrieves the top-k most similar vectors
    ///
    /// Returns at most k results, sorted by similarity (descending).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    ///
    /// for i in 0..10 {
    ///     memory.store(format!("item{}", i), Hypervector::random());
    /// }
    ///
    /// let query = Hypervector::random();
    /// let top5 = memory.retrieve_top_k(&query, 5);
    ///
    /// assert!(top5.len() <= 5);
    /// ```
    pub fn retrieve_top_k(&self, query: &Hypervector, k: usize) -> Vec<(String, f32)> {
        let mut results: Vec<_> = self
            .items
            .iter()
            .map(|(key, vector)| (key.clone(), query.similarity(vector)))
            .collect();

        // Partial sort to get top k (NaN-safe)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Less));

        results.into_iter().take(k).collect()
    }

    /// Gets a stored vector by key
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// let vector = Hypervector::random();
    ///
    /// memory.store("key", vector.clone());
    ///
    /// let retrieved = memory.get("key").unwrap();
    /// assert_eq!(&vector, retrieved);
    /// ```
    pub fn get(&self, key: &str) -> Option<&Hypervector> {
        self.items.get(key)
    }

    /// Checks if a key exists in memory
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    ///
    /// assert!(!memory.contains_key("key"));
    /// memory.store("key", Hypervector::random());
    /// assert!(memory.contains_key("key"));
    /// ```
    pub fn contains_key(&self, key: &str) -> bool {
        self.items.contains_key(key)
    }

    /// Removes a vector by key
    ///
    /// Returns the removed vector if it existed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// let vector = Hypervector::random();
    ///
    /// memory.store("key", vector.clone());
    /// let removed = memory.remove("key").unwrap();
    /// assert_eq!(vector, removed);
    /// assert!(!memory.contains_key("key"));
    /// ```
    pub fn remove(&mut self, key: &str) -> Option<Hypervector> {
        self.items.remove(key)
    }

    /// Returns the number of stored vectors
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// assert_eq!(memory.len(), 0);
    ///
    /// memory.store("key", Hypervector::random());
    /// assert_eq!(memory.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Checks if the memory is empty
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::HdcMemory;
    ///
    /// let memory = HdcMemory::new();
    /// assert!(memory.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clears all stored vectors
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// memory.store("key", Hypervector::random());
    ///
    /// memory.clear();
    /// assert!(memory.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns an iterator over all keys
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// memory.store("key1", Hypervector::random());
    /// memory.store("key2", Hypervector::random());
    ///
    /// let keys: Vec<_> = memory.keys().collect();
    /// assert_eq!(keys.len(), 2);
    /// ```
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.items.keys()
    }

    /// Returns an iterator over all (key, vector) pairs
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
    ///
    /// let mut memory = HdcMemory::new();
    /// memory.store("key", Hypervector::random());
    ///
    /// for (key, vector) in memory.iter() {
    ///     println!("{}: {:?}", key, vector);
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Hypervector)> {
        self.items.iter()
    }
}

impl Default for HdcMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_memory_empty() {
        let memory = HdcMemory::new();
        assert_eq!(memory.len(), 0);
        assert!(memory.is_empty());
    }

    #[test]
    fn test_store_and_get() {
        let mut memory = HdcMemory::new();
        let vector = Hypervector::random();

        memory.store("key", vector.clone());

        assert_eq!(memory.len(), 1);
        assert_eq!(memory.get("key").unwrap(), &vector);
    }

    #[test]
    fn test_store_overwrite() {
        let mut memory = HdcMemory::new();
        let v1 = Hypervector::from_seed(1);
        let v2 = Hypervector::from_seed(2);

        memory.store("key", v1);
        memory.store("key", v2.clone());

        assert_eq!(memory.len(), 1);
        assert_eq!(memory.get("key").unwrap(), &v2);
    }

    #[test]
    fn test_retrieve_exact_match() {
        let mut memory = HdcMemory::new();
        let vector = Hypervector::random();

        memory.store("exact", vector.clone());

        let results = memory.retrieve(&vector, 0.99);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "exact");
        assert!(results[0].1 > 0.99);
    }

    #[test]
    fn test_retrieve_threshold() {
        let mut memory = HdcMemory::new();

        let v1 = Hypervector::from_seed(1);
        let v2 = Hypervector::from_seed(2);
        let v3 = Hypervector::from_seed(3);

        memory.store("v1", v1.clone());
        memory.store("v2", v2);
        memory.store("v3", v3);

        // High threshold should return only exact match
        let results = memory.retrieve(&v1, 0.99);
        assert_eq!(results.len(), 1);

        // Low threshold (-1.0 is min similarity) should return all
        let results = memory.retrieve(&v1, -1.0);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_retrieve_sorted() {
        let mut memory = HdcMemory::new();

        for i in 0..5 {
            memory.store(format!("v{}", i), Hypervector::from_seed(i));
        }

        let query = Hypervector::from_seed(0);
        let results = memory.retrieve(&query, 0.0);

        // Should be sorted by similarity descending
        for i in 0..(results.len() - 1) {
            assert!(results[i].1 >= results[i + 1].1);
        }
    }

    #[test]
    fn test_retrieve_top_k() {
        let mut memory = HdcMemory::new();

        for i in 0..10 {
            memory.store(format!("v{}", i), Hypervector::from_seed(i));
        }

        let query = Hypervector::random();
        let top3 = memory.retrieve_top_k(&query, 3);

        assert_eq!(top3.len(), 3);

        // Should be sorted
        assert!(top3[0].1 >= top3[1].1);
        assert!(top3[1].1 >= top3[2].1);
    }

    #[test]
    fn test_retrieve_top_k_more_than_stored() {
        let mut memory = HdcMemory::new();

        for i in 0..3 {
            memory.store(format!("v{}", i), Hypervector::random());
        }

        let results = memory.retrieve_top_k(&Hypervector::random(), 10);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_contains_key() {
        let mut memory = HdcMemory::new();

        assert!(!memory.contains_key("key"));

        memory.store("key", Hypervector::random());
        assert!(memory.contains_key("key"));
    }

    #[test]
    fn test_remove() {
        let mut memory = HdcMemory::new();
        let vector = Hypervector::random();

        memory.store("key", vector.clone());
        assert_eq!(memory.len(), 1);

        let removed = memory.remove("key").unwrap();
        assert_eq!(removed, vector);
        assert_eq!(memory.len(), 0);
        assert!(!memory.contains_key("key"));
    }

    #[test]
    fn test_clear() {
        let mut memory = HdcMemory::new();

        for i in 0..5 {
            memory.store(format!("v{}", i), Hypervector::random());
        }

        assert_eq!(memory.len(), 5);

        memory.clear();
        assert_eq!(memory.len(), 0);
        assert!(memory.is_empty());
    }

    #[test]
    fn test_keys_iterator() {
        let mut memory = HdcMemory::new();

        memory.store("key1", Hypervector::random());
        memory.store("key2", Hypervector::random());
        memory.store("key3", Hypervector::random());

        let keys: Vec<_> = memory.keys().collect();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_iter() {
        let mut memory = HdcMemory::new();

        for i in 0..3 {
            memory.store(format!("v{}", i), Hypervector::from_seed(i));
        }

        let mut count = 0;
        for (key, vector) in memory.iter() {
            assert!(key.starts_with("v"));
            assert!(vector.popcount() > 0);
            count += 1;
        }

        assert_eq!(count, 3);
    }

    #[test]
    fn test_with_capacity() {
        let memory = HdcMemory::with_capacity(100);
        assert!(memory.is_empty());
    }
}
