//! Integrated Coherence Benchmarks for Prime-Radiant
//!
//! End-to-end benchmarks combining all modules:
//! - Full coherence pipeline (topology -> spectral -> causal -> decision)
//! - Memory usage profiling
//! - Throughput measurements
//! - Scalability analysis
//!
//! Target metrics:
//! - End-to-end coherence: < 50ms for 1K entities
//! - Memory overhead: < 100MB for 10K entities
//! - Throughput: > 100 decisions/second

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// INTEGRATED COHERENCE ENGINE
// ============================================================================

/// Entity in the coherence graph
#[derive(Clone, Debug)]
struct Entity {
    id: usize,
    state: Vec<f64>,
    beliefs: Vec<Belief>,
}

/// A belief with confidence
#[derive(Clone, Debug)]
struct Belief {
    content: String,
    confidence: f64,
    source_id: usize,
}

/// Constraint between entities
#[derive(Clone, Debug)]
struct Constraint {
    source: usize,
    target: usize,
    weight: f64,
    restriction_map: Vec<Vec<f64>>,
}

/// Coherence decision
#[derive(Clone, Debug)]
pub enum CoherenceDecision {
    Accept { confidence: f64 },
    Reject { reason: String, energy: f64 },
    Defer { required_evidence: Vec<String> },
}

/// Full coherence computation result
#[derive(Clone, Debug)]
pub struct CoherenceResult {
    /// Total coherence energy (lower is better)
    pub total_energy: f64,
    /// Topological coherence (from cohomology)
    pub topological_energy: f64,
    /// Spectral coherence (from eigenvalues)
    pub spectral_energy: f64,
    /// Causal coherence (from intervention consistency)
    pub causal_energy: f64,
    /// Betti numbers
    pub betti: Vec<usize>,
    /// Spectral gap
    pub spectral_gap: f64,
    /// Final decision
    pub decision: CoherenceDecision,
}

/// Integrated coherence engine
struct CoherenceEngine {
    entities: Vec<Entity>,
    constraints: Vec<Constraint>,
    /// Thresholds for decision making
    accept_threshold: f64,
    reject_threshold: f64,
}

impl CoherenceEngine {
    fn new() -> Self {
        Self {
            entities: Vec::new(),
            constraints: Vec::new(),
            accept_threshold: 0.1,
            reject_threshold: 1.0,
        }
    }

    fn add_entity(&mut self, state_dim: usize) -> usize {
        let id = self.entities.len();
        let entity = Entity {
            id,
            state: vec![0.0; state_dim],
            beliefs: Vec::new(),
        };
        self.entities.push(entity);
        id
    }

    fn set_state(&mut self, id: usize, state: Vec<f64>) {
        if id < self.entities.len() {
            self.entities[id].state = state;
        }
    }

    fn add_constraint(&mut self, source: usize, target: usize, weight: f64) {
        let dim = if source < self.entities.len() {
            self.entities[source].state.len()
        } else {
            16
        };

        // Identity restriction map
        let restriction_map: Vec<Vec<f64>> = (0..dim)
            .map(|i| {
                let mut row = vec![0.0; dim];
                row[i] = 1.0;
                row
            })
            .collect();

        self.constraints.push(Constraint {
            source,
            target,
            weight,
            restriction_map,
        });
    }

    /// Compute full coherence
    fn compute_coherence(&self) -> CoherenceResult {
        // 1. Topological coherence via coboundary computation
        let topological_energy = self.compute_topological_energy();

        // 2. Spectral coherence via Laplacian eigenvalues
        let (spectral_energy, spectral_gap) = self.compute_spectral_coherence();

        // 3. Causal coherence via intervention consistency
        let causal_energy = self.compute_causal_energy();

        // 4. Combined energy
        let total_energy = topological_energy + spectral_energy + causal_energy;

        // 5. Betti numbers approximation
        let betti = self.compute_betti_numbers();

        // 6. Decision
        let decision = if total_energy < self.accept_threshold {
            CoherenceDecision::Accept {
                confidence: 1.0 - total_energy / self.accept_threshold,
            }
        } else if total_energy > self.reject_threshold {
            CoherenceDecision::Reject {
                reason: "Energy exceeds rejection threshold".to_string(),
                energy: total_energy,
            }
        } else {
            CoherenceDecision::Defer {
                required_evidence: vec!["Additional context needed".to_string()],
            }
        };

        CoherenceResult {
            total_energy,
            topological_energy,
            spectral_energy,
            causal_energy,
            betti,
            spectral_gap,
            decision,
        }
    }

