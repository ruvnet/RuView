# ADR-019: Tiered Quantization Formats for Temporal Tensor Store

**Status**: Proposed
**Date**: 2026-02-08
**Parent**: ADR-017 Temporal Tensor Compression, ADR-018 Block-Based Storage Engine
**Author**: System Architecture Team

**Note**: Tiered quantization formats are now implemented in the rvf-quant crate as part of ADR-029 (RVF). See the RVF temperature-tiering specification (docs/research/rvf/spec/03-temperature-tiering.md).

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-08 | Architecture Team | Initial proposal |

---

## Abstract

This ADR defines the concrete quantization formats, bit-packing layouts, and codec
interfaces for the five tiers of tensor storage established in ADR-017. Where ADR-017
introduced the concept of access-frequency-driven quantization and temporal scale
reuse, this document specifies the exact byte-level formats for 8-bit (Tier 1 / Hot),
7-bit and 5-bit (Tier 2 / Warm), 3-bit (Tier 3 / Cold), and Compression-to-Zero
(Tier 0 / Absent). It also resolves two open design questions from ADR-017: whether
5-bit quantization is permitted within the warm tier, and how Tier 0 reads behave
when no reconstruction policy exists.

The `codec_bits` module provides a single allocation-free bit packer/unpacker that
all sub-byte formats share. The `quant` module provides per-format quantize and
dequantize functions, with SIMD-accelerated `max_abs` on native targets and a
portable fallback for WASM. Rust trait interfaces are defined so that new bit widths
can be added without modifying the core codec.

---

## 1. Context and Motivation

### 1.1 Gap in ADR-017

ADR-017 established the tiered compression architecture and segment binary format
but left the per-tier quantization details at the algorithmic level. Implementers
need exact byte layouts to write interoperable encoders and decoders, particularly
for the sub-byte formats (7-bit, 5-bit, 3-bit) where values do not align on byte
boundaries.

### 1.2 Sub-Byte Packing Complexity

Standard 8-bit quantization maps trivially to `[u8]` storage. Sub-byte formats
require a bit-packing codec that can write and read arbitrary-width codes into a
byte stream without wasting bits. The codec must:

- Handle bit widths 3, 5, and 7 (with 8 as a degenerate identity case).
- Operate without heap allocations (caller provides output slice).
- Be deterministic and platform-independent (little-endian byte order).
- Support WASM targets where SIMD is optional.

### 1.3 Outlier Handling in 3-Bit

At 3 bits per value, the quantization range is `[-3, +3]` (qmax = 3). Large
outliers in the tensor distribution can cause severe clamping. ADR-017 noted this
risk but did not specify a mitigation. This ADR introduces a two-level scale
option for Tier 3 that uses a 1-bit flag per value to select between a primary
scale (covering the majority of values) and a secondary scale (covering outliers),
while keeping the packed format compact.

### 1.4 Tier 0 Semantics

ADR-017 listed Compression-to-Zero as a future possibility. This ADR formalizes
it: Tier 0 stores no quantized data at all. Only metadata and an optional
`reconstruct_policy` survive. This enables aggressive memory reclamation for
tensors that are no longer accessed but may be reconstructable from other sources
(deltas, factors, or recomputation).

### 1.5 Design Questions Resolved

| Question | Resolution |
|----------|------------|
| Allow 5-bit within warm tier? | Yes. Dynamic downgrade from 7-bit to 5-bit when warm set exceeds a configurable byte cap (`warm_byte_cap`). |
| Tier 0 read semantics? | Return zeros by default. If a `reconstruct_policy` (Delta or Factor) exists, reconstruct from stored representation. |

---

## 2. Decision

We adopt the following five-tier quantization format hierarchy, each with a
well-defined byte layout, packing strategy, and error budget:

| Tier | Name | Bits | Compression vs f32 | Use Case |
|------|------|------|-------------------|----------|
| 1 | Hot | 8 | 4.00x | Active tensors, full fidelity |
| 2a | Warm | 7 | 4.57x | Default warm, near-lossless |
| 2b | Warm-aggressive | 5 | 6.40x | Warm set exceeds `warm_byte_cap` |
| 3 | Cold | 3 | 10.67x | Archived tensors, bounded error |
| 0 | Absent | 0 | Infinite | No data stored; metadata only |

All sub-byte formats share the `codec_bits` packer. All quantization formats use
symmetric per-block quantization with `scale = max_abs / qmax` stored as f32 per
block. The choice of f32 (rather than f16 as in ADR-017 segment headers) is
deliberate at this layer: the segment encoder may convert to f16 for storage, but
the quantizer operates in f32 for precision during the quantize/dequantize path.

---

## 3. Detailed Design

