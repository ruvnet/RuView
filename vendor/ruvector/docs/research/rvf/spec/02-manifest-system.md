# RVF Manifest System

## 1. Two-Level Manifest Architecture

The manifest system is what makes RVF progressive. Instead of a monolithic directory
that must be fully parsed before any query, RVF uses a two-level manifest that
enables instant boot followed by incremental refinement.

```
                          EOF
                           |
                           v
+--------------------------------------------------+
| Level 0: Root Manifest (fixed 4096 bytes)        |
|   - Magic + version                              |
|   - Pointer to Level 1 manifest segment          |
|   - Hotset pointers (inline)                     |
|   - Total vector count                           |
|   - Dimension                                    |
|   - Epoch counter                                |
|   - Profile declaration                          |
+--------------------------------------------------+
          |
          | points to
          v
+--------------------------------------------------+
| Level 1: Full Manifest (variable size)           |
|   - Complete segment directory                   |
|   - Temperature tier map                         |
|   - Index layer availability                     |
|   - Overlay epoch chain                          |
|   - Compaction state                             |
|   - Shard references (multi-file)                |
|   - Capability manifest                          |
+--------------------------------------------------+
```

### Why Two Levels

A reader performing cold start only needs Level 0 (4 KB). From Level 0 alone,
it can locate the entry points, coarse routing graph, quantization dictionary,
and centroids — enough to answer approximate queries immediately.

Level 1 is loaded asynchronously to enable full-quality queries, but the system
is functional before Level 1 is fully parsed.

## 2. Level 0: Root Manifest

The root manifest is always the **last 4096 bytes** of the file (or the last
4096 bytes of the most recent MANIFEST_SEG). Its fixed size enables instant
location: `seek(EOF - 4096)`.

### Binary Layout

```
Offset  Size  Field                     Description
------  ----  -----                     -----------
0x000   4     magic                     0x52564D30 ("RVM0")
0x004   2     version                   Root manifest version
0x006   2     flags                     Root manifest flags
0x008   8     l1_manifest_offset        Byte offset to Level 1 manifest segment
0x010   8     l1_manifest_length        Byte length of Level 1 manifest segment
0x018   8     total_vector_count        Total vectors across all segments
0x020   2     dimension                 Vector dimensionality
0x022   1     base_dtype                Base data type enum
0x023   1     profile_id                Domain profile (0=generic, 1=dna, 2=text, 3=graph, 4=vision)
0x024   4     epoch                     Current overlay epoch number
0x028   8     created_ns                File creation timestamp (ns)
0x030   8     modified_ns               Last modification timestamp (ns)

--- Hotset Pointers (the key to instant boot) ---

0x038   8     entrypoint_seg_offset     Offset to segment containing HNSW entry points
0x040   4     entrypoint_block_offset   Block offset within that segment
0x044   4     entrypoint_count          Number of entry points

0x048   8     toplayer_seg_offset       Offset to segment with top-layer adjacency
0x050   4     toplayer_block_offset     Block offset
0x054   4     toplayer_node_count       Nodes in top layer

0x058   8     centroid_seg_offset       Offset to segment with cluster centroids / pivots
0x060   4     centroid_block_offset     Block offset
0x064   4     centroid_count            Number of centroids

0x068   8     quantdict_seg_offset      Offset to quantization dictionary segment
0x070   4     quantdict_block_offset    Block offset
0x074   4     quantdict_size            Dictionary size in bytes

0x078   8     hot_cache_seg_offset      Offset to HOT_SEG with interleaved hot vectors
0x080   4     hot_cache_block_offset    Block offset
0x084   4     hot_cache_vector_count    Vectors in hot cache

0x088   8     prefetch_map_offset       Offset to prefetch hint table
0x090   4     prefetch_map_entries      Number of prefetch entries

--- Crypto ---

0x094   2     sig_algo                  Manifest signature algorithm
0x096   2     sig_length                Signature length
0x098   var   signature                 Manifest signature (up to 3400 bytes for ML-DSA-65)

--- Padding to 4096 bytes ---

0xF00   252   reserved                  Reserved / zero-padded to 4096
0xFFC   4     root_checksum             CRC32C of bytes 0x000-0xFFB
```

**Total**: Exactly 4096 bytes (one page, one disk sector on most hardware).

### Hotset Pointers

The six hotset pointers are the minimum information needed to answer a query:

1. **Entry points**: Where to start HNSW traversal
2. **Top-layer adjacency**: Coarse routing to the right neighborhood
3. **Centroids/pivots**: For IVF-style pre-filtering or partition routing
4. **Quantization dictionary**: For decoding compressed vectors
5. **Hot cache**: Pre-decoded interleaved vectors for top-K refinement
6. **Prefetch map**: Contiguous neighbor-list pages with prefetch offsets

With these six pointers, a reader can:
- Start HNSW search at the entry point
- Route through the top layer
- Quantize the query using the dictionary
- Scan the hot cache for refinement
- Prefetch neighbor pages for cache-friendly traversal

All without reading Level 1 or any cold segments.

## 3. Level 1: Full Manifest

Level 1 is a variable-size segment (type `MANIFEST_SEG`) referenced by Level 0.
It contains the complete file directory.

### Structure

Level 1 is encoded as a sequence of typed records using a tag-length-value (TLV)
scheme for forward compatibility:

```
+---+---+---+---+---+---+---+---+
| Tag (2B) | Length (4B) | Pad   |  <- 8-byte aligned record header
+---+---+---+---+---+---+---+---+
| Value (Length bytes)            |
| [padded to 8-byte boundary]    |
+---------------------------------+
```

