# EXO-AI 2025: Rust Libraries & Crates Catalog

## SPARC Research Phase: Implementation Building Blocks

This document catalogs Rust crates and libraries applicable to the EXO-AI cognitive substrate architecture.

---

## 1. Tensor & Neural Network Frameworks

### Primary Frameworks

| Crate | Description | WASM | no_std | Use Case |
|-------|-------------|------|--------|----------|
| **[burn](https://lib.rs/crates/burn)** | Next-gen DL framework with backend flexibility | ✅ | ✅ | Core tensor operations, model training |
| **[candle](https://github.com/huggingface/candle)** | HuggingFace minimalist ML framework | ✅ | ❌ | Transformer inference, production models |
| **[ndarray](https://lib.rs/crates/ndarray)** | N-dimensional arrays | ❌ | ❌ | General numerical computing |
| **[burn-candle](https://crates.io/crates/burn-candle)** | Burn backend using Candle | ✅ | ❌ | Unified interface over Candle |
| **[burn-ndarray](https://crates.io/crates/burn-ndarray)** | Burn backend using ndarray | ❌ | ✅ | CPU-only, embedded targets |

### Key Characteristics

**Burn Framework**:
```rust
// Burn's backend flexibility enables future hardware abstraction
use burn::backend::Wgpu;  // GPU via WebGPU
use burn::backend::NdArray;  // CPU via ndarray
use burn::backend::Candle;  // HuggingFace models

// Example: Backend-agnostic tensor operation
fn matmul<B: Backend>(a: Tensor<B, 2>, b: Tensor<B, 2>) -> Tensor<B, 2> {
    a.matmul(b)
}
```

**Candle Strengths**:
- Transformer-specific optimizations
- ONNX model loading
- Quantization support (INT8, BF16)
- ~429KB WASM binary for BERT-style models

### Tensor Train Decomposition

| Crate/Paper | Description | Status |
|-------------|-------------|--------|
| [Functional TT Library (Springer 2024)](https://link.springer.com/chapter/10.1007/978-3-031-56208-2_22) | Function-Train decomposition in Rust | Research |

**Note**: This appears to be the only Rust-specific Tensor Train implementation, focused on PDEs rather than neural network compression. Opportunity exists for TT decomposition crate targeting learned manifold storage.

---

## 2. Graph & Hypergraph Libraries

### Core Graph Libraries

| Crate | Description | Features | Use Case |
|-------|-------------|----------|----------|
| **[petgraph](https://github.com/petgraph/petgraph)** | Primary Rust graph library | Graph/StableGraph/GraphMap, algorithms | Base graph operations |
| **[simplicial_topology](https://lib.rs/crates/simplicial_topology)** | Simplicial complexes | Random generation (Linial-Meshulam), upward/downward closure | TDA primitives |

### petgraph Capabilities
```rust
use petgraph::Graph;
use petgraph::algo::{toposort, kosaraju_scc, tarjan_scc};

// Topological sort for dependency ordering
let sorted = toposort(&graph, None)?;

// Strongly connected components for hyperedge detection
let sccs = kosaraju_scc(&graph);
```

### Simplicial Complex Operations
```toml
[dependencies]
simplicial_topology = { version = "0.1.1", features = ["sc_plot"] }
```

**Supported Models**:
- Linial-Meshulam (random hypergraphs)
- Lower/Upper closure
- Pure simplicial complexes

### Gap Analysis
No dedicated Rust hypergraph crate exists. Current approach:
1. Use petgraph for base graph operations
2. Extend with simplicial_topology for TDA
3. Implement hyperedge layer consuming ruvector-graph

---

## 3. Topological Data Analysis

### Persistent Homology

| Crate | Description | Features |
|-------|-------------|----------|
| **[tda](https://crates.io/crates/tda)** | TDA for neuroscience | Persistence diagrams, Mapper algorithm |
| **[teia](https://crates.io/crates/teia)** | Persistent homology library | Column reduction, persistence pairing |
| **[annembed](https://lib.rs/crates/annembed)** | UMAP-style dimension reduction | Links to Julia Ripserer.jl for TDA |

### tda Crate Structure
```rust
use tda::simplicial_complex::SimplicialComplex;
use tda::persistence::PersistenceDiagram;
use tda::mapper::Mapper;

// Compute persistent homology
let complex = SimplicialComplex::from_point_cloud(&points, epsilon);
let diagram = complex.persistence_diagram();
```

### teia CLI
```bash
# Compute homology generators
teia homology complex.json

# Compute persistent homology
teia persistence complex.json
```

**Planned Features** (teia):
- Persistent cohomology
- Lower-star complex
- Vietoris-Rips complex

---

## 4. WASM & NAPI-RS Integration

### WASM Ecosystem

| Crate | Description | Use Case |
|-------|-------------|----------|
| **[wasm-bindgen](https://crates.io/crates/wasm-bindgen)** | JS/Rust interop | Browser deployment |
| **[wasm-bindgen-futures](https://crates.io/crates/wasm-bindgen-futures)** | Async WASM | Async vector operations |
| **[web-sys](https://crates.io/crates/web-sys)** | Web APIs | Worker threads, WebGPU |
| **[js-sys](https://crates.io/crates/js-sys)** | JS types | ArrayBuffer interop |

### NAPI-RS for Node.js

| Crate | Description | Use Case |
|-------|-------------|----------|
| **[napi](https://crates.io/crates/napi)** | Node.js bindings | Server-side deployment |
| **[napi-derive](https://crates.io/crates/napi-derive)** | Macro support | Ergonomic API generation |

### Integration Pattern (ruvector style)
```rust
// NAPI-RS binding example
#[napi]
pub struct VectorIndex {
    inner: Arc<RwLock<HnswIndex>>,
}

#[napi]
impl VectorIndex {
    #[napi(constructor)]
    pub fn new(dimensions: u32) -> Result<Self> { ... }

    #[napi]
    pub async fn search(&self, query: Float32Array, k: u32) -> Result<SearchResults> { ... }
}
```

### WASM Neural Network Inference

| Tool | Description | Size |
|------|-------------|------|
| **WasmEdge WASI-NN** | TensorFlow/ONNX in WASM | Container: ~4MB |
| **Tract** | Native ONNX inference engine | Binary: ~500KB |
| **EdgeBERT** | Custom BERT inference | ~429KB WASM + 30MB model |

---

## 5. Post-Quantum Cryptography

### Primary Libraries

| Crate | Description | Algorithms |
|-------|-------------|------------|
| **[pqcrypto](https://github.com/rustpq/pqcrypto)** | Post-quantum crypto | Multiple NIST candidates |
| **[liboqs-rust](https://github.com/open-quantum-safe/liboqs-rust)** | OQS bindings | Full liboqs suite |
| **[kyberlib](https://kyberlib.com/)** | CRYSTALS-Kyber | ML-KEM (FIPS 203) |

### NIST Standardized Algorithms
```rust
// Kyber example (key encapsulation)
use kyberlib::{keypair, encapsulate, decapsulate};

let (public_key, secret_key) = keypair()?;
let (ciphertext, shared_secret_a) = encapsulate(&public_key)?;
let shared_secret_b = decapsulate(&ciphertext, &secret_key)?;
assert_eq!(shared_secret_a, shared_secret_b);
```

### Algorithm Support
- **ML-KEM** (Kyber): Key encapsulation
- **ML-DSA** (Dilithium): Digital signatures
- **FALCON**: Alternative signatures
- **SPHINCS+**: Hash-based signatures

---

## 6. Distributed Systems & Consensus

### Consensus Primitives

| Crate | Description | Use Case |
|-------|-------------|----------|
| **ruvector-raft** | Raft consensus | Leader election, log replication |
| **ruvector-cluster** | Cluster management | Node discovery, sharding |
| **ruvector-replication** | Data replication | Multi-region sync |

### CRDT Candidates

| Crate | Description | Status |
|-------|-------------|--------|
| **[crdts](https://crates.io/crates/crdts)** | CRDT implementations | Production-ready |
| **[automerge](https://crates.io/crates/automerge)** | JSON CRDT | Collaborative editing |

### ruvector Integration
```rust
// Existing ruvector-raft capabilities
use ruvector_raft::{RaftNode, RaftConfig};
use ruvector_cluster::{ClusterManager, NodeDiscovery};

let config = RaftConfig::default()
    .with_election_timeout(Duration::from_millis(150))
    .with_heartbeat_interval(Duration::from_millis(50));

let node = RaftNode::new(config, storage)?;
```

---

## 7. Performance & SIMD

### SIMD Libraries

| Crate | Description | Use Case |
|-------|-------------|----------|
| **[simsimd](https://crates.io/crates/simsimd)** | SIMD similarity functions | Distance metrics |
| **[packed_simd_2](https://crates.io/crates/packed_simd_2)** | Portable SIMD | General vectorization |
| **[wide](https://crates.io/crates/wide)** | Wide SIMD types | AVX-512 operations |

### ruvector Usage
```rust
// simsimd for distance calculations (already in ruvector-core)
use simsimd::{cosine, euclidean, dot};

let similarity = cosine(&vec_a, &vec_b);
let distance = euclidean(&vec_a, &vec_b);
```

### Parallelism

| Crate | Description | Use Case |
|-------|-------------|----------|
| **[rayon](https://crates.io/crates/rayon)** | Data parallelism | Parallel iterators |
| **[crossbeam](https://crates.io/crates/crossbeam)** | Concurrency primitives | Lock-free structures |
| **[tokio](https://crates.io/crates/tokio)** | Async runtime | Async I/O, networking |

---

## 8. Serialization & Storage

### Serialization

| Crate | Description | Speed | Size |
|-------|-------------|-------|------|
| **[rkyv](https://crates.io/crates/rkyv)** | Zero-copy deserialization | Fastest | Moderate |
| **[bincode](https://crates.io/crates/bincode)** | Binary serialization | Fast | Small |
| **[serde](https://crates.io/crates/serde)** | Serialization framework | Varies | Varies |

### Storage Backends

| Crate | Description | Use Case |
|-------|-------------|----------|
| **[redb](https://crates.io/crates/redb)** | Embedded ACID database | Persistent storage |
| **[memmap2](https://crates.io/crates/memmap2)** | Memory mapping | Large file access |
| **[hnsw_rs](https://crates.io/crates/hnsw_rs)** | HNSW index | Vector similarity |

---

## 9. Emerging Research Libraries

### Neuromorphic Simulation

| Status | Description | Gap |
|--------|-------------|-----|
| ⚠️ Limited | No mature Rust SNN library | Opportunity |

**Current Options**:
- Bind to C++ Brian2/NEST via FFI
- Port key algorithms from Python implementations
- Build minimal spike encoding layer

### Photonic Simulation

| Status | Description | Gap |
|--------|-------------|-----|
| ⚠️ None | No Rust photonic neural network library | Major gap |

**Approach**: Abstract optical matrix-multiply as backend trait

### Memristor Simulation

| Status | Description | Gap |
|--------|-------------|-----|
| ⚠️ None | No Rust memristor crossbar simulation | Research opportunity |

---

## 10. Recommended Stack for EXO-AI

### Core Foundation (ruvector SDK)
```toml
[dependencies]
ruvector-core = "0.1.16"
ruvector-graph = "0.1.16"
ruvector-gnn = "0.1.16"
ruvector-raft = "0.1.16"
ruvector-cluster = "0.1.16"
```

### ML/Tensor Operations
```toml
burn = { version = "0.14", features = ["wgpu", "ndarray"] }
candle-core = "0.6"
ndarray = { version = "0.16", features = ["serde"] }
```

### TDA/Topology
```toml
petgraph = "0.6"
simplicial_topology = "0.1"
teia = "0.1"
tda = "0.1"
```

### Post-Quantum Security
```toml
pqcrypto = "0.18"
kyberlib = "0.0.6"
```

### WASM/NAPI
```toml
wasm-bindgen = "0.2"
napi = { version = "2.16", features = ["napi9", "async", "tokio_rt"] }
napi-derive = "2.16"
```

### Distribution
```toml
tokio = { version = "1.41", features = ["full"] }
rayon = "1.10"
crossbeam = "0.8"
```

---

## Library Maturity Assessment

| Category | Maturity | Notes |
|----------|----------|-------|
| Tensors/ML | 🟢 High | Burn, Candle production-ready |
| Graphs | 🟢 High | petgraph is mature |
| Hypergraphs | 🟡 Medium | Need to build on simplicial_topology |
| TDA | 🟡 Medium | tda/teia usable, feature-incomplete |
| PQ Crypto | 🟢 High | Multiple options, NIST standardized |
| WASM | 🟢 High | wasm-bindgen ecosystem mature |
| NAPI-RS | 🟢 High | ruvector already uses successfully |
| Neuromorphic | 🔴 Low | Major gap, build or bind |
| Photonic | 🔴 Low | No existing libraries |
| Memristor | 🔴 Low | Research prototype needed |