### 3.1 Tier 1: 8-Bit Quantization (Hot)

**Algorithm**: Symmetric per-block quantization.

```
Given: block of N f32 values, block_size typically 64 or 128
  scale    = max_abs(values) / 127
  q[i]     = round(values[i] / scale)
  q[i]     = clamp(q[i], -127, +127)      // i8 range
  store:     q as [i8; N] + scale as f32
```

**Storage layout** (one block, block_size = 8 for illustration):

```
Byte offset:  0    1    2    3    4    5    6    7    8    9   10   11
             [  scale (f32, LE)  ] [q0] [q1] [q2] [q3] [q4] [q5] [q6] [q7]
              ~~~~~~~~~~~~~~~~~~~  ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
              4 bytes              8 bytes (1 byte per i8 value)

Total per block: 4 + block_size bytes
```

**Effective compression** (block_size = 64):

```
raw     = 64 * 4           = 256 bytes
quant   = 4 + 64 * 1       = 68 bytes
ratio   = 256 / 68         = 3.76x (single block)
```

With temporal amortization (100 frames sharing scales): `256*100 / (4 + 64*100)` = 4.00x.

**Dequantize**:

```
values[i] = q[i] as f32 * scale
```

**Error bound**: `max_error = scale / (2 * 127)`. See Section 3.7 for full analysis.

### 3.2 Tier 2a: 7-Bit Quantization (Warm)

**Algorithm**: Symmetric per-block, 7-bit codes packed into a bitstream.

```
Given: block of N f32 values
  scale    = max_abs(values) / 63          // qmax = 2^(7-1) - 1 = 63
  q[i]     = round(values[i] / scale)
  q[i]     = clamp(q[i], -63, +63)
  u[i]     = q[i] + 63                    // bias to unsigned [0, 126], fits 7 bits
  pack u[i] values using codec_bits at width=7
```

**Bit-packing layout** (8 values packed into 7 bytes):

```
Values:     u0       u1       u2       u3       u4       u5       u6       u7
Bits:    [6..0]   [6..0]   [6..0]   [6..0]   [6..0]   [6..0]   [6..0]   [6..0]
         7 bits   7 bits   7 bits   7 bits   7 bits   7 bits   7 bits   7 bits

Packed into 7 bytes (56 bits = 8 * 7 bits):

Byte 0:  [u0[6:0]              | u1[0]  ]   = u0(7) + u1(1) = 8 bits
         |<--- 7 bits --->|<1>|

Byte 1:  [u1[6:1]                | u2[1:0]]  = u1(6) + u2(2) = 8 bits
         |<--- 6 bits --->|<-2->|

Byte 2:  [u2[6:2]              | u3[2:0] ]   = u2(5) + u3(3) = 8 bits
         |<-- 5 bits -->|<--3-->|

Byte 3:  [u3[6:3]            | u4[3:0]   ]   = u3(4) + u4(4) = 8 bits
         |<- 4 bits ->|<--4--->|

Byte 4:  [u4[6:4]          | u5[4:0]     ]   = u4(3) + u5(5) = 8 bits
         |<-3->|<---- 5 bits ---->|

Byte 5:  [u5[6:5]        | u6[5:0]       ]   = u5(2) + u6(6) = 8 bits
         |<2>|<----- 6 bits ------>|

Byte 6:  [u6[6]        | u7[6:0]         ]   = u6(1) + u7(7) = 8 bits
         |1|<------- 7 bits ------->|

Total:   7 bytes for 8 values = 0.875 bytes/value
```

**Storage per block** (block_size = 64):

```
scale:   4 bytes (f32)
data:    ceil(64 * 7 / 8) = 56 bytes
total:   60 bytes
ratio:   256 / 60 = 4.27x
```

### 3.3 Tier 2b: 5-Bit Quantization (Warm Aggressive)

**Algorithm**: Symmetric per-block, 5-bit codes.

```
Given: block of N f32 values
  scale    = max_abs(values) / 15          // qmax = 2^(5-1) - 1 = 15
  q[i]     = round(values[i] / scale)
  q[i]     = clamp(q[i], -15, +15)
  u[i]     = q[i] + 15                    // bias to unsigned [0, 30], fits 5 bits
  pack u[i] values using codec_bits at width=5
```

**Activation policy**: 5-bit is used instead of 7-bit when the total warm set
size exceeds `warm_byte_cap` (default: 64 MiB). The tier policy monitors
aggregate warm storage and downgrades from 7-bit to 5-bit for the least recently
accessed warm tensors until the cap is satisfied.

**Bit-packing layout** (8 values packed into 5 bytes):

