//! Benchmarks for 256-tile parallel tick
//!
//! ADR-014 Performance Target: < 1ms for 256-tile parallel tick
//!
//! The cognitum-gate-kernel provides 256 WASM tiles, each maintaining
//! a local graph shard with E-value accumulation and witness fragments.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// Tile Types (Simulated, matching cognitum-gate-kernel structure)
// ============================================================================

/// Maximum delta buffer per tile
pub const MAX_DELTA_BUFFER: usize = 64;
/// Number of tiles in fabric
pub const NUM_TILES: usize = 256;
/// Maximum vertices per shard
pub const MAX_SHARD_VERTICES: usize = 256;
/// Maximum edges per shard
pub const MAX_SHARD_EDGES: usize = 1024;

/// Delta operation type
#[derive(Clone, Copy)]
pub enum DeltaType {
    EdgeAdd,
    EdgeRemove,
    Observation,
    WeightUpdate,
}

/// Delta (change event) for tile
#[derive(Clone, Copy)]
pub struct Delta {
    pub delta_type: DeltaType,
    pub source: u16,
    pub target: u16,
    pub weight: u16,
    pub payload: u32,
}

impl Delta {
    pub fn edge_add(src: u16, tgt: u16, weight: u16) -> Self {
        Self {
            delta_type: DeltaType::EdgeAdd,
            source: src,
            target: tgt,
            weight,
            payload: 0,
        }
    }

    pub fn observation(vertex: u16, positive: bool) -> Self {
        Self {
            delta_type: DeltaType::Observation,
            source: vertex,
            target: 0,
            weight: 0,
            payload: positive as u32,
        }
    }
}

/// Compact vertex state
#[derive(Clone, Copy, Default)]
pub struct VertexState {
    pub degree: u8,
    pub component_id: u8,
    pub active: bool,
    pub energy_contrib: f32,
}

impl VertexState {
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Compact edge
#[derive(Clone, Copy, Default)]
pub struct CompactEdge {
    pub source: u16,
    pub target: u16,
    pub weight: u16,
    pub active: bool,
}

impl CompactEdge {
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Compact graph for single tile
pub struct CompactGraph {
    pub vertices: [VertexState; MAX_SHARD_VERTICES],
    pub edges: [CompactEdge; MAX_SHARD_EDGES],
    pub edge_count: usize,
    pub vertex_count: usize,
    pub component_count: u8,
}

impl CompactGraph {
    pub fn new() -> Self {
        Self {
            vertices: [VertexState::default(); MAX_SHARD_VERTICES],
            edges: [CompactEdge::default(); MAX_SHARD_EDGES],
            edge_count: 0,
            vertex_count: 0,
            component_count: 0,
        }
    }

    pub fn add_edge(&mut self, src: u16, tgt: u16, weight: u16) -> bool {
        if self.edge_count >= MAX_SHARD_EDGES {
            return false;
        }

        // Activate vertices
        self.vertices[src as usize].active = true;
        self.vertices[src as usize].degree += 1;
        self.vertices[tgt as usize].active = true;
        self.vertices[tgt as usize].degree += 1;

        // Add edge
        self.edges[self.edge_count] = CompactEdge {
            source: src,
            target: tgt,
            weight,
            active: true,
        };
        self.edge_count += 1;

        true
    }

    pub fn recompute_components(&mut self) {
        // Simple union-find simulation
        let mut parent = [0u8; MAX_SHARD_VERTICES];
        for i in 0..MAX_SHARD_VERTICES {
            parent[i] = i as u8;
        }

        // Union edges
        for edge in &self.edges[..self.edge_count] {
            if edge.active {
                let s = edge.source as usize;
                let t = edge.target as usize;
                parent[s] = parent[t];
            }
        }

        // Count unique components
        let mut seen = [false; MAX_SHARD_VERTICES];
        let mut count = 0u8;
        for i in 0..MAX_SHARD_VERTICES {
            if self.vertices[i].active && !seen[parent[i] as usize] {
                seen[parent[i] as usize] = true;
                count += 1;
            }
        }
        self.component_count = count;
    }

