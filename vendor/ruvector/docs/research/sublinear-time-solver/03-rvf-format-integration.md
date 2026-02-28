# RVF Format Integration Analysis for Sublinear-Time-Solver

**Agent**: 3 (RVF Format Integration Analysis)
**Date**: 2026-02-20
**Status**: Complete

---

## 1. RVF Format Specification Details

### 1.1 Format Overview

RVF (RuVector Format) is a self-reorganizing binary substrate adopted as the canonical format across all RuVector libraries (ADR-029, accepted 2026-02-13). It is not a static file format but a runtime substrate that supports append-only writes, progressive loading, temperature-tiered storage, and crash safety without a write-ahead log.

The format is governed by four inviolable design laws:

1. **Truth Lives at the Tail** -- The most recent `MANIFEST_SEG` at EOF is the sole source of truth.
2. **Every Segment Is Independently Valid** -- Each segment carries its own magic, length, content hash, and type.
3. **Data and State Are Separated** -- Vector payloads, indexes, overlays, and metadata occupy distinct segment types.
4. **The Format Adapts to Its Workload** -- Access sketches drive temperature-tiered promotion and compaction.

### 1.2 Segment Header (64 bytes)

Every segment begins with a fixed 64-byte header defined in `/home/user/ruvector/crates/rvf/rvf-types/src/segment.rs` as a `#[repr(C)]` struct with compile-time size assertion:

```
Offset  Type    Field              Size
------  ----    -----              ----
0x00    u32     magic              4B    (0x52564653 = "RVFS")
0x04    u8      version            1B    (currently 1)
0x05    u8      seg_type           1B    (segment type enum)
0x06    u16     flags              2B    (bitfield, 12 defined bits)
0x08    u64     segment_id         8B    (monotonic ordinal)
0x10    u64     payload_length     8B
0x18    u64     timestamp_ns       8B    (UNIX nanoseconds)
0x20    u8      checksum_algo      1B    (0=CRC32C, 1=XXH3-128, 2=SHAKE-256)
0x21    u8      compression        1B    (0=none, 1=LZ4, 2=ZSTD, 3=custom)
0x22    u16     reserved_0         2B
0x24    u32     reserved_1         4B
0x28    [u8;16] content_hash       16B   (first 128 bits of payload hash)
0x38    u32     uncompressed_len   4B
0x3C    u32     alignment_pad      4B
                                   ----
                                   64B total
```

Key constants (from `/home/user/ruvector/crates/rvf/rvf-types/src/constants.rs`):
- `SEGMENT_MAGIC`: `0x5256_4653` ("RVFS" big-endian)
- `ROOT_MANIFEST_MAGIC`: `0x5256_4D30` ("RVM0")
- `SEGMENT_ALIGNMENT`: 64 bytes
- `MAX_SEGMENT_PAYLOAD`: 4 GiB
- `SEGMENT_HEADER_SIZE`: 64 bytes
- `SEGMENT_VERSION`: 1

### 1.3 Segment Type Registry (23 variants)

Defined in `/home/user/ruvector/crates/rvf/rvf-types/src/segment_type.rs`:

| Value | Name | Purpose |
|-------|------|---------|
| 0x00 | Invalid | Uninitialized / zeroed region |
| 0x01 | Vec | Raw vector payloads (embeddings) |
| 0x02 | Index | HNSW adjacency lists |
| 0x03 | Overlay | Graph overlay deltas |
| 0x04 | Journal | Metadata mutations |
| 0x05 | Manifest | Segment directory |
| 0x06 | Quant | Quantization dictionaries and codebooks |
| 0x07 | Meta | Arbitrary key-value metadata |
| 0x08 | Hot | Temperature-promoted data (interleaved) |
| 0x09 | Sketch | Access counter sketches |
| 0x0A | Witness | Capability manifests, audit trails |
| 0x0B | Profile | Domain profile declarations |
| 0x0C | Crypto | Key material, signature chains |
| 0x0D | MetaIdx | Metadata inverted indexes |
| 0x0E | Kernel | Embedded kernel image |
| 0x0F | Ebpf | Embedded eBPF program |
| 0x10 | Wasm | Embedded WASM bytecode |
| 0x20 | CowMap | COW cluster mapping |
| 0x21 | Refcount | Cluster reference counts |
| 0x22 | Membership | Vector membership filter |
| 0x23 | Delta | Sparse delta patches |
| 0x30 | TransferPrior | Cross-domain posterior summaries |
| 0x31 | PolicyKernel | Policy kernel configuration |
| 0x32 | CostCurve | Cost curve convergence data |

