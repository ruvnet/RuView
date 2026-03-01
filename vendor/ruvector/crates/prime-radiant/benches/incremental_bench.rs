//! Benchmarks for incremental coherence updates
//!
//! ADR-014 Performance Target: < 100us for single node update
//!
//! Incremental computation recomputes only affected edges when
//! a single node changes, avoiding full graph recomputation.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::{HashMap, HashSet};

// ============================================================================
// Types (Simulated for benchmarking)
// ============================================================================

#[derive(Clone)]
pub struct RestrictionMap {
    pub matrix: Vec<f32>,
    pub bias: Vec<f32>,
    pub input_dim: usize,
    pub output_dim: usize,
}

impl RestrictionMap {
    pub fn identity(dim: usize) -> Self {
        let mut matrix = vec![0.0f32; dim * dim];
        for i in 0..dim {
            matrix[i * dim + i] = 1.0;
        }
        Self {
            matrix,
            bias: vec![0.0; dim],
            input_dim: dim,
            output_dim: dim,
        }
    }

    #[inline]
    pub fn apply_into(&self, input: &[f32], output: &mut [f32]) {
        output.copy_from_slice(&self.bias);
        for i in 0..self.output_dim {
            let row_start = i * self.input_dim;
            for j in 0..self.input_dim {
                output[i] += self.matrix[row_start + j] * input[j];
            }
        }
    }
}

#[derive(Clone)]
pub struct SheafNode {
    pub id: u64,
    pub state: Vec<f32>,
}

#[derive(Clone)]
pub struct SheafEdge {
    pub id: u64,
    pub source: u64,
    pub target: u64,
    pub weight: f32,
    pub rho_source: RestrictionMap,
    pub rho_target: RestrictionMap,
}

impl SheafEdge {
    #[inline]
    pub fn weighted_residual_energy_into(
        &self,
        source: &[f32],
        target: &[f32],
        source_buf: &mut [f32],
        target_buf: &mut [f32],
    ) -> f32 {
        self.rho_source.apply_into(source, source_buf);
        self.rho_target.apply_into(target, target_buf);

        let mut norm_sq = 0.0f32;
        for i in 0..source_buf.len() {
            let diff = source_buf[i] - target_buf[i];
            norm_sq += diff * diff;
        }

        self.weight * norm_sq
    }
}

/// Incremental coherence tracker
pub struct IncrementalCoherence {
    pub nodes: HashMap<u64, SheafNode>,
    pub edges: Vec<SheafEdge>,
    pub state_dim: usize,
    /// Node -> incident edge indices
    pub node_to_edges: HashMap<u64, Vec<usize>>,
    /// Cached per-edge energies
    pub edge_energies: Vec<f32>,
    /// Cached total energy
    pub total_energy: f32,
    /// Fingerprint for staleness detection
    pub fingerprint: u64,
}

impl IncrementalCoherence {
    pub fn new(nodes: HashMap<u64, SheafNode>, edges: Vec<SheafEdge>, state_dim: usize) -> Self {
        // Build node-to-edge index
        let mut node_to_edges: HashMap<u64, Vec<usize>> = HashMap::new();
        for (idx, edge) in edges.iter().enumerate() {
            node_to_edges.entry(edge.source).or_default().push(idx);
            node_to_edges.entry(edge.target).or_default().push(idx);
        }

        let mut tracker = Self {
            nodes,
            edges,
            state_dim,
            node_to_edges,
            edge_energies: Vec::new(),
            total_energy: 0.0,
            fingerprint: 0,
        };

        tracker.full_recompute();
        tracker
    }

    /// Full recomputation (initial or when needed)
    pub fn full_recompute(&mut self) {
        let mut source_buf = vec![0.0f32; self.state_dim];
        let mut target_buf = vec![0.0f32; self.state_dim];

        self.edge_energies = self
            .edges
            .iter()
            .map(|edge| {
                let source_state = &self.nodes[&edge.source].state;
                let target_state = &self.nodes[&edge.target].state;
                edge.weighted_residual_energy_into(
                    source_state,
                    target_state,
                    &mut source_buf,
                    &mut target_buf,
                )
            })
            .collect();

        self.total_energy = self.edge_energies.iter().sum();
        self.update_fingerprint();
    }

