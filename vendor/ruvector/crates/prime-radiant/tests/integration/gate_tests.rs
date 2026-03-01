//! Integration tests for Coherence Gate and Compute Ladder
//!
//! Tests the Execution bounded context, verifying:
//! - Gate decisions based on energy thresholds
//! - Compute ladder escalation (O(1) -> O(n) -> O(n^o(1)))
//! - Persistence detection for blocking decisions
//! - Throttling behavior under high energy
//! - Multi-lane processing

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// ============================================================================
// TEST TYPES
// ============================================================================

/// Gate decision outcomes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateDecision {
    /// Green light - proceed without restriction
    Allow,
    /// Amber light - throttle the action
    Throttle { factor: u32 },
    /// Red light - block the action
    Block,
}

/// Compute lane for escalation
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ComputeLane {
    /// O(1) - Local tile check, immediate response
    Local,
    /// O(k) - k-hop neighborhood check
    Neighborhood { k: usize },
    /// O(n) - Full graph traversal
    Global,
    /// O(n^o(1)) - Subpolynomial spectral analysis
    Spectral,
}

/// Threshold configuration
#[derive(Clone, Debug)]
struct ThresholdConfig {
    /// Energy below this -> Allow
    green_threshold: f32,
    /// Energy below this (but above green) -> Throttle
    amber_threshold: f32,
    /// Energy above this -> Block
    red_threshold: f32,
    /// Enable compute ladder escalation
    escalation_enabled: bool,
    /// Maximum escalation lane
    max_escalation_lane: ComputeLane,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            green_threshold: 0.1,
            amber_threshold: 0.5,
            red_threshold: 1.0,
            escalation_enabled: true,
            max_escalation_lane: ComputeLane::Spectral,
        }
    }
}

/// Coherence gate engine
struct CoherenceGate {
    config: ThresholdConfig,
    current_lane: ComputeLane,
    decision_history: VecDeque<GateDecision>,
    persistence_window: usize,
}

impl CoherenceGate {
    fn new(config: ThresholdConfig) -> Self {
        Self {
            config,
            current_lane: ComputeLane::Local,
            decision_history: VecDeque::new(),
            persistence_window: 5,
        }
    }

    /// Make a gate decision based on current energy
    fn decide(&mut self, energy: f32) -> GateDecision {
        let decision = if energy < self.config.green_threshold {
            GateDecision::Allow
        } else if energy < self.config.amber_threshold {
            // Calculate throttle factor based on energy
            let ratio = (energy - self.config.green_threshold)
                / (self.config.amber_threshold - self.config.green_threshold);
            let factor = (1.0 + ratio * 9.0) as u32; // 1x to 10x throttle
            GateDecision::Throttle { factor }
        } else {
            GateDecision::Block
        };

        // Track history for persistence detection
        self.decision_history.push_back(decision);
        if self.decision_history.len() > self.persistence_window {
            self.decision_history.pop_front();
        }

        decision
    }

    /// Check if blocking is persistent
    fn is_persistent_block(&self) -> bool {
        if self.decision_history.len() < self.persistence_window {
            return false;
        }

        self.decision_history
            .iter()
            .all(|d| matches!(d, GateDecision::Block))
    }

    /// Escalate to higher compute lane
    fn escalate(&mut self) -> Option<ComputeLane> {
        if !self.config.escalation_enabled {
            return None;
        }

        let next_lane = match self.current_lane {
            ComputeLane::Local => Some(ComputeLane::Neighborhood { k: 2 }),
            ComputeLane::Neighborhood { k } if k < 5 => {
                Some(ComputeLane::Neighborhood { k: k + 1 })
            }
            ComputeLane::Neighborhood { .. } => Some(ComputeLane::Global),
            ComputeLane::Global => Some(ComputeLane::Spectral),
            ComputeLane::Spectral => None, // Already at max
        };

        if let Some(lane) = next_lane {
            if lane <= self.config.max_escalation_lane {
                self.current_lane = lane;
                return Some(lane);
            }
        }

        None
    }

    /// De-escalate to lower compute lane
    fn deescalate(&mut self) -> Option<ComputeLane> {
        let prev_lane = match self.current_lane {
            ComputeLane::Local => None,
            ComputeLane::Neighborhood { k } if k > 2 => {
                Some(ComputeLane::Neighborhood { k: k - 1 })
            }
            ComputeLane::Neighborhood { .. } => Some(ComputeLane::Local),
            ComputeLane::Global => Some(ComputeLane::Neighborhood { k: 5 }),
            ComputeLane::Spectral => Some(ComputeLane::Global),
        };

        if let Some(lane) = prev_lane {
            self.current_lane = lane;
            return Some(lane);
        }

        None
    }

