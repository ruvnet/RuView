//! Index integrity and graph maintenance benchmarks
//!
//! Benchmarks for v2 structural integrity features:
//! - Contracted graph construction
//! - Mincut computation time
//! - State transition overhead
//! - Gating check latency
//! - Graph connectivity verification

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

// ============================================================================
// Graph Structures for Index Integrity
// ============================================================================

mod graph {
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

    /// Node in the HNSW graph (simplified)
    #[derive(Clone)]
    pub struct GraphNode {
        pub id: u64,
        pub neighbors: Vec<u64>,
        pub layer: usize,
    }

    /// Graph for integrity checking
    pub struct Graph {
        pub nodes: HashMap<u64, GraphNode>,
        pub max_layer: usize,
    }

    impl Graph {
        pub fn new() -> Self {
            Self {
                nodes: HashMap::new(),
                max_layer: 0,
            }
        }

        pub fn add_node(&mut self, id: u64, layer: usize) {
            self.nodes.insert(
                id,
                GraphNode {
                    id,
                    neighbors: Vec::new(),
                    layer,
                },
            );
            self.max_layer = self.max_layer.max(layer);
        }

        pub fn add_edge(&mut self, from: u64, to: u64) {
            if let Some(node) = self.nodes.get_mut(&from) {
                if !node.neighbors.contains(&to) {
                    node.neighbors.push(to);
                }
            }
        }

        pub fn len(&self) -> usize {
            self.nodes.len()
        }
    }

    /// Contracted graph for integrity verification
    pub struct ContractedGraph {
        /// Super-nodes (contracted regions)
        pub super_nodes: Vec<SuperNode>,
        /// Edges between super-nodes
        pub super_edges: Vec<(usize, usize, f32)>,
        /// Node to super-node mapping
        pub node_mapping: HashMap<u64, usize>,
    }

    #[derive(Clone)]
    pub struct SuperNode {
        pub id: usize,
        pub original_nodes: Vec<u64>,
        pub internal_edges: usize,
    }

    impl ContractedGraph {
        pub fn new() -> Self {
            Self {
                super_nodes: Vec::new(),
                super_edges: Vec::new(),
                node_mapping: HashMap::new(),
            }
        }

        /// Build contracted graph from original graph
        pub fn build_from_graph(graph: &Graph, contraction_factor: usize) -> Self {
            let mut contracted = ContractedGraph::new();

            // Group nodes by region (simplified partitioning)
            let node_ids: Vec<u64> = graph.nodes.keys().copied().collect();
            let num_super_nodes = (node_ids.len() / contraction_factor).max(1);

            for (i, chunk) in node_ids.chunks(contraction_factor).enumerate() {
                let super_node = SuperNode {
                    id: i,
                    original_nodes: chunk.to_vec(),
                    internal_edges: chunk
                        .iter()
                        .filter_map(|&id| graph.nodes.get(&id))
                        .flat_map(|n| n.neighbors.iter())
                        .filter(|&&neighbor| chunk.contains(&neighbor))
                        .count(),
                };

                for &node_id in chunk {
                    contracted.node_mapping.insert(node_id, i);
                }

                contracted.super_nodes.push(super_node);
            }

            // Build super edges
            let mut edge_weights: HashMap<(usize, usize), f32> = HashMap::new();

            for node in graph.nodes.values() {
                let from_super = contracted.node_mapping[&node.id];

                for &neighbor in &node.neighbors {
                    if let Some(&to_super) = contracted.node_mapping.get(&neighbor) {
                        if from_super != to_super {
                            let key = if from_super < to_super {
                                (from_super, to_super)
                            } else {
                                (to_super, from_super)
                            };
                            *edge_weights.entry(key).or_insert(0.0) += 1.0;
                        }
                    }
                }
            }

            contracted.super_edges = edge_weights
                .into_iter()
                .map(|((a, b), w)| (a, b, w))
                .collect();

            contracted
        }

        pub fn num_super_nodes(&self) -> usize {
            self.super_nodes.len()
        }

