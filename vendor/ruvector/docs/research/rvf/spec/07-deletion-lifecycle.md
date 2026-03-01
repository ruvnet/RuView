# RVF Deletion Lifecycle

## 1. Overview

Deletion in RVF follows a two-phase protocol consistent with the append-only
segment architecture. Vectors are never removed in-place. Instead, a soft
delete records intent in a JOURNAL_SEG, and a subsequent compaction hard
deletes by physically excluding the vectors from sealed output segments.

```
                  JOURNAL_SEG         Compaction           GC / Rewrite
                  (append)            (merge)              (reclaim)
    ACTIVE -----> SOFT_DELETED -----> HARD_DELETED ------> RECLAIMED
      |               |                    |                    |
      |  query path   |  query path        |                   |
      |  returns vec  |  skips vec         |  vec absent       |  space freed
      |               |  (bitmap check)    |  from output seg  |
```

Readers always see a consistent snapshot: a deletion is invisible until
the manifest referencing the new deletion bitmap is durably committed.

## 2. Vector Lifecycle State Machine

```
+----------+     JOURNAL_SEG        +-----------------+
|          |  DELETE_VECTOR / RANGE  |                 |
|  ACTIVE  +----------------------->+  SOFT_DELETED   |
|          |                        |                 |
+----------+                        +--------+--------+
                                             |  Compaction seals output
                                             v  excluding this vector
                                    +--------+--------+
                                    |  HARD_DELETED   |
                                    +--------+--------+
                                             |  File rewrite / truncation
                                             v  reclaims physical space
                                    +--------+--------+
                                    |   RECLAIMED     |
                                    +-----------------+
```

| State | Bitmap Bit | Physical Bytes | Query Visible |
|-------|------------|----------------|---------------|
| ACTIVE | 0 | Vector in VEC_SEG | Yes |
| SOFT_DELETED | 1 | Vector in VEC_SEG | No |
| HARD_DELETED | N/A | Excluded from sealed output | No |
| RECLAIMED | N/A | Bytes overwritten / freed | No |

| Transition | Trigger | Durability |
|------------|---------|------------|
| ACTIVE -> SOFT_DELETED | JOURNAL_SEG + MANIFEST_SEG with bitmap | After manifest fsync |
| SOFT_DELETED -> HARD_DELETED | Compaction writes sealed VEC_SEG without vector | After compaction manifest fsync |
| HARD_DELETED -> RECLAIMED | File rewrite or old shard deletion | After shard unlink |

## 3. JOURNAL_SEG Wire Format (type 0x04)

A JOURNAL_SEG records metadata mutations: deletions, metadata updates, tier
moves, and ID remappings. Its payload follows the standard 64-byte segment
header (see `01-segment-model.md` section 2).

### 3.1 Journal Header (64 bytes)

```
Offset  Type    Field                 Description
------  ----    -----                 -----------
0x00    u32     entry_count           Number of journal entries
0x04    u32     journal_epoch         Epoch when this journal was written
0x08    u64     prev_journal_seg_id   Segment ID of previous JOURNAL_SEG (0 if first)
0x10    u32     flags                 Reserved, must be 0
0x14    u8[44]  reserved              Zero-padded to 64-byte alignment
```

### 3.2 Journal Entry Format

Each entry begins on an 8-byte aligned boundary:

```
Offset  Type    Field          Description
------  ----    -----          -----------
0x00    u8      entry_type     Entry type enum
0x01    u8      reserved       Must be 0x00
0x02    u16     entry_length   Byte length of type-specific payload
0x04    u8[]    payload        Type-specific payload
var     u8[]    padding        Zero-pad to next 8-byte boundary
```

### 3.3 Entry Types

```
Value  Name              Payload Size  Description
-----  ----              ------------  -----------
0x01   DELETE_VECTOR      8 B          Delete a single vector by ID
0x02   DELETE_RANGE      16 B          Delete a contiguous range of vector IDs
0x03   UPDATE_METADATA   variable      Update key-value metadata for a vector
0x04   MOVE_VECTOR       24 B          Reassign vector to a different segment/tier
0x05   REMAP_ID          16 B          Reassign vector ID (post-compaction)
```

### 3.4 Type-Specific Payloads

**DELETE_VECTOR (0x01)**
```
0x00  u64  vector_id    ID of the vector to soft-delete
```