    fn compute_topological_energy(&self) -> f64 {
        let mut energy = 0.0;

        // Compute residuals at each constraint (coboundary)
        for constraint in &self.constraints {
            if constraint.source >= self.entities.len()
                || constraint.target >= self.entities.len()
            {
                continue;
            }

            let source_state = &self.entities[constraint.source].state;
            let target_state = &self.entities[constraint.target].state;

            // Apply restriction map
            let restricted_source = self.apply_restriction(&constraint.restriction_map, source_state);

            // Residual = rho(source) - target
            let mut residual_sq = 0.0;
            for (rs, ts) in restricted_source.iter().zip(target_state.iter()) {
                let diff = rs - ts;
                residual_sq += diff * diff;
            }

            energy += constraint.weight * residual_sq;
        }

        energy
    }

    fn apply_restriction(&self, map: &[Vec<f64>], state: &[f64]) -> Vec<f64> {
        map.iter()
            .map(|row| {
                row.iter()
                    .zip(state.iter())
                    .map(|(a, b)| a * b)
                    .sum()
            })
            .collect()
    }

    fn compute_spectral_coherence(&self) -> (f64, f64) {
        let n = self.entities.len();
        if n == 0 {
            return (0.0, 0.0);
        }

        // Build Laplacian
        let mut laplacian = vec![vec![0.0; n]; n];
        let mut degrees = vec![0.0; n];

        for constraint in &self.constraints {
            if constraint.source < n && constraint.target < n {
                let w = constraint.weight;
                laplacian[constraint.source][constraint.target] -= w;
                laplacian[constraint.target][constraint.source] -= w;
                degrees[constraint.source] += w;
                degrees[constraint.target] += w;
            }
        }

        for i in 0..n {
            laplacian[i][i] = degrees[i];
        }

        // Power iteration for largest eigenvalue
        let mut v: Vec<f64> = (0..n).map(|i| ((i + 1) as f64).sqrt().sin()).collect();
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        for x in &mut v {
            *x /= norm;
        }

        let mut lambda_max = 0.0;
        for _ in 0..50 {
            let mut y = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    y[i] += laplacian[i][j] * v[j];
                }
            }

            lambda_max = v.iter().zip(y.iter()).map(|(a, b)| a * b).sum();

            let norm: f64 = y.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-10 {
                v = y.iter().map(|x| x / norm).collect();
            }
        }

        // Estimate spectral gap (lambda_2 / lambda_max)
        let spectral_gap = if n > 1 { 0.1 } else { 1.0 }; // Simplified

        // Spectral energy based on eigenvalue distribution
        let spectral_energy = if lambda_max > 0.0 {
            (lambda_max - degrees.iter().sum::<f64>() / n as f64).abs()
        } else {
            0.0
        };

        (spectral_energy * 0.01, spectral_gap)
    }

    fn compute_causal_energy(&self) -> f64 {
        // Check if state updates are consistent with causal ordering
        // Simplified: measure variance in state transitions

        let mut energy = 0.0;
        let mut count = 0;

        for constraint in &self.constraints {
            if constraint.source >= self.entities.len()
                || constraint.target >= self.entities.len()
            {
                continue;
            }

            let source_state = &self.entities[constraint.source].state;
            let target_state = &self.entities[constraint.target].state;

            // Causal consistency: target should be "downstream" of source
            let source_norm: f64 = source_state.iter().map(|x| x * x).sum();
            let target_norm: f64 = target_state.iter().map(|x| x * x).sum();

            // Penalize if target has unexplained variance
            if target_norm > source_norm * 1.5 {
                energy += (target_norm - source_norm * 1.5) * 0.1;
            }

            count += 1;
        }

        if count > 0 {
            energy / count as f64
        } else {
            0.0
        }
    }

    fn compute_betti_numbers(&self) -> Vec<usize> {
        let n = self.entities.len();
        let m = self.constraints.len();

        // Very rough approximation
        // Betti_0 = connected components
        // Betti_1 = independent cycles

        let betti_0 = if n > m { n - m } else { 1 };
        let betti_1 = if m > n { m - n } else { 0 };

        vec![betti_0.max(1), betti_1]
    }
}

