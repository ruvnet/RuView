# RVF Segment Model

## 1. Append-Only Segment Architecture

An RVF file is a linear sequence of **segments**. Each segment is a self-contained,
independently verifiable unit. New data is always appended — never inserted into or
overwritten within existing segments.

```
+------------+------------+------------+     +------------+
| Segment 0  | Segment 1  | Segment 2  | ... | Segment N  |  <-- EOF
+------------+------------+------------+     +------------+
                                                    ^
                                            Latest MANIFEST_SEG
                                            (source of truth)
```

### Why Append-Only

| Property | Benefit |
|----------|---------|
| Write amplification | Zero — each byte written once until compaction |
| Crash safety | Partial segment at tail is detectable and discardable |
| Concurrent reads | Readers see a consistent snapshot at any manifest boundary |
| Streaming ingest | Writer never blocks on reorganization |
| mmap friendliness | Pages only grow — no invalidation of mapped regions |

## 2. Segment Header

Every segment begins with a fixed 64-byte header. The header is 64-byte aligned
to match SIMD register width.

```
Offset  Size  Field              Description
------  ----  -----              -----------
0x00    4     magic              0x52564653 ("RVFS" in ASCII)
0x04    1     version            Segment format version (currently 1)
0x05    1     seg_type           Segment type enum (see below)
0x06    2     flags              Bitfield: compressed, encrypted, signed, sealed, etc.
0x08    8     segment_id         Monotonically increasing segment ordinal
0x10    8     payload_length     Byte length of payload (after header, before footer)
0x18    8     timestamp_ns       Nanosecond UNIX timestamp of segment creation
0x20    1     checksum_algo      Hash algorithm enum: 0=CRC32C, 1=XXH3-128, 2=SHAKE-256
0x21    1     compression        Compression enum: 0=none, 1=LZ4, 2=ZSTD, 3=custom
0x22    2     reserved_0         Must be zero
0x24    4     reserved_1         Must be zero
0x28    16    content_hash       First 128 bits of payload hash (algorithm per checksum_algo)
0x38    4     uncompressed_len   Original payload size (0 if no compression)
0x3C    4     alignment_pad      Padding to reach 64-byte boundary
```

**Total header**: 64 bytes (one cache line, one AVX-512 register width).

### Magic Validation

Readers scanning backward from EOF look for `0x52564653` at 64-byte aligned
boundaries. This enables fast tail-scan even on corrupted files.

### Flags Bitfield

```
Bit 0:  COMPRESSED    Payload is compressed per compression field
Bit 1:  ENCRYPTED     Payload is encrypted (key info in manifest)
Bit 2:  SIGNED        A signature footer follows the payload
Bit 3:  SEALED        Segment is immutable (compaction output)
Bit 4:  PARTIAL       Segment is a partial write (streaming ingest)
Bit 5:  TOMBSTONE     Segment logically deletes a prior segment
Bit 6:  HOT           Segment contains temperature-promoted data
Bit 7:  OVERLAY       Segment contains overlay/delta data
Bit 8:  SNAPSHOT      Segment contains full snapshot (not delta)
Bit 9:  CHECKPOINT    Segment is a safe rollback point
Bits 10-15: reserved
```

## 3. Segment Types

```
Value  Name            Purpose
-----  ----            -------
0x01   VEC_SEG         Raw vector payloads (the actual embeddings)
0x02   INDEX_SEG       HNSW adjacency lists, entry points, routing tables
0x03   OVERLAY_SEG     Graph overlay deltas, partition updates, min-cut witnesses
0x04   JOURNAL_SEG     Metadata mutations (label changes, deletions, moves)
0x05   MANIFEST_SEG    Segment directory, hotset pointers, epoch state
0x06   QUANT_SEG       Quantization dictionaries and codebooks
0x07   META_SEG        Arbitrary key-value metadata (tags, provenance, lineage)
0x08   HOT_SEG         Temperature-promoted hot data (vectors + neighbors)
0x09   SKETCH_SEG      Access counter sketches for temperature decisions
0x0A   WITNESS_SEG     Capability manifests, proof of computation, audit trails
0x0B   PROFILE_SEG     Domain profile declarations (RVDNA, RVText, etc.)
0x0C   CRYPTO_SEG      Key material, signature chains, certificate anchors
0x0D   METAIDX_SEG     Metadata inverted indexes for filtered search
```