```
Values:     u0       u1       u2       u3       u4       u5       u6       u7
Bits:    [4..0]   [4..0]   [4..0]   [4..0]   [4..0]   [4..0]   [4..0]   [4..0]
         5 bits   5 bits   5 bits   5 bits   5 bits   5 bits   5 bits   5 bits

Packed into 5 bytes (40 bits = 8 * 5 bits):

Byte 0:  [u0[4:0]        | u1[2:0]   ]   = u0(5) + u1(3) = 8 bits
         |<-- 5 bits -->|<--3-->|

Byte 1:  [u1[4:3]    | u2[4:0]   | u3[0]]  = u1(2) + u2(5) + u3(1) = 8 bits
         |<2>|<-- 5 bits -->|<1>|

Byte 2:  [u3[4:1]          | u4[3:0]  ]   = u3(4) + u4(4) = 8 bits
         |<-- 4 bits -->|<--4-->|

Byte 3:  [u4[4]    | u5[4:0]   | u6[1:0]] = u4(1) + u5(5) + u6(2) = 8 bits
         |1|<-- 5 bits -->|<-2->|

Byte 4:  [u6[4:2]        | u7[4:0]    ]   = u6(3) + u7(5) = 8 bits
         |<-3->|<--- 5 bits --->|

Total:   5 bytes for 8 values = 0.625 bytes/value
```

**Storage per block** (block_size = 64):

```
scale:   4 bytes (f32)
data:    ceil(64 * 5 / 8) = 40 bytes
total:   44 bytes
ratio:   256 / 44 = 5.82x
```

### 3.4 Tier 3: 3-Bit Quantization (Cold)

**Algorithm**: Symmetric per-block, 3-bit codes with optional two-level scale.

#### Standard Mode

```
Given: block of N f32 values
  scale    = max_abs(values) / 3           // qmax = 2^(3-1) - 1 = 3
  q[i]     = round(values[i] / scale)
  q[i]     = clamp(q[i], -3, +3)
  u[i]     = q[i] + 3                     // bias to unsigned [0, 6], fits 3 bits
  pack u[i] values using codec_bits at width=3
```

#### Two-Level Scale Mode (Outlier Handling)

When the value distribution has outliers (values significantly larger than the
bulk of the distribution), a single scale wastes most of the 3-bit range on the
long tail. The two-level scale splits the range:

```
Given: block of N f32 values, outlier_fraction (default: 0.05)
  sorted_abs     = sort(|values|, descending)
  outlier_count  = ceil(N * outlier_fraction)
  primary_max    = sorted_abs[outlier_count]      // excludes top 5%
  secondary_max  = sorted_abs[0]                  // full range

  primary_scale   = primary_max / 3               // covers bulk values
  secondary_scale = secondary_max / 3             // covers outliers

  For each value[i]:
    if |value[i]| > primary_max:
      flag[i] = 1                                  // use secondary scale
      q[i]    = round(value[i] / secondary_scale)
    else:
      flag[i] = 0                                  // use primary scale
      q[i]    = round(value[i] / primary_scale)
    q[i] = clamp(q[i], -3, +3)
    u[i] = q[i] + 3

  store: primary_scale (f32) + secondary_scale (f32) + flag bits + packed codes
```

**Bit-packing layout** (8 values packed into 3 bytes):

```
Values:     u0       u1       u2       u3       u4       u5       u6       u7
Bits:    [2..0]   [2..0]   [2..0]   [2..0]   [2..0]   [2..0]   [2..0]   [2..0]
         3 bits   3 bits   3 bits   3 bits   3 bits   3 bits   3 bits   3 bits

Packed into 3 bytes (24 bits = 8 * 3 bits):

Byte 0:  [u0[2:0]  | u1[2:0]  | u2[1:0] ]   = u0(3) + u1(3) + u2(2) = 8 bits
         |<-3->|<-3->|<2>|

Byte 1:  [u2[2]  | u3[2:0]  | u4[2:0]  | u5[0]]  = u2(1) + u3(3) + u4(3) + u5(1) = 8 bits
         |1|<-3->|<-3->|1|

Byte 2:  [u5[2:1]  | u6[2:0]  | u7[2:0]  ]   = u5(2) + u6(3) + u7(3) = 8 bits
         |<2>|<-3->|<-3->|

Total:   3 bytes for 8 values = 0.375 bytes/value
```

**Two-level scale storage layout** (one block, block_size = 64):

