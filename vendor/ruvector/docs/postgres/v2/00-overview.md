# RuVector Postgres v2 - Architecture Overview
<!-- Last reviewed: 2025-12-25 -->

## What We're Building

Most databases, including vector databases, are **performance-first systems**. They optimize for speed, recall, and throughput, then bolt on monitoring. Structural safety is assumed, not measured.

RuVector does something different.

We give the system a **continuous, internal measure of its own structural integrity**, and the ability to **change its own behavior based on that signal**.

This puts RuVector in a very small class of systems.

---

## Why This Actually Matters

### 1. From Symptom Monitoring to Causal Monitoring

Everyone else watches outputs: latency, errors, recall.

We watch **connectivity and dependence**, which are upstream causes.

By the time latency spikes, the graph has already weakened. We detect that weakening while everything still looks healthy.

> **This is the difference between a smoke alarm and a structural stress sensor.**

### 2. Mincut Is a Leading Indicator, Not a Metric

Mincut answers a question no metric answers:

> *"How close is this system to splitting?"*

Not how slow it is. Not how many errors. **How close it is to losing coherence.**

That is a different axis of observability.

### 3. An Algorithm Becomes a Control Signal

Most people use graph algorithms for analysis. We use mincut to **gate behavior**.

That makes it a **control plane**, not analytics.

Very few production systems have mathematically grounded control loops.

### 4. Failure Mode Changes Class

| Without Integrity Control | With Integrity Control |
|---------------------------|------------------------|
| Fast → stressed → cascading failure → manual recovery | Fast → stressed → scope reduction → graceful degradation → automatic recovery |

Changing failure mode is what separates hobby systems from infrastructure.

### 5. Explainable Operations

The **witness edges** are huge.

When something slows down or freezes, we can say: *"Here are the exact links that would have failed next."*

That is gold in production, audits, and regulated environments.

---

## Why Nobody Else Has Done This

Not because it's impossible. Because:

1. **Most systems don't model themselves as graphs** — we do
2. **Mincut was too expensive dynamically** — we use contracted graphs (~1000 nodes, not millions)
3. **Ops culture reacts, it doesn't preempt** — we preempt
4. **Survivability isn't a KPI until after outages** — we measure it continuously

---

## The Honest Framing

Will this get applause from model benchmarks or social media? No.

Will this make systems boringly reliable and therefore indispensable? Yes.

Those are the ideas that end up everywhere.

**We're not making vector search faster. We're making vector infrastructure survivable.**

---

## What This Is, Concretely

RuVector Postgres v2 is a **PostgreSQL extension** (built with pgrx) that provides:

- **100% pgvector compatibility** — drop-in replacement, change extension name, queries work unchanged
- **Architecture separation** — PostgreSQL handles ACID/joins, RuVector handles vectors/graphs/learning
- **Dynamic mincut integrity gating** — the control plane described above
- **Self-learning pipeline** — GNN-based query optimization that improves over time
- **Tiered storage** — automatic hot/warm/cool/cold management with compression
- **Graph engine with Cypher** — property graphs with SQL joins

---

## Architecture Principles

### Separation of Concerns

```
+------------------------------------------------------------------+
|                     PostgreSQL Frontend                           |
|  (SQL Parsing, Planning, ACID, Transactions, Joins, Aggregates)   |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                   Extension Boundary (pgrx)                       |
|  - Type definitions (vector, sparsevec, halfvec)                 |
|  - Operator overloads (<->, <=>, <#>)                            |
|  - Index access method hooks                                      |
|  - Background worker registration                                 |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                    RuVector Engine (Rust)                         |
|  - HNSW/IVFFlat indexing                                         |
|  - SIMD distance calculations                                     |
|  - Graph storage & Cypher execution                              |
|  - GNN training & inference                                       |
|  - Compression & tiering                                          |
|  - Mincut integrity control                                       |
+------------------------------------------------------------------+
```

### Core Design Decisions

| Decision | Rationale |
|----------|-----------|
| **pgrx for extension** | Safe Rust bindings, modern build system, well-maintained |
| **Background worker pattern** | Long-lived engine, avoid per-query initialization |
| **Shared memory IPC** | Bounded request queue with explicit payload limits (see [02-background-workers](02-background-workers.md)) |
| **WAL as source of truth** | Leverage Postgres replication, durability guarantees |
| **Contracted mincut graph** | Never compute on full similarity - use operational graph |
| **Hybrid consistency** | Synchronous hot tier, async background ops (see [10-consistency-replication](10-consistency-replication.md)) |

