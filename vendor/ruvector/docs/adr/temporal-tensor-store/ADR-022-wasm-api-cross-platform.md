# ADR-022: WASM API Surface and Cross-Platform Strategy

**Status**: Proposed
**Date**: 2026-02-08
**Parent**: ADR-017 Temporal Tensor Compression, ADR-005 WASM Runtime Integration, ADR-018 Block-Based Storage Engine
**Author**: System Architecture Team

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-08 | Architecture Team | Initial proposal |

---

## Abstract

This ADR defines the **WASM API surface** for the Temporal Tensor Store (TTS),
enabling the tiering gate and quantizer to be called from **Node.js** and
**browser** environments with identical semantics. The design extends the
frame-level `ttc_*` FFI established in ADR-017 with a new block-level `tts_*`
function set, introduces host-imported IO functions for pluggable storage
backends, and specifies the cross-platform binding strategy for native, Node.js,
browser, and edge targets.

The API surface is intentionally narrow -- five core exports, three host imports,
and two memory-management helpers -- to minimise the attack surface exposed
across the WASM boundary while remaining sufficient for full tiered tensor
storage operations.

---

## 1. Context and Motivation

### 1.1 The Cross-Platform Imperative

ADR-017 established a Rust-native temporal tensor compressor with a WASM FFI
layer (`ttc_*` functions) for frame-level compression. ADR-005 established the
WASM sandboxing model with epoch-based interruption and raw ABI. ADR-018
defined the block-based storage engine with tiered placement.

However, these designs assume the storage backend is directly accessible from
within the WASM module. In practice:

- **Node.js**: Storage lives in AgentDB/RuVector file-backed databases that
  the WASM module cannot access directly via filesystem calls.
- **Browser**: Persistent storage requires IndexedDB, which is asynchronous and
  unavailable from within WASM linear memory.
- **Edge/Embedded**: Storage may be in-memory only, with no filesystem at all.

The WASM module must delegate all IO to the **host** via imported functions,
while retaining ownership of the tiering policy, quantization logic, and
block management.

### 1.2 tensor_id Splitting Problem

WASM's value types are limited to `i32`, `i64`, `f32`, and `f64`. The Temporal
Tensor Store uses `u128` tensor identifiers internally, but `u128` cannot cross
the WASM FFI boundary as a single value. The standard solution is to split the
identifier into two `u64` halves (`hi` and `lo`), which the host reconstructs
on its side.

### 1.3 Design Goals

| Goal | Rationale |
|------|-----------|
| Narrow API surface (< 10 exports) | Minimise WASM boundary complexity and audit scope |
| Host-delegated IO | Enable platform-specific storage without WASM recompilation |
| Zero-copy where possible | Avoid redundant copies across the WASM boundary |
| Identical semantics across platforms | Same WASM binary runs on Node.js, browser, and edge |
| Coexistence with ADR-017 `ttc_*` | Both function sets share the same WASM module |

---

## 2. Decision

### 2.1 Introduce `tts_*` WASM Exports for Block-Level Storage

We extend the WASM module with five core export functions and two memory
management helpers, all using `extern "C"` linkage with `#[no_mangle]`:

```c
// Initialize the store with a JSON-encoded policy configuration.
// Returns 0 on success, negative error code on failure.
int32_t tts_init(const uint8_t* policy_ptr, usize policy_len) -> i32;

// Ingest a tensor block. The tensor_id is split into hi/lo halves.
// data_ptr points to f32 values in WASM linear memory.
// Returns 0 on success, negative error code on failure.
int32_t tts_put(uint64_t tensor_id_hi, uint64_t tensor_id_lo,
                uint32_t block_index,
                const float* data_ptr, usize data_len) -> i32;

// Read a tensor block, dequantized back to f32.
// out_ptr is a pre-allocated buffer in WASM linear memory.
// Returns 0 on success, negative error code on failure.
int32_t tts_get(uint64_t tensor_id_hi, uint64_t tensor_id_lo,
                uint32_t block_index,
                float* out_ptr, usize out_len) -> i32;

// Run a maintenance tick: promote/demote blocks, evict to meet budgets.
// budget_bytes: maximum bytes to write during this tick.
// budget_ops: maximum IO operations during this tick.
// Returns number of blocks moved, or negative error code.
int32_t tts_tick(uint32_t budget_bytes, uint32_t budget_ops) -> i32;

// Write a JSON-encoded statistics snapshot into out_ptr.
// Returns bytes written, or negative error code if buffer too small.
int32_t tts_stats(uint8_t* out_ptr, usize out_len) -> i32;
```

### 2.2 Host-Imported IO Functions

The WASM module imports three functions from the host environment for all
persistent IO. These are declared in the `"tts_host"` import namespace:

```c
// Read a block from host storage into dst buffer.
// tier: 0=hot, 1=warm, 2=cold
// key_ptr/key_len: block key (tensor_id:block_index encoded as bytes)
// dst_ptr/dst_len: destination buffer in WASM linear memory
// Returns bytes read, or negative error code.
int32_t read_block(uint32_t tier, const uint8_t* key_ptr, usize key_len,
                   uint8_t* dst_ptr, usize dst_len) -> i32;

// Write a block to host storage from src buffer.
// Returns 0 on success, negative error code on failure.
int32_t write_block(uint32_t tier, const uint8_t* key_ptr, usize key_len,
                    const uint8_t* src_ptr, usize src_len) -> i32;

// Delete a block from host storage.
// Returns 0 on success, negative error code on failure.
int32_t delete_block(uint32_t tier, const uint8_t* key_ptr, usize key_len) -> i32;
```