```
Byte offset:  0         3         7        8       9    ...   15       16  ...
             [primary_scale f32] [secondary_scale f32] [flag bytes  ] [packed codes]
              ~~~~~~~~~~~~~~~~~~  ~~~~~~~~~~~~~~~~~~~~~~ ~~~~~~~~~~~~  ~~~~~~~~~~~~~
              4 bytes             4 bytes               ceil(64/8)=8   ceil(64*3/8)=24

Total per block (two-level): 4 + 4 + 8 + 24 = 40 bytes
Total per block (standard):  4 + 24          = 28 bytes
ratio (standard):  256 / 28 = 9.14x
ratio (two-level): 256 / 40 = 6.40x
```

The two-level mode trades compression ratio for outlier fidelity. It is selected
automatically when the ratio `max_abs / median_abs` exceeds a configurable
threshold (default: 5.0), indicating a heavy-tailed distribution.

### 3.5 Tier 0: Compression to Zero (Absent)

**Algorithm**: No quantized data is stored.

```
Tier 0 representation:
  metadata:            TensorMeta (id, shape, dtype, timestamps)
  reconstruct_policy:  Option<ReconstructPolicy>
  quantized_data:      None

enum ReconstructPolicy {
    None,                                    // reads return zeros
    Delta { base_id: TensorId, delta: ... }, // reconstruct as base + delta
    Factor { source_id: TensorId, ... },     // reconstruct via transformation
}
```

**Read semantics**:

| `reconstruct_policy` | Behavior |
|----------------------|----------|
| `None` | Return a zero-filled tensor of the recorded shape. Fast-fail mode returns `Err(TierZeroNoPolicy)` instead. |
| `Delta` | Load base tensor, apply stored delta. May trigger recursive decompression if base is also tiered. |
| `Factor` | Load source tensor, apply stored transformation (scale, permutation, projection). |

**Transition to Tier 0**: A tensor is eligible for Tier 0 when its tier score
drops below `absent_min_score` (default: 1) and it has not been accessed for
longer than `absent_age_threshold` (default: 24 hours). The transition is
irreversible without external data: once quantized data is discarded, only the
reconstruction policy (if any) can recover approximate values.

### 3.6 Bit Packing Module: `codec_bits`

The core packing and unpacking functions shared by all sub-byte formats.

```rust
/// Errors from bit codec operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecErr {
    /// Output buffer too small. Contains the required size in bytes.
    OutputTooSmall { required: usize },
    /// Input buffer too small for the declared number of values.
    InputTooSmall { required: usize },
    /// Bit width must be in [1, 8].
    InvalidBitWidth { bits: u8 },
}

/// Pack `values.len()` signed codes into `out`, using `bits` bits per code.
///
/// Each value in `values` is treated as a signed integer in `[-(2^(bits-1)-1), 2^(bits-1)-1]`.
/// It is biased to unsigned before packing: `u = v + (2^(bits-1) - 1)`.
///
/// Returns the number of bytes written to `out`.
///
/// # Errors
/// - `CodecErr::OutputTooSmall` if `out` cannot hold the packed data.
/// - `CodecErr::InvalidBitWidth` if `bits` is 0 or greater than 8.
pub fn pack_bits(values: &[i8], bits: u8, out: &mut [u8]) -> Result<usize, CodecErr> {
    if bits == 0 || bits > 8 {
        return Err(CodecErr::InvalidBitWidth { bits });
    }
    let total_bits = values.len() as u64 * bits as u64;
    let required = ((total_bits + 7) / 8) as usize;
    if out.len() < required {
        return Err(CodecErr::OutputTooSmall { required });
    }

    let qmax = (1i8 << (bits - 1)) - 1;  // bias offset
    let mask: u64 = (1u64 << bits) - 1;
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut pos: usize = 0;

    for &v in values {
        let u = (v as i16 + qmax as i16) as u64 & mask;
        acc |= u << acc_bits;
        acc_bits += bits as u32;
        while acc_bits >= 8 {
            out[pos] = (acc & 0xFF) as u8;
            pos += 1;
            acc >>= 8;
            acc_bits -= 8;
        }
    }
    // Flush remaining bits
    if acc_bits > 0 {
        out[pos] = (acc & 0xFF) as u8;
        pos += 1;
    }
    Ok(pos)
}

/// Unpack codes from `inp` into `out`, reading `bits` bits per code.
///
/// Reads exactly `out.len()` values. Each unsigned code is unbiased back to signed:
/// `v = u - (2^(bits-1) - 1)`.
///
/// Returns the number of bytes consumed from `inp`.
///
/// # Errors
/// - `CodecErr::InputTooSmall` if `inp` does not contain enough data.
/// - `CodecErr::InvalidBitWidth` if `bits` is 0 or greater than 8.
pub fn unpack_bits(inp: &[u8], bits: u8, out: &mut [i8]) -> Result<usize, CodecErr> {
    if bits == 0 || bits > 8 {
        return Err(CodecErr::InvalidBitWidth { bits });
    }
    let total_bits = out.len() as u64 * bits as u64;
    let required = ((total_bits + 7) / 8) as usize;
    if inp.len() < required {
        return Err(CodecErr::InputTooSmall { required });
    }

    let qmax = (1i8 << (bits - 1)) - 1;
    let mask: u64 = (1u64 << bits) - 1;
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut byte_pos: usize = 0;
    let mut val_pos: usize = 0;

    while val_pos < out.len() {
        while acc_bits < bits as u32 {
            acc |= (inp[byte_pos] as u64) << acc_bits;
            acc_bits += 8;
            byte_pos += 1;
        }
        let u = (acc & mask) as i16;
        out[val_pos] = (u - qmax as i16) as i8;
        acc >>= bits;
        acc_bits -= bits as u32;
        val_pos += 1;
    }
    Ok(required)
}
```

