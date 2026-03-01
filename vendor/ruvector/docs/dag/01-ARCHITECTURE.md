# Neural Self-Learning DAG Architecture

## Overview

The Neural Self-Learning DAG system transforms RuVector-Postgres from a static query executor into an adaptive system that learns optimal configurations from query patterns.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        NEURAL DAG RUVECTOR-POSTGRES                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         SQL INTERFACE LAYER                          │   │
│  │  ruvector_enable_neural_dag() | ruvector_dag_patterns() | ...       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                      QUERY OPTIMIZER LAYER                           │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │   Pattern   │  │  Attention  │  │    Cost     │  │   Plan     │  │   │
│  │  │   Matcher   │  │  Selector   │  │  Estimator  │  │  Rewriter  │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                       DAG ATTENTION LAYER                            │   │
│  │  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐           │   │
│  │  │Topological│ │  Causal   │ │ Critical  │ │  MinCut   │           │   │
│  │  │ Attention │ │   Cone    │ │   Path    │ │  Gated    │           │   │
│  │  └───────────┘ └───────────┘ └───────────┘ └───────────┘           │   │
│  │  ┌───────────┐ ┌───────────┐ ┌───────────┐                         │   │
│  │  │Hierarchic │ │ Parallel  │ │ Temporal  │                         │   │
│  │  │  Lorentz  │ │  Branch   │ │   BTSP    │                         │   │
│  │  └───────────┘ └───────────┘ └───────────┘                         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                        SONA LEARNING LAYER                           │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  INSTANT LOOP (<100μs)           BACKGROUND LOOP (hourly)   │    │   │
│  │  │  ┌─────────────┐                 ┌─────────────┐            │    │   │
│  │  │  │  MicroLoRA  │                 │  BaseLoRA   │            │    │   │
│  │  │  │  (rank 1-2) │                 │  (rank 8)   │            │    │   │
│  │  │  └─────────────┘                 └─────────────┘            │    │   │
│  │  │  ┌─────────────┐                 ┌─────────────┐            │    │   │
│  │  │  │ Trajectory  │ ──────────────► │ ReasoningBk │            │    │   │
│  │  │  │   Buffer    │                 │  (K-means)  │            │    │   │
│  │  │  └─────────────┘                 └─────────────┘            │    │   │
│  │  │                                  ┌─────────────┐            │    │   │
│  │  │                                  │   EWC++     │            │    │   │
│  │  │                                  │ (forgetting)│            │    │   │
│  │  │                                  └─────────────┘            │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                      OPTIMIZATION LAYER                              │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │   MinCut    │  │    HDC      │  │    BTSP     │  │   Self-    │  │   │
│  │  │  Analysis   │  │   State     │  │   Memory    │  │  Healing   │  │   │
│  │  │ O(n^0.12)   │  │ Compression │  │  One-Shot   │  │   Engine   │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                      STORAGE LAYER                                   │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │   Pattern   │  │  Embedding  │  │  Trajectory │  │   Index    │  │   │
│  │  │   Store     │  │   Cache     │  │   History   │  │  Metadata  │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                   OPTIONAL: QUDAG CONSENSUS LAYER                    │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │  Federated  │  │  Pattern    │  │   ML-DSA    │  │    rUv     │  │   │
│  │  │  Learning   │  │  Consensus  │  │  Signatures │  │   Tokens   │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Component Descriptions

### 1. SQL Interface Layer

Provides PostgreSQL-native functions for interacting with the Neural DAG system.

**Key Components:**
- `ruvector_enable_neural_dag()` - Enable learning for a table
- `ruvector_dag_patterns()` - View learned patterns
- `ruvector_attention_*()` - DAG attention functions
- `ruvector_dag_learn()` - Trigger learning cycle

**Location:** `crates/ruvector-postgres/src/dag/operators.rs`

### 2. Query Optimizer Layer

Intercepts queries and applies learned optimizations.

**Key Components:**
- **Pattern Matcher**: Finds similar past query patterns via cosine similarity
- **Attention Selector**: UCB bandit for choosing optimal attention type
- **Cost Estimator**: Adaptive cost model with micro-LoRA updates
- **Plan Rewriter**: Applies learned operator ordering and parameters

**Location:** `crates/ruvector-postgres/src/dag/optimizer.rs`

### 3. DAG Attention Layer

Seven specialized attention mechanisms for DAG structures.

| Attention Type | Use Case | Complexity |
|----------------|----------|------------|
| Topological | Respect DAG ordering | O(n·k) |
| Causal Cone | Distance-weighted ancestors | O(n·d) |
| Critical Path | Focus on bottlenecks | O(n + critical_len) |
| MinCut Gated | Gate by criticality | O(n^0.12 + n·k) |
| Hierarchical Lorentz | Deep nesting | O(n·d) |
| Parallel Branch | Coordinate branches | O(n·b) |
| Temporal BTSP | Time-correlated patterns | O(n·w) |

**Location:** `crates/ruvector-postgres/src/dag/attention/`

### 4. SONA Learning Layer

Two-tier learning system for continuous optimization.