**Platform-specific host bindings:**

| Platform | `read_block` | `write_block` | `delete_block` |
|----------|-------------|--------------|----------------|
| Node.js | AgentDB get | AgentDB put | AgentDB delete |
| Browser | IndexedDB getAll | IndexedDB put | IndexedDB delete |
| Native (server) | mmap read | mmap write | unlink |
| Edge/Embedded | ArrayBuffer slice | ArrayBuffer copy | zeroed/freed |

### 2.3 Memory Management Exports

```c
// Allocate len bytes in WASM linear memory.
// Returns pointer to allocated region, or 0 on failure.
uint32_t tts_alloc(usize len) -> u32;

// Deallocate a previously allocated region.
void tts_dealloc(uint32_t ptr, usize len);

// Retrieve the last error message as a UTF-8 string.
// Returns bytes written, or negative if buffer too small.
int32_t tts_last_error(uint8_t* out_ptr, usize out_len) -> i32;
```

---

## 3. Detailed Design

### 3.1 WASM Memory Layout

```
+========================================================================+
|                        WASM Linear Memory                              |
|========================================================================|
|                                                                        |
|  0x0000 +-----------------+                                            |
|         | WASM Stack      |  (grows downward, managed by WASM runtime) |
|         +-----------------+                                            |
|         | Static Data     |  (STORE, policy config, error buffer)      |
|         +-----------------+                                            |
|         |                 |                                            |
|         | Heap            |  (managed by tts_alloc / tts_dealloc)      |
|         |                 |                                            |
|         | +-------------+ |                                            |
|         | | Input Buffer| |  Host writes f32 frames here               |
|         | | (f32[N])    | |  via tts_alloc -> memcpy -> tts_put        |
|         | +-------------+ |                                            |
|         |                 |                                            |
|         | +-------------+ |                                            |
|         | | Output Buf  | |  tts_get writes dequantized f32 here       |
|         | | (f32[N])    | |  Host reads after tts_get returns           |
|         | +-------------+ |                                            |
|         |                 |                                            |
|         | +-------------+ |                                            |
|         | | IO Staging  | |  Temporary buffer for host import calls    |
|         | | Buffer      | |  (read_block / write_block payloads)       |
|         | +-------------+ |                                            |
|         |                 |                                            |
|  0xFFFF +-----------------+  (grows via memory.grow as needed)         |
|                                                                        |
+========================================================================+
```

### 3.2 Host-Guest Interaction Pattern

```
  HOST (Node.js / Browser / Native)              GUEST (WASM Module)
  ====================================           ========================

  1. Load WASM module
  2. Provide host imports:
     - tts_host::read_block
     - tts_host::write_block
     - tts_host::delete_block
  3. Instantiate module
                                                  |
  4. Encode policy as JSON bytes          ------->|
  5. ptr = tts_alloc(policy_len)                  | allocate in linear mem
  6. Write policy bytes to ptr                    |
  7. tts_init(ptr, policy_len)            ------->| parse policy, init STORE
  8. tts_dealloc(ptr, policy_len)                 | free policy buffer
                                                  |
  --- INGEST LOOP ---                             |
                                                  |
  9.  buf = tts_alloc(N * 4)                      | allocate f32 buffer
  10. Write f32 data into buf                     |
  11. tts_put(id_hi, id_lo, idx,          ------->| quantize frame
              buf, N)                             | tier policy selects bits
                                                  | calls write_block(tier,
                                                  |   key, compressed)
                                          <-------| write_block import
  12. Host persists block                         |
                                          ------->| returns 0 (success)
  13. tts_dealloc(buf, N * 4)                     |
                                                  |
  --- READ LOOP ---                               |
                                                  |
  14. out = tts_alloc(N * 4)                      | allocate output buffer
  15. tts_get(id_hi, id_lo, idx,          ------->| calls read_block(tier,
              out, N)                             |   key, staging_buf)
                                          <-------| read_block import
  16. Host reads from storage,                    |
      writes into staging_buf                     |
                                          ------->| dequantize into out
  17. Host reads f32 from out                     |
  18. tts_dealloc(out, N * 4)                     |
                                                  |
  --- MAINTENANCE ---                             |
                                                  |
  19. tts_tick(budget_bytes,              ------->| evaluate tier scores
              budget_ops)                         | promote/demote blocks
                                                  | calls write_block,
                                                  |   delete_block as needed
                                          <-------| host import callbacks
  20. Host handles IO                             |
                                          ------->| returns blocks_moved
```

### 3.3 Import/Export Function Table

**Exports (WASM -> Host):**