        pub fn num_super_edges(&self) -> usize {
            self.super_edges.len()
        }
    }

    /// Mincut computation using Ford-Fulkerson algorithm
    pub struct MincutComputer {
        /// Adjacency list with capacities
        adj: Vec<Vec<(usize, f32)>>,
        pub n: usize,
    }

    impl MincutComputer {
        pub fn from_contracted_graph(contracted: &ContractedGraph) -> Self {
            let n = contracted.num_super_nodes();
            let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];

            for &(a, b, w) in &contracted.super_edges {
                adj[a].push((b, w));
                adj[b].push((a, w));
            }

            Self { adj, n }
        }

        /// Find mincut using BFS-based augmenting paths
        pub fn compute_mincut(&self, source: usize, sink: usize) -> f32 {
            if source == sink || self.n == 0 {
                return 0.0;
            }

            // Create residual capacity matrix
            let mut residual: Vec<Vec<f32>> = vec![vec![0.0; self.n]; self.n];

            for (from, edges) in self.adj.iter().enumerate() {
                for &(to, cap) in edges {
                    residual[from][to] = cap;
                }
            }

            let mut max_flow = 0.0;

            // BFS to find augmenting path
            loop {
                let mut parent = vec![None; self.n];
                let mut visited = vec![false; self.n];
                let mut queue = VecDeque::new();

                visited[source] = true;
                queue.push_back(source);

                while let Some(u) = queue.pop_front() {
                    for v in 0..self.n {
                        if !visited[v] && residual[u][v] > 0.0 {
                            visited[v] = true;
                            parent[v] = Some(u);
                            queue.push_back(v);
                        }
                    }
                }

                if !visited[sink] {
                    break;
                }

                // Find minimum residual capacity along path
                let mut path_flow = f32::MAX;
                let mut v = sink;
                while let Some(u) = parent[v] {
                    path_flow = path_flow.min(residual[u][v]);
                    v = u;
                }

                // Update residual capacities
                v = sink;
                while let Some(u) = parent[v] {
                    residual[u][v] -= path_flow;
                    residual[v][u] += path_flow;
                    v = u;
                }

                max_flow += path_flow;
            }

            max_flow
        }

