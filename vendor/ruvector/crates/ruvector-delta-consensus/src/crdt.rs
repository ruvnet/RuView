//! CRDT implementations for delta-based replication
//!
//! Conflict-free Replicated Data Types that can be used with delta propagation.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::ReplicaId;

/// Trait for delta-based CRDTs
pub trait DeltaCrdt: Clone + Send + Sync {
    /// The delta type for this CRDT
    type Delta: Clone + Send + Sync;

    /// Get the current delta (changes since last sync)
    fn delta(&self) -> Self::Delta;

    /// Apply a delta from another replica
    fn apply_delta(&mut self, delta: &Self::Delta);

    /// Merge with another CRDT state
    fn merge(&mut self, other: &Self);

    /// Clear the delta accumulator
    fn clear_delta(&mut self);
}

/// G-Counter (Grow-only counter)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCounter {
    /// Per-replica counts
    counts: HashMap<ReplicaId, u64>,
    /// Delta since last sync
    delta: HashMap<ReplicaId, u64>,
}

impl GCounter {
    /// Create a new G-Counter
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
            delta: HashMap::new(),
        }
    }

    /// Increment the counter for a replica
    pub fn increment(&mut self, replica: &ReplicaId) {
        let count = self.counts.entry(replica.clone()).or_insert(0);
        *count += 1;

        // Track delta
        let delta_count = self.delta.entry(replica.clone()).or_insert(0);
        *delta_count += 1;
    }

    /// Get the current value
    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }
}

impl Default for GCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl DeltaCrdt for GCounter {
    type Delta = HashMap<ReplicaId, u64>;

    fn delta(&self) -> Self::Delta {
        self.delta.clone()
    }

    fn apply_delta(&mut self, delta: &Self::Delta) {
        for (replica, &count) in delta {
            let current = self.counts.entry(replica.clone()).or_insert(0);
            *current = (*current).max(count);
        }
    }

    fn merge(&mut self, other: &Self) {
        for (replica, &count) in &other.counts {
            let current = self.counts.entry(replica.clone()).or_insert(0);
            *current = (*current).max(count);
        }
    }

    fn clear_delta(&mut self) {
        self.delta.clear();
    }
}

/// PN-Counter (Positive-Negative counter)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PNCounter {
    /// Positive counts
    positive: GCounter,
    /// Negative counts
    negative: GCounter,
}

impl PNCounter {
    /// Create a new PN-Counter
    pub fn new() -> Self {
        Self {
            positive: GCounter::new(),
            negative: GCounter::new(),
        }
    }

    /// Increment the counter
    pub fn increment(&mut self, replica: &ReplicaId) {
        self.positive.increment(replica);
    }

    /// Decrement the counter
    pub fn decrement(&mut self, replica: &ReplicaId) {
        self.negative.increment(replica);
    }

    /// Get the current value
    pub fn value(&self) -> i64 {
        self.positive.value() as i64 - self.negative.value() as i64
    }
}

impl Default for PNCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Delta for PN-Counter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PNCounterDelta {
    positive: HashMap<ReplicaId, u64>,
    negative: HashMap<ReplicaId, u64>,
}

impl DeltaCrdt for PNCounter {
    type Delta = PNCounterDelta;

    fn delta(&self) -> Self::Delta {
        PNCounterDelta {
            positive: self.positive.delta(),
            negative: self.negative.delta(),
        }
    }

    fn apply_delta(&mut self, delta: &Self::Delta) {
        self.positive.apply_delta(&delta.positive);
        self.negative.apply_delta(&delta.negative);
    }

    fn merge(&mut self, other: &Self) {
        self.positive.merge(&other.positive);
        self.negative.merge(&other.negative);
    }

    fn clear_delta(&mut self) {
        self.positive.clear_delta();
        self.negative.clear_delta();
    }
}

/// LWW-Register (Last-Writer-Wins Register)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T: Clone> {
    /// Current value
    value: Option<T>,
    /// Timestamp of last write
    timestamp: u64,
    /// Replica that made last write
    replica: ReplicaId,
    /// Delta (new write)
    delta: Option<(T, u64, ReplicaId)>,
}

impl<T: Clone> LWWRegister<T> {
    /// Create a new register
    pub fn new() -> Self {
        Self {
            value: None,
            timestamp: 0,
            replica: String::new(),
            delta: None,
        }
    }

    /// Set the value
    pub fn set(&mut self, value: T, timestamp: u64, replica: ReplicaId) {
        if timestamp > self.timestamp || (timestamp == self.timestamp && replica > self.replica) {
            self.delta = Some((value.clone(), timestamp, replica.clone()));
            self.value = Some(value);
            self.timestamp = timestamp;
            self.replica = replica;
        }
    }

    /// Get the current value
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl<T: Clone> Default for LWWRegister<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Delta for LWW-Register
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegisterDelta<T: Clone> {
    pub value: Option<T>,
    pub timestamp: u64,
    pub replica: ReplicaId,
}

impl<T: Clone + Send + Sync> DeltaCrdt for LWWRegister<T> {
    type Delta = LWWRegisterDelta<T>;

    fn delta(&self) -> Self::Delta {
        if let Some((value, timestamp, replica)) = &self.delta {
            LWWRegisterDelta {
                value: Some(value.clone()),
                timestamp: *timestamp,
                replica: replica.clone(),
            }
        } else {
            LWWRegisterDelta {
                value: None,
                timestamp: 0,
                replica: String::new(),
            }
        }
    }