| Export | Signature (WASM types) | Description |
|--------|----------------------|-------------|
| `tts_init` | `(i32, i32) -> i32` | Init store with policy JSON |
| `tts_put` | `(i64, i64, i32, i32, i32) -> i32` | Ingest tensor block |
| `tts_get` | `(i64, i64, i32, i32, i32) -> i32` | Read tensor block |
| `tts_tick` | `(i32, i32) -> i32` | Maintenance tick |
| `tts_stats` | `(i32, i32) -> i32` | Statistics snapshot |
| `tts_alloc` | `(i32) -> i32` | Allocate linear memory |
| `tts_dealloc` | `(i32, i32) -> ()` | Free linear memory |
| `tts_last_error` | `(i32, i32) -> i32` | Get error message |

**Imports (Host -> WASM), namespace `tts_host`:**

| Import | Signature (WASM types) | Description |
|--------|----------------------|-------------|
| `read_block` | `(i32, i32, i32, i32, i32) -> i32` | Read from host storage |
| `write_block` | `(i32, i32, i32, i32, i32) -> i32` | Write to host storage |
| `delete_block` | `(i32, i32, i32) -> i32` | Delete from host storage |

### 3.4 tensor_id Encoding

```
u128 tensor_id:
+----------------------------------+----------------------------------+
|           hi (u64)               |           lo (u64)               |
| bits [127..64]                   | bits [63..0]                     |
+----------------------------------+----------------------------------+

Reconstruction (host side):
  tensor_id = (hi as u128) << 64 | (lo as u128)

Block key encoding (for host import calls):
  key = tensor_id_hi.to_le_bytes() ++ tensor_id_lo.to_le_bytes() ++ block_index.to_le_bytes()
  key_len = 8 + 8 + 4 = 20 bytes
```

This encoding is deterministic and platform-independent (little-endian).

### 3.5 Error Handling

**Return code convention:**

| Code | Name | Description |
|------|------|-------------|
| 0 | `TTS_OK` | Operation succeeded |
| -1 | `TTS_ERR_INVALID_HANDLE` | Store not initialized or handle invalid |
| -2 | `TTS_ERR_TENSOR_EVICTED` | Requested block was evicted from all tiers |
| -3 | `TTS_ERR_BUDGET_EXHAUSTED` | Tick budget fully consumed |
| -4 | `TTS_ERR_IO` | Host IO import returned an error |
| -5 | `TTS_ERR_CORRUPT_BLOCK` | Block data failed integrity check |
| -6 | `TTS_ERR_BUFFER_TOO_SMALL` | Output buffer insufficient |
| -7 | `TTS_ERR_INVALID_POLICY` | Policy JSON failed validation |
| -8 | `TTS_ERR_NULL_POINTER` | Null pointer passed for required argument |
| -9 | `TTS_ERR_ALLOC_FAILED` | Memory allocation failed |

**Error message retrieval:**

```rust
// Guest-side implementation
static mut LAST_ERROR: [u8; 256] = [0u8; 256];
static mut LAST_ERROR_LEN: usize = 0;

fn set_error(msg: &str) {
    unsafe {
        let bytes = msg.as_bytes();
        let len = bytes.len().min(256);
        LAST_ERROR[..len].copy_from_slice(&bytes[..len]);
        LAST_ERROR_LEN = len;
    }
}

#[no_mangle]
pub extern "C" fn tts_last_error(out_ptr: *mut u8, out_len: usize) -> i32 {
    if out_ptr.is_null() {
        return TTS_ERR_NULL_POINTER;
    }
    unsafe {
        let copy_len = LAST_ERROR_LEN.min(out_len);
        core::ptr::copy_nonoverlapping(LAST_ERROR.as_ptr(), out_ptr, copy_len);
        copy_len as i32
    }
}
```

### 3.6 Memory Model Details

The WASM module uses linear memory exclusively. The host interacts with this
memory through the exported `tts_alloc` and `tts_dealloc` functions:

```rust
// Guest-side allocator (simple bump allocator for WASM)
#[no_mangle]
pub extern "C" fn tts_alloc(len: usize) -> u32 {
    let layout = core::alloc::Layout::from_size_align(len, 4);
    match layout {
        Ok(layout) => {
            let ptr = unsafe { alloc::alloc::alloc(layout) };
            if ptr.is_null() {
                set_error("allocation failed");
                0
            } else {
                ptr as u32
            }
        }
        Err(_) => {
            set_error("invalid allocation layout");
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn tts_dealloc(ptr: u32, len: usize) {
    if ptr == 0 || len == 0 {
        return;
    }
    let layout = core::alloc::Layout::from_size_align(len, 4);
    if let Ok(layout) = layout {
        unsafe { alloc::alloc::dealloc(ptr as *mut u8, layout); }
    }
}
```

**Lifecycle protocol:**

1. Host calls `tts_alloc(N)` to get a pointer in WASM linear memory.
2. Host writes data into that pointer region (via `memory.buffer` in JS).
3. Host calls `tts_put(...)` or `tts_init(...)` with the pointer.
4. Host calls `tts_dealloc(ptr, N)` to free the buffer.
5. For reads: host allocates output buffer, calls `tts_get(...)`, reads result,
   then deallocates.

---

## 4. Cross-Platform Strategy

### 4.1 Platform Binding Matrix

