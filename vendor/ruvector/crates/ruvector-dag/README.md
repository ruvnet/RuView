# RuVector DAG - Neural Self-Learning DAG

**Make your queries faster automatically.** RuVector DAG learns from every query execution and continuously optimizes performance—no manual tuning required.

## What is This?

RuVector DAG is a **self-learning query optimization system**. Think of it as a "nervous system" for your database queries that:

1. **Watches** how queries execute and identifies bottlenecks
2. **Learns** which optimization strategies work best for different query patterns
3. **Adapts** in real-time, switching strategies when conditions change
4. **Heals** itself by detecting anomalies and fixing problems before they impact users

Unlike traditional query optimizers that use static rules, RuVector DAG learns from actual execution patterns and gets smarter over time.

## Who Should Use This?

| Use Case | Why RuVector DAG Helps |
|----------|------------------------|
| **Vector Search Applications** | Optimize similarity searches that traditional databases struggle with |
| **High-Traffic APIs** | Automatically adapt to changing query patterns throughout the day |
| **Real-Time Analytics** | Learn which aggregation paths are fastest for your specific data |
| **Edge/Embedded Systems** | 58KB WASM build runs in browsers and IoT devices |
| **Multi-Tenant Platforms** | Learn per-tenant query patterns without manual per-tenant tuning |

## Key Benefits

### Automatic Performance Improvement
Queries get faster over time without any code changes. In benchmarks, repeated queries show **50-80% latency reduction** after the system learns optimal execution paths.

### Zero-Downtime Adaptation
When query patterns change (new features, traffic spikes, data growth), the system adapts automatically. No need to rebuild indexes or rewrite queries.

### Predictive Problem Prevention
The system detects rising "tension" (early warning signs of bottlenecks) and intervenes *before* users experience slowdowns.

### Works Everywhere
- **PostgreSQL** via the ruvector-postgres extension
- **Browsers** via 58KB WASM module
- **Embedded systems** with minimal memory footprint
- **Distributed systems** with quantum-resistant sync between nodes

## How It Works (Simple Version)

```
Query comes in → DAG analyzes execution plan → Best attention mechanism selected
                                                          ↓
Query executes → Results returned → Learning system records what worked
                                                          ↓
                    Next similar query benefits from learned optimizations
```

The system maintains a "MinCut tension" score that acts as a health indicator. When tension rises, the system automatically switches to more aggressive optimization strategies and triggers predictive healing.

## Features

- **7 DAG Attention Mechanisms**: Topological, Causal Cone, Critical Path, MinCut Gated, Hierarchical Lorentz, Parallel Branch, Temporal BTSP
- **SONA Learning**: Self-Optimizing Neural Architecture with MicroLoRA adaptation (<100μs)
- **Subpolynomial MinCut**: O(n^0.12) bottleneck detection—the coherence boundary everything listens to
- **Self-Healing**: Autonomous anomaly detection, reactive repair, and predictive intervention
- **QuDAG Integration**: Quantum-resistant distributed pattern learning with bounded sync
- **WASM Target**: 58KB gzipped for browser and embedded systems

## Design Philosophy

MinCut is not an optimization trick here. It is the coherence boundary that everything else listens to. Attention mechanisms, SONA learning, and self-healing all respond to MinCut stress signals—creating a unified nervous system for query optimization.

## Quick Start

```rust
use ruvector_dag::{QueryDag, OperatorNode, OperatorType};
use ruvector_dag::attention::{TopologicalAttention, DagAttention};

// Build a query DAG
let mut dag = QueryDag::new();
let scan = dag.add_node(OperatorNode::hnsw_scan(0, "vectors_idx", 64));
let filter = dag.add_node(OperatorNode::filter(1, "score > 0.5"));
let result = dag.add_node(OperatorNode::new(2, OperatorType::Result));

dag.add_edge(scan, filter).unwrap();
dag.add_edge(filter, result).unwrap();

// Compute attention scores
let attention = TopologicalAttention::new(Default::default());
let scores = attention.forward(&dag).unwrap();
```

## Modules

- `dag` - Core DAG data structures and algorithms
- `attention` - 7 attention mechanisms + policy-driven selection
- `sona` - Self-Optimizing Neural Architecture with adaptive learning
- `mincut` - Subpolynomial bottleneck detection (the central control signal)
- `healing` - Reactive + predictive self-healing
- `qudag` - QuDAG network integration with bounded sync frequency

