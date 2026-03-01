# RVF WASM Microkernel and Cognitum Hardware Mapping

## 1. Design Philosophy

RVF must run on hardware ranging from a 64 KB WASM tile to a petabyte
cluster. The WASM microkernel is the minimal runtime that makes a tile
a first-class RVF citizen — capable of answering queries, ingesting
streams, and participating in distributed search.

The microkernel is not a shrunken version of the full runtime. It is a
**purpose-built execution core** that exposes the exact set of operations
a tile needs, and nothing more.

## 2. Cognitum Tile Architecture

### Hardware Constraints

```
+-----------------------------------+
| Cognitum Tile                     |
|                                   |
|  Code Memory:    8 KB             |
|  Data Memory:    8 KB             |
|  SIMD Scratch:   64 KB            |
|  Registers:      v128 (WASM SIMD) |
|  Clock:          ~1 GHz           |
|  Interconnect:   Mesh to hub      |
|                                   |
|  No filesystem. No mmap.          |
|  No allocator beyond scratch.     |
|  All I/O through hub messages.    |
+-----------------------------------+
```

### Memory Map

```
Code (8 KB):
  0x0000 - 0x0FFF   Microkernel WASM bytecode (4 KB)
  0x1000 - 0x17FF   Distance function hot path (2 KB)
  0x1800 - 0x1FFF   Decode / quantization stubs (2 KB)

Data (8 KB):
  0x0000 - 0x003F   Tile configuration (64 B)
  0x0040 - 0x00FF   Query scratch (192 B: query vector fp16)
  0x0100 - 0x01FF   Result buffer (256 B: top-K candidates)
  0x0200 - 0x03FF   Routing table (512 B: entry points + centroids)
  0x0400 - 0x07FF   Decode workspace (1 KB)
  0x0800 - 0x0FFF   Message I/O buffer (2 KB)
  0x1000 - 0x1FFF   Neighbor list cache (4 KB)

SIMD Scratch (64 KB):
  0x0000 - 0x7FFF   Vector block (up to 85 vectors @ 384-dim fp16)
  0x8000 - 0xBFFF   Distance accumulator / PQ tables (16 KB)
  0xC000 - 0xEFFF   Hot cache subset (12 KB)
  0xF000 - 0xFFFF   Temporary / spill (4 KB)
```

### Tile Budget

For 384-dim fp16 vectors:
- One vector: 768 bytes
- SIMD scratch holds: 64 KB / 768 = ~85 vectors
- Top-K result buffer: 16 candidates * 16 B = 256 B
- Query vector: 768 B

A tile can process one block of ~85 vectors per cycle, computing distances
and maintaining a top-K heap entirely within scratch memory.

## 3. Microkernel Exports

The WASM microkernel exports exactly these functions:

```wat
;; === Core Query Path ===

;; Initialize tile with configuration
;; config_ptr: pointer to 64B tile config in data memory
(export "rvf_init" (func $rvf_init (param $config_ptr i32) (result i32)))

;; Load query vector into query scratch
;; query_ptr: pointer to fp16 vector in data memory
;; dim: vector dimensionality
(export "rvf_load_query" (func $rvf_load_query
    (param $query_ptr i32) (param $dim i32) (result i32)))

;; Load a block of vectors into SIMD scratch
;; block_ptr: pointer to vector block in SIMD scratch
;; count: number of vectors
;; dtype: data type enum
(export "rvf_load_block" (func $rvf_load_block
    (param $block_ptr i32) (param $count i32)
    (param $dtype i32) (result i32)))

;; Compute distances between query and loaded block
;; metric: 0=L2, 1=IP, 2=cosine, 3=hamming
;; result_ptr: pointer to write distances
(export "rvf_distances" (func $rvf_distances
    (param $metric i32) (param $result_ptr i32) (result i32)))

;; Merge distances into top-K heap
;; dist_ptr: pointer to distance array
;; id_ptr: pointer to vector ID array
;; count: number of candidates
;; k: top-K to maintain
(export "rvf_topk_merge" (func $rvf_topk_merge
    (param $dist_ptr i32) (param $id_ptr i32)
    (param $count i32) (param $k i32) (result i32)))

;; Read current top-K results
;; out_ptr: pointer to write results (id, distance pairs)
(export "rvf_topk_read" (func $rvf_topk_read
    (param $out_ptr i32) (result i32)))

;; === Quantization ===

;; Load scalar quantization parameters (min/max per dim)
(export "rvf_load_sq_params" (func $rvf_load_sq_params
    (param $params_ptr i32) (param $dim i32) (result i32)))

;; Dequantize int8 block to fp16 in SIMD scratch
(export "rvf_dequant_i8" (func $rvf_dequant_i8
    (param $src_ptr i32) (param $dst_ptr i32)
    (param $count i32) (result i32)))

;; Load PQ codebook subset
(export "rvf_load_pq_codebook" (func $rvf_load_pq_codebook
    (param $codebook_ptr i32) (param $M i32)
    (param $K i32) (result i32)))

;; Compute PQ asymmetric distances
(export "rvf_pq_distances" (func $rvf_pq_distances
    (param $codes_ptr i32) (param $count i32)
    (param $result_ptr i32) (result i32)))

;; === HNSW Navigation ===

;; Load neighbor list for a node
(export "rvf_load_neighbors" (func $rvf_load_neighbors
    (param $node_id i64) (param $layer i32)
    (param $out_ptr i32) (result i32)))

;; Greedy search step: given current node, find nearest neighbor
(export "rvf_greedy_step" (func $rvf_greedy_step
    (param $current_id i64) (param $layer i32) (result i64)))

;; === Segment Verification ===

;; Verify segment header hash
(export "rvf_verify_header" (func $rvf_verify_header
    (param $header_ptr i32) (result i32)))

;; Compute CRC32C of a data region
(export "rvf_crc32c" (func $rvf_crc32c
    (param $data_ptr i32) (param $len i32) (result i32)))
```

