# ADR-001: Ruvector Core Architecture

**Status**: Proposed
**Date**: 2026-01-18
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

**Note**: The storage layer described in this ADR is superseded by ADR-029 (RVF as Canonical Binary Format). All vector persistence now uses the RVF segment model.

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-18 | ruv.io | Initial architecture proposal |

---

## Context

### The Vector Database Challenge

Modern AI applications require vector databases that can:

1. **Store high-dimensional embeddings** from LLMs and embedding models
2. **Search with sub-millisecond latency** for real-time inference
3. **Scale to billions of vectors** while maintaining performance
4. **Deploy anywhere** - edge devices, browsers (WASM), cloud servers
5. **Integrate seamlessly** with LLM inference pipelines

### Current State of Vector Databases

Existing solutions fall into several categories:

| Category | Examples | Limitations |
|----------|----------|-------------|
| **Cloud-only** | Pinecone | No edge deployment, vendor lock-in |
| **Heavy native** | Milvus, Qdrant | Complex deployment, high memory |
| **Python-first** | ChromaDB, FAISS | Performance overhead, no WASM |
| **Learning-capable** | None | No existing solutions learn from usage |

### The Ruvector Vision

Ruvector is designed as a **high-performance, learning-capable vector database** implemented in Rust that:

- Achieves **61us p50 latency** for k=10 search on 384-dim vectors
- Provides **2-32x memory compression** through tiered quantization
- Runs **anywhere** - native (x86_64, ARM64), WASM (browser, edge), PostgreSQL extension
- **Learns from usage** via GNN layers that improve search quality over time
- Integrates with **AI agent memory systems** for policy, session state, and audit logs

---

## Decision

### Adopt a Layered, SIMD-Optimized Architecture

We implement ruvector-core as the foundational vector database engine with the following architecture:

```
+-----------------------------------------------------------------------------+
|                              APPLICATION LAYER                               |
|  AgenticDB | VectorDB API | Cypher Queries | REST/gRPC Server               |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                              INDEX LAYER                                     |
|  HNSW Index | Flat Index | Filtered Search | Hybrid Search | MMR            |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                              QUANTIZATION LAYER                              |
|  Scalar (4x) | Product (8-16x) | Binary (32x) | Conformal Prediction        |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                              DISTANCE LAYER                                  |
|  Euclidean | Cosine | Dot Product | Manhattan | SIMD Dispatch               |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                              SIMD INTRINSICS LAYER                           |
|  AVX2/AVX-512 (x86_64) | NEON (ARM64/Apple Silicon) | Scalar Fallback       |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                              STORAGE LAYER                                   |
|  REDB (native) | Memory-only (WASM) | PostgreSQL Extension                  |
+-----------------------------------------------------------------------------+
```

---

## Key Components

### 1. SIMD Intrinsics Layer (`simd_intrinsics.rs`)

The performance foundation of ruvector, providing hardware-accelerated distance calculations.

#### Architecture Dispatch

```rust
pub fn euclidean_distance_simd(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { euclidean_distance_avx2_impl(a, b) }
        } else {
            euclidean_distance_scalar(a, b)
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        unsafe { euclidean_distance_neon_impl(a, b) }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        euclidean_distance_scalar(a, b)
    }
}
```

#### Supported Operations

| Operation | AVX2 (x86_64) | NEON (ARM64) | Scalar Fallback |
|-----------|---------------|--------------|-----------------|
| Euclidean Distance | 8 floats/cycle | 4 floats/cycle | 1 float/cycle |
| Dot Product | 8 floats/cycle | 4 floats/cycle | 1 float/cycle |
| Cosine Similarity | 8 floats/cycle | 4 floats/cycle | 1 float/cycle |
| Manhattan Distance | N/A | 4 floats/cycle | 1 float/cycle |

#### Performance Characteristics

