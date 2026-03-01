# RVF Wire Format Reference

## 1. File Structure

An RVF file is a byte stream with no fixed header at offset 0. All structure
is discovered from the tail.

```
Byte 0                                                               EOF
|                                                                      |
v                                                                      v
+--------+--------+--------+     +--------+---------+--------+---------+
| Seg 0  | Seg 1  | Seg 2  | ... | Seg N  | Seg N+1 | Seg N+2| Mfst K |
| VEC    | VEC    | INDEX  |     | VEC    | HOT     | INDEX  | MANIF  |
+--------+--------+--------+     +--------+---------+--------+---------+
                                                               ^       ^
                                                               |       |
                                                        Level 1 Mfst   |
                                                                Level 0
                                                              (last 4KB)
```

### Alignment Rule

Every segment starts at a **64-byte aligned** boundary. If a segment's
payload + footer does not end on a 64-byte boundary, zero-padding is inserted
before the next segment header.

### Byte Order

All multi-byte integers are **little-endian**. All floating-point values
are IEEE 754 little-endian. This matches x86, ARM (in default mode), and
WASM native byte order.

## 2. Primitive Types

```
Type        Size    Encoding
----        ----    --------
u8          1       Unsigned 8-bit integer
u16         2       Unsigned 16-bit little-endian
u32         4       Unsigned 32-bit little-endian
u64         8       Unsigned 64-bit little-endian
i32         4       Signed 32-bit little-endian (two's complement)
i64         8       Signed 64-bit little-endian (two's complement)
f16         2       IEEE 754 half-precision little-endian
f32         4       IEEE 754 single-precision little-endian
f64         8       IEEE 754 double-precision little-endian
varint      1-10    LEB128 unsigned variable-length integer
svarint     1-10    ZigZag + LEB128 signed variable-length integer
hash128     16      First 128 bits of hash output
hash256     32      First 256 bits of hash output
```

### Varint Encoding (LEB128)

```
Value 0-127:        1 byte   [0xxxxxxx]
Value 128-16383:    2 bytes  [1xxxxxxx 0xxxxxxx]
Value 16384-2097151: 3 bytes [1xxxxxxx 1xxxxxxx 0xxxxxxx]
...up to 10 bytes for u64
```

### Delta Encoding

Sequences of sorted integers use delta encoding:
```
Original:  [100, 105, 108, 120, 200]
Deltas:    [100,   5,   3,  12,  80]
Encoded:   [varint(100), varint(5), varint(3), varint(12), varint(80)]
```

With restart points every N entries, the first value in each restart group
is absolute (not delta-encoded).

## 3. Segment Header (64 bytes)

```
Offset  Type    Field              Notes
------  ----    -----              -----
0x00    u32     magic              Always 0x52564653 ("RVFS")
0x04    u8      version            Format version (1)
0x05    u8      seg_type           Segment type enum
0x06    u16     flags              See flags bitfield
0x08    u64     segment_id         Monotonic ordinal
0x10    u64     payload_length     Bytes after header, before footer
0x18    u64     timestamp_ns       UNIX nanoseconds
0x20    u8      checksum_algo      0=CRC32C, 1=XXH3-128, 2=SHAKE-256
0x21    u8      compression        0=none, 1=LZ4, 2=ZSTD, 3=custom
0x22    u16     reserved_0         Must be 0x0000
0x24    u32     reserved_1         Must be 0x00000000
0x28    hash128 content_hash       Payload hash (first 128 bits)
0x38    u32     uncompressed_len   Original payload size (0 if no compression)
0x3C    u32     alignment_pad      Zero padding to 64B boundary
```

### Segment Type Enum