// ============================================================================
// STREAMING COHERENCE PROCESSOR
// ============================================================================

/// Incremental coherence updates
struct StreamingCoherence {
    engine: CoherenceEngine,
    /// Cache for incremental updates
    residual_cache: HashMap<(usize, usize), f64>,
    /// Rolling energy window
    energy_history: Vec<f64>,
    history_window: usize,
}

impl StreamingCoherence {
    fn new(history_window: usize) -> Self {
        Self {
            engine: CoherenceEngine::new(),
            residual_cache: HashMap::new(),
            energy_history: Vec::new(),
            history_window,
        }
    }

    fn update_entity(&mut self, id: usize, state: Vec<f64>) -> f64 {
        self.engine.set_state(id, state);

        // Compute incremental energy delta
        let mut delta = 0.0;

        for constraint in &self.engine.constraints {
            if constraint.source == id || constraint.target == id {
                let old_residual = self.residual_cache
                    .get(&(constraint.source, constraint.target))
                    .copied()
                    .unwrap_or(0.0);

                let new_residual = self.compute_residual(constraint);
                delta += (new_residual - old_residual).abs();

                self.residual_cache
                    .insert((constraint.source, constraint.target), new_residual);
            }
        }

        // Update history
        self.energy_history.push(delta);
        if self.energy_history.len() > self.history_window {
            self.energy_history.remove(0);
        }

        delta
    }

    fn compute_residual(&self, constraint: &Constraint) -> f64 {
        if constraint.source >= self.engine.entities.len()
            || constraint.target >= self.engine.entities.len()
        {
            return 0.0;
        }

        let source = &self.engine.entities[constraint.source].state;
        let target = &self.engine.entities[constraint.target].state;

        let restricted = self.engine.apply_restriction(&constraint.restriction_map, source);

        let mut residual_sq = 0.0;
        for (r, t) in restricted.iter().zip(target.iter()) {
            let diff = r - t;
            residual_sq += diff * diff;
        }

        constraint.weight * residual_sq
    }

    fn get_trend(&self) -> f64 {
        if self.energy_history.len() < 2 {
            return 0.0;
        }

        let n = self.energy_history.len();
        let recent = &self.energy_history[(n / 2)..];
        let older = &self.energy_history[..(n / 2)];

        let recent_avg: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
        let older_avg: f64 = older.iter().sum::<f64>() / older.len().max(1) as f64;

        recent_avg - older_avg
    }
}

// ============================================================================
// BATCH COHERENCE PROCESSOR
// ============================================================================

/// Batch processing for high throughput
struct BatchCoherence {
    batch_size: usize,
    pending: Vec<(usize, Vec<f64>)>,
    engine: CoherenceEngine,
}

impl BatchCoherence {
    fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            pending: Vec::new(),
            engine: CoherenceEngine::new(),
        }
    }

    fn add_update(&mut self, id: usize, state: Vec<f64>) -> Option<Vec<CoherenceResult>> {
        self.pending.push((id, state));

        if self.pending.len() >= self.batch_size {
            Some(self.process_batch())
        } else {
            None
        }
    }

    fn process_batch(&mut self) -> Vec<CoherenceResult> {
        let mut results = Vec::with_capacity(self.pending.len());

        for (id, state) in &self.pending {
            self.engine.set_state(*id, state.clone());
            results.push(self.engine.compute_coherence());
        }

        self.pending.clear();
        results
    }

    fn flush(&mut self) -> Vec<CoherenceResult> {
        self.process_batch()
    }
}

// ============================================================================
// MEMORY PROFILING
// ============================================================================

struct MemoryProfile {
    entity_bytes: usize,
    constraint_bytes: usize,
    cache_bytes: usize,
    total_bytes: usize,
}