    /// Update single node and recompute affected edges only
    pub fn update_node(&mut self, node_id: u64, new_state: Vec<f32>) {
        // Update node state
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.state = new_state;
        } else {
            return;
        }

        // Get affected edges
        let affected_edges = match self.node_to_edges.get(&node_id) {
            Some(edges) => edges.clone(),
            None => return,
        };

        // Recompute only affected edges
        let mut source_buf = vec![0.0f32; self.state_dim];
        let mut target_buf = vec![0.0f32; self.state_dim];

        let mut energy_delta = 0.0f32;

        for &edge_idx in &affected_edges {
            let edge = &self.edges[edge_idx];
            let source_state = &self.nodes[&edge.source].state;
            let target_state = &self.nodes[&edge.target].state;

            let old_energy = self.edge_energies[edge_idx];
            let new_energy = edge.weighted_residual_energy_into(
                source_state,
                target_state,
                &mut source_buf,
                &mut target_buf,
            );

            energy_delta += new_energy - old_energy;
            self.edge_energies[edge_idx] = new_energy;
        }

        self.total_energy += energy_delta;
        self.update_fingerprint();
    }

    /// Update multiple nodes in batch
    pub fn update_nodes_batch(&mut self, updates: Vec<(u64, Vec<f32>)>) {
        // Collect all affected edges
        let mut affected_edges: HashSet<usize> = HashSet::new();

        for (node_id, new_state) in updates {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.state = new_state;
            }
            if let Some(edges) = self.node_to_edges.get(&node_id) {
                affected_edges.extend(edges.iter());
            }
        }

        // Recompute affected edges
        let mut source_buf = vec![0.0f32; self.state_dim];
        let mut target_buf = vec![0.0f32; self.state_dim];

        let mut energy_delta = 0.0f32;

        for edge_idx in affected_edges {
            let edge = &self.edges[edge_idx];
            let source_state = &self.nodes[&edge.source].state;
            let target_state = &self.nodes[&edge.target].state;

            let old_energy = self.edge_energies[edge_idx];
            let new_energy = edge.weighted_residual_energy_into(
                source_state,
                target_state,
                &mut source_buf,
                &mut target_buf,
            );

            energy_delta += new_energy - old_energy;
            self.edge_energies[edge_idx] = new_energy;
        }

        self.total_energy += energy_delta;
        self.update_fingerprint();
    }

    fn update_fingerprint(&mut self) {
        self.fingerprint = self.fingerprint.wrapping_add(1);
    }

    /// Get current total energy
    pub fn energy(&self) -> f32 {
        self.total_energy
    }

    /// Get energy for specific edge
    pub fn edge_energy(&self, edge_idx: usize) -> f32 {
        self.edge_energies[edge_idx]
    }

    /// Check if cache is stale (fingerprint changed)
    pub fn is_stale(&self, last_fingerprint: u64) -> bool {
        self.fingerprint != last_fingerprint
    }
}

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_state(dim: usize, seed: u64) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    (0..dim)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
        })
        .collect()
}