        /// Compute global mincut (minimum over all pairs)
        pub fn compute_global_mincut(&self) -> f32 {
            if self.n <= 1 {
                return 0.0;
            }

            let mut min_cut = f32::MAX;

            // Use Stoer-Wagner-like approach: fix node 0 as source
            for sink in 1..self.n {
                let cut = self.compute_mincut(0, sink);
                min_cut = min_cut.min(cut);
            }

            min_cut
        }
    }

    /// State machine for index integrity
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IndexState {
        Uninitialized,
        Building,
        Ready,
        Updating,
        Corrupted,
        Recovering,
    }

    pub struct IndexStateMachine {
        pub state: IndexState,
        pub transition_count: usize,
        pub last_integrity_check: std::time::Instant,
        pub integrity_score: f32,
    }

    impl IndexStateMachine {
        pub fn new() -> Self {
            Self {
                state: IndexState::Uninitialized,
                transition_count: 0,
                last_integrity_check: std::time::Instant::now(),
                integrity_score: 1.0,
            }
        }

        pub fn can_transition(&self, to: IndexState) -> bool {
            match (self.state, to) {
                (IndexState::Uninitialized, IndexState::Building) => true,
                (IndexState::Building, IndexState::Ready) => true,
                (IndexState::Ready, IndexState::Updating) => true,
                (IndexState::Updating, IndexState::Ready) => true,
                (_, IndexState::Corrupted) => true,
                (IndexState::Corrupted, IndexState::Recovering) => true,
                (IndexState::Recovering, IndexState::Ready) => true,
                _ => false,
            }
        }

        pub fn transition(&mut self, to: IndexState) -> Result<(), &'static str> {
            if self.can_transition(to) {
                self.state = to;
                self.transition_count += 1;
                Ok(())
            } else {
                Err("Invalid state transition")
            }
        }
    }

    /// Gating check for index operations
    pub struct GatingCheck {
        /// Minimum connectivity threshold
        pub min_connectivity: f32,
        /// Maximum allowed dead nodes
        pub max_dead_nodes_ratio: f32,
        /// Maximum layer imbalance
        pub max_layer_imbalance: f32,
    }

    impl GatingCheck {
        pub fn default() -> Self {
            Self {
                min_connectivity: 0.95,
                max_dead_nodes_ratio: 0.01,
                max_layer_imbalance: 2.0,
            }
        }

        /// Check if graph passes all gates
        pub fn check(&self, graph: &Graph) -> GatingResult {
            let connectivity = self.check_connectivity(graph);
            let dead_ratio = self.check_dead_nodes(graph);
            let layer_balance = self.check_layer_balance(graph);

            GatingResult {
                passed: connectivity >= self.min_connectivity
                    && dead_ratio <= self.max_dead_nodes_ratio
                    && layer_balance <= self.max_layer_imbalance,
                connectivity,
                dead_nodes_ratio: dead_ratio,
                layer_imbalance: layer_balance,
            }
        }

        fn check_connectivity(&self, graph: &Graph) -> f32 {
            if graph.len() <= 1 {
                return 1.0;
            }

            // BFS from first node
            let start = *graph.nodes.keys().next().unwrap();
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();

            visited.insert(start);
            queue.push_back(start);

            while let Some(node) = queue.pop_front() {
                if let Some(n) = graph.nodes.get(&node) {
                    for &neighbor in &n.neighbors {
                        if !visited.contains(&neighbor) && graph.nodes.contains_key(&neighbor) {
                            visited.insert(neighbor);
                            queue.push_back(neighbor);
                        }
                    }
                }
            }

            visited.len() as f32 / graph.len() as f32
        }

        fn check_dead_nodes(&self, graph: &Graph) -> f32 {
            let dead_count = graph
                .nodes
                .values()
                .filter(|n| n.neighbors.is_empty())
                .count();

            dead_count as f32 / graph.len() as f32
        }

        fn check_layer_balance(&self, graph: &Graph) -> f32 {
            if graph.max_layer == 0 {
                return 1.0;
            }

            let mut layer_counts = vec![0usize; graph.max_layer + 1];
            for node in graph.nodes.values() {
                layer_counts[node.layer] += 1;
            }

            let max_count = layer_counts.iter().max().copied().unwrap_or(1) as f32;
            let min_count = layer_counts
                .iter()
                .filter(|&&c| c > 0)
                .min()
                .copied()
                .unwrap_or(1) as f32;

            max_count / min_count
        }
    }

    #[derive(Debug)]
    pub struct GatingResult {
        pub passed: bool,
        pub connectivity: f32,
        pub dead_nodes_ratio: f32,
        pub layer_imbalance: f32,
    }
}

use graph::{ContractedGraph, GatingCheck, Graph, IndexState, IndexStateMachine, MincutComputer};

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_random_graph(n: usize, avg_neighbors: usize, max_layer: usize, seed: u64) -> Graph {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut graph = Graph::new();

    // Add nodes with random layers
    for id in 0..n {
        let layer = if id == 0 {
            max_layer
        } else {
            let ml = 1.0 / (16.0_f64).ln();
            let r: f64 = rng.gen();
            ((-r.ln() * ml).floor() as usize).min(max_layer)
        };
        graph.add_node(id as u64, layer);
    }

    // Add random edges (maintaining HNSW-like structure)
    for id in 0..n {
        let num_neighbors = rng.gen_range(1..=avg_neighbors * 2);
        for _ in 0..num_neighbors {
            let neighbor = rng.gen_range(0..n) as u64;
            if neighbor != id as u64 {
                graph.add_edge(id as u64, neighbor);
            }
        }
    }

    graph
}

