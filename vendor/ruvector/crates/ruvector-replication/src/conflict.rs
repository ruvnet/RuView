//! Conflict resolution strategies for distributed replication
//!
//! Provides vector clocks for causality tracking and various
//! conflict resolution strategies including Last-Write-Wins
//! and custom merge functions.

use crate::{ReplicationError, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

/// Vector clock for tracking causality in distributed systems
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Map of replica ID to logical timestamp
    clock: HashMap<String, u64>,
}

impl VectorClock {
    /// Create a new vector clock
    pub fn new() -> Self {
        Self {
            clock: HashMap::new(),
        }
    }

    /// Increment the clock for a replica
    pub fn increment(&mut self, replica_id: &str) {
        let counter = self.clock.entry(replica_id.to_string()).or_insert(0);
        *counter += 1;
    }

    /// Get the timestamp for a replica
    pub fn get(&self, replica_id: &str) -> u64 {
        self.clock.get(replica_id).copied().unwrap_or(0)
    }

    /// Update with another vector clock (taking max of each component)
    pub fn merge(&mut self, other: &VectorClock) {
        for (replica_id, &timestamp) in &other.clock {
            let current = self.clock.entry(replica_id.clone()).or_insert(0);
            *current = (*current).max(timestamp);
        }
    }

    /// Check if this clock happens-before another clock
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut less = false;
        let mut equal = true;

        // Check all replicas in self
        for (replica_id, &self_ts) in &self.clock {
            let other_ts = other.get(replica_id);
            if self_ts > other_ts {
                return false;
            }
            if self_ts < other_ts {
                less = true;
                equal = false;
            }
        }

        // Check replicas only in other
        for (replica_id, &other_ts) in &other.clock {
            if !self.clock.contains_key(replica_id) && other_ts > 0 {
                less = true;
                equal = false;
            }
        }

        less || equal
    }

    /// Compare vector clocks for causality
    pub fn compare(&self, other: &VectorClock) -> ClockOrdering {
        if self == other {
            return ClockOrdering::Equal;
        }

        if self.happens_before(other) {
            return ClockOrdering::Before;
        }

        if other.happens_before(self) {
            return ClockOrdering::After;
        }

        ClockOrdering::Concurrent
    }

    /// Check if two clocks are concurrent (conflicting)
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        matches!(self.compare(other), ClockOrdering::Concurrent)
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for VectorClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (i, (replica, ts)) in self.clock.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", replica, ts)?;
        }
        write!(f, "}}")
    }
}

/// Ordering relationship between vector clocks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockOrdering {
    /// Clocks are equal
    Equal,
    /// First clock happens before second
    Before,
    /// First clock happens after second
    After,
    /// Clocks are concurrent (conflicting)
    Concurrent,
}

/// A versioned value with vector clock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versioned<T> {
    /// The value
    pub value: T,
    /// Vector clock for this version
    pub clock: VectorClock,
    /// Replica that created this version
    pub replica_id: String,
}

impl<T> Versioned<T> {
    /// Create a new versioned value
    pub fn new(value: T, replica_id: String) -> Self {
        let mut clock = VectorClock::new();
        clock.increment(&replica_id);
        Self {
            value,
            clock,
            replica_id,
        }
    }

    /// Update the version with a new value
    pub fn update(&mut self, value: T) {
        self.value = value;
        self.clock.increment(&self.replica_id);
    }

    /// Compare versions for causality
    pub fn compare(&self, other: &Versioned<T>) -> ClockOrdering {
        self.clock.compare(&other.clock)
    }
}

/// Trait for conflict resolution strategies
pub trait ConflictResolver<T: Clone>: Send + Sync {
    /// Resolve a conflict between two versions
    fn resolve(&self, v1: &Versioned<T>, v2: &Versioned<T>) -> Result<Versioned<T>>;

    /// Resolve multiple conflicting versions
    fn resolve_many(&self, versions: Vec<Versioned<T>>) -> Result<Versioned<T>> {
        if versions.is_empty() {
            return Err(ReplicationError::ConflictResolution(
                "No versions to resolve".to_string(),
            ));
        }

        if versions.len() == 1 {
            return Ok(versions.into_iter().next().unwrap());
        }

        let mut result = versions[0].clone();
        for version in versions.iter().skip(1) {
            result = self.resolve(&result, version)?;
        }
        Ok(result)
    }
}

/// Last-Write-Wins conflict resolution strategy
pub struct LastWriteWins;

impl<T: Clone> ConflictResolver<T> for LastWriteWins {
    fn resolve(&self, v1: &Versioned<T>, v2: &Versioned<T>) -> Result<Versioned<T>> {
        match v1.compare(v2) {
            ClockOrdering::Before | ClockOrdering::Concurrent => Ok(v2.clone()),
            ClockOrdering::After | ClockOrdering::Equal => Ok(v1.clone()),
        }
    }
}