**DELETE_RANGE (0x02)**
```
0x00  u64  start_id     First vector ID (inclusive)
0x08  u64  end_id       Last vector ID (exclusive)
```
Invariant: `start_id < end_id`. Range `[start_id, end_id)` is half-open.

**UPDATE_METADATA (0x03)**
```
0x00  u64   vector_id   Target vector ID
0x08  u16   key_len     Byte length of metadata key
0x0A  u8[]  key         Metadata key (UTF-8)
var   u16   val_len     Byte length of metadata value
var+2 u8[]  val         Metadata value (opaque bytes)
```

**MOVE_VECTOR (0x04)**
```
0x00  u64  vector_id    Target vector ID
0x08  u64  src_seg      Source segment ID
0x10  u64  dst_seg      Destination segment ID
```

**REMAP_ID (0x05)**
```
0x00  u64  old_id       Original vector ID
0x08  u64  new_id       New vector ID after compaction
```

### 3.5 Complete JOURNAL_SEG Example

Deleting vector 42, deleting range [1000, 2000), remapping ID 500 -> 3:

```
Byte offset   Content                    Notes
-----------   -------                    -----
0x00-0x3F     Segment header (64 B)      seg_type=0x04, magic=RVFS
0x40-0x7F     Journal header (64 B)      entry_count=3, epoch=7,
                                         prev_journal_seg_id=12
--- Entry 0: DELETE_VECTOR ---
0x80          0x01                       entry_type
0x81          0x00                       reserved
0x82-0x83     0x0008                     entry_length = 8
0x84-0x8B     0x000000000000002A         vector_id = 42
0x8C-0x8F     0x00000000                 padding to 8B

--- Entry 1: DELETE_RANGE ---
0x90          0x02                       entry_type
0x91          0x00                       reserved
0x92-0x93     0x0010                     entry_length = 16
0x94-0x9B     0x00000000000003E8         start_id = 1000
0x9C-0xA3     0x00000000000007D0         end_id = 2000

--- Entry 2: REMAP_ID ---
0xA4          0x05                       entry_type
0xA5          0x00                       reserved
0xA6-0xA7     0x0010                     entry_length = 16
0xA8-0xAF     0x00000000000001F4         old_id = 500
0xB0-0xB7     0x0000000000000003         new_id = 3
```

## 4. Deletion Bitmap

### 4.1 Manifest Record

The deletion bitmap is stored in the Level 1 manifest as a TLV record:

```
Tag     Name                Description
---     ----                -----------
0x000E  DELETION_BITMAP     Roaring bitmap of soft-deleted vector IDs
```

This extends the TLV tag space (previous: 0x000D KEY_DIRECTORY).

### 4.2 Roaring Bitmap Binary Layout

Vector IDs are 64-bit. The upper 32 bits select a **high key**; the lower
32 bits index into a **container** for that high key.

```
+---------------------------------------------+
| DELETION_BITMAP TLV Value                   |
+---------------------------------------------+
| Bitmap Header                               |
|   cookie: u32       (0x3B3A3332)            |
|   high_key_count: u32                       |
|   For each high key:                        |
|     high_key: u32                           |
|     container_type: u8                      |
|       0x01 = ARRAY_CONTAINER               |
|       0x02 = BITMAP_CONTAINER              |
|       0x03 = RUN_CONTAINER                 |
|     container_offset: u32 (from bitmap start)|
|   [8B aligned]                              |
+---------------------------------------------+
| Container Data                              |
|   Container 0: [type-specific layout]       |
|   Container 1: ...                          |
|   [8B aligned per container]                |
+---------------------------------------------+
```

### 4.3 Container Types

**ARRAY_CONTAINER (0x01)** -- Sparse deletions (< 4096 set bits per 64K range).
```
0x00  u16    cardinality   Number of set values (1-4096)
0x02  u16[]  values        Sorted array of 16-bit values
```
Size: `2 + 2 * cardinality` bytes.

**BITMAP_CONTAINER (0x02)** -- Dense deletions (>= 4096 set bits per 64K range).
```
0x00  u16      cardinality   Number of set bits
0x02  u8[8192] bitmap        Fixed 65536-bit bitmap (8 KB)
```
Size: 8194 bytes (fixed).

**RUN_CONTAINER (0x03)** -- Contiguous ranges of deletions.
```
0x00  u16        run_count   Number of runs
0x02  (u16,u16)  runs[]      Array of (start, length-1) pairs
```
Size: `2 + 4 * run_count` bytes.