fn generate_connected_graph(n: usize, avg_neighbors: usize, seed: u64) -> Graph {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut graph = Graph::new();

    // Add nodes
    for id in 0..n {
        let layer = if id == 0 { 5 } else { rng.gen_range(0..=5) };
        graph.add_node(id as u64, layer);
    }

    // Ensure connectivity: chain all nodes
    for id in 1..n {
        graph.add_edge(id as u64, (id - 1) as u64);
        graph.add_edge((id - 1) as u64, id as u64);
    }

    // Add random extra edges
    for id in 0..n {
        let num_extra = rng.gen_range(0..avg_neighbors);
        for _ in 0..num_extra {
            let neighbor = rng.gen_range(0..n) as u64;
            if neighbor != id as u64 {
                graph.add_edge(id as u64, neighbor);
            }
        }
    }

    graph
}

// ============================================================================
// Contracted Graph Benchmarks
// ============================================================================

fn bench_contracted_graph_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("Contracted Graph Build");
    group.sample_size(10);

    for &n in [1_000, 10_000, 100_000].iter() {
        let graph = generate_connected_graph(n, 16, 42);

        for &factor in [10, 50, 100, 500].iter() {
            if factor > n {
                continue;
            }

            group.bench_with_input(
                BenchmarkId::new(format!("n{}_factor{}", n, factor), n),
                &(&graph, factor),
                |bench, (g, f)| bench.iter(|| black_box(ContractedGraph::build_from_graph(g, *f))),
            );
        }
    }

    group.finish();
}

fn bench_contracted_graph_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("Contracted Graph Memory");
    group.sample_size(10);

    for &n in [10_000, 100_000].iter() {
        let graph = generate_connected_graph(n, 16, 42);

        for &factor in [10, 50, 100].iter() {
            group.bench_with_input(
                BenchmarkId::new(format!("n{}_factor{}", n, factor), n),
                &(&graph, factor),
                |bench, (g, f)| {
                    bench.iter(|| {
                        let contracted = ContractedGraph::build_from_graph(g, *f);

                        // Calculate memory usage
                        let super_node_mem = contracted
                            .super_nodes
                            .iter()
                            .map(|sn| sn.original_nodes.len() * 8)
                            .sum::<usize>();
                        let edge_mem = contracted.super_edges.len() * 20; // (usize, usize, f32)
                        let mapping_mem = contracted.node_mapping.len() * 16;

                        black_box(super_node_mem + edge_mem + mapping_mem)
                    })
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// Mincut Computation Benchmarks
// ============================================================================

fn bench_mincut_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("Mincut Computation");
    group.sample_size(10);

    for &n in [1_000, 5_000, 10_000].iter() {
        let graph = generate_connected_graph(n, 16, 42);
        let contracted = ContractedGraph::build_from_graph(&graph, 50);
        let mincut_computer = MincutComputer::from_contracted_graph(&contracted);

        group.bench_with_input(
            BenchmarkId::new("single_pair", n),
            &mincut_computer,
            |bench, mc| bench.iter(|| black_box(mc.compute_mincut(0, mc.n - 1))),
        );

        group.bench_with_input(
            BenchmarkId::new("global", n),
            &mincut_computer,
            |bench, mc| bench.iter(|| black_box(mc.compute_global_mincut())),
        );
    }

    group.finish();
}

fn bench_mincut_contraction_factors(c: &mut Criterion) {
    let mut group = c.benchmark_group("Mincut vs Contraction Factor");
    group.sample_size(10);

    let n = 10_000;
    let graph = generate_connected_graph(n, 16, 42);

    for &factor in [10, 25, 50, 100, 200].iter() {
        let contracted = ContractedGraph::build_from_graph(&graph, factor);
        let mincut_computer = MincutComputer::from_contracted_graph(&contracted);

        group.bench_with_input(
            BenchmarkId::from_parameter(factor),
            &mincut_computer,
            |bench, mc| bench.iter(|| black_box(mc.compute_global_mincut())),
        );
    }

    group.finish();
}

// ============================================================================
// State Transition Benchmarks
// ============================================================================

fn bench_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("State Transitions");

    // Single transition
    group.bench_function("single_transition", |bench| {
        bench.iter(|| {
            let mut sm = IndexStateMachine::new();
            black_box(sm.transition(IndexState::Building))
        })
    });

    // Full lifecycle
    group.bench_function("full_lifecycle", |bench| {
        bench.iter(|| {
            let mut sm = IndexStateMachine::new();
            sm.transition(IndexState::Building).ok();
            sm.transition(IndexState::Ready).ok();
            sm.transition(IndexState::Updating).ok();
            sm.transition(IndexState::Ready).ok();
            black_box(sm.state)
        })
    });

    // Transition check only (no mutation)
    group.bench_function("transition_check", |bench| {
        let sm = IndexStateMachine::new();
        bench.iter(|| black_box(sm.can_transition(IndexState::Building)))
    });

    // Many transitions
    group.bench_function("1000_transitions", |bench| {
        bench.iter(|| {
            let mut sm = IndexStateMachine::new();
            sm.transition(IndexState::Building).ok();
            sm.transition(IndexState::Ready).ok();

            for _ in 0..500 {
                sm.transition(IndexState::Updating).ok();
                sm.transition(IndexState::Ready).ok();
            }

            black_box(sm.transition_count)
        })
    });

    group.finish();
}

fn bench_state_machine_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("State Machine Overhead");

    // Measure overhead of state checking before operations
    let graph = generate_connected_graph(10_000, 16, 42);

    group.bench_function("with_state_check", |bench| {
        let mut sm = IndexStateMachine::new();
        sm.transition(IndexState::Building).ok();
        sm.transition(IndexState::Ready).ok();

        bench.iter(|| {
            // Simulate operation with state check
            if sm.state == IndexState::Ready {
                // Perform "operation"
                let count = graph.nodes.len();
                black_box(count)
            } else {
                black_box(0)
            }
        })
    });

    group.bench_function("without_state_check", |bench| {
        bench.iter(|| {
            // Perform operation directly
            let count = graph.nodes.len();
            black_box(count)
        })
    });

    group.finish();
}

// ============================================================================
// Gating Check Benchmarks
// ============================================================================

fn bench_gating_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("Gating Check");

    for &n in [1_000, 10_000, 100_000].iter() {
        let graph = generate_connected_graph(n, 16, 42);
        let gating = GatingCheck::default();

        group.bench_with_input(
            BenchmarkId::new("full_check", n),
            &(&graph, &gating),
            |bench, (g, gate)| bench.iter(|| black_box(gate.check(g))),
        );
    }

    group.finish();
}

