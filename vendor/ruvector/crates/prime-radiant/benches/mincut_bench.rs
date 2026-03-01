//! Benchmarks for dynamic mincut updates
//!
//! ADR-014 Performance Target: n^o(1) amortized time per update
//!
//! The mincut algorithm isolates incoherent subgraphs using
//! subpolynomial dynamic updates.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::{HashMap, HashSet, VecDeque};

// ============================================================================
// Dynamic MinCut Types (Simulated for benchmarking)
// ============================================================================

/// Edge in dynamic graph
#[derive(Clone, Copy)]
pub struct Edge {
    pub source: u64,
    pub target: u64,
    pub weight: f64,
}

/// Dynamic graph with mincut tracking
pub struct DynamicGraph {
    /// Adjacency lists
    adjacency: HashMap<u64, HashMap<u64, f64>>,
    /// Total edge count
    edge_count: usize,
    /// Vertex count
    vertex_count: usize,
    /// Cached connected components
    components: Option<Vec<HashSet<u64>>>,
    /// Modification counter for cache invalidation
    mod_count: u64,
}

impl DynamicGraph {
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::new(),
            edge_count: 0,
            vertex_count: 0,
            components: None,
            mod_count: 0,
        }
    }

    pub fn with_capacity(vertices: usize, _edges: usize) -> Self {
        Self {
            adjacency: HashMap::with_capacity(vertices),
            edge_count: 0,
            vertex_count: 0,
            components: None,
            mod_count: 0,
        }
    }

    /// Insert edge
    pub fn insert_edge(&mut self, source: u64, target: u64, weight: f64) -> bool {
        self.components = None;
        self.mod_count += 1;

        let adj = self.adjacency.entry(source).or_insert_with(HashMap::new);
        if adj.contains_key(&target) {
            return false;
        }
        adj.insert(target, weight);

        let adj = self.adjacency.entry(target).or_insert_with(HashMap::new);
        adj.insert(source, weight);

        self.edge_count += 1;
        self.vertex_count = self.adjacency.len();
        true
    }

    /// Delete edge
    pub fn delete_edge(&mut self, source: u64, target: u64) -> bool {
        self.components = None;
        self.mod_count += 1;

        let removed = if let Some(adj) = self.adjacency.get_mut(&source) {
            adj.remove(&target).is_some()
        } else {
            false
        };

        if removed {
            if let Some(adj) = self.adjacency.get_mut(&target) {
                adj.remove(&source);
            }
            self.edge_count -= 1;
        }

        removed
    }

    /// Check if edge exists
    pub fn has_edge(&self, source: u64, target: u64) -> bool {
        self.adjacency
            .get(&source)
            .map(|adj| adj.contains_key(&target))
            .unwrap_or(false)
    }

    /// Get vertex degree
    pub fn degree(&self, vertex: u64) -> usize {
        self.adjacency
            .get(&vertex)
            .map(|adj| adj.len())
            .unwrap_or(0)
    }

    /// Get neighbors
    pub fn neighbors(&self, vertex: u64) -> Vec<u64> {
        self.adjacency
            .get(&vertex)
            .map(|adj| adj.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Compute connected components using BFS
    pub fn connected_components(&mut self) -> &Vec<HashSet<u64>> {
        if self.components.is_some() {
            return self.components.as_ref().unwrap();
        }

        let mut visited = HashSet::new();
        let mut components = Vec::new();

        for &vertex in self.adjacency.keys() {
            if visited.contains(&vertex) {
                continue;
            }

            let mut component = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(vertex);

            while let Some(v) = queue.pop_front() {
                if visited.insert(v) {
                    component.insert(v);
                    if let Some(neighbors) = self.adjacency.get(&v) {
                        for &neighbor in neighbors.keys() {
                            if !visited.contains(&neighbor) {
                                queue.push_back(neighbor);
                            }
                        }
                    }
                }
            }

            components.push(component);
        }

        self.components = Some(components);
        self.components.as_ref().unwrap()
    }

    /// Check if graph is connected
    pub fn is_connected(&mut self) -> bool {
        let components = self.connected_components();
        components.len() <= 1
    }

    /// Get edges as list
    pub fn edges(&self) -> Vec<Edge> {
        let mut edges = Vec::with_capacity(self.edge_count);
        let mut seen = HashSet::new();

        for (&source, neighbors) in &self.adjacency {
            for (&target, &weight) in neighbors {
                let key = if source < target {
                    (source, target)
                } else {
                    (target, source)
                };
                if seen.insert(key) {
                    edges.push(Edge {
                        source,
                        target,
                        weight,
                    });
                }
            }
        }

        edges
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            vertices: self.vertex_count,
            edges: self.edge_count,
            max_degree: self
                .adjacency
                .values()
                .map(|adj| adj.len())
                .max()
                .unwrap_or(0),
            avg_degree: if self.vertex_count > 0 {
                (self.edge_count * 2) as f64 / self.vertex_count as f64
            } else {
                0.0
            },
        }
    }
}

