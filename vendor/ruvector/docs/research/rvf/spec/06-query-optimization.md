# RVF Ultra-Fast Query Path

## 1. CPU Shape Optimization

The block layout determines performance at the hardware level. RVF is designed
to match the shape of modern CPUs: wide SIMD, deep caches, hardware prefetch.

### Four Optimizations

1. **Strict 64-byte alignment** for all numeric arrays
2. **Columnar + interleaved hybrid** for compression and speed
3. **Prefetch hints** for cache-friendly graph traversal
4. **Dictionary-coded IDs** for fast random access

## 2. Strict Alignment

Every numeric array in RVF starts at a 64-byte aligned offset. This matches:

| Target | Register Width | Alignment |
|--------|---------------|-----------|
| AVX-512 | 512 bits = 64 bytes | 64 B |
| AVX2 | 256 bits = 32 bytes | 64 B (superset) |
| ARM NEON | 128 bits = 16 bytes | 64 B (superset) |
| WASM v128 | 128 bits = 16 bytes | 64 B (superset) |
| Cache line | Typically 64 bytes | 64 B (exact) |

By aligning to 64 bytes, RVF ensures:
- Zero-copy load into any SIMD register (no unaligned penalty)
- No cache-line splits (each access touches exactly one cache line)
- Optimal hardware prefetch behavior (prefetcher operates on cache lines)

### Alignment in Practice

```
Segment header:           64 B (naturally aligned, first item in segment)
Block header:             Padded to 64 B boundary
Vector data start:        64 B aligned from block start
Each dimension column:    64 B aligned (columnar VEC_SEG)
Each vector entry:        64 B aligned (interleaved HOT_SEG)
ID map:                   64 B aligned
Restart point index:      64 B aligned
```

Padding bytes between sections are zero-filled and excluded from checksums.

## 3. Columnar + Interleaved Hybrid

### Columnar Storage (VEC_SEG) — Optimized for Compression

```
Block layout (1024 vectors, 384 dimensions, fp16):

Offset 0x000:   dim_0[vec_0], dim_0[vec_1], ..., dim_0[vec_1023]   (2048 B)
Offset 0x800:   dim_1[vec_0], dim_1[vec_1], ..., dim_1[vec_1023]   (2048 B)
...
Offset 0xBF800: dim_383[vec_0], ..., dim_383[vec_1023]              (2048 B)

Total: 384 * 2048 = 786,432 bytes (768 KB per block)
```

**Why columnar for cold/warm storage**:
- Adjacent values in the same dimension are correlated -> higher compression ratio
- LZ4 on columnar fp16 achieves 1.5-2.5x compression (vs 1.1-1.3x on interleaved)
- ZSTD on columnar fp16 achieves 2.5-4x compression
- Batch operations (computing mean, variance) scan one dimension at a time

### Interleaved Storage (HOT_SEG) — Optimized for Speed

```
Entry layout (one hot vector, 384 dim fp16):

Offset 0x000:   vector_id (8 B)
Offset 0x008:   dim_0, dim_1, dim_2, ..., dim_383  (768 B)
Offset 0x308:   neighbor_count (2 B)
Offset 0x30A:   neighbor_0, neighbor_1, ... (8 B each)
Offset 0x38A:   padding to 64B boundary
                --> 960 bytes per entry (at M=16 neighbors)
```

**Why interleaved for hot data**:
- One vector = one sequential read (no column gathering)
- Distance computation: load vector, compute, move to next (streaming pattern)
- Neighbors co-located: after finding a good candidate, immediately traverse
- 960 bytes per entry = 15 cache lines = predictable memory access

### When to Use Each

| Operation | Layout | Reason |
|-----------|--------|--------|
| Bulk distance computation | Columnar | SIMD operates on dimension columns |
| Top-K refinement scan | Interleaved | Sequential scan of candidates |
| Compression/archival | Columnar | Better ratio |
| HNSW search (hot region) | Interleaved | Vector + neighbors together |
| Batch insert | Columnar | Write once, compress well |

## 4. Prefetch Hints

### The Problem

HNSW search is pointer-chasing: compute distance at node A, read neighbor
list, jump to node B, compute distance, repeat. Each jump is a random
memory access. On a 10M vector file, this means:

```
HNSW search: ~100-200 distance computations per query
Each computation: 1 random read (vector) + 1 random read (neighbors)
Random read latency: 50-100 ns (DRAM), 10-50 μs (SSD)
Total: 10-40 μs (DRAM), 1-10 ms (SSD) without prefetch
```

### The Solution

Store neighbor lists **contiguously** and add **prefetch offsets** in the
manifest so the runtime can issue prefetch instructions ahead of time.

### Prefetch Table Structure

The manifest contains a prefetch table mapping node ID ranges to contiguous
page regions:

```
prefetch_table:
  entry_count: u32
  entries:
    [0]: node_ids 0-9999      -> pages at offset 0x100000, 50 pages, prefetch 3 ahead
    [1]: node_ids 10000-19999  -> pages at offset 0x200000, 50 pages, prefetch 3 ahead
    ...
```

### Runtime Prefetch Strategy

```python
def hnsw_search_with_prefetch(query, entry_point, ef_search):
    candidates = MaxHeap()
    visited = BitSet()
    worklist = MinHeap([(distance(query, entry_point), entry_point)])

    while worklist:
        dist, node = worklist.pop()

        # PREFETCH: while processing this node, prefetch neighbors' data
        neighbors = get_neighbors(node)
        for n in neighbors[:PREFETCH_AHEAD]:
            if n not in visited:
                prefetch_vector(n)      # madvise(WILLNEED) or __builtin_prefetch
                prefetch_neighbors(n)   # prefetch neighbor list page

        # COMPUTE: distance to neighbors (data should be in cache by now)
        for n in neighbors:
            if n not in visited:
                visited.add(n)
                d = distance(query, get_vector(n))
                if d < candidates.max() or len(candidates) < ef_search:
                    candidates.push((d, n))
                    worklist.push((d, n))

    return candidates.top_k(k)
```

### Contiguous Neighbor Layout

HOT_SEG stores vectors and neighbors together. For cold INDEX_SEGs, neighbor
lists are laid out in **node ID order** within contiguous pages:

```
Page 0:  neighbors[node_0], neighbors[node_1], ..., neighbors[node_63]
Page 1:  neighbors[node_64], ..., neighbors[node_127]
...
```

Because HNSW search tends to traverse nodes in the same graph neighborhood
(spatially close node IDs if data was inserted in order), sequential node
IDs tend to be accessed together. Contiguous layout turns random access
into sequential reads.

### Expected Improvement

| Configuration | p95 Latency (10M vectors) |
|--------------|--------------------------|
| No prefetch, random layout | 2.5 ms |
| No prefetch, contiguous layout | 1.2 ms |
| Prefetch, contiguous layout | 0.3 ms |
| Prefetch, contiguous + hot cache | 0.15 ms |

## 5. Dictionary-Coded IDs

### The Problem

Vector IDs in neighbor lists and ID maps are 64-bit integers. For 10M vectors,
most IDs fit in 24 bits. Storing full 64-bit IDs wastes ~5 bytes per entry.

With M=16 neighbors per node and 10M nodes:
- Raw: 10M * 16 * 8 = 1.2 GB of ID data
- Desired: < 300 MB

### Varint Delta Encoding

IDs within a block or neighbor list are sorted and delta-encoded:

```
Original IDs:    [1000, 1005, 1008, 1020, 1100]
Deltas:          [1000,    5,    3,   12,   80]
Varint bytes:    [  2B,  1B,  1B,   1B,   1B]  = 6 bytes (vs 40 bytes raw)
```

### Restart Points

Every N entries (default N=64), the delta resets to an absolute value:

```
Group 0 (entries 0-63):    delta from 0 (absolute start)
Group 1 (entries 64-127):  delta from entry[64] (restart)
Group 2 (entries 128-191): delta from entry[128] (restart)
```

The restart point index stores the offset of each restart group:

```
restart_index:
  interval: 64
  offsets: [0, 156, 298, 445, ...]  // byte offsets into encoded data
```

### Random Access

To find the neighbors of node N:

```
1. group = N / restart_interval            // O(1)
2. offset = restart_index[group]           // O(1)
3. seek to offset in encoded data          // O(1)
4. decode sequentially from restart to N   // O(restart_interval) = O(64)
```

Total: O(64) varint decodes = ~50-100 ns. Compare with sorted array binary
search: O(log N) = O(24) comparisons with cache misses = ~200-500 ns.

### SIMD Varint Decoding

Modern SIMD can decode varints in bulk:

```
AVX-512 VBMI: ~8 varints per cycle using VPERMB + VPSHUFB
Throughput: 2-4 billion integers/second (Lemire et al.)
```

At 16 neighbors per node, one HNSW search step decodes 16 varints in ~2-4 ns.

### Compression Ratio

| Encoding | Bytes per ID (avg) | 10M * 16 neighbors |
|----------|-------------------|-------------------|
| Raw u64 | 8.0 B | 1,220 MB |
| Raw u32 | 4.0 B | 610 MB |
| Varint (no delta) | 3.2 B | 488 MB |
| Varint delta | 1.5 B | 229 MB |
| Varint delta + restart | 1.6 B | 244 MB |

Delta encoding with restart points achieves ~5x compression over raw u64
while maintaining fast random access.

## 6. Cache Behavior Analysis

### L1/L2/L3 Working Sets

For a typical query on 10M vectors (384 dim, fp16):