### Record Types

```
Tag     Name                    Description
---     ----                    -----------
0x0001  SEGMENT_DIR             Array of segment directory entries
0x0002  TEMP_TIER_MAP           Temperature tier assignments per block
0x0003  INDEX_LAYERS            Index layer availability bitmap
0x0004  OVERLAY_CHAIN           Epoch chain with rollback pointers
0x0005  COMPACTION_STATE        Active/tombstoned segment sets
0x0006  SHARD_REFS              Multi-file shard references
0x0007  CAPABILITY_MANIFEST     What this file can do (features, limits)
0x0008  PROFILE_CONFIG          Domain-specific configuration
0x0009  ACCESS_SKETCH_REF       Pointer to latest SKETCH_SEG
0x000A  PREFETCH_TABLE          Full prefetch hint table
0x000B  ID_RESTART_POINTS       Restart point index for varint delta IDs
0x000C  WITNESS_CHAIN           Proof-of-computation witness chain
0x000D  KEY_DIRECTORY           Encryption key references (not keys themselves)
```

### Segment Directory Entry

```
Offset  Size  Field                Description
------  ----  -----                -----------
0x00    8     segment_id           Segment ordinal
0x08    1     seg_type             Segment type enum
0x09    1     tier                 Temperature tier (0=hot, 1=warm, 2=cold)
0x0A    2     flags                Segment flags
0x0C    4     reserved             Must be zero
0x10    8     file_offset          Byte offset in file (or shard)
0x18    8     payload_length       Decompressed payload length
0x20    8     compressed_length    Compressed length (0 if uncompressed)
0x28    2     shard_id             Shard index (0 for main file)
0x2A    2     compression          Compression algorithm
0x2C    4     block_count          Number of blocks in segment
0x30    16    content_hash         Payload hash (first 128 bits)
```

**Total**: 64 bytes per entry (cache-line aligned).

## 4. Manifest Lifecycle

### Writing a New Manifest

Every mutation to the file produces a new MANIFEST_SEG appended at the tail:

```
1. Compute new Level 1 manifest (segment directory + metadata)
2. Write Level 1 as a MANIFEST_SEG payload
3. Compute Level 0 root manifest pointing to Level 1
4. Write Level 0 as the last 4096 bytes of the MANIFEST_SEG
5. fsync
```

The MANIFEST_SEG payload structure is:

```
+-----------------------------------+
| Level 1 manifest (variable size)  |
+-----------------------------------+
| Level 0 root manifest (4096 B)   |  <-- Always the last 4096 bytes
+-----------------------------------+
```

### Reading the Manifest

```
1. seek(EOF - 4096)
2. Read 4096 bytes -> Level 0 root manifest
3. Validate magic (0x52564D30) and checksum
4. If valid: extract hotset pointers -> system is queryable
5. Async: read Level 1 at l1_manifest_offset -> full directory
6. If Level 0 is invalid: scan backward for previous MANIFEST_SEG
```

Step 6 provides crash recovery. If the latest manifest write was interrupted,
the previous manifest is still valid. Readers scan backward at 64-byte aligned
boundaries looking for the RVFS magic + MANIFEST_SEG type.

### Manifest Chain

Each manifest implicitly forms a chain through the segment ID ordering. For
explicit rollback support, Level 1 contains the `OVERLAY_CHAIN` record which
stores:

```
epoch: u32              Current epoch
prev_manifest_offset: u64   Offset of previous MANIFEST_SEG
prev_manifest_id: u64       Segment ID of previous MANIFEST_SEG
checkpoint_hash: [u8; 16]   Hash of the complete state at this epoch
```

This enables point-in-time recovery and bisection debugging.

## 5. Hotset Pointer Semantics

### Entry Point Stability

Entry points are the HNSW nodes at the highest layer. They change rarely (only
when the index is rebuilt or a new highest-layer node is inserted). The root
manifest caches them directly so they survive across manifest generations without
re-reading the index.

### Centroid Refresh

Centroids may drift as data is added. The manifest tracks a `centroid_epoch` — if
the current epoch exceeds centroid_epoch + threshold, the runtime should schedule
centroid recomputation. But the stale centroids remain usable (recall degrades
gracefully, it does not fail).

### Hot Cache Coherence

The hot cache in HOT_SEG is a **read-optimized snapshot** of the most-accessed
vectors. It may be stale relative to the latest VEC_SEGs. The manifest tracks
a `hot_cache_epoch` for staleness detection. Queries use the hot cache for fast
initial results, then refine against authoritative VEC_SEGs if needed.

## 6. Progressive Boot Sequence

```
Time     Action                          System State
----     ------                          ------------
t=0      Read last 4 KB (Level 0)        Booting
t+1ms    Parse hotset pointers            Queryable (approximate)
t+2ms    mmap entry points + top layer    Better routing
t+5ms    mmap hot cache + quant dict      Fast top-K refinement
t+10ms   Start loading Level 1            Discovering full directory
t+50ms   Level 1 parsed                   Full segment awareness
t+100ms  mmap warm VEC_SEGs              Recall improving
t+500ms  mmap cold VEC_SEGs              Full recall
t+1s     Background index layer build     Converging to optimal
```

For a 10M vector file (~4 GB at 384 dimensions, float16):
- Level 0 read: 4 KB in <1 ms
- Hotset data: ~2-4 MB (entry points + top layer + centroids + hot cache)
- First query: within 5-10 ms of open
- Full convergence: 1-5 seconds depending on storage speed
