//! Conflict-Free Replicated Data Types (CRDTs)
//!
//! Implements CRDTs for eventual consistency across federation:
//! - G-Set (Grow-only Set)
//! - LWW-Register (Last-Writer-Wins Register)
//! - Reconciliation algorithms

use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Grow-only Set CRDT
///
/// A set that only supports additions. Merge is simply union.
/// This is useful for accumulating search results from multiple peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GSet<T: Eq + std::hash::Hash + Clone> {
    elements: HashSet<T>,
}

impl<T: Eq + std::hash::Hash + Clone> GSet<T> {
    /// Create a new empty G-Set
    pub fn new() -> Self {
        Self {
            elements: HashSet::new(),
        }
    }

    /// Add an element to the set
    pub fn add(&mut self, element: T) {
        self.elements.insert(element);
    }

    /// Check if set contains element
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }

    /// Get all elements
    pub fn elements(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }

    /// Get the size of the set
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Merge with another G-Set
    ///
    /// G-Set merge is simply the union of both sets.
    /// This operation is:
    /// - Commutative: merge(A, B) = merge(B, A)
    /// - Associative: merge(merge(A, B), C) = merge(A, merge(B, C))
    /// - Idempotent: merge(A, A) = A
    pub fn merge(&mut self, other: &GSet<T>) {
        for element in &other.elements {
            self.elements.insert(element.clone());
        }
    }
}

impl<T: Eq + std::hash::Hash + Clone> Default for GSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Last-Writer-Wins Register CRDT
///
/// A register that resolves conflicts by timestamp.
/// The value with the highest timestamp wins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T: Clone> {
    value: T,
    timestamp: u64,
}

impl<T: Clone> LWWRegister<T> {
    /// Create a new LWW-Register with initial value
    pub fn new(value: T, timestamp: u64) -> Self {
        Self { value, timestamp }
    }

    /// Set a new value with timestamp
    pub fn set(&mut self, value: T, timestamp: u64) {
        if timestamp > self.timestamp {
            self.value = value;
            self.timestamp = timestamp;
        }
    }

    /// Get the current value
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Merge with another LWW-Register
    ///
    /// The register with the higher timestamp wins.
    /// If timestamps are equal, we need a tie-breaker (e.g., node ID).
    pub fn merge(&mut self, other: &LWWRegister<T>) {
        if other.timestamp > self.timestamp {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
        }
    }
}

/// Last-Writer-Wins Map CRDT
///
/// A map where each key has an LWW-Register value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWMap<K: Eq + std::hash::Hash + Clone, V: Clone> {
    entries: HashMap<K, LWWRegister<V>>,
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> LWWMap<K, V> {
    /// Create a new LWW-Map
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Set a value with timestamp
    pub fn set(&mut self, key: K, value: V, timestamp: u64) {
        self.entries
            .entry(key)
            .and_modify(|reg| reg.set(value.clone(), timestamp))
            .or_insert_with(|| LWWRegister::new(value, timestamp));
    }

    /// Get a value
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|reg| reg.get())
    }

    /// Get all entries
    pub fn entries(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, reg)| (k, reg.get()))
    }

    /// Merge with another LWW-Map
    pub fn merge(&mut self, other: &LWWMap<K, V>) {
        for (key, other_reg) in &other.entries {
            self.entries
                .entry(key.clone())
                .and_modify(|reg| reg.merge(other_reg))
                .or_insert_with(|| other_reg.clone());
        }
    }
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> Default for LWWMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Federated query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedResponse<T: Clone> {
    pub results: Vec<T>,
    pub rankings: Vec<(String, f32, u64)>, // (id, score, timestamp)
}

/// Reconcile CRDT data from multiple federated responses
///
/// # Implementation from PSEUDOCODE.md
///
/// ```pseudocode
/// FUNCTION ReconcileCRDT(responses, local_state):
///     merged_results = GSet()
///     FOR response IN responses:
///         FOR result IN response.results:
///             merged_results.add(result)
///
///     ranking_map = LWWMap()
///     FOR response IN responses:
///         FOR (result_id, score, timestamp) IN response.rankings:
///             ranking_map.set(result_id, score, timestamp)
///
///     final_results = []
///     FOR result IN merged_results:
///         score = ranking_map.get(result.id)
///         final_results.append((result, score))
///
///     final_results.sort(by=score, descending=True)
///     RETURN final_results
/// ```
pub fn reconcile_crdt<T>(responses: Vec<FederatedResponse<T>>) -> Result<Vec<(T, f32)>>
where
    T: Clone + Eq + std::hash::Hash + std::fmt::Display,
{
    // Step 1: Merge all results using G-Set
    let mut merged_results = GSet::new();
    for response in &responses {
        for result in &response.results {
            merged_results.add(result.clone());
        }
    }

    // Step 2: Merge rankings using LWW-Map
    let mut ranking_map = LWWMap::new();
    for response in &responses {
        for (result_id, score, timestamp) in &response.rankings {
            ranking_map.set(result_id.clone(), *score, *timestamp);
        }
    }

    // Step 3: Combine results with their scores (look up by Display representation)
    let mut final_results: Vec<(T, f32)> = merged_results
        .elements()
        .map(|result| {
            let key = format!("{}", result);
            let score = ranking_map.get(&key).copied().unwrap_or(0.5);
            (result.clone(), score)
        })
        .collect();

    // Step 4: Sort by score descending
    final_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    Ok(final_results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gset() {
        let mut set1 = GSet::new();
        set1.add(1);
        set1.add(2);

        let mut set2 = GSet::new();
        set2.add(2);
        set2.add(3);

        set1.merge(&set2);

        assert_eq!(set1.len(), 3);
        assert!(set1.contains(&1));
        assert!(set1.contains(&2));
        assert!(set1.contains(&3));
    }

    #[test]
    fn test_gset_idempotent() {
        let mut set1 = GSet::new();
        set1.add(1);
        set1.add(2);

        let set2 = set1.clone();
        set1.merge(&set2);

        assert_eq!(set1.len(), 2);
    }

    #[test]
    fn test_lww_register() {
        let mut reg1 = LWWRegister::new(100, 1);
        let reg2 = LWWRegister::new(200, 2);

        reg1.merge(&reg2);
        assert_eq!(*reg1.get(), 200);

        // Older timestamp should not override
        let reg3 = LWWRegister::new(300, 1);
        reg1.merge(&reg3);
        assert_eq!(*reg1.get(), 200);
    }

    #[test]
    fn test_lww_map() {
        let mut map1 = LWWMap::new();
        map1.set("key1", 100, 1);
        map1.set("key2", 200, 1);

        let mut map2 = LWWMap::new();
        map2.set("key2", 250, 2); // Newer timestamp
        map2.set("key3", 300, 1);

        map1.merge(&map2);

        assert_eq!(*map1.get(&"key1").unwrap(), 100);
        assert_eq!(*map1.get(&"key2").unwrap(), 250); // Updated
        assert_eq!(*map1.get(&"key3").unwrap(), 300);
    }

    #[test]
    fn test_reconcile_crdt() {
        let response1 = FederatedResponse {
            results: vec![1, 2, 3],
            rankings: vec![("1".to_string(), 0.9, 100), ("2".to_string(), 0.8, 100)],
        };

        let response2 = FederatedResponse {
            results: vec![2, 3, 4],
            rankings: vec![
                ("2".to_string(), 0.85, 101), // Newer
                ("3".to_string(), 0.7, 100),
            ],
        };

        let reconciled = reconcile_crdt(vec![response1, response2]).unwrap();

        // Should have all unique results
        assert_eq!(reconciled.len(), 4);
    }
}
