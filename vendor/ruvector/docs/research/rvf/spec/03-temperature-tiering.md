# RVF Temperature Tiering

## 1. Adaptive Layout as a First-Class Concept

Traditional vector formats place data once and leave it. RVF treats data placement
as a **continuous optimization problem**. Every vector block has a temperature, and
the format periodically reorganizes to keep hot data fast and cold data small.

```
                Access Frequency
                     ^
                     |
Tier 0 (HOT)        |  ████████   fp16 / 8-bit, interleaved
                     |  ████████   < 1μs random access
                     |
Tier 1 (WARM)        |  ░░░░░░░░░░░░░░░░   5-7 bit quantized
                     |  ░░░░░░░░░░░░░░░░   columnar, compressed
                     |
Tier 2 (COLD)        |  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒   3-bit or 1-bit
                     |  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒   heavy compression
                     |
                     +------------------------------------> Vector ID
```

### Tier Definitions

| Tier | Name | Quantization | Layout | Compression | Access Latency |
|------|------|-------------|--------|-------------|----------------|
| 0 | Hot | fp16 or int8 | Interleaved (row-major) | None or LZ4 | < 1 μs |
| 1 | Warm | 5-7 bit SQ/PQ | Columnar | LZ4 or ZSTD | 1-10 μs |
| 2 | Cold | 3-bit or binary | Columnar | ZSTD level 9+ | 10-100 μs |

### Memory Ratios

For 384-dimensional vectors (typical embedding size):

| Tier | Bytes/Vector | Ratio vs fp32 | 10M Vectors |
|------|-------------|---------------|-------------|
| fp32 (raw) | 1536 B | 1.0x | 14.3 GB |
| Tier 0 (fp16) | 768 B | 2.0x | 7.2 GB |
| Tier 0 (int8) | 384 B | 4.0x | 3.6 GB |
| Tier 1 (6-bit) | 288 B | 5.3x | 2.7 GB |
| Tier 1 (5-bit) | 240 B | 6.4x | 2.2 GB |
| Tier 2 (3-bit) | 144 B | 10.7x | 1.3 GB |
| Tier 2 (1-bit) | 48 B | 32.0x | 0.45 GB |

## 2. Access Counter Sketch

Temperature decisions require knowing which blocks are accessed frequently.
RVF maintains a lightweight **Count-Min Sketch** per block set, stored in
SKETCH_SEG segments.

### Sketch Parameters

```
Width (w):    1024 counters
Depth (d):    4 hash functions
Counter size: 8-bit saturating (max 255)
Memory:       1024 * 4 * 1 = 4 KB per sketch
Granularity:  One sketch per 1024-vector block
Decay:        Halve all counters every 2^16 accesses (aging)
```

For 10M vectors in 1024-vector blocks:
- 9,766 blocks
- 9,766 * 4 KB = ~38 MB of sketches
- Stored in SKETCH_SEG, referenced by manifest

### Sketch Operations

**On query access**:
```
block_id = vector_id / block_size
for i in 0..depth:
    idx = hash_i(block_id) % width
    sketch[i][idx] = min(sketch[i][idx] + 1, 255)
```

**On temperature check**:
```
count = min over i of sketch[i][hash_i(block_id) % width]
if count > HOT_THRESHOLD:   tier = 0
elif count > WARM_THRESHOLD: tier = 1
else:                        tier = 2
```

**Aging** (every 2^16 accesses):
```
for all counters: counter = counter >> 1
```

This ensures the sketch tracks *recent* access patterns, not cumulative history.

### Why Count-Min Sketch

| Alternative | Memory | Accuracy | Update Cost |
|------------|--------|----------|-------------|
| Per-vector counter | 80 MB (10M * 8B) | Exact | O(1) |
| Count-Min Sketch | 38 MB | ~99.9% | O(depth) = O(4) |
| HyperLogLog | 6 MB | ~98% | O(1) but cardinality only |
| Bloom filter | 12 MB | No counting | N/A |

Count-Min Sketch is the best trade-off: sub-exact accuracy with bounded memory
and constant-time updates.

## 3. Promotion and Demotion

### Promotion: Warm/Cold -> Hot

When a block's access count exceeds HOT_THRESHOLD for two consecutive sketch
epochs:

```
1. Read the block from its current VEC_SEG
2. Decode/dequantize vectors to fp16 or int8
3. Rearrange from columnar to interleaved layout
4. Write as a new HOT_SEG (or append to existing HOT_SEG)
5. Update manifest with new tier assignment
6. Optionally: add neighbor lists to HOT_SEG for locality
```

### Demotion: Hot -> Warm -> Cold

When a block's access count drops below WARM_THRESHOLD:

```
1. The block is not immediately rewritten
2. On next compaction cycle, the block is written to the appropriate tier
3. Quantization is applied during compaction (not lazily)
4. The HOT_SEG entry is tombstoned in the manifest
```

### Eviction as Compression

The key insight: **eviction from hot tier is just compression, not deletion**.
The vector data is always present — it just moves to a more compressed
representation. This means:

- No data loss on eviction
- Recall degrades gracefully (quantized vectors still contribute to search)
- The file naturally compresses over time as access patterns stabilize

## 4. Temperature-Aware Compaction

Standard compaction merges segments for space efficiency. Temperature-aware
compaction also **rearranges blocks by tier**:

```
Before compaction:
  VEC_SEG_1:  [hot] [cold] [warm] [hot] [cold]
  VEC_SEG_2:  [warm] [hot] [cold] [warm] [warm]

After temperature-aware compaction:
  HOT_SEG:    [hot] [hot] [hot]       <- interleaved, fp16
  VEC_SEG_W:  [warm] [warm] [warm] [warm]  <- columnar, 6-bit
  VEC_SEG_C:  [cold] [cold] [cold]     <- columnar, 3-bit
```

This creates **physical locality by temperature**: hot blocks are contiguous
(good for sequential scan), warm blocks are contiguous (good for batch decode),
cold blocks are contiguous (good for compression ratio).

### Compaction Triggers

| Trigger | Condition | Action |
|---------|-----------|--------|
| Sketch epoch | Every N writes | Evaluate all block temperatures |
| Space amplification | Dead space > 30% | Merge + rewrite segments |
| Tier imbalance | Hot tier > 20% of data | Demote cold blocks |
| Hot miss rate | Hot cache miss > 10% | Promote missing blocks |

## 5. Quantization Strategies by Tier

### Tier 0: Hot

**Scalar quantization to int8** (preferred) or **fp16** (for maximum recall).

```
Encoding:
  q = round((v - min) / (max - min) * 255)

Decoding:
  v = q / 255 * (max - min) + min

Parameters stored in QUANT_SEG:
  min: f32 per dimension
  max: f32 per dimension
```

Distance computation directly on int8 using SIMD (vpsubb + vpmaddubsw on AVX-512).

### Tier 1: Warm

**Product Quantization (PQ)** with 5-7 bits per sub-vector.

```
Parameters:
  M subspaces:          48 (for 384-dim vectors, 8 dims per subspace)
  K centroids per sub:  64 (6-bit) or 128 (7-bit)
  Codebook:             M * K * 8 * sizeof(f32) = 48 * 64 * 8 * 4 = 96 KB

Encoding:
  For each subvector: find nearest centroid -> store centroid index

Distance computation:
  ADC (Asymmetric Distance Computation) with precomputed distance tables
```

### Tier 2: Cold

**Binary quantization** (1-bit) or **ternary quantization** (2-bit / 3-bit).

```
Binary encoding:
  b = sign(v)  -> 1 bit per dimension
  384 dims -> 48 bytes per vector (32x compression)

Distance:
  Hamming distance via POPCNT
  XOR + POPCNT on AVX-512: 512 bits per cycle

Ternary (3-bit with magnitude):
  t = {-1, 0, +1} based on threshold
  magnitude = |v| quantized to 3 levels
  384 dims -> 144 bytes per vector (10.7x compression)
```

### Codebook Storage

All quantization parameters (codebooks, min/max ranges, centroids) are stored
in QUANT_SEG segments. The root manifest's `quantdict_seg_offset` hotset pointer
references the active quantization dictionary for fast boot.

Multiple QUANT_SEGs can coexist for different tiers — the manifest maps each
tier to its dictionary.

## 6. Hardware Adaptation

### Desktop (AVX-512)

- Hot tier: int8 with VNNI dot product (4 int8 multiplies per cycle)
- Warm tier: PQ with AVX-512 gather for table lookups
- Cold tier: Binary with VPOPCNTDQ (512-bit popcount)

### ARM (NEON)

- Hot tier: int8 with SDOT instruction
- Warm tier: PQ with TBL for table lookups
- Cold tier: Binary with CNT (population count)

### WASM (v128)

- Hot tier: int8 with i8x16.dot_i7x16_i16x8 (if available)
- Warm tier: Scalar PQ (no gather)
- Cold tier: Binary with manual popcount

### Cognitum Tile (8KB code + 8KB data + 64KB SIMD)

- Hot tier only: int8 interleaved, fits in SIMD scratch
- No warm/cold — data stays on hub, tile fetches blocks on demand
- Sketch is maintained by hub, not tile

## 7. Self-Organization Over Time

```
t=0    All data Tier 1 (default warm)
       |
t+N    First sketch epoch: identify hot blocks
       Promote top 5% to Tier 0
       |
t+2N   Second epoch: validate promotions
       Demote false positives back to Tier 1
       Identify true cold blocks (0 access in 2 epochs)
       |
t+3N   Compaction: physically separate tiers
       HOT_SEG created with interleaved layout
       Cold blocks compressed to 3-bit
       |
t+∞    Equilibrium: ~5% hot, ~30% warm, ~65% cold
       File size: ~2-3x smaller than uniform fp16
       Query p95: dominated by hot tier latency
```

The format converges to an equilibrium that reflects actual usage. No manual
tuning required.