**Properties**:

- No heap allocations. Callers provide both input and output slices.
- Single bit writer / bit reader using a 64-bit accumulator.
- Deterministic little-endian byte order.
- The `pack_bits` / `unpack_bits` pair is its own inverse: `unpack(pack(v)) == v`
  for all valid inputs.

### 3.7 Quant Module Functions

```rust
/// Block-level quantization configuration.
pub struct QuantConfig {
    pub block_size: usize,          // elements per quantization block (default: 64)
    pub two_level_threshold: f32,   // max/median ratio to trigger two-level (default: 5.0)
}

/// Quantized block result.
pub struct QuantizedBlock {
    pub scale: f32,
    pub secondary_scale: Option<f32>,    // only for two-level 3-bit
    pub flags: Option<Vec<u8>>,          // 1-bit-per-value flags for two-level
    pub codes: Vec<i8>,                  // signed quantized codes
    pub bits: u8,
}

/// Symmetric 8-bit quantization (Tier 1 - Hot).
///
/// Quantizes each block of `block_size` values independently.
/// scale = max_abs(block) / 127
/// q[i]  = clamp(round(x[i] / scale), -127, 127)
pub fn quantize_s8(
    values: &[f32],
    config: &QuantConfig,
) -> Vec<QuantizedBlock>;

/// Symmetric N-bit quantization (Tier 2/3 - Warm/Cold).
///
/// `bits` must be one of: 7, 5, 3.
/// qmax  = 2^(bits-1) - 1
/// scale = max_abs(block) / qmax
/// q[i]  = clamp(round(x[i] / scale), -qmax, qmax)
///
/// For bits=3 and config.two_level_threshold exceeded: uses two-level scale.
pub fn quantize_bits(
    values: &[f32],
    bits: u8,
    config: &QuantConfig,
) -> Vec<QuantizedBlock>;

/// Dequantize a block back to f32 values.
///
/// For standard mode:   x'[i] = codes[i] as f32 * scale
/// For two-level mode:  x'[i] = codes[i] as f32 * (if flags[i] then secondary_scale else scale)
pub fn dequantize(block: &QuantizedBlock) -> Vec<f32>;

/// Compute the maximum absolute value across a slice.
///
/// On native targets with `target_feature = "avx2"` or `target_feature = "neon"`:
///   uses SIMD intrinsics for 4-8x throughput.
/// On WASM with `target_feature = "simd128"` (optional):
///   uses wasm_simd128 intrinsics.
/// Fallback: portable scalar loop.
#[inline]
pub fn max_abs(values: &[f32]) -> f32;
```

**SIMD implementation sketch for `max_abs`** (AVX2):

```rust
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn max_abs_avx2(values: &[f32]) -> f32 {
    use std::arch::x86_64::*;
    let sign_mask = _mm256_set1_ps(f32::from_bits(0x7FFF_FFFF)); // abs mask
    let mut vmax = _mm256_setzero_ps();
    let chunks = values.len() / 8;

    for i in 0..chunks {
        let v = _mm256_loadu_ps(values.as_ptr().add(i * 8));
        let abs_v = _mm256_and_ps(v, sign_mask);
        vmax = _mm256_max_ps(vmax, abs_v);
    }

    // Horizontal max reduction
    let hi128 = _mm256_extractf128_ps(vmax, 1);
    let lo128 = _mm256_castps256_ps128(vmax);
    let max128 = _mm_max_ps(hi128, lo128);
    let shuf = _mm_movehdup_ps(max128);
    let max64 = _mm_max_ps(max128, shuf);
    let shuf2 = _mm_movehl_ps(max64, max64);
    let max32 = _mm_max_ss(max64, shuf2);
    let mut result = _mm_cvtss_f32(max32);

    // Handle remainder
    for i in (chunks * 8)..values.len() {
        result = result.max(values[i].abs());
    }
    result
}
```