### 4.4 Size Estimation

| Deletion Pattern | Deleted IDs | Container Types | Bitmap Size |
|------------------|-------------|-----------------|-------------|
| Sparse random | 10,000 (0.1%) | ~153 array | ~22 KB |
| Clustered ranges | 10,000 (0.1%) | ~5 run | ~0.1 KB |
| Mixed workload | 100,000 (1%) | array + run | ~80 KB |
| Heavy deletion | 1,000,000 (10%) | bitmap + run | ~200 KB |

Even at 200 KB the bitmap fits entirely in L2 cache.

### 4.5 Bitmap Operations

```python
def bitmap_check(bitmap, vector_id):
    """Returns True if vector_id is soft-deleted. O(1) amortized."""
    high_key = vector_id >> 16
    low_val  = vector_id & 0xFFFF
    container = bitmap.get_container(high_key)
    if container is None:
        return False
    return container.contains(low_val)  # array: bsearch, bitmap: bit test, run: bsearch

def bitmap_set(bitmap, vector_id):
    """Mark a vector as soft-deleted."""
    high_key = vector_id >> 16
    low_val  = vector_id & 0xFFFF
    container = bitmap.get_or_create_container(high_key)
    container.add(low_val)
    if container.type == ARRAY and container.cardinality > 4096:
        container.promote_to_bitmap()
```

## 5. Delete-Aware Query Path

### 5.1 HNSW Traversal with Deletion Filtering

Deleted vectors remain in the HNSW graph until compaction rebuilds the index.
During search, the deletion bitmap is checked per candidate. Deleted nodes are
still traversed for connectivity but excluded from the result set.

```python
def hnsw_search_delete_aware(query, entry_point, ef_search, k, del_bitmap):
    candidates = MaxHeap()   # worst candidate on top
    visited    = BitSet()
    worklist   = MinHeap()   # best candidate first

    d0 = distance(query, get_vector(entry_point))
    worklist.push((d0, entry_point))
    visited.add(entry_point)
    if not bitmap_check(del_bitmap, entry_point):
        candidates.push((d0, entry_point))

    while worklist:
        dist, node = worklist.pop()
        if candidates.size() >= ef_search and dist > candidates.peek_max():
            break

        neighbors = get_neighbors(node)
        for n in neighbors[:PREFETCH_AHEAD]:
            if n not in visited:
                prefetch_vector(n)

        for n in neighbors:
            if n in visited:
                continue
            visited.add(n)
            d = distance(query, get_vector(n))
            is_deleted = bitmap_check(del_bitmap, n)   # O(1) bitmap lookup

            # Always add to worklist (graph connectivity)
            if candidates.size() < ef_search or d < candidates.peek_max():
                worklist.push((d, n))
            # Only add to results if NOT deleted
            if not is_deleted:
                if candidates.size() < ef_search:
                    candidates.push((d, n))
                elif d < candidates.peek_max():
                    candidates.replace_max((d, n))

    return candidates.top_k(k)
```

### 5.2 Top-K Refinement with Deletion Filtering

```python
def topk_refine_delete_aware(candidates, hot_cache, query, k, del_bitmap):
    heap = MaxHeap()
    for cand_dist, cand_id in candidates:
        heap.push((cand_dist, cand_id))

    for entry in hot_cache.sequential_scan():
        if bitmap_check(del_bitmap, entry.vector_id):
            continue   # skip soft-deleted
        d = distance(query, entry.vector)
        if heap.size() < k:
            heap.push((d, entry.vector_id))
        elif d < heap.peek_max():
            heap.replace_max((d, entry.vector_id))

    return heap.drain_sorted()
```

### 5.3 Performance Impact

| Operation | Without Deletions | With Deletions | Overhead |
|-----------|-------------------|----------------|----------|
| Bitmap check | N/A | ~2-5 ns (L1/L2 hit) | Per candidate |
| HNSW step (M=16) | ~300-500 ns | ~330-580 ns | +10% |
| Top-K refine (1000) | ~10 us | ~12 us | +20% worst |
| Total query | ~50-75 us | ~55-85 us | +10-13% |

At typical deletion rates (< 5%), overhead is negligible: the bitmap fits in
L2 cache, graph connectivity is preserved, and the cost is one branch plus
one bitmap load per candidate.