### Export Count

14 exports. Each maps to a tight inner loop that fits in the 8 KB code budget.
The host (hub) is responsible for all I/O, segment parsing, and orchestration.

## 4. Host-Tile Protocol

Communication between the hub and tile uses fixed-size messages through
the 2 KB I/O buffer:

### Message Format

```
Offset  Size  Field        Description
------  ----  -----        -----------
0x00    2     msg_type     Message type enum
0x02    2     msg_length   Payload length
0x04    4     msg_id       Correlation ID
0x08    var   payload      Type-specific payload
```

### Message Types

```
Hub -> Tile:
  0x01  LOAD_QUERY       Send query vector (768 B for 384-dim fp16)
  0x02  LOAD_BLOCK       Send vector block (up to ~1.5 KB compressed)
  0x03  LOAD_NEIGHBORS   Send neighbor list for a node
  0x04  LOAD_PARAMS      Send quantization parameters
  0x05  COMPUTE          Trigger distance computation
  0x06  READ_TOPK        Request current top-K results
  0x07  RESET            Clear tile state for new query

Tile -> Hub:
  0x81  TOPK_RESULT      Top-K results (id, distance pairs)
  0x82  NEED_BLOCK       Request a specific vector block
  0x83  NEED_NEIGHBORS   Request neighbor list for a node
  0x84  DONE             Computation complete
  0x85  ERROR            Error with code
```

### Execution Flow

```
Hub                                 Tile
 |                                    |
 |--- LOAD_QUERY (768B) ------------>|
 |                                    | rvf_load_query()
 |--- LOAD_PARAMS (SQ params) ------>|
 |                                    | rvf_load_sq_params()
 |--- LOAD_BLOCK (block 0) -------->|
 |                                    | rvf_load_block()
 |                                    | rvf_distances()
 |                                    | rvf_topk_merge()
 |--- LOAD_BLOCK (block 1) -------->|
 |                                    | rvf_load_block()
 |                                    | rvf_distances()
 |                                    | rvf_topk_merge()
 |    ...                             |
 |--- READ_TOPK -------------------->|
 |                                    | rvf_topk_read()
 |<--- TOPK_RESULT ------------------|
 |                                    |
```

### Pull Mode

For HNSW search, the tile drives the traversal:

```
Hub                                 Tile
 |                                    |
 |--- LOAD_QUERY -------------------->|
 |--- LOAD_NEIGHBORS (entry point) -->|
 |                                    | rvf_greedy_step()
 |<--- NEED_NEIGHBORS (next node) ----|
 |--- LOAD_NEIGHBORS (next node) ---->|
 |                                    | rvf_greedy_step()
 |<--- NEED_BLOCK (for candidate) ----|
 |--- LOAD_BLOCK -------------------->|
 |                                    | rvf_distances()
 |                                    | rvf_topk_merge()
 |<--- DONE ----------------------------|
 |--- READ_TOPK --------------------->|
 |<--- TOPK_RESULT ------------------|
```

## 5. Three Hardware Profiles

### RVF Core Profile (Tile)

```
Target:         Cognitum tile (8KB + 8KB + 64KB)
Features:       Distance compute, top-K, SQ dequant, CRC32C verify
Max vectors:    ~85 per block load
Max dimensions: 384 (fp16) or 768 (i8)
Index:          None (hub routes, tile computes)
Streaming:      Receive blocks from hub
Quantization:   i8 scalar only (no PQ on tile)
Compression:    None (hub decompresses before sending)
```

