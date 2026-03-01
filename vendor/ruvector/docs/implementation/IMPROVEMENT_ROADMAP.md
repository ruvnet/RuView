# rUvector Improvement Roadmap

Based on analysis of Qdrant's production-ready features and industry best practices, here's a prioritized roadmap to enhance rUvector.

---

## Priority 1: Production Essentials (Critical)

### 1.1 REST/gRPC API Server

**Current State:** CLI-only, no network API
**Target:** Full REST + gRPC server with OpenAPI spec

```rust
// Proposed: crates/ruvector-server/
pub struct RuvectorServer {
    db: Arc<VectorDB>,
    rest_port: u16,    // Default: 6333
    grpc_port: u16,    // Default: 6334
}

// REST endpoints
POST   /collections                    // Create collection
GET    /collections                    // List collections
DELETE /collections/{name}             // Delete collection
PUT    /collections/{name}/points      // Upsert points
POST   /collections/{name}/points/search  // Search
DELETE /collections/{name}/points/{id} // Delete point
```

**Implementation:**
- Use `axum` for REST (async, tower middleware)
- Use `tonic` for gRPC (protobuf, streaming)
- OpenAPI spec generation via `utoipa`
- Swagger UI at `/docs`

**Effort:** 2-3 weeks

---

### 1.2 Advanced Payload Indexing

**Current State:** Basic metadata filtering (HashMap comparison)
**Target:** 9 index types like Qdrant

```rust
// New: crates/ruvector-core/src/payload_index.rs

pub enum PayloadIndex {
    // Numeric (range queries)
    Integer(BTreeMap<i64, Vec<VectorId>>),
    Float(IntervalTree<f64, VectorId>),
    DateTime(BTreeMap<i64, Vec<VectorId>>),

    // Exact match (O(1) lookup)
    Keyword(HashMap<String, Vec<VectorId>>),
    Uuid(HashMap<Uuid, Vec<VectorId>>),

    // Full-text search
    FullText {
        index: tantivy::Index,
        tokenizer: TokenizerType,
    },

    // Geo-spatial
    Geo(RTree<VectorId>),

    // Boolean
    Bool(HashMap<bool, Vec<VectorId>>),
}

pub enum FilterExpression {
    // Comparison
    Eq(String, Value),
    Ne(String, Value),
    Gt(String, Value),
    Gte(String, Value),
    Lt(String, Value),
    Lte(String, Value),

    // Range
    Range { field: String, gte: Option<Value>, lte: Option<Value> },

    // Geo
    GeoRadius { field: String, center: GeoPoint, radius_m: f64 },
    GeoBoundingBox { field: String, top_left: GeoPoint, bottom_right: GeoPoint },

    // Text
    Match { field: String, text: String },
    MatchPhrase { field: String, phrase: String },

    // Logical
    And(Vec<FilterExpression>),
    Or(Vec<FilterExpression>),
    Not(Box<FilterExpression>),
}
```

**Dependencies:**
- `tantivy` for full-text search
- `rstar` for R-tree geo indexing
- `intervallum` for interval trees

**Effort:** 3-4 weeks

---

### 1.3 Collection Management

**Current State:** Single implicit collection per database
**Target:** Multi-collection support with aliases

```rust
// New: crates/ruvector-core/src/collection.rs

pub struct CollectionManager {
    collections: DashMap<String, Collection>,
    aliases: DashMap<String, String>,  // alias -> collection name
}

pub struct Collection {
    name: String,
    config: CollectionConfig,
    index: HnswIndex,
    payload_indices: HashMap<String, PayloadIndex>,
    stats: CollectionStats,
}

pub struct CollectionConfig {
    dimensions: usize,
    distance_metric: DistanceMetric,
    hnsw_config: HnswConfig,
    quantization: Option<QuantizationConfig>,
    on_disk_payload: bool,  // Store payloads on disk vs RAM
    replication_factor: u32,
    write_consistency: u32,
}

impl CollectionManager {
    // CRUD operations
    pub fn create_collection(&self, name: &str, config: CollectionConfig) -> Result<()>;
    pub fn delete_collection(&self, name: &str) -> Result<()>;
    pub fn get_collection(&self, name: &str) -> Option<Arc<Collection>>;
    pub fn list_collections(&self) -> Vec<String>;

    // Alias management
    pub fn create_alias(&self, alias: &str, collection: &str) -> Result<()>;
    pub fn delete_alias(&self, alias: &str) -> Result<()>;
    pub fn switch_alias(&self, alias: &str, new_collection: &str) -> Result<()>;
}
```