/// Custom merge function for conflict resolution
pub struct MergeFunction<T, F>
where
    F: Fn(&T, &T) -> T + Send + Sync,
{
    merge_fn: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> MergeFunction<T, F>
where
    F: Fn(&T, &T) -> T + Send + Sync,
{
    /// Create a new merge function resolver
    pub fn new(merge_fn: F) -> Self {
        Self {
            merge_fn,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Clone + Send + Sync, F> ConflictResolver<T> for MergeFunction<T, F>
where
    F: Fn(&T, &T) -> T + Send + Sync,
{
    fn resolve(&self, v1: &Versioned<T>, v2: &Versioned<T>) -> Result<Versioned<T>> {
        match v1.compare(v2) {
            ClockOrdering::Equal | ClockOrdering::Before => Ok(v2.clone()),
            ClockOrdering::After => Ok(v1.clone()),
            ClockOrdering::Concurrent => {
                let merged_value = (self.merge_fn)(&v1.value, &v2.value);
                let mut merged_clock = v1.clock.clone();
                merged_clock.merge(&v2.clock);

                Ok(Versioned {
                    value: merged_value,
                    clock: merged_clock,
                    replica_id: v1.replica_id.clone(),
                })
            }
        }
    }
}

/// CRDT-inspired merge for numeric values (takes max)
pub struct MaxMerge;

impl ConflictResolver<i64> for MaxMerge {
    fn resolve(&self, v1: &Versioned<i64>, v2: &Versioned<i64>) -> Result<Versioned<i64>> {
        match v1.compare(v2) {
            ClockOrdering::Equal | ClockOrdering::Before => Ok(v2.clone()),
            ClockOrdering::After => Ok(v1.clone()),
            ClockOrdering::Concurrent => {
                let merged_value = v1.value.max(v2.value);
                let mut merged_clock = v1.clock.clone();
                merged_clock.merge(&v2.clock);

                Ok(Versioned {
                    value: merged_value,
                    clock: merged_clock,
                    replica_id: v1.replica_id.clone(),
                })
            }
        }
    }
}

/// CRDT-inspired merge for sets (takes union)
pub struct SetUnion;

impl<T: Clone + Eq + std::hash::Hash> ConflictResolver<Vec<T>> for SetUnion {
    fn resolve(&self, v1: &Versioned<Vec<T>>, v2: &Versioned<Vec<T>>) -> Result<Versioned<Vec<T>>> {
        match v1.compare(v2) {
            ClockOrdering::Equal | ClockOrdering::Before => Ok(v2.clone()),
            ClockOrdering::After => Ok(v1.clone()),
            ClockOrdering::Concurrent => {
                let mut merged_value = v1.value.clone();
                for item in &v2.value {
                    if !merged_value.contains(item) {
                        merged_value.push(item.clone());
                    }
                }

                let mut merged_clock = v1.clock.clone();
                merged_clock.merge(&v2.clock);

                Ok(Versioned {
                    value: merged_value,
                    clock: merged_clock,
                    replica_id: v1.replica_id.clone(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock() {
        let mut clock1 = VectorClock::new();
        clock1.increment("r1");
        clock1.increment("r1");

        let mut clock2 = VectorClock::new();
        clock2.increment("r1");

        assert_eq!(clock1.compare(&clock2), ClockOrdering::After);
        assert_eq!(clock2.compare(&clock1), ClockOrdering::Before);
    }

    #[test]
    fn test_concurrent_clocks() {
        let mut clock1 = VectorClock::new();
        clock1.increment("r1");

        let mut clock2 = VectorClock::new();
        clock2.increment("r2");

        assert_eq!(clock1.compare(&clock2), ClockOrdering::Concurrent);
        assert!(clock1.is_concurrent(&clock2));
    }

    #[test]
    fn test_clock_merge() {
        let mut clock1 = VectorClock::new();
        clock1.increment("r1");
        clock1.increment("r1");

        let mut clock2 = VectorClock::new();
        clock2.increment("r2");
        clock2.increment("r2");
        clock2.increment("r2");

        clock1.merge(&clock2);
        assert_eq!(clock1.get("r1"), 2);
        assert_eq!(clock1.get("r2"), 3);
    }

    #[test]
    fn test_versioned() {
        let mut v1 = Versioned::new(100, "r1".to_string());
        v1.update(200);

        assert_eq!(v1.value, 200);
        assert_eq!(v1.clock.get("r1"), 2);
    }

    #[test]
    fn test_last_write_wins() {
        let v1 = Versioned::new(100, "r1".to_string());
        let mut v2 = Versioned::new(200, "r1".to_string());
        v2.clock.increment("r1");

        let resolver = LastWriteWins;
        let result = resolver.resolve(&v1, &v2).unwrap();
        assert_eq!(result.value, 200);
    }

    #[test]
    fn test_merge_function() {
        let v1 = Versioned::new(100, "r1".to_string());
        let v2 = Versioned::new(200, "r2".to_string());

        let resolver = MergeFunction::new(|a, b| a + b);
        let result = resolver.resolve(&v1, &v2).unwrap();
        assert_eq!(result.value, 300);
    }

    #[test]
    fn test_max_merge() {
        let v1 = Versioned::new(100, "r1".to_string());
        let v2 = Versioned::new(200, "r2".to_string());

        let resolver = MaxMerge;
        let result = resolver.resolve(&v1, &v2).unwrap();
        assert_eq!(result.value, 200);
    }

    #[test]
    fn test_set_union() {
        let v1 = Versioned::new(vec![1, 2, 3], "r1".to_string());
        let v2 = Versioned::new(vec![3, 4, 5], "r2".to_string());

        let resolver = SetUnion;
        let result = resolver.resolve(&v1, &v2).unwrap();
        assert_eq!(result.value.len(), 5);
        assert!(result.value.contains(&1));
        assert!(result.value.contains(&4));
    }
}