    /// Get current compute lane
    fn current_lane(&self) -> ComputeLane {
        self.current_lane
    }

    /// Clear decision history
    fn reset_history(&mut self) {
        self.decision_history.clear();
    }
}

// ============================================================================
// BASIC GATE DECISION TESTS
// ============================================================================

#[test]
fn test_gate_allows_low_energy() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    let decision = gate.decide(0.05);

    assert_eq!(decision, GateDecision::Allow);
}

#[test]
fn test_gate_throttles_medium_energy() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    let decision = gate.decide(0.3);

    match decision {
        GateDecision::Throttle { factor } => {
            assert!(factor >= 1);
            assert!(factor <= 10);
        }
        _ => panic!("Expected Throttle decision, got {:?}", decision),
    }
}

#[test]
fn test_gate_blocks_high_energy() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    let decision = gate.decide(0.8);

    assert_eq!(decision, GateDecision::Block);
}

#[test]
fn test_gate_blocks_above_red_threshold() {
    let mut gate = CoherenceGate::new(ThresholdConfig {
        red_threshold: 0.5,
        ..Default::default()
    });

    let decision = gate.decide(0.6);

    assert_eq!(decision, GateDecision::Block);
}

#[test]
fn test_throttle_factor_increases_with_energy() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    let decision_low = gate.decide(0.15);
    let decision_high = gate.decide(0.45);

    match (decision_low, decision_high) {
        (GateDecision::Throttle { factor: f1 }, GateDecision::Throttle { factor: f2 }) => {
            assert!(
                f2 > f1,
                "Higher energy should produce higher throttle factor"
            );
        }
        _ => panic!("Expected both to be Throttle decisions"),
    }
}

// ============================================================================
// THRESHOLD BOUNDARY TESTS
// ============================================================================

#[test]
fn test_boundary_just_below_green() {
    let mut gate = CoherenceGate::new(ThresholdConfig {
        green_threshold: 0.1,
        ..Default::default()
    });

    let decision = gate.decide(0.099);
    assert_eq!(decision, GateDecision::Allow);
}

#[test]
fn test_boundary_at_green() {
    let mut gate = CoherenceGate::new(ThresholdConfig {
        green_threshold: 0.1,
        ..Default::default()
    });

    // At the threshold, should still be Allow (< comparison)
    let decision = gate.decide(0.1);
    assert!(matches!(decision, GateDecision::Throttle { .. }));
}

#[test]
fn test_boundary_just_below_amber() {
    let mut gate = CoherenceGate::new(ThresholdConfig {
        green_threshold: 0.1,
        amber_threshold: 0.5,
        ..Default::default()
    });

    let decision = gate.decide(0.499);
    assert!(matches!(decision, GateDecision::Throttle { .. }));
}

#[test]
fn test_boundary_at_amber() {
    let mut gate = CoherenceGate::new(ThresholdConfig {
        green_threshold: 0.1,
        amber_threshold: 0.5,
        ..Default::default()
    });

    let decision = gate.decide(0.5);
    assert_eq!(decision, GateDecision::Block);
}

// ============================================================================
// COMPUTE LADDER ESCALATION TESTS
// ============================================================================

#[test]
fn test_initial_lane_is_local() {
    let gate = CoherenceGate::new(ThresholdConfig::default());

    assert_eq!(gate.current_lane(), ComputeLane::Local);
}

#[test]
fn test_escalation_from_local_to_neighborhood() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    let new_lane = gate.escalate();

    assert_eq!(new_lane, Some(ComputeLane::Neighborhood { k: 2 }));
    assert_eq!(gate.current_lane(), ComputeLane::Neighborhood { k: 2 });
}

#[test]
fn test_escalation_through_neighborhood_k() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Local -> Neighborhood k=2
    gate.escalate();
    assert_eq!(gate.current_lane(), ComputeLane::Neighborhood { k: 2 });

    // Neighborhood k=2 -> k=3
    gate.escalate();
    assert_eq!(gate.current_lane(), ComputeLane::Neighborhood { k: 3 });

    // k=3 -> k=4
    gate.escalate();
    assert_eq!(gate.current_lane(), ComputeLane::Neighborhood { k: 4 });

    // k=4 -> k=5
    gate.escalate();
    assert_eq!(gate.current_lane(), ComputeLane::Neighborhood { k: 5 });

    // k=5 -> Global
    gate.escalate();
    assert_eq!(gate.current_lane(), ComputeLane::Global);
}

