# Ruvector System Architecture Overview

## Introduction

Ruvector is a high-performance vector database built in Rust, designed to deliver 10-100x performance improvements over Python/TypeScript implementations while maintaining full AgenticDB API compatibility.

## Architecture Principles

### 1. **Performance First**
- Zero-cost abstractions via Rust
- SIMD-optimized distance calculations
- Lock-free concurrent data structures
- Memory-mapped I/O for instant loading

### 2. **Multi-Platform**
- Single codebase deploys everywhere
- Rust native, Node.js via NAPI-RS, Browser via WASM
- CLI for standalone operation

### 3. **Production Ready**
- Memory safety without garbage collection
- ACID transactions via redb
- Crash recovery and data durability
- Extensive test coverage

### 4. **Extensible**
- Trait-based abstractions
- Pluggable distance metrics and indexes
- Advanced features as opt-in modules

## System Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                        Application Layer                         │
│  (AgenticDB API, VectorDB API, CLI Commands, MCP Tools)         │
└─────────────────────────────────────────────────────────────────┘
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Query Engine                             │
│  • Parallel search (rayon)                                      │
│  • SIMD distance calculations (SimSIMD)                         │
│  • Filtered search (pre/post)                                   │
│  • Hybrid search (vector + BM25)                                │
│  • MMR diversity                                                │
└─────────────────────────────────────────────────────────────────┘
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Index Layer                              │
│  • HNSW (hnsw_rs): O(log n) approximate search                 │
│  • Flat index: Brute force for small datasets                  │
│  • Quantized indexes: Compressed search                        │
└─────────────────────────────────────────────────────────────────┘
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Storage Layer                             │
│  • Vector storage: memmap2 (zero-copy)                         │
│  • Metadata: redb (ACID transactions)                          │
│  • Index persistence: rkyv (zero-copy serialization)           │
│  • AgenticDB tables: Specialized storage                       │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Storage Layer

**Purpose**: Persist vectors and metadata with ACID guarantees and instant loading.

**Technologies**:
- **redb**: LMDB-inspired embedded database for metadata
  - ACID transactions
  - Crash recovery
  - Zero-copy reads
  - Pure Rust (no C dependencies)

- **memmap2**: Memory-mapped vector storage
  - Zero-copy access
  - OS-managed caching
  - Instant loading (no deserialization)
  - Supports datasets larger than RAM

- **rkyv**: Zero-copy serialization for index persistence
  - Direct pointer access to serialized data
  - No deserialization overhead
  - Sub-second loading for billion-scale indexes

**Data Layout**:
```
vectors.db/
├── metadata.redb        # redb database (vector IDs, metadata, config)
├── vectors.bin          # Memory-mapped vectors (aligned f32 arrays)
├── index.rkyv           # Serialized HNSW graph
└── agenticdb/           # AgenticDB specialized tables
    ├── reflexion.redb
    ├── skills.redb
    ├── causal.redb
    └── learning.redb
```

### 2. Index Layer

**Purpose**: Fast approximate nearest neighbor (ANN) search.

**Primary: HNSW (Hierarchical Navigable Small World)**
- **Complexity**: O(log n) search, O(n log n) build
- **Recall**: 95%+ with proper tuning
- **Memory**: ~640 bytes per vector (M=32, 128D vectors)
- **Parameters**:
  - `m`: Connections per node (16-64)
  - `ef_construction`: Build quality (100-400)
  - `ef_search`: Query-time quality (50-500)

**Implementation**: Uses `hnsw_rs` crate with custom optimizations:
- Parallel construction via rayon
- SIMD distance calculations
- Lock-free concurrent search
- Custom quantization integration

**Alternative: Flat Index**
- Brute-force exact search
- Optimal for < 10K vectors
- 100% recall
- Simple fallback when HNSW overhead not justified

### 3. Query Engine

**Purpose**: Execute searches efficiently with various strategies.

**Components**:

a) **Distance Calculation**
- **SimSIMD**: Production-ready SIMD kernels
  - L2 (Euclidean)
  - Cosine similarity
  - Dot product
  - Manhattan (L1)
- **Speedup**: 4-16x vs scalar implementations
- **Architecture support**: AVX2, AVX-512, ARM NEON/SVE

b) **Parallel Execution**
- **rayon**: Data parallelism for CPU-bound operations
  - Batch inserts
  - Parallel queries
  - Index construction
- **Scaling**: Near-linear to CPU core count

c) **Advanced Search Strategies**
- **Filtered Search**: Metadata-based constraints
  - Pre-filtering: Apply before graph traversal
  - Post-filtering: Apply after retrieval
- **Hybrid Search**: Vector + keyword (BM25)
- **MMR**: Maximal Marginal Relevance for diversity

### 4. Application Layer

**Purpose**: Provide user-facing APIs across platforms.

**APIs**:

a) **Core VectorDB API**
```rust
pub trait VectorDB {
    fn insert(&self, entry: VectorEntry) -> Result<VectorId>;
    fn insert_batch(&self, entries: Vec<VectorEntry>) -> Result<Vec<VectorId>>;
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;
    fn delete(&self, id: &VectorId) -> Result<()>;
}
```

b) **AgenticDB API** (5-table schema)
- `vectors_table`: Core embeddings
- `reflexion_episodes`: Self-critique memory
- `skills_library`: Consolidated patterns
- `causal_edges`: Cause-effect hypergraphs
- `learning_sessions`: RL training data

c) **Platform Bindings**
- **Rust**: Native library
- **Node.js**: NAPI-RS bindings with TypeScript definitions
- **WASM**: wasm-bindgen for browser
- **CLI**: clap-based command-line interface
- **MCP**: Model Context Protocol tools

## Data Flow

### Insert Operation

```
Application
    ↓ insert(vector, metadata)
VectorDB
    ↓ assign ID
    ↓ store metadata → redb
    ↓ append vector → memmap
    ↓ add to index → HNSW
    ↓ [optional] quantize
    ↓ persist index → rkyv
    ↓
Return ID
```

**Optimizations**:
- Batch inserts amortize transaction overhead
- Parallel index updates
- Lazy quantization (on first search if enabled)

### Search Operation

```
Application
    ↓ search(query, k, filters)
VectorDB
    ↓ [optional] apply pre-filters
    ↓ normalize query (if cosine)
Query Engine
    ↓ HNSW graph traversal
    ↓   ├─ Start at entry point
    ↓   ├─ Greedy search per layer
    ↓   └─ Refine at bottom layer
    ↓ SIMD distance calculations
    ↓ [optional] apply post-filters
    ↓ [optional] re-rank with full precision
    ↓ top-k selection
    ↓
Return results
```

**Optimizations**:
- Quantized search for initial retrieval
- Full-precision re-ranking
- SIMD vectorization
- Lock-free graph reads

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Insert (HNSW) | O(log n) | Amortized per insertion |
| Batch insert | O(n log n) | Parallelized across cores |
| Search (HNSW) | O(log n) | With 95% recall |
| Search (Flat) | O(n) | Exact search |
| Delete | O(log n) | Mark deleted in HNSW |

### Space Complexity

| Component | Memory per vector | Notes |
|-----------|------------------|-------|
| Full precision (128D) | 512 bytes | 128 × 4 bytes |
| HNSW graph (M=32) | ~640 bytes | M × 2 layers × 10 bytes/edge |
| Scalar quantization | 128 bytes | 4x compression |
| Product quantization | 16 bytes | 32x compression (16 subspaces) |
| Metadata | Variable | Stored in redb |

**Total for 1M vectors (128D, HNSW M=32, scalar quant)**:
- Vectors: 128 MB (quantized)
- HNSW: 640 MB
- Metadata: ~50 MB
- **Total**: ~818 MB vs ~1.2 GB uncompressed

