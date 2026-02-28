//! Replay Determinism Tests
//!
//! Verifies that replaying the same sequence of events produces identical state.
//! This is critical for:
//! - Reproducible debugging
//! - Witness chain validation
//! - Distributed consensus
//! - Audit trail verification

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// ============================================================================
// EVENT TYPES
// ============================================================================

/// Domain events that can modify coherence state
#[derive(Clone, Debug, PartialEq)]
enum DomainEvent {
    /// Add a node with initial state
    NodeAdded {
        node_id: u64,
        state: Vec<f32>,
        timestamp: u64,
    },
    /// Update a node's state
    NodeUpdated {
        node_id: u64,
        old_state: Vec<f32>,
        new_state: Vec<f32>,
        timestamp: u64,
    },
    /// Remove a node
    NodeRemoved { node_id: u64, timestamp: u64 },
    /// Add an edge between nodes
    EdgeAdded {
        source: u64,
        target: u64,
        weight: f32,
        timestamp: u64,
    },
    /// Update edge weight
    EdgeWeightUpdated {
        source: u64,
        target: u64,
        old_weight: f32,
        new_weight: f32,
        timestamp: u64,
    },
    /// Remove an edge
    EdgeRemoved {
        source: u64,
        target: u64,
        timestamp: u64,
    },
    /// Policy threshold change
    ThresholdChanged {
        scope: String,
        old_threshold: f32,
        new_threshold: f32,
        timestamp: u64,
    },
}

impl DomainEvent {
    fn timestamp(&self) -> u64 {
        match self {
            DomainEvent::NodeAdded { timestamp, .. } => *timestamp,
            DomainEvent::NodeUpdated { timestamp, .. } => *timestamp,
            DomainEvent::NodeRemoved { timestamp, .. } => *timestamp,
            DomainEvent::EdgeAdded { timestamp, .. } => *timestamp,
            DomainEvent::EdgeWeightUpdated { timestamp, .. } => *timestamp,
            DomainEvent::EdgeRemoved { timestamp, .. } => *timestamp,
            DomainEvent::ThresholdChanged { timestamp, .. } => *timestamp,
        }
    }
}

// ============================================================================
// COHERENCE STATE
// ============================================================================

/// Coherence engine state (simplified for testing)
#[derive(Clone, Debug)]
struct CoherenceState {
    nodes: HashMap<u64, Vec<f32>>,
    edges: HashMap<(u64, u64), f32>,
    thresholds: HashMap<String, f32>,
    energy_cache: Option<f32>,
    event_count: u64,
}