| Platform | BlockIO Binding | MetaLog Binding | Async Model | Notes |
|----------|----------------|-----------------|-------------|-------|
| Native (server) | Memory-mapped files per tier | Append-only file | Sync | mmap for zero-copy reads; direct filesystem access |
| Node.js (WASM) | AgentDB / RuVector | AgentDB | Sync wrapper over async | Host imports bridge WASM to AgentDB API |
| Browser (WASM) | IndexedDB | IndexedDB | Async wrapper needed | Requires Atomics.wait or promise-based shim |
| Edge / Embedded | In-memory buffers | In-memory ring | Sync | No persistence; eviction on budget pressure |

### 4.2 Node.js Binding Architecture

```
+------------------------------------------------------------------+
|                        Node.js Process                           |
|                                                                  |
|  +------------------+          +-----------------------------+   |
|  | TypeScript API   |          | WASM Instance               |   |
|  |                  |  alloc   |                             |   |
|  | tts.init(policy) |--------->| tts_init(ptr, len)          |   |
|  | tts.put(id, blk, |--------->| tts_put(hi, lo, idx,        |   |
|  |         data)    |          |         ptr, len)           |   |
|  | tts.get(id, blk) |--------->| tts_get(hi, lo, idx,        |   |
|  | tts.tick(budget) |--------->|         ptr, len)           |   |
|  | tts.stats()      |          | tts_tick(bytes, ops)        |   |
|  +------------------+          | tts_stats(ptr, len)         |   |
|         ^                      +----------+------------------+   |
|         |                                 |                      |
|         |                      host imports|                     |
|         |                                 v                      |
|  +------+------+              +-----------+-----------+          |
|  | AgentDB     |<-------------| tts_host::read_block  |          |
|  | (storage)   |<-------------| tts_host::write_block |          |
|  |             |<-------------| tts_host::delete_block|          |
|  +-------------+              +-----------------------+          |
+------------------------------------------------------------------+
```

### 4.3 Browser Binding Architecture

In the browser, IndexedDB is asynchronous. The host imports must bridge this
gap. Two strategies are available:

**Strategy A: SharedArrayBuffer + Atomics (preferred for performance)**

The host import writes to a shared buffer and signals completion via
`Atomics.notify`. The WASM thread (running in a Web Worker) waits via
`Atomics.wait`. This provides synchronous semantics from the WASM perspective.

**Strategy B: Asyncify (fallback)**

For browsers without SharedArrayBuffer support, the Asyncify transform
(applied at WASM compile time via `wasm-opt --asyncify`) enables the WASM
module to yield execution and resume after the host completes an async
IndexedDB operation.

| Strategy | Latency | Compatibility | Complexity |
|----------|---------|---------------|------------|
| SharedArrayBuffer + Atomics | ~1ms per IO | Requires COOP/COEP headers | Moderate |
| Asyncify | ~2-5ms per IO | Universal | Higher (binary transform) |

### 4.4 Edge/Embedded Strategy

For edge and embedded deployments, all storage is in-memory:

- `read_block`: Returns data from a pre-allocated `ArrayBuffer` or `Vec<u8>`.
- `write_block`: Copies data into the in-memory store.
- `delete_block`: Zeros or frees the slot.
- No persistence. The `tts_tick` maintenance function handles eviction when
  memory budget is exceeded.
- The in-memory ring for MetaLog provides bounded audit logging with automatic
  overwrite of oldest entries.

---

## 5. Integration with ADR-017 WASM FFI

### 5.1 Coexistence of `ttc_*` and `tts_*`

ADR-017 defined frame-level compression functions (`ttc_create`, `ttc_push_frame`,
`ttc_flush`, `ttc_decode_segment`, etc.). ADR-022 introduces block-level storage
functions (`tts_init`, `tts_put`, `tts_get`, `tts_tick`, `tts_stats`).

Both function sets coexist in the same WASM module:

```
WASM Module Exports
===================================================
 ADR-017 (frame-level compression)    ADR-022 (block-level storage)
 ----------------------------------   ----------------------------
 ttc_create                           tts_init
 ttc_free                             tts_put
 ttc_touch                            tts_get
 ttc_set_access                       tts_tick
 ttc_push_frame                       tts_stats
 ttc_flush                            tts_alloc
 ttc_decode_segment                   tts_dealloc
 ttc_alloc                            tts_last_error
 ttc_dealloc
===================================================
```

**Shared allocator**: `tts_alloc` and `ttc_alloc` use the same underlying
allocator. If both are present, either can be called; they are aliases.

**Layering**: `tts_put` internally invokes the `ttc_*` quantization pipeline
to compress the ingested f32 data before passing compressed blocks to the host
via `write_block`. `tts_get` reads compressed blocks via `read_block` and
invokes `ttc_decode_segment` to dequantize before writing f32 to the output
buffer.

### 5.2 Shared State

```rust
// Single-threaded WASM: static mut is sound
static mut STORE: Option<TemporalTensorStore> = None;

// The store holds:
// - TierPolicy (from tts_init config)
// - Block metadata index (tensor_id -> block_index -> tier, size, access stats)
// - Active compressor handles (reusing ttc_* compressor pool from ADR-017)
// - IO staging buffer (reused across calls to avoid repeated allocation)
```

---

## 6. TypeScript Type Definitions

The following types define the Node.js binding surface:

```typescript
/** 128-bit tensor identifier, split for WASM compatibility. */
export interface TensorId {
  /** Upper 64 bits of the tensor ID. */
  readonly hi: bigint;
  /** Lower 64 bits of the tensor ID. */
  readonly lo: bigint;
}

/** Policy configuration for the Temporal Tensor Store. */
export interface TtsPolicy {
  /** Minimum score for hot tier placement (default: 512). */
  hot_min_score?: number;
  /** Minimum score for warm tier placement (default: 64). */
  warm_min_score?: number;
  /** Bit width for warm tier: 5 or 7 (default: 7). */
  warm_bits?: 5 | 7;
  /** Drift tolerance as Q8 fixed-point: 26 = ~10% (default: 26). */
  drift_pct_q8?: number;
  /** Elements per quantization group (default: 64). */
  group_len?: number;
  /** Maximum bytes across all tiers before eviction. */
  max_total_bytes?: number;
}

/** Statistics snapshot returned by tts.stats(). */
export interface TtsStats {
  /** Number of tensor blocks in each tier. */
  blocks_by_tier: { hot: number; warm: number; cold: number };
  /** Total bytes stored in each tier. */
  bytes_by_tier: { hot: number; warm: number; cold: number };
  /** Total number of unique tensor IDs tracked. */
  tensor_count: number;
  /** Number of blocks promoted in the last tick. */
  last_tick_promotions: number;
  /** Number of blocks demoted in the last tick. */
  last_tick_demotions: number;
  /** Number of blocks evicted in the last tick. */
  last_tick_evictions: number;
}

/** Budget parameters for a maintenance tick. */
export interface TtsTickBudget {
  /** Maximum bytes to write during this tick. */
  bytes: number;
  /** Maximum IO operations during this tick. */
  ops: number;
}

/** Result of a maintenance tick. */
export interface TtsTickResult {
  /** Number of blocks moved (promoted + demoted + evicted). */
  blocks_moved: number;
}

/** Error codes returned by tts_* functions. */
export const enum TtsError {
  OK = 0,
  INVALID_HANDLE = -1,
  TENSOR_EVICTED = -2,
  BUDGET_EXHAUSTED = -3,
  IO_ERROR = -4,
  CORRUPT_BLOCK = -5,
  BUFFER_TOO_SMALL = -6,
  INVALID_POLICY = -7,
  NULL_POINTER = -8,
  ALLOC_FAILED = -9,
}

/** Host IO interface that platform bindings must implement. */
export interface TtsHostIO {
  /** Read a block from storage. Returns the block bytes. */
  readBlock(tier: number, key: Uint8Array): Uint8Array | null;
  /** Write a block to storage. */
  writeBlock(tier: number, key: Uint8Array, data: Uint8Array): void;
  /** Delete a block from storage. */
  deleteBlock(tier: number, key: Uint8Array): void;
}

/**
 * High-level TypeScript wrapper around the TTS WASM module.
 *
 * Usage:
 *   const tts = await TtsStore.create(wasmBytes, hostIO, policy);
 *   tts.put(tensorId, blockIndex, float32Data);
 *   const data = tts.get(tensorId, blockIndex);
 *   const moved = tts.tick({ bytes: 1048576, ops: 100 });
 *   const stats = tts.stats();
 *   tts.dispose();
 */
export declare class TtsStore {
  /**
   * Instantiate the WASM module and initialize the store.
   * @param wasmBytes - Compiled WASM module bytes.
   * @param hostIO - Platform-specific IO implementation.
   * @param policy - Tiering policy configuration.
   */
  static create(
    wasmBytes: ArrayBuffer,
    hostIO: TtsHostIO,
    policy?: TtsPolicy,
  ): Promise<TtsStore>;

  /**
   * Ingest a tensor block.
   * @param id - 128-bit tensor identifier (split into hi/lo).
   * @param blockIndex - Block index within the tensor.
   * @param data - Float32 data to store.
   * @throws TtsStoreError on failure.
   */
  put(id: TensorId, blockIndex: number, data: Float32Array): void;

  /**
   * Read a tensor block, dequantized to f32.
   * @param id - 128-bit tensor identifier.
   * @param blockIndex - Block index within the tensor.
   * @returns Dequantized Float32Array.
   * @throws TtsStoreError if block was evicted or corrupted.
   */
  get(id: TensorId, blockIndex: number): Float32Array;

  /**
   * Run a maintenance tick to promote, demote, or evict blocks.
   * @param budget - IO budget for this tick.
   * @returns Number of blocks moved.
   */
  tick(budget: TtsTickBudget): TtsTickResult;

  /** Get a statistics snapshot. */
  stats(): TtsStats;

  /** Release all WASM resources. */
  dispose(): void;
}
```

---

## 7. Safety Considerations

### 7.1 Static Mutable State

```rust
// WASM (single-threaded): sound, no data races possible
static mut STORE: Option<TemporalTensorStore> = None;

// Native targets: MUST use thread-safe alternatives
#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static STORE: RefCell<Option<TemporalTensorStore>> = RefCell::new(None);
}

// Or for shared-state native:
#[cfg(not(target_arch = "wasm32"))]
static STORE: once_cell::sync::Lazy<Mutex<Option<TemporalTensorStore>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));
```