fn estimate_memory(engine: &CoherenceEngine) -> MemoryProfile {
    let entity_bytes: usize = engine.entities.iter()
        .map(|e| {
            std::mem::size_of::<Entity>()
                + e.state.len() * std::mem::size_of::<f64>()
                + e.beliefs.len() * std::mem::size_of::<Belief>()
        })
        .sum();

    let constraint_bytes: usize = engine.constraints.iter()
        .map(|c| {
            std::mem::size_of::<Constraint>()
                + c.restriction_map.len() * c.restriction_map.get(0).map(|r| r.len()).unwrap_or(0) * std::mem::size_of::<f64>()
        })
        .sum();

    let cache_bytes = 0; // Would include residual cache if implemented

    let total_bytes = entity_bytes + constraint_bytes + cache_bytes
        + std::mem::size_of::<CoherenceEngine>();

    MemoryProfile {
        entity_bytes,
        constraint_bytes,
        cache_bytes,
        total_bytes,
    }
}

// ============================================================================
// DATA GENERATORS
// ============================================================================

fn generate_coherence_graph(num_entities: usize, avg_degree: usize, state_dim: usize) -> CoherenceEngine {
    let mut engine = CoherenceEngine::new();

    // Add entities
    for i in 0..num_entities {
        let id = engine.add_entity(state_dim);
        let state: Vec<f64> = (0..state_dim)
            .map(|j| ((i * state_dim + j) as f64 * 0.1).sin())
            .collect();
        engine.set_state(id, state);
    }

    // Add constraints with random-ish pattern
    let mut rng_state = 42u64;
    for i in 0..num_entities {
        for _ in 0..avg_degree {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (rng_state as usize) % num_entities;

            if i != j {
                let weight = ((rng_state >> 32) as f64 / (u32::MAX as f64)) * 0.9 + 0.1;
                engine.add_constraint(i, j, weight);
            }
        }
    }

    engine
}

fn generate_hierarchical_graph(
    num_levels: usize,
    branching: usize,
    state_dim: usize,
) -> CoherenceEngine {
    let mut engine = CoherenceEngine::new();
    let mut level_nodes: Vec<Vec<usize>> = Vec::new();

    // Create hierarchical structure
    for level in 0..num_levels {
        let num_nodes = branching.pow(level as u32);
        let mut nodes = Vec::new();

        for i in 0..num_nodes {
            let id = engine.add_entity(state_dim);
            let state: Vec<f64> = (0..state_dim)
                .map(|j| ((level * 1000 + i * state_dim + j) as f64 * 0.1).sin())
                .collect();
            engine.set_state(id, state);
            nodes.push(id);
        }

        // Connect to parent level
        if level > 0 {
            for (i, &node) in nodes.iter().enumerate() {
                let parent_idx = i / branching;
                if parent_idx < level_nodes[level - 1].len() {
                    let parent = level_nodes[level - 1][parent_idx];
                    engine.add_constraint(parent, node, 1.0);
                }
            }
        }

        level_nodes.push(nodes);
    }

    engine
}

// ============================================================================
// BENCHMARKS
// ============================================================================

fn bench_end_to_end_coherence(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/end_to_end");
    group.sample_size(20);

    for &num_entities in &[100, 500, 1000, 2000] {
        let engine = generate_coherence_graph(num_entities, 5, 32);

        group.throughput(Throughput::Elements(num_entities as u64));

        group.bench_with_input(
            BenchmarkId::new("full_coherence", num_entities),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.compute_coherence()))
            },
        );
    }

    group.finish();
}

fn bench_component_breakdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/components");
    group.sample_size(30);

    for &num_entities in &[500, 1000, 2000] {
        let engine = generate_coherence_graph(num_entities, 5, 32);

        group.throughput(Throughput::Elements(num_entities as u64));

        group.bench_with_input(
            BenchmarkId::new("topological", num_entities),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.compute_topological_energy()))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("spectral", num_entities),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.compute_spectral_coherence()))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("causal", num_entities),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.compute_causal_energy()))
            },
        );
    }

    group.finish();
}

