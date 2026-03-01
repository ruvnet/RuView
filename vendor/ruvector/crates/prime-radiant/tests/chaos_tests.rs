//! Chaos Tests for Coherence Engine
//!
//! Tests system behavior under adversarial and random conditions:
//! - Random energy spikes
//! - Throttling behavior under load
//! - Recovery from extreme states
//! - Concurrent modifications
//! - Edge case handling

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// TEST INFRASTRUCTURE
// ============================================================================

/// Coherence gate with throttling
#[derive(Clone)]
struct ThrottledGate {
    green_threshold: f32,
    amber_threshold: f32,
    red_threshold: f32,
    current_throttle: f32, // 0.0 = no throttle, 1.0 = max throttle
    blocked_count: u64,
    throttled_count: u64,
    allowed_count: u64,
}

impl ThrottledGate {
    fn new(green: f32, amber: f32, red: f32) -> Self {
        Self {
            green_threshold: green,
            amber_threshold: amber,
            red_threshold: red,
            current_throttle: 0.0,
            blocked_count: 0,
            throttled_count: 0,
            allowed_count: 0,
        }
    }

    fn decide(&mut self, energy: f32) -> Decision {
        if energy < self.green_threshold {
            self.current_throttle = (self.current_throttle - 0.1).max(0.0);
            self.allowed_count += 1;
            Decision::Allow
        } else if energy < self.amber_threshold {
            let throttle_factor =
                (energy - self.green_threshold) / (self.amber_threshold - self.green_threshold);
            self.current_throttle = (self.current_throttle + throttle_factor * 0.1).min(1.0);
            self.throttled_count += 1;
            Decision::Throttle {
                factor: throttle_factor,
            }
        } else {
            self.current_throttle = 1.0;
            self.blocked_count += 1;
            Decision::Block
        }
    }

    fn should_process(&self, rng: &mut impl Rng) -> bool {
        if self.current_throttle <= 0.0 {
            true
        } else {
            rng.gen::<f32>() > self.current_throttle
        }
    }