impl CoherenceState {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            thresholds: HashMap::new(),
            energy_cache: None,
            event_count: 0,
        }
    }

    fn apply(&mut self, event: &DomainEvent) {
        self.event_count += 1;
        self.energy_cache = None; // Invalidate cache

        match event {
            DomainEvent::NodeAdded { node_id, state, .. } => {
                self.nodes.insert(*node_id, state.clone());
            }
            DomainEvent::NodeUpdated {
                node_id, new_state, ..
            } => {
                self.nodes.insert(*node_id, new_state.clone());
            }
            DomainEvent::NodeRemoved { node_id, .. } => {
                self.nodes.remove(node_id);
                // Remove incident edges
                self.edges
                    .retain(|(s, t), _| *s != *node_id && *t != *node_id);
            }
            DomainEvent::EdgeAdded {
                source,
                target,
                weight,
                ..
            } => {
                self.edges.insert((*source, *target), *weight);
            }
            DomainEvent::EdgeWeightUpdated {
                source,
                target,
                new_weight,
                ..
            } => {
                self.edges.insert((*source, *target), *new_weight);
            }
            DomainEvent::EdgeRemoved { source, target, .. } => {
                self.edges.remove(&(*source, *target));
            }
            DomainEvent::ThresholdChanged {
                scope,
                new_threshold,
                ..
            } => {
                self.thresholds.insert(scope.clone(), *new_threshold);
            }
        }
    }

    fn compute_energy(&mut self) -> f32 {
        if let Some(cached) = self.energy_cache {
            return cached;
        }

        let mut total = 0.0;
        for ((src, tgt), weight) in &self.edges {
            if let (Some(src_state), Some(tgt_state)) = (self.nodes.get(src), self.nodes.get(tgt)) {
                let dim = src_state.len().min(tgt_state.len());
                let residual_norm_sq: f32 = src_state
                    .iter()
                    .take(dim)
                    .zip(tgt_state.iter().take(dim))
                    .map(|(a, b)| (a - b).powi(2))
                    .sum();
                total += weight * residual_norm_sq;
            }
        }

        self.energy_cache = Some(total);
        total
    }

    /// Compute a deterministic fingerprint of the state
    fn fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();

        // Hash nodes in sorted order
        let mut node_keys: Vec<_> = self.nodes.keys().collect();
        node_keys.sort();
        for key in node_keys {
            key.hash(&mut hasher);
            let state = self.nodes.get(key).unwrap();
            for val in state {
                val.to_bits().hash(&mut hasher);
            }
        }

        // Hash edges in sorted order
        let mut edge_keys: Vec<_> = self.edges.keys().collect();
        edge_keys.sort();
        for key in edge_keys {
            key.hash(&mut hasher);
            let weight = self.edges.get(key).unwrap();
            weight.to_bits().hash(&mut hasher);
        }

        // Hash thresholds
        let mut threshold_keys: Vec<_> = self.thresholds.keys().collect();
        threshold_keys.sort();
        for key in threshold_keys {
            key.hash(&mut hasher);
            let val = self.thresholds.get(key).unwrap();
            val.to_bits().hash(&mut hasher);
        }

        hasher.finish()
    }
}

// ============================================================================
// EVENT LOG
// ============================================================================

/// Event log for replay
#[derive(Clone, Debug)]
struct EventLog {
    events: Vec<DomainEvent>,
}

impl EventLog {
    fn new() -> Self {
        Self { events: Vec::new() }
    }

    fn append(&mut self, event: DomainEvent) {
        self.events.push(event);
    }

    fn replay(&self) -> CoherenceState {
        let mut state = CoherenceState::new();
        for event in &self.events {
            state.apply(event);
        }
        state
    }

    fn replay_until(&self, timestamp: u64) -> CoherenceState {
        let mut state = CoherenceState::new();
        for event in &self.events {
            if event.timestamp() <= timestamp {
                state.apply(event);
            }
        }
        state
    }

    fn len(&self) -> usize {
        self.events.len()
    }
}

// ============================================================================
// TESTS: BASIC REPLAY DETERMINISM
// ============================================================================

#[test]
fn test_empty_replay() {
    let log = EventLog::new();

    let state1 = log.replay();
    let state2 = log.replay();

    assert_eq!(state1.fingerprint(), state2.fingerprint());
    assert_eq!(state1.event_count, 0);
}

#[test]
fn test_single_event_replay() {
    let mut log = EventLog::new();
    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0, 0.5, 0.3],
        timestamp: 1000,
    });

    let state1 = log.replay();
    let state2 = log.replay();

    assert_eq!(state1.fingerprint(), state2.fingerprint());
    assert_eq!(state1.nodes.len(), 1);
    assert_eq!(state2.nodes.len(), 1);
}

#[test]
fn test_multiple_events_replay() {
    let mut log = EventLog::new();

    // Create a small graph
    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0, 0.0],
        timestamp: 1000,
    });
    log.append(DomainEvent::NodeAdded {
        node_id: 2,
        state: vec![0.5, 0.5],
        timestamp: 1001,
    });
    log.append(DomainEvent::NodeAdded {
        node_id: 3,
        state: vec![0.0, 1.0],
        timestamp: 1002,
    });
    log.append(DomainEvent::EdgeAdded {
        source: 1,
        target: 2,
        weight: 1.0,
        timestamp: 1003,
    });
    log.append(DomainEvent::EdgeAdded {
        source: 2,
        target: 3,
        weight: 1.0,
        timestamp: 1004,
    });

    let state1 = log.replay();
    let state2 = log.replay();
    let state3 = log.replay();

    assert_eq!(state1.fingerprint(), state2.fingerprint());
    assert_eq!(state2.fingerprint(), state3.fingerprint());

    assert_eq!(state1.nodes.len(), 3);
    assert_eq!(state1.edges.len(), 2);
}