Available ranges for extension: `0x11-0x1F`, `0x24-0x2F`, `0x33-0xEF`. Values `0xF0-0xFF` are reserved.

### 1.4 Flags Bitfield (12 bits defined)

From `/home/user/ruvector/crates/rvf/rvf-types/src/flags.rs`:

| Bit | Mask | Name | Meaning |
|-----|------|------|---------|
| 0 | 0x0001 | COMPRESSED | Payload compressed |
| 1 | 0x0002 | ENCRYPTED | Payload encrypted |
| 2 | 0x0004 | SIGNED | Signature footer follows |
| 3 | 0x0008 | SEALED | Immutable (compaction output) |
| 4 | 0x0010 | PARTIAL | Streaming write |
| 5 | 0x0020 | TOMBSTONE | Logically deletes prior segment |
| 6 | 0x0040 | HOT | Temperature-promoted data |
| 7 | 0x0080 | OVERLAY | Contains overlay/delta data |
| 8 | 0x0100 | SNAPSHOT | Full snapshot (not delta) |
| 9 | 0x0200 | CHECKPOINT | Safe rollback point |
| 10 | 0x0400 | ATTESTED | Produced inside TEE |
| 11 | 0x0800 | HAS_LINEAGE | Carries lineage provenance |

### 1.5 Wire Format Primitives

From `/home/user/ruvector/crates/rvf/rvf-wire/src/varint.rs` and `delta.rs`:

- **Byte order**: All multi-byte integers are little-endian. IEEE 754 little-endian for floats.
- **Varint**: LEB128 unsigned encoding, 1-10 bytes for u64.
- **Signed varint**: ZigZag + LEB128.
- **Delta encoding**: Sorted integer sequences stored as deltas with restart points every N entries (default 128). Restart points store absolute values for random access.

### 1.6 Data Type Enum

From `/home/user/ruvector/crates/rvf/rvf-types/src/data_type.rs`:

| Value | Type | Bits/Element |
|-------|------|-------------|
| 0x00 | f32 | 32 |
| 0x01 | f16 | 16 |
| 0x02 | bf16 | 16 |
| 0x03 | i8 | 8 |
| 0x04 | u8 | 8 |
| 0x05 | i4 | 4 |
| 0x06 | binary | 1 |
| 0x07 | PQ | variable |
| 0x08 | custom | variable |

### 1.7 Key Payload Layouts

**VEC_SEG** (columnar, from `/home/user/ruvector/crates/rvf/rvf-wire/src/vec_seg_codec.rs`):
- Block directory: `block_count(u32)` + per-block entries of `offset(u32) + count(u32) + dim(u16) + dtype(u8) + tier(u8)` = 12 bytes each
- Vector data stored columnar: all dim_0 values, then dim_1, etc.
- ID map: delta-varint encoded sorted IDs with restart points
- Per-block CRC32C integrity

**INDEX_SEG** (from `/home/user/ruvector/crates/rvf/rvf-wire/src/index_seg_codec.rs`):
- Index header: `index_type(u8) + layer_level(u8) + M(u16) + ef_construction(u32) + node_count(u64)` = 16 bytes
- Restart point index for random access
- Adjacency data: per-node varint layer_count, then per-layer varint neighbor_count + delta-encoded neighbor IDs

**HOT_SEG** (interleaved, from `/home/user/ruvector/crates/rvf/rvf-wire/src/hot_seg_codec.rs`):
- Header: `vector_count(u32) + dim(u16) + dtype(u8) + neighbor_m(u16)` = 9 bytes, padded to 64B
- Per-entry: `vector_id(u64) + vector_data[dim*elem_size] + neighbor_count(u16) + neighbor_ids[count*8]`, each entry 64B aligned

### 1.8 Existing Serialization Infrastructure

The RVF crate ecosystem already provides:
- `rvf-wire`: Complete binary reader/writer with XXH3-128 content hashing
- `rvf-quant`: Scalar, product, and binary quantization codecs
- `rvf-crypto`: SHAKE-256 witness chains, Ed25519 and ML-DSA-65 signatures
- `rvf-manifest`: Two-level manifest system (4 KB Level 0 root + Level 1 TLV records)
- `rvf-runtime`: Full store with compaction, streaming ingest, and query paths
- `rvf-server`: TCP streaming protocol with length-prefixed framing

