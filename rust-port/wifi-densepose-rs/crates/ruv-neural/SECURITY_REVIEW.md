# ruv-neural Crate System: Security and Performance Review

**Date**: 2026-03-09
**Version**: 0.1.0
**Scope**: All 12 workspace crates in the ruv-neural system
**Status**: Implementation checklist for v0.1 and v0.2 milestones

---

## Table of Contents

1. [Crate Inventory](#crate-inventory)
2. [Security Review](#security-review)
   - [Input Validation](#input-validation)
   - [Memory Safety](#memory-safety)
   - [Data Privacy](#data-privacy)
   - [Network Security (ESP32)](#network-security-esp32)
   - [Supply Chain](#supply-chain)
   - [Findings from Code Audit](#findings-from-code-audit)
3. [Performance Review](#performance-review)
   - [Computational Complexity](#computational-complexity)
   - [Memory Usage](#memory-usage)
   - [Optimization Opportunities](#optimization-opportunities)
   - [ESP32 Constraints](#esp32-constraints)
   - [Benchmarking Recommendations](#benchmarking-recommendations)
   - [Performance Findings from Code Audit](#performance-findings-from-code-audit)
4. [Action Items](#action-items)

---

## Crate Inventory

| Crate | Status | Lines (approx) | Role |
|-------|--------|-----------------|------|
| `ruv-neural-core` | Implemented | ~500 | Types, traits, error types, RVF format |
| `ruv-neural-sensor` | Implemented | ~170 | Sensor data acquisition, calibration, quality |
| `ruv-neural-signal` | Implemented | ~450 | Filtering, spectral analysis, Hilbert, connectivity |
| `ruv-neural-graph` | Stub | ~2 | Graph construction from signals |
| `ruv-neural-mincut` | Implemented | ~700 | Stoer-Wagner, spectral cut, Cheeger, dynamic tracking |
| `ruv-neural-embed` | Implemented | ~350 | Spectral, topology, node2vec embeddings |
| `ruv-neural-memory` | Implemented | ~425 | Embedding store, HNSW index |
| `ruv-neural-decoder` | Implemented (lib) | ~25 | KNN, threshold, transition decoders |
| `ruv-neural-esp32` | Implemented | ~265 | ADC interface, sensor readout |
| `ruv-neural-wasm` | Stub | ~2 | WebAssembly bindings |
| `ruv-neural-viz` | Implemented (lib) | ~20 | Visualization, ASCII rendering, export |
| `ruv-neural-cli` | Stub | ~2 | CLI binary |

---

## Security Review

### Input Validation

All public APIs must validate their inputs at system boundaries. This section catalogs each validation requirement and its current status.

#### Sensor Data Validation

| Check | Required In | Status | Notes |
|-------|------------|--------|-------|
| `sample_rate_hz > 0` | `MultiChannelTimeSeries::new` | **MISSING** | Constructor accepts `sample_rate_hz` without validating it is positive and finite. Division by zero in `duration_s()` if zero. |
| `num_channels > 0` | `MultiChannelTimeSeries::new` | PASS | Returns error if `data.len() == 0`. |
| Channel lengths equal | `MultiChannelTimeSeries::new` | PASS | Validates all channels have the same length. |
| Non-NaN/Inf values | All signal processing | **MISSING** | No validation that input signals contain only finite f64 values. NaN propagation through FFT, PLV, and connectivity metrics produces silent garbage. |
| `num_samples > 0` | `AdcReader::read_samples` | PASS | Returns error if `num_samples == 0`. |
| Channel count > 0 | `AdcReader::read_samples` | PASS | Returns error if no channels configured. |
| Channel index bounds | `AdcReader::load_buffer` | PASS | Returns `ChannelOutOfRange` error. |
| `sensitivity > 0` | `SensorChannel` | **MISSING** | `sensitivity_ft_sqrt_hz` is a public field with no validation on construction. |
| `sample_rate > 0` | `SensorChannel` | **MISSING** | `sample_rate_hz` is a public field with no validation. |

**Recommendation**: Add a `SensorChannel::new()` constructor that validates `sensitivity_ft_sqrt_hz > 0`, `sample_rate_hz > 0`, and that the orientation vector is a unit normal. Add `sample_rate_hz > 0` and `sample_rate_hz.is_finite()` checks to `MultiChannelTimeSeries::new`. Add a `validate_finite()` utility for signal data.

#### Graph Construction Validation

| Check | Required In | Status | Notes |
|-------|------------|--------|-------|
| Edge indices < `num_nodes` | `BrainGraph::adjacency_matrix` | PARTIAL | Silently skips out-of-bounds edges rather than reporting an error. This masks data corruption. |
| Edge weight is finite | `BrainGraph` | **MISSING** | `BrainEdge.weight` is not validated. NaN/Inf weights propagate silently through Stoer-Wagner and spectral analysis. |
| `num_nodes >= 2` | `stoer_wagner_mincut` | PASS | Returns proper error. |
| `num_nodes >= 2` | `fiedler_decomposition` | PASS | Returns proper error. |
| `num_nodes >= 2` | `SpectralEmbedder::embed` | PASS | Returns proper error. |
| `num_nodes >= 2` | `cheeger_constant` | PASS | Returns proper error. |
| Self-loops | `BrainGraph` | **MISSING** | No validation that `source != target` on edges. Self-loops could inflate degree calculations. |

**Recommendation**: Add a `BrainGraph::validate()` method that checks all edge indices are within bounds, weights are finite, and no self-loops exist. Call it from `stoer_wagner_mincut`, `spectral_bisection`, and `SpectralEmbedder::embed`. Consider making `adjacency_matrix()` return `Result` with an error for out-of-bounds edges instead of silently ignoring them.

#### RVF Format Validation

| Check | Required In | Status | Notes |
|-------|------------|--------|-------|
| Magic bytes | `RvfHeader::validate` | PASS | Validates against `RVF_MAGIC`. |
| Version | `RvfHeader::validate` | PASS | Rejects unknown versions. |
| Header length | `RvfHeader::from_bytes` | PASS | Checks `bytes.len() < 22`. |
| Data type tag | `RvfDataType::from_tag` | PASS | Returns error for unknown tags. |
| `metadata_json_len` overflow | `RvfFile::read_from` | **CONCERN** | `metadata_json_len` is cast from `u32` to `usize` and used to allocate a `Vec`. A malicious file with `metadata_json_len = u32::MAX` (~4 GB) would cause an OOM allocation. |
| Payload length | `RvfFile::read_from` | **CONCERN** | `read_to_end` reads unbounded data into memory. A malicious file could exhaust memory. |
| JSON validity | `RvfFile::read_from` | PASS | Uses `serde_json::from_slice` which returns an error on invalid JSON. |
| `num_entries` vs actual data | `RvfFile::read_from` | **MISSING** | The header declares `num_entries` and `embedding_dim`, but these are never cross-checked against the actual payload size. |

**Recommendation**: Add maximum size limits for `metadata_json_len` (e.g., 16 MB) and total payload size. Validate that `num_entries * entry_size_for_type <= data.len()` after reading. Use `Read::take()` to cap reads.

#### Embedding Validation

| Check | Required In | Status | Notes |
|-------|------------|--------|-------|
| Non-empty vector | `NeuralEmbedding::new` (core) | PASS | Returns error for empty vectors. |
| Non-empty vector | `NeuralEmbedding::new` (embed) | PASS | Returns error for empty vectors. |
| Dimension match | `cosine_similarity`, `euclidean_distance` | PASS | Returns `DimensionMismatch` error. |
| Zero-norm handling | `cosine_similarity` | PASS | Returns 0.0 for zero-norm vectors. |
| NaN/Inf in vector | `NeuralEmbedding::new` | **MISSING** | No check for non-finite values in the embedding vector. |

#### Memory Store Validation

| Check | Required In | Status | Notes |
|-------|------------|--------|-------|
| Capacity > 0 | `NeuralMemoryStore::new` | **MISSING** | Capacity 0 is accepted, producing a store that evicts on every insertion. |
| k > 0 | `query_nearest` | **MISSING** | k=0 produces an empty result silently (acceptable but undocumented). |
| Dimension consistency | `NeuralMemoryStore::store` | **MISSING** | No check that all stored embeddings have the same dimensionality. Mixed dimensions cause silent errors in `query_nearest`. |

#### JSON Parsing

| Check | Status | Notes |
|-------|--------|-------|
| Uses serde derive | PASS | All types use `#[derive(Serialize, Deserialize)]`. No manual parsing anywhere. |
| No `unsafe` JSON parsing | PASS | Standard `serde_json` throughout. |

---

### Memory Safety

| Check | Status | Notes |
|-------|--------|-------|
| No `unsafe` code | PASS | Zero `unsafe` blocks across all crates. |
| Vec instead of raw pointers | PASS | All data structures use `Vec`, `HashMap`, `BinaryHeap`. |
| ndarray for matrix ops | **NOT USED** | Despite being listed in `workspace.dependencies`, matrix operations use `Vec<Vec<f64>>` throughout. This is bounds-checked but less efficient. |
| No C FFI | PASS | No FFI calls. ESP32 code uses pure Rust types. |
| No `std::mem::transmute` | PASS | None found. |
| No `std::ptr` usage | PASS | None found. |
| Bounds checking on slices | PASS | Uses `.get()`, iterator methods, and Rust's built-in bounds checks. |
| Integer overflow | **CONCERN** | `max_raw_value()` in `adc.rs` casts `(1u32 << resolution_bits) - 1` to `i16`. If `resolution_bits > 15`, this overflows silently. Currently only 12 or 16 are intended, but 16 produces `i16::MAX` wrapping. |

**Recommendation**: Add a validation check on `resolution_bits` in `AdcConfig` (must be <= 15 for i16 representation, or switch to u16/i32). Consider migrating `Vec<Vec<f64>>` matrix representations to `ndarray::Array2<f64>` for better cache performance and built-in bounds checking.

---

### Data Privacy

Neural data is among the most sensitive personal data categories. This section covers data handling practices.

| Check | Status | Notes |
|-------|--------|-------|
| No PII in log messages | **NEEDS AUDIT** | The crate uses `tracing` in workspace dependencies but currently has no `tracing::info!` or `tracing::debug!` calls with data fields. As logging is added, ensure neural data values, subject IDs, and session IDs are never logged at INFO level or below. |
| No neural data in error messages | PASS | Error messages contain structural information (dimensions, indices, version numbers) but not raw signal values or embeddings. |
| `subject_id` handling | **CONCERN** | `EmbeddingMetadata.subject_id` is stored as plaintext `Option<String>`. This is PII that is included in serialized embeddings (serde), HNSW indices, and RVF files. |
| `session_id` handling | **CONCERN** | Same concern as `subject_id`. |
| Memory store encryption | **NOT IMPLEMENTED** | `NeuralMemoryStore` holds embeddings in plaintext `Vec<f64>`. No encryption-at-rest. |
| Memory zeroization on drop | **NOT IMPLEMENTED** | Embedding data is not zeroed when dropped. Sensitive neural data persists in deallocated memory. |
| WASM data boundary | STUB | WASM crate is not yet implemented. When implemented, must ensure no neural data is sent to external services without explicit user consent. |
| RVF file privacy | **CONCERN** | `RvfFile` serializes `metadata` as JSON, which may contain `subject_id`. No option to strip or anonymize metadata before export. |

**Recommendations**:
- Implement a `Redactable` trait for types that may contain PII, providing `redact()` and `anonymize()` methods.
- Use the `zeroize` crate to zero sensitive data on drop for `NeuralEmbedding`, `NeuralMemoryStore`, and `MultiChannelTimeSeries`.
- Add a `strip_pii()` method to `RvfFile` that removes or hashes identifiers before export.
- Document privacy responsibilities in each crate's module documentation.
- For v0.2: Add optional encryption-at-rest for `NeuralMemoryStore` using `ring` or `aes-gcm`.

---

### Network Security (ESP32)

| Check | Status | Notes |
|-------|--------|-------|
| Node ID authentication | **NOT IMPLEMENTED** | ESP32 crate (`ruv-neural-esp32`) is currently a local ADC reader with no network protocol. When TDM protocol is added, node IDs must be authenticated. |
| CRC32 integrity | **NOT IMPLEMENTED** | No data packet framing or integrity checks exist yet. |
| TLS encryption | **NOT IMPLEMENTED** | v0.1 has no network layer. Planned for v0.2. |
| Packet size limits | **NOT IMPLEMENTED** | No packet protocol exists yet. |
| Buffer overflow prevention | PARTIAL | `AdcReader` uses a fixed-size ring buffer (4096 samples), which prevents unbounded growth. However, `load_buffer` silently truncates data that exceeds buffer size rather than reporting it. |
| DMA configuration | N/A | `dma_enabled` is a configuration flag only; actual DMA is not implemented in std mode. |

**Recommendations for v0.2 TDM Protocol**:
- Authenticate node IDs using a pre-shared key or challenge-response.
- Add CRC32 or CRC32-C to every data packet.
- Set maximum packet size to 1460 bytes (single WiFi frame MTU).
- Use DTLS or TLS 1.3 for encryption when available.
- Rate-limit incoming packets per node to prevent flooding.
- Validate all fields in received packets before processing.

---

### Supply Chain

| Check | Status | Notes |
|-------|--------|-------|
| Minimal dependencies | PASS | Core dependencies: `thiserror`, `serde`, `serde_json`, `num-complex`, `rustfft`, `rand`. All are well-maintained, widely-used crates. |
| No proc macros except serde | PASS | Only `serde`'s derive macros and `thiserror`'s derive macro are used. `clap`'s derive is CLI-only. |
| All deps from crates.io | PASS | No git dependencies or path dependencies outside the workspace. |
| Workspace-managed versions | PASS | All dependency versions are declared in `[workspace.dependencies]`. |
| `petgraph` usage | **UNUSED** | Listed in workspace dependencies but not imported by any crate. Remove to reduce supply chain surface. |
| `tokio` usage | **UNUSED** | Listed in workspace dependencies but not imported by any crate. Remove unless async is planned. |
| `ruvector-*` crates | **UNUSED** | Five RuVector crates listed but not imported by any workspace member. Remove unused dependencies. |
| `Cargo.lock` | PRESENT | `Cargo.lock` is committed, ensuring reproducible builds. |

**Recommendation**: Run `cargo deny check` to audit for known vulnerabilities. Remove unused workspace dependencies (`petgraph`, `tokio`, `ruvector-*` crates) to minimize attack surface. Add `cargo audit` to CI.

---

### Findings from Code Audit

#### SEC-001: RVF Unbounded Allocation (Severity: Medium)

**Location**: `ruv-neural-core/src/rvf.rs`, line 193

```rust
let mut meta_bytes = vec![0u8; header.metadata_json_len as usize];
```

A crafted RVF file with `metadata_json_len = 0xFFFFFFFF` allocates 4 GB. Similarly, `read_to_end` on line 201 reads unbounded data.

**Fix**: Add maximum size constants and validate before allocating:
```rust
const MAX_METADATA_LEN: u32 = 16 * 1024 * 1024; // 16 MB
const MAX_PAYLOAD_LEN: usize = 256 * 1024 * 1024; // 256 MB

if header.metadata_json_len > MAX_METADATA_LEN {
    return Err(RuvNeuralError::Serialization(
        format!("metadata_json_len {} exceeds maximum {}", header.metadata_json_len, MAX_METADATA_LEN)
    ));
}
```

#### SEC-002: Missing Sample Rate Validation (Severity: Medium)

**Location**: `ruv-neural-core/src/signal.rs`, `MultiChannelTimeSeries::new`

The `sample_rate_hz` parameter is not validated. A value of 0.0 causes division by zero in `duration_s()`. A negative or NaN value causes incorrect spectral analysis throughout the pipeline.

**Fix**: Add validation in the constructor:
```rust
if sample_rate_hz <= 0.0 || !sample_rate_hz.is_finite() {
    return Err(RuvNeuralError::Signal(
        format!("sample_rate_hz must be positive and finite, got {}", sample_rate_hz)
    ));
}
```

#### SEC-003: NaN Propagation in Signal Processing (Severity: Low)

**Location**: `ruv-neural-signal/src/connectivity.rs`, all functions

If either input signal contains NaN, the Hilbert transform produces NaN outputs, which propagate silently through PLV, coherence, and all connectivity metrics. The result is a brain graph with NaN edge weights, which causes undefined behavior in Stoer-Wagner (infinite loops or wrong results).

**Fix**: Add a `validate_signal` helper and call it at entry points:
```rust
fn validate_signal(signal: &[f64]) -> Result<()> {
    if signal.iter().any(|x| !x.is_finite()) {
        return Err(RuvNeuralError::Signal("Signal contains NaN or Inf values".into()));
    }
    Ok(())
}
```

#### SEC-004: Integer Overflow in ADC (Severity: Low)

**Location**: `ruv-neural-esp32/src/adc.rs`, `AdcConfig::max_raw_value`

```rust
pub fn max_raw_value(&self) -> i16 {
    ((1u32 << self.resolution_bits) - 1) as i16
}
```

For `resolution_bits = 16`, this computes `65535 as i16 = -1`, which causes incorrect voltage conversion (division by -1 flips sign).

**Fix**: Change return type to `u16` or `i32`, or validate `resolution_bits <= 15`.

#### SEC-005: HNSW Visited Array Allocation (Severity: Low)

**Location**: `ruv-neural-memory/src/hnsw.rs`, `search_layer`, line 261

```rust
let mut visited = vec![false; self.embeddings.len()];
```

This allocates a visited array proportional to the total number of embeddings on every search call. For large indices (100K+ embeddings), this causes unnecessary allocation pressure. More critically, if `entry` is >= `self.embeddings.len()`, the indexing on line 262 panics.

**Fix**: Use a `HashSet<usize>` instead of a boolean array for sparse visitation. Add bounds check on `entry`.

---

## Performance Review

### Computational Complexity

| Operation | Complexity | Target Latency | Current Status |
|-----------|-----------|----------------|----------------|
| FFT (1024 points) | O(N log N) | <1 ms | Implemented via `rustfft` (SIMD-optimized). Meets target. |
| Hilbert transform | O(N log N) | <1 ms | Two FFTs (forward + inverse). Meets target for N <= 4096. |
| PLV (channel pair) | O(N) + 2x FFT | <0.5 ms | Calls `hilbert_transform` twice. Meets target for N <= 2048. |
| Coherence (channel pair) | O(N) + 2x FFT | <0.5 ms | Same as PLV. |
| Connectivity matrix (68 regions) | O(N^2 x M) | <10 ms | M = samples per channel, N = 68: 2,278 Hilbert pairs. May exceed target for long windows. |
| Stoer-Wagner mincut (68 nodes) | O(V^3) | <5 ms | 68^3 = ~314K operations. Meets target. |
| Spectral embedding (68 nodes) | O(V^2 x k x iterations) | <3 ms | With k=8, iterations=100: 68^2 x 8 x 100 = ~37M ops. May be tight. |
| Fiedler decomposition | O(V^2 x iterations) | <2 ms | 1000 iterations x 68^2 = ~4.6M ops. Meets target. |
| Cheeger constant (exact, n<=16) | O(2^n x n^2) | <5 ms | Exponential but capped at n=16: 65K x 256 = ~16M ops. Meets target. |
| HNSW insert | O(log N x ef x M) | <1 ms | ef=200, M=16: ~3200 distance computations per insert. Meets target. |
| HNSW search (10K embeddings) | O(log N x ef) | <1 ms | ef=50: ~50-200 distance computations. Meets target. |
| Brute-force NN (10K embeddings) | O(N x d) | <5 ms | d=256, N=10K: 2.56M f64 ops. Acceptable but HNSW preferred. |
| Full pipeline (68 regions) | - | <50 ms | Sum of above stages. Should meet target. |

### Memory Usage

| Component | Calculation | Size |
|-----------|------------|------|
| 64-channel x 1000 Hz x 8 bytes x 1s | 64 x 1000 x 8 | 512 KB per second |
| Brain graph adjacency (68 nodes) | 68^2 x 8 bytes | ~37 KB |
| Brain graph adjacency (400 nodes) | 400^2 x 8 bytes | ~1.25 MB |
| Single embedding (256-d) | 256 x 8 bytes | 2 KB |
| Memory store (10K embeddings, 256-d) | 10K x 2 KB | ~20 MB |
| HNSW index (10K, M=16, 256-d) | 10K x (2KB + 16 x 16 bytes) | ~22.5 MB |
| Stoer-Wagner working memory (68 nodes) | 2 x 68^2 x 8 + 68 x vec overhead | ~75 KB |
| Spectral embedder (68 nodes, k=8) | k x 68 x 8 + Laplacian 68^2 x 8 | ~41 KB |
| RVF file in memory | header + metadata + payload | Variable, unbounded (see SEC-001) |

### Optimization Opportunities

#### Immediate (v0.1)

1. **Eliminate redundant Hilbert transforms in connectivity matrix**
   - `compute_all_pairs` calls `hilbert_transform` twice per channel pair.
   - For 68 channels, this means 68 x 67 = 4,556 Hilbert transforms instead of 68.
   - **Fix**: Pre-compute analytic signals for all channels, then compute metrics pairwise.
   - **Expected speedup**: ~67x for connectivity matrix computation.

2. **Replace Vec<Vec<f64>> with flat Vec<f64> for adjacency matrices**
   - Current `Vec<Vec<f64>>` has poor cache locality due to heap-allocated inner Vecs.
   - **Fix**: Use `Vec<f64>` with manual row-major indexing, or migrate to `ndarray::Array2<f64>`.
   - **Expected speedup**: 2-4x for matrix-heavy operations (Stoer-Wagner, Laplacian).

3. **Avoid Vec::remove(0) in eviction**
   - `NeuralMemoryStore::evict_oldest` calls `self.embeddings.remove(0)`, which is O(n).
   - **Fix**: Use a `VecDeque` or circular buffer.
   - **Expected speedup**: O(1) eviction instead of O(n).

4. **Pre-allocate FFT planner**
   - `compute_psd`, `compute_stft`, and `hilbert_transform` each create a new `FftPlanner` per call.
   - **Fix**: Cache the planner or use a thread-local planner.
   - **Expected speedup**: Eliminates repeated plan computation.

#### Medium-term (v0.2)

5. **Rayon for parallel channel processing**
   - `compute_all_pairs` iterates channel pairs sequentially.
   - **Fix**: Use `rayon::par_iter` for the outer loop.
   - **Expected speedup**: Linear with core count for connectivity computation.

6. **SIMD for distance computations in HNSW**
   - Euclidean distance in `HnswIndex::distance` uses scalar iteration.
   - **Fix**: Use `packed_simd2` or auto-vectorization hints.
   - **Expected speedup**: 4-8x for 256-d vectors on AVX2.

7. **Sparse graph representation**
   - Dense adjacency matrix wastes memory for sparse brain graphs.
   - For Schaefer400, storing all 160K entries when only ~10K edges exist is wasteful.
   - **Fix**: Use compressed sparse row (CSR) format or `petgraph`'s sparse graph.

8. **Quantized embeddings for WASM**
   - f64 embeddings are unnecessarily precise for browser-based applications.
   - **Fix**: Support f32 embeddings in WASM builds, halving memory and transfer size.

#### Long-term (v0.3+)

9. **Streaming signal processing**
   - Current design loads entire time windows into memory.
   - **Fix**: Implement ring-buffer based streaming for real-time operation.

10. **GPU acceleration for large-scale spectral analysis**
    - For Schaefer400 atlas, eigendecomposition of 400x400 matrices benefits from GPU.
    - **Fix**: Optional `wgpu` or `vulkano` backend for matrix operations.

### ESP32 Constraints

| Resource | Limit | Current Usage | Status |
|----------|-------|---------------|--------|
| SRAM | 520 KB | Ring buffer: 4096 x channels x 2 bytes = 8 KB (1 channel) | OK |
| SRAM (multi-channel) | 520 KB | 4096 x 16 x 2 = 128 KB (16 channels) | **TIGHT** |
| CPU | 240 MHz dual-core | ADC sampling + data transmission | OK for 1 kHz |
| Flash | 4 MB | Binary size with release profile | Needs measurement |
| WiFi throughput | ~1 Mbps sustained | 64 ch x 1000 Hz x 2 bytes = 128 KB/s = 1 Mbps | **AT LIMIT** |

**Recommendations**:
- Use fixed-point arithmetic (i16 or Q15) instead of f64 on ESP32.
- Implement delta encoding or simple compression for data packets.
- Limit on-device processing to ADC readout and basic quality checks.
- Move all signal processing (FFT, connectivity, graph construction) to the host.
- Profile binary size with `cargo bloat` to ensure it fits in 4 MB flash.
- Consider reducing ring buffer size for multi-channel configurations.

### Benchmarking Recommendations

#### Per-Crate Microbenchmarks (criterion)

```toml
# Add to each crate's Cargo.toml
[[bench]]
name = "benchmarks"
harness = false

[dev-dependencies]
criterion = { workspace = true }
```

| Crate | Benchmark | Input Size | Metric |
|-------|-----------|------------|--------|
| `ruv-neural-signal` | `bench_hilbert_transform` | 256, 512, 1024, 2048, 4096 samples | ns/op |
| `ruv-neural-signal` | `bench_compute_psd` | 1024, 4096 samples | ns/op |
| `ruv-neural-signal` | `bench_plv_pair` | 1024 samples | ns/op |
| `ruv-neural-signal` | `bench_connectivity_matrix` | 16, 32, 68 channels x 1024 samples | ms/op |
| `ruv-neural-mincut` | `bench_stoer_wagner` | 10, 20, 50, 68, 100 nodes | us/op |
| `ruv-neural-mincut` | `bench_spectral_bisection` | 10, 20, 50, 68, 100 nodes | us/op |
| `ruv-neural-mincut` | `bench_cheeger_constant` | 8, 12, 16 nodes (exact), 32, 68 (approx) | us/op |
| `ruv-neural-embed` | `bench_spectral_embed` | 20, 50, 68, 100 nodes | us/op |
| `ruv-neural-memory` | `bench_brute_force_nn` | 100, 1K, 10K embeddings x 256-d | us/op |
| `ruv-neural-memory` | `bench_hnsw_insert` | 1K, 10K embeddings x 256-d | us/op |
| `ruv-neural-memory` | `bench_hnsw_search` | 1K, 10K embeddings, k=10, ef=50 | us/op |
| `ruv-neural-esp32` | `bench_adc_read` | 100, 1000 samples x 1-16 channels | us/op |

#### Full Pipeline Profiling

```bash
# Generate a flamegraph of the full pipeline
cargo flamegraph --bench full_pipeline -- --bench

# Memory profiling with DHAT
cargo test --features dhat-heap -- --test full_pipeline
```

#### WASM Performance

```javascript
// When ruv-neural-wasm is implemented, measure with:
performance.mark('embed-start');
const embedding = ruv_neural.embed(graphData);
performance.mark('embed-end');
performance.measure('embed', 'embed-start', 'embed-end');
```

#### ESP32 Hardware Timing

```rust
// Use esp-idf-hal's timer for hardware-level benchmarks
let start = esp_idf_hal::timer::now();
let samples = reader.read_samples(1000)?;
let elapsed_us = esp_idf_hal::timer::now() - start;
```

### Performance Findings from Code Audit

#### PERF-001: Redundant Hilbert Transforms (Severity: High)

**Location**: `ruv-neural-signal/src/connectivity.rs`, `compute_all_pairs`

Each call to `phase_locking_value`, `coherence`, `imaginary_coherence`, or `amplitude_envelope_correlation` independently calls `hilbert_transform` on both input signals. In `compute_all_pairs` with 68 channels, each channel's analytic signal is computed 67 times.

**Impact**: For 68 channels x 1024 samples, this means 4,556 FFTs instead of 68. Estimated waste: ~98.5% of FFT compute in the connectivity matrix.

**Fix**: Pre-compute all analytic signals, then pass slices to pairwise metrics:
```rust
pub fn compute_all_pairs_optimized(channels: &[Vec<f64>], metric: &ConnectivityMetric) -> Vec<Vec<f64>> {
    let analytics: Vec<Vec<Complex<f64>>> = channels.iter()
        .map(|ch| hilbert_transform(ch))
        .collect();
    // ... use pre-computed analytics for all pair computations
}
```

#### PERF-002: O(n) Eviction in Memory Store (Severity: Medium)

**Location**: `ruv-neural-memory/src/store.rs`, `evict_oldest`

```rust
fn evict_oldest(&mut self) {
    self.embeddings.remove(0);  // O(n) shift
    self.rebuild_index();       // O(n) rebuild
}
```

For a store with 10K embeddings, every insertion at capacity triggers an O(n) shift and full index rebuild.

**Fix**: Use `VecDeque<NeuralEmbedding>` and maintain the index incrementally.

#### PERF-003: FFT Planner Re-creation (Severity: Medium)

**Location**: `ruv-neural-signal/src/spectral.rs` (lines 12-13), `hilbert.rs` (lines 25-27)

A new `FftPlanner` is created on every function call. `rustfft` caches FFT plans internally in the planner, but creating a new planner discards the cache.

**Fix**: Use a thread-local or static planner:
```rust
thread_local! {
    static FFT_PLANNER: RefCell<FftPlanner<f64>> = RefCell::new(FftPlanner::new());
}
```

#### PERF-004: Dense Adjacency for Sparse Graphs (Severity: Low)

**Location**: `ruv-neural-core/src/graph.rs`, `adjacency_matrix`

Always allocates an N x N matrix even when the graph has far fewer edges. For Schaefer400 with ~5K edges, this allocates 1.25 MB for a matrix that is ~97% zeros.

**Fix**: Return a sparse representation for large graphs, or provide both `adjacency_matrix()` and `sparse_adjacency()`.

#### PERF-005: Power Iteration Convergence Not Checked (Severity: Low)

**Location**: `ruv-neural-mincut/src/spectral_cut.rs`, `largest_eigenvalue`

Runs a fixed 200 iterations regardless of convergence. Many graphs converge in 20-50 iterations.

**Fix**: Add early termination when eigenvalue change < epsilon:
```rust
if (eigenvalue - prev_eigenvalue).abs() < 1e-12 {
    break;
}
```

Note: `fiedler_decomposition` already has this check, but `largest_eigenvalue` does not.

---

## Action Items

### Critical (Must fix before v0.1 release)

- [ ] **SEC-001**: Add maximum size limits to RVF deserialization
- [ ] **SEC-002**: Validate `sample_rate_hz > 0` and `is_finite()` in `MultiChannelTimeSeries::new`
- [ ] **SEC-004**: Fix integer overflow in `AdcConfig::max_raw_value`
- [ ] **PERF-001**: Pre-compute Hilbert transforms in `compute_all_pairs`

### Important (Should fix before v0.1 release)

- [ ] **SEC-003**: Add NaN/Inf validation for signal data at pipeline entry points
- [ ] **SEC-005**: Add bounds check on HNSW entry point index
- [ ] **PERF-002**: Replace `Vec::remove(0)` with `VecDeque` in memory store
- [ ] **PERF-003**: Cache FFT planner across calls
- [ ] Add `BrainGraph::validate()` for edge index bounds and weight finiteness
- [ ] Add dimension consistency check to `NeuralMemoryStore::store`
- [ ] Remove unused workspace dependencies (`petgraph`, `tokio`, `ruvector-*`)

### Recommended (Fix in v0.2)

- [ ] Implement `zeroize`-on-drop for `NeuralEmbedding` and `NeuralMemoryStore`
- [ ] Add `strip_pii()` to `RvfFile`
- [ ] Migrate `Vec<Vec<f64>>` matrices to `ndarray::Array2<f64>`
- [ ] Add Rayon parallelism for connectivity matrix computation
- [ ] Add criterion benchmarks for all crates
- [ ] Implement TDM protocol with CRC32 and node authentication
- [ ] Add `cargo deny` and `cargo audit` to CI
- [ ] Profile and optimize binary size for ESP32

### Future (v0.3+)

- [ ] Encryption-at-rest for `NeuralMemoryStore`
- [ ] DTLS/TLS for ESP32 network protocol
- [ ] Sparse graph representation for large atlases
- [ ] f32 quantized embeddings for WASM
- [ ] Streaming signal processing pipeline
- [ ] GPU backend for large-scale spectral analysis

---

*This document should be reviewed and updated after each milestone. All security findings should be verified as resolved before the corresponding release.*
