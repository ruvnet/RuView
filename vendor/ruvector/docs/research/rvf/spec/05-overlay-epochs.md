# RVF Overlay Epochs

## 1. Streaming Dynamic Min-Cut Overlay

The overlay system manages dynamic graph partitioning — how the vector space is
subdivided for distributed search, shard routing, and load balancing. Unlike
static partitioning, RVF overlays evolve with the data through an epoch-based
model that bounds memory, bounds load time, and enables rollback.

## 2. Overlay Segment Structure

Each OVERLAY_SEG stores a delta relative to the previous epoch's partition state:

```
+-------------------------------------------+
| Header: OVERLAY_SEG                       |
+-------------------------------------------+
| Epoch Header                              |
|   epoch: u32                              |
|   parent_epoch: u32                       |
|   parent_seg_id: u64                      |
|   rollback_offset: u64                    |
|   timestamp_ns: u64                       |
|   delta_count: u32                        |
|   partition_count: u32                    |
+-------------------------------------------+
| Edge Deltas                               |
|   For each delta:                         |
|     delta_type: u8 (ADD=1, REMOVE=2,      |
|                     REWEIGHT=3)           |
|     src_node: u64                         |
|     dst_node: u64                         |
|     weight: f32 (for ADD/REWEIGHT)        |
|   [64B aligned]                           |
+-------------------------------------------+
| Partition Summaries                       |
|   For each partition:                     |
|     partition_id: u32                     |
|     node_count: u64                       |
|     edge_cut_weight: f64                  |
|     centroid: [fp16 * dim]                |
|     node_id_range_start: u64              |
|     node_id_range_end: u64               |
|   [64B aligned]                           |
+-------------------------------------------+
| Min-Cut Witness                           |
|   witness_type: u8                        |
|     0 = checksum only                     |
|     1 = full certificate                  |
|   cut_value: f64                          |
|   cut_edge_count: u32                     |
|   partition_hash: [u8; 32] (SHAKE-256)    |
|   If witness_type == 1:                   |
|     [cut_edge: (u64, u64)] * count        |
|   [64B aligned]                           |
+-------------------------------------------+
| Rollback Pointer                          |
|   prev_epoch_offset: u64                  |
|   prev_epoch_hash: [u8; 16]              |
+-------------------------------------------+
```

## 3. Epoch Lifecycle

### Epoch Creation

A new epoch is created when:
- A batch of vectors is inserted that changes partition balance by > threshold
- The accumulated edge deltas exceed a size limit (default: 1 MB)
- A manual rebalance is triggered
- A merge/compaction produces a new partition layout

```
Epoch 0 (initial)     Epoch 1             Epoch 2
+----------------+    +----------------+   +----------------+
| Full snapshot  |    | Deltas vs E0   |   | Deltas vs E1   |
| of partitions  |    | +50 edges      |   | +30 edges      |
| 32 partitions  |    | -12 edges      |   | -8 edges       |
| min-cut: 0.342 |    | rebalance: P3  |   | split: P7->P7a |
+----------------+    +----------------+   +----------------+
```

### State Reconstruction

To reconstruct the current partition state:

```
1. Read latest MANIFEST_SEG -> get current_epoch
2. Read OVERLAY_SEG for current_epoch
3. If overlay is a delta: recursively read parent epochs
4. Apply deltas in order: base -> epoch 1 -> epoch 2 -> ... -> current
5. Result: complete partition state
```

For efficiency, the manifest caches the **last full snapshot epoch**. Delta
chains never exceed a configurable depth (default: 8 epochs) before a new
snapshot is forced.

### Compaction (Epoch Collapse)

When the delta chain reaches maximum depth:

```
1. Reconstruct full state from chain
2. Write new OVERLAY_SEG with witness_type=full_snapshot
3. This becomes the new base epoch
4. Old overlay segments are tombstoned
5. New delta chain starts from this base
```

```
Before:  E0(snap) -> E1(delta) -> E2(delta) -> ... -> E8(delta)
After:   E0(snap) -> ... -> E8(delta) -> E9(snap, compacted)
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
         These can be garbage collected
```

## 4. Min-Cut Witness

The min-cut witness provides a cryptographic proof that the current partition
is "good enough" — that the edge cut is within acceptable bounds.

### Witness Types

**Type 0: Checksum Only**

A SHAKE-256 hash of the complete partition state. Allows verification that
the state is consistent but doesn't prove optimality.

```
witness = SHAKE-256(
    for each partition sorted by id:
        partition_id || node_count || sorted(node_ids) || edge_cut_weight
)
```

**Type 1: Full Certificate**

Lists the actual cut edges. Allows any reader to verify that:
1. The listed edges are the only edges crossing partition boundaries
2. The total cut weight matches `cut_value`
3. No better cut exists within the local search neighborhood (optional)

### Bounded-Time Min-Cut Updates

Full min-cut computation is expensive (O(V * E) for max-flow). RVF uses
**incremental min-cut maintenance**:

For each edge delta:
```
1. If ADD(u, v) where u and v are in same partition:
   -> No cut change. O(1).

2. If ADD(u, v) where u in P_i and v in P_j:
   -> cut_weight[P_i][P_j] += weight. O(1).
   -> Check if moving u to P_j or v to P_i reduces total cut.
   -> If yes: execute move, update partition summaries. O(degree).

3. If REMOVE(u, v) across partitions:
   -> cut_weight[P_i][P_j] -= weight. O(1).
   -> No rebalance needed (cut improved).

4. If REMOVE(u, v) within same partition:
   -> Check connectivity. If partition splits: create new partition. O(component).
```

This bounds update time to O(max_degree) per edge delta in the common case,
with O(component_size) in the rare partition-split case.

### Semi-Streaming Min-Cut

For large-scale rebalancing (e.g., after bulk insert), RVF uses a semi-streaming
algorithm inspired by Assadi et al.:

```
Phase 1: Single pass over edges to build a sparse skeleton
  - Sample each edge with probability O(1/epsilon)
  - Space: O(n * polylog(n))

Phase 2: Compute min-cut on skeleton
  - Standard max-flow on sparse graph
  - Time: O(n^2 * polylog(n))

Phase 3: Verify against full edge set
  - Stream edges again, check cut validity
  - If invalid: refine skeleton and repeat
```

This runs in O(n * polylog(n)) space regardless of edge count, making it
suitable for streaming over massive graphs.

## 5. Overlay Size Management

### Size Threshold

Each OVERLAY_SEG has a maximum payload size (configurable, default 1 MB).
When the accumulated deltas for the current epoch approach this threshold,
a new epoch is forced.

### Memory Budget

The total memory for overlay state is bounded:

```
max_overlay_memory = max_chain_depth * max_seg_size + snapshot_size
                   = 8 * 1 MB + snapshot_size
```

For 10M vectors with 32 partitions:
- Snapshot: ~32 * (8 + 16 + 768) bytes per partition ≈ 25 KB
- Delta chain: ≤ 8 MB
- Total: ≤ 9 MB

This is a fixed overhead regardless of dataset size (partition count scales
sublinearly).

### Garbage Collection

Overlay segments behind the last full snapshot are candidates for garbage
collection. The manifest tracks which overlay segments are still reachable
from the current epoch chain.

```
Reachable:    current_epoch -> parent -> ... -> last_snapshot
Unreachable:  Everything before last_snapshot (safely deletable)
```

GC runs during compaction. Old OVERLAY_SEGs are tombstoned in the manifest
and their space is reclaimed on file rewrite.

## 6. Distributed Overlay Coordination

When RVF files are sharded across multiple nodes, the overlay system coordinates
partition state:

### Shard-Local Overlays

Each shard maintains its own OVERLAY_SEG chain for its local partitions.
The global partition state is the union of all shard-local overlays.

### Cross-Shard Rebalancing

When a partition becomes unbalanced across shards:

```
1. Coordinator computes target partition assignment
2. Each shard writes a JOURNAL_SEG with vector move instructions
3. Vectors are copied (not moved — append-only) to target shards
4. Each shard writes a new OVERLAY_SEG reflecting the new partition
5. Coordinator writes a global MANIFEST_SEG with new shard map
```

This is eventually consistent — during rebalancing, queries may search both
old and new locations and deduplicate results.

### Consistency Model

**Within a shard**: Linearizable (single-writer, manifest chain)
**Across shards**: Eventually consistent with bounded staleness

The epoch counter provides a total order for convergence checking:
- If all shards report epoch >= E, the global state at epoch E is complete
- Stale shards are detectable by comparing epoch counters

## 7. Epoch-Aware Query Routing

Queries use the overlay state for partition routing:

```python
def route_query(query, overlay):
    # Find nearest partition centroids
    dists = [distance(query, p.centroid) for p in overlay.partitions]
    target_partitions = top_n(dists, n_probe)

    # Check epoch freshness
    if overlay.epoch < current_epoch - stale_threshold:
        # Overlay is stale — broaden search
        target_partitions = top_n(dists, n_probe * 2)

    return target_partitions
```

### Epoch Rollback

If an overlay epoch is found to be corrupt or suboptimal:

```
1. Read rollback_pointer from current OVERLAY_SEG
2. The pointer gives the offset of the previous epoch's OVERLAY_SEG
3. Write a new MANIFEST_SEG pointing to the previous epoch as current
4. Future writes continue from the rolled-back state
```

This provides O(1) rollback to any ancestor epoch in the chain.

## 8. Integration with Progressive Indexing

The overlay system and the index system are coupled:

- **Partition centroids** in the overlay guide Layer A routing
- **Partition boundaries** determine which INDEX_SEGs cover which regions
- **Partition rebalancing** may invalidate Layer B adjacency for moved vectors
  (these are rebuilt lazily)
- **Layer C** is partitioned aligned — each INDEX_SEG covers vectors within
  a single partition for locality

This means overlay compaction can trigger partial index rebuild, but only for
the affected partitions — not the entire index.