### 7.2 Pointer Validation

All exported functions validate pointers before use:

```rust
#[no_mangle]
pub extern "C" fn tts_put(
    tensor_id_hi: u64, tensor_id_lo: u64,
    block_index: u32,
    data_ptr: *const f32, data_len: usize,
) -> i32 {
    // Null check
    if data_ptr.is_null() {
        set_error("data_ptr is null");
        return TTS_ERR_NULL_POINTER;
    }
    // Bounds check: ensure the slice is within WASM linear memory
    #[cfg(debug_assertions)]
    {
        let end = (data_ptr as usize) + (data_len * core::mem::size_of::<f32>());
        assert!(end <= core::arch::wasm32::memory_size(0) * 65536,
                "data_ptr + data_len exceeds linear memory");
    }
    // Safe slice construction
    let data = unsafe { core::slice::from_raw_parts(data_ptr, data_len) };
    // ... proceed with quantization and storage
}
```

### 7.3 Host Import Trust Model

The WASM module trusts that host-imported functions (`read_block`,
`write_block`, `delete_block`) behave correctly with respect to the pointers
passed to them. This is the standard WASM host-guest contract:

- The host must only read from `src_ptr` ranges within WASM linear memory.
- The host must only write to `dst_ptr` ranges within WASM linear memory.
- The host must not retain pointers across calls (WASM memory may relocate
  on `memory.grow`).

### 7.4 Debug Assertions

Debug builds include additional safety checks:

| Check | Location | Purpose |
|-------|----------|---------|
| Pointer bounds | All exported functions | Prevent out-of-bounds access |
| Block key length | `read_block`, `write_block` | Ensure 20-byte key format |
| Policy JSON validity | `tts_init` | Reject malformed configuration |
| Tier range | Host import calls | Ensure tier in {0, 1, 2} |
| Alloc alignment | `tts_alloc` | Ensure 4-byte alignment for f32 |

---

## 8. Alternatives Considered

### 8.1 WASI Filesystem for Storage

**Rejected.** WASI provides `fd_read` / `fd_write` for filesystem access, which
would allow the WASM module to perform IO directly. However, WASI filesystem
access is not available in browsers, and granting filesystem access to the WASM
module undermines the sandboxing model established in ADR-005. Host-imported IO
keeps the module fully sandboxed.

### 8.2 Component Model for the API

**Rejected for now.** The WASM Component Model provides richer type definitions
and automatic binding generation via WIT (WASM Interface Types). However, as
noted in ADR-005 section 3.1, the Component Model is still evolving and adds
canonical ABI overhead. The raw C ABI is stable, universally supported, and
sufficient for this narrow API surface. Migration path: the `tts_*` signatures
are designed to be expressible in WIT for future migration.

### 8.3 Separate WASM Modules for Compressor and Store

**Rejected.** Running `ttc_*` and `tts_*` in separate WASM modules would
require cross-module communication (via the host) for every put/get operation,
adding significant overhead. A single module with shared linear memory is
simpler and faster.

### 8.4 Passing tensor_id as a Pointer to 16 Bytes

**Rejected.** While passing `tensor_id` as a `*const u8` pointing to 16 bytes
would avoid the hi/lo split, it adds a pointer indirection and requires the
host to allocate and manage a 16-byte buffer for every call. The hi/lo split
uses value types only, which is more efficient and eliminates a class of
pointer-related bugs.

---

## 9. Acceptance Criteria

### 9.1 Functional Requirements

- [ ] `tts_init` correctly parses JSON policy and initializes the store
- [ ] `tts_put` quantizes f32 data and delegates to `write_block` host import
- [ ] `tts_get` calls `read_block`, dequantizes, and writes f32 to output
- [ ] `tts_tick` evaluates tier scores and moves blocks between tiers
- [ ] `tts_stats` returns valid JSON with tier-level statistics
- [ ] `tts_last_error` returns meaningful error messages for all error codes
- [ ] Host imports are called with correct tier, key, and buffer parameters
- [ ] Same WASM binary works in Node.js and browser without recompilation

### 9.2 Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| `tts_put` latency (512-dim, WASM) | < 5us | Includes quantization + host IO |
| `tts_get` latency (512-dim, WASM) | < 5us | Includes host IO + dequantization |
| `tts_tick` latency (100 blocks) | < 1ms | Budget-bounded |
| WASM binary size (tts + ttc) | < 150KB | Release build, wasm-opt -Oz |
| Memory overhead per tracked tensor | < 64 bytes | Metadata only, excludes block data |

### 9.3 Cross-Platform Targets

| Platform | Requirement |
|----------|-------------|
| Node.js 20+ | Full functionality with AgentDB backend |
| Chrome 110+ | Full functionality with IndexedDB backend |
| Firefox 110+ | Full functionality with IndexedDB backend |
| Safari 16.4+ | Full functionality (SharedArrayBuffer with COOP/COEP) |
| Deno 1.30+ | Full functionality with filesystem backend |
| Edge / Embedded | In-memory mode, no persistence |

---