### Reserved Range

Types `0x00` and `0xF0`-`0xFF` are reserved. `0x00` indicates an uninitialized
or zeroed region (not a valid segment). `0xF0`-`0xFF` are reserved for
implementation-specific extensions.

## 4. Segment Footer

If the `SIGNED` flag is set, the payload is followed by a signature footer:

```
Offset  Size   Field              Description
------  ----   -----              -----------
0x00    2      sig_algo           Signature algorithm: 0=Ed25519, 1=ML-DSA-65, 2=SLH-DSA-128s
0x02    2      sig_length         Byte length of signature
0x04    var    signature          The signature bytes
var     4      footer_length      Total footer size (for backward scanning)
```

Unsigned segments have no footer — the next segment header follows immediately
after the payload (at the next 64-byte aligned boundary).

## 5. Segment Lifecycle

### Write Path

```
1. Allocate segment ID (monotonic counter)
2. Compute payload hash
3. Write header + payload + optional footer
4. fsync (or fdatasync for non-manifest segments)
5. Write MANIFEST_SEG referencing the new segment
6. fsync the manifest
```

The two-fsync protocol ensures that:
- If crash occurs before step 6, the orphan segment is harmless (no manifest points to it)
- If crash occurs during step 6, the partial manifest is detectable (bad hash)
- After step 6, the segment is durably committed

### Read Path

```
1. Seek to EOF
2. Scan backward for latest MANIFEST_SEG (look for magic at aligned boundaries)
3. Parse manifest -> get segment directory
4. Map segments on demand (progressive loading)
```

### Compaction

Compaction merges multiple segments into fewer, larger, sealed segments:

```
Before:  [VEC_SEG_1] [VEC_SEG_2] [VEC_SEG_3] [MANIFEST_3]
After:   [VEC_SEG_1] [VEC_SEG_2] [VEC_SEG_3] [MANIFEST_3] [VEC_SEG_sealed] [MANIFEST_4]
                                                              ^^^^^^^^^^^^^^^^^
                                                              New sealed segment
                                                              merging 1+2+3
```

Old segments are marked with TOMBSTONE entries in the new manifest. Space is
reclaimed when the file is eventually rewritten (or old segments are in a
separate file in multi-file mode).

### Multi-File Mode

For very large datasets, RVF can span multiple files:

```
data.rvf          Main file with manifests and hot data
data.rvf.cold.0   Cold segment shard 0
data.rvf.cold.1   Cold segment shard 1
data.rvf.idx.0    Index segment shard 0
```

The manifest in the main file contains shard references with file paths and
byte ranges. This enables cold data to live on slower storage while hot data
stays on fast storage.

## 6. Segment Addressing

Segments are addressed by their `segment_id` (monotonically increasing 64-bit
integer). The manifest maps segment IDs to file offsets (and optionally shard
file paths in multi-file mode).

Within a segment, data is addressed by **block offset** — a 32-bit offset from
the start of the segment payload. This limits individual segments to 4 GB, which
is intentional: it keeps segments manageable for compaction and progressive loading.

### Block Structure Within VEC_SEG

```
+-------------------+
| Block Header (16B)|
|   block_id: u32   |
|   count: u32      |
|   dim: u16        |
|   dtype: u8       |
|   pad: [u8; 5]    |
+-------------------+
| Vectors           |
| (count * dim *    |
|  sizeof(dtype))   |
| [64B aligned]     |
+-------------------+
| ID Map            |
| (varint delta     |
|  encoded IDs)     |
+-------------------+
| Block Footer      |
|   crc32c: u32     |
+-------------------+
```

Vectors within a block are stored **columnar** — all dimension 0 values, then all
dimension 1 values, etc. This maximizes compression ratio. But the HOT_SEG stores
vectors **interleaved** (row-major) for cache-friendly sequential scan during
top-K refinement.

## 7. Invariants

1. Segment IDs are strictly monotonically increasing within a file
2. A valid RVF file contains at least one MANIFEST_SEG
3. The last MANIFEST_SEG is always the source of truth
4. Segment headers are always 64-byte aligned
5. No segment payload exceeds 4 GB
6. Content hashes are computed over the raw (uncompressed, unencrypted) payload
7. Sealed segments are never modified — only tombstoned
8. A reader that cannot find a valid MANIFEST_SEG must reject the file