---

## System Architecture

### High-Level Components

```
                                   +-----------------------+
                                   |   Client Application  |
                                   +-----------+-----------+
                                               |
                                   +-----------v-----------+
                                   |     PostgreSQL        |
                                   |  +-----------------+  |
                                   |  | Query Executor  |  |
                                   |  +--------+--------+  |
                                   |           |           |
                                   |  +--------v--------+  |
                                   |  | RuVector SQL    |  |
                                   |  | Surface Layer   |  |
                                   |  +--------+--------+  |
                                   +-----------|----------+
                                               |
                          +--------------------+--------------------+
                          |                                         |
               +----------v----------+                  +-----------v-----------+
               |   Index AM Hooks    |                  |  Background Workers   |
               |  (HNSW, IVFFlat)   |                  |  (Maintenance, GNN)   |
               +----------+----------+                  +-----------+-----------+
                          |                                         |
                          +--------------------+--------------------+
                                               |
                                   +-----------v-----------+
                                   |   Shared Memory      |
                                   |   Communication      |
                                   +-----------+-----------+
                                               |
                                   +-----------v-----------+
                                   |   RuVector Engine    |
                                   |  +-------+ +-------+ |
                                   |  | Index | | Graph | |
                                   |  +-------+ +-------+ |
                                   |  +-------+ +-------+ |
                                   |  |  GNN  | | Tier  | |
                                   |  +-------+ +-------+ |
                                   |  +------------------+|
                                   |  | Integrity Ctrl   ||
                                   |  +------------------+|
                                   +-----------------------+
```

### Component Responsibilities

#### 1. SQL Surface Layer
- **pgvector type compatibility**: `vector(n)`, operators `<->`, `<#>`, `<=>`
- **Extended types**: `sparsevec`, `halfvec`, `binaryvec`
- **Function catalog**: `ruvector_*` functions for advanced features
- **Views**: `ruvector_nodes`, `ruvector_edges`, `ruvector_hyperedges`

#### 2. Index Access Methods
- **ruhnsw**: HNSW index with configurable M, ef_construction
- **ruivfflat**: IVF-Flat index with automatic centroid updates
- **Scan hooks**: Route queries to RuVector engine
- **Build hooks**: Incremental and bulk index construction

#### 3. Background Workers
- **Engine Worker**: Long-lived RuVector engine instance
- **Maintenance Worker**: Tiering, compaction, statistics
- **GNN Training Worker**: Periodic model updates
- **Integrity Worker**: Mincut sampling and state updates

#### 4. RuVector Engine
- **Index Manager**: HNSW/IVFFlat in-memory structures
- **Graph Store**: Property graph with Cypher support
- **GNN Pipeline**: Training data capture, model inference
- **Tier Manager**: Hot/warm/cool/cold classification
- **Integrity Controller**: Mincut-based operation gating

---

## Feature Matrix

### Phase 1: pgvector Compatibility (Foundation)

| Feature | Status | Description |
|---------|--------|-------------|
| `vector(n)` type | Core | Dense vector storage |
| `<->` operator | Core | L2 (Euclidean) distance |
| `<=>` operator | Core | Cosine distance |
| `<#>` operator | Core | Negative inner product |
| HNSW index | Core | `CREATE INDEX ... USING hnsw` |
| IVFFlat index | Core | `CREATE INDEX ... USING ivfflat` |
| `vector_l2_ops` | Core | Operator class for L2 |
| `vector_cosine_ops` | Core | Operator class for cosine |
| `vector_ip_ops` | Core | Operator class for inner product |

### Phase 2: Tiered Storage & Compression

| Feature | Status | Description |
|---------|--------|-------------|
| `ruvector_set_tiers()` | v2 | Configure tier thresholds |
| `ruvector_compact()` | v2 | Trigger manual compaction |
| Access frequency tracking | v2 | Background counter updates |
| Automatic tier promotion/demotion | v2 | Policy-based migration |
| SQ8/PQ compression | v2 | Transparent quantization |