```
0x00    INVALID         Not a valid segment
0x01    VEC_SEG         Vector payloads
0x02    INDEX_SEG       HNSW adjacency
0x03    OVERLAY_SEG     Graph overlay deltas
0x04    JOURNAL_SEG     Metadata mutations
0x05    MANIFEST_SEG    Segment directory
0x06    QUANT_SEG       Quantization dictionaries
0x07    META_SEG        Key-value metadata
0x08    HOT_SEG         Temperature-promoted data
0x09    SKETCH_SEG      Access counter sketches
0x0A    WITNESS_SEG     Capability manifests
0x0B    PROFILE_SEG     Domain profile declarations
0x0C    CRYPTO_SEG      Key material / certificate anchors
0x0D-0xEF  reserved
0xF0-0xFF  extension    Implementation-specific
```

### Flags Bitfield

```
Bit  Mask    Name         Meaning
---  ----    ----         -------
0    0x0001  COMPRESSED   Payload compressed per compression field
1    0x0002  ENCRYPTED    Payload encrypted (key in CRYPTO_SEG)
2    0x0004  SIGNED       Signature footer follows payload
3    0x0008  SEALED       Immutable (compaction output)
4    0x0010  PARTIAL      Partial/streaming write
5    0x0020  TOMBSTONE    Logically deletes prior segment
6    0x0040  HOT          Contains hot-tier data
7    0x0080  OVERLAY      Contains overlay/delta data
8    0x0100  SNAPSHOT     Full snapshot (not delta)
9    0x0200  CHECKPOINT   Safe rollback point
10-15        reserved     Must be zero
```

## 4. Signature Footer

Present only if `SIGNED` flag is set. Follows immediately after the payload.

```
Offset  Type    Field           Notes
------  ----    -----           -----
0x00    u16     sig_algo        0=Ed25519, 1=ML-DSA-65, 2=SLH-DSA-128s
0x02    u16     sig_length      Signature byte length
0x04    u8[]    signature       Signature bytes
var     u32     footer_length   Total footer size (for backward scan)
```

### Signature Algorithm Sizes

| Algorithm | sig_length | Post-Quantum | Performance |
|-----------|-----------|-------------|-------------|
| Ed25519 | 64 B | No | ~76,000 sign/s |
| ML-DSA-65 | 3,309 B | Yes (NIST Level 3) | ~4,500 sign/s |
| SLH-DSA-128s | 7,856 B | Yes (NIST Level 1) | ~350 sign/s |

## 5. VEC_SEG Payload Layout

Vector segments store blocks of vectors in columnar layout for compression.

```
+------------------------------------------+
| VEC_SEG Payload                          |
+------------------------------------------+
| Block Directory                          |
|   block_count: u32                       |
|   For each block:                        |
|     block_offset: u32 (from payload start)|
|     vector_count: u32                    |
|     dim: u16                             |
|     dtype: u8                            |
|     tier: u8                             |
|   [64B aligned]                          |
+------------------------------------------+
| Block 0                                  |
|   +-- Columnar Vectors --+               |
|   | dim_0[0..count]      |  <- all vals  |
|   | dim_1[0..count]      |     for dim 0 |
|   | ...                  |     then dim 1 |
|   | dim_D[0..count]      |     etc.      |
|   +----------------------+               |
|   +-- ID Map --+                         |
|   | encoding: u8 (0=raw, 1=delta-varint) |
|   | restart_interval: u16                |
|   | id_count: u32                        |
|   | [restart_offsets: u32[]] (if delta)   |
|   | [ids: encoded]                       |
|   +-----------+                          |
|   +-- Block CRC --+                      |
|   | crc32c: u32    |                     |
|   +----------------+                     |
|   [64B padding]                          |
+------------------------------------------+
| Block 1                                  |
|   ...                                    |
+------------------------------------------+
```

### Data Type Enum

```
0x00    f32     32-bit float
0x01    f16     16-bit float
0x02    bf16    bfloat16
0x03    i8      signed 8-bit integer (scalar quantized)
0x04    u8      unsigned 8-bit integer
0x05    i4      4-bit integer (packed, 2 per byte)
0x06    binary  1-bit (packed, 8 per byte)
0x07    pq      Product-quantized codes
0x08    custom  Custom encoding (see QUANT_SEG)
```