**WASM portable fallback**:

```rust
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn max_abs(values: &[f32]) -> f32 {
    let mut m: f32 = 0.0;
    for &v in values {
        let a = v.abs();
        if a > m {
            m = a;
        }
    }
    m
}
```

When WASM SIMD is enabled via `target_feature = "simd128"`, a vectorized path
processes 4 f32 values per iteration using `v128` types. This is optional and
gated behind a cargo feature flag `wasm-simd`.

### 3.8 Error Bound Analysis

For symmetric quantization with bit width `B`, block scale `s`, and `qmax = 2^(B-1) - 1`:

```
quantization_step   = s / qmax
max_element_error   = quantization_step / 2         (from rounding)
max_relative_error  = 1 / (2 * qmax)                (per element, worst case)
rms_error           = quantization_step / sqrt(12)   (uniform quantization noise)
```

**Per-tier error bounds**:

| Tier | Bits | qmax | Max Rel. Error | RMS Rel. Error | Max Abs. Error (scale=1.0) |
|------|------|------|---------------|----------------|---------------------------|
| Hot (8-bit) | 8 | 127 | 0.394% | 0.228% | 0.00394 |
| Warm (7-bit) | 7 | 63 | 0.794% | 0.458% | 0.00794 |
| Warm-agg (5-bit) | 5 | 15 | 3.333% | 1.925% | 0.03333 |
| Cold (3-bit, std) | 3 | 3 | 16.667% | 9.623% | 0.16667 |
| Cold (3-bit, 2-level) | 3 | 3 | 16.667% per scale | 9.623% | Reduced for bulk values |

**Two-level scale improvement for 3-bit**: When 95% of values fall within
`primary_max` and outliers use `secondary_scale`:

| Component | Fraction | Scale | Effective Max Error |
|-----------|----------|-------|-------------------|
| Bulk values (95%) | 0.95 | primary_scale (smaller) | 16.7% of primary_max |
| Outlier values (5%) | 0.05 | secondary_scale (larger) | 16.7% of secondary_max |

The bulk values achieve much lower absolute error because `primary_scale` is
typically 3-10x smaller than the single-scale `scale`. The outliers retain the
same relative error but are fewer in number.

**Drift compounding**: When drift tolerance is `d` (e.g., 10%), and a frame is
quantized with scales from an earlier frame, the effective max relative error
becomes `(1 + d) / (2 * qmax)`. For 8-bit with 10% drift: `1.1 / 254 = 0.433%`.

**Cumulative error table with drift**:

| Tier | Bits | No Drift | 10% Drift | 20% Drift |
|------|------|----------|-----------|-----------|
| Hot | 8 | 0.394% | 0.433% | 0.472% |
| Warm | 7 | 0.794% | 0.873% | 0.952% |
| Warm-agg | 5 | 3.333% | 3.667% | 4.000% |
| Cold | 3 | 16.667% | 18.333% | 20.000% |

### 3.9 Complete Quantizer and Packer Traits

```rust
/// Trait for quantization formats that can encode and decode tensor blocks.
pub trait TensorQuantizer {
    /// The bit width of this quantizer.
    fn bit_width(&self) -> u8;

    /// Quantize a block of f32 values into signed codes and scale(s).
    fn quantize_block(
        &self,
        values: &[f32],
        config: &QuantConfig,
    ) -> QuantizedBlock;

    /// Dequantize a block back to f32 values.
    fn dequantize_block(
        &self,
        block: &QuantizedBlock,
        out: &mut [f32],
    ) -> Result<(), CodecErr>;

    /// Returns the packed byte size for `num_values` at this bit width,
    /// excluding scale storage.
    fn packed_data_size(&self, num_values: usize) -> usize {
        (num_values * self.bit_width() as usize + 7) / 8
    }

    /// Returns total block storage size including scale(s) and flags.
    fn block_storage_size(&self, block_size: usize) -> usize;
}

/// Trait for bit-level packing codecs.
pub trait BitCodec {
    /// Pack signed codes into a byte buffer.
    fn pack(
        &self,
        codes: &[i8],
        bits: u8,
        out: &mut [u8],
    ) -> Result<usize, CodecErr>;

    /// Unpack codes from a byte buffer.
    fn unpack(
        &self,
        data: &[u8],
        bits: u8,
        out: &mut [i8],
    ) -> Result<usize, CodecErr>;
}

/// Standard implementation using the accumulator-based codec_bits functions.
pub struct StandardBitCodec;

impl BitCodec for StandardBitCodec {
    fn pack(
        &self,
        codes: &[i8],
        bits: u8,
        out: &mut [u8],
    ) -> Result<usize, CodecErr> {
        pack_bits(codes, bits, out)
    }

    fn unpack(
        &self,
        data: &[u8],
        bits: u8,
        out: &mut [i8],
    ) -> Result<usize, CodecErr> {
        unpack_bits(data, bits, out)
    }
}
```