// ============================================================================
// TESTS: ENERGY DETERMINISM
// ============================================================================

#[test]
fn test_energy_determinism() {
    let mut log = EventLog::new();

    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0, 0.5, 0.3],
        timestamp: 1000,
    });
    log.append(DomainEvent::NodeAdded {
        node_id: 2,
        state: vec![0.8, 0.6, 0.4],
        timestamp: 1001,
    });
    log.append(DomainEvent::EdgeAdded {
        source: 1,
        target: 2,
        weight: 1.0,
        timestamp: 1002,
    });

    let mut state1 = log.replay();
    let mut state2 = log.replay();

    let energy1 = state1.compute_energy();
    let energy2 = state2.compute_energy();

    assert!(
        (energy1 - energy2).abs() < 1e-10,
        "Energy should be deterministic: {} vs {}",
        energy1,
        energy2
    );
}

#[test]
fn test_energy_determinism_after_updates() {
    let mut log = EventLog::new();

    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0, 0.5],
        timestamp: 1000,
    });
    log.append(DomainEvent::NodeAdded {
        node_id: 2,
        state: vec![0.5, 0.5],
        timestamp: 1001,
    });
    log.append(DomainEvent::EdgeAdded {
        source: 1,
        target: 2,
        weight: 1.0,
        timestamp: 1002,
    });
    log.append(DomainEvent::NodeUpdated {
        node_id: 1,
        old_state: vec![1.0, 0.5],
        new_state: vec![0.7, 0.6],
        timestamp: 1003,
    });
    log.append(DomainEvent::EdgeWeightUpdated {
        source: 1,
        target: 2,
        old_weight: 1.0,
        new_weight: 2.0,
        timestamp: 1004,
    });

    let mut state1 = log.replay();
    let mut state2 = log.replay();

    let energy1 = state1.compute_energy();
    let energy2 = state2.compute_energy();

    assert!(
        (energy1 - energy2).abs() < 1e-10,
        "Energy should be deterministic after updates"
    );
}

// ============================================================================
// TESTS: PARTIAL REPLAY
// ============================================================================

#[test]
fn test_partial_replay_consistent() {
    let mut log = EventLog::new();

    for i in 1..=10 {
        log.append(DomainEvent::NodeAdded {
            node_id: i,
            state: vec![i as f32 / 10.0],
            timestamp: 1000 + i,
        });
    }

    // Replay until different points
    let state_5 = log.replay_until(1005);
    let state_10 = log.replay_until(1010);

    assert_eq!(state_5.nodes.len(), 5);
    assert_eq!(state_10.nodes.len(), 10);

    // Replaying to the same point should give the same state
    let state_5_again = log.replay_until(1005);
    assert_eq!(state_5.fingerprint(), state_5_again.fingerprint());
}

#[test]
fn test_partial_replay_monotonic() {
    let mut log = EventLog::new();

    for i in 1..=5 {
        log.append(DomainEvent::NodeAdded {
            node_id: i,
            state: vec![i as f32],
            timestamp: 1000 + i,
        });
    }

    // State should grow monotonically with timestamp
    let mut prev_nodes = 0;
    for t in 1001..=1005 {
        let state = log.replay_until(t);
        assert!(state.nodes.len() > prev_nodes || prev_nodes == 0);
        prev_nodes = state.nodes.len();
    }
}

// ============================================================================
// TESTS: EVENT ORDER INDEPENDENCE
// ============================================================================

