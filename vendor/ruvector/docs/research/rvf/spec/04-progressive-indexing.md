# RVF Progressive Indexing

## 1. Index as Layers of Availability

Traditional HNSW serialization is all-or-nothing: either the full graph is loaded,
or nothing works. RVF decomposes the index into three layers of availability, each
independently useful, each stored in separate INDEX_SEG segments.

```
Layer C: Full Adjacency
+--------------------------------------------------+
| Complete neighbor lists for every node at every   |
| HNSW level. Built lazily. Optional for queries.   |
| Recall: >= 0.95                                   |
+--------------------------------------------------+
        ^  loaded last (seconds to minutes)
        |
Layer B: Partial Adjacency
+--------------------------------------------------+
| Neighbor lists for the most-accessed region       |
| (determined by temperature sketch). Covers the    |
| hot working set of the graph.                     |
| Recall: >= 0.85                                   |
+--------------------------------------------------+
        ^  loaded second (100ms - 1s)
        |
Layer A: Entry Points + Coarse Routing
+--------------------------------------------------+
| HNSW entry points. Top-layer adjacency lists.     |
| Cluster centroids for IVF pre-routing.            |
| Always present. Always in Level 0 hotset.         |
| Recall: >= 0.70                                   |
+--------------------------------------------------+
        ^  loaded first (< 5ms)
        |
      File open
```

### Why Three Layers

| Layer | Purpose | Data Size (10M vectors) | Load Time (NVMe) |
|-------|---------|------------------------|-------------------|
| A | First query possible | 1-4 MB | < 5 ms |
| B | Good quality for working set | 50-200 MB | 100-500 ms |
| C | Full recall for all queries | 1-4 GB | 2-10 s |

A system that only loads Layer A can still answer queries — just with lower recall.
As layers B and C load asynchronously, quality improves transparently.

## 2. Layer A: Entry Points and Coarse Routing

### Content

- **HNSW entry points**: The node(s) at the highest layer of the HNSW graph.
  Typically 1 node, but may be multiple for redundancy.
- **Top-layer adjacency**: Full neighbor lists for all nodes at HNSW layers
  >= ceil(ln(N) / ln(M)) - 2. For 10M vectors with M=16, this is layers 5-6,
  containing ~100-1000 nodes.
- **Cluster centroids**: K centroids (K = sqrt(N) typically, so ~3162 for 10M)
  used for IVF-style partition routing.
- **Centroid-to-partition map**: Which centroid owns which vector ID ranges.

### Storage

Layer A data is stored in a dedicated INDEX_SEG with `flags.HOT` set. The root
manifest's hotset pointers reference this segment directly. On cold start, this
is the first data mapped after the manifest.

### Binary Layout of Layer A INDEX_SEG

```
+-------------------------------------------+
| Header: INDEX_SEG, flags=HOT              |
+-------------------------------------------+
| Block 0: Entry Points                     |
|   entry_count: u32                        |
|   max_layer: u32                          |
|   [entry_node_id: u64, layer: u32] * N    |
+-------------------------------------------+
| Block 1: Top-Layer Adjacency              |
|   layer_count: u32                        |
|   For each layer (top to bottom):         |
|     node_count: u32                       |
|     For each node:                        |
|       node_id: u64                        |
|       neighbor_count: u16                 |
|       [neighbor_id: u64] * neighbor_count |
|     [64B padding]                         |
+-------------------------------------------+
| Block 2: Centroids                        |
|   centroid_count: u32                     |
|   dim: u16                                |
|   dtype: u8 (fp16)                        |
|   [centroid_vector: fp16 * dim] * K       |
|   [64B aligned]                           |
+-------------------------------------------+
| Block 3: Partition Map                    |
|   partition_count: u32                    |
|   For each partition:                     |
|     centroid_id: u32                      |
|     vector_id_start: u64                  |
|     vector_id_end: u64                    |
|     segment_ref: u64 (segment_id)         |
|     block_ref: u32 (block offset)         |
+-------------------------------------------+
```