## 10. Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Browser async IO adds significant latency | High | Medium | SharedArrayBuffer + Atomics for sync semantics; batch IO in `tts_tick` |
| IndexedDB storage limits in browser | Medium | Medium | Implement LRU eviction in `tts_tick`; surface quota warnings in `tts_stats` |
| Host import ABI mismatch across platforms | High | Low | Comprehensive integration tests per platform; ABI versioning in policy JSON |
| WASM memory.grow invalidates host pointers | Medium | Medium | Document that host must re-read `memory.buffer` after any call that may allocate |
| Shared allocator contention between ttc/tts | Low | Low | Single-threaded WASM eliminates contention; native targets use separate pools |
| Future WASM multi-threading breaks static mut | Medium | Low | Replace with `thread_local!` for native; WASM threads require explicit opt-in |

---

## 11. Open Questions

1. **IndexedDB transaction granularity**: Should each `read_block`/`write_block`
   call be a separate IndexedDB transaction, or should we batch within a
   `tts_tick` invocation?

2. **WASM module size budget**: With both `ttc_*` and `tts_*` in one module,
   the 150KB target may be tight. Should we provide a `tts_*`-only build for
   environments that do not need frame-level compression?

3. **Policy hot-reload**: Should `tts_init` be callable multiple times to
   update policy without losing block metadata, or should policy changes
   require a full re-initialization?

4. **Streaming reads**: Should `tts_get` support partial block reads (offset +
   length) for large tensor blocks, or always return the full block?

5. **Host import error propagation**: When a host import returns an error,
   should `tts_put`/`tts_get` propagate the raw error code or map it to a
   TTS-specific error?

---

## 12. Implementation Roadmap

### Phase 1: Core API Surface (Week 1)
- [ ] Define `tts_*` export functions in `ffi.rs`
- [ ] Define `tts_host` import declarations
- [ ] Implement `tts_init` with JSON policy parsing
- [ ] Implement `tts_alloc` / `tts_dealloc` / `tts_last_error`
- [ ] Unit tests for error handling and pointer validation

### Phase 2: Storage Integration (Week 2)
- [ ] Implement `tts_put` with quantization pipeline and `write_block` calls
- [ ] Implement `tts_get` with `read_block` calls and dequantization
- [ ] Implement block key encoding (tensor_id + block_index)
- [ ] Integration tests with mock host imports

### Phase 3: Tier Management (Week 2-3)
- [ ] Implement `tts_tick` with tier score evaluation
- [ ] Implement block promotion/demotion with budget enforcement
- [ ] Implement `tts_stats` with JSON serialization
- [ ] Stress tests: 10K blocks, rapid tier transitions

### Phase 4: Node.js Binding (Week 3)
- [ ] TypeScript wrapper class (`TtsStore`)
- [ ] AgentDB `TtsHostIO` implementation
- [ ] npm package build with wasm-pack
- [ ] Integration tests against live AgentDB

### Phase 5: Browser Binding (Week 4)
- [ ] IndexedDB `TtsHostIO` implementation
- [ ] SharedArrayBuffer + Atomics synchronization layer
- [ ] Asyncify fallback build
- [ ] Browser integration tests (Playwright)

### Phase 6: Edge / Embedded (Week 4+)
- [ ] In-memory `TtsHostIO` implementation
- [ ] Ring-buffer MetaLog for audit
- [ ] Memory budget enforcement tests
- [ ] Binary size optimization (wasm-opt -Oz)

---

## 13. References

1. ADR-017: Temporal Tensor Compression with Tiered Quantization (this repo)
2. ADR-005: WASM Runtime Integration (this repo)
3. ADR-018: Block-Based Storage Engine (this repo)
4. WebAssembly Specification, Section 5: Binary Format.
   https://webassembly.github.io/spec/core/binary/
5. WebAssembly JS API.
   https://developer.mozilla.org/en-US/docs/WebAssembly/JavaScript_interface
6. Asyncify: Turning WASM modules into async generators.
   https://kripken.github.io/blog/wasm/2019/07/16/asyncify.html
7. IndexedDB API.
   https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API
8. SharedArrayBuffer and Atomics.
   https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer
9. wasm-bindgen: Facilitating high-level interactions between WASM and JS.
   https://rustwasm.github.io/docs/wasm-bindgen/
10. Pelkonen, T., et al. "Gorilla: A Fast, Scalable, In-Memory Time Series
    Database." VLDB 2015.

---

## Appendix A: Node.js Host Import Implementation

```typescript
import { TtsHostIO } from "./types";
import { AgentDB } from "@ruvector/agentdb";

const TIER_NAMES = ["hot", "warm", "cold"] as const;

export class AgentDBHostIO implements TtsHostIO {
  constructor(private readonly db: AgentDB) {}

  readBlock(tier: number, key: Uint8Array): Uint8Array | null {
    const namespace = `tts:${TIER_NAMES[tier]}`;
    const keyHex = Buffer.from(key).toString("hex");
    return this.db.getSync(namespace, keyHex);
  }

  writeBlock(tier: number, key: Uint8Array, data: Uint8Array): void {
    const namespace = `tts:${TIER_NAMES[tier]}`;
    const keyHex = Buffer.from(key).toString("hex");
    this.db.putSync(namespace, keyHex, data);
  }

  deleteBlock(tier: number, key: Uint8Array): void {
    const namespace = `tts:${TIER_NAMES[tier]}`;
    const keyHex = Buffer.from(key).toString("hex");
    this.db.deleteSync(namespace, keyHex);
  }
}
```