fn bench_streaming_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/streaming");
    group.sample_size(50);

    for &num_entities in &[500, 1000, 2000] {
        let base_engine = generate_coherence_graph(num_entities, 5, 32);

        group.throughput(Throughput::Elements(100)); // 100 updates per iteration

        group.bench_with_input(
            BenchmarkId::new("incremental_updates", num_entities),
            &num_entities,
            |b, &n| {
                b.iter_batched(
                    || {
                        let mut streaming = StreamingCoherence::new(100);
                        streaming.engine = generate_coherence_graph(n, 5, 32);
                        streaming
                    },
                    |mut streaming| {
                        for i in 0..100 {
                            let state: Vec<f64> = (0..32)
                                .map(|j| ((i * 32 + j) as f64 * 0.01).sin())
                                .collect();
                            black_box(streaming.update_entity(i % n, state));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_batch_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/batch_throughput");
    group.sample_size(20);

    for &batch_size in &[10, 50, 100, 200] {
        let num_entities = 1000;

        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("process_batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let mut batch = BatchCoherence::new(batch_size);
                        batch.engine = generate_coherence_graph(num_entities, 5, 32);

                        // Pre-fill pending
                        for i in 0..(batch_size - 1) {
                            let state: Vec<f64> = (0..32)
                                .map(|j| ((i * 32 + j) as f64 * 0.01).cos())
                                .collect();
                            batch.pending.push((i % num_entities, state));
                        }

                        batch
                    },
                    |mut batch| {
                        let state: Vec<f64> = (0..32).map(|j| (j as f64 * 0.02).sin()).collect();
                        black_box(batch.add_update(0, state))
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_hierarchical_coherence(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/hierarchical");
    group.sample_size(20);

    for &(levels, branching) in &[(3, 4), (4, 3), (5, 2), (4, 4)] {
        let engine = generate_hierarchical_graph(levels, branching, 32);
        let total_nodes: usize = (0..levels).map(|l| branching.pow(l as u32)).sum();

        group.throughput(Throughput::Elements(total_nodes as u64));

        group.bench_with_input(
            BenchmarkId::new(format!("{}L_{}B", levels, branching), total_nodes),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.compute_coherence()))
            },
        );
    }

    group.finish();
}

fn bench_memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/memory");
    group.sample_size(10);

    for &num_entities in &[1000, 5000, 10000] {
        group.bench_with_input(
            BenchmarkId::new("estimate_memory", num_entities),
            &num_entities,
            |b, &n| {
                b.iter_batched(
                    || generate_coherence_graph(n, 5, 32),
                    |engine| black_box(estimate_memory(&engine)),
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_decision_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/decision_throughput");
    group.sample_size(50);

    let engine = generate_coherence_graph(1000, 5, 32);

    group.throughput(Throughput::Elements(1000));

    group.bench_function("decisions_per_second", |b| {
        b.iter(|| {
            let mut count = 0;
            for _ in 0..1000 {
                let result = engine.compute_coherence();
                match result.decision {
                    CoherenceDecision::Accept { .. } => count += 1,
                    CoherenceDecision::Reject { .. } => count += 1,
                    CoherenceDecision::Defer { .. } => count += 1,
                }
            }
            black_box(count)
        })
    });

    group.finish();
}

fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated/scalability");
    group.sample_size(10);

    // Test scaling with both entities and constraints
    for &(entities, avg_degree) in &[(500, 3), (500, 10), (1000, 3), (1000, 10), (2000, 5)] {
        let engine = generate_coherence_graph(entities, avg_degree, 32);
        let total_constraints = engine.constraints.len();

        group.throughput(Throughput::Elements((entities + total_constraints) as u64));

        group.bench_with_input(
            BenchmarkId::new(format!("{}e_{}d", entities, avg_degree), entities),
            &engine,
            |b, engine| {
                b.iter(|| black_box(engine.compute_coherence()))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_end_to_end_coherence,
    bench_component_breakdown,
    bench_streaming_updates,
    bench_batch_throughput,
    bench_hierarchical_coherence,
    bench_memory_scaling,
    bench_decision_throughput,
    bench_scalability,
);
criterion_main!(benches);