**Effort:** 1-2 weeks

---

### 1.4 Snapshots & Backup

**Current State:** No backup capability
**Target:** Collection snapshots with S3 support

```rust
// New: crates/ruvector-core/src/snapshot.rs

pub struct SnapshotManager {
    storage: Box<dyn SnapshotStorage>,
}

pub trait SnapshotStorage: Send + Sync {
    fn create(&self, collection: &Collection) -> Result<SnapshotId>;
    fn restore(&self, id: &SnapshotId, target: &str) -> Result<Collection>;
    fn list(&self) -> Result<Vec<SnapshotInfo>>;
    fn delete(&self, id: &SnapshotId) -> Result<()>;
}

// Implementations
pub struct LocalSnapshotStorage { base_path: PathBuf }
pub struct S3SnapshotStorage { bucket: String, client: S3Client }

pub struct Snapshot {
    id: SnapshotId,
    collection_name: String,
    config: CollectionConfig,
    vectors: Vec<(VectorId, Vec<f32>)>,
    payloads: HashMap<VectorId, Value>,
    created_at: DateTime<Utc>,
    checksum: String,
}
```

**Effort:** 2 weeks

---

## Priority 2: Scalability (High)

### 2.1 Distributed Mode (Sharding)

**Current State:** Single-node only
**Target:** Horizontal scaling with sharding

```rust
// New: crates/ruvector-cluster/

pub struct ClusterConfig {
    node_id: NodeId,
    peers: Vec<PeerAddress>,
    replication_factor: u32,
    shards_per_collection: u32,
}

pub struct ShardManager {
    local_shards: HashMap<ShardId, Shard>,
    shard_routing: ConsistentHash<NodeId>,
}

pub enum ShardingStrategy {
    // Automatic hash-based distribution
    Hash { num_shards: u32 },
    // User-defined shard keys
    Custom { shard_key_field: String },
}

// Shard placement
pub struct Shard {
    id: ShardId,
    collection: String,
    replica_set: Vec<NodeId>,
    state: ShardState,  // Active, Partial, Dead, Initializing
}
```

**Components:**
- Consistent hashing for shard routing
- gRPC for inter-node communication
- Write-ahead log for durability

**Effort:** 6-8 weeks

---

### 2.2 Raft Consensus (Metadata)

**Current State:** No consensus
**Target:** Raft for cluster metadata

```rust
// Use: raft-rs or openraft crate

pub struct RaftNode {
    id: NodeId,
    state_machine: ClusterStateMachine,
    log: RaftLog,
}

// Raft manages:
// - Collection creation/deletion
// - Shard assignments
// - Node membership
// - NOT point operations (bypass for performance)
```

**Effort:** 4-6 weeks

---

### 2.3 Replication

**Current State:** No replication
**Target:** Configurable replication factor

```rust
pub struct ReplicationManager {
    factor: u32,
    write_consistency: WriteConsistency,
}

pub enum WriteConsistency {
    One,      // Ack after 1 replica
    Quorum,   // Ack after majority
    All,      // Ack after all replicas
}

// Replication states
pub enum ReplicaState {
    Active,       // Serving reads and writes
    Partial,      // Catching up
    Dead,         // Unreachable
    Listener,     // Read-only replica
}
```

**Effort:** 3-4 weeks

---

## Priority 3: Enterprise Features (Medium)

### 3.1 Authentication & RBAC

**Current State:** No authentication
**Target:** API keys + JWT RBAC

```rust
// New: crates/ruvector-auth/

pub struct AuthConfig {
    api_key: Option<String>,
    jwt_secret: Option<String>,
    rbac_enabled: bool,
}

pub struct JwtClaims {
    sub: String,           // User ID
    exp: u64,              // Expiration
    collections: Vec<CollectionAccess>,
}

pub struct CollectionAccess {
    collection: String,    // Collection name or "*"
    permissions: Permissions,
}

bitflags! {
    pub struct Permissions: u32 {
        const READ = 0b0001;
        const WRITE = 0b0010;
        const DELETE = 0b0100;
        const ADMIN = 0b1000;
    }
}
```

**Effort:** 2 weeks

---

### 3.2 TLS Support

**Current State:** No encryption
**Target:** TLS for client and inter-node