**Instant Loop (per-query):**
- MicroLoRA adaptation (rank 1-2)
- Trajectory recording
- <100μs overhead

**Background Loop (hourly):**
- K-means++ pattern extraction
- BaseLoRA updates (rank 8)
- EWC++ constraint application

**Location:** `crates/ruvector-postgres/src/dag/learning/`

### 5. Optimization Layer

Advanced optimization components.

**Key Components:**
- **MinCut Analysis**: O(n^0.12) bottleneck detection
- **HDC State**: 10K-bit hypervector compression
- **BTSP Memory**: One-shot pattern recall
- **Self-Healing**: Proactive index repair

**Location:** `crates/ruvector-postgres/src/dag/optimization/`

### 6. Storage Layer

Persistent storage for learned patterns and state.

**Key Components:**
- **Pattern Store**: DashMap + PostgreSQL tables
- **Embedding Cache**: LRU cache for hot embeddings
- **Trajectory History**: Ring buffer for recent queries
- **Index Metadata**: Pattern-to-index mappings

**Location:** `crates/ruvector-postgres/src/dag/storage/`

### 7. QuDAG Consensus Layer (Optional)

Distributed learning via quantum-resistant consensus.

**Key Components:**
- **Federated Learning**: Privacy-preserving pattern sharing
- **Pattern Consensus**: QR-Avalanche for pattern validation
- **ML-DSA Signatures**: Quantum-resistant pattern signing
- **rUv Tokens**: Incentivize learning contributions

**Location:** `crates/ruvector-postgres/src/dag/qudag/`

## Data Flow

### Query Execution Flow

```
SQL Query
    │
    ▼
┌─────────────────────────────────────┐
│ 1. Pattern Matching                 │
│    - Embed query plan               │
│    - Find similar patterns in       │
│      ReasoningBank (cosine sim)     │
│    - Return top-k matches           │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 2. Optimization Decision            │
│    - If pattern found (conf > 0.8): │
│      Apply learned configuration    │
│    - Else:                          │
│      Use defaults + micro-LoRA      │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 3. Attention Selection              │
│    - UCB bandit selects attention   │
│    - Based on query pattern type    │
│    - Exploration vs exploitation    │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 4. Plan Execution                   │
│    - Execute with optimized params  │
│    - Record operator timings        │
│    - Track intermediate results     │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 5. Trajectory Recording             │
│    - Store query embedding          │
│    - Store operator activations     │
│    - Store outcome metrics          │
│    - Compute quality score          │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 6. Instant Learning                 │
│    - MicroLoRA gradient accumulate  │
│    - Auto-flush at 100 queries      │
│    - Update attention selector      │
└─────────────────────────────────────┘
```

### Learning Cycle Flow

```
Hourly Trigger
    │
    ▼
┌─────────────────────────────────────┐
│ 1. Drain Trajectory Buffer          │
│    - Collect 1000+ trajectories     │
│    - Filter by quality threshold    │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 2. K-means++ Clustering             │
│    - 100 clusters                   │
│    - Deterministic initialization   │
│    - Max 100 iterations             │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 3. Pattern Extraction               │
│    - Compute cluster centroids      │
│    - Extract optimal parameters     │
│    - Calculate confidence scores    │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 4. EWC++ Constraint Check           │
│    - Compute Fisher information     │
│    - Apply forgetting prevention    │
│    - Detect task boundaries         │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 5. BaseLoRA Update                  │
│    - Apply constrained gradients    │
│    - Update all layers              │
│    - Merge weights if needed        │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 6. ReasoningBank Update             │
│    - Store new patterns             │
│    - Consolidate similar patterns   │
│    - Evict low-confidence patterns  │
└─────────────────────────────────────┘
```

## Module Dependencies

```
ruvector-postgres/src/dag/
├── mod.rs                    # Module root, re-exports
├── operators.rs              # SQL function definitions
│
├── attention/
│   ├── mod.rs               # Attention trait and registry
│   ├── topological.rs       # TopologicalAttention
│   ├── causal_cone.rs       # CausalConeAttention
│   ├── critical_path.rs     # CriticalPathAttention
│   ├── mincut_gated.rs      # MinCutGatedAttention
│   ├── hierarchical.rs      # HierarchicalLorentzAttention
│   ├── parallel_branch.rs   # ParallelBranchAttention
│   ├── temporal_btsp.rs     # TemporalBTSPAttention
│   └── ensemble.rs          # EnsembleAttention
│
├── learning/
│   ├── mod.rs               # Learning coordinator
│   ├── sona_engine.rs       # SONA integration wrapper
│   ├── trajectory.rs        # Trajectory buffer
│   ├── patterns.rs          # Pattern extraction
│   ├── reasoning_bank.rs    # Pattern storage
│   ├── ewc.rs               # EWC++ integration
│   └── attention_selector.rs # UCB bandit selector
│
├── optimizer/
│   ├── mod.rs               # Optimizer coordinator
│   ├── pattern_matcher.rs   # Pattern matching
│   ├── cost_estimator.rs    # Adaptive costs
│   └── plan_rewriter.rs     # Plan transformation
│
├── optimization/
│   ├── mod.rs               # Optimization utilities
│   ├── mincut.rs            # Min-cut integration
│   ├── hdc_state.rs         # HDC compression
│   ├── btsp_memory.rs       # BTSP one-shot
│   └── self_healing.rs      # Self-healing engine
│
├── storage/
│   ├── mod.rs               # Storage coordinator
│   ├── pattern_store.rs     # Pattern persistence
│   ├── embedding_cache.rs   # Embedding LRU
│   └── trajectory_store.rs  # Trajectory history
│
├── qudag/
│   ├── mod.rs               # QuDAG integration
│   ├── federated.rs         # Federated learning
│   ├── consensus.rs         # Pattern consensus
│   ├── signatures.rs        # ML-DSA signing
│   └── tokens.rs            # rUv token interface
│
└── types/
    ├── mod.rs               # Type definitions
    ├── neural_plan.rs       # NeuralDagPlan
    ├── trajectory.rs        # DagTrajectory
    ├── pattern.rs           # LearnedDagPattern
    └── metrics.rs           # ExecutionMetrics
```