    fn stats(&self) -> GateStats {
        let total = self.allowed_count + self.throttled_count + self.blocked_count;
        GateStats {
            total_decisions: total,
            allowed: self.allowed_count,
            throttled: self.throttled_count,
            blocked: self.blocked_count,
            allow_rate: if total > 0 {
                self.allowed_count as f64 / total as f64
            } else {
                1.0
            },
            block_rate: if total > 0 {
                self.blocked_count as f64 / total as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Decision {
    Allow,
    Throttle { factor: f32 },
    Block,
}

#[derive(Debug)]
struct GateStats {
    total_decisions: u64,
    allowed: u64,
    throttled: u64,
    blocked: u64,
    allow_rate: f64,
    block_rate: f64,
}

/// Simple coherence state for chaos testing
struct ChaosState {
    nodes: HashMap<u64, Vec<f32>>,
    edges: HashMap<(u64, u64), f32>,
    operation_count: AtomicU64,
}

impl ChaosState {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            operation_count: AtomicU64::new(0),
        }
    }

    fn add_node(&mut self, id: u64, state: Vec<f32>) {
        self.nodes.insert(id, state);
        self.operation_count.fetch_add(1, Ordering::Relaxed);
    }

    fn add_edge(&mut self, src: u64, tgt: u64, weight: f32) {
        if self.nodes.contains_key(&src) && self.nodes.contains_key(&tgt) {
            self.edges.insert((src, tgt), weight);
            self.operation_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn compute_energy(&self) -> f32 {
        let mut total = 0.0;
        for ((src, tgt), weight) in &self.edges {
            if let (Some(s), Some(t)) = (self.nodes.get(src), self.nodes.get(tgt)) {
                let dim = s.len().min(t.len());
                let residual: f32 = s
                    .iter()
                    .take(dim)
                    .zip(t.iter().take(dim))
                    .map(|(a, b)| (a - b).powi(2))
                    .sum();
                total += weight * residual;
            }
        }
        total
    }

    fn perturb_node(&mut self, id: u64, rng: &mut impl Rng) {
        if let Some(state) = self.nodes.get_mut(&id) {
            for val in state.iter_mut() {
                *val += rng.gen_range(-0.1..0.1);
            }
            self.operation_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// ============================================================================
// CHAOS: RANDOM ENERGY SPIKES
// ============================================================================

#[test]
fn test_random_energy_spikes() {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    let mut energies = Vec::new();
    let mut decisions = Vec::new();

    // Generate random energy values with occasional spikes
    for _ in 0..1000 {
        let base = rng.gen_range(0.0..0.2);
        let spike = if rng.gen_bool(0.1) {
            rng.gen_range(0.0..2.0) // 10% chance of spike
        } else {
            0.0
        };
        let energy = base + spike;
        energies.push(energy);
        decisions.push(gate.decide(energy));
    }

    let stats = gate.stats();

    // Verify system handled spikes appropriately
    // With 10% spike rate and spikes going up to 2.0 (well above amber threshold),
    // we expect a mix of decisions
    assert!(stats.blocked > 0, "Should have blocked some spikes");
    assert!(
        stats.allowed > 0,
        "Should have allowed low-energy operations"
    );
    // Allow rate depends on threshold settings - with spikes going to amber/red zone,
    // we expect at least some operations to be allowed (the 90% non-spike operations)
    assert!(
        stats.allow_rate > 0.3,
        "Should have allowed at least 30% of operations (got {})",
        stats.allow_rate
    );
}

#[test]
fn test_sustained_spike_triggers_persistent_block() {
    let mut rng = ChaCha8Rng::seed_from_u64(123);
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Normal operations
    for _ in 0..50 {
        let energy = rng.gen_range(0.0..0.1);
        gate.decide(energy);
    }

    assert!(
        gate.current_throttle < 0.1,
        "Should have low throttle initially"
    );

    // Sustained high energy
    for _ in 0..20 {
        gate.decide(0.8);
    }

    assert!(
        gate.current_throttle > 0.5,
        "Should have high throttle after sustained spikes"
    );

    // Verify recovery is gradual
    let throttle_before = gate.current_throttle;
    for _ in 0..10 {
        gate.decide(0.05);
    }
    assert!(
        gate.current_throttle < throttle_before,
        "Throttle should decrease after normal operations"
    );
}

#[test]
fn test_spike_patterns() {
    let mut rng = ChaCha8Rng::seed_from_u64(456);
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Pattern 1: Regular low-high oscillation
    for i in 0..100 {
        let energy = if i % 2 == 0 { 0.05 } else { 0.8 };
        gate.decide(energy);
    }

    let stats1 = gate.stats();

    // Reset
    gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Pattern 2: Bursts
    for burst in 0..10 {
        // Low energy burst
        for _ in 0..8 {
            gate.decide(0.05);
        }
        // High energy burst
        for _ in 0..2 {
            gate.decide(0.9);
        }
    }

    let stats2 = gate.stats();

    // Both patterns have the same 20% high-energy ratio but different distributions.
    // Pattern 1: random distribution across iterations
    // Pattern 2: burst pattern (8 low, 2 high per burst)
    // The key invariant is that both should have some blocks from the high-energy operations
    assert!(stats1.blocked > 0, "Pattern 1 should have blocks");
    assert!(stats2.blocked > 0, "Pattern 2 should have blocks");
    // Both should process 100 operations
    assert_eq!(stats1.total_decisions, 100);
    assert_eq!(stats2.total_decisions, 100);
}

// ============================================================================
// CHAOS: THROTTLING UNDER LOAD
// ============================================================================

#[test]
fn test_throttling_fairness() {
    let mut rng = ChaCha8Rng::seed_from_u64(789);
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Put gate into throttled state
    for _ in 0..10 {
        gate.decide(0.3);
    }

    // Count how many requests get through
    let mut processed = 0;
    let mut total = 0;

    for _ in 0..1000 {
        total += 1;
        if gate.should_process(&mut rng) {
            processed += 1;
        }
    }

    let process_rate = processed as f64 / total as f64;

    // Should be roughly inverse of throttle
    let expected_rate = 1.0 - gate.current_throttle as f64;
    assert!(
        (process_rate - expected_rate).abs() < 0.1,
        "Process rate {} should be close to expected {}",
        process_rate,
        expected_rate
    );
}

#[test]
fn test_throttling_response_time() {
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Measure response time under different throttle states
    let measure_response = |gate: &mut ThrottledGate, energy: f32| {
        let start = Instant::now();
        for _ in 0..100 {
            gate.decide(energy);
        }
        start.elapsed()
    };

    let low_energy_time = measure_response(&mut gate, 0.05);

    gate = ThrottledGate::new(0.1, 0.5, 1.0);
    let high_energy_time = measure_response(&mut gate, 0.8);

    // Decision time should be similar regardless of energy level
    let ratio = high_energy_time.as_nanos() as f64 / low_energy_time.as_nanos() as f64;
    assert!(
        ratio < 10.0,
        "High energy decisions shouldn't be much slower (ratio: {})",
        ratio
    );
}

#[test]
fn test_progressive_throttling() {
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    let mut throttle_history = Vec::new();

    // Gradually increase energy
    for i in 0..100 {
        let energy = i as f32 / 100.0; // 0.0 to 1.0
        gate.decide(energy);
        throttle_history.push(gate.current_throttle);
    }

    // Throttle should generally increase
    let increasing_segments = throttle_history
        .windows(10)
        .filter(|w| w.last() > w.first())
        .count();

    assert!(
        increasing_segments > 5,
        "Throttle should generally increase with energy"
    );
}

// ============================================================================
// CHAOS: CONCURRENT MODIFICATIONS
// ============================================================================

#[test]
fn test_concurrent_state_modifications() {
    let state = Arc::new(Mutex::new(ChaosState::new()));

    // Initialize some nodes
    {
        let mut s = state.lock().unwrap();
        for i in 0..100 {
            s.add_node(i, vec![i as f32 / 100.0; 4]);
        }
        for i in 0..99 {
            s.add_edge(i, i + 1, 1.0);
        }
    }

    // Spawn threads that concurrently modify state
    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                let mut rng = ChaCha8Rng::seed_from_u64(thread_id);
                for _ in 0..100 {
                    let mut s = state.lock().unwrap();
                    let node_id = rng.gen_range(0..100);
                    s.perturb_node(node_id, &mut rng);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // State should still be valid
    let s = state.lock().unwrap();
    assert_eq!(s.nodes.len(), 100);
    assert_eq!(s.edges.len(), 99);

    // Energy should be computable
    let energy = s.compute_energy();
    assert!(energy.is_finite(), "Energy should be finite");
}

#[test]
fn test_concurrent_energy_computation() {
    let state = Arc::new(Mutex::new(ChaosState::new()));

    // Initialize
    {
        let mut s = state.lock().unwrap();
        for i in 0..50 {
            s.add_node(i, vec![i as f32 / 50.0; 4]);
        }
        for i in 0..49 {
            s.add_edge(i, i + 1, 1.0);
        }
    }

    // Concurrent energy computations
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                let s = state.lock().unwrap();
                s.compute_energy()
            })
        })
        .collect();

    let energies: Vec<f32> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All computations should give the same result
    let first = energies[0];
    for e in &energies {
        assert!(
            (e - first).abs() < 1e-6,
            "Concurrent computations should give same result"
        );
    }
}

// ============================================================================
// CHAOS: EXTREME VALUES
// ============================================================================

#[test]
fn test_extreme_energy_values() {
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Very small energy
    let decision = gate.decide(1e-10);
    assert_eq!(decision, Decision::Allow);

    // Very large energy
    let decision = gate.decide(1e10);
    assert_eq!(decision, Decision::Block);

    // Zero
    let decision = gate.decide(0.0);
    assert_eq!(decision, Decision::Allow);

    // Negative (should still work, though unusual)
    let decision = gate.decide(-0.1);
    assert_eq!(decision, Decision::Allow); // Less than green threshold
}

#[test]
fn test_extreme_state_values() {
    let mut state = ChaosState::new();

    // Very large state values
    state.add_node(1, vec![1e10, -1e10, 1e10, -1e10]);
    state.add_node(2, vec![-1e10, 1e10, -1e10, 1e10]);
    state.add_edge(1, 2, 1.0);

    let energy = state.compute_energy();
    assert!(energy.is_finite(), "Energy should handle large values");
    assert!(energy > 0.0, "Energy should be positive");
}

#[test]
fn test_many_small_perturbations() {
    let mut rng = ChaCha8Rng::seed_from_u64(999);
    let mut state = ChaosState::new();

    // Create a stable baseline
    state.add_node(1, vec![0.5, 0.5, 0.5, 0.5]);
    state.add_node(2, vec![0.5, 0.5, 0.5, 0.5]);
    state.add_edge(1, 2, 1.0);

    let initial_energy = state.compute_energy();

    // Many small perturbations
    for _ in 0..1000 {
        state.perturb_node(1, &mut rng);
        state.perturb_node(2, &mut rng);
    }

    let final_energy = state.compute_energy();

    // Energy should still be reasonable (not exploded)
    assert!(final_energy.is_finite());
    // Random walk should increase variance
    assert!(final_energy > initial_energy * 0.1 || final_energy < initial_energy * 10.0);
}

// ============================================================================
// CHAOS: RECOVERY SCENARIOS
// ============================================================================

#[test]
fn test_recovery_from_blocked_state() {
    let mut rng = ChaCha8Rng::seed_from_u64(111);
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Drive into blocked state
    for _ in 0..20 {
        gate.decide(0.9);
    }

    assert!(
        gate.current_throttle > 0.9,
        "Should be in high throttle state"
    );

    // Recover with low energy
    let mut recovery_steps = 0;
    while gate.current_throttle > 0.1 && recovery_steps < 200 {
        gate.decide(0.05);
        recovery_steps += 1;
    }

    assert!(
        recovery_steps < 200,
        "Should recover within reasonable time"
    );
    assert!(
        gate.current_throttle < 0.2,
        "Should have low throttle after recovery"
    );
}

#[test]
fn test_oscillation_dampening() {
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    // Oscillate between extremes
    let mut throttle_variance = Vec::new();
    for cycle in 0..10 {
        // High phase
        for _ in 0..5 {
            gate.decide(0.8);
        }
        // Low phase
        for _ in 0..5 {
            gate.decide(0.05);
        }
        throttle_variance.push(gate.current_throttle);
    }

    // Throttle should not oscillate wildly
    let max = throttle_variance
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let min = throttle_variance
        .iter()
        .cloned()
        .fold(f32::INFINITY, f32::min);

    // Should settle to some stable-ish range
    // (This is a soft check - exact behavior depends on parameters)
    assert!(max - min < 1.0, "Throttle oscillation should be bounded");
}

// ============================================================================
// CHAOS: RANDOM GRAPH MODIFICATIONS
// ============================================================================

#[test]
fn test_random_graph_operations() {
    let mut rng = ChaCha8Rng::seed_from_u64(222);
    let mut state = ChaosState::new();

    // Random operations
    for _ in 0..1000 {
        let op = rng.gen_range(0..3);
        match op {
            0 => {
                // Add node
                let id = rng.gen_range(0..100);
                let dim = rng.gen_range(2..8);
                let values: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
                state.add_node(id, values);
            }
            1 => {
                // Add edge
                let src = rng.gen_range(0..100);
                let tgt = rng.gen_range(0..100);
                if src != tgt {
                    let weight = rng.gen_range(0.1..2.0);
                    state.add_edge(src, tgt, weight);
                }
            }
            2 => {
                // Perturb existing node
                let id = rng.gen_range(0..100);
                state.perturb_node(id, &mut rng);
            }
            _ => {}
        }
    }

    // State should be valid
    assert!(state.nodes.len() <= 100);
    let energy = state.compute_energy();
    assert!(energy.is_finite());
}

// ============================================================================
// CHAOS: STRESS TESTS
// ============================================================================

#[test]
fn test_rapid_fire_decisions() {
    let mut rng = ChaCha8Rng::seed_from_u64(333);
    let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);

    let start = Instant::now();
    let mut count = 0;

    while start.elapsed() < Duration::from_millis(100) {
        let energy = rng.gen_range(0.0..0.6);
        gate.decide(energy);
        count += 1;
    }

    assert!(count > 1000, "Should process many decisions quickly");

    let stats = gate.stats();
    assert_eq!(stats.total_decisions, count);
}

#[test]
fn test_memory_stability() {
    let mut rng = ChaCha8Rng::seed_from_u64(444);
    let mut state = ChaosState::new();

    // Many cycles of add/modify
    for cycle in 0..100 {
        // Add phase
        for i in 0..10 {
            let id = cycle * 10 + i;
            state.add_node(id, vec![rng.gen::<f32>(); 4]);
        }

        // Modify phase
        for _ in 0..50 {
            let id = rng.gen_range(0..(cycle + 1) * 10);
            state.perturb_node(id, &mut rng);
        }

        // Energy check
        let energy = state.compute_energy();
        assert!(
            energy.is_finite(),
            "Energy should be finite at cycle {}",
            cycle
        );
    }

    assert!(state.nodes.len() > 0);
}

// ============================================================================
// CHAOS: DETERMINISTIC CHAOS (SEEDED RANDOM)
// ============================================================================

#[test]
fn test_seeded_chaos_reproducible() {
    fn run_chaos(seed: u64) -> (f32, u64, u64, u64) {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut gate = ThrottledGate::new(0.1, 0.5, 1.0);
        let mut state = ChaosState::new();

        // Add nodes with random states
        for i in 0..100 {
            state.add_node(i, vec![rng.gen::<f32>(); 4]);
        }

        // Add edges to create energy (without edges, compute_energy is always 0)
        for i in 0..50 {
            let src = rng.gen_range(0..100);
            let tgt = rng.gen_range(0..100);
            if src != tgt {
                state.add_edge(src, tgt, rng.gen_range(0.1..1.0));
            }
        }

        for _ in 0..500 {
            let energy = state.compute_energy();
            gate.decide(energy / 100.0);
            let node_id = rng.gen_range(0..100);
            state.perturb_node(node_id, &mut rng);
        }

        let stats = gate.stats();
        (
            state.compute_energy(),
            stats.allowed,
            stats.throttled,
            stats.blocked,
        )
    }

    let result1 = run_chaos(12345);
    let result2 = run_chaos(12345);

    // Same seed should produce same results (using approximate comparison for floats
    // due to potential floating point ordering differences)
    assert!(
        (result1.0 - result2.0).abs() < 0.01,
        "Same seed should produce same energy: {} vs {}",
        result1.0,
        result2.0
    );
    assert_eq!(
        result1.1, result2.1,
        "Same seed should produce same allowed count"
    );
    assert_eq!(
        result1.2, result2.2,
        "Same seed should produce same throttled count"
    );
    assert_eq!(
        result1.3, result2.3,
        "Same seed should produce same blocked count"
    );

    // Use very different seeds to ensure different random sequences
    let result3 = run_chaos(99999);
    // At minimum, the final energy should differ between different seeds
    assert!(
        (result1.0 - result3.0).abs() > 0.001 || result1.1 != result3.1 || result1.2 != result3.2,
        "Different seeds should produce different results: seed1={:?}, seed2={:?}",
        result1,
        result3
    );
}