    pub fn compute_total_energy(&self) -> f32 {
        let mut energy = 0.0f32;
        for edge in &self.edges[..self.edge_count] {
            if edge.active {
                // Simplified: weight as energy contribution
                energy += edge.weight as f32 / 100.0;
            }
        }
        energy
    }
}

/// E-value accumulator (log-space evidence)
pub struct EvidenceAccumulator {
    /// Log e-value (fixed-point: value / 65536 = log2(e-value))
    pub log_e_values: Vec<i32>,
    pub hypothesis_count: usize,
}

impl EvidenceAccumulator {
    pub fn new(capacity: usize) -> Self {
        Self {
            log_e_values: vec![0; capacity],
            hypothesis_count: 0,
        }
    }

    pub fn add_hypothesis(&mut self) -> usize {
        let idx = self.hypothesis_count;
        if idx < self.log_e_values.len() {
            self.hypothesis_count += 1;
        }
        idx
    }

    #[inline]
    pub fn update(&mut self, idx: usize, log_lr: i32) {
        if idx < self.hypothesis_count {
            self.log_e_values[idx] = self.log_e_values[idx].saturating_add(log_lr);
        }
    }

    pub fn global_log_e(&self) -> i64 {
        self.log_e_values[..self.hypothesis_count]
            .iter()
            .map(|&v| v as i64)
            .sum()
    }
}

/// Tile report (output of tick)
#[derive(Clone, Copy)]
pub struct TileReport {
    pub tile_id: u8,
    pub tick: u32,
    pub connected: bool,
    pub component_count: u8,
    pub log_e_value: i64,
    pub energy: f32,
    pub witness_hash: u64,
}

impl TileReport {
    pub fn new(tile_id: u8) -> Self {
        Self {
            tile_id,
            tick: 0,
            connected: true,
            component_count: 1,
            log_e_value: 0,
            energy: 0.0,
            witness_hash: 0,
        }
    }
}

/// Single tile state
pub struct TileState {
    pub tile_id: u8,
    pub graph: CompactGraph,
    pub evidence: EvidenceAccumulator,
    pub delta_buffer: Vec<Delta>,
    pub tick_count: u32,
}

impl TileState {
    pub fn new(tile_id: u8) -> Self {
        Self {
            tile_id,
            graph: CompactGraph::new(),
            evidence: EvidenceAccumulator::new(64),
            delta_buffer: Vec::with_capacity(MAX_DELTA_BUFFER),
            tick_count: 0,
        }
    }

    pub fn ingest_delta(&mut self, delta: &Delta) -> bool {
        if self.delta_buffer.len() >= MAX_DELTA_BUFFER {
            return false;
        }
        self.delta_buffer.push(*delta);
        true
    }

    pub fn tick(&mut self, tick_number: u32) -> TileReport {
        // Process pending deltas
        for delta in self.delta_buffer.drain(..) {
            match delta.delta_type {
                DeltaType::EdgeAdd => {
                    self.graph
                        .add_edge(delta.source, delta.target, delta.weight);
                }
                DeltaType::Observation => {
                    // Update evidence accumulator
                    let log_lr = if delta.payload != 0 { 65536 } else { -65536 };
                    if self.evidence.hypothesis_count > 0 {
                        self.evidence.update(0, log_lr);
                    }
                }
                _ => {}
            }
        }

        // Recompute components if needed
        self.graph.recompute_components();

        // Compute energy
        let energy = self.graph.compute_total_energy();

        // Build report
        self.tick_count = tick_number;
        TileReport {
            tile_id: self.tile_id,
            tick: tick_number,
            connected: self.graph.component_count <= 1,
            component_count: self.graph.component_count,
            log_e_value: self.evidence.global_log_e(),
            energy,
            witness_hash: self.compute_witness_hash(),
        }
    }

    fn compute_witness_hash(&self) -> u64 {
        let mut hash = self.tile_id as u64;
        hash = hash.wrapping_mul(0x517cc1b727220a95);
        hash ^= self.tick_count as u64;
        hash = hash.wrapping_mul(0x517cc1b727220a95);
        hash ^= self.graph.edge_count as u64;
        hash
    }

    pub fn reset(&mut self) {
        self.graph = CompactGraph::new();
        self.delta_buffer.clear();
        self.tick_count = 0;
    }
}

/// 256-tile coherence fabric
pub struct CoherenceFabric {
    pub tiles: Vec<TileState>,
}

impl CoherenceFabric {
    pub fn new() -> Self {
        Self {
            tiles: (0..NUM_TILES).map(|i| TileState::new(i as u8)).collect(),
        }
    }