### 1.9 Existing Bridge Pattern

The domain expansion bridge (`/home/user/ruvector/crates/ruvector-domain-expansion/src/rvf_bridge.rs`) provides a concrete example of how external data types map to RVF segments. Key patterns:
- Wire-format wrapper structs (e.g., `WireTransferPrior`) convert HashMap keys to Vec-of-tuples for JSON serialization
- `transfer_prior_to_segment()` serializes via JSON, then wraps in an RVF segment using `rvf_wire::writer::write_segment()`
- `transfer_prior_from_segment()` validates header, verifies content hash, then deserializes JSON
- TLV encoding: `[tag: u16 LE][length: u32 LE][value: length bytes]`
- Multi-segment assembly concatenates individually 64-byte-aligned segments

---

## 2. Sublinear-Time-Solver Data Type Mapping to RVF

### 2.1 Type Inventory

The sublinear-time-solver codebase uses these core serializable types:

| Type | Serde Support | Primary Content |
|------|--------------|-----------------|
| `SparseMatrix` (CSR/CSC/COO) | Yes (serde) | row_ptr, col_idx, values arrays |
| `Matrix` (dense) | Yes (serde) | rows, cols, data Vec<f64> |
| `SolverOptions` | Yes (serde) | tolerance, max_iter, method config |
| `SublinearConfig` | Yes (serde) | sampling rates, sketch params |
| `SolverResult` | Yes (serde) | solution vector, residual, iterations |
| `PartialSolution` | Yes (serde) | partial results, convergence state |
| `SolutionStep` | Yes (serde) | iteration snapshot, step metrics |

### 2.2 Mapping Strategy

Each solver type maps naturally to one or more RVF segment types:

#### Dense Matrix -> VEC_SEG

Dense matrices map directly to VEC_SEG using columnar layout:
- Each column of the matrix becomes one "dimension" in the RVF vector model
- `dtype = 0x00` (f32) or a proposed `0x09` extension for f64
- Block directory entries carry `dim = cols` and `vector_count = rows`
- The columnar layout aligns with how many numerical solvers access matrix data (column-major operations)

```
Matrix { rows: 1000, cols: 128, data: Vec<f64> }
  -> VEC_SEG {
       block_count: 1,
       block_entries: [{
         block_offset: 64,
         vector_count: 1000,
         dim: 128,
         dtype: 0x09,  // f64 extension
         tier: 0
       }],
       data: columnar f64 layout
     }
```

#### SparseMatrix -> New SPARSE_SEG (proposed 0x24) or META_SEG + VEC_SEG hybrid

Sparse matrices require a dedicated approach because RVF's VEC_SEG assumes dense, fixed-dimension vectors. Three options, in order of preference:

**Option A: New SPARSE_SEG (0x24)** -- Uses the reserved segment type range:
```
SPARSE_SEG Payload Layout:
  Sparse Header (64B aligned):
    format: u8          (0=CSR, 1=CSC, 2=COO)
    dtype: u8           (0x00=f32, 0x09=f64)
    rows: u64
    cols: u64
    nnz: u64            (number of non-zeros)
    [padding to 64B]

  CSR Layout:
    row_ptr: [u64; rows+1]     delta-varint encoded
    col_idx: [u64; nnz]        delta-varint encoded per row
    values: [dtype; nnz]       raw little-endian

  CSC Layout:
    col_ptr: [u64; cols+1]     delta-varint encoded
    row_idx: [u64; nnz]        delta-varint encoded per column
    values: [dtype; nnz]       raw little-endian

  COO Layout:
    row_idx: [u64; nnz]        delta-varint encoded (sorted)
    col_idx: [u64; nnz]        delta-varint encoded per row group
    values: [dtype; nnz]       raw little-endian
```

**Option B: META_SEG + VEC_SEG compound** -- Stores structure in META_SEG and values in VEC_SEG:
- META_SEG contains JSON with format type, dimensions, and pointer indices
- VEC_SEG contains the values array as a single-dimension vector block
- Cross-referencing via segment IDs in the manifest

**Option C: Delta segment repurposing** -- The existing `Delta` segment type (0x23) is described as "sparse delta patches" and could be extended for general sparse matrix storage.

**Recommendation**: Option A (new SPARSE_SEG at 0x24) provides the cleanest integration. It uses the existing RVF primitives (varint delta encoding, 64-byte alignment, content hashing) while adding sparse-specific structure.

#### SolverOptions / SublinearConfig -> META_SEG

