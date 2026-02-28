# rUvector Performance Benchmarks

**Date:** November 25, 2025
**Test Environment:** Linux 4.4.0, Rust 1.91.1

---

## ⚠️ Important Disclaimer

**This document contains internal rUvector benchmark results only.**

The previous version of this document made unfounded performance claims comparing rUvector to other vector databases (e.g., "100-4,400x faster than Qdrant"). These claims were based on fabricated data and hardcoded multipliers in test code, not actual comparative benchmarks.

**We have removed all false comparison claims.** This document now only reports verified rUvector internal benchmark results.

---

## Verified rUvector Benchmark Results

### 1. Distance Metrics Performance (SimSIMD + AVX2)

rUvector uses SimSIMD with custom AVX2 intrinsics for SIMD-optimized distance calculations:

| Dimensions | Euclidean | Cosine | Dot Product |
|------------|-----------|--------|-------------|
| **128D** | 25 ns | 22 ns | 22 ns |
| **384D** | 47 ns | 42 ns | 42 ns |
| **768D** | 90 ns | 78 ns | 78 ns |
| **1536D** | 167 ns | 135 ns | 135 ns |

**Batch Processing (1000 vectors × 384D):** 278 µs total = **3.6M distance ops/sec**

### 2. HNSW Search Performance

Benchmarked with 1,000 vectors, 128 dimensions:

| k (neighbors) | Latency | QPS Equivalent |
|---------------|---------|----------------|
| **k=1** | 45 µs | 22,222 QPS |
| **k=10** | 61 µs | 16,393 QPS |
| **k=100** | 165 µs | 6,061 QPS |

### 3. rUvector Internal Scaling Tests

#### 10,000 Vectors, 384 Dimensions

| Configuration | Insert (ops/s) | Search QPS | p50 Latency |
|---------------|----------------|------------|-------------|
| **rUvector** | 34,435,442 | 623 | 1.57 ms |
| **rUvector (quantized)** | 29,673,943 | 742 | 1.34 ms |

#### 50,000 Vectors, 384 Dimensions

| Configuration | Insert (ops/s) | Search QPS | p50 Latency |
|---------------|----------------|------------|-------------|
| **rUvector** | 16,697,377 | 113 | 8.71 ms |
| **rUvector (quantized)** | 35,065,891 | 143 | 6.86 ms |

### 4. Quantization Performance

#### Scalar Quantization (4x compression)

| Operation | 384D | 768D | 1536D |
|-----------|------|------|-------|
| Encode | 605 ns | 1.27 µs | 2.11 µs |
| Decode | 493 ns | 971 ns | 1.89 µs |
| Distance | 64 ns | 127 ns | 256 ns |

#### Binary Quantization (32x compression)

| Operation | 384D | 768D | 1536D |
|-----------|------|------|-------|
| Encode | 625 ns | 1.27 µs | 2.5 µs |
| Decode | 485 ns | 970 ns | 1.9 µs |
| Hamming Distance | 33 ns | 65 ns | 128 ns |

**Compression Ratios:**
- Scalar (int8): **4x** memory reduction
- Product Quantization: **8-16x** memory reduction
- Binary: **32x** memory reduction (with ~10% recall loss)

---

## Architecture

### rUvector

| Component | Technology | Benefit |
|-----------|------------|---------|
| Core | Rust + NAPI-RS | Zero-overhead bindings |
| Distance | SimSIMD + AVX2/AVX-512 | 4-16x faster than scalar |
| Index | hnsw_rs | O(log n) search |
| Storage | redb (memory-mapped) | Zero-copy I/O |
| Concurrency | DashMap + RwLock | Lock-free reads |
| WASM | wasm-bindgen | Browser support |

---

## Features

| Feature | rUvector |
|---------|----------|
| **HNSW Index** | ✅ |
| **Cosine/Euclidean/DotProduct** | ✅ |
| **Scalar Quantization** | ✅ |
| **Product Quantization** | ✅ |
| **Binary Quantization** | ✅ |
| **Filtered Search** | ✅ |
| **Hybrid Search (BM25)** | ✅ |
| **MMR Diversity** | ✅ |
| **Hypergraph Support** | ✅ |
| **Neural Hashing** | ✅ |
| **Conformal Prediction** | ✅ |
| **AgenticDB API** | ✅ |
| **Browser/WASM** | ✅ |

---

## Use Cases

### rUvector is ideal for:
- **Embedded/Edge deployment** - Single binary, no external dependencies
- **Low-latency requirements** - Sub-millisecond search times
- **Browser/WASM** - Need vector search in frontend
- **AI Agent integration** - AgenticDB API, hypergraphs, causal memory
- **Research/experimental** - Neural hashing, TDA, learned indexes

---

## Reproducing Benchmarks

```bash
# rUvector Rust benchmarks
cargo bench -p ruvector-core --bench hnsw_search
cargo bench -p ruvector-core --bench distance_metrics
cargo bench -p ruvector-core --bench quantization_bench
```

## References

- [rUvector Repository](https://github.com/ruvnet/ruvector)
- [SimSIMD SIMD Library](https://github.com/ashvardanian/SimSIMD)
- [hnsw_rs Rust Implementation](https://github.com/jean-pierreBoth/hnswlib-rs)

---

## Note on Comparisons

**We do not currently have verified comparative benchmarks against other vector databases.**

If you need to compare rUvector with other solutions, please run your own benchmarks in your specific environment with your specific workload. Performance characteristics vary significantly based on:

- Vector dimensions and count
- Search parameters (k, ef_search)
- Hardware configuration
- Dataset distribution
- Query patterns

We welcome community contributions of fair, reproducible comparative benchmarks.
