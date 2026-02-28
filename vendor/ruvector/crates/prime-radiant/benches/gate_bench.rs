//! Benchmarks for coherence gate evaluation
//!
//! ADR-014 Performance Target: < 500us per gate evaluation
//!
//! The gate is a deterministic decision point that:
//! 1. Evaluates current energy against thresholds
//! 2. Checks persistence history
//! 3. Determines compute lane (Reflex/Retrieval/Heavy/Human)
//! 4. Creates witness record

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::VecDeque;
use std::time::Duration;

// ============================================================================
// Types (Simulated for benchmarking)
// ============================================================================

/// Compute lanes for escalating complexity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComputeLane {
    /// Lane 0: Local residual updates (<1ms)
    Reflex = 0,
    /// Lane 1: Evidence fetching (~10ms)
    Retrieval = 1,
    /// Lane 2: Multi-step planning (~100ms)
    Heavy = 2,
    /// Lane 3: Human escalation
    Human = 3,
}

/// Coherence energy snapshot
#[derive(Clone)]
pub struct CoherenceEnergy {
    pub total_energy: f32,
    pub scope_energies: Vec<(u64, f32)>, // (scope_id, energy)
    pub timestamp: u64,
    pub fingerprint: u64,
}

impl CoherenceEnergy {
    pub fn new(total: f32, num_scopes: usize) -> Self {
        let scope_energies: Vec<(u64, f32)> = (0..num_scopes)
            .map(|i| (i as u64, total / num_scopes as f32))
            .collect();

        Self {
            total_energy: total,
            scope_energies,
            timestamp: 0,
            fingerprint: (total.to_bits() as u64).wrapping_mul(0x517cc1b727220a95),
        }
    }

    pub fn scope_energy(&self, scope_id: u64) -> f32 {
        self.scope_energies
            .iter()
            .find(|(id, _)| *id == scope_id)
            .map(|(_, e)| *e)
            .unwrap_or(0.0)
    }
}

/// Action to be gated
#[derive(Clone)]
pub struct Action {
    pub id: u64,
    pub scope_id: u64,
    pub action_type: ActionType,
    pub payload_hash: u64,
}

#[derive(Clone, Copy)]
pub enum ActionType {
    Read,
    Write,
    Execute,
    External,
}

/// Threshold configuration
#[derive(Clone)]
pub struct ThresholdConfig {
    pub reflex: f32,
    pub retrieval: f32,
    pub heavy: f32,
    pub persistence_window_ms: u64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            reflex: 0.1,
            retrieval: 0.5,
            heavy: 1.0,
            persistence_window_ms: 5000,
        }
    }
}

/// Energy history for persistence detection
pub struct EnergyHistory {
    /// Rolling window of (timestamp_ms, energy) pairs per scope
    history: Vec<VecDeque<(u64, f32)>>,
    max_scopes: usize,
    window_size: usize,
}

impl EnergyHistory {
    pub fn new(max_scopes: usize, window_size: usize) -> Self {
        Self {
            history: (0..max_scopes)
                .map(|_| VecDeque::with_capacity(window_size))
                .collect(),
            max_scopes,
            window_size,
        }
    }

    pub fn record(&mut self, scope_id: u64, timestamp_ms: u64, energy: f32) {
        if (scope_id as usize) < self.max_scopes {
            let queue = &mut self.history[scope_id as usize];
            if queue.len() >= self.window_size {
                queue.pop_front();
            }
            queue.push_back((timestamp_ms, energy));
        }
    }

    pub fn is_above_threshold(
        &self,
        scope_id: u64,
        threshold: f32,
        window_ms: u64,
        current_time_ms: u64,
    ) -> bool {
        if (scope_id as usize) >= self.max_scopes {
            return false;
        }

        let queue = &self.history[scope_id as usize];
        let cutoff = current_time_ms.saturating_sub(window_ms);

        // Check if all samples in window are above threshold
        let samples_in_window: Vec<_> = queue.iter().filter(|(ts, _)| *ts >= cutoff).collect();

        if samples_in_window.is_empty() {
            return false;
        }

        samples_in_window.iter().all(|(_, e)| *e >= threshold)
    }