### 3.10 Block Storage Summary Diagram

```
TIER 1 (8-bit):
+--------+-------+-------+-------+-----+-------+
| scale  | q[0]  | q[1]  | q[2]  | ... | q[63] |
| f32 LE | i8    | i8    | i8    |     | i8    |
+--------+-------+-------+-------+-----+-------+
  4 bytes   1       1       1             1        = 68 bytes / block

TIER 2a (7-bit):
+--------+--------------------------------------------+
| scale  | packed 7-bit codes (56 bytes for 64 vals)   |
| f32 LE | bitstream, little-endian accumulator        |
+--------+--------------------------------------------+
  4 bytes   ceil(64*7/8) = 56 bytes                    = 60 bytes / block

TIER 2b (5-bit):
+--------+--------------------------------------------+
| scale  | packed 5-bit codes (40 bytes for 64 vals)   |
| f32 LE | bitstream, little-endian accumulator        |
+--------+--------------------------------------------+
  4 bytes   ceil(64*5/8) = 40 bytes                    = 44 bytes / block

TIER 3 standard (3-bit):
+--------+--------------------------------------------+
| scale  | packed 3-bit codes (24 bytes for 64 vals)   |
| f32 LE | bitstream, little-endian accumulator        |
+--------+--------------------------------------------+
  4 bytes   ceil(64*3/8) = 24 bytes                    = 28 bytes / block

TIER 3 two-level (3-bit):
+--------+--------+----------+-------------------------------+
| pscale | sscale | flags    | packed 3-bit codes            |
| f32 LE | f32 LE | ceil(N/8)| bitstream                     |
+--------+--------+----------+-------------------------------+
  4         4        8 bytes    24 bytes                       = 40 bytes / block

TIER 0 (absent):
+--------------------------------------+
| TensorMeta + ReconstructPolicy only  |
| NO quantized data                    |
+--------------------------------------+
  variable (typically 32-128 bytes metadata)
```

---

## 4. Alternatives Considered

### 4.1 4-Bit as the Warm Tier

4-bit quantization (qmax = 7, 8.00x compression) is the most widely studied
format (GPTQ, AWQ). We considered using 4-bit instead of 7-bit for the warm
tier. **Rejected** because: (a) the jump from 8-bit to 4-bit is too large for
tensors that were recently hot, causing unnecessary quality loss; (b) 7-bit
provides a gentler step-down; (c) 5-bit is available as an intermediate when
memory pressure increases.

### 4.2 Uniform 4-Bit Across All Non-Hot Tiers

A simpler design with only two quantization levels (8-bit hot, 4-bit everything
else). **Rejected** because: (a) cold tensors waste 1 extra bit per value when
3-bit suffices; (b) no path to aggressive compression under memory pressure;
(c) loses the granularity that enables smooth quality degradation.

### 4.3 Asymmetric Quantization for 3-Bit

Using asymmetric quantization (with zero-point) for 3-bit to better utilize the
`[0, 7]` unsigned range when distributions are not centered. **Rejected**
because: (a) adds 4 bytes of zero-point storage per block; (b) requires an
additional subtraction in the dequantize path; (c) the two-level scale approach
handles asymmetric distributions more effectively by splitting the scale rather
than shifting the range.

### 4.4 Lookup Table (Codebook) Quantization for Cold

Using a small codebook (e.g., 8 centroids) instead of uniform 3-bit levels.
**Rejected** because: (a) requires a per-block or per-tensor codebook training
step that is expensive for streaming data; (b) codebook storage overhead is
comparable to scale storage but with higher decode complexity; (c) uniform
quantization is simpler to implement and reason about.

### 4.5 No Two-Level Scale (Simpler 3-Bit)

Omitting the two-level scale option entirely. **Considered but rejected** because
agent embedding tensors frequently exhibit heavy-tailed distributions where a few
dimensions carry disproportionate magnitude. Without two-level scale, these
outliers cause the single scale to be too large, wasting most of the 3-bit range
on the bulk of near-zero values.

---

## 5. Acceptance Criteria

### 5.1 Format Correctness

- [ ] `pack_bits` followed by `unpack_bits` is a lossless round-trip for all
      bit widths (3, 5, 7, 8) and all valid signed input ranges.
- [ ] `quantize_s8` followed by `dequantize` produces values within the
      theoretical error bound (`scale / 254`) of the originals.
- [ ] `quantize_bits(7, ...)` followed by `dequantize` produces values within
      `scale / 126` of the originals.
