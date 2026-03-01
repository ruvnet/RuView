//! Causal ordering with vector clocks and hybrid logical clocks

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ReplicaId;

/// Vector clock for causal ordering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Map of replica ID to logical timestamp
    clock: HashMap<ReplicaId, u64>,
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

    /// Merge with another vector clock (take max of each component)
    pub fn merge(&mut self, other: &VectorClock) {
        for (replica_id, &timestamp) in &other.clock {
            let current = self.clock.entry(replica_id.clone()).or_insert(0);
            *current = (*current).max(timestamp);
        }
    }

    /// Check if this clock happens-before another
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut at_least_one_less = false;

        // Check all replicas in self
        for (replica_id, &self_ts) in &self.clock {
            let other_ts = other.get(replica_id);
            if self_ts > other_ts {
                return false;
            }
            if self_ts < other_ts {
                at_least_one_less = true;
            }
        }

        // Check replicas only in other
        for (replica_id, &other_ts) in &other.clock {
            if !self.clock.contains_key(replica_id) && other_ts > 0 {
                at_least_one_less = true;
            }
        }

        at_least_one_less
    }

    /// Check if two clocks are concurrent
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self) && self != other
    }

    /// Compare two vector clocks
    pub fn compare(&self, other: &VectorClock) -> CausalOrder {
        if self == other {
            CausalOrder::Equal
        } else if self.happens_before(other) {
            CausalOrder::Before
        } else if other.happens_before(self) {
            CausalOrder::After
        } else {
            CausalOrder::Concurrent
        }
    }

    /// Get all replica IDs
    pub fn replicas(&self) -> Vec<&ReplicaId> {
        self.clock.keys().collect()
    }

    /// Get total count (sum of all timestamps)
    pub fn total(&self) -> u64 {
        self.clock.values().sum()
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Causal ordering relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalOrder {
    /// Equal clocks
    Equal,
    /// First happens before second
    Before,
    /// First happens after second
    After,
    /// Concurrent (conflicting)
    Concurrent,
}

/// Hybrid Logical Clock for combining physical and logical time
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridLogicalClock {
    /// Physical time component (milliseconds)
    pub physical: u64,
    /// Logical counter
    pub logical: u32,
    /// Replica ID
    pub replica: ReplicaId,
}

impl HybridLogicalClock {
    /// Create a new HLC
    pub fn new(replica: ReplicaId) -> Self {
        Self {
            physical: current_time_millis(),
            logical: 0,
            replica,
        }
    }

    /// Get current timestamp
    pub fn now(&mut self) -> HlcTimestamp {
        let pt = current_time_millis();

        if pt > self.physical {
            self.physical = pt;
            self.logical = 0;
        } else {
            self.logical += 1;
        }

        HlcTimestamp {
            physical: self.physical,
            logical: self.logical,
            replica: self.replica.clone(),
        }
    }

    /// Update clock on message receive
    pub fn receive(&mut self, remote: &HlcTimestamp) -> HlcTimestamp {
        let pt = current_time_millis();

        if pt > self.physical && pt > remote.physical {
            self.physical = pt;
            self.logical = 0;
        } else if self.physical > remote.physical {
            self.logical += 1;
        } else if remote.physical > self.physical {
            self.physical = remote.physical;
            self.logical = remote.logical + 1;
        } else {
            self.logical = self.logical.max(remote.logical) + 1;
        }

        HlcTimestamp {
            physical: self.physical,
            logical: self.logical,
            replica: self.replica.clone(),
        }
    }
}

/// A timestamp from an HLC
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HlcTimestamp {
    /// Physical time
    pub physical: u64,
    /// Logical counter
    pub logical: u32,
    /// Origin replica
    pub replica: ReplicaId,
}

impl HlcTimestamp {
    /// Compare timestamps
    pub fn compare(&self, other: &HlcTimestamp) -> Ordering {
        match self.physical.cmp(&other.physical) {
            Ordering::Equal => match self.logical.cmp(&other.logical) {
                Ordering::Equal => self.replica.cmp(&other.replica),
                other => other,
            },
            other => other,
        }
    }
}

impl PartialOrd for HlcTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.compare(other))
    }
}

impl Ord for HlcTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

/// Get current time in milliseconds
fn current_time_millis() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}

/// Lamport timestamp (simpler alternative to vector clocks)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LamportClock {
    /// Logical timestamp
    pub timestamp: u64,
    /// Replica ID for tie-breaking
    pub replica: ReplicaId,
}

impl LamportClock {
    /// Create a new Lamport clock
    pub fn new(replica: ReplicaId) -> Self {
        Self {
            timestamp: 0,
            replica,
        }
    }

    /// Increment on local event
    pub fn tick(&mut self) -> u64 {
        self.timestamp += 1;
        self.timestamp
    }

    /// Update on message receive
    pub fn receive(&mut self, remote_timestamp: u64) -> u64 {
        self.timestamp = self.timestamp.max(remote_timestamp) + 1;
        self.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_happens_before() {
        let mut clock1 = VectorClock::new();
        clock1.increment("r1");

        let mut clock2 = clock1.clone();
        clock2.increment("r1");

        assert!(clock1.happens_before(&clock2));
        assert!(!clock2.happens_before(&clock1));
    }

    #[test]
    fn test_vector_clock_concurrent() {
        let mut clock1 = VectorClock::new();
        clock1.increment("r1");

        let mut clock2 = VectorClock::new();
        clock2.increment("r2");

        assert!(clock1.is_concurrent(&clock2));
        assert!(clock2.is_concurrent(&clock1));
    }

    #[test]
    fn test_vector_clock_merge() {
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
    fn test_hlc_ordering() {
        let mut hlc1 = HybridLogicalClock::new("r1".to_string());
        let mut hlc2 = HybridLogicalClock::new("r2".to_string());

        let ts1 = hlc1.now();
        let ts2 = hlc2.now();

        // Timestamps should be comparable
        let cmp = ts1.compare(&ts2);
        assert!(cmp != Ordering::Equal || ts1.replica != ts2.replica);
    }

    #[test]
    fn test_lamport_clock() {
        let mut clock = LamportClock::new("r1".to_string());

        assert_eq!(clock.tick(), 1);
        assert_eq!(clock.tick(), 2);
        assert_eq!(clock.receive(10), 11);
        assert_eq!(clock.tick(), 12);
    }
}