    pub fn trend(&self, scope_id: u64, window_ms: u64, current_time_ms: u64) -> Option<f32> {
        if (scope_id as usize) >= self.max_scopes {
            return None;
        }

        let queue = &self.history[scope_id as usize];
        let cutoff = current_time_ms.saturating_sub(window_ms);

        let samples: Vec<_> = queue.iter().filter(|(ts, _)| *ts >= cutoff).collect();

        if samples.len() < 2 {
            return None;
        }

        // Simple linear trend: (last - first) / count
        let first = samples.first().unwrap().1;
        let last = samples.last().unwrap().1;
        Some((last - first) / samples.len() as f32)
    }
}

/// Witness record for audit
#[derive(Clone)]
pub struct WitnessRecord {
    pub id: u64,
    pub action_hash: u64,
    pub energy_fingerprint: u64,
    pub lane: ComputeLane,
    pub allowed: bool,
    pub timestamp: u64,
    pub content_hash: u64,
}

impl WitnessRecord {
    pub fn new(
        action: &Action,
        energy: &CoherenceEnergy,
        lane: ComputeLane,
        allowed: bool,
        timestamp: u64,
    ) -> Self {
        let content_hash = Self::compute_hash(action, energy, lane, allowed, timestamp);

        Self {
            id: timestamp, // Simplified
            action_hash: action.payload_hash,
            energy_fingerprint: energy.fingerprint,
            lane,
            allowed,
            timestamp,
            content_hash,
        }
    }

    fn compute_hash(
        action: &Action,
        energy: &CoherenceEnergy,
        lane: ComputeLane,
        allowed: bool,
        timestamp: u64,
    ) -> u64 {
        // Simplified hash computation (in production: use Blake3)
        let mut h = action.payload_hash;
        h = h.wrapping_mul(0x517cc1b727220a95);
        h ^= energy.fingerprint;
        h = h.wrapping_mul(0x517cc1b727220a95);
        h ^= (lane as u64) << 32 | (allowed as u64);
        h = h.wrapping_mul(0x517cc1b727220a95);
        h ^= timestamp;
        h
    }
}

/// Gate decision result
pub struct GateDecision {
    pub allow: bool,
    pub lane: ComputeLane,
    pub witness: WitnessRecord,
    pub denial_reason: Option<&'static str>,
}

/// Coherence gate
pub struct CoherenceGate {
    pub config: ThresholdConfig,
    pub history: EnergyHistory,
    current_time_ms: u64,
}

impl CoherenceGate {
    pub fn new(config: ThresholdConfig, max_scopes: usize) -> Self {
        Self {
            config,
            history: EnergyHistory::new(max_scopes, 100),
            current_time_ms: 0,
        }
    }

    /// Evaluate whether action should proceed
    pub fn evaluate(&mut self, action: &Action, energy: &CoherenceEnergy) -> GateDecision {
        let current_energy = energy.scope_energy(action.scope_id);

        // Record in history
        self.history
            .record(action.scope_id, self.current_time_ms, current_energy);

        // Determine lane based on energy
        let lane = if current_energy < self.config.reflex {
            ComputeLane::Reflex
        } else if current_energy < self.config.retrieval {
            ComputeLane::Retrieval
        } else if current_energy < self.config.heavy {
            ComputeLane::Heavy
        } else {
            ComputeLane::Human
        };

        // Check for persistent incoherence
        let persistent = self.history.is_above_threshold(
            action.scope_id,
            self.config.retrieval,
            self.config.persistence_window_ms,
            self.current_time_ms,
        );

        // Check for growing incoherence (trend)
        let growing = self
            .history
            .trend(
                action.scope_id,
                self.config.persistence_window_ms,
                self.current_time_ms,
            )
            .map(|t| t > 0.01)
            .unwrap_or(false);

        // Escalate if persistent and not already at high lane
        let final_lane = if (persistent || growing) && lane < ComputeLane::Heavy {
            ComputeLane::Heavy
        } else {
            lane
        };

        // Allow unless Human lane
        let allow = final_lane < ComputeLane::Human;

        let denial_reason = if !allow {
            Some("Energy exceeds all automatic thresholds")
        } else if persistent {
            Some("Persistent incoherence - escalated")
        } else {
            None
        };

        let witness = WitnessRecord::new(action, energy, final_lane, allow, self.current_time_ms);

        self.current_time_ms += 1;

        GateDecision {
            allow,
            lane: final_lane,
            witness,
            denial_reason,
        }
    }