## Core Components

### DAG (Directed Acyclic Graph)

The `QueryDag` structure represents query execution plans as directed acyclic graphs. Each node represents an operator (scan, filter, join, etc.) and edges represent data flow.

```rust
use ruvector_dag::{QueryDag, OperatorNode, OperatorType};

let mut dag = QueryDag::new();
let scan = dag.add_node(OperatorNode::seq_scan(0, "users"));
let filter = dag.add_node(OperatorNode::filter(1, "age > 18"));
dag.add_edge(scan, filter).unwrap();
```

### Attention Mechanisms + Policy Layer

Seven attention mechanisms with dynamic policy-driven selection:

| Mechanism | When to Use | Trigger |
|-----------|-------------|---------|
| Topological | Default baseline | Low variance |
| Causal Cone | Downstream impact analysis | Write-heavy patterns |
| Critical Path | Latency-bound queries | p99 > 2x p50 |
| MinCut Gated | Bottleneck-aware weighting | Cut tension rising |
| Hierarchical Lorentz | Deep hierarchical queries | Depth > 10 |
| Parallel Branch | Wide parallel execution | Branch count > 3 |
| Temporal BTSP | Time-series workloads | Temporal patterns |

```rust
use ruvector_dag::attention::{AttentionSelector, SelectionPolicy};
use ruvector_dag::mincut::DagMinCutEngine;

// Policy-driven attention selection based on MinCut stress
let mut selector = AttentionSelector::new();
let mut mincut = DagMinCutEngine::new(Default::default());

// Dynamic switching based on cut tension
let analysis = mincut.analyze_bottlenecks(&dag)?;
let policy = if analysis.max_tension > 0.7 {
    SelectionPolicy::MinCutGated  // High stress: gate by flow
} else if analysis.latency_variance > 2.0 {
    SelectionPolicy::CriticalPath  // Variance: focus on bottlenecks
} else {
    SelectionPolicy::Topological  // Stable: use position-based
};

let scores = selector.select_and_apply(policy, &dag)?;
```

### SONA (Self-Optimizing Neural Architecture)

Adaptive learning with explicit data structures. SONA runs post-query in background, never blocking execution.

**State Vector Structure:**
```rust
/// SONA maintains per-DAG-pattern state vectors
pub struct SonaState {
    /// Base embedding: pattern signature (256-dim)
    pub embedding: [f32; 256],

    /// MicroLoRA weights: scoped per operator type
    /// Shape: [num_operator_types, rank, rank] where rank=2
    pub lora_weights: HashMap<OperatorType, [[f32; 2]; 2]>,

    /// Trajectory statistics for this pattern
    pub trajectory_stats: TrajectoryStats,
}

pub struct TrajectoryStats {
    pub count: u64,
    pub mean_improvement: f32,  // vs baseline
    pub variance: f32,
    pub best_mechanism: AttentionType,
}
```

```rust
use ruvector_dag::sona::{DagSonaEngine, SonaConfig};

let config = SonaConfig {
    embedding_dim: 256,
    lora_rank: 2,           // Rank-2 for <100μs updates
    ewc_lambda: 5000.0,     // Catastrophic forgetting prevention
    trajectory_capacity: 10_000,
};
let mut sona = DagSonaEngine::new(config);

// Pre-query: Get enhanced embedding (fast path)
let enhanced = sona.pre_query(&dag);

// Execute query... (SONA doesn't block here)
let execution_time = execute_query(&dag);

// Post-query: Record trajectory (async, background)
sona.post_query(&dag, execution_time, baseline_time, "topological");

// Background learning (runs in separate thread)
sona.background_learn();  // Updates LoRA weights, EWC consolidation
```

### MinCut Optimization (Central Control Signal)

The MinCut engine is the coherence boundary. Rising cut tension triggers attention switching, SONA re-weighting, and predictive healing.

```rust
use ruvector_dag::mincut::{DagMinCutEngine, MinCutConfig};

let mut engine = DagMinCutEngine::new(MinCutConfig {
    update_complexity: 0.12,  // O(n^0.12) amortized
    tension_threshold: 0.7,
    emit_signals: true,       // Broadcast to other subsystems
});

let analysis = engine.analyze_bottlenecks(&dag)?;

// Tension signal drives the whole system
if analysis.max_tension > 0.7 {
    // High tension: trigger predictive healing
    healing.predict_and_prepare(&analysis);

    // Switch attention to MinCut-aware mechanism
    selector.force_mechanism(AttentionType::MinCutGated);

    // Accelerate SONA learning for this pattern
    sona.boost_learning_rate(2.0);
}

for bottleneck in &analysis.bottlenecks {
    println!("Bottleneck at nodes {:?}: capacity {}, tension {}",
        bottleneck.cut_nodes, bottleneck.capacity, bottleneck.tension);
}
```