| Metric | AVX2 | NEON | Scalar |
|--------|------|------|--------|
| **512-dim Euclidean** | ~16M ops/sec | ~8M ops/sec | ~2M ops/sec |
| **384-dim Cosine** | ~143ns | ~200ns | ~800ns |
| **1536-dim Dot Product** | ~33ns | ~50ns | ~150ns |

#### Security Guarantees

- Bounds checking via `assert_eq!(a.len(), b.len())` prevents buffer overflows
- Unaligned loads (`_mm256_loadu_ps`, `vld1q_f32`) handle arbitrary alignment
- Scalar fallback handles remainder elements after SIMD processing

### 2. Distance Metrics Layer (`distance.rs`)

High-level distance API with optional SimSIMD integration for additional acceleration.

#### Supported Metrics

```rust
pub enum DistanceMetric {
    Euclidean,   // L2 distance: sqrt(sum((a[i] - b[i])^2))
    Cosine,      // 1 - cosine_similarity
    DotProduct,  // Negative dot product (for maximization)
    Manhattan,   // L1 distance: sum(|a[i] - b[i]|)
}
```

#### Feature Flags

| Feature | Description | Use Case |
|---------|-------------|----------|
| `simd` | SimSIMD acceleration | Native builds |
| `parallel` | Rayon batch processing | Multi-core systems |
| None | Pure Rust fallback | WASM builds |

#### Batch Distance API

```rust
pub fn batch_distances(
    query: &[f32],
    vectors: &[Vec<f32>],
    metric: DistanceMetric,
) -> Result<Vec<f32>> {
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        use rayon::prelude::*;
        vectors.par_iter()
            .map(|v| distance(query, v, metric))
            .collect()
    }
    // Sequential fallback for WASM...
}
```

### 3. Index Structures (`index/`)

#### HNSW Index (`index/hnsw.rs`)

Hierarchical Navigable Small World graph for approximate nearest neighbor search.

**Configuration Parameters:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `m` | 32 | Connections per layer (higher = better recall, more memory) |
| `ef_construction` | 200 | Build-time search depth (higher = better graph, slower build) |
| `ef_search` | 100 | Query-time search depth (higher = better recall, slower query) |
| `max_elements` | 10M | Pre-allocated capacity |

**Complexity Analysis:**

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Insert | O(log n * m * ef_construction) | O(m * log n) per vector |
| Search | O(log n * m * ef_search) | O(ef_search) |
| Delete | O(1)* | O(1) |

*Note: HNSW deletion marks vectors as removed but does not restructure the graph.

**Serialization:**

```rust
pub struct HnswState {
    vectors: Vec<(String, Vec<f32>)>,
    id_to_idx: Vec<(String, usize)>,
    idx_to_id: Vec<(usize, String)>,
    next_idx: usize,
    config: SerializableHnswConfig,
    dimensions: usize,
    metric: SerializableDistanceMetric,
}
```

#### Flat Index

Linear scan index for small datasets or exact search.

**Use Cases:**
- Datasets < 10K vectors
- Exact k-NN required
- Benchmarking HNSW recall

### 4. Quantization Strategies (`quantization.rs`)

Memory compression techniques trading precision for storage efficiency.

#### Scalar Quantization (4x compression)

Quantizes f32 to u8 using min-max scaling.

```rust
pub struct ScalarQuantized {
    pub data: Vec<u8>,     // Quantized values
    pub min: f32,          // Minimum for dequantization
    pub scale: f32,        // Scale factor
}
```

**Characteristics:**
- Compression: 4x (f32 -> u8)
- Distance calculation: Uses average scale for symmetric distance
- Reconstruction error: < 0.4% for typical embedding distributions

#### Product Quantization (8-16x compression)

Divides vectors into subspaces, each quantized independently via k-means codebooks.

```rust
pub struct ProductQuantized {
    pub codes: Vec<u8>,                    // One code per subspace
    pub codebooks: Vec<Vec<Vec<f32>>>,     // Learned centroids
}
```