Configuration types are small, structured data that maps naturally to META_SEG:
```
META_SEG payload:
  TLV records:
    [tag=0x0100 "solver_options"][len][JSON payload]
    [tag=0x0101 "sublinear_config"][len][JSON payload]
```

This mirrors how the domain expansion bridge stores PolicyKernel and TransferPrior configurations. The existing serde_json support in the solver types makes this trivial.

#### SolverResult -> WITNESS_SEG + VEC_SEG

Solver results contain both the solution vector and computation metadata:
- Solution vector -> VEC_SEG (dense column vector)
- Convergence metadata (residual, iterations, timing) -> WITNESS_SEG as computation proof
- The WITNESS_SEG integration provides tamper-evident verification of solver correctness

#### PartialSolution -> VEC_SEG with PARTIAL flag

Partial solutions map to VEC_SEG segments with the `PARTIAL` flag (bit 4) set:
- Each checkpoint during iterative solving emits a VEC_SEG with PARTIAL + CHECKPOINT flags
- The convergence state metadata goes into an associated META_SEG
- Progressive loading allows clients to read partial results before the solve completes

#### SolutionStep -> WITNESS_SEG chain

Individual solution steps form a witness chain:
- Each step's metrics (iteration number, residual, wall time) are hashed into a SHAKE-256 witness entry
- The chain provides verifiable proof that the solver followed a valid convergence trajectory
- This extends the existing witness chain pattern used by `rvf-solver-wasm`

### 2.3 Data Type Extension: f64 Support

The current RVF DataType enum supports f32 but not f64. The sublinear-time-solver uses f64 extensively. Two approaches:

**Approach 1: Extend DataType enum** -- Add `F64 = 0x09` to `/home/user/ruvector/crates/rvf/rvf-types/src/data_type.rs`. This is the preferred approach because:
- The enum has room (0x09 is unused)
- The wire format already handles 8-byte element sizes in other contexts
- All vec_seg_codec and hot_seg_codec functions use `dtype_element_size()` which is easily extended

**Approach 2: Use Custom (0x08) with QUANT_SEG metadata** -- Store f64 data using the Custom dtype and describe the encoding in an associated QUANT_SEG. This works but adds unnecessary indirection for a standard numeric type.

---

## 3. Sparse Matrix Serialization Compatibility

### 3.1 CSR Format in RVF

CSR (Compressed Sparse Row) is the most common sparse matrix format in numerical computing. Its components map to RVF primitives as follows:

| CSR Component | RVF Primitive | Encoding |
|---------------|--------------|----------|
| `row_ptr[rows+1]` | Sorted u64 array | Delta-varint with restart points |
| `col_idx[nnz]` | Sorted-per-row u64 array | Delta-varint per row group |
| `values[nnz]` | f32/f64 array | Raw little-endian, 64B aligned |

The delta-varint encoding is particularly efficient for CSR because:
- `row_ptr` is monotonically increasing (perfect for delta encoding)
- `col_idx` within each row is typically sorted (column indices in ascending order)
- Average delta between consecutive column indices is small for structured matrices

**Size analysis for a 10M x 10M sparse matrix with 100M non-zeros (10 nnz/row avg)**:

| Component | Raw Size | Delta-Varint Size | Compression Ratio |
|-----------|----------|-------------------|-------------------|
| row_ptr (10M+1 entries) | 80 MB | ~15 MB | 5.3x |
| col_idx (100M entries) | 800 MB | ~200 MB | 4.0x |
| values (100M f64) | 800 MB | 800 MB (raw) | 1.0x |
| **Total** | **1,680 MB** | **~1,015 MB** | **1.65x** |

With ZSTD compression on the values (which often have low entropy in structured problems), total size drops to approximately 600-700 MB.

### 3.2 CSC and COO Formats

CSC (Compressed Sparse Column) follows the same pattern as CSR with transposed roles. COO (Coordinate) format stores explicit (row, col, value) triples and benefits from double delta encoding (row-sorted, then column-sorted within each row group).

### 3.3 Block-Sparse Structure

For block-sparse matrices common in finite element and graph partitioning problems, the existing RVF block directory mechanism in VEC_SEG can be repurposed:
- Each dense block becomes a VEC_SEG block with its own directory entry
- Block position metadata (block row, block column) stored in META_SEG
- This leverages the existing block-level CRC32C integrity checking

### 3.4 Compatibility with Existing Serde Support

The sublinear-time-solver uses serde (bincode, rmp-serde, serde_yaml) for serialization. The integration path:

1. **bincode format** -- The existing binary format using bincode can be wrapped in a META_SEG or custom segment payload. This is the fastest migration path but loses RVF-native benefits (progressive loading, independent segment validation).

2. **Native RVF format** -- Converting sparse matrices to the proposed SPARSE_SEG layout requires custom serialization code but gains all RVF benefits. The `rvf-wire` crate provides the necessary primitives.

3. **Hybrid approach** -- Use bincode serialization inside a META_SEG for metadata and configuration, while using native RVF VEC_SEG layout for the dense value arrays. This balances migration effort with performance.

---

## 4. Binary Format Conversion Strategies

### 4.1 Bincode-to-RVF Converter

The sublinear-time-solver's bincode serialization can be converted to RVF through a streaming converter:

```rust
// Conceptual converter structure
pub struct BincodeToRvf {
    segment_id_counter: u64,
    output: Vec<u8>,
}

impl BincodeToRvf {
    /// Convert a bincode-serialized SparseMatrix to RVF segments.
    pub fn convert_sparse_matrix(&mut self, bincode_data: &[u8]) -> Result<(), Error> {
        let matrix: SparseMatrix = bincode::deserialize(bincode_data)?;

        // 1. Emit SPARSE_SEG with matrix structure
        let sparse_payload = encode_sparse_seg(&matrix);
        let seg = rvf_wire::writer::write_segment(
            0x24, // SPARSE_SEG
            &sparse_payload,
            SegmentFlags::empty(),
            self.next_segment_id(),
        );
        self.output.extend_from_slice(&seg);

        // 2. Emit META_SEG with solver-specific metadata
        let meta_json = serde_json::to_vec(&SparseMatrixMeta {
            format: matrix.format_name(),
            rows: matrix.rows(),
            cols: matrix.cols(),
            nnz: matrix.nnz(),
            solver_version: env!("CARGO_PKG_VERSION"),
        })?;
        let meta_seg = rvf_wire::writer::write_segment(
            SegmentType::Meta as u8,
            &meta_json,
            SegmentFlags::empty(),
            self.next_segment_id(),
        );
        self.output.extend_from_slice(&meta_seg);

        Ok(())
    }
}
```

### 4.2 rmp-serde (MessagePack) to RVF

MessagePack-serialized solver results can be converted similarly. The MessagePack binary representation is compact but lacks RVF's segment-level integrity and progressive loading. The converter should:

1. Deserialize the MessagePack payload using rmp-serde
2. Split the result into appropriate RVF segments (VEC_SEG for vectors, META_SEG for metadata)
3. Add WITNESS_SEG entries for computation proofs
4. Write a MANIFEST_SEG at the tail

### 4.3 serde_yaml to RVF

YAML-serialized configurations (SolverOptions, SublinearConfig) are straightforward:
- Deserialize YAML
- Re-serialize as JSON (compatible with existing RVF bridge patterns)
- Wrap in META_SEG with appropriate TLV tags

### 4.4 base64-Encoded Data

Base64-encoded binary data in the solver can be decoded and stored natively:
- Decode base64 to raw bytes
- Write directly as VEC_SEG payload (for vector data)
- This eliminates the ~33% size overhead of base64 encoding

### 4.5 Conversion Direction and Losslessness

All conversions should be bidirectional:

| Direction | Strategy | Lossless |
|-----------|----------|----------|
| bincode -> RVF | Deserialize, re-encode to RVF segments | Yes |
| RVF -> bincode | Read RVF segments, serialize via bincode | Yes |
| rmp-serde -> RVF | Deserialize, re-encode | Yes |
| RVF -> rmp-serde | Read segments, serialize via rmp-serde | Yes |
| base64 -> RVF | Decode, store raw in VEC_SEG | Yes |
| RVF -> base64 | Read VEC_SEG, encode | Yes |

---

## 5. Streaming Format Considerations

### 5.1 RVF's Native Streaming Support

RVF's append-only segment model is inherently streaming-compatible. Key properties relevant to the sublinear-time-solver:

1. **Progressive loading**: Clients can begin reading solver results before the computation completes. The PARTIAL flag on VEC_SEG segments signals that more data follows.

2. **TCP streaming protocol**: The existing rvf-server TCP protocol (`/home/user/ruvector/crates/rvf/rvf-server/src/tcp.rs`) uses length-prefixed binary framing:
   ```
   [4 bytes: payload length (big-endian)]
   [1 byte: msg_type]
   [3 bytes: msg_id]
   [payload]
   ```
   Maximum frame size: 16 MB. This protocol can carry solver segments directly.