- [ ] `quantize_bits(5, ...)` followed by `dequantize` produces values within
      `scale / 30` of the originals.
- [ ] `quantize_bits(3, ...)` followed by `dequantize` produces values within
      `scale / 6` of the originals (standard mode).
- [ ] Two-level 3-bit mode activates when `max/median > two_level_threshold`.
- [ ] Tier 0 reads return zeros when `reconstruct_policy` is `None`.
- [ ] Tier 0 reads invoke reconstruction when a policy exists.

### 5.2 Performance

- [ ] `pack_bits` throughput >= 2 GB/s on native (AVX2-capable hardware).
- [ ] `unpack_bits` throughput >= 2 GB/s on native.
- [ ] `max_abs` with SIMD is >= 3x faster than the scalar fallback on 512+ element blocks.
- [ ] WASM `pack_bits` / `unpack_bits` throughput >= 500 MB/s (without SIMD).
- [ ] No heap allocations in `pack_bits`, `unpack_bits`, or `max_abs`.

### 5.3 Storage Efficiency

- [ ] 8-bit block storage: exactly `4 + block_size` bytes.
- [ ] 7-bit block storage: exactly `4 + ceil(block_size * 7 / 8)` bytes.
- [ ] 5-bit block storage: exactly `4 + ceil(block_size * 5 / 8)` bytes.
- [ ] 3-bit block storage (standard): exactly `4 + ceil(block_size * 3 / 8)` bytes.
- [ ] 3-bit block storage (two-level): exactly `8 + ceil(block_size / 8) + ceil(block_size * 3 / 8)` bytes.
- [ ] No padding bits between consecutive blocks in a segment.

### 5.4 Dynamic Tier 2 Downgrade

- [ ] When aggregate warm storage exceeds `warm_byte_cap`, the least recently
      accessed warm tensors are re-encoded from 7-bit to 5-bit.
- [ ] The downgrade is reversible: if warm storage drops below
      `warm_byte_cap * 0.8` (hysteresis), tensors can be re-promoted to 7-bit
      on next access.

---

## 6. Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| 3-bit two-level scale adds format complexity without sufficient accuracy gain for most distributions | Medium | Medium | Gate behind a cargo feature `two-level-cold`; default to standard 3-bit. Benchmark on real agent embeddings before enabling by default. |
| Dynamic 7-bit to 5-bit downgrade causes thrashing when warm set oscillates near the byte cap | Medium | Medium | Implement hysteresis (20% band). Only downgrade when above cap; only upgrade when below 80% of cap. Rate-limit downgrades to at most once per minute. |
| `pack_bits` accumulator overflow for large inputs | Low | Low | The 64-bit accumulator can hold up to 56 bits of pending data (7 bytes). Since we flush at 8 bits, the maximum pending bits is `bits - 1 = 7`, well within the 64-bit range. No overflow possible. |
| Tier 0 reconstruction from Delta/Factor introduces unbounded latency | Medium | Low | Set a maximum reconstruction depth (default: 3). If the base tensor is also Tier 0, fail with `ReconstructionDepthExceeded` rather than recursing indefinitely. |
| WASM scalar `max_abs` is a bottleneck for large tensors | Low | High | Expected. The WASM SIMD feature flag provides 3-4x improvement. For non-SIMD targets, `max_abs` cost is small relative to the full quantize pipeline. |
| Block size mismatch between encoder and decoder | High | Low | Block size is stored in the segment header (ADR-017 format). Decoder reads it from the header rather than assuming a default. |

---

## 7. References

1. ADR-017: Temporal Tensor Compression with Tiered Quantization. RuVector Architecture Team, 2026.
2. ADR-018: Block-Based Storage Engine for Temporal Tensor Segments (forthcoming).
3. Frantar, E., et al. "GPTQ: Accurate Post-Training Quantization for Generative Pre-trained Transformers." ICLR 2023.
4. Lin, J., et al. "AWQ: Activation-aware Weight Quantization for LLM Compression and Acceleration." MLSys 2024.
5. Kim, S., et al. "SqueezeLLM: Dense-and-Sparse Quantization." ICML 2024.
6. Liu, Z., et al. "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache." ICML 2024.
7. Pelkonen, T., et al. "Gorilla: A Fast, Scalable, In-Memory Time Series Database." VLDB 2015.
8. IEEE 754-2019. "IEEE Standard for Floating-Point Arithmetic."
9. Lemire, D. and Boytsov, L. "Decoding billions of integers in milliseconds through vectorized bit packing." Software: Practice and Experience, 2015.
10. WebAssembly SIMD Proposal. https://github.com/WebAssembly/simd. Finalized 2023.