```
HNSW search:
  ~150 distance computations
  Each computation: 768 B (vector) + ~128 B (neighbor list) ≈ 896 B
  Total working set: 150 * 896 ≈ 131 KB

Top-K refinement (hot cache scan):
  ~1000 candidates checked
  Each: 960 B (interleaved HOT_SEG entry)
  Total: 960 KB

Query vector: 768 B (always in L1)
Quantization tables: 96 KB (PQ codebook, always in L2)
```

| Cache Level | Size | What Fits |
|------------|------|-----------|
| L1 (32-48 KB) | Query vector + current node | Always hit |
| L2 (256 KB-1 MB) | PQ tables + 100-200 hot entries | Usually hit |
| L3 (8-32 MB) | Hot cache + partial index | Mostly hit |
| DRAM | Everything | Full dataset |

### p95 Latency Budget

```
HNSW traversal:    150 nodes * 100 ns/node = 15 μs (L3 hit)
Distance compute:  150 * 50 ns = 7.5 μs (SIMD)
Top-K refinement:  1000 * 10 ns = 10 μs (hot cache, L2/L3 hit)
Overhead:          5 μs (heap ops, bookkeeping)
                   -------
Total p95:         ~37.5 μs ≈ 0.04 ms

With prefetch:     ~30 μs (hide 25% of traversal latency)
```

This matches the target of < 0.3 ms p95 on desktop hardware. The dominant
cost is memory bandwidth, not computation — which is why cache-friendly
layout and prefetch are critical.

## 7. Distance Function SIMD Implementations

### L2 Distance (fp16, 384 dim, AVX-512)

```
; 384 fp16 values = 768 bytes = 12 ZMM registers
; Process 32 fp16 values per iteration (convert to 16 fp32 per half)

.loop:
    vmovdqu16   zmm0, [rsi + rcx]      ; Load 32 fp16 from A
    vmovdqu16   zmm1, [rdi + rcx]      ; Load 32 fp16 from B
    vcvtph2ps   zmm2, ymm0             ; Convert low 16 to fp32
    vcvtph2ps   zmm3, ymm1
    vsubps      zmm2, zmm2, zmm3       ; diff = A - B
    vfmadd231ps zmm4, zmm2, zmm2       ; acc += diff * diff
    ; Repeat for high 16
    vextracti64x4 ymm0, zmm0, 1
    vextracti64x4 ymm1, zmm1, 1
    vcvtph2ps   zmm2, ymm0
    vcvtph2ps   zmm3, ymm1
    vsubps      zmm2, zmm2, zmm3
    vfmadd231ps zmm4, zmm2, zmm2
    add         rcx, 64
    cmp         rcx, 768
    jl          .loop

; Horizontal sum of zmm4 -> scalar result
; ~12 iterations, ~24 FMA ops, ~12 cycles total
```

### Inner Product (int8, 384 dim, AVX-512 VNNI)

```
; 384 int8 values = 384 bytes = 6 ZMM registers
; VPDPBUSD: 64 uint8*int8 multiply-adds per cycle

.loop:
    vmovdqu8    zmm0, [rsi + rcx]      ; 64 uint8 from A
    vmovdqu8    zmm1, [rdi + rcx]      ; 64 int8 from B
    vpdpbusd    zmm2, zmm0, zmm1       ; acc += dot(A, B) per 4 bytes
    add         rcx, 64
    cmp         rcx, 384
    jl          .loop

; 6 iterations, 6 VPDPBUSD ops, ~6 cycles
; ~16x faster than fp16 L2
```

### Hamming Distance (binary, 384 dim, AVX-512)

```
; 384 bits = 48 bytes = 1 partial ZMM load
; VPOPCNTDQ: popcount on 8 x 64-bit words per cycle

    vmovdqu8    zmm0, [rsi]            ; Load 48 bytes (384 bits) from A
    vmovdqu8    zmm1, [rdi]            ; Load 48 bytes from B
    vpxorq      zmm2, zmm0, zmm1       ; XOR -> differing bits
    vpopcntq    zmm3, zmm2             ; Popcount per 64-bit word
    ; Horizontal sum of 6 popcounts -> Hamming distance
    ; ~3 cycles total
```

## 8. Summary: Query Path Hot Loop

The complete hot path for one HNSW search step:

```
1. Load current node's neighbor list       [L2/L3 cache, 128 B, ~5 ns]
2. Issue prefetch for next neighbors       [~1 ns]
3. For each neighbor (M=16):
   a. Check visited bitmap                 [L1, ~1 ns]
   b. Load neighbor vector (hot cache)     [L2/L3, 768 B, ~5-10 ns]
   c. SIMD distance (fp16, 384 dim)        [~12 cycles = ~4 ns]
   d. Heap insert if better                [~5 ns]
4. Total per step: ~300-500 ns
5. Total per query (~150 steps): ~50-75 μs
```

This achieves 13,000-20,000 QPS per thread on desktop hardware — matching
or exceeding dedicated vector databases for in-memory workloads.