    /// Fast path evaluation (no history update)
    #[inline]
    pub fn evaluate_fast(&self, scope_energy: f32) -> ComputeLane {
        if scope_energy < self.config.reflex {
            ComputeLane::Reflex
        } else if scope_energy < self.config.retrieval {
            ComputeLane::Retrieval
        } else if scope_energy < self.config.heavy {
            ComputeLane::Heavy
        } else {
            ComputeLane::Human
        }
    }

    /// Advance time (for benchmarking)
    pub fn advance_time(&mut self, delta_ms: u64) {
        self.current_time_ms += delta_ms;
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark full gate evaluation
fn bench_gate_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_evaluate");
    group.throughput(Throughput::Elements(1));

    let config = ThresholdConfig::default();
    let mut gate = CoherenceGate::new(config, 100);

    let action = Action {
        id: 1,
        scope_id: 0,
        action_type: ActionType::Write,
        payload_hash: 0x12345678,
    };

    // Low energy (Reflex lane)
    let low_energy = CoherenceEnergy::new(0.05, 10);
    group.bench_function("low_energy_reflex", |b| {
        b.iter(|| {
            let decision = gate.evaluate(black_box(&action), black_box(&low_energy));
            black_box(decision.lane)
        })
    });

    // Medium energy (Retrieval lane)
    let med_energy = CoherenceEnergy::new(0.3, 10);
    group.bench_function("medium_energy_retrieval", |b| {
        b.iter(|| {
            let decision = gate.evaluate(black_box(&action), black_box(&med_energy));
            black_box(decision.lane)
        })
    });

    // High energy (Heavy lane)
    let high_energy = CoherenceEnergy::new(0.8, 10);
    group.bench_function("high_energy_heavy", |b| {
        b.iter(|| {
            let decision = gate.evaluate(black_box(&action), black_box(&high_energy));
            black_box(decision.lane)
        })
    });

    // Critical energy (Human lane)
    let critical_energy = CoherenceEnergy::new(2.0, 10);
    group.bench_function("critical_energy_human", |b| {
        b.iter(|| {
            let decision = gate.evaluate(black_box(&action), black_box(&critical_energy));
            black_box(decision.lane)
        })
    });

    group.finish();
}

/// Benchmark fast path evaluation (no history)
fn bench_gate_fast_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_fast_path");
    group.throughput(Throughput::Elements(1));

    let config = ThresholdConfig::default();
    let gate = CoherenceGate::new(config, 100);

    for energy in [0.05, 0.3, 0.8, 2.0] {
        group.bench_with_input(
            BenchmarkId::new("evaluate_fast", format!("{:.2}", energy)),
            &energy,
            |b, &e| b.iter(|| black_box(gate.evaluate_fast(black_box(e)))),
        );
    }

    group.finish();
}

/// Benchmark witness record creation
fn bench_witness_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_witness");
    group.throughput(Throughput::Elements(1));

    let action = Action {
        id: 1,
        scope_id: 0,
        action_type: ActionType::Write,
        payload_hash: 0x12345678,
    };
    let energy = CoherenceEnergy::new(0.3, 10);

    group.bench_function("create_witness", |b| {
        b.iter(|| {
            WitnessRecord::new(
                black_box(&action),
                black_box(&energy),
                black_box(ComputeLane::Retrieval),
                black_box(true),
                black_box(12345),
            )
        })
    });

    group.finish();
}