    /// Execute tick on all tiles sequentially
    pub fn tick_sequential(&mut self, tick_number: u32) -> Vec<TileReport> {
        self.tiles.iter_mut().map(|t| t.tick(tick_number)).collect()
    }

    /// Aggregate reports into global coherence
    pub fn aggregate_reports(reports: &[TileReport]) -> FabricReport {
        let total_energy: f32 = reports.iter().map(|r| r.energy).sum();
        let total_log_e: i64 = reports.iter().map(|r| r.log_e_value).sum();
        let all_connected = reports.iter().all(|r| r.connected);

        // Compute global witness hash
        let mut global_hash = 0u64;
        for r in reports {
            global_hash = global_hash.wrapping_mul(0x517cc1b727220a95);
            global_hash ^= r.witness_hash;
        }

        FabricReport {
            tick: reports.first().map(|r| r.tick).unwrap_or(0),
            total_energy,
            total_log_e,
            all_connected,
            global_witness_hash: global_hash,
        }
    }

    /// Distribute delta to appropriate tile
    pub fn distribute_delta(&mut self, node_id: u64, delta: &Delta) {
        let tile_id = (node_id % NUM_TILES as u64) as usize;
        self.tiles[tile_id].ingest_delta(delta);
    }
}

/// Aggregated fabric report
pub struct FabricReport {
    pub tick: u32,
    pub total_energy: f32,
    pub total_log_e: i64,
    pub all_connected: bool,
    pub global_witness_hash: u64,
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark single tile tick
fn bench_single_tile_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_single_tick");
    group.throughput(Throughput::Elements(1));

    // Empty tick
    let mut tile = TileState::new(0);
    group.bench_function("empty", |b| b.iter(|| black_box(tile.tick(black_box(1)))));

    // Tick with small graph
    let mut tile = TileState::new(0);
    for i in 0..20u16 {
        tile.ingest_delta(&Delta::edge_add(i, i + 1, 100));
    }
    tile.tick(0);

    group.bench_function("small_graph_20_edges", |b| {
        b.iter(|| black_box(tile.tick(black_box(1))))
    });

    // Tick with pending deltas
    group.bench_function("with_10_deltas", |b| {
        b.iter_batched(
            || {
                let mut t = TileState::new(0);
                for i in 0..10u16 {
                    t.ingest_delta(&Delta::edge_add(i, i + 1, 100));
                }
                t
            },
            |mut t| black_box(t.tick(1)),
            criterion::BatchSize::SmallInput,
        )
    });