#[test]
fn test_independent_events_commute() {
    // Events on different parts of the graph should commute
    let events_a = vec![
        DomainEvent::NodeAdded {
            node_id: 1,
            state: vec![1.0],
            timestamp: 1000,
        },
        DomainEvent::NodeAdded {
            node_id: 2,
            state: vec![2.0],
            timestamp: 1001,
        },
    ];

    let events_b = vec![
        DomainEvent::NodeAdded {
            node_id: 2,
            state: vec![2.0],
            timestamp: 1001,
        },
        DomainEvent::NodeAdded {
            node_id: 1,
            state: vec![1.0],
            timestamp: 1000,
        },
    ];

    let mut log_a = EventLog::new();
    for e in events_a {
        log_a.append(e);
    }

    let mut log_b = EventLog::new();
    for e in events_b {
        log_b.append(e);
    }

    let state_a = log_a.replay();
    let state_b = log_b.replay();

    // Independent node additions should give same final state
    assert_eq!(state_a.nodes.len(), state_b.nodes.len());
    assert_eq!(state_a.fingerprint(), state_b.fingerprint());
}

#[test]
fn test_dependent_events_order_matters() {
    // Update after add vs. add directly with new value
    let mut log1 = EventLog::new();
    log1.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0],
        timestamp: 1000,
    });
    log1.append(DomainEvent::NodeUpdated {
        node_id: 1,
        old_state: vec![1.0],
        new_state: vec![2.0],
        timestamp: 1001,
    });

    let mut log2 = EventLog::new();
    log2.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![2.0],
        timestamp: 1000,
    });

    let state1 = log1.replay();
    let state2 = log2.replay();

    // Both should result in node 1 having state [2.0]
    assert_eq!(state1.nodes.get(&1), state2.nodes.get(&1));
}

// ============================================================================
// TESTS: LARGE SCALE REPLAY
// ============================================================================

#[test]
fn test_large_event_log_replay() {
    let mut log = EventLog::new();

    // Create a moderately large graph
    let num_nodes = 100;
    let num_edges = 200;

    for i in 0..num_nodes {
        log.append(DomainEvent::NodeAdded {
            node_id: i,
            state: vec![i as f32 / num_nodes as f32; 4],
            timestamp: 1000 + i,
        });
    }

    for i in 0..num_edges {
        log.append(DomainEvent::EdgeAdded {
            source: i % num_nodes,
            target: (i + 1) % num_nodes,
            weight: 1.0,
            timestamp: 1000 + num_nodes + i,
        });
    }

    // Replay multiple times
    let states: Vec<_> = (0..5).map(|_| log.replay()).collect();

    // All replays should produce the same fingerprint
    let first_fp = states[0].fingerprint();
    for state in &states {
        assert_eq!(state.fingerprint(), first_fp);
        assert_eq!(state.nodes.len(), num_nodes as usize);
    }
}

#[test]
fn test_replay_with_many_updates() {
    let mut log = EventLog::new();

    // Create nodes
    for i in 0..10 {
        log.append(DomainEvent::NodeAdded {
            node_id: i,
            state: vec![0.0; 3],
            timestamp: 1000 + i,
        });
    }

    // Many updates
    for iteration in 0..100 {
        let node_id = iteration % 10;
        let new_val = iteration as f32 / 100.0;
        log.append(DomainEvent::NodeUpdated {
            node_id,
            old_state: vec![0.0; 3], // Simplified
            new_state: vec![new_val; 3],
            timestamp: 2000 + iteration,
        });
    }

    // Replay should be deterministic
    let state1 = log.replay();
    let state2 = log.replay();

    assert_eq!(state1.fingerprint(), state2.fingerprint());
    assert_eq!(log.len(), 110); // 10 adds + 100 updates
}

// ============================================================================
// TESTS: SNAPSHOT AND RESTORE
// ============================================================================