fn create_random_graph(
    num_nodes: usize,
    avg_degree: usize,
    state_dim: usize,
) -> IncrementalCoherence {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
        .map(|id| {
            (
                id,
                SheafNode {
                    id,
                    state: generate_state(state_dim, id),
                },
            )
        })
        .collect();

    let num_edges = (num_nodes * avg_degree) / 2;
    let edges: Vec<SheafEdge> = (0..num_edges)
        .filter_map(|i| {
            let mut hasher = DefaultHasher::new();
            (42u64, i, "src").hash(&mut hasher);
            let source = hasher.finish() % num_nodes as u64;

            let mut hasher = DefaultHasher::new();
            (42u64, i, "tgt").hash(&mut hasher);
            let target = hasher.finish() % num_nodes as u64;

            if source != target {
                Some(SheafEdge {
                    id: i as u64,
                    source,
                    target,
                    weight: 1.0,
                    rho_source: RestrictionMap::identity(state_dim),
                    rho_target: RestrictionMap::identity(state_dim),
                })
            } else {
                None
            }
        })
        .collect();

    IncrementalCoherence::new(nodes, edges, state_dim)
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark single node update at various graph sizes
fn bench_single_node_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_single_node");
    group.throughput(Throughput::Elements(1));

    // ADR-014 target: <100us for single node update
    for num_nodes in [100, 1_000, 10_000] {
        let state_dim = 64;
        let avg_degree = 4;
        let mut tracker = create_random_graph(num_nodes, avg_degree, state_dim);

        group.bench_with_input(
            BenchmarkId::new("update", format!("{}nodes", num_nodes)),
            &num_nodes,
            |b, _| {
                let node_id = (num_nodes / 2) as u64; // Update middle node
                b.iter(|| {
                    let new_state = generate_state(state_dim, black_box(rand::random()));
                    tracker.update_node(black_box(node_id), new_state);
                    black_box(tracker.energy())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark incremental vs full recomputation
fn bench_incremental_vs_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_vs_full");

    let num_nodes = 10_000;
    let state_dim = 64;
    let avg_degree = 4;
    let mut tracker = create_random_graph(num_nodes, avg_degree, state_dim);

    // Incremental update
    group.bench_function("incremental_single", |b| {
        let node_id = 5000u64;
        b.iter(|| {
            let new_state = generate_state(state_dim, rand::random());
            tracker.update_node(black_box(node_id), new_state);
            black_box(tracker.energy())
        })
    });

    // Full recomputation
    group.bench_function("full_recompute", |b| {
        b.iter(|| {
            tracker.full_recompute();
            black_box(tracker.energy())
        })
    });

    group.finish();
}

/// Benchmark node degree impact on update time
fn bench_node_degree_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_degree_impact");

    let num_nodes = 10_000;
    let state_dim = 64;

    // Create graph with hub node (high degree)
    let nodes: HashMap<u64, SheafNode> = (0..num_nodes as u64)
        .map(|id| {
            (
                id,
                SheafNode {
                    id,
                    state: generate_state(state_dim, id),
                },
            )
        })
        .collect();

    // Hub node 0 connects to many nodes
    let hub_degree = 1000;
    let mut edges: Vec<SheafEdge> = (1..=hub_degree)
        .map(|i| SheafEdge {
            id: i as u64,
            source: 0,
            target: i as u64,
            weight: 1.0,
            rho_source: RestrictionMap::identity(state_dim),
            rho_target: RestrictionMap::identity(state_dim),
        })
        .collect();

    // Regular edges for other nodes (degree ~4)
    for i in hub_degree + 1..num_nodes - 1 {
        edges.push(SheafEdge {
            id: i as u64,
            source: i as u64,
            target: (i + 1) as u64,
            weight: 1.0,
            rho_source: RestrictionMap::identity(state_dim),
            rho_target: RestrictionMap::identity(state_dim),
        });
    }

    let mut tracker = IncrementalCoherence::new(nodes, edges, state_dim);

    // Update hub node (high degree)
    group.bench_function("update_hub_1000_edges", |b| {
        b.iter(|| {
            let new_state = generate_state(state_dim, rand::random());
            tracker.update_node(black_box(0), new_state);
            black_box(tracker.energy())
        })
    });

    // Update leaf node (degree 1-2)
    group.bench_function("update_leaf_2_edges", |b| {
        let leaf_id = (hub_degree + 100) as u64;
        b.iter(|| {
            let new_state = generate_state(state_dim, rand::random());
            tracker.update_node(black_box(leaf_id), new_state);
            black_box(tracker.energy())
        })
    });

    group.finish();
}

/// Benchmark batch updates
fn bench_batch_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_batch");

    let num_nodes = 10_000;
    let state_dim = 64;
    let avg_degree = 4;

    for batch_size in [1, 10, 100, 1000] {
        let mut tracker = create_random_graph(num_nodes, avg_degree, state_dim);

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_update", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let updates: Vec<(u64, Vec<f32>)> = (0..size)
                        .map(|i| {
                            let node_id = (i * 10) as u64 % num_nodes as u64;
                            let state = generate_state(state_dim, rand::random());
                            (node_id, state)
                        })
                        .collect();

                    tracker.update_nodes_batch(black_box(updates));
                    black_box(tracker.energy())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark state dimension impact
fn bench_state_dim_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_state_dim");

    let num_nodes = 10_000;
    let avg_degree = 4;

    for state_dim in [8, 32, 64, 128, 256] {
        let mut tracker = create_random_graph(num_nodes, avg_degree, state_dim);

        group.bench_with_input(
            BenchmarkId::new("update", state_dim),
            &state_dim,
            |b, &dim| {
                let node_id = 5000u64;
                b.iter(|| {
                    let new_state = generate_state(dim, rand::random());
                    tracker.update_node(black_box(node_id), new_state);
                    black_box(tracker.energy())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark index lookup performance
fn bench_index_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_index_lookup");

    let num_nodes = 100_000;
    let avg_degree = 4;
    let state_dim = 64;
    let tracker = create_random_graph(num_nodes, avg_degree, state_dim);

    // Lookup incident edges for a node
    group.bench_function("lookup_incident_edges", |b| {
        b.iter(|| {
            let node_id = black_box(50_000u64);
            black_box(tracker.node_to_edges.get(&node_id))
        })
    });

    // Iterate incident edges
    group.bench_function("iterate_incident_edges", |b| {
        let node_id = 50_000u64;
        b.iter(|| {
            let sum = if let Some(edges) = tracker.node_to_edges.get(&node_id) {
                edges.iter().map(|&idx| tracker.edge_energies[idx]).sum()
            } else {
                0.0f32
            };
            black_box(sum)
        })
    });

    group.finish();
}

/// Benchmark fingerprint operations
fn bench_fingerprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_fingerprint");

    let num_nodes = 10_000;
    let avg_degree = 4;
    let state_dim = 64;
    let mut tracker = create_random_graph(num_nodes, avg_degree, state_dim);

    group.bench_function("check_staleness", |b| {
        let fp = tracker.fingerprint;
        b.iter(|| black_box(tracker.is_stale(black_box(fp))))
    });

    group.bench_function("update_with_fingerprint_check", |b| {
        let node_id = 5000u64;
        b.iter(|| {
            let old_fp = tracker.fingerprint;
            let new_state = generate_state(state_dim, rand::random());
            tracker.update_node(black_box(node_id), new_state);
            let is_changed = tracker.is_stale(old_fp);
            black_box((tracker.energy(), is_changed))
        })
    });

    group.finish();
}

/// Benchmark worst case: update all nodes sequentially
fn bench_sequential_all_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_sequential_all");
    group.sample_size(10);

    let num_nodes = 1000;
    let avg_degree = 4;
    let state_dim = 64;

    let mut tracker = create_random_graph(num_nodes, avg_degree, state_dim);

    group.bench_function("update_all_1000_sequential", |b| {
        b.iter(|| {
            for node_id in 0..num_nodes as u64 {
                let new_state = generate_state(state_dim, node_id);
                tracker.update_node(node_id, new_state);
            }
            black_box(tracker.energy())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_node_update,
    bench_incremental_vs_full,
    bench_node_degree_impact,
    bench_batch_updates,
    bench_state_dim_impact,
    bench_index_lookup,
    bench_fingerprint,
    bench_sequential_all_updates,
);

criterion_main!(benches);