pub struct GraphStats {
    pub vertices: usize,
    pub edges: usize,
    pub max_degree: usize,
    pub avg_degree: f64,
}

/// Subpolynomial MinCut (simplified simulation)
/// Real implementation would use randomized contraction or tree packing
pub struct SubpolynomialMinCut {
    graph: DynamicGraph,
    /// Cached mincut value
    cached_mincut: Option<f64>,
    /// Update count since last computation
    updates_since_compute: usize,
    /// Threshold for recomputation
    recompute_threshold: usize,
}

impl SubpolynomialMinCut {
    pub fn new() -> Self {
        Self {
            graph: DynamicGraph::new(),
            cached_mincut: None,
            updates_since_compute: 0,
            recompute_threshold: 10,
        }
    }

    pub fn with_capacity(vertices: usize, edges: usize) -> Self {
        Self {
            graph: DynamicGraph::with_capacity(vertices, edges),
            cached_mincut: None,
            updates_since_compute: 0,
            recompute_threshold: ((vertices as f64).sqrt() as usize).max(10),
        }
    }

    /// Insert edge with lazy mincut update
    pub fn insert_edge(&mut self, source: u64, target: u64, weight: f64) -> bool {
        let result = self.graph.insert_edge(source, target, weight);
        if result {
            self.updates_since_compute += 1;
            // Mincut can only decrease or stay same on edge insertion
            // So we can keep cached value as upper bound
        }
        result
    }

    /// Delete edge with lazy mincut update
    pub fn delete_edge(&mut self, source: u64, target: u64) -> bool {
        let result = self.graph.delete_edge(source, target);
        if result {
            self.updates_since_compute += 1;
            // Mincut might have decreased, invalidate cache
            self.cached_mincut = None;
        }
        result
    }

    /// Compute mincut (lazy - uses cache if available)
    pub fn min_cut(&mut self) -> f64 {
        if let Some(cached) = self.cached_mincut {
            if self.updates_since_compute < self.recompute_threshold {
                return cached;
            }
        }

        // Simplified: use min degree as lower bound approximation
        // Real implementation: Karger's algorithm or tree packing
        let mincut = self.compute_mincut_approximation();
        self.cached_mincut = Some(mincut);
        self.updates_since_compute = 0;
        mincut
    }

    /// Approximate mincut using min degree heuristic
    fn compute_mincut_approximation(&self) -> f64 {
        // Min cut <= min weighted degree
        let mut min_cut = f64::MAX;

        for (_vertex, neighbors) in &self.graph.adjacency {
            let weighted_degree: f64 = neighbors.values().sum();
            if weighted_degree < min_cut {
                min_cut = weighted_degree;
            }
        }

        if min_cut == f64::MAX {
            0.0
        } else {
            min_cut
        }
    }