## 6. Deletion Write Path

All deletion operations follow the same two-fsync protocol:

```python
def delete_vectors(file, entries):
    """Soft-delete vectors. entries: list of DeleteVector or DeleteRange."""
    # 1. Append JOURNAL_SEG
    journal = JournalSegment(
        epoch=current_epoch(file),
        prev_journal_seg_id=latest_journal_id(file),
        entries=entries
    )
    append_segment(file, journal)
    fsync(file)   # orphan-safe: no manifest references this yet

    # 2. Update deletion bitmap in memory
    bitmap = load_deletion_bitmap(file)
    for e in entries:
        if e.type == DELETE_VECTOR:
            bitmap_set(bitmap, e.vector_id)
        elif e.type == DELETE_RANGE:
            bitmap.add_range(e.start_id, e.end_id)

    # 3. Append MANIFEST_SEG with updated bitmap
    manifest = build_manifest(file, deletion_bitmap=bitmap)
    append_segment(file, manifest)
    fsync(file)   # deletion now visible to all new readers
```

Single deletes, bulk ranges, and batch deletes all use this path. Batch
operations pack multiple entries into one JOURNAL_SEG to amortize fsync cost.

## 7. Compaction with Deletions

### 7.1 Compaction Process

```
Before:
[VEC_1] [VEC_2] [JOURNAL_1] [VEC_3] [JOURNAL_2] [MANIFEST_5]
 0-999   1000-   del:42,     3000-   del:[1000,   bitmap={42,500,
         2999    del:500     4999    2000)         1000..1999}

After:
... [MANIFEST_5] [VEC_sealed] [INDEX_new] [MANIFEST_6]
                  vectors 0-4999           bitmap={}
                  MINUS deleted            (empty for
                                           compacted range)
```

### 7.2 Compaction Algorithm

```python
def compact_with_deletions(file, seg_ids):
    bitmap = load_deletion_bitmap(file)
    output, id_remap, next_id = [], {}, 0

    for seg_id in sorted(seg_ids):
        seg = load_segment(file, seg_id)
        if seg.seg_type != VEC_SEG:
            continue
        for vec_id, vector in seg.all_vectors():
            if bitmap_check(bitmap, vec_id):
                continue                        # physically exclude
            id_remap[vec_id] = next_id
            output.append((next_id, vector))
            next_id += 1

    append_segment(file, VecSegment(flags=SEALED, vectors=output))

    remaps = [RemapIdEntry(old, new) for old, new in id_remap.items() if old != new]
    if remaps:
        append_segment(file, JournalSegment(entries=remaps))

    append_segment(file, build_hnsw_index(output))

    for old_id in id_remap:
        bitmap.remove(old_id)

    manifest = build_manifest(file,
        tombstone_seg_ids=seg_ids,
        deletion_bitmap=bitmap)
    append_segment(file, manifest)
    fsync(file)
```

### 7.3 Journal Merging

During compaction, JOURNAL_SEGs covering the compacted range are consumed:

| Entry Type | Materialization |
|------------|-----------------|
| DELETE_VECTOR / DELETE_RANGE | Vectors excluded from output |
| UPDATE_METADATA | Applied to output META_SEG |
| MOVE_VECTOR | Tier assignment applied in new manifest |
| REMAP_ID | Chained: old remap composed with new remap |

Consumed JOURNAL_SEGs are tombstoned alongside compacted VEC_SEGs.

### 7.4 Compaction Invariants

| ID | Invariant |
|----|-----------|
| INV-D1 | After compaction, deletion bitmap is empty for compacted range |
| INV-D2 | Sealed output contains only ACTIVE vectors |
| INV-D3 | REMAP_ID entries journaled for every relocated vector |
| INV-D4 | Compacted input segments tombstoned in new manifest |
| INV-D5 | Sealed segments are never modified |
| INV-D6 | Rebuilt indexes exclude deleted nodes |

## 8. Deletion Consistency

### 8.1 Crash Safety

```
Write path:
  1. Append JOURNAL_SEG -> fsync         crash here: orphan, invisible
  2. Append MANIFEST_SEG -> fsync        crash here: partial manifest, fallback

Recovery:
- Crash after step 1: JOURNAL_SEG orphaned. No manifest references it.
  Reader sees previous manifest. Deletion NOT visible. Orphan cleaned
  up by next compaction.
- Crash during step 2: Partial MANIFEST_SEG has bad checksum. Reader
  falls back to previous valid manifest. Deletion NOT visible.
- After step 2 success: Manifest durable. Deletion visible.
```