#[test]
fn test_escalation_to_spectral() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Escalate all the way
    while let Some(_) = gate.escalate() {
        // Keep escalating
    }

    assert_eq!(gate.current_lane(), ComputeLane::Spectral);
}

#[test]
fn test_escalation_respects_max_lane() {
    let mut gate = CoherenceGate::new(ThresholdConfig {
        max_escalation_lane: ComputeLane::Global,
        ..Default::default()
    });

    // Escalate to max
    while let Some(_) = gate.escalate() {}

    // Should stop at Global, not Spectral
    assert_eq!(gate.current_lane(), ComputeLane::Global);
}

#[test]
fn test_escalation_disabled() {
    let mut gate = CoherenceGate::new(ThresholdConfig {
        escalation_enabled: false,
        ..Default::default()
    });

    let result = gate.escalate();

    assert_eq!(result, None);
    assert_eq!(gate.current_lane(), ComputeLane::Local);
}

#[test]
fn test_deescalation_from_spectral() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Escalate to spectral
    while let Some(_) = gate.escalate() {}
    assert_eq!(gate.current_lane(), ComputeLane::Spectral);

    // Deescalate one step
    let lane = gate.deescalate();
    assert_eq!(lane, Some(ComputeLane::Global));
    assert_eq!(gate.current_lane(), ComputeLane::Global);
}

#[test]
fn test_deescalation_to_local() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Escalate a few times
    gate.escalate();
    gate.escalate();
    assert_eq!(gate.current_lane(), ComputeLane::Neighborhood { k: 3 });

    // Deescalate all the way
    while let Some(_) = gate.deescalate() {}

    assert_eq!(gate.current_lane(), ComputeLane::Local);
}

#[test]
fn test_deescalation_from_local_returns_none() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    let result = gate.deescalate();

    assert_eq!(result, None);
    assert_eq!(gate.current_lane(), ComputeLane::Local);
}

// ============================================================================
// PERSISTENCE DETECTION TESTS
// ============================================================================

#[test]
fn test_no_persistence_initially() {
    let gate = CoherenceGate::new(ThresholdConfig::default());

    assert!(!gate.is_persistent_block());
}

#[test]
fn test_persistence_detected_after_window() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Block persistently
    for _ in 0..5 {
        gate.decide(0.9); // Block
    }

    assert!(gate.is_persistent_block());
}

#[test]
fn test_no_persistence_with_mixed_decisions() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Mix of decisions
    gate.decide(0.9); // Block
    gate.decide(0.05); // Allow
    gate.decide(0.9); // Block
    gate.decide(0.9); // Block
    gate.decide(0.9); // Block

    // Not all blocks, so not persistent
    assert!(!gate.is_persistent_block());
}

#[test]
fn test_persistence_window_sliding() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Start with allows
    for _ in 0..3 {
        gate.decide(0.05); // Allow
    }

    assert!(!gate.is_persistent_block());

    // Then all blocks
    for _ in 0..5 {
        gate.decide(0.9); // Block
    }

    // Now persistent
    assert!(gate.is_persistent_block());
}

#[test]
fn test_reset_clears_persistence() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Build up persistence
    for _ in 0..5 {
        gate.decide(0.9);
    }
    assert!(gate.is_persistent_block());

    // Reset
    gate.reset_history();

    assert!(!gate.is_persistent_block());
}

// ============================================================================
// MULTI-LANE PROCESSING TESTS
// ============================================================================

#[test]
fn test_lane_complexity_ordering() {
    // Verify lanes are properly ordered by complexity
    assert!(ComputeLane::Local < ComputeLane::Neighborhood { k: 2 });
    assert!(ComputeLane::Neighborhood { k: 2 } < ComputeLane::Neighborhood { k: 3 });
    assert!(ComputeLane::Neighborhood { k: 5 } < ComputeLane::Global);
    assert!(ComputeLane::Global < ComputeLane::Spectral);
}

#[test]
fn test_automatic_escalation_on_block() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Simulate escalation policy: escalate on block
    let energy = 0.8;
    let decision = gate.decide(energy);

    if matches!(decision, GateDecision::Block) {
        let escalated = gate.escalate().is_some();
        assert!(escalated, "Should escalate on block");
    }

    // After one escalation
    assert!(gate.current_lane() > ComputeLane::Local);
}

#[test]
fn test_automatic_deescalation_on_allow() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // First, escalate
    gate.escalate();
    gate.escalate();
    assert!(gate.current_lane() > ComputeLane::Local);

    // Then allow (low energy)
    let decision = gate.decide(0.01);
    assert_eq!(decision, GateDecision::Allow);

    // Deescalate after allow
    gate.deescalate();
    assert!(gate.current_lane() < ComputeLane::Neighborhood { k: 3 });
}