### Columnar vs Interleaved

**VEC_SEG** (columnar): `dim_0[all], dim_1[all], ..., dim_D[all]`
- Better compression (similar values adjacent)
- Better for batch operations
- Worse for single-vector random access

**HOT_SEG** (interleaved): `vec_0[all_dims], vec_1[all_dims], ...`
- Better for single-vector access (one cache line per vector)
- Better for top-K refinement (sequential scan)
- No compression benefit

## 6. INDEX_SEG Payload Layout

```
+------------------------------------------+
| INDEX_SEG Payload                        |
+------------------------------------------+
| Index Header                             |
|   index_type: u8 (0=HNSW, 1=IVF, 2=flat)|
|   layer_level: u8 (A=0, B=1, C=2)       |
|   M: u16 (HNSW max neighbors per layer)  |
|   ef_construction: u32                   |
|   node_count: u64                        |
|   [64B aligned]                          |
+------------------------------------------+
| Restart Point Index                      |
|   restart_interval: u32                  |
|   restart_count: u32                     |
|   [restart_offset: u32] * count          |
|   [64B aligned]                          |
+------------------------------------------+
| Adjacency Data                           |
|   For each node (sorted by node_id):     |
|     layer_count: varint                  |
|     For each layer:                      |
|       neighbor_count: varint             |
|       [delta_neighbor_id: varint] * cnt  |
|   [64B padding per restart group]        |
+------------------------------------------+
| Prefetch Hints (optional)                |
|   hint_count: u32                        |
|   For each hint:                         |
|     node_range_start: u64                |
|     node_range_end: u64                  |
|     page_offset: u64                     |
|     page_count: u32                      |
|     prefetch_ahead: u32                  |
|   [64B aligned]                          |
+------------------------------------------+
```

## 7. HOT_SEG Payload Layout

The hot segment stores the most-accessed vectors in interleaved (row-major)
layout with their neighbor lists co-located for cache locality.

```
+------------------------------------------+
| HOT_SEG Payload                          |
+------------------------------------------+
| Hot Header                               |
|   vector_count: u32                      |
|   dim: u16                               |
|   dtype: u8 (f16 or i8)                  |
|   neighbor_M: u16                        |
|   [64B aligned]                          |
+------------------------------------------+
| Interleaved Hot Data                     |
|   For each hot vector:                   |
|     vector_id: u64                       |
|     vector: [dtype * dim]                |
|     neighbor_count: u16                  |
|     [neighbor_id: u64] * neighbor_count  |
|     [64B aligned per entry]              |
+------------------------------------------+
```

Each hot entry is self-contained: vector + neighbors in one contiguous block.
A sequential scan of the HOT_SEG for top-K refinement reads vectors and
neighbors without any pointer chasing.

### Hot Entry Size Example

For 384-dim fp16 vectors with M=16 neighbors:
```
8 (id) + 768 (vector) + 2 (count) + 128 (neighbors) = 906 bytes
Padded to 64B: 960 bytes per entry
```

1000 hot vectors = 960 KB (fits in L2 cache on most CPUs).

## 8. MANIFEST_SEG Payload Layout

```
+------------------------------------------+
| MANIFEST_SEG Payload                     |
+------------------------------------------+
| TLV Records (Level 1 manifest)           |
|   For each record:                       |
|     tag: u16                             |
|     length: u32                          |
|     pad: u16 (to 8B alignment)           |
|     value: [u8; length]                  |
|     [8B aligned]                         |
+------------------------------------------+
| Level 0 Root Manifest (last 4096 bytes)  |
|   (See 02-manifest-system.md for layout) |
+------------------------------------------+
```

## 9. SKETCH_SEG Payload Layout