### Phase 3: Graph Engine & Cypher

| Feature | Status | Description |
|---------|--------|-------------|
| `ruvector_cypher()` | v2 | Execute Cypher queries |
| `ruvector_nodes` view | v2 | Graph nodes as relations |
| `ruvector_edges` view | v2 | Graph edges as relations |
| `ruvector_hyperedges` view | v2 | Hyperedge support |
| SQL-graph joins | v2 | Mix Cypher with SQL |

### Phase 4: Integrity Control Plane

| Feature | Status | Description |
|---------|--------|-------------|
| `ruvector_integrity_sample()` | v2 | Sample contracted graph |
| `ruvector_integrity_policy_set()` | v2 | Configure policies |
| `ruvector_integrity_gate()` | v2 | Check operation permission |
| Integrity states | v2 | normal/stress/critical |
| Signed audit events | v2 | Cryptographic audit trail |

---

## Data Flow Patterns

### Vector Search (Read Path)

```
1. Client: SELECT ... ORDER BY embedding <-> $query LIMIT k

2. PostgreSQL Planner:
   - Recognizes index on embedding column
   - Generates Index Scan plan using ruhnsw

3. Index AM (amgettuple):
   - Submits search request to shared memory queue
   - Engine worker receives request

4. RuVector Engine:
   - Checks integrity gate (normal state: proceed)
   - Executes HNSW greedy search
   - Applies post-filter if needed
   - Returns top-k with distances

5. Index AM:
   - Fetches results from shared memory
   - Returns TIDs to executor

6. PostgreSQL Executor:
   - Fetches heap tuples
   - Applies remaining WHERE clauses
   - Returns to client
```

### Vector Insert (Write Path)

```
1. Client: INSERT INTO items (embedding) VALUES ($vec)

2. PostgreSQL Executor:
   - Assigns TID, writes heap tuple
   - Generates WAL record

3. Index AM (aminsert):
   - Checks integrity gate (normal: proceed, stress: throttle)
   - Submits insert to engine queue

4. RuVector Engine:
   - Integrates vector into HNSW graph
   - Updates tier counters
   - Writes to hot tier

5. WAL Writer:
   - Persists operation for durability

6. Replication (if configured):
   - Streams WAL to replicas
   - Replicas apply via engine
```

### Integrity Gating

```
1. Background Worker (periodic):
   - Samples contracted operational graph
   - Computes lambda_cut (minimum cut value) on contracted graph
   - Optionally computes lambda2 (algebraic connectivity) as drift signal
   - Updates integrity state in shared memory

2. Any Operation:
   - Reads current integrity state
   - normal (lambda > T_high): allow all
   - stress (T_low < lambda < T_high): throttle bulk ops
   - critical (lambda < T_low): freeze mutations

3. On State Change:
   - Logs signed integrity event
   - Notifies waiting operations
   - Adjusts background worker priorities
```

---

## Deployment Modes

### Mode 1: Single Postgres Embedded

```
+--------------------------------------------+
|            PostgreSQL Instance             |
|  +--------------------------------------+  |
|  |          RuVector Extension          |  |
|  |  +--------+  +---------+  +-------+  |  |
|  |  | Engine |  | Workers |  | Index |  |  |
|  |  +--------+  +---------+  +-------+  |  |
|  +--------------------------------------+  |
|                                            |
|  +--------------------------------------+  |
|  |           Data Directory             |  |
|  |  vectors/ graphs/ indexes/ wal/      |  |
|  +--------------------------------------+  |
+--------------------------------------------+
```

**Use case**: Development, small-medium deployments (< 100M vectors)

### Mode 2: Postgres + RuVector Cluster

```
+------------------+      +------------------+
|   PostgreSQL 1   |      |   PostgreSQL 2   |
|  (Primary)       |      |  (Replica)       |
+--------+---------+      +--------+---------+
         |                         |
         | WAL Stream              | WAL Apply
         |                         |
+--------v-------------------------v---------+
|              RuVector Cluster              |
|  +-------+  +-------+  +-------+  +------+ |
|  | Node1 |  | Node2 |  | Node3 |  | ...  | |
|  +-------+  +-------+  +-------+  +------+ |
|                                             |
|  Distributed HNSW | Sharded Graph | GNN    |
+---------------------------------------------+
```