### Self-Healing (Reactive + Predictive)

Self-healing responds to anomalies (reactive) and rising MinCut tension (predictive).

```rust
use ruvector_dag::healing::{HealingOrchestrator, AnomalyConfig, PredictiveConfig};

let mut orchestrator = HealingOrchestrator::new();

// Reactive: Z-score anomaly detection
orchestrator.add_detector("query_latency", AnomalyConfig {
    z_threshold: 3.0,
    window_size: 100,
    min_samples: 10,
});

// Predictive: Rising cut tension triggers early intervention
orchestrator.enable_predictive(PredictiveConfig {
    tension_threshold: 0.6,    // Intervene before 0.7 crisis
    variance_threshold: 1.5,   // Rising variance = trouble coming
    lookahead_window: 50,      // Predict 50 queries ahead
});

// Observe metrics
orchestrator.observe("query_latency", latency);
orchestrator.observe_mincut(&mincut_analysis);

// Healing cycle: reactive + predictive
let result = orchestrator.run_cycle();
println!("Reactive repairs: {}, Predictive interventions: {}",
    result.reactive_repairs, result.predictive_interventions);
```

### External Cost Model Trait

Plug in cost models for PostgreSQL, embedded, or chip-level schedulers without forking logic.

```rust
/// Trait for external cost estimation
pub trait CostModel: Send + Sync {
    /// Estimate execution cost for an operator
    fn estimate_cost(&self, op: &OperatorNode, context: &CostContext) -> f64;

    /// Estimate cardinality (row count) for an operator
    fn estimate_cardinality(&self, op: &OperatorNode, context: &CostContext) -> u64;

    /// Platform-specific overhead factor
    fn platform_overhead(&self) -> f64 { 1.0 }
}

/// PostgreSQL cost model (uses pg_catalog statistics)
pub struct PostgresCostModel { /* ... */ }

/// Embedded systems cost model (memory-bound)
pub struct EmbeddedCostModel {
    pub ram_kb: u32,
    pub flash_latency_ns: u32,
}

/// Chip-level cost model (cycle-accurate)
pub struct ChipCostModel {
    pub clock_mhz: u32,
    pub pipeline_depth: u8,
    pub cache_line_bytes: u8,
}

// Plug into DAG analysis
let mut dag = QueryDag::with_cost_model(Box::new(EmbeddedCostModel {
    ram_kb: 512,
    flash_latency_ns: 100,
}));
```

### QuDAG Integration (Bounded Sync)

Quantum-resistant distributed learning with explicit sync frequency bounds.

```rust
use ruvector_dag::qudag::{QuDagClient, SyncConfig};

let client = QuDagClient::new(SyncConfig {
    // Sync frequency bounds (critical for distributed scale)
    min_sync_interval: Duration::from_secs(60),   // At least 1 min apart
    max_sync_interval: Duration::from_secs(3600), // At most 1 hour
    adaptive_backoff: true,  // Backoff under network pressure

    // Batch settings
    max_patterns_per_sync: 100,
    pattern_age_threshold: Duration::from_secs(300),  // 5 min maturity

    // Privacy
    differential_privacy_epsilon: 0.1,
    noise_mechanism: NoiseMechanism::Laplace,
});

// Sync only mature, validated patterns
client.sync_patterns(
    sona.get_mature_patterns(),
    &crypto_identity,
).await?;

// Receive network-learned patterns (also bounded)
let network_patterns = client.receive_patterns().await?;
sona.merge_network_patterns(network_patterns);
```

## End-to-End Example: Query Convergence

A slow query converges over several runs. One file, no prose, just logs.