#[test]
fn test_snapshot_consistency() {
    let mut log = EventLog::new();

    // Build up state
    for i in 0..5 {
        log.append(DomainEvent::NodeAdded {
            node_id: i,
            state: vec![i as f32],
            timestamp: 1000 + i,
        });
    }

    // Take a "snapshot" (clone the state)
    let snapshot = log.replay();
    let snapshot_fp = snapshot.fingerprint();

    // Add more events
    for i in 5..10 {
        log.append(DomainEvent::NodeAdded {
            node_id: i,
            state: vec![i as f32],
            timestamp: 2000 + i,
        });
    }

    // Replay up to snapshot point should match snapshot
    let restored = log.replay_until(1004);
    assert_eq!(restored.fingerprint(), snapshot_fp);
}

// ============================================================================
// TESTS: CONCURRENT REPLAYS
// ============================================================================

#[test]
fn test_concurrent_replays() {
    use std::sync::Arc;
    use std::thread;

    let mut log = EventLog::new();

    for i in 0..50 {
        log.append(DomainEvent::NodeAdded {
            node_id: i,
            state: vec![i as f32 / 50.0; 4],
            timestamp: 1000 + i,
        });
    }

    for i in 0..100 {
        log.append(DomainEvent::EdgeAdded {
            source: i % 50,
            target: (i + 1) % 50,
            weight: 1.0,
            timestamp: 2000 + i,
        });
    }

    let log = Arc::new(log);

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let log = Arc::clone(&log);
            thread::spawn(move || {
                let state = log.replay();
                state.fingerprint()
            })
        })
        .collect();

    let fingerprints: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All concurrent replays should produce the same fingerprint
    let first = fingerprints[0];
    for fp in &fingerprints {
        assert_eq!(
            *fp, first,
            "All replays should produce the same fingerprint"
        );
    }
}

// ============================================================================
// TESTS: IDEMPOTENCY
// ============================================================================

#[test]
fn test_double_replay_idempotent() {
    let mut log = EventLog::new();

    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0, 0.5],
        timestamp: 1000,
    });
    log.append(DomainEvent::EdgeAdded {
        source: 1,
        target: 1, // Self-loop (edge case)
        weight: 0.5,
        timestamp: 1001,
    });

    // Replay twice from the log
    let state1 = log.replay();
    let state2 = log.replay();

    // States should be identical
    assert_eq!(state1.fingerprint(), state2.fingerprint());
    assert_eq!(state1.event_count, state2.event_count);
}

// ============================================================================
// TESTS: DELETION HANDLING
// ============================================================================

#[test]
fn test_deletion_replay() {
    let mut log = EventLog::new();

    // Add nodes
    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0],
        timestamp: 1000,
    });
    log.append(DomainEvent::NodeAdded {
        node_id: 2,
        state: vec![2.0],
        timestamp: 1001,
    });
    log.append(DomainEvent::EdgeAdded {
        source: 1,
        target: 2,
        weight: 1.0,
        timestamp: 1002,
    });

    // Delete node (should cascade to edge)
    log.append(DomainEvent::NodeRemoved {
        node_id: 1,
        timestamp: 1003,
    });

    let state1 = log.replay();
    let state2 = log.replay();

    assert_eq!(state1.fingerprint(), state2.fingerprint());
    assert_eq!(state1.nodes.len(), 1); // Only node 2 remains
    assert_eq!(state1.edges.len(), 0); // Edge was removed with node 1
}

#[test]
fn test_add_delete_add_determinism() {
    let mut log = EventLog::new();

    // Add node
    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![1.0],
        timestamp: 1000,
    });

    // Delete node
    log.append(DomainEvent::NodeRemoved {
        node_id: 1,
        timestamp: 1001,
    });

    // Re-add node with different state
    log.append(DomainEvent::NodeAdded {
        node_id: 1,
        state: vec![2.0],
        timestamp: 1002,
    });

    let state1 = log.replay();
    let state2 = log.replay();

    assert_eq!(state1.fingerprint(), state2.fingerprint());
    assert_eq!(state1.nodes.get(&1), Some(&vec![2.0]));
}