### Query Using Only Layer A

```python
def query_layer_a_only(query, k, layer_a):
    # Step 1: Find nearest centroids
    dists = [distance(query, c) for c in layer_a.centroids]
    top_partitions = top_n(dists, n_probe)

    # Step 2: HNSW search through top layers only
    entry = layer_a.entry_points[0]
    current = entry
    for layer in range(layer_a.max_layer, layer_a.min_available_layer, -1):
        current = greedy_search(query, current, layer_a.adjacency[layer])

    # Step 3: If hot cache available, refine against it
    if hot_cache:
        candidates = scan_hot_cache(query, hot_cache, current.partition)
        return top_k(candidates, k)

    # Step 4: Otherwise, return centroid-approximate results
    return approximate_from_centroids(query, top_partitions, k)
```

Expected recall: 0.65-0.75 (depends on centroid quality and hot cache coverage).

## 3. Layer B: Partial Adjacency

### Content

Neighbor lists for the **hot region** of the graph — the set of nodes that appear
most frequently in query traversals. Determined by the temperature sketch (see
03-temperature-tiering.md).

Typically covers:
- All nodes at HNSW layers >= 2
- Layer 0-1 nodes in the hot temperature tier
- ~10-20% of total nodes

### Storage

Layer B is stored in one or more INDEX_SEGs without the HOT flag. The Level 1
manifest maps these segments and records which node ID ranges they cover.

### Incremental Build

Layer B can be built incrementally:

```
1. After Layer A is loaded, begin query serving
2. In background: read VEC_SEGs for hot-tier blocks
3. Build HNSW adjacency for those blocks
4. Write as new INDEX_SEG
5. Update manifest to include Layer B
6. Future queries use Layer B for better recall
```

This means the index improves over time without blocking any queries.

### Partial Adjacency Routing

When a query traversal reaches a node without Layer B adjacency (i.e., it's in
the cold region), the system falls back to:

1. **Centroid routing**: Use Layer A centroids to estimate the nearest region
2. **Linear scan**: Scan the relevant VEC_SEG block directly
3. **Approximate**: Accept slightly lower recall for that portion

```python
def search_with_partial_index(query, k, layers):
    # Start with Layer A routing
    current = hnsw_search_layers(query, layers.a, layers.a.max_layer, 2)

    # Continue with Layer B (where available)
    if layers.b.has_node(current):
        current = hnsw_search_layers(query, layers.b, 1, 0,
                                      start=current)
    else:
        # Fallback: scan the block containing current
        candidates = linear_scan_block(query, current.block)
        current = best_of(current, candidates)

    return top_k(current.visited, k)
```

## 4. Layer C: Full Adjacency

### Content

Complete neighbor lists for every node at every HNSW level. This is the
traditional full HNSW graph.

### Storage

Layer C may be split across multiple INDEX_SEGs for large datasets. The
manifest records the node ID ranges covered by each segment.

### Lazy Build

Layer C is built lazily — it is not required for the file to be functional.
The build process runs as a background task:

```
1. Identify unindexed VEC_SEG blocks (those without Layer C adjacency)
2. Read blocks in partition order (good locality)
3. Build HNSW adjacency using the existing partial graph as scaffold
4. Write new INDEX_SEG(s)
5. Update manifest
```

### Build Prioritization