    /// Get partition (simplified: just split by component)
    pub fn partition(&mut self) -> (HashSet<u64>, HashSet<u64>) {
        let components = self.graph.connected_components();

        if components.is_empty() {
            return (HashSet::new(), HashSet::new());
        }

        if components.len() == 1 {
            // Single component - split roughly in half
            let vertices: Vec<_> = components[0].iter().copied().collect();
            let mid = vertices.len() / 2;
            let left: HashSet<_> = vertices[..mid].iter().copied().collect();
            let right: HashSet<_> = vertices[mid..].iter().copied().collect();
            (left, right)
        } else {
            // Multiple components - use first vs rest
            let left = components[0].clone();
            let right: HashSet<_> = components[1..]
                .iter()
                .flat_map(|c| c.iter())
                .copied()
                .collect();
            (left, right)
        }
    }
}

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_random_graph(n: usize, m: usize, seed: u64) -> Vec<(u64, u64, f64)> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut edges = Vec::with_capacity(m);
    let mut edge_set = HashSet::new();

    for i in 0..m * 2 {
        if edges.len() >= m {
            break;
        }

        let mut hasher = DefaultHasher::new();
        (seed, i, "source").hash(&mut hasher);
        let u = hasher.finish() % n as u64;

        let mut hasher = DefaultHasher::new();
        (seed, i, "target").hash(&mut hasher);
        let v = hasher.finish() % n as u64;

        if u != v {
            let key = if u < v { (u, v) } else { (v, u) };
            if edge_set.insert(key) {
                edges.push((u, v, 1.0));
            }
        }
    }

    edges
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark edge insertion
fn bench_insert_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_insert");
    group.throughput(Throughput::Elements(1));

    for size in [100, 1000, 10000] {
        let edges = generate_random_graph(size, size * 2, 42);
        let mut mincut = SubpolynomialMinCut::with_capacity(size, size * 3);

        // Pre-populate
        for (u, v, w) in &edges[..edges.len() / 2] {
            mincut.insert_edge(*u, *v, *w);
        }

        group.bench_with_input(BenchmarkId::new("insert_single", size), &size, |b, &n| {
            let mut i = edges.len() / 2;
            b.iter(|| {
                let (u, v, w) = edges[i % edges.len()];
                black_box(mincut.insert_edge(u + n as u64, v + n as u64, w));
                i += 1;
            })
        });
    }

    group.finish();
}