3. **Segment-at-a-time streaming**: Each RVF segment is independently valid. A streaming solver can emit segments as they are produced:
   - SPARSE_SEG for the input matrix (once)
   - META_SEG for solver configuration (once)
   - VEC_SEG with PARTIAL+CHECKPOINT for intermediate solutions (periodic)
   - VEC_SEG for the final solution (once)
   - WITNESS_SEG for the convergence proof chain (once)
   - MANIFEST_SEG at the tail (once, after all other segments)

### 5.2 Streaming Sparse Matrix Ingest

For very large sparse matrices that do not fit in memory, streaming ingest uses multiple SPARSE_SEG segments:

```
Stream:
  SPARSE_SEG[0]: rows 0-99,999       (with PARTIAL flag)
  SPARSE_SEG[1]: rows 100,000-199,999 (with PARTIAL flag)
  ...
  SPARSE_SEG[N]: rows 900,000-999,999 (no PARTIAL flag = final)
  MANIFEST_SEG: references all SPARSE_SEGs
```

Each segment is independently verifiable via its content hash. If a network interruption occurs, only the last incomplete segment needs retransmission.

### 5.3 Iterative Solver Checkpointing via Streaming

The CHECKPOINT flag (bit 9) enables recovery from crashes during long-running solves:

```
Solve iteration 0:    VEC_SEG[PARTIAL|CHECKPOINT] + META_SEG{iter:0, residual:1e2}
Solve iteration 100:  VEC_SEG[PARTIAL|CHECKPOINT] + META_SEG{iter:100, residual:1e-1}
Solve iteration 200:  VEC_SEG[PARTIAL|CHECKPOINT] + META_SEG{iter:200, residual:1e-4}
...
Final:                VEC_SEG[SNAPSHOT] + WITNESS_SEG{chain} + MANIFEST_SEG
```

On crash recovery:
1. Tail-scan to find the latest MANIFEST_SEG
2. If no MANIFEST_SEG, scan backward for the latest CHECKPOINT
3. Resume solving from the checkpointed state

### 5.4 Inter-Agent Streaming for Distributed Solvers

For distributed sublinear solvers, RVF's streaming protocol enables:
- **Partition distribution**: Each solver node receives a SPARSE_SEG shard of the matrix
- **Partial solution exchange**: Nodes stream VEC_SEG segments containing their local solution updates
- **Consensus**: WITNESS_SEG chains prove each node's computation was valid
- **Reduction**: A coordinator assembles partial solutions into the final result

The existing `agentic-flow` adapter pattern (from `/home/user/ruvector/crates/rvf/rvf-adapters/agentic-flow/`) provides the swarm coordination layer.

### 5.5 Compression for Streaming

For streaming scenarios, per-segment compression choices should consider:

| Tier | Compression | Latency | Use Case |
|------|-------------|---------|----------|
| Hot (iterating) | None (0) | 0 ms | Current solution vector, updated every iteration |
| Warm (checkpoint) | LZ4 (1) | ~1 ms | Checkpoint snapshots, accessed on recovery |
| Cold (history) | ZSTD (2) | ~5 ms | Historical solutions, accessed rarely |

Sparse matrix structure data (row_ptr, col_idx) benefits more from compression than value arrays because the varint-delta encoding produces highly compressible byte sequences.

---

## 6. Recommended Format Bridges and Converters

### 6.1 Crate Architecture

The recommended integration consists of a new bridge crate and segment type extension:

```
crates/
  rvf/
    rvf-types/
      src/
        data_type.rs      # Add F64 = 0x09
        segment_type.rs   # Add SparseSeg = 0x24
    rvf-wire/
      src/
        sparse_seg_codec.rs  # New: CSR/CSC/COO codec
        lib.rs               # Add: pub mod sparse_seg_codec
  sublinear-solver-rvf/     # New bridge crate
    src/
      lib.rs                # Re-exports
      sparse_bridge.rs      # SparseMatrix <-> SPARSE_SEG
      dense_bridge.rs       # Matrix <-> VEC_SEG
      config_bridge.rs      # SolverOptions <-> META_SEG
      result_bridge.rs      # SolverResult <-> VEC_SEG + WITNESS_SEG
      checkpoint.rs         # PartialSolution <-> PARTIAL VEC_SEG
      witness.rs            # SolutionStep chain -> WITNESS_SEG
      stream.rs             # Streaming solver integration
    Cargo.toml              # depends on rvf-wire, rvf-types, sublinear-time-solver
```