Blocks are indexed in temperature order:
1. Hot blocks first (most query benefit)
2. Warm blocks next
3. Cold blocks last (may never be indexed if queries don't reach them)

This means the index build converges to useful quality fast, then approaches
completeness asymptotically.

## 5. Index Segment Binary Format

### Adjacency List Encoding

Neighbor lists are stored using **varint delta encoding with restart points**
for fast random access:

```
+-------------------------------------------+
| Restart Point Index                       |
|   restart_interval: u32 (e.g., 64)       |
|   restart_count: u32                      |
|   [restart_offset: u32] * restart_count   |
|   [64B aligned]                           |
+-------------------------------------------+
| Adjacency Data                            |
|   For each node (sorted by node_id):      |
|     neighbor_count: varint                |
|     [delta_encoded_neighbor_id: varint]   |
|     (restart point every N nodes)         |
+-------------------------------------------+
```

**Restart points**: Every `restart_interval` nodes (default 64), the delta
encoding resets to absolute IDs. This enables O(1) random access to any node's
neighbors by:

1. Binary search the restart point index for the nearest restart <= target
2. Seek to that restart offset
3. Sequentially decode from restart to target (at most 63 decodes)

### Varint Encoding

Standard LEB128 varint:
- Values 0-127: 1 byte
- Values 128-16383: 2 bytes
- Values 16384-2097151: 3 bytes

For delta-encoded neighbor IDs (typical delta: 1-1000), most values fit in 1-2
bytes, giving ~3-4x compression over fixed u64.

### Prefetch Hints

The manifest's prefetch table maps node ID ranges to contiguous page ranges:

```
Prefetch Entry:
  node_id_start: u64
  node_id_end: u64
  page_offset: u64      Offset of first contiguous page
  page_count: u32       Number of contiguous pages
  prefetch_ahead: u32   Pages to prefetch ahead of current access
```

When the HNSW search accesses a node, the runtime issues `madvise(WILLNEED)`
(or equivalent) for the next `prefetch_ahead` pages. This hides disk/memory
latency behind computation.

## 6. Index Consistency

### Append-Only Index Updates

When new vectors are added:

1. New vectors go into a **fresh VEC_SEG** (append-only)
2. A temporary in-memory index covers the new vectors
3. When the in-memory index reaches a threshold, it is written as a new INDEX_SEG
4. The manifest is updated to include both the old and new INDEX_SEGs
5. Queries search both indexes and merge results

This is analogous to LSM-tree compaction levels but for graph indexes.

### Index Merging

When too many small INDEX_SEGs accumulate:

```
1. Read all small INDEX_SEGs
2. Build a unified HNSW graph over all vectors
3. Write as a single sealed INDEX_SEG
4. Tombstone old INDEX_SEGs in manifest
```

### Concurrent Read/Write

Readers always see a consistent snapshot through the manifest chain:
- Reader opens file -> reads manifest -> has immutable segment set
- Writer appends new segments + new manifest
- Reader continues using old manifest until it explicitly re-reads
- No locks needed — append-only guarantees no mutation of existing data

## 7. Query Path Integration

The complete query path combining progressive indexing with temperature tiering:

```
                         Query
                           |
                           v
                    +-----------+
                    | Layer A   |   Entry points + top-layer routing
                    | (always)  |   ~5ms to load on cold start
                    +-----------+
                           |
                    Is Layer B available for this region?
                      /              \
                    Yes               No
                    /                   \
            +-----------+         +-----------+
            | Layer B   |         | Centroid  |
            | HNSW      |         | Fallback  |
            | search    |         | + scan    |
            +-----------+         +-----------+
                    \                  /
                     \                /
                      v              v
                    +-----------+
                    | Candidate |
                    | Set       |
                    +-----------+
                           |
                    Is hot cache available?
                      /              \
                    Yes               No
                    /                   \
            +-----------+         +-----------+
            | Hot cache |         | Decode    |
            | re-rank   |         | from      |
            | (int8/fp16)|        | VEC_SEG   |
            +-----------+         +-----------+
                    \                  /
                     v                v
                    +-----------+
                    | Top-K     |
                    | Results   |
                    +-----------+
```

### Recall Expectations by State

| State | Layers Available | Expected Recall@10 |
|-------|-----------------|-------------------|
| Cold start (L0 only) | A | 0.65-0.75 |
| L0 + hot cache | A + hot | 0.75-0.85 |
| L0 + L1 loading | A + B partial | 0.80-0.90 |
| L1 complete | A + B | 0.85-0.92 |
| Full load | A + B + C | 0.95-0.99 |
| Full + optimized | A + B + C + hot | 0.98-0.999 |