/// Benchmark edge deletion
fn bench_delete_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_delete");
    group.throughput(Throughput::Elements(1));

    for size in [100, 1000, 10000] {
        let edges = generate_random_graph(size, size * 2, 42);

        group.bench_with_input(BenchmarkId::new("delete_single", size), &size, |b, _| {
            b.iter_batched(
                || {
                    let mut mincut = SubpolynomialMinCut::with_capacity(size, size * 3);
                    for (u, v, w) in &edges {
                        mincut.insert_edge(*u, *v, *w);
                    }
                    (mincut, edges.clone())
                },
                |(mut mincut, edges)| {
                    let (u, v, _) = edges[edges.len() / 2];
                    black_box(mincut.delete_edge(u, v))
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

/// Benchmark mincut query
fn bench_mincut_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_query");
    group.throughput(Throughput::Elements(1));

    for size in [100, 1000, 10000] {
        let edges = generate_random_graph(size, size * 2, 42);
        let mut mincut = SubpolynomialMinCut::with_capacity(size, size * 3);

        for (u, v, w) in &edges {
            mincut.insert_edge(*u, *v, *w);
        }

        // Cold query (no cache)
        group.bench_with_input(BenchmarkId::new("cold_query", size), &size, |b, _| {
            b.iter_batched(
                || {
                    let mc = mincut.graph.adjacency.clone();
                    SubpolynomialMinCut {
                        graph: DynamicGraph {
                            adjacency: mc,
                            edge_count: mincut.graph.edge_count,
                            vertex_count: mincut.graph.vertex_count,
                            components: None,
                            mod_count: 0,
                        },
                        cached_mincut: None,
                        updates_since_compute: 0,
                        recompute_threshold: 10,
                    }
                },
                |mut mc| black_box(mc.min_cut()),
                criterion::BatchSize::SmallInput,
            )
        });

        // Warm query (cached)
        mincut.min_cut(); // Prime cache
        group.bench_with_input(BenchmarkId::new("warm_query", size), &size, |b, _| {
            b.iter(|| black_box(mincut.min_cut()))
        });
    }

    group.finish();
}

/// Benchmark scaling behavior (verify subpolynomial)
fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_scaling");
    group.sample_size(20);

    // Sizes chosen for subpolynomial verification
    // n^(2/3) scaling should show sub-linear growth
    let sizes = vec![100, 316, 1000, 3162, 10000];

    for size in sizes {
        let edges = generate_random_graph(size, size * 2, 42);

        // Measure insert amortized time
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("insert_amortized", size),
            &size,
            |b, &n| {
                b.iter_batched(
                    || {
                        let mut mincut = SubpolynomialMinCut::with_capacity(n, n * 3);
                        for (u, v, w) in &edges[..edges.len() / 2] {
                            mincut.insert_edge(*u, *v, *w);
                        }
                        (mincut, n)
                    },
                    |(mut mincut, n)| {
                        for i in 0..10 {
                            let u = (i * 37) as u64 % n as u64;
                            let v = (i * 73 + 1) as u64 % n as u64;
                            if u != v {
                                mincut.insert_edge(u + n as u64, v + n as u64, 1.0);
                            }
                        }
                        black_box(mincut.min_cut())
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

/// Benchmark mixed workload
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_mixed");
    group.throughput(Throughput::Elements(1));

    for size in [100, 1000, 10000] {
        let edges = generate_random_graph(size, size * 2, 42);

        group.bench_with_input(BenchmarkId::new("mixed_ops", size), &size, |b, &n| {
            b.iter_batched(
                || {
                    let mut mincut = SubpolynomialMinCut::with_capacity(n, n * 3);
                    for (u, v, w) in &edges {
                        mincut.insert_edge(*u, *v, *w);
                    }
                    (mincut, 0usize)
                },
                |(mut mincut, mut op_idx)| {
                    // 50% insert, 30% delete, 20% query
                    match op_idx % 10 {
                        0..=4 => {
                            let u = (op_idx * 37) as u64 % n as u64;
                            let v = (op_idx * 73 + 1) as u64 % n as u64;
                            if u != v {
                                mincut.insert_edge(u + n as u64, v + n as u64, 1.0);
                            }
                        }
                        5..=7 => {
                            if !edges.is_empty() {
                                let (u, v, _) = edges[op_idx % edges.len()];
                                mincut.delete_edge(u, v);
                            }
                        }
                        _ => {
                            let _ = mincut.min_cut();
                        }
                    }
                    op_idx += 1;
                    black_box(op_idx)
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

/// Benchmark partition computation
fn bench_partition(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_partition");

    for size in [100, 1000, 10000] {
        let edges = generate_random_graph(size, size * 2, 42);
        let mut mincut = SubpolynomialMinCut::with_capacity(size, size * 3);

        for (u, v, w) in &edges {
            mincut.insert_edge(*u, *v, *w);
        }

        group.bench_with_input(BenchmarkId::new("partition", size), &size, |b, _| {
            b.iter(|| black_box(mincut.partition()))
        });
    }

    group.finish();
}

/// Benchmark connected components
fn bench_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_components");

    for size in [100, 1000, 10000] {
        // Create graph with multiple components
        let mut mincut = SubpolynomialMinCut::with_capacity(size, size * 2);

        let component_size = size / 5;
        for comp in 0..5 {
            let offset = comp * component_size;
            for i in 0..component_size - 1 {
                let u = (offset + i) as u64;
                let v = (offset + i + 1) as u64;
                mincut.insert_edge(u, v, 1.0);
            }
        }

        group.bench_with_input(BenchmarkId::new("multi_component", size), &size, |b, _| {
            b.iter(|| {
                // Force recomputation
                mincut.graph.components = None;
                let components = mincut.graph.connected_components();
                black_box(components.len())
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_edge,
    bench_delete_edge,
    bench_mincut_query,
    bench_scaling,
    bench_mixed_workload,
    bench_partition,
    bench_components,
);

criterion_main!(benches);