```
+------------------------------------------+
| SKETCH_SEG Payload                       |
+------------------------------------------+
| Sketch Header                            |
|   block_count: u32                       |
|   width: u32 (counters per row)          |
|   depth: u32 (hash functions)            |
|   counter_bits: u8 (8 or 16)            |
|   decay_shift: u8 (aging right-shift)    |
|   total_accesses: u64                    |
|   [64B aligned]                          |
+------------------------------------------+
| Sketch Data                              |
|   For each block:                        |
|     block_id: u32                        |
|     counters: [u8; width * depth]        |
|   [64B aligned per block]               |
+------------------------------------------+
```

## 10. QUANT_SEG Payload Layout

```
+------------------------------------------+
| QUANT_SEG Payload                        |
+------------------------------------------+
| Quant Header                             |
|   quant_type: u8                         |
|     0 = scalar (min-max per dim)         |
|     1 = product quantization             |
|     2 = binary threshold                 |
|     3 = residual PQ                      |
|   tier: u8                               |
|   dim: u16                               |
|   [64B aligned]                          |
+------------------------------------------+
| Type-specific data:                      |
|                                          |
| Scalar (type 0):                         |
|   min: [f32; dim]                        |
|   max: [f32; dim]                        |
|                                          |
| PQ (type 1):                             |
|   M: u16 (subspaces)                     |
|   K: u16 (centroids per sub)             |
|   sub_dim: u16 (dims per sub)            |
|   codebook: [f32; M * K * sub_dim]       |
|                                          |
| Binary (type 2):                         |
|   threshold: [f32; dim]                  |
|                                          |
| Residual PQ (type 3):                    |
|   coarse_centroids: [f32; K_coarse * dim]|
|   residual_codebook: [f32; M * K * sub]  |
|                                          |
| [64B aligned]                            |
+------------------------------------------+
```

## 11. Checksum Algorithms

| ID | Algorithm | Output | Speed (HW accel) | Use Case |
|----|-----------|--------|-------------------|----------|
| 0 | CRC32C | 4 B (stored in 16B field, zero-padded) | ~3 GB/s (SSE4.2) | Per-block integrity |
| 1 | XXH3-128 | 16 B | ~50 GB/s (AVX2) | Segment content hash |
| 2 | SHAKE-256 | 16 or 32 B | ~1 GB/s | Cryptographic verification |

Default recommendation:
- Block-level CRC: CRC32C (fastest, hardware accelerated)
- Segment content hash: XXH3-128 (fast, good distribution)
- Crypto witness hashes: SHAKE-256 (post-quantum safe)

## 12. Compression

| ID | Algorithm | Ratio | Decompress Speed | Use Case |
|----|-----------|-------|-----------------|----------|
| 0 | None | 1.0x | N/A | Hot tier |
| 1 | LZ4 | 1.5-3x | ~4 GB/s | Warm tier, low latency |
| 2 | ZSTD | 3-6x | ~1.5 GB/s | Cold tier, high ratio |
| 3 | Custom | Varies | Varies | Domain-specific |

Compression is applied per-segment payload. Individual blocks within a
segment share the same compression.

## 13. Tail Scan Algorithm

```python
def find_latest_manifest(file):
    file_size = file.seek(0, SEEK_END)

    # Try fast path: last 4096 bytes
    file.seek(file_size - 4096)
    root = file.read(4096)
    if root[0:4] == b'RVM0' and verify_crc(root):
        return parse_root_manifest(root)

    # Slow path: scan backward for MANIFEST_SEG header
    scan_pos = file_size - 64  # Start at last 64B boundary
    while scan_pos >= 0:
        file.seek(scan_pos)
        header = file.read(64)
        if (header[0:4] == b'RVFS' and
            header[5] == 0x05 and  # MANIFEST_SEG
            verify_segment_header(header)):
            return parse_manifest_segment(file, scan_pos)
        scan_pos -= 64  # Previous 64B boundary

    raise CorruptFileError("No valid MANIFEST_SEG found")
```

Worst case: full backward scan at 64B granularity. For a 4 GB file, this is
67M checks — but each check is a 4-byte comparison, so it completes in ~100ms
on a modern CPU with mmap. In practice, the fast path succeeds on the first try
for non-corrupt files.