## Configuration

### Default Configuration

```rust
pub struct NeuralDagConfig {
    // Learning
    pub learning_enabled: bool,           // true
    pub max_trajectories: usize,          // 10000
    pub pattern_clusters: usize,          // 100
    pub quality_threshold: f32,           // 0.3
    pub background_interval_ms: u64,      // 3600000 (1 hour)

    // Attention
    pub default_attention: DagAttentionType, // Topological
    pub attention_exploration: f32,       // 0.1
    pub ucb_exploration_c: f32,           // 1.414

    // SONA
    pub micro_lora_rank: usize,           // 2
    pub micro_lora_lr: f32,               // 0.002
    pub base_lora_rank: usize,            // 8
    pub base_lora_lr: f32,                // 0.001

    // EWC++
    pub ewc_lambda: f32,                  // 2000.0
    pub ewc_max_lambda: f32,              // 15000.0
    pub ewc_fisher_decay: f32,            // 0.999

    // MinCut
    pub mincut_enabled: bool,             // true
    pub mincut_threshold: f32,            // 0.5

    // HDC
    pub hdc_dimensions: usize,            // 10000

    // Self-Healing
    pub healing_enabled: bool,            // true
    pub healing_check_interval_ms: u64,   // 300000 (5 min)
}
```

### PostgreSQL GUC Variables

```sql
-- Enable/disable neural DAG
SET ruvector.neural_dag_enabled = true;

-- Learning parameters
SET ruvector.dag_learning_rate = 0.002;
SET ruvector.dag_pattern_clusters = 100;
SET ruvector.dag_quality_threshold = 0.3;

-- Attention parameters
SET ruvector.dag_attention_type = 'auto';
SET ruvector.dag_attention_exploration = 0.1;

-- EWC parameters
SET ruvector.dag_ewc_lambda = 2000.0;

-- MinCut parameters
SET ruvector.dag_mincut_enabled = true;
SET ruvector.dag_mincut_threshold = 0.5;
```

## Performance Targets

| Operation | Target Latency | Notes |
|-----------|----------------|-------|
| Pattern matching | <1ms | Top-5 similar patterns |
| Attention computation | <500μs | Per operator |
| MicroLoRA forward | <100μs | Per query |
| Trajectory recording | <50μs | Non-blocking |
| Background learning | <5s | 1000 trajectories |
| MinCut analysis | <10ms | O(n^0.12) |
| HDC encoding | <100μs | 10K dimensions |

## Memory Budget

| Component | Budget | Notes |
|-----------|--------|-------|
| Pattern Store | 50MB | ~1000 patterns per table |
| Embedding Cache | 20MB | LRU for hot embeddings |
| Trajectory Buffer | 20MB | 10K trajectories |
| MicroLoRA | 10KB | Per active query |
| BaseLoRA | 400KB | Per table |
| HDC State | 1.2KB | Per state snapshot |

**Total per table:** ~100MB maximum

## Thread Safety

All components use thread-safe primitives:

- `DashMap` for concurrent pattern storage
- `parking_lot::RwLock` for embedding cache
- `crossbeam::ArrayQueue` for trajectory buffer
- `AtomicU64` for counters and statistics
- PostgreSQL background workers for learning cycles

## Error Handling

```rust
pub enum NeuralDagError {
    // Configuration errors
    InvalidConfig(String),
    TableNotEnabled(String),

    // Learning errors
    InsufficientTrajectories,
    PatternExtractionFailed,
    EwcConstraintViolation,

    // Attention errors
    AttentionComputationFailed,
    InvalidDagStructure,

    // Storage errors
    PatternStoreFull,
    EmbeddingCacheMiss,

    // MinCut errors
    MinCutComputationFailed,
    GraphDisconnected,

    // QuDAG errors (optional)
    ConsensusTimeout,
    SignatureVerificationFailed,
}
```

All errors are logged and non-fatal - the system falls back to default behavior on error.