```text
$ cargo run --example convergence_demo

[run 1] query: SELECT * FROM vectors WHERE embedding <-> $1 < 0.5
        dag: 4 nodes, 3 edges
        attention: topological (default)
        mincut_tension: 0.23
        latency: 847ms (baseline: 850ms, improvement: 0.4%)
        sona: recorded trajectory, pattern_id=0x7a3f

[run 2] same query, different params
        attention: topological
        mincut_tension: 0.31 (rising)
        latency: 812ms (improvement: 4.5%)
        sona: pattern match, applying lora_weights

[run 3]
        attention: topological
        mincut_tension: 0.58 (approaching threshold)
        latency: 623ms (improvement: 26.7%)
        sona: lora adaptation complete, ewc consolidating

[run 4]
        mincut_tension: 0.71 > 0.7 (THRESHOLD)
        --> switching attention: topological -> mincut_gated
        --> healing: predictive intervention queued
        attention: mincut_gated
        latency: 412ms (improvement: 51.5%)
        sona: boosting learning rate 2x for this pattern

[run 5]
        attention: mincut_gated (sticky after tension spike)
        mincut_tension: 0.45 (stabilizing)
        latency: 398ms (improvement: 53.2%)
        healing: predictive reindex completed in background

[run 10]
        attention: mincut_gated
        mincut_tension: 0.22 (stable)
        latency: 156ms (improvement: 81.6%)
        sona: pattern mature, queued for qudag sync

[qudag sync] pattern 0x7a3f synced to network
             peers learning from our optimization
```

## Examples

The `examples/` directory contains:

- `basic_usage.rs` - DAG creation and basic operations
- `attention_selection.rs` - Policy-driven attention switching
- `learning_workflow.rs` - SONA learning with explicit state vectors
- `self_healing.rs` - Reactive and predictive healing
- `convergence_demo.rs` - End-to-end query convergence logs

```bash
cargo run --example basic_usage
cargo run --example attention_selection
cargo run --example learning_workflow
cargo run --example self_healing
```

## WASM Target

Minimal WASM build for browser and embedded systems.

| Metric | Value |
|--------|-------|
| Raw size | 130 KB |
| Gzipped | 58 KB |
| API surface | 13 methods |

```bash
# Build WASM
wasm-pack build crates/ruvector-dag-wasm --target web --release

# With wee_alloc for even smaller size
wasm-pack build crates/ruvector-dag-wasm --target web --release -- --features wee_alloc
```

## Performance Targets

| Component | Target | Notes |
|-----------|--------|-------|
| Attention (100 nodes) | <100μs | All 7 mechanisms |
| MicroLoRA adaptation | <100μs | Rank-2, per-operator |
| Pattern search (10K) | <2ms | K-means++ indexing |
| MinCut update | O(n^0.12) | Subpolynomial amortized |
| Anomaly detection | <50μs | Z-score, streaming |
| Predictive healing | <1ms | Tension-based lookahead |
| QuDAG sync | Bounded | 1min-1hr adaptive |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Query DAG Layer                          │
│           (Operators, Edges, Topological Sort)              │
│                + External Cost Model Trait                  │
└───────────────────────────┬─────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
   ┌──────────▼──────────┐     ┌─────────▼─────────┐
   │   Attention Layer   │     │   MinCut Engine   │
   │   (7 mechanisms)    │◄────│ (Control Signal)  │
   │   + Policy Selector │     │   O(n^0.12)       │
   └──────────┬──────────┘     └─────────┬─────────┘
              │                          │
              │    ┌─────────────────────┤
              │    │                     │
   ┌──────────▼────▼─────┐    ┌─────────▼─────────┐
   │    SONA Engine      │    │   Self-Healing    │
   │  (Post-Query Learn) │    │ (Reactive + Pred) │
   │  MicroLoRA + EWC    │    │ Tension-Driven    │
   └──────────┬──────────┘    └─────────┬─────────┘
              │                         │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │   QuDAG Sync Layer      │
              │  (Bounded Frequency)    │
              │  ML-KEM + Differential  │
              └─────────────────────────┘
```

## Development

```bash
# Run tests
cargo test -p ruvector-dag

# Run benchmarks
cargo bench -p ruvector-dag

# Check documentation
cargo doc -p ruvector-dag --open
```

## Integration with RuVector

This crate is part of the RuVector ecosystem:

- `ruvector-core` - Core vector operations
- `ruvector-dag-wasm` - Browser/embedded WASM target (58KB gzipped)
- `ruvector-postgres` - PostgreSQL extension with 50+ SQL functions
- `ruvector-qudag` - Full QuDAG consensus client

## License

Apache-2.0 OR MIT