**Training:**
- K-means clustering on subspace vectors
- Codebook size typically 256 (fits in u8)
- Iterations: 10-100 for convergence

#### Binary Quantization (32x compression)

Single-bit representation based on sign.

```rust
pub struct BinaryQuantized {
    pub bits: Vec<u8>,      // Packed bits (8 dimensions per byte)
    pub dimensions: usize,
}
```

**Characteristics:**
- Compression: 32x (f32 -> 1 bit)
- Distance: Hamming distance (XOR + popcount)
- Best for: Filtering stage before exact distance on candidates

#### Tiered Compression Strategy

Ruvector automatically manages compression based on access patterns:

| Access Frequency | Format | Compression | Latency |
|-----------------|--------|-------------|---------|
| Hot (>80%) | f32 | 1x | Instant |
| Warm (40-80%) | f16 | 2x | ~1us |
| Cool (10-40%) | Scalar | 4x | ~10us |
| Cold (1-10%) | Product | 8-16x | ~100us |
| Archive (<1%) | Binary | 32x | ~1ms |

### 5. Memory Management

#### Arena Allocator (`arena.rs`)

Bump allocator for batch operations reducing allocation overhead.

#### Lock-Free Structures (`lockfree.rs`)

- Crossbeam-based concurrent data structures
- Lock-free queues for batch ingestion
- Available only on `parallel` feature (not WASM)

#### Cache-Optimized Operations (`cache_optimized.rs`)

- Prefetching hints for sequential access
- Cache-line aligned storage
- NUMA-aware allocation on supported platforms

### 6. Storage Layer (`storage.rs`)

#### Native Storage (REDB)

- ACID transactions
- Memory-mapped vectors
- Configuration persistence
- Connection pooling for multiple VectorDB instances

```rust
const VECTORS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("vectors");
const METADATA_TABLE: TableDefinition<&str, &str> = TableDefinition::new("metadata");
const CONFIG_TABLE: TableDefinition<&str, &str> = TableDefinition::new("config");
```

**Security:**
- Path traversal protection
- Validates relative paths don't escape working directory

#### Memory-Only Storage (`storage_memory.rs`)

- Pure in-memory for WASM
- No persistence
- DashMap for concurrent access

---

## Integration Points

### 1. Policy Memory Store

Ruvector serves as the backing store for AI agent policy memory:

```
+-------------------+       +-------------------+       +-------------------+
|   AI Agent        |       |   Policy Memory   |       |   ruvector-core   |
|                   | ----> |   (AgenticDB)     | ----> |                   |
| "What action for  |       | Search similar    |       | HNSW search       |
|  this situation?" |       | past situations   |       | with metadata     |
+-------------------+       +-------------------+       +-------------------+
```

**Use Cases:**
- Q-learning state-action lookups
- Contextual bandit policy retrieval
- Episodic memory for reasoning

### 2. Session State Index

Real-time session context for conversational AI:

```
+-------------------+       +-------------------+       +-------------------+
|   Chat Session    |       |   Session Index   |       |   ruvector-core   |
|                   | ----> |                   | ----> |                   |
| Current context   |       | Find relevant     |       | Cosine similarity |
| embedding         |       | past turns        |       | top-k search      |
+-------------------+       +-------------------+       +-------------------+
```

**Requirements:**
- < 10ms latency for interactive use
- Session isolation via namespaces
- TTL-based cleanup

### 3. Witness Log for Audit

Cryptographically-linked audit trail:

```
+-------------------+       +-------------------+       +-------------------+
|   Agent Action    |       |   Witness Log     |       |   ruvector-core   |
|                   | ----> |                   | ----> |                   |
| Action embedding  |       | Store with hash   |       | Append-only       |
| + metadata        |       | chain reference   |       | with timestamps   |
+-------------------+       +-------------------+       +-------------------+
```