## Appendix B: Browser Host Import Implementation (Asyncify)

```typescript
import { TtsHostIO } from "./types";

const DB_NAME = "tts-blocks";
const STORE_NAMES = ["hot", "warm", "cold"];

export class IndexedDBHostIO implements TtsHostIO {
  private db: IDBDatabase | null = null;

  async init(): Promise<void> {
    return new Promise((resolve, reject) => {
      const req = indexedDB.open(DB_NAME, 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        for (const store of STORE_NAMES) {
          if (!db.objectStoreNames.contains(store)) {
            db.createObjectStore(store);
          }
        }
      };
      req.onsuccess = () => { this.db = req.result; resolve(); };
      req.onerror = () => reject(req.error);
    });
  }

  readBlock(tier: number, key: Uint8Array): Uint8Array | null {
    // With Asyncify, this synchronous-looking call actually yields
    // to the event loop and resumes when the IDB transaction completes.
    const tx = this.db!.transaction(STORE_NAMES[tier], "readonly");
    const store = tx.objectStore(STORE_NAMES[tier]);
    const keyHex = Array.from(key, (b) => b.toString(16).padStart(2, "0")).join("");
    const req = store.get(keyHex);
    // Asyncify transforms this into an awaitable suspension point
    return req.result ? new Uint8Array(req.result) : null;
  }

  writeBlock(tier: number, key: Uint8Array, data: Uint8Array): void {
    const tx = this.db!.transaction(STORE_NAMES[tier], "readwrite");
    const store = tx.objectStore(STORE_NAMES[tier]);
    const keyHex = Array.from(key, (b) => b.toString(16).padStart(2, "0")).join("");
    store.put(data.buffer, keyHex);
  }

  deleteBlock(tier: number, key: Uint8Array): void {
    const tx = this.db!.transaction(STORE_NAMES[tier], "readwrite");
    const store = tx.objectStore(STORE_NAMES[tier]);
    const keyHex = Array.from(key, (b) => b.toString(16).padStart(2, "0")).join("");
    store.delete(keyHex);
  }
}
```

## Appendix C: WASM Module Instantiation (Node.js)

```typescript
import { readFile } from "node:fs/promises";
import { TtsStore, TtsPolicy, TtsHostIO } from "./types";

export async function loadTtsModule(
  wasmPath: string,
  hostIO: TtsHostIO,
  policy: TtsPolicy = {},
): Promise<TtsStore> {
  const wasmBytes = await readFile(wasmPath);
  const wasmMemory = new WebAssembly.Memory({ initial: 256, maximum: 4096 });

  const importObject = {
    env: { memory: wasmMemory },
    tts_host: {
      read_block: (tier: number, keyPtr: number, keyLen: number,
                   dstPtr: number, dstLen: number): number => {
        const mem = new Uint8Array(wasmMemory.buffer);
        const key = mem.slice(keyPtr, keyPtr + keyLen);
        const result = hostIO.readBlock(tier, key);
        if (!result) return -2; // TTS_ERR_TENSOR_EVICTED
        if (result.length > dstLen) return -6; // TTS_ERR_BUFFER_TOO_SMALL
        mem.set(result, dstPtr);
        return result.length;
      },
      write_block: (tier: number, keyPtr: number, keyLen: number,
                    srcPtr: number, srcLen: number): number => {
        const mem = new Uint8Array(wasmMemory.buffer);
        const key = mem.slice(keyPtr, keyPtr + keyLen);
        const data = mem.slice(srcPtr, srcPtr + srcLen);
        hostIO.writeBlock(tier, key, data);
        return 0;
      },
      delete_block: (tier: number, keyPtr: number, keyLen: number): number => {
        const mem = new Uint8Array(wasmMemory.buffer);
        const key = mem.slice(keyPtr, keyPtr + keyLen);
        hostIO.deleteBlock(tier, key);
        return 0;
      },
    },
  };

  const { instance } = await WebAssembly.instantiate(wasmBytes, importObject);
  const exports = instance.exports as Record<string, Function>;

  // Initialize the store with policy
  const policyJson = new TextEncoder().encode(JSON.stringify(policy));
  const policyPtr = exports.tts_alloc(policyJson.length) as number;
  new Uint8Array(wasmMemory.buffer).set(policyJson, policyPtr);
  const initResult = exports.tts_init(policyPtr, policyJson.length) as number;
  exports.tts_dealloc(policyPtr, policyJson.length);

  if (initResult !== 0) {
    throw new Error(`tts_init failed with code ${initResult}`);
  }

  // Return wrapped store object
  return new TtsStoreImpl(exports, wasmMemory);
}
```

---

## Related Decisions

- **ADR-005**: WASM Runtime Integration (sandboxing model, epoch interruption, raw ABI)
- **ADR-017**: Temporal Tensor Compression (frame-level `ttc_*` FFI, quantization pipeline)
- **ADR-018**: Block-Based Storage Engine (tiered placement, block format)
- **ADR-001**: RuVector Core Architecture (crate structure, dependency graph)
- **ADR-004**: KV Cache Management (three-tier cache model)