**Use case**: Production, large deployments (100M+ vectors)

### v2 Cluster Mode Clarification

```
+------------------------------------------------------------------+
|              CLUSTER DEPLOYMENT DECISION                          |
+------------------------------------------------------------------+

v2 cluster mode is a SEPARATE SERVICE with a stable RPC API.
The Postgres extension acts as a CLIENT to the cluster.

ARCHITECTURE OPTIONS:

Option A: SIDECAR (per Postgres instance)
  • RuVector cluster node co-located with each Postgres
  • Pros: Low latency, simple networking
  • Cons: Resource contention, harder to scale independently
  • Use when: Latency-sensitive, moderate scale

Option B: SHARED SERVICE (separate cluster)
  • Dedicated RuVector cluster serving multiple Postgres instances
  • Pros: Independent scaling, resource isolation
  • Cons: Network latency, requires service discovery
  • Use when: Large scale, multi-tenant

PROTOCOL:
  • gRPC with protobuf serialization
  • mTLS for authentication
  • Connection pooling in extension

PARTITION ASSIGNMENT:
  • Consistent hashing for shard routing
  • Automatic rebalancing on node join/leave
  • Partition map cached in extension shared memory

PARTITION MAP VERSIONING AND FENCING:
  • partition_map_version: monotonic counter incremented on any change
  • lease_epoch: obtained from cluster leader, prevents split-brain
  • Extension rejects stale map updates unless epoch matches current
  • On leader failover:
    1. New leader increments epoch
    2. Extensions must re-fetch map with new epoch
    3. Stale-epoch operations return ESTALE, client retries

v2 RECOMMENDATION:
  Start with Mode 1 (embedded). Add cluster mode only when:
  • Dataset exceeds single-node memory
  • Need independent scaling of compute/storage
  • Multi-region deployment required

+------------------------------------------------------------------+
```

---

## Consistency Contract

### Heap-Engine Relationship

```
+------------------------------------------------------------------+
|                    CONSISTENCY CONTRACT                           |
+------------------------------------------------------------------+
|                                                                   |
|  PostgreSQL Heap is AUTHORITATIVE for:                           |
|    • Row existence and visibility (MVCC xmin/xmax)               |
|    • Transaction commit status                                    |
|    • Data integrity constraints                                   |
|                                                                   |
|  RuVector Engine Index is EVENTUALLY CONSISTENT:                 |
|    • Bounded lag window (configurable, default 100ms)            |
|    • Never returns invisible tuples (heap recheck)               |
|    • Never resurrects deleted vectors                             |
|                                                                   |
|  v2 HYBRID MODEL:                                                 |
|    • SYNCHRONOUS: Hot tier mutations, primary HNSW inserts       |
|    • ASYNCHRONOUS: Compaction, tier moves, graph maintenance     |
|                                                                   |
+------------------------------------------------------------------+
```

See [10-consistency-replication.md](10-consistency-replication.md) for full specification.

---

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Query latency (p50) | < 5ms | 1M vectors, top-10 |
| Query latency (p99) | < 20ms | 1M vectors, top-10 |
| Insert throughput | > 10K/sec | Bulk mode |
| Index build | < 30min | 10M 768-dim vectors |
| Recall@10 | > 95% | HNSW default params |
| Compression ratio | 4-32x | Tier-dependent |
| Memory overhead | < 2x | Compared to pgvector |

### Benchmark Specification

Performance targets must be validated against a defined benchmark suite:

```
+------------------------------------------------------------------+
|                    BENCHMARK SPECIFICATION                        |
+------------------------------------------------------------------+

VECTOR CONFIGURATIONS:
  • Dimensions: 768 (typical text embeddings), 1536 (large embedding models)
  • Row counts: 1M, 10M, 100M
  • Data type: float32

QUERY PATTERNS:
  • Pure vector search (no filter)
  • Vector + metadata filter (10% selectivity)
  • Vector + metadata filter (1% selectivity)
  • Batch query (100 queries)

HARDWARE BASELINE:
  • CPU: 8 cores (AMD EPYC or Intel Xeon)
  • RAM: 64GB
  • Storage: NVMe SSD (3GB/s read)
  • Single node, no replication

CONCURRENCY:
  • Single thread baseline
  • 8 concurrent queries (parallel)
  • 32 concurrent queries (stress)

RECALL MEASUREMENT:
  • Brute-force baseline on 10K sampled queries
  • Report recall@1, recall@10, recall@100
  • Calculate 95th percentile recall

INDEX CONFIGURATIONS:
  • HNSW: M=16, ef_construction=200, ef_search=100
  • IVFFlat: nlist=sqrt(N), nprobe=10

TIER-SPECIFIC TARGETS:
  • Hot tier: exact float32, recall > 98%
  • Warm tier: exact or float16, recall > 96%
  • Cool tier: approximate + rerank, recall > 94%
  • Cold tier: approximate only, recall > 90%

+------------------------------------------------------------------+
```

---

## Security Considerations

### Integrity Event Signing

All integrity state changes are cryptographically signed:

```rust
struct IntegrityEvent {
    timestamp: DateTime<Utc>,
    event_type: IntegrityEventType,
    previous_state: IntegrityState,
    new_state: IntegrityState,
    lambda_cut: f64,
    witness_edges: Vec<EdgeId>,
    signature: Ed25519Signature,
}
```

### Access Control

- Leverages PostgreSQL GRANT/REVOKE
- Separate roles for:
  - `ruvector_admin`: Full access
  - `ruvector_operator`: Maintenance operations
  - `ruvector_user`: Query and insert only

### Audit Trail

- All administrative operations logged
- Integrity events stored in `ruvector_integrity_events`
- Optional export to external SIEM

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-4)
- [ ] Extension skeleton with pgrx
- [ ] Collection metadata tables
- [ ] Basic HNSW integration
- [ ] pgvector compatibility tests
- [ ] Recall/performance benchmarks

### Phase 2: Tiered Storage (Weeks 5-8)
- [ ] Access counter infrastructure
- [ ] Tier policy table
- [ ] Background compactor
- [ ] Compression integration
- [ ] Tier report functions

### Phase 3: Graph & Cypher (Weeks 9-12)
- [ ] Graph storage schema
- [ ] Cypher parser integration
- [ ] Relational bridge views
- [ ] SQL-graph join helpers
- [ ] Graph maintenance

### Phase 4: Integrity Control (Weeks 13-16)
- [ ] Contracted graph construction
- [ ] Lambda cut computation
- [ ] Policy application layer
- [ ] Signed audit events
- [ ] Control plane testing

---

## Dependencies

### Rust Crates

| Crate | Purpose |
|-------|---------|
| `pgrx` | PostgreSQL extension framework |
| `parking_lot` | Fast synchronization primitives |
| `crossbeam` | Lock-free data structures |
| `serde` | Serialization |
| `ed25519-dalek` | Signature verification |

### PostgreSQL Features

| Feature | Minimum Version |
|---------|-----------------|
| Background workers | 9.4+ |
| Custom access methods | 9.6+ |
| Parallel query | 9.6+ |
| Logical replication | 10+ |
| Partitioning | 10+ (native) |

---

## Related Documents

| Document | Description |
|----------|-------------|
| [01-sql-schema.md](01-sql-schema.md) | Complete SQL schema |
| [02-background-workers.md](02-background-workers.md) | Worker specifications with IPC contract |
| [03-index-access-methods.md](03-index-access-methods.md) | Index AM details |
| [04-integrity-events.md](04-integrity-events.md) | Event schema, policies, hysteresis, operation classes |
| [05-phase1-pgvector-compat.md](05-phase1-pgvector-compat.md) | Phase 1 specification with incremental AM path |
| [06-phase2-tiered-storage.md](06-phase2-tiered-storage.md) | Phase 2 specification with tier exactness modes |
| [07-phase3-graph-cypher.md](07-phase3-graph-cypher.md) | Phase 3 specification with SQL join keys |
| [08-phase4-integrity-control.md](08-phase4-integrity-control.md) | Phase 4 specification (mincut + λ₂) |
| [09-migration-guide.md](09-migration-guide.md) | pgvector migration |
| [10-consistency-replication.md](10-consistency-replication.md) | Consistency contract, MVCC, WAL, recovery |