**Properties:**
- Immutable entries
- Hash-chain linking
- Semantic searchability

---

## Decision Drivers

### 1. Performance (Sub-millisecond Latency)

| Requirement | Implementation |
|-------------|----------------|
| 61us p50 search | SIMD-optimized distance + HNSW |
| 16,400 QPS | Parallel search with Rayon |
| Batch ingestion | Lock-free queues + bulk insert |

### 2. Memory Efficiency (Quantization Support)

| Requirement | Implementation |
|-------------|----------------|
| 4x compression | Scalar quantization |
| 8-16x compression | Product quantization |
| 32x compression | Binary quantization |
| Automatic tiering | Access pattern tracking |

### 3. Cross-Platform Portability (WASM, Native)

| Platform | Features Available |
|----------|-------------------|
| x86_64 Linux/macOS | Full (SIMD, parallel, storage) |
| ARM64 macOS (Apple Silicon) | Full (NEON, parallel, storage) |
| WASM (browser) | Memory-only, scalar fallback |
| PostgreSQL extension | Full + SQL integration |

### 4. LLM Integration

| Requirement | Implementation |
|-------------|----------------|
| Embedding ingestion | API-based and local providers |
| Semantic search | Cosine/dot product metrics |
| RAG pipeline | Hybrid search + metadata filtering |

---

## Alternatives Considered

### Alternative 1: Pure Python Implementation (NumPy/FAISS)

**Rejected because:**
- 10-100x slower than Rust SIMD
- No WASM support
- GIL contention in concurrent workloads

### Alternative 2: C++ with Bindings

**Rejected because:**
- Memory safety concerns
- Complex cross-compilation
- Build system complexity (CMake)

### Alternative 3: Qdrant/Milvus Integration

**Rejected because:**
- External service dependency
- No WASM support
- Complex deployment for edge use cases

### Alternative 4: GPU-Only Acceleration (CUDA/ROCm)

**Rejected because:**
- Not portable to edge/mobile
- Driver dependencies
- Overkill for < 100M vectors

---

## Consequences

### Benefits

1. **Performance**: Sub-millisecond latency enables real-time AI applications
2. **Portability**: Single codebase runs native, WASM, and PostgreSQL
3. **Memory Efficiency**: 2-32x compression makes large datasets practical on edge
4. **Integration**: Native Rust means zero-cost abstractions for embedding in other systems
5. **Learning**: GNN layers can improve search quality without reindexing

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| HNSW recall < 100% | High | Medium | ef_search tuning, hybrid with exact search |
| Quantization accuracy loss | Medium | Medium | Conformal prediction bounds |
| WASM performance gap | Medium | Low | Specialized WASM-optimized builds |
| API embeddings require external call | High | Low | Local embedding option via ONNX |

### Performance Targets

| Metric | Target | Achieved |
|--------|--------|----------|
| HNSW Search (k=10, 384-dim) | < 100us p50 | 61us |
| HNSW Search (k=100, 384-dim) | < 200us p50 | 164us |
| Cosine Distance (1536-dim) | < 200ns | 143ns |
| Dot Product (384-dim) | < 50ns | 33ns |
| Batch Distance (1000 vectors) | < 500us | 237us |
| QPS (10K vectors, k=10) | > 10K | 16,400 |

---

## Implementation Status

### Completed (v0.1.x)

| Module | Status | Description |
|--------|--------|-------------|
| `simd_intrinsics` | Complete | AVX2/NEON dispatch with scalar fallback |
| `distance` | Complete | All 4 metrics with SimSIMD integration |
| `index/hnsw` | Complete | Full HNSW with serialization |
| `index/flat` | Complete | Linear scan baseline |
| `quantization` | Complete | Scalar, Product, Binary |
| `storage` | Complete | REDB-based with connection pooling |
| `storage_memory` | Complete | In-memory for WASM |
| `types` | Complete | Core types with serde |
| `error` | Complete | Error types with thiserror |
| `vector_db` | Complete | High-level API |
| `agenticdb` | Complete | AI agent memory interface |