```rust
pub struct TlsConfig {
    // Server TLS
    cert_path: PathBuf,
    key_path: PathBuf,
    ca_cert_path: Option<PathBuf>,

    // Client verification
    require_client_cert: bool,

    // Inter-node TLS
    cluster_tls_enabled: bool,
}
```

**Implementation:**
- Use `rustls` for TLS
- Support mTLS for cluster communication
- ACME/Let's Encrypt integration

**Effort:** 1 week

---

### 3.3 Metrics & Observability

**Current State:** Basic stats only
**Target:** Prometheus + OpenTelemetry

```rust
// New: crates/ruvector-metrics/

pub struct MetricsConfig {
    prometheus_port: u16,  // Default: 9090
    otlp_endpoint: Option<String>,
}

// Metrics to expose
lazy_static! {
    static ref SEARCH_LATENCY: HistogramVec = register_histogram_vec!(
        "ruvector_search_latency_seconds",
        "Search latency in seconds",
        &["collection", "quantile"]
    ).unwrap();

    static ref VECTORS_TOTAL: IntGaugeVec = register_int_gauge_vec!(
        "ruvector_vectors_total",
        "Total vectors stored",
        &["collection"]
    ).unwrap();

    static ref QPS: CounterVec = register_counter_vec!(
        "ruvector_queries_total",
        "Total queries processed",
        &["collection", "status"]
    ).unwrap();
}
```

**Endpoints:**
- `/metrics` - Prometheus format
- `/health` - Health check
- `/ready` - Readiness probe

**Effort:** 1 week

---

## Priority 4: Performance Enhancements (Medium)

### 4.1 Asymmetric Quantization

**Current State:** Symmetric quantization only
**Target:** Different quantization for storage vs query

```rust
// Qdrant 1.15+ feature

pub struct AsymmetricQuantization {
    // Storage: Binary (32x compression)
    storage_quantization: QuantizationConfig::Binary,
    // Query: Scalar (better precision)
    query_quantization: QuantizationConfig::Scalar,
}

// Benefits:
// - Storage/RAM: Binary compression (32x)
// - Precision: Improved via scalar query quantization
// - Use case: Memory-constrained deployments
```

**Effort:** 1 week

---

### 4.2 1.5-bit and 2-bit Quantization

**Current State:** 1-bit binary only
**Target:** Variable bit-width quantization

```rust
pub enum QuantizationBits {
    OneBit,      // 32x compression, ~90% recall
    OnePointFive, // 21x compression, ~93% recall
    TwoBit,      // 16x compression, ~95% recall
    FourBit,     // 8x compression, ~98% recall
    EightBit,    // 4x compression, ~99% recall
}
```

**Effort:** 2 weeks

---

### 4.3 On-Disk Vector Storage

**Current State:** Memory-only or full mmap
**Target:** Tiered storage (hot/warm/cold)

```rust
pub struct TieredStorage {
    // Hot: In-memory, frequently accessed
    hot_cache: LruCache<VectorId, Vec<f32>>,

    // Warm: Memory-mapped, recent
    mmap_storage: MmapStorage,

    // Cold: Disk-only, archival
    disk_storage: DiskStorage,
}

pub struct StoragePolicy {
    hot_threshold_days: u32,
    warm_threshold_days: u32,
    max_memory_gb: f64,
}
```

**Effort:** 3 weeks

---

## Priority 5: Developer Experience (Low)

### 5.1 Client SDKs

**Current State:** Node.js only
**Target:** Multi-language SDKs

| Language | Priority | Approach |
|----------|----------|----------|
| Python | High | Native (PyO3) |
| Go | High | gRPC client |
| Java | Medium | gRPC client |
| C#/.NET | Medium | gRPC client |
| TypeScript | Low | REST client (existing) |

**Python SDK Example:**
```python
from ruvector import RuvectorClient

client = RuvectorClient(url="http://localhost:6333")

# Create collection
client.create_collection(
    name="my_collection",
    dimensions=384,
    distance="cosine"
)

# Insert vectors
client.upsert(
    collection="my_collection",
    points=[
        {"id": "1", "vector": [...], "payload": {"type": "doc"}}
    ]
)

# Search
results = client.search(
    collection="my_collection",
    query_vector=[...],
    limit=10,
    filter={"type": "doc"}
)
```

**Effort:** 2 weeks per SDK

---