    fn apply_delta(&mut self, delta: &Self::Delta) {
        if let Some(value) = &delta.value {
            self.set(value.clone(), delta.timestamp, delta.replica.clone());
        }
    }

    fn merge(&mut self, other: &Self) {
        if let Some(value) = &other.value {
            self.set(value.clone(), other.timestamp, other.replica.clone());
        }
    }

    fn clear_delta(&mut self) {
        self.delta = None;
    }
}

/// OR-Set (Observed-Remove Set)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORSet<T: Clone + Eq + Hash> {
    /// Elements with their unique tags
    elements: HashMap<T, HashSet<String>>,
    /// Tombstones (removed tags)
    tombstones: HashSet<String>,
    /// Delta additions
    delta_adds: Vec<(T, String)>,
    /// Delta removes
    delta_removes: Vec<String>,
}

impl<T: Clone + Eq + Hash> ORSet<T> {
    /// Create a new OR-Set
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            tombstones: HashSet::new(),
            delta_adds: Vec::new(),
            delta_removes: Vec::new(),
        }
    }

    /// Add an element
    pub fn add(&mut self, element: T, replica: &ReplicaId) {
        let tag = format!("{}:{}", replica, uuid::Uuid::new_v4());
        self.elements
            .entry(element.clone())
            .or_insert_with(HashSet::new)
            .insert(tag.clone());
        self.delta_adds.push((element, tag));
    }

    /// Remove an element (all its tags)
    pub fn remove(&mut self, element: &T) {
        if let Some(tags) = self.elements.remove(element) {
            for tag in tags {
                self.tombstones.insert(tag.clone());
                self.delta_removes.push(tag);
            }
        }
    }

    /// Check if element is in set
    pub fn contains(&self, element: &T) -> bool {
        if let Some(tags) = self.elements.get(element) {
            !tags.is_empty()
        } else {
            false
        }
    }

    /// Get all elements
    pub fn elements(&self) -> Vec<&T> {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .map(|(e, _)| e)
            .collect()
    }

    /// Get size
    pub fn len(&self) -> usize {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .count()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Clone + Eq + Hash> Default for ORSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Delta for OR-Set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORSetDelta<T: Clone> {
    pub adds: Vec<(T, String)>,
    pub removes: Vec<String>,
}

impl<T: Clone + Eq + Hash + Send + Sync> DeltaCrdt for ORSet<T> {
    type Delta = ORSetDelta<T>;

    fn delta(&self) -> Self::Delta {
        ORSetDelta {
            adds: self.delta_adds.clone(),
            removes: self.delta_removes.clone(),
        }
    }

    fn apply_delta(&mut self, delta: &Self::Delta) {
        // Apply removes first
        for tag in &delta.removes {
            self.tombstones.insert(tag.clone());
            for tags in self.elements.values_mut() {
                tags.remove(tag);
            }
        }

        // Apply adds (if not tombstoned)
        for (element, tag) in &delta.adds {
            if !self.tombstones.contains(tag) {
                self.elements
                    .entry(element.clone())
                    .or_insert_with(HashSet::new)
                    .insert(tag.clone());
            }
        }
    }

    fn merge(&mut self, other: &Self) {
        // Merge tombstones
        for tag in &other.tombstones {
            self.tombstones.insert(tag.clone());
            for tags in self.elements.values_mut() {
                tags.remove(tag);
            }
        }

        // Merge elements
        for (element, other_tags) in &other.elements {
            let tags = self
                .elements
                .entry(element.clone())
                .or_insert_with(HashSet::new);
            for tag in other_tags {
                if !self.tombstones.contains(tag) {
                    tags.insert(tag.clone());
                }
            }
        }
    }

    fn clear_delta(&mut self) {
        self.delta_adds.clear();
        self.delta_removes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcounter() {
        let mut c1 = GCounter::new();
        let mut c2 = GCounter::new();

        c1.increment(&"r1".to_string());
        c1.increment(&"r1".to_string());
        c2.increment(&"r2".to_string());

        c1.merge(&c2);

        assert_eq!(c1.value(), 3);
    }

    #[test]
    fn test_pncounter() {
        let mut c = PNCounter::new();

        c.increment(&"r1".to_string());
        c.increment(&"r1".to_string());
        c.decrement(&"r1".to_string());

        assert_eq!(c.value(), 1);
    }

    #[test]
    fn test_lww_register() {
        let mut r1 = LWWRegister::new();
        let mut r2 = LWWRegister::new();

        r1.set("value1".to_string(), 1, "r1".to_string());
        r2.set("value2".to_string(), 2, "r2".to_string());

        r1.merge(&r2);

        assert_eq!(r1.get(), Some(&"value2".to_string()));
    }

    #[test]
    fn test_orset() {
        let mut s1 = ORSet::new();
        let mut s2 = ORSet::new();

        s1.add("a", &"r1".to_string());
        s1.add("b", &"r1".to_string());
        s2.add("c", &"r2".to_string());

        s1.merge(&s2);

        assert!(s1.contains(&"a"));
        assert!(s1.contains(&"b"));
        assert!(s1.contains(&"c"));
        assert_eq!(s1.len(), 3);
    }

    #[test]
    fn test_orset_remove() {
        let mut s1 = ORSet::new();
        let mut s2 = ORSet::new();

        s1.add("a", &"r1".to_string());
        s2.merge(&s1);

        s1.remove(&"a");

        s2.merge(&s1);

        assert!(!s2.contains(&"a"));
    }
}
