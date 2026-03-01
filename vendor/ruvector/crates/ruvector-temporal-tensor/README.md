# ruvector-temporal-tensor

[![Crates.io](https://img.shields.io/crates/v/ruvector-temporal-tensor.svg)](https://crates.io/crates/ruvector-temporal-tensor)
[![Documentation](https://docs.rs/ruvector-temporal-tensor/badge.svg)](https://docs.rs/ruvector-temporal-tensor)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Shrink your vector data 4-10x without losing the signal.**

`ruvector-temporal-tensor` compresses streams of floating-point tensors by exploiting two properties that most vector workloads share:

1. **Values within a group are similar** — so a single scale factor per group captures the range, and a small integer code captures the value. This is *groupwise symmetric quantization*.
2. **Consecutive frames barely change** — so the same scale factors can be reused across many frames until the data drifts. This is *temporal segment reuse*.

The crate automatically picks the right bit-width based on how "hot" (frequently accessed) the tensor is, giving you aggressive compression on cold data while preserving accuracy on hot data.

Zero external dependencies. Compiles to WASM. Ships with a C FFI.

## How It Works

```
f32 frame ──► tier policy ──► quantizer ──► bitpack ──► segment blob
                  │
         "How hot is this tensor?"
          Hot  → 8-bit (lossless-ish)
          Warm → 7 or 5-bit
          Cold → 3-bit (10x smaller)
```

Each frame of `f32` values is divided into fixed-size groups (default 64). Per group, the compressor computes a single scale factor (`max_abs / qmax`) and maps every value to a signed integer code. Codes are packed into a tight bitstream with no byte-alignment waste.

When the next frame arrives, the compressor checks whether the existing scale factors still cover the new data (within a configurable drift tolerance). If they do, the frame is appended to the current **segment** — reusing the same scales. If they don't, the segment is finalized and a new one starts.

Segments are self-contained binary blobs with a 22-byte header, the f16-encoded scales, and the packed data. They can be decoded independently, or you can random-access a single frame by index.

## Compression Ratios

| Tier | Bits | Ratio vs f32 | Typical Error | When Used |
|------|------|-------------|---------------|-----------|
| Hot  | 8    | **~4x**     | < 0.5%        | Frequently accessed tensors |
| Warm | 7    | **~4.6x**   | < 1%          | Moderate access patterns |
| Warm | 5    | **~6.4x**   | < 3%          | Aggressively compressed warm data |
| Cold | 3    | **~10.7x**  | < 15%         | Rarely accessed / archival |

Ratios improve further with temporal reuse — the scale overhead is amortized across all frames in a segment.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ruvector-temporal-tensor = "2.0"
```

### Compress and decompress

```rust
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

// 1. Create a compressor for 128-element tensors
let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 128, 0);
comp.set_access(100, 0); // mark as hot → 8-bit quantization

let frame = vec![1.0f32; 128];
let mut segment = Vec::new();

// 2. Push frames — segment stays empty until a boundary is crossed
comp.push_frame(&frame, 1, &mut segment);

// 3. Force-emit the current segment
comp.flush(&mut segment);

// 4. Decode back to f32
let mut decoded = Vec::new();
ruvector_temporal_tensor::segment::decode(&segment, &mut decoded);
assert_eq!(decoded.len(), 128);
```

### Stream many frames

```rust
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 512, 0);
comp.set_access(100, 0);

let mut segments: Vec<Vec<u8>> = Vec::new();
let mut seg = Vec::new();

for t in 0..1000 {
    let frame: Vec<f32> = (0..512).map(|i| ((i + t) as f32 * 0.01).sin()).collect();
    comp.push_frame(&frame, t as u32, &mut seg);
    if !seg.is_empty() {
        segments.push(seg.clone());
    }
}
comp.flush(&mut seg);
if !seg.is_empty() {
    segments.push(seg);
}
```

### Random-access a single frame

```rust
use ruvector_temporal_tensor::segment;
# use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};
# let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 64, 0);
# let mut seg = Vec::new();
# comp.push_frame(&vec![1.0f32; 64], 0, &mut seg);
# comp.flush(&mut seg);

// Decode only frame 0 — skips all other frames in the segment
let values = segment::decode_single_frame(&seg, 0).unwrap();
assert_eq!(values.len(), 64);

// Check compression ratio
let ratio = segment::compression_ratio(&seg);
assert!(ratio > 1.0);
```

### Custom tier policy

```rust
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

let policy = TierPolicy {
    hot_min_score: 512,   // score threshold for 8-bit
    warm_min_score: 64,   // score threshold for warm tier
    warm_bits: 5,         // use 5-bit instead of default 7 for warm
    drift_pct_q8: 26,     // ~10% drift tolerance (Q8 fixed-point)
    group_len: 32,        // smaller groups = more scales, tighter fit
};

let mut comp = TemporalTensorCompressor::new(policy, 256, 0);
```

## Feature Flags

```toml
[dependencies]
ruvector-temporal-tensor = { version = "2.0", features = ["ffi"] }
```

| Feature | Default | Description |
|---------|---------|-------------|
| `ffi`   | off     | Enable `extern "C"` exports for WASM and C interop |
| `simd`  | off     | Reserved for future SIMD-accelerated quantization |

## API Reference

### Core Types

| Type | Description |
|------|-------------|
| `TemporalTensorCompressor` | Main entry point — push frames, get segments |
| `TierPolicy` | Controls bit-width selection and drift tolerance |

### Compressor Methods

| Method | Description |
|--------|-------------|
| `new(policy, len, now_ts)` | Create a compressor for tensors of `len` elements |
| `push_frame(frame, now_ts, out)` | Compress a frame; emits a segment on boundary crossings |
| `flush(out)` | Force-emit the current segment |
| `touch(now_ts)` | Record an access event (increments count + updates timestamp) |
| `set_access(count, ts)` | Set access stats directly (for restoring state) |
| `active_bits()` | Current quantization bit-width |
| `active_frame_count()` | Frames buffered in the current segment |
| `len()` / `is_empty()` | Tensor length |

### Segment Functions

| Function | Description |
|----------|-------------|
| `segment::decode(bytes, out)` | Decode all frames from a segment |
| `segment::decode_single_frame(bytes, idx)` | Decode one frame by index |
| `segment::parse_header(bytes)` | Read segment metadata without decoding |
| `segment::compression_ratio(bytes)` | Compute raw-to-compressed ratio |
| `segment::encode(...)` | Low-level segment encoder (used internally) |

### Low-Level Modules

| Module | Description |
|--------|-------------|
| `quantizer` | Groupwise symmetric quantization and dequantization |
| `bitpack` | Arbitrary-width bitstream packer and unpacker |
| `f16` | Software IEEE 754 half-precision conversion |
| `tier_policy` | Access-pattern scoring and bit-width selection |

## Segment Binary Format

Segments are self-contained, portable, and version-tagged:

```
Offset  Size  Field
──────  ────  ─────────────────
0       4     Magic: 0x43545154 ("TQTC")
4       1     Version (currently 1)
5       1     Bits per code (3, 5, 7, or 8)
6       4     Group length
10      4     Tensor length (elements per frame)
14      4     Frame count
18      4     Scale count (S)
22      2*S   Scales (f16, little-endian)
22+2S   4     Data length (D)
26+2S   D     Packed quantization codes
```

## FFI / WASM Usage

Enable the `ffi` feature and compile with `--target wasm32-unknown-unknown`:

```bash
cargo build --release --target wasm32-unknown-unknown --features ffi
```

Exported C functions:

| Function | Description |
|----------|-------------|
| `ttc_create(len, now_ts, out_handle)` | Create compressor, get handle |
| `ttc_create_with_policy(...)` | Create with custom tier policy |
| `ttc_free(handle)` | Free a compressor |
| `ttc_touch(handle, now_ts)` | Record access |
| `ttc_set_access(handle, count, ts)` | Set access stats |
| `ttc_push_frame(handle, ts, in, len, out, cap, written)` | Compress a frame |
| `ttc_flush(handle, out, cap, written)` | Flush current segment |
| `ttc_decode_segment(seg, len, out, cap, written)` | Decode a segment |
| `ttc_alloc(size, out_ptr)` | Allocate WASM linear memory |
| `ttc_dealloc(ptr, cap)` | Free allocated memory |

## Design Decisions

See **[ADR-017](../../docs/adr/ADR-017-temporal-tensor-compression.md)** for the full architecture decision record, including SOTA survey, compression math, safety analysis, and integration guidance.

Key decisions:

- **Groupwise symmetric** (no zero-point) — simpler, faster, well-suited for normally-distributed embeddings
- **f16 scales** — 2 bytes per group vs 4 for f32, with negligible accuracy loss
- **64-bit bitstream accumulator** — handles any sub-byte width without byte-alignment waste
- **Score-based tiering** — `access_count * 1024 / age` balances recency and frequency
- **~10% drift tolerance** — Q8 fixed-point configurable, default 26/256

## Building and Testing

```bash
# Build
cargo build -p ruvector-temporal-tensor --release

# Run all tests (41 unit + 3 doc-tests)
cargo test -p ruvector-temporal-tensor

# Clippy
cargo clippy -p ruvector-temporal-tensor -- -W clippy::all

# Build WASM target
cargo build -p ruvector-temporal-tensor --release --target wasm32-unknown-unknown --features ffi
```

## Related Crates

| Crate | Relationship |
|-------|-------------|
| [ruvector-core](../ruvector-core/) | Parent vector database engine; temporal tensors integrate as a storage backend |
| [ruvector-temporal-tensor-wasm](../ruvector-temporal-tensor-wasm/) | Thin WASM re-export wrapper |

## License

MIT License — see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector)**

</div>