### Advanced Features

| Module | Status | Description |
|--------|--------|-------------|
| `advanced_features/filtered_search` | Complete | Metadata-based filtering |
| `advanced_features/hybrid_search` | Complete | Dense + sparse (BM25) |
| `advanced_features/mmr` | Complete | Maximal Marginal Relevance |
| `advanced_features/conformal_prediction` | Complete | Uncertainty quantification |
| `advanced_features/product_quantization` | Complete | Enhanced PQ with training |

### Research Features (`advanced/`)

| Module | Status | Description |
|--------|--------|-------------|
| `hypergraph` | Experimental | Hyperedge relationships |
| `learned_index` | Experimental | Neural index structures |
| `neural_hash` | Experimental | LSH with neural tuning |
| `tda` | Experimental | Topological data analysis |

---

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `default` | Yes | simd, storage, hnsw, api-embeddings, parallel |
| `simd` | Yes | SimSIMD acceleration |
| `parallel` | Yes | Rayon parallel processing |
| `storage` | Yes | REDB file-based storage |
| `hnsw` | Yes | HNSW index support |
| `api-embeddings` | Yes | HTTP-based embedding providers |
| `memory-only` | No | Pure in-memory (WASM) |
| `real-embeddings` | No | Deprecated, use api-embeddings |

---

## Dependencies

### Core Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `hnsw_rs` | workspace | HNSW implementation |
| `simsimd` | workspace | SIMD distance functions |
| `rayon` | workspace | Parallel iteration |
| `redb` | workspace | Embedded database |
| `bincode` | workspace | Binary serialization |
| `dashmap` | workspace | Concurrent hash map |
| `parking_lot` | workspace | Optimized locks |

### Optional Dependencies

| Dependency | Feature | Purpose |
|------------|---------|---------|
| `reqwest` | api-embeddings | HTTP client for embedding APIs |
| `memmap2` | storage | Memory-mapped files |
| `crossbeam` | parallel | Lock-free data structures |

---

## API Examples

### Basic Vector Search

```rust
use ruvector_core::{VectorDB, DistanceMetric, HnswConfig};

// Create database
let config = HnswConfig {
    m: 32,
    ef_construction: 200,
    ef_search: 100,
    max_elements: 1_000_000,
};
let mut db = VectorDB::new(384, DistanceMetric::Cosine, config)?;

// Insert vectors
db.insert("doc_1".to_string(), vec![0.1; 384])?;
db.insert("doc_2".to_string(), vec![0.2; 384])?;

// Search
let query = vec![0.15; 384];
let results = db.search(&query, 10)?;
```

### Quantized Search

```rust
use ruvector_core::quantization::{ScalarQuantized, QuantizedVector};

// Quantize vectors for storage
let quantized = ScalarQuantized::quantize(&vector);

// Distance in quantized space
let distance = quantized.distance(&other_quantized);

// Reconstruct if needed
let reconstructed = quantized.reconstruct();
```

### Batch Operations

```rust
use ruvector_core::distance::batch_distances;

// Calculate distances to many vectors in parallel
let distances = batch_distances(
    &query,
    &corpus_vectors,
    DistanceMetric::Cosine,
)?;
```

---

## References

1. Malkov, Y., & Yashunin, D. (2018). "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs." arXiv:1603.09320.

2. Jegou, H., Douze, M., & Schmid, C. (2011). "Product quantization for nearest neighbor search." IEEE TPAMI.

3. RuVector Team. "ruvector-core Benchmarks." /crates/ruvector-core/benches/

4. SimSIMD Documentation. https://github.com/ashvardanian/SimSIMD

---

## Appendix A: SIMD Register Usage

### AVX2 (256-bit registers)