### 6.2 Core Bridge Functions

Following the pattern established by `rvf_bridge.rs` in the domain expansion crate:

```rust
// sparse_bridge.rs -- SparseMatrix to RVF
pub fn sparse_matrix_to_segment(matrix: &SparseMatrix, segment_id: u64) -> Vec<u8>;
pub fn sparse_matrix_from_segment(data: &[u8]) -> Result<SparseMatrix, BridgeError>;

// dense_bridge.rs -- Dense Matrix to RVF VEC_SEG
pub fn dense_matrix_to_vec_seg(matrix: &Matrix, segment_id: u64) -> Vec<u8>;
pub fn dense_matrix_from_vec_seg(data: &[u8]) -> Result<Matrix, BridgeError>;

// config_bridge.rs -- Solver configuration
pub fn solver_options_to_meta_seg(opts: &SolverOptions, segment_id: u64) -> Vec<u8>;
pub fn solver_options_from_meta_seg(data: &[u8]) -> Result<SolverOptions, BridgeError>;

// result_bridge.rs -- Solver results with witness chain
pub fn solver_result_to_segments(
    result: &SolverResult,
    base_segment_id: u64,
) -> Vec<u8>;  // Returns VEC_SEG + WITNESS_SEG concatenated
pub fn solver_result_from_segments(data: &[u8]) -> Result<SolverResult, BridgeError>;

// checkpoint.rs -- Streaming checkpoints
pub fn checkpoint_to_segment(
    partial: &PartialSolution,
    segment_id: u64,
) -> Vec<u8>;  // VEC_SEG with PARTIAL|CHECKPOINT flags
pub fn checkpoint_from_segment(data: &[u8]) -> Result<PartialSolution, BridgeError>;

// witness.rs -- Solution step witness chain
pub fn build_solver_witness_chain(
    steps: &[SolutionStep],
) -> Vec<u8>;  // SHAKE-256 witness chain bytes
```

### 6.3 SPARSE_SEG Codec Implementation

The sparse segment codec should follow the RVF codec pattern (64-byte alignment, content hashing, varint encoding):

```rust
// sparse_seg_codec.rs

/// Sparse matrix format identifier.
#[repr(u8)]
pub enum SparseFormat {
    CSR = 0,
    CSC = 1,
    COO = 2,
}

/// Sparse segment header (padded to 64 bytes).
#[repr(C)]
pub struct SparseHeader {
    pub format: u8,       // SparseFormat
    pub dtype: u8,        // DataType (0x00=f32, 0x09=f64)
    pub reserved: [u8; 6],
    pub rows: u64,
    pub cols: u64,
    pub nnz: u64,
    pub padding: [u8; 32],
}

/// Write a CSR sparse matrix as a SPARSE_SEG payload.
pub fn write_csr_seg(
    rows: u64,
    cols: u64,
    row_ptr: &[u64],
    col_idx: &[u64],
    values: &[f64],
) -> Vec<u8> {
    let mut buf = Vec::new();

    // Header (64 bytes)
    // ... write SparseHeader fields ...

    // row_ptr: delta-varint encoded (monotonically increasing)
    let mut row_ptr_buf = Vec::new();
    encode_delta(row_ptr, 128, &mut row_ptr_buf);
    // length prefix for row_ptr section
    buf.extend_from_slice(&(row_ptr_buf.len() as u32).to_le_bytes());
    buf.extend_from_slice(&row_ptr_buf);
    // pad to 64B

    // col_idx: delta-varint encoded per row group
    // ... similar pattern ...

    // values: raw f64 little-endian, 64B aligned
    for &v in values {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    buf
}
```

### 6.4 f64 DataType Extension

Add to `/home/user/ruvector/crates/rvf/rvf-types/src/data_type.rs`:

```rust
/// 64-bit IEEE 754 double-precision float.
F64 = 9,
```

And update `bits_per_element()`:
```rust
Self::F64 => Some(64),
```

Update `dtype_element_size()` in both `vec_seg_codec.rs` and `hot_seg_codec.rs`:
```rust
0x09 => 8, // f64
```

### 6.5 WASM Integration Path

Following the `rvf-solver-wasm` pattern (ADR-039), the sublinear-time-solver can be compiled to WASM:

1. **no_std + alloc** build target matching `rvf-solver-wasm`
2. **C ABI exports** for solver lifecycle: `create`, `load_matrix`, `solve`, `read_result`, `read_witness`
3. **Handle-based API** (up to 8 concurrent solver instances, same as rvf-solver-wasm)
4. **Witness chain integration** via `rvf-crypto::create_witness_chain()`

### 6.6 Segment Forward Compatibility

Per ADR-029's segment forward compatibility rule: "RVF readers and rewriters MUST skip segment types they do not recognize and MUST preserve them byte-for-byte on rewrite." This means:

- Adding SPARSE_SEG (0x24) is safe: existing RVF tools will skip it
- Existing RVF compaction will preserve SPARSE_SEG segments unchanged
- Older tools that encounter SPARSE_SEG in an RVF file will not corrupt it

### 6.7 Migration Tooling

Following the pattern of `rvf-import` (`/home/user/ruvector/crates/rvf/rvf-import/`) which handles CSV, JSON, and NumPy imports:

```rust
// New import module in rvf-import or sublinear-solver-rvf
pub fn import_matrix_market(path: &Path) -> Result<Vec<u8>, ImportError>;
pub fn import_scipy_sparse(path: &Path) -> Result<Vec<u8>, ImportError>;
pub fn import_bincode_solver(path: &Path) -> Result<Vec<u8>, ImportError>;
```

### 6.8 Performance Targets

Based on RVF's acceptance test benchmarks (ADR-029):

| Operation | Target | Notes |
|-----------|--------|-------|
| Sparse matrix cold load | <50 ms | Tail-scan + manifest parse + structure load |
| Solver result first read | <5 ms | 4 KB manifest read |
| Checkpoint write | <1 ms | Single VEC_SEG + fsync |
| Streaming ingest rate | 100K+ rows/s | Append-only, no rewrite |
| WASM sparse solve | <10x native | Matches rvf-solver-wasm overhead |

---

## Summary of Key Files Analyzed

| File Path | Relevance |
|-----------|-----------|
| `/home/user/ruvector/docs/adr/ADR-029-rvf-canonical-format.md` | Canonical format adoption decision, segment type registry |
| `/home/user/ruvector/docs/research/rvf/wire/binary-layout.md` | Complete wire format specification |
| `/home/user/ruvector/docs/research/rvf/spec/00-overview.md` | Design philosophy and four laws |
| `/home/user/ruvector/docs/research/rvf/spec/01-segment-model.md` | Segment lifecycle, write/read paths |
| `/home/user/ruvector/docs/research/rvf/spec/06-query-optimization.md` | SIMD alignment, prefetch, columnar layout |
| `/home/user/ruvector/crates/rvf/rvf-types/src/segment.rs` | 64-byte SegmentHeader struct (repr(C)) |
| `/home/user/ruvector/crates/rvf/rvf-types/src/segment_type.rs` | 23-variant segment type enum |
| `/home/user/ruvector/crates/rvf/rvf-types/src/data_type.rs` | 9-variant data type enum (needs f64 extension) |
| `/home/user/ruvector/crates/rvf/rvf-types/src/flags.rs` | 12-bit segment flags bitfield |
| `/home/user/ruvector/crates/rvf/rvf-types/src/constants.rs` | Magic numbers, alignment, size limits |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/lib.rs` | Wire format crate structure |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/writer.rs` | Segment writer with XXH3-128 hashing |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/reader.rs` | Segment reader with validation |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/varint.rs` | LEB128 varint codec |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/delta.rs` | Delta encoding with restart points |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/vec_seg_codec.rs` | VEC_SEG block directory and columnar codec |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/index_seg_codec.rs` | INDEX_SEG HNSW adjacency codec |
| `/home/user/ruvector/crates/rvf/rvf-wire/src/hot_seg_codec.rs` | HOT_SEG interleaved codec |
| `/home/user/ruvector/crates/rvf/rvf-quant/src/codec.rs` | Quantization and sketch codecs |
| `/home/user/ruvector/crates/rvf/rvf-server/src/tcp.rs` | TCP streaming protocol |
| `/home/user/ruvector/crates/rvf/rvf-solver-wasm/src/lib.rs` | WASM solver integration pattern |
| `/home/user/ruvector/crates/ruvector-domain-expansion/src/rvf_bridge.rs` | Bridge pattern reference implementation |
| `/home/user/ruvector/docs/adr/ADR-039-rvf-solver-wasm-agi-integration.md` | WASM solver integration architecture |