### 5.2 Web Dashboard

**Current State:** CLI only
**Target:** Browser-based management UI

```
/dashboard
├── Collections
│   ├── List all collections
│   ├── Collection details
│   ├── Index visualization
│   └── Query builder
├── Monitoring
│   ├── QPS charts
│   ├── Latency histograms
│   └── Memory/disk usage
├── Cluster
│   ├── Node status
│   ├── Shard distribution
│   └── Replication status
└── Settings
    ├── Authentication
    ├── TLS configuration
    └── Backup schedules
```

**Implementation:**
- Svelte or React frontend
- Embedded in server binary
- Served at `/dashboard`

**Effort:** 4-6 weeks

---

### 5.3 Migration Tools

**Current State:** TODOs for FAISS, Pinecone, Weaviate
**Target:** Import/export utilities

```bash
# Import from other databases
ruvector import --from faiss --input index.faiss --collection my_collection
ruvector import --from pinecone --api-key $KEY --index my_index
ruvector import --from weaviate --url http://localhost:8080 --class Article
ruvector import --from qdrant --url http://localhost:6333 --collection docs

# Export
ruvector export --collection my_collection --format jsonl --output data.jsonl
ruvector export --collection my_collection --format parquet --output data.parquet
```

**Effort:** 1-2 weeks per format

---

## Implementation Timeline

### Phase 1: Q1 (12 weeks)
- [x] Benchmark comparison (completed)
- [ ] REST/gRPC API server
- [ ] Collection management
- [ ] Advanced filtering
- [ ] Snapshots

### Phase 2: Q2 (12 weeks)
- [ ] Distributed mode (sharding)
- [ ] Replication
- [ ] Authentication/RBAC
- [ ] Metrics/observability

### Phase 3: Q3 (12 weeks)
- [ ] Raft consensus
- [ ] Python SDK
- [ ] Web dashboard
- [ ] Migration tools

### Phase 4: Q4 (12 weeks)
- [ ] Tiered storage
- [ ] Advanced quantization
- [ ] Additional SDKs
- [ ] Cloud deployment guides

---

## Quick Wins (Can Implement Now)

### 1. Add OpenAPI Spec (1 day)
```yaml
# openapi.yaml
openapi: 3.0.0
info:
  title: rUvector API
  version: 0.1.0
paths:
  /collections:
    post:
      summary: Create collection
      ...
```

### 2. Health Endpoints (2 hours)
```rust
// Add to CLI server
GET /health  -> { "status": "ok" }
GET /ready   -> { "status": "ready", "collections": 5 }
```

### 3. Basic Prometheus Metrics (1 day)
```rust
use prometheus::{Counter, Histogram, register_counter, register_histogram};
```

### 4. Collection Aliases (3 hours)
```rust
// Simple HashMap wrapper
aliases: HashMap<String, String>
```

### 5. Geo Filtering (2 days)
```rust
// Add rstar dependency
use rstar::RTree;
```

---

## Summary: Feature Gap Analysis

| Feature | Qdrant | rUvector | Gap |
|---------|--------|----------|-----|
| REST API | ✅ | ❌ | **Critical** |
| gRPC API | ✅ | ❌ | **Critical** |
| Multi-collection | ✅ | ❌ | **Critical** |
| Payload indexing | ✅ (9 types) | ⚠️ (basic) | **High** |
| Snapshots | ✅ | ❌ | **High** |
| Distributed | ✅ | ❌ | Medium |
| Replication | ✅ | ❌ | Medium |
| RBAC | ✅ | ❌ | Medium |
| TLS | ✅ | ❌ | Medium |
| Metrics | ✅ | ⚠️ (basic) | Medium |
| Web UI | ✅ | ❌ | Low |
| Python SDK | ✅ | ❌ | Low |

**rUvector Advantages to Preserve:**
- ✅ 22x faster search (keep SIMD/SimSIMD)
- ✅ WASM support (browser deployment)
- ✅ Hypergraph/Neural hash (unique features)
- ✅ AgenticDB API (AI-native)
- ✅ Sub-100µs latency (embedded use)

---

## Next Steps

1. **Immediate:** Implement REST API server (axum)
2. **This Week:** Add collection management
3. **This Month:** Advanced filtering + snapshots
4. **This Quarter:** Distributed mode basics

The goal is to match Qdrant's production readiness while preserving rUvector's performance advantages and unique AI-native features.