```
+-------+-------+-------+-------+-------+-------+-------+-------+
|  f32  |  f32  |  f32  |  f32  |  f32  |  f32  |  f32  |  f32  |
+-------+-------+-------+-------+-------+-------+-------+-------+
   [0]     [1]     [2]     [3]     [4]     [5]     [6]     [7]

Operations per cycle:
- _mm256_loadu_ps: Load 8 floats
- _mm256_sub_ps: 8 subtractions
- _mm256_mul_ps: 8 multiplications
- _mm256_add_ps: 8 additions
```

### NEON (128-bit registers)

```
+-------+-------+-------+-------+
|  f32  |  f32  |  f32  |  f32  |
+-------+-------+-------+-------+
   [0]     [1]     [2]     [3]

Operations per cycle:
- vld1q_f32: Load 4 floats
- vsubq_f32: 4 subtractions
- vfmaq_f32: 4 fused multiply-add
- vaddvq_f32: Horizontal sum
```

---

## Appendix B: Memory Layout

### VectorEntry

```
+------------------+------------------+------------------+
|     id: String   |  vector: Vec<f32>|  metadata: JSON  |
|     (optional)   |  (required)      |  (optional)      |
+------------------+------------------+------------------+
```

### HNSW Graph Structure

```
Level 3:  [v0] -------- [v5]
            \            /
Level 2:  [v0] -- [v3] -- [v5] -- [v9]
            \    /    \    /    \
Level 1:  [v0]-[v1]-[v3]-[v4]-[v5]-[v7]-[v9]
            |    |    |    |    |    |    |
Level 0:  [v0]-[v1]-[v2]-[v3]-[v4]-[v5]-[v6]-[v7]-[v8]-[v9]
```

---

## Appendix C: Benchmark Results

### Platform: Apple M2 (ARM64 NEON)

```
HNSW Search k=10 (10K vectors, 384-dim):
  p50: 61us
  p95: 89us
  p99: 112us
  Throughput: 16,400 QPS

HNSW Search k=100 (10K vectors, 384-dim):
  p50: 164us
  p95: 203us
  p99: 245us
  Throughput: 6,100 QPS

Distance Operations (1536-dim):
  Cosine: 143ns
  Euclidean: 156ns
  Dot Product: 33ns (384-dim)

Batch Distance (1000 vectors, 384-dim):
  Parallel (Rayon): 237us
  Sequential: 890us
```

### Platform: Intel i7 (AVX2)

```
HNSW Search k=10 (10K vectors, 384-dim):
  p50: 72us
  p95: 105us
  p99: 134us
  Throughput: 13,900 QPS

Distance Operations (1536-dim):
  Cosine: 128ns
  Euclidean: 141ns
  Dot Product: 29ns (384-dim)
```

---

## Related Decisions

- **ADR-002**: RuvLLM Integration with Ruvector
- **ADR-003**: SIMD Optimization Strategy
- **ADR-004**: KV Cache Management
- **ADR-005**: WASM Runtime Integration
- **ADR-006**: Memory Management
- **ADR-007**: Security Review & Technical Debt

---

## Implementation Status (v2.1)

| Component | Status | Notes |
|-----------|--------|-------|
| HNSW Index | ✅ Implemented | M=32, ef_construct=256, 16K QPS |
| SIMD Distance | ✅ Implemented | AVX2/NEON with fallback |
| Scalar Quantization | ✅ Implemented | 8-bit with min/max scaling |
| Batch Operations | ✅ Implemented | Rayon parallel distances |
| Graph Store | ✅ Implemented | Adjacency list with metadata |
| Persistence | ✅ Implemented | Binary format with versioning |

**Security Status:** Core components reviewed. No critical vulnerabilities in ruvector-core. See ADR-007 for full audit (RuvLLM-specific issues).

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-18 | Ruvector Architecture Team | Initial version |
| 1.1 | 2026-01-19 | Security Review Agent | Added implementation status, related decisions |
