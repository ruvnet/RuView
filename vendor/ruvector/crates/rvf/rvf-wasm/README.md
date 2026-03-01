# rvf-wasm

WASM microkernel and control plane for running RuVector Format operations in the browser and at the edge.

## Overview

`rvf-wasm` compiles the core RVF runtime to WebAssembly for use in browsers, Cloudflare Workers, and other WASM environments:

- **Compact binary** -- optimized with `opt-level = "z"` and LTO
- **No-std compatible** -- runs with a lightweight `dlmalloc` allocator
- **Browser-ready** -- works with `wasm-bindgen` or standalone instantiation
- **Full control plane** -- create, query, modify, and export `.rvf` stores entirely in-memory

## Build

```bash
cargo build --target wasm32-unknown-unknown --release -p rvf-wasm
```

## API

### Tile Compute (14 exports)

Low-level distance computation, quantization, and HNSW navigation for the Cognitum tile architecture:

| Export | Description |
|--------|-------------|
| `rvf_init` | Initialize tile with configuration |
| `rvf_load_query` | Load query vector into scratch |
| `rvf_load_block` | Load vector block into SIMD scratch |
| `rvf_distances` | Compute distances (L2, IP, cosine, hamming) |
| `rvf_topk_merge` | Merge distances into top-K heap |
| `rvf_topk_read` | Read sorted top-K results |
| `rvf_load_sq_params` | Load scalar quantization parameters |
| `rvf_dequant_i8` | Dequantize int8 to fp16 |
| `rvf_load_pq_codebook` | Load PQ codebook for asymmetric distance |
| `rvf_pq_distances` | Compute PQ distances |
| `rvf_load_neighbors` | Load HNSW neighbor list |
| `rvf_greedy_step` | HNSW greedy search step |
| `rvf_verify_header` | Verify segment header magic/version |
| `rvf_crc32c` | Compute CRC32C checksum |

### Control Plane (10 exports)

In-memory store operations for creating and querying `.rvf` data without filesystem access:

| Export | Description |
|--------|-------------|
| `rvf_store_create(dim, metric) -> handle` | Create in-memory store |
| `rvf_store_open(buf_ptr, buf_len) -> handle` | Parse `.rvf` bytes into queryable store |
| `rvf_store_ingest(handle, vecs, ids, count)` | Add vectors |
| `rvf_store_query(handle, query, k, metric, out)` | k-NN search |
| `rvf_store_delete(handle, ids, count)` | Soft-delete by ID |
| `rvf_store_count(handle)` | Live vector count |
| `rvf_store_dimension(handle)` | Get dimensionality |
| `rvf_store_status(handle, out)` | Write 20-byte status |
| `rvf_store_export(handle, out, len)` | Serialize to `.rvf` bytes |
| `rvf_store_close(handle)` | Free store |

### Segment Inspection (4 exports)

Parse and inspect `.rvf` file structure from raw bytes:

| Export | Description |
|--------|-------------|
| `rvf_parse_header(buf, len, out)` | Parse a 64-byte segment header |
| `rvf_segment_count(buf, len)` | Count segments in buffer |
| `rvf_segment_info(buf, len, idx, out)` | Get segment details by index |
| `rvf_verify_checksum(buf, len)` | Verify CRC32C integrity |

### Witness Chain Verification (2 exports)

Verify SHAKE-256 witness chains from WITNESS_SEG payloads:

| Export | Description |
|--------|-------------|
| `rvf_witness_verify(chain_ptr, chain_len) -> i32` | Verify full chain integrity; returns entry count or negative error (-2 = truncated, -3 = hash mismatch) |
| `rvf_witness_count(chain_len) -> i32` | Count entries without full verification (chain_len / 73) |

These exports enable browser-side verification of acceptance test artifacts and audit trails without any backend. See [ADR-037](../../docs/adr/ADR-037-publishable-rvf-acceptance-test.md).

### Memory Management (2 exports)

| Export | Description |
|--------|-------------|
| `rvf_alloc(size) -> ptr` | Allocate memory for JS interop |
| `rvf_free(ptr, size)` | Free allocated memory |

## Usage from JavaScript

```javascript
const wasm = await WebAssembly.instantiate(wasmBytes);
const { rvf_store_create, rvf_store_ingest, rvf_store_query,
        rvf_store_export, rvf_store_close, rvf_alloc, rvf_free } = wasm.instance.exports;

// Create a 4-dimensional store with L2 metric
const handle = rvf_store_create(4, 0);

// Allocate and copy vectors into WASM memory
const vecs = new Float32Array([1,0,0,0, 0,1,0,0, 0,0,1,0]);
const ids = new BigUint64Array([0n, 1n, 2n]);
const vecPtr = rvf_alloc(vecs.byteLength);
const idPtr = rvf_alloc(ids.byteLength);
new Uint8Array(wasm.instance.exports.memory.buffer, vecPtr, vecs.byteLength)
  .set(new Uint8Array(vecs.buffer));
new Uint8Array(wasm.instance.exports.memory.buffer, idPtr, ids.byteLength)
  .set(new Uint8Array(ids.buffer));

// Ingest
rvf_store_ingest(handle, vecPtr, idPtr, 3);
rvf_free(vecPtr, vecs.byteLength);
rvf_free(idPtr, ids.byteLength);

// Query
const queryVec = new Float32Array([1,0,0,0]);
const qPtr = rvf_alloc(16);
new Uint8Array(wasm.instance.exports.memory.buffer, qPtr, 16)
  .set(new Uint8Array(queryVec.buffer));
const outPtr = rvf_alloc(12 * 3); // 3 results * (8 bytes id + 4 bytes dist)
const count = rvf_store_query(handle, qPtr, 3, 0, outPtr);

// Export to .rvf bytes
const exportBuf = rvf_alloc(65536);
const written = rvf_store_export(handle, exportBuf, 65536);
const rvfBytes = new Uint8Array(wasm.instance.exports.memory.buffer, exportBuf, written);

rvf_store_close(handle);
```

## License

MIT OR Apache-2.0