    // Tick with full delta buffer
    group.bench_function("with_64_deltas", |b| {
        b.iter_batched(
            || {
                let mut t = TileState::new(0);
                for i in 0..MAX_DELTA_BUFFER as u16 {
                    t.ingest_delta(&Delta::edge_add(i % 200, (i + 1) % 200, 100));
                }
                t
            },
            |mut t| black_box(t.tick(1)),
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// Benchmark 256-tile parallel tick (sequential baseline)
fn bench_256_tile_tick_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_256_sequential");
    group.throughput(Throughput::Elements(NUM_TILES as u64));

    // Empty fabric
    let mut fabric = CoherenceFabric::new();
    group.bench_function("empty_fabric", |b| {
        b.iter(|| black_box(fabric.tick_sequential(black_box(1))))
    });

    // Fabric with some data per tile
    let mut fabric = CoherenceFabric::new();
    for i in 0..NUM_TILES {
        for j in 0..10u16 {
            fabric.tiles[i].ingest_delta(&Delta::edge_add(j, j + 1, 100));
        }
        fabric.tiles[i].tick(0);
    }

    group.bench_function("populated_10_edges_per_tile", |b| {
        b.iter(|| black_box(fabric.tick_sequential(black_box(1))))
    });

    group.finish();
}

/// Benchmark report aggregation
fn bench_report_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_report_aggregation");
    group.throughput(Throughput::Elements(NUM_TILES as u64));

    // Generate 256 reports
    let reports: Vec<TileReport> = (0..NUM_TILES)
        .map(|i| TileReport {
            tile_id: i as u8,
            tick: 1,
            connected: i % 10 != 0,
            component_count: (i % 5) as u8 + 1,
            log_e_value: (i as i64) * 1000 - 128000,
            energy: (i as f32) * 0.1,
            witness_hash: i as u64 * 0x517cc1b727220a95,
        })
        .collect();

    group.bench_function("aggregate_256_reports", |b| {
        b.iter(|| black_box(CoherenceFabric::aggregate_reports(black_box(&reports))))
    });

    group.finish();
}

/// Benchmark delta distribution
fn bench_delta_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_delta_distribution");

    let mut fabric = CoherenceFabric::new();

    // Single delta
    let delta = Delta::edge_add(0, 1, 100);
    group.bench_function("distribute_single", |b| {
        b.iter(|| fabric.distribute_delta(black_box(12345), black_box(&delta)))
    });

    // Batch distribution
    for batch_size in [100, 1000, 10000] {
        let deltas: Vec<(u64, Delta)> = (0..batch_size)
            .map(|i| {
                (
                    i as u64,
                    Delta::edge_add((i % 200) as u16, ((i + 1) % 200) as u16, 100),
                )
            })
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("distribute_batch", batch_size),
            &deltas,
            |b, deltas| {
                b.iter(|| {
                    for (node_id, delta) in deltas {
                        fabric.distribute_delta(*node_id, delta);
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark evidence accumulator
fn bench_evidence_accumulator(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_evidence");

    let mut acc = EvidenceAccumulator::new(64);
    for _ in 0..16 {
        acc.add_hypothesis();
    }

    // Single update
    group.bench_function("update_single", |b| {
        b.iter(|| acc.update(black_box(5), black_box(65536)))
    });

    // Global e-value computation
    group.bench_function("global_log_e_16_hyp", |b| {
        b.iter(|| black_box(acc.global_log_e()))
    });

    // 64 hypotheses
    let mut acc64 = EvidenceAccumulator::new(64);
    for _ in 0..64 {
        acc64.add_hypothesis();
    }
    for i in 0..64 {
        acc64.log_e_values[i] = (i as i32 - 32) * 1000;
    }

    group.bench_function("global_log_e_64_hyp", |b| {
        b.iter(|| black_box(acc64.global_log_e()))
    });

    group.finish();
}

/// Benchmark component recomputation
fn bench_component_recompute(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_component_recompute");

    for edge_count in [50, 200, 500, 1000] {
        let mut graph = CompactGraph::new();
        for i in 0..edge_count.min(MAX_SHARD_EDGES) {
            let src = (i % 200) as u16;
            let tgt = ((i + 1) % 200) as u16;
            if src != tgt {
                graph.add_edge(src, tgt, 100);
            }
        }

        group.bench_with_input(
            BenchmarkId::new("recompute", edge_count),
            &edge_count,
            |b, _| {
                b.iter(|| {
                    graph.recompute_components();
                    black_box(graph.component_count)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark full tick + aggregate cycle
fn bench_full_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_full_cycle");
    group.sample_size(50);

    // Populate fabric
    let mut fabric = CoherenceFabric::new();
    for i in 0..NUM_TILES {
        for j in 0..50u16 {
            fabric.tiles[i].ingest_delta(&Delta::edge_add(j, (j + 1) % 200, 100));
        }
        fabric.tiles[i].tick(0);
    }

    group.bench_function("tick_and_aggregate_256_tiles", |b| {
        let mut tick = 1u32;
        b.iter(|| {
            let reports = fabric.tick_sequential(tick);
            let fabric_report = CoherenceFabric::aggregate_reports(&reports);
            tick += 1;
            black_box(fabric_report)
        })
    });

    group.finish();
}

/// Benchmark memory access patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_memory");

    // Sequential tile access
    let fabric = CoherenceFabric::new();
    group.bench_function("sequential_tile_scan", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for tile in &fabric.tiles {
                total += tile.graph.edge_count;
            }
            black_box(total)
        })
    });

    // Strided tile access
    group.bench_function("strided_tile_scan", |b| {
        let stride = 7;
        b.iter(|| {
            let mut total = 0usize;
            let mut idx = 0;
            for _ in 0..NUM_TILES {
                total += fabric.tiles[idx % NUM_TILES].graph.edge_count;
                idx += stride;
            }
            black_box(total)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_tile_tick,
    bench_256_tile_tick_sequential,
    bench_report_aggregation,
    bench_delta_distribution,
    bench_evidence_accumulator,
    bench_component_recompute,
    bench_full_cycle,
    bench_memory_patterns,
);

criterion_main!(benches);