fn bench_connectivity_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("Connectivity Check");

    for &n in [1_000, 10_000, 100_000].iter() {
        // Well-connected graph
        let connected_graph = generate_connected_graph(n, 16, 42);

        // Sparse graph (may have disconnected components)
        let sparse_graph = generate_random_graph(n, 2, 5, 42);

        let gating = GatingCheck::default();

        group.bench_with_input(
            BenchmarkId::new("connected", n),
            &(&connected_graph, &gating),
            |bench, (g, gate)| bench.iter(|| black_box(gate.check(g).connectivity)),
        );

        group.bench_with_input(
            BenchmarkId::new("sparse", n),
            &(&sparse_graph, &gating),
            |bench, (g, gate)| bench.iter(|| black_box(gate.check(g).connectivity)),
        );
    }

    group.finish();
}

fn bench_dead_node_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("Dead Node Detection");

    for &n in [10_000, 100_000].iter() {
        let graph = generate_connected_graph(n, 16, 42);
        let gating = GatingCheck::default();

        group.bench_with_input(
            BenchmarkId::from_parameter(n),
            &(&graph, &gating),
            |bench, (g, gate)| bench.iter(|| black_box(gate.check(g).dead_nodes_ratio)),
        );
    }

    group.finish();
}

fn bench_layer_balance_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("Layer Balance Check");

    for &n in [10_000, 100_000].iter() {
        let graph = generate_random_graph(n, 16, 10, 42);
        let gating = GatingCheck::default();

        group.bench_with_input(
            BenchmarkId::from_parameter(n),
            &(&graph, &gating),
            |bench, (g, gate)| bench.iter(|| black_box(gate.check(g).layer_imbalance)),
        );
    }

    group.finish();
}