/// Benchmark history operations
fn bench_history_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_history");

    let mut history = EnergyHistory::new(100, 1000);

    // Pre-populate with some history
    for t in 0..500 {
        for scope in 0..10u64 {
            history.record(scope, t, 0.3 + (t % 10) as f32 * 0.01);
        }
    }

    // Record operation
    group.bench_function("record_single", |b| {
        let mut t = 1000u64;
        b.iter(|| {
            history.record(black_box(5), black_box(t), black_box(0.35));
            t += 1;
        })
    });

    // Check threshold
    group.bench_function("check_threshold", |b| {
        b.iter(|| {
            history.is_above_threshold(black_box(5), black_box(0.3), black_box(100), black_box(500))
        })
    });

    // Compute trend
    group.bench_function("compute_trend", |b| {
        b.iter(|| history.trend(black_box(5), black_box(100), black_box(500)))
    });

    group.finish();
}

/// Benchmark persistence detection with various window sizes
fn bench_persistence_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_persistence");

    for window_size in [10, 100, 1000] {
        let mut history = EnergyHistory::new(10, window_size);

        // Fill history
        for t in 0..window_size as u64 {
            history.record(0, t, 0.4); // Consistently above retrieval threshold
        }

        group.bench_with_input(
            BenchmarkId::new("check_persistent", window_size),
            &window_size,
            |b, &size| {
                b.iter(|| {
                    history.is_above_threshold(
                        black_box(0),
                        black_box(0.3),
                        black_box(size as u64),
                        black_box(size as u64),
                    )
                })
            },
        );
    }

    group.finish();
}

/// Benchmark batch evaluation (multiple actions)
fn bench_batch_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_batch");

    let config = ThresholdConfig::default();
    let mut gate = CoherenceGate::new(config, 100);

    for batch_size in [10, 100, 1000] {
        let actions: Vec<Action> = (0..batch_size)
            .map(|i| Action {
                id: i as u64,
                scope_id: (i % 10) as u64,
                action_type: ActionType::Write,
                payload_hash: i as u64 * 0x517cc1b727220a95,
            })
            .collect();

        let energies: Vec<CoherenceEnergy> = (0..batch_size)
            .map(|i| CoherenceEnergy::new(0.1 + (i % 20) as f32 * 0.05, 10))
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("evaluate_batch", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    let mut lanes = Vec::with_capacity(actions.len());
                    for (action, energy) in actions.iter().zip(energies.iter()) {
                        let decision = gate.evaluate(action, energy);
                        lanes.push(decision.lane);
                    }
                    black_box(lanes)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark scope energy lookup
fn bench_scope_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_scope_lookup");

    for num_scopes in [10, 100, 1000] {
        let energy = CoherenceEnergy::new(1.0, num_scopes);

        group.bench_with_input(
            BenchmarkId::new("lookup", num_scopes),
            &num_scopes,
            |b, &n| {
                let scope_id = (n / 2) as u64;
                b.iter(|| black_box(energy.scope_energy(black_box(scope_id))))
            },
        );
    }

    group.finish();
}

/// Benchmark threshold comparison patterns
fn bench_threshold_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_threshold_cmp");

    let config = ThresholdConfig::default();

    // Sequential if-else (current implementation)
    group.bench_function("sequential_if_else", |b| {
        let energies: Vec<f32> = (0..1000).map(|i| (i as f32) * 0.002).collect();
        b.iter(|| {
            let mut lanes = [0u32; 4];
            for &e in &energies {
                let lane = if e < config.reflex {
                    0
                } else if e < config.retrieval {
                    1
                } else if e < config.heavy {
                    2
                } else {
                    3
                };
                lanes[lane] += 1;
            }
            black_box(lanes)
        })
    });

    // Binary search pattern
    group.bench_function("binary_search", |b| {
        let thresholds = [config.reflex, config.retrieval, config.heavy, f32::MAX];
        let energies: Vec<f32> = (0..1000).map(|i| (i as f32) * 0.002).collect();
        b.iter(|| {
            let mut lanes = [0u32; 4];
            for &e in &energies {
                let lane = thresholds.partition_point(|&t| t <= e);
                lanes[lane.min(3)] += 1;
            }
            black_box(lanes)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_gate_evaluate,
    bench_gate_fast_path,
    bench_witness_creation,
    bench_history_operations,
    bench_persistence_detection,
    bench_batch_evaluation,
    bench_scope_lookup,
    bench_threshold_comparison,
);

criterion_main!(benches);
