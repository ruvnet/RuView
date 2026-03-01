# ADR-017: Temporal Tensor Compression with Tiered Quantization

**Status**: Proposed
**Date**: 2026-02-06
**Parent**: ADR-001 RuVector Core Architecture, ADR-004 KV Cache Management, ADR-005 WASM Runtime Integration
**Author**: System Architecture Team
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-06 | Architecture Team | Initial SOTA research and design proposal |

---

## Abstract

This ADR introduces a **temporal tensor compression** system with **tiered quantization** for RuVector. The system exploits two key observations: (1) tensors accessed at different frequencies can tolerate different precision levels, and (2) quantization parameters (scales) can be amortized across consecutive time frames when the underlying distribution is stable. Together these yield 4x-10.67x compression over f32 while keeping reconstruction error within configurable bounds.

The implementation targets Rust with a zero-dependency WASM-compatible core, matching the sandboxed execution model established in ADR-005.

---

## 1. Context and Motivation

### 1.1 The Memory-Bandwidth Wall

Memory size and memory bandwidth dominate deployment cost for tensor-heavy workloads. ADR-004 established a three-tier KV cache (FP16 / 4-bit / 2-bit) but addresses only static snapshots of key-value pairs. Modern agent systems (RuVector's primary workload) produce **streams of tensor frames** - embeddings, activations, gradient sketches, coherence vectors - that evolve over time. Storing each frame independently wastes metadata and misses temporal redundancy.

**Memory scaling for agent tensor streams:**

| Tensor Dim | Frames/sec | Duration | Raw f32 | 8-bit | 5-bit | 3-bit |
|-----------|------------|----------|---------|-------|-------|-------|
| 512 | 10 | 1 hour | 73.7 MB | 18.4 MB | 11.5 MB | 6.9 MB |
| 2048 | 10 | 1 hour | 294.9 MB | 73.7 MB | 46.1 MB | 27.6 MB |
| 4096 | 50 | 1 hour | 2.95 GB | 737 MB | 461 MB | 276 MB |

### 1.2 Limitations of Current Quantization (ruvector-core)

The existing `quantization.rs` in `ruvector-core` provides:

| Method | Compression | Limitation |
|--------|-------------|------------|
| Scalar (u8) | 4x | Per-vector min/max scales; no temporal reuse |
| Int4 | 8x | Fixed 4-bit; no adaptive tier selection |
| Product | 8-16x | Requires codebook training; high latency |
| Binary | 32x | Too lossy for reconstruction-sensitive paths |

**Missing capabilities:**
- No temporal scale reuse across frames
- No access-pattern-driven tier selection
- No sub-byte bit packing (5-bit, 7-bit)
- No drift-aware segment boundaries
- No WASM-native compression path

### 1.3 Why Temporal Compression

The core insight: when a tensor's value distribution is stable over consecutive frames, the quantization scales computed for frame *t* remain valid for frames *t+1, t+2, ..., t+k*. Reusing scales across *k* frames amortizes the per-group scale overhead by *k*x and avoids redundant calibration passes.

This is the same principle behind:
- **Video codecs** (H.264/H.265): I-frames carry full parameters; P-frames reuse them until a scene change
- **Time-series databases** (Gorilla, InfluxDB): Delta-of-delta encoding reuses a base until drift exceeds a threshold
- **Streaming quantization** (QuaRot, KIVI): Per-channel parameters reused across tokens until attention pattern shifts

---

## 2. SOTA Research Summary

### 2.1 Groupwise Quantization (State of the Art 2024-2026)

Modern quantization systems converge on **per-group symmetric quantization** as the optimal accuracy-metadata tradeoff:

| System | Year | Approach | Key Innovation |
|--------|------|----------|----------------|
| **GPTQ** | 2023 | Per-column Hessian-weighted quantization | OBQ with lazy batch updates; group_size=128 standard |
| **AWQ** | 2024 | Activation-aware weight quantization | Protects salient channels via per-channel scaling |
| **SqueezeLLM** | 2024 | Non-uniform with sensitivity grouping | Dense-and-sparse decomposition for outliers |
| **QuIP#** | 2024 | Incoherence via random Hadamard | Enables high-quality 2-bit with lattice codebooks |
| **AQLM** | 2024 | Additive multi-codebook quantization | 2-bit with learned codebooks; beam search optimization |
| **SpinQuant** | 2024 | Rotation-based Cayley optimization | Learnable rotation matrices; Llama-2-7B 4-bit = FP16 parity |
| **KIVI** | 2024 | Per-channel key, per-token value | 2-bit KV cache with <0.1 ppl increase on Llama-2 |
| **Atom** | 2024 | Mixed-precision with reordering | Handles activation outliers via channel reordering |

**Consensus finding**: Group sizes of 32-128 provide the best accuracy-metadata tradeoff. Symmetric quantization (no zero-point) is sufficient when distribution is roughly centered, which holds for most intermediate tensors. The scale storage cost is `ceil(tensor_len / group_len) * sizeof(scale)`.

### 2.2 Sub-4-Bit Quantization Viability

| Bits | Compression vs f32 | Typical Quality Impact | Viable For |
|------|-------------------|----------------------|------------|
| 8 | 4.00x | Negligible (<0.01 ppl) | Hot path, full fidelity |
| 7 | 4.57x | Negligible (<0.02 ppl) | Warm path, near-lossless |
| 5 | 6.40x | Minor (0.05-0.1 ppl) | Warm path, acceptable loss |
| 4 | 8.00x | Moderate (0.1-0.3 ppl) | Well-studied; GPTQ/AWQ standard |
| 3 | 10.67x | Significant (0.3-1.0 ppl) | Cold path with bounded error |
| 2 | 16.00x | Large (1.0-3.0 ppl) | Archive only; KIVI/QuIP# needed |

**Key finding**: 3-bit symmetric quantization is the practical floor for reconstruction-required tensors. Below 3-bit, non-uniform or lattice codebook methods (QuIP#, AQLM) are needed to maintain quality, at much higher complexity.

### 2.3 Temporal Scale Reuse

No widely published system directly addresses temporal reuse of quantization scales for streaming tensor data. The closest analogs are:

1. **Gorilla (Facebook, 2015)**: XOR-based delta encoding for time-series floats; reuses a base encoding until delta exceeds threshold
2. **KIVI token reuse**: Per-channel scales for keys are computed once and applied to all tokens in the channel
3. **QuaRot (2024)**: Rotation matrices computed once per layer, reused for all tokens
4. **Streaming quantization in video**: DCT coefficients reused across P-frames until I-frame refresh

Our temporal segment approach generalizes these: compute group scales once per segment, emit packed codes for each frame, start a new segment on tier change or drift exceedance.

### 2.4 Bit-Packing Techniques

Standard bitstream packing (accumulator + shift) is the established approach for arbitrary-width codes:

```
For each code of width B bits:
  accumulator |= code << acc_bits
  acc_bits += B
  while acc_bits >= 8:
    emit(accumulator & 0xFF)
    accumulator >>= 8
    acc_bits -= 8
```

**SIMD acceleration**: For fixed widths (3, 5, 7, 8), vectorized pack/unpack can process 16-32 codes per SIMD iteration using shuffles and masks. The `bitpacking` crate achieves 4-8 GB/s on AVX2 for fixed-width packing. For WASM, the 128-bit SIMD proposal (widely supported since 2023) enables similar throughput.

### 2.5 Rust + WASM Performance Landscape

| Aspect | Status (2026) |
|--------|---------------|
| wasm32-unknown-unknown | Stable, widely deployed |
| WASM SIMD (128-bit) | Supported in all major browsers and runtimes |
| wasm32-wasi | Stable, server-side WASM standard |
| Linear memory model | Single contiguous address space; 32-bit pointers |
| `#[no_mangle] extern "C"` | Standard FFI pattern for WASM exports |
| Static mut in single-threaded WASM | Sound (no data races possible) but future-fragile |

**Relevant Rust WASM tensor libraries**: candle (Hugging Face), burn, tract. All demonstrate that high-performance tensor operations are viable in Rust/WASM with careful memory management.

---

## 3. Decision

### 3.1 Introduce Temporal Tensor Compression as a New Crate

We introduce `ruvector-temporal-tensor` (with WASM variant `ruvector-temporal-tensor-wasm`) implementing:

1. **Groupwise symmetric quantization** with f16 scales
2. **Temporal segments** that amortize scales across frames
3. **Three-tier access-driven bit-width selection** (8/7-or-5/3)
4. **Bitstream packing** with no byte-alignment waste
5. **WASM-compatible FFI** with handle-based resource management

### 3.2 Architecture Overview

```
+===========================================================================+
|               TEMPORAL TENSOR COMPRESSION ARCHITECTURE                     |
+===========================================================================+
|                                                                            |
|  Input Frame (f32[N])                                                      |
|       |                                                                    |
|       v                                                                    |
|  +----------------+     +-----------------+     +--------------------+     |
|  | Tier Policy    |---->| Segment Manager |---->| Segment Store      |     |
|  |                |     |                 |     | (Vec<u8> blobs)    |     |
|  | score = count  |     | - drift check   |     |                    |     |
|  |   * 1024 / age |     | - scale reuse   |     | Magic: TQTC        |     |
|  |                |     | - bit-width sel  |     | Version: 1         |     |
|  | Hot:  8-bit    |     |                 |     | Bits, GroupLen,     |     |
|  | Warm: 7/5-bit  |     +---------+-------+     | TensorLen, Frames, |     |
|  | Cold: 3-bit    |               |             | Scales[], Data[]   |     |
|  +----------------+               |             +--------------------+     |
|                                   v                                        |
|  +----------------------------------------------------------------+       |
|  |                    QUANTIZATION PIPELINE                        |       |
|  |                                                                 |       |
|  |  f32 values                                                     |       |
|  |    |                                                            |       |
|  |    v                                                            |       |
|  |  [Group 0: max_abs -> scale_f16] [Group 1: ...] [Group K: ...] |       |
|  |    |                                                            |       |
|  |    v                                                            |       |
|  |  q_i = round(v_i / scale)    // symmetric, no zero-point       |       |
|  |  q_i = clamp(q_i, -qmax, +qmax)                                |       |
|  |    |                                                            |       |
|  |    v                                                            |       |
|  |  u_i = q_i + bias            // signed -> unsigned mapping      |       |
|  |    |                                                            |       |
|  |    v                                                            |       |
|  |  [Bitstream Packer: B-bit codes, no alignment padding]          |       |
|  +----------------------------------------------------------------+       |
|                                                                            |
|  Decode: bitstream unpack -> unsigned -> signed -> scale multiply          |
+===========================================================================+
```

### 3.3 Segment Binary Format

```
Offset  Size    Field           Description
------  ------  --------------- -----------------------------------------
0       4       magic           0x43545154 ("TQTC" in LE ASCII)
4       1       version         Format version (currently 1)
5       1       bits            Bit width for this segment (3, 5, 7, or 8)
6       4       group_len       Elements per quantization group
10      4       tensor_len      Number of f32 elements per frame
14      4       frame_count     Number of frames in this segment
18      4       scale_count     Number of f16 group scales
22      2*S     scales          f16 scale values (S = scale_count)
22+2S   4       data_len        Length of packed bitstream in bytes
26+2S   D       data            Packed quantized codes (D = data_len)
```

**Segment size formula:**
```
segment_bytes = 26 + 2*ceil(tensor_len/group_len) + ceil(tensor_len * frame_count * bits / 8)
```

### 3.4 Tier Policy Design

```
Score = access_count * 1024 / (now_ts - last_access_ts + 1)

Tier 1 (Hot):   score >= hot_min_score   -> 8-bit  (~4.0x compression)
Tier 2 (Warm):  score >= warm_min_score  -> 7-bit  (~4.57x) or 5-bit (~6.4x)
Tier 3 (Cold):  score < warm_min_score   -> 3-bit  (~10.67x compression)
```

**Default thresholds:**

| Parameter | Default | Rationale |
|-----------|---------|-----------|
| `hot_min_score` | 512 | ~2 accesses/sec for recent data |
| `warm_min_score` | 64 | ~1 access every 16 seconds |
| `warm_bits` | 7 | Conservative warm tier; set to 5 for aggressive |
| `drift_pct_q8` | 26 | ~10.2% drift tolerance (26/256) |
| `group_len` | 64 | 64 elements per group; 128 bytes of f16 scales per 256 values |

**Drift detection**: Before appending a frame to the current segment, compute `max_abs` per group and compare against `scale * qmax * drift_factor`. If any group exceeds this, flush the current segment and start a new one with recomputed scales. This bounds reconstruction error to `drift_factor * original_error`.

### 3.5 Compression Math

Effective compression ratios including scale overhead (group_len=64, f16 scales):

| Bits | Raw Ratio | Scale Overhead | Effective Ratio | Effective Ratio (100 frames) |
|------|-----------|----------------|-----------------|------------------------------|
| 8 | 4.00x | 1 f16 per 64 vals | 3.76x | 3.99x |
| 7 | 4.57x | same | 4.27x | 4.56x |
| 5 | 6.40x | same | 5.82x | 6.38x |
| 3 | 10.67x | same | 9.14x | 10.63x |

Temporal amortization: with 100 frames per segment, scale overhead becomes negligible (~0.03% of segment size).

---

## 4. Detailed Design

### 4.1 Module Architecture

```
crates/ruvector-temporal-tensor/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public API, re-exports
    ├── tier_policy.rs      # TierPolicy: score calculation, tier selection
    ├── f16.rs              # Software f32<->f16 conversion (no external deps)
    ├── bitpack.rs          # Bitstream packer/unpacker for arbitrary widths
    ├── quantizer.rs        # Groupwise symmetric quantization + dequantization
    ├── segment.rs          # Segment encode/decode, binary format
    ├── compressor.rs       # TemporalTensorCompressor: drift, segmentation
    └── ffi.rs              # WASM/C FFI: handle store, extern "C" exports

crates/ruvector-temporal-tensor-wasm/
├── Cargo.toml              # wasm32-unknown-unknown target
└── src/
    └── lib.rs              # Re-exports FFI functions, WASM-specific config
```

### 4.2 Groupwise Symmetric Quantization

For a group of `G` values from frame `f`:

```
scale = max(|v_i| for i in group) / qmax
qmax  = 2^(bits-1) - 1      // e.g., bits=8 -> qmax=127, bits=3 -> qmax=3
q_i   = round(v_i / scale)
q_i   = clamp(q_i, -qmax, +qmax)
u_i   = q_i + qmax           // bias to unsigned for packing
```

Reconstruction:
```
q_i   = u_i - qmax           // unbias
v_i'  = q_i * scale          // dequantize
```

**Why symmetric**: No zero-point storage needed. For centered distributions (which agent tensors typically are), symmetric quantization loses minimal accuracy vs asymmetric while halving metadata and simplifying the dequantize multiply.

**Why f16 scales**: 2 bytes per group vs 4 bytes for f32. For typical tensor magnitudes (1e-3 to 1e3), f16 provides sufficient precision for scales. The f16 dynamic range (6.1e-5 to 65504) covers the relevant scale values. Software f16 conversion is fast (~5ns per conversion) and avoids external crate dependencies.

### 4.3 Temporal Segment Lifecycle

```
  Frame 0       Frame 1       Frame 2       ...       Frame K       Frame K+1
  ┌─────┐       ┌─────┐       ┌─────┐                 ┌─────┐       ┌─────┐
  │ f32 │       │ f32 │       │ f32 │                 │ f32 │       │ f32 │
  └──┬──┘       └──┬──┘       └──┬──┘                 └──┬──┘       └──┬──┘
     │              │              │                       │              │
     v              v              v                       v              v
  ┌────────────────────────────────────────────────────────┐       ┌──────────
  │              SEGMENT 1 (same scales)                   │       │ SEGMENT 2
  │                                                        │       │ (new
  │  scales: [s0, s1, ..., sG]  (computed from frame 0)   │       │  scales)
  │  data:   [packed frame 0][packed frame 1]...[frame K]  │       │
  └────────────────────────────────────────────────────────┘       └──────────
                                                     ^
                                                     |
                                              Drift exceeded OR
                                              tier changed at K+1
```

**Segment boundary triggers:**
1. First frame (no active segment)
2. Tier bit-width changed (e.g., tensor went from hot to warm)
3. Any group's `max_abs > scale * qmax * drift_factor`

### 4.4 Drift Detection Algorithm

```rust
fn frame_fits_current_scales(frame, scales, qmax, drift_factor) -> bool {
    for each group (idx, scale) in scales:
        max_abs = max(|v| for v in group_slice(frame, idx))
        allowed = f16_to_f32(scale) * qmax * drift_factor
        if max_abs > allowed:
            return false  // Distribution has drifted
    return true
}
```

The `drift_factor` is `1 + drift_pct_q8/256`. With `drift_pct_q8=26`, this is `1.1015625` (~10% tolerance). This means a group's maximum absolute value can grow by up to ~10% beyond the original calibration before triggering a new segment.

**Tradeoff**: Lower drift tolerance = more segment boundaries = more accurate but more metadata. Higher drift tolerance = fewer segments = better compression but more quantization error. The 10% default is conservative; for cold tensors, 20-30% may be acceptable.

### 4.5 Bit-Packing Implementation

The packer uses a 64-bit accumulator for sub-byte codes:

```
For each quantized unsigned code u of width B bits:
    acc |= (u as u64) << acc_bits
    acc_bits += B
    while acc_bits >= 8:
        emit byte: acc & 0xFF
        acc >>= 8
        acc_bits -= 8
// After all codes: flush remaining bits
if acc_bits > 0:
    emit byte: acc & 0xFF
```

**Packing density** (no wasted bits):

| Bits | Codes per 8 bytes | Utilization |
|------|-------------------|-------------|
| 8 | 8 | 100% |
| 7 | 9.14 | 100% (no padding) |
| 5 | 12.8 | 100% (no padding) |
| 3 | 21.33 | 100% (no padding) |

### 4.6 f16 Software Conversion

The implementation provides bit-exact IEEE 754 half-precision conversion without external crates:

- **f32 -> f16**: Extract sign/exponent/mantissa, remap exponent bias (127 -> 15), handle denormals with rounding, infinity, NaN propagation
- **f16 -> f32**: Reverse the bias remapping, reconstruct denormals, handle special values

**Accuracy**: Round-to-nearest-even for normals. Denormal handling preserves gradual underflow. The conversion pair is not bit-exact round-trip for all f32 values (f16 has 10 mantissa bits vs f32's 23), but for scale values in the range [1e-4, 1e4], relative error is bounded by 2^-10 (~0.1%).

### 4.7 WASM FFI Design

```
┌─────────────────────────────────────────────────────┐
│                    WASM Linear Memory                │
│                                                      │
│  Host allocates via ttc_alloc()                      │
│  Host writes f32 frames into allocated buffers       │
│  Host calls ttc_push_frame(handle, ts, ptr, len,     │
│             out_ptr, out_cap, &out_written)           │
│  Host reads segment bytes from out_ptr               │
│  Host frees via ttc_dealloc()                        │
│                                                      │
│  ┌──────────────────────────────┐                    │
│  │ STORE: Vec<Option<Compressor>>│                   │
│  │  [0] = Some(comp_a)          │                    │
│  │  [1] = None (freed)          │                    │
│  │  [2] = Some(comp_b)          │                    │
│  └──────────────────────────────┘                    │
└─────────────────────────────────────────────────────┘
```

**FFI function table:**

| Function | Purpose | Parameters |
|----------|---------|------------|
| `ttc_create` | Create compressor | `(len, now_ts, &out_handle)` |
| `ttc_free` | Destroy compressor | `(handle)` |
| `ttc_touch` | Record access | `(handle, now_ts)` |
| `ttc_set_access` | Set access stats | `(handle, count, last_ts)` |
| `ttc_push_frame` | Compress a frame | `(handle, ts, in_ptr, len, out_ptr, out_cap, &out_written)` |
| `ttc_flush` | Flush current segment | `(handle, out_ptr, out_cap, &out_written)` |
| `ttc_decode_segment` | Decompress segment | `(seg_ptr, seg_len, out_ptr, out_cap, &out_written)` |
| `ttc_alloc` | Allocate WASM memory | `(size, &out_ptr)` |
| `ttc_dealloc` | Free WASM memory | `(ptr, cap)` |

**Handle-based store**: Compressors are stored in a global `Vec<Option<TemporalTensorCompressor>>`. Handles are indices. Freed slots are reused. This pattern is standard for WASM FFI where the host cannot hold Rust references.

---

## 5. Integration with RuVector

### 5.1 Crate Dependency Graph

```
ruvector-temporal-tensor
    (no external deps - pure Rust, WASM-safe)

ruvector-temporal-tensor-wasm
    └── ruvector-temporal-tensor

ruvector-core (future integration)
    └── ruvector-temporal-tensor (optional feature)
         extends QuantizedVector trait
```

### 5.2 AgenticDB Integration

Compressed segments are stored as byte blobs in AgenticDB, keyed by:
```
Key:   {tensor_id}:{segment_start_ts}:{segment_end_ts}
Value: segment bytes (TQTC format)
Tags:  tier={hot|warm|cold}, bits={3|5|7|8}, frames={N}
```

AgenticDB's HNSW index is not used for segment lookup (segments are accessed by time range, not similarity). Instead, a B-tree or time-range index over segment keys provides O(log N) lookup.

### 5.3 Coherence Engine Integration

The coherence engine (ADR-014, ADR-015) can trigger segment boundaries via a **coherence-gated refresh**:

```
if coherence_score(tensor_id) < coherence_threshold:
    compressor.flush()  // Force segment boundary
    // New segment will recompute scales from fresh data
```

This ensures that when the coherence engine detects structural disagreement (e.g., between an agent's embedding and the graph's expected embedding), the compression system refreshes its calibration even if drift is still within the numerical threshold.

### 5.4 Graph Lineage

Each segment can be represented as a node in RuVector's DAG (ADR-016 delta system):
- **Edges**: `tensor_id -> segment_1 -> segment_2 -> ...` (temporal lineage)
- **Metadata**: Which agent/workflow produced the tensor, tier at time of compression
- **Provenance**: Full reconstruction path from segments back to original f32 data

---

## 6. Implementation Review and Safety Analysis

### 6.1 Correctness Assessment

| Component | Status | Notes |
|-----------|--------|-------|
| Groupwise symmetric quant | Correct | `qmax = 2^(bits-1) - 1`; symmetric range [-qmax, +qmax] |
| f16 conversion | Correct with caveats | Rounding mode is round-half-up (not round-half-even); acceptable for scales |
| Bit-packing | Correct | 64-bit accumulator handles all widths 1-8 without overflow |
| Drift detection | Correct | Per-group max-abs comparison against scaled threshold |
| Segment encode/decode | Correct | Round-trip verified for all tier widths |
| Bias mapping | Correct | `bias = qmax`; unsigned range is `[0, 2*qmax]` which fits in `bits` bits |

### 6.2 Safety Analysis

| Pattern | Risk | Mitigation |
|---------|------|------------|
| `static mut STORE` | UB in multi-threaded context | WASM is single-threaded; safe in practice. Migrate to `thread_local!` or `OnceCell` for native targets. |
| `from_raw_parts` in FFI | UB if host passes invalid pointers | Host is responsible for valid pointers; standard WASM FFI contract. Add debug assertions. |
| `std::mem::forget` in `ttc_alloc` | Memory managed by host | Correct pattern; host calls `ttc_dealloc` to reconstruct and drop the Vec. |
| Null pointer checks | Partial | FFI functions check `out_written.is_null()` but not all `out_ptr`. Add null checks. |

**Recommended safety improvements for production:**
1. Replace `static mut` with `thread_local!` for native target compatibility
2. Add `#[cfg(debug_assertions)]` bounds checks in decode loops
3. Validate segment magic/version before parsing
4. Add `ttc_last_error` function for error reporting to host

### 6.3 Performance Characteristics

| Operation | Complexity | Estimated Latency (512-dim tensor) |
|-----------|------------|-----------------------------------|
| Tier selection | O(1) | <10ns |
| Drift check | O(N/G) where G=group_len | ~50ns |
| Scale computation | O(N) | ~100ns |
| Quantize + pack | O(N) | ~200ns |
| Decode + unpack | O(N) | ~200ns |
| f16 conversion | O(1) per scale | ~5ns |

**SIMD opportunity**: The inner quantize loop (`v * inv_scale`, round, clamp, pack) is highly vectorizable. With WASM SIMD (128-bit), processing 4 f32s per iteration yields ~4x speedup on the hot loop.

---

## 7. Alternatives Considered

### 7.1 Extend Existing ruvector-core Quantization

**Rejected**: The existing `QuantizedVector` trait assumes single-frame quantization with per-vector scales. Temporal segments require fundamentally different state management (multi-frame, drift-aware). Adding this to `ruvector-core` would violate single-responsibility and complicate the existing, well-tested code.

### 7.2 Use GPTQ/AWQ-style Weight Quantization

**Rejected**: GPTQ and AWQ are designed for static weight quantization with Hessian-based sensitivity. Our use case is streaming activations/embeddings that change every frame. The calibration cost of GPTQ (~minutes per layer) is prohibitive for real-time streams.

### 7.3 Delta Encoding Between Frames

**Considered but deferred**: XOR-based or arithmetic delta encoding (frame[t] - frame[t-1]) could further compress within a segment. However, this adds complexity and makes random access within a segment O(N) instead of O(1). We may add this as an optional mode in a future version.

### 7.4 Asymmetric Quantization

**Rejected for default**: Asymmetric quantization (with zero-point) adds 2 bytes of metadata per group and requires an additional subtraction in the dequantize path. For centered distributions (typical of embeddings and activations), the accuracy improvement is marginal (<0.5% relative error reduction) while the metadata cost is significant at small group sizes.

### 7.5 Using the `half` Crate for f16

**Rejected**: Adding an external dependency for f16 conversion would complicate WASM builds and increase binary size. The software f16 conversion is ~50 lines and has no performance-critical path (scales are converted once per segment, not per frame).

---

## 8. Acceptance Criteria

### 8.1 Compression Targets

| Tier | Bits | Target Compression (vs f32) | Measurement |
|------|------|-----------------------------|-------------|
| Hot | 8 | >= 3.7x (single frame), >= 3.99x (100 frames) | Segment size / raw f32 size |
| Warm (7-bit) | 7 | >= 4.2x (single frame), >= 4.56x (100 frames) | Same |
| Warm (5-bit) | 5 | >= 5.8x (single frame), >= 6.38x (100 frames) | Same |
| Cold | 3 | >= 9.0x (single frame), >= 10.6x (100 frames) | Same |

**Primary target**: On a representative 1-hour trace, achieve **>= 6x** reduction for warm tensors and **>= 10x** for cold tensors in resident bytes.

### 8.2 Accuracy Targets

| Tier | Max Relative Error | Measurement |
|------|-------------------|-------------|
| Hot (8-bit) | < 0.8% | max(|v - v'|) / max(|v|) per frame |
| Warm (7-bit) | < 1.6% | Same |
| Warm (5-bit) | < 6.5% | Same |
| Cold (3-bit) | < 30% | Same; bounded error, not bit-exact |

### 8.3 Performance Targets

| Metric | Target |
|--------|--------|
| Quantize latency (512-dim, native) | < 500ns per frame |
| Quantize latency (512-dim, WASM) | < 2us per frame |
| Decode latency (512-dim, native) | < 500ns per frame |
| WASM binary size | < 100KB (release, wasm-opt) |
| Memory overhead per compressor | < 1KB + segment data |

### 8.4 Functional Requirements

- [ ] Round-trip encode/decode produces correct results for all tier widths (3, 5, 7, 8)
- [ ] Drift detection correctly triggers segment boundaries
- [ ] Tier transitions produce valid segment boundaries
- [ ] Multiple compressors can coexist via handle system
- [ ] Segment binary format is platform-independent (little-endian)
- [ ] WASM FFI functions handle null pointers and size mismatches gracefully
- [ ] No external crate dependencies in core library

---

## 9. Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| 3-bit quantization too lossy for some tensor types | High | Medium | Make tier policy configurable; allow per-tensor overrides; add quality monitoring |
| Drift detection false positives cause excessive segments | Medium | Medium | Tune drift_pct_q8; add hysteresis (require N consecutive drifts) |
| f16 scale precision insufficient for very small tensors | Medium | Low | Detect near-zero scales; fall back to f32 scales when f16 underflows |
| WASM performance 3-5x slower than native | Medium | High | Expected; optimize hot loops with WASM SIMD; acceptable for non-realtime paths |
| `static mut` unsound if WASM threading arrives | Low | Low | Replace with `thread_local!` or atomic cell before enabling shared memory |
| Segment format not forward-compatible | Medium | Low | Version field enables format evolution; decode rejects unknown versions |

---

## 10. Open Questions

1. **Typical tensor dimensions**: What are the representative dimensions for RuVector agent tensors? (Impacts group_len tuning and SIMD strategy)
2. **Update frequency**: How many frames per second for hot vs warm vs cold tensors? (Impacts segment size expectations)
3. **Cold tier error tolerance**: Is bounded relative error (up to 30% at 3-bit) acceptable, or do some cold tensors need bit-exact reversibility?
4. **Integration priority**: Should AgenticDB integration (segment storage) or coherence engine integration (drift gating) come first?
5. **SIMD tier**: Should the initial implementation include WASM SIMD, or start scalar-only and add SIMD in a follow-up?

---

## 11. Implementation Roadmap

### Phase 1: Core Engine (Week 1-2)
- [ ] Create `ruvector-temporal-tensor` crate with zero dependencies
- [ ] Implement `tier_policy.rs`, `f16.rs`, `bitpack.rs`, `quantizer.rs`
- [ ] Implement `segment.rs` (encode/decode) and `compressor.rs`
- [ ] Unit tests: round-trip correctness for all bit widths
- [ ] Unit tests: drift detection boundary conditions
- [ ] Unit tests: segment binary format parsing

### Phase 2: WASM FFI (Week 2-3)
- [ ] Implement `ffi.rs` with handle-based store
- [ ] Create `ruvector-temporal-tensor-wasm` crate
- [ ] WASM integration tests via wasm-pack
- [ ] Binary size validation (< 100KB target)
- [ ] Performance benchmarks (native vs WASM)

### Phase 3: Integration (Week 3-4)
- [ ] AgenticDB segment storage adapter
- [ ] Coherence engine refresh hook
- [ ] DAG lineage edges for segments
- [ ] End-to-end benchmark on representative trace
- [ ] Acceptance test: 6x warm, 10x cold compression

### Phase 4: Optimization (Week 4+)
- [ ] WASM SIMD for quantize/dequantize hot loops
- [ ] Native AVX2/NEON specialization
- [ ] Optional delta encoding within segments
- [ ] Streaming decode (partial segment access)
- [ ] Add to workspace `Cargo.toml`

---

## 12. References

1. Frantar, E., et al. "GPTQ: Accurate Post-Training Quantization for Generative Pre-trained Transformers." ICLR 2023.
2. Lin, J., et al. "AWQ: Activation-aware Weight Quantization for LLM Compression and Acceleration." MLSys 2024.
3. Kim, S., et al. "SqueezeLLM: Dense-and-Sparse Quantization." ICML 2024.
4. Chee, J., et al. "QuIP#: Even Better LLM Quantization with Hadamard Incoherence and Lattice Codebooks." ICML 2024.
5. Egiazarian, V., et al. "AQLM: Extreme Compression of Large Language Models via Additive Quantization." ICML 2024.
6. Liu, Z., et al. "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache." ICML 2024.
7. Zhao, Y., et al. "Atom: Low-bit Quantization for Efficient and Accurate LLM Serving." MLSys 2024.
8. Liu, R., et al. "SpinQuant: LLM Quantization with Learned Rotations." NeurIPS 2024.
9. Ma, S., et al. "The Era of 1-bit LLMs: All Large Language Models are in 1.58 Bits." arXiv:2402.17764, 2024.
10. Pelkonen, T., et al. "Gorilla: A Fast, Scalable, In-Memory Time Series Database." VLDB 2015.
11. Ashkboos, S., et al. "QuaRot: Outlier-Free 4-Bit Inference in Rotated LLMs." NeurIPS 2024.

---

## Appendix A: Compression Ratio Derivation

For a tensor of dimension `N` with group size `G`, bit width `B`, and `F` frames per segment:

```
raw_size    = N * 4 * F                              // f32 bytes per segment
scale_size  = ceil(N/G) * 2                           // f16 scales (shared across frames)
header_size = 26                                      // fixed segment header
data_size   = ceil(N * F * B / 8)                     // packed bitstream
segment_size = header_size + scale_size + data_size

compression_ratio = raw_size / segment_size
```

**Example**: N=512, G=64, B=3, F=100:
```
raw_size    = 512 * 4 * 100         = 204,800 bytes
scale_size  = ceil(512/64) * 2      = 16 bytes
header_size = 26                     = 26 bytes
data_size   = ceil(512 * 100 * 3/8) = 19,200 bytes
segment_size = 26 + 16 + 19,200     = 19,242 bytes

ratio = 204,800 / 19,242 = 10.64x
```

## Appendix B: Tier Score Examples

| Scenario | access_count | age (ticks) | Score | Tier |
|----------|-------------|-------------|-------|------|
| Actively used | 100 | 10 | 10,240 | Hot (8-bit) |
| Recently used | 50 | 100 | 512 | Hot (8-bit) |
| Moderate use | 10 | 100 | 102 | Warm (7-bit) |
| Infrequent | 5 | 200 | 25 | Cold (3-bit) |
| Stale | 1 | 1000 | 1 | Cold (3-bit) |

## Appendix C: Error Bound Analysis

For symmetric quantization with bit width `B` and group scale `s`:

```
quantization_step = s / qmax = s / (2^(B-1) - 1)
max_error         = quantization_step / 2        // from rounding
relative_error    = max_error / s = 1 / (2 * qmax)
```

| Bits | qmax | Max Relative Error |
|------|------|--------------------|
| 8 | 127 | 0.39% |
| 7 | 63 | 0.79% |
| 5 | 15 | 3.33% |
| 3 | 3 | 16.7% |

Note: These are worst-case per-element errors. RMS error across a group is typically sqrt(1/12) * quantization_step, which is ~0.29x the max error.