**Guarantee**: Uncommitted deletions never affect readers. Deletion is
atomic at the manifest fsync boundary.

### 8.2 Manifest Chain Visibility

```
MANIFEST_3: bitmap = {}
  |  JOURNAL_SEG written (delete vector 42)
MANIFEST_4: bitmap = {42}     <-- deletion visible from here
  |  Compaction runs
MANIFEST_5: bitmap = {}       <-- vector 42 physically removed
```

A reader holding MANIFEST_3 continues to see vector 42. A reader opening
after MANIFEST_4 will not. This provides snapshot isolation at manifest
granularity.

### 8.3 Multi-File Mode

In multi-file mode, each shard maintains its own deletion bitmap. The
DELETION_BITMAP TLV record supports two modes:

```
+----------------------------------------------+
| mode: u8                                     |
|   0x00 = SINGLE   (one bitmap, inline)       |
|   0x01 = SHARDED  (per-shard references)     |
+----------------------------------------------+
SINGLE (0x00):
|   roaring_bitmap: [u8; ...]                  |

SHARDED (0x01):
|   shard_count: u16                           |
|   For each shard:                            |
|     shard_id: u16                            |
|     bitmap_offset: u64  (in shard file)      |
|     bitmap_length: u32                       |
|     bitmap_hash: hash128                     |
+----------------------------------------------+
```

Queries spanning shards load per-shard bitmaps and check each candidate
against its shard's bitmap.

### 8.4 Concurrent Access

One writer at a time (file-level advisory lock). Multiple readers are safe
due to append-only architecture. A reader that opened before a deletion
sees the pre-deletion snapshot until it re-reads the manifest.

## 9. Space Reclamation

| Trigger | Threshold | Action |
|---------|-----------|--------|
| Deletion ratio | > 20% of vectors deleted | Schedule compaction |
| Bitmap size | > 1 MB | Schedule compaction |
| Segment count | > 64 mutable segments | Schedule compaction |
| Manual | User-initiated | Compact immediately |

Space accounting derived from the manifest:
```
total_vector_count:     10,000,000   (Level 0 root manifest)
deleted_vector_count:      150,000   (bitmap cardinality)
active_vector_count:     9,850,000   (total - deleted)
deletion_ratio:              1.5%    (below threshold)
wasted_bytes:           ~115 MB      (150K * 768 B per fp16-384 vector)
```

## 10. Summary

### Deletion Protocol

| Step | Action | Durability |
|------|--------|------------|
| 1 | Append JOURNAL_SEG with DELETE entries | fsync (orphan-safe) |
| 2 | Update roaring deletion bitmap | In-memory |
| 3 | Append MANIFEST_SEG with new bitmap | fsync (deletion visible) |
| 4 | Compaction excludes deleted vectors | fsync (physical removal) |
| 5 | File rewrite reclaims space | fsync (space freed) |

### New Wire Format Elements

| Element | Type / Tag | Section |
|---------|------------|---------|
| JOURNAL_SEG | Segment type 0x04 | 3 |
| DELETE_VECTOR | Journal entry 0x01 | 3.4 |
| DELETE_RANGE | Journal entry 0x02 | 3.4 |
| UPDATE_METADATA | Journal entry 0x03 | 3.4 |
| MOVE_VECTOR | Journal entry 0x04 | 3.4 |
| REMAP_ID | Journal entry 0x05 | 3.4 |
| DELETION_BITMAP | Level 1 TLV 0x000E | 4 |

### Invariants

| ID | Invariant |
|----|-----------|
| INV-D1 | After compaction, deletion bitmap is empty for compacted range |
| INV-D2 | Sealed output segments contain only ACTIVE vectors |
| INV-D3 | ID remappings journaled for every compaction-relocated vector |
| INV-D4 | Compacted input segments tombstoned in new manifest |
| INV-D5 | Sealed segments are never modified |
| INV-D6 | Rebuilt indexes exclude deleted nodes |
| INV-D7 | Uncommitted deletions never affect readers (crash safety) |
| INV-D8 | Deletion visibility is atomic at the manifest fsync boundary |