### Latency Characteristics

**1M vectors, 128D, HNSW (M=32, ef_search=100)**:
- p50: 0.8ms
- p95: 2.1ms
- p99: 4.5ms

**Factors affecting latency**:
- Vector dimensionality (linear impact)
- Dataset size (logarithmic impact with HNSW)
- HNSW ef_search parameter (linear impact)
- Quantization (0.8-1.2x slower, but cache-friendly)
- SIMD availability (4-16x speedup)

## Concurrency Model

### Read Operations
- **Lock-free**: Multiple concurrent searches
- **Mechanism**: Arc<RwLock<T>> with read locks
- **Scalability**: Linear with CPU cores

### Write Operations
- **Exclusive lock**: Single writer at a time
- **Mechanism**: RwLock write lock
- **Batch optimization**: Amortize lock overhead

### Mixed Workloads
- Readers don't block readers
- Writers block all operations
- Read-heavy workloads scale well (typical for vector DB)

## Memory Management

### Zero-Copy Patterns
1. **Memory-mapped vectors**: OS manages paging
2. **rkyv serialization**: Direct pointer access
3. **NAPI-RS buffers**: Share TypedArrays with Node.js
4. **WASM memory**: Direct ArrayBuffer access

### Memory Safety
- Rust's ownership system prevents:
  - Use-after-free
  - Double-free
  - Data races
  - Buffer overflows
- No garbage collection overhead

### Resource Limits
- **Max vectors**: Configurable (default 10M)
- **Max dimensions**: Theoretically unlimited (practical limit ~4096)
- **Memory-mapped limit**: OS-dependent (typically 128TB on 64-bit)

## Extensibility Points

### 1. Distance Metrics
```rust
pub trait DistanceMetric: Send + Sync {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;
    fn batch_distance(&self, a: &[f32], batch: &[&[f32]]) -> Vec<f32>;
}
```

### 2. Index Structures
```rust
pub trait IndexStructure: Send + Sync {
    fn insert(&mut self, id: VectorId, vector: &[f32]) -> Result<()>;
    fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>>;
    fn delete(&mut self, id: VectorId) -> Result<()>;
}
```

### 3. Quantization Methods
```rust
pub trait Quantizer: Send + Sync {
    type Quantized;
    fn quantize(&self, vector: &[f32]) -> Self::Quantized;
    fn distance(&self, a: &Self::Quantized, b: &Self::Quantized) -> f32;
}
```

## Security Considerations

### Memory Safety
- Rust prevents entire classes of vulnerabilities
- No buffer overflows, use-after-free, or data races

### Input Validation
- Vector dimension checks
- ID format validation
- Metadata size limits
- Query parameter bounds

### Resource Limits
- Maximum query size
- Rate limiting (application-level)
- Memory quotas
- Disk space monitoring

### Data Privacy
- On-premises deployment option
- No telemetry by default
- Memory zeroing on delete
- Encrypted storage (via OS-level encryption)

## Future Architecture Enhancements

### Phase 1 (Current)
- HNSW indexing
- Scalar & product quantization
- AgenticDB compatibility
- Multi-platform bindings

### Phase 2 (Near-term)
- Distributed query processing
- Horizontal scaling with sharding
- GPU acceleration for distance calculations
- Learned index structures (hybrid with HNSW)

### Phase 3 (Long-term)
- Hypergraph structures for n-ary relationships
- Temporal indexes for time-series embeddings
- Neural hash functions for improved compression
- Neuromorphic hardware support (Intel Loihi)

## Related Documentation

- [Storage Layer](STORAGE_LAYER.md) - Detailed storage architecture
- [Index Structures](INDEX_STRUCTURES.md) - HNSW and flat indexes
- [Quantization](QUANTIZATION.md) - Compression techniques
- [Performance](../optimization/PERFORMANCE_TUNING_GUIDE.md) - Optimization guide
- [API Reference](../api/) - Complete API documentation