### RVF Hot Profile (Chip)

```
Target:         Cognitum chip (multiple tiles + shared memory)
Features:       Core + PQ distance, HNSW navigation, parallel tiles
Max vectors:    Limited by shared memory (~10K in shared cache)
Max dimensions: 1024
Index:          Layer A in shared memory
Streaming:      Block streaming across tiles
Quantization:   i8 scalar + PQ (6-bit)
Compression:    LZ4 decompress in shared memory
```

### RVF Full Profile (Hub/Desktop)

```
Target:         Desktop CPU, server, hub controller
Features:       All features, all segment types, all quantization
Max vectors:    Billions (limited by storage)
Max dimensions: Unlimited
Index:          Full HNSW (Layers A + B + C)
Streaming:      Full append-only segment model
Quantization:   All tiers (fp16, i8, PQ, binary)
Compression:    All (LZ4, ZSTD, custom)
Crypto:         Full (ML-DSA-65 signatures, SHAKE-256)
Temperature:    Full adaptive tiering
Overlay:        Full epoch model with compaction
```

### Profile Detection

The root manifest's `profile_id` field declares the minimum profile needed:

```
0x00    generic     Requires Full Profile features
0x01    core        Fully usable with Core Profile
0x02    hot         Requires Hot Profile minimum
0x03    full        Requires Full Profile
```

A Full Profile reader can always read Core or Hot files. A Core Profile
reader rejects Full Profile files but can read Core files. Hot Profile
readers can read Core and Hot files.

## 6. SIMD Strategy by Platform

### WASM v128 (Tile/Browser)

```wasm
;; L2 distance: fp16 vectors, 384 dimensions
;; Process 8 fp16 values per v128 operation

(func $l2_fp16_384 (param $a_ptr i32) (param $b_ptr i32) (result f32)
    (local $acc v128)
    (local $i i32)
    (local.set $acc (v128.const i64x2 0 0))
    (local.set $i (i32.const 0))

    (block $done
        (loop $loop
            ;; Load 8 fp16 values, widen to f32x4 pairs
            ;; Subtract, square, accumulate
            ;; ... (8 values per iteration, 48 iterations for 384 dims)

            (br_if $done (i32.ge_u (local.get $i) (i32.const 384)))
            (br $loop)
        )
    )
    ;; Horizontal sum of accumulator
    ;; Return L2 distance
)
```

### AVX-512 (Desktop/Server)

```
; Process 32 fp16 values per cycle with VCVTPH2PS + VFMADD231PS
; 384 dims = 12 iterations of 32 values
; ~12 cycles per distance computation
```

### ARM NEON (Mobile/Edge)

```
; Process 8 fp16 values per cycle with FMLA
; 384 dims = 48 iterations of 8 values
; ~48 cycles per distance computation
```

## 7. Microkernel Size Budget

```
Function                    Estimated Size
--------                    --------------
rvf_init                    128 B
rvf_load_query              64 B
rvf_load_block              256 B
rvf_distances (L2 fp16)     512 B
rvf_distances (L2 i8)       384 B
rvf_distances (IP fp16)     512 B
rvf_distances (hamming)     256 B
rvf_topk_merge              384 B
rvf_topk_read               64 B
rvf_load_sq_params          64 B
rvf_dequant_i8              256 B
rvf_load_pq_codebook        128 B
rvf_pq_distances            512 B
rvf_load_neighbors          128 B
rvf_greedy_step             512 B
rvf_verify_header           128 B
rvf_crc32c                  256 B
Message dispatch loop       384 B
Utility functions           256 B
WASM overhead               512 B
                            ----------
Total                       ~5,500 B (< 8 KB code budget)
```

Remaining ~2.5 KB of code space is available for domain-specific extensions
(e.g., codon distance for RVDNA profile, token overlap for RVText profile).

## 8. Fault Isolation

Each tile runs in a WASM sandbox. A tile cannot:
- Access hub memory directly
- Communicate with other tiles except through the hub
- Allocate memory beyond its 8 KB data + 64 KB scratch
- Execute code beyond its 8 KB code space
- Trap without the hub catching and recovering

If a tile traps (out-of-bounds, unreachable, stack overflow):
1. Hub catches the trap
2. Hub marks tile as faulted
3. Hub reassigns the tile's work to another tile (or processes locally)
4. Hub optionally restarts the faulted tile with fresh state

This makes the system resilient to individual tile failures — important for
large tile arrays where hardware faults are inevitable.