// ============================================================================
// Parallel Integrity Checks
// ============================================================================

fn bench_parallel_integrity(c: &mut Criterion) {
    let mut group = c.benchmark_group("Parallel Integrity Check");
    group.sample_size(10);

    let n = 100_000;
    let graph = generate_connected_graph(n, 16, 42);
    let gating = GatingCheck::default();

    // Sequential checks
    group.bench_function("sequential", |bench| {
        bench.iter(|| {
            let result = gating.check(&graph);
            black_box(result)
        })
    });

    // Parallel checks (connectivity, dead nodes, layer balance)
    group.bench_function("parallel", |bench| {
        bench.iter(|| {
            let (connectivity, (dead_ratio, layer_balance)) = rayon::join(
                || {
                    // Connectivity check
                    if graph.len() <= 1 {
                        return 1.0;
                    }
                    let start = *graph.nodes.keys().next().unwrap();
                    let mut visited = HashSet::new();
                    let mut queue = VecDeque::new();
                    visited.insert(start);
                    queue.push_back(start);
                    while let Some(node) = queue.pop_front() {
                        if let Some(n) = graph.nodes.get(&node) {
                            for &neighbor in &n.neighbors {
                                if !visited.contains(&neighbor)
                                    && graph.nodes.contains_key(&neighbor)
                                {
                                    visited.insert(neighbor);
                                    queue.push_back(neighbor);
                                }
                            }
                        }
                    }
                    visited.len() as f32 / graph.len() as f32
                },
                || {
                    rayon::join(
                        || {
                            // Dead nodes
                            let dead = graph
                                .nodes
                                .values()
                                .filter(|n| n.neighbors.is_empty())
                                .count();
                            dead as f32 / graph.len() as f32
                        },
                        || {
                            // Layer balance
                            let mut layer_counts = vec![0usize; graph.max_layer + 1];
                            for node in graph.nodes.values() {
                                layer_counts[node.layer] += 1;
                            }
                            let max_count = layer_counts.iter().max().copied().unwrap_or(1) as f32;
                            let min_count = layer_counts
                                .iter()
                                .filter(|&&c| c > 0)
                                .min()
                                .copied()
                                .unwrap_or(1) as f32;
                            max_count / min_count
                        },
                    )
                },
            );

            let passed = connectivity >= gating.min_connectivity
                && dead_ratio <= gating.max_dead_nodes_ratio
                && layer_balance <= gating.max_layer_imbalance;

            black_box(passed)
        })
    });

    group.finish();
}

// ============================================================================
// Complete Integrity Pipeline
// ============================================================================

fn bench_full_integrity_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("Full Integrity Pipeline");
    group.sample_size(10);

    for &n in [10_000, 50_000, 100_000].iter() {
        let graph = generate_connected_graph(n, 16, 42);
        let gating = GatingCheck::default();

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| {
                // 1. State check
                let mut sm = IndexStateMachine::new();
                sm.transition(IndexState::Building).ok();
                sm.transition(IndexState::Ready).ok();

                // 2. Gating check
                let gate_result = gating.check(&graph);

                // 3. If passed, build contracted graph
                if gate_result.passed {
                    let contracted = ContractedGraph::build_from_graph(&graph, 100);

                    // 4. Compute mincut
                    let mincut_computer = MincutComputer::from_contracted_graph(&contracted);
                    let mincut = mincut_computer.compute_global_mincut();

                    black_box((gate_result, mincut))
                } else {
                    black_box((gate_result, 0.0))
                }
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    // Contracted Graph
    bench_contracted_graph_build,
    bench_contracted_graph_memory,
    // Mincut
    bench_mincut_compute,
    bench_mincut_contraction_factors,
    // State Transitions
    bench_state_transitions,
    bench_state_machine_overhead,
    // Gating Checks
    bench_gating_check,
    bench_connectivity_check,
    bench_dead_node_detection,
    bench_layer_balance_check,
    // Parallel Integrity
    bench_parallel_integrity,
    // Full Pipeline
    bench_full_integrity_pipeline,
);

criterion_main!(benches);