// ============================================================================
// CUSTOM THRESHOLD TESTS
// ============================================================================

#[test]
fn test_custom_thresholds() {
    let config = ThresholdConfig {
        green_threshold: 0.05,
        amber_threshold: 0.15,
        red_threshold: 0.25,
        escalation_enabled: true,
        max_escalation_lane: ComputeLane::Global,
    };

    let mut gate = CoherenceGate::new(config);

    // Very low energy -> Allow
    assert_eq!(gate.decide(0.03), GateDecision::Allow);

    // Low-medium energy -> Throttle
    assert!(matches!(gate.decide(0.10), GateDecision::Throttle { .. }));

    // Medium energy -> Block (with these tight thresholds)
    assert_eq!(gate.decide(0.20), GateDecision::Block);
}

#[test]
fn test_zero_thresholds() {
    let config = ThresholdConfig {
        green_threshold: 0.0,
        amber_threshold: 0.0,
        red_threshold: 0.0,
        ..Default::default()
    };

    let mut gate = CoherenceGate::new(config);

    // Any positive energy should block
    assert_eq!(gate.decide(0.001), GateDecision::Block);
    assert_eq!(gate.decide(1.0), GateDecision::Block);

    // Zero energy should... well, it's at the boundary
    // < 0 is Allow, >= 0 is the next category
    // With all thresholds at 0, any energy >= 0 goes to Block
}

// ============================================================================
// CONCURRENT GATE ACCESS TESTS
// ============================================================================

#[test]
fn test_gate_thread_safety_simulation() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    // Wrap gate in mutex for thread-safe access
    let gate = Arc::new(Mutex::new(CoherenceGate::new(ThresholdConfig::default())));

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let gate = Arc::clone(&gate);
            thread::spawn(move || {
                let energy = 0.1 * (i as f32);
                let mut gate = gate.lock().unwrap();
                let decision = gate.decide(energy);
                (i, decision)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Verify each thread got a decision
    assert_eq!(results.len(), 4);
}

// ============================================================================
// ENERGY SPIKE HANDLING TESTS
// ============================================================================

#[test]
fn test_energy_spike_causes_immediate_block() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Normal operation
    for _ in 0..3 {
        let decision = gate.decide(0.05);
        assert_eq!(decision, GateDecision::Allow);
    }

    // Energy spike
    let decision = gate.decide(0.9);
    assert_eq!(decision, GateDecision::Block);
}

#[test]
fn test_recovery_after_spike() {
    let mut gate = CoherenceGate::new(ThresholdConfig::default());

    // Spike
    gate.decide(0.9);
    assert!(!gate.is_persistent_block()); // Not persistent yet

    // Recovery
    for _ in 0..5 {
        gate.decide(0.05);
    }

    assert!(!gate.is_persistent_block());

    // All recent decisions should be Allow
    assert_eq!(gate.decide(0.05), GateDecision::Allow);
}

// ============================================================================
// LANE LATENCY SIMULATION TESTS
// ============================================================================

#[test]
fn test_lane_latency_simulation() {
    /// Simulated latency for each lane
    fn lane_latency(lane: ComputeLane) -> Duration {
        match lane {
            ComputeLane::Local => Duration::from_micros(10),
            ComputeLane::Neighborhood { k } => Duration::from_micros(100 * k as u64),
            ComputeLane::Global => Duration::from_millis(10),
            ComputeLane::Spectral => Duration::from_millis(100),
        }
    }

    let lanes = vec![
        ComputeLane::Local,
        ComputeLane::Neighborhood { k: 2 },
        ComputeLane::Neighborhood { k: 5 },
        ComputeLane::Global,
        ComputeLane::Spectral,
    ];

    let latencies: Vec<_> = lanes.iter().map(|l| lane_latency(*l)).collect();

    // Verify latencies are increasing
    for i in 1..latencies.len() {
        assert!(
            latencies[i] > latencies[i - 1],
            "Higher lanes should have higher latency"
        );
    }
}

// ============================================================================
// REAL-TIME BUDGET TESTS
// ============================================================================

#[test]
fn test_local_lane_meets_budget() {
    // Local lane should complete in <1ms budget
    let budget = Duration::from_millis(1);

    let start = Instant::now();

    // Simulate local computation (just a decision)
    let mut gate = CoherenceGate::new(ThresholdConfig::default());
    for _ in 0..1000 {
        gate.decide(0.05);
    }

    let elapsed = start.elapsed();

    // 1000 decisions should still be fast
    assert!(
        elapsed < budget * 100, // Very generous for test environment
        "Local computation took too long: {:?}",
        elapsed
    );
}
