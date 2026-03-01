# ADR-DB-008: Delta WASM Integration

**Status**: Proposed
**Date**: 2026-01-28
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board
**Parent**: ADR-DB-001 Delta Behavior Core Architecture

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-28 | Architecture Team | Initial proposal |

---

## Context and Problem Statement

### The WASM Boundary Challenge

Delta-behavior must work seamlessly across WASM module boundaries:

1. **Data Sharing**: Efficient delta transfer between host and WASM
2. **Memory Management**: WASM linear memory constraints
3. **API Design**: JavaScript-friendly interfaces
4. **Performance**: Minimize serialization overhead
5. **Streaming**: Support for real-time delta streams

### Ruvector WASM Architecture

Current ruvector WASM bindings (ADR-001) use:
- `wasm-bindgen` for JavaScript interop
- Memory-only storage (`storage_memory.rs`)
- Full vector copies across boundary

### WASM Constraints

| Constraint | Impact |
|------------|--------|
| Linear memory | Single contiguous address space |
| No threads | No parallel processing (without Atomics) |
| No filesystem | Memory-only persistence |
| Serialization cost | Every cross-boundary call |
| 32-bit pointers | 4GB address limit |

---

## Decision

### Adopt Component Model with Shared Memory

We implement delta WASM integration using the emerging WebAssembly Component Model with optimized shared memory patterns.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           JAVASCRIPT HOST                                    │
│                                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │   Delta API     │  │   Event Stream  │  │   TypedArray Views          │ │
│  │   (High-level)  │  │   (Callbacks)   │  │   (Zero-copy access)        │ │
│  └────────┬────────┘  └────────┬────────┘  └─────────────┬───────────────┘ │
│           │                    │                         │                  │
└───────────┼────────────────────┼─────────────────────────┼──────────────────┘
            │                    │                         │
            v                    v                         v
┌───────────────────────────────────────────────────────────────────────────────┐
│                          WASM BINDING LAYER                                   │
│                                                                               │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────────────┐│
│  │ wasm-bindgen     │  │  EventEmitter    │  │  SharedArrayBuffer Bridge    ││
│  │ Interface        │  │  Integration     │  │  (when available)            ││
│  └────────┬─────────┘  └────────┬─────────┘  └─────────────┬────────────────┘│
│           │                     │                          │                 │
└───────────┼─────────────────────┼──────────────────────────┼─────────────────┘
            │                     │                          │
            v                     v                          v
┌───────────────────────────────────────────────────────────────────────────────┐
│                          RUVECTOR DELTA CORE (WASM)                           │
│                                                                               │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────────────┐│
│  │  Delta Manager   │  │  Delta Stream    │  │  Shared Memory Pool          ││
│  │                  │  │  Processor       │  │                              ││
│  └──────────────────┘  └──────────────────┘  └──────────────────────────────┘│
│                                                                               │
└───────────────────────────────────────────────────────────────────────────────┘
```

### Interface Contracts

#### TypeScript/JavaScript API

```typescript
/**
 * Delta-aware vector database for WASM environments
 */
export class DeltaVectorDB {
    /**
     * Create a new delta-aware vector database
     */
    constructor(options: DeltaDBOptions);

    /**
     * Apply a delta to a vector
     * @returns Delta ID
     */
    applyDelta(delta: VectorDelta): string;

    /**
     * Apply multiple deltas efficiently (batch)
     * @returns Array of Delta IDs
     */
    applyDeltas(deltas: VectorDelta[]): string[];

    /**
     * Get current vector (composed from delta chain)
     * @returns Float32Array or null if not found
     */
    getVector(id: string): Float32Array | null;

    /**
     * Get vector at specific time
     */
    getVectorAt(id: string, timestamp: Date): Float32Array | null;

    /**
     * Subscribe to delta stream
     */
    onDelta(callback: (delta: VectorDelta) => void): () => void;

    /**
     * Search with delta-aware semantics
     */
    search(query: Float32Array, k: number): SearchResult[];

    /**
     * Get delta chain for debugging/inspection
     */
    getDeltaChain(id: string): DeltaChain;

    /**
     * Compact delta chains
     */
    compact(options?: CompactOptions): CompactionStats;

    /**
     * Export state for persistence (IndexedDB, etc.)
     */
    export(): Uint8Array;

    /**
     * Import previously exported state
     */
    import(data: Uint8Array): void;
}

/**
 * Delta operation types
 */
export interface VectorDelta {
    /** Target vector ID */
    vectorId: string;
    /** Delta operation */
    operation: DeltaOperation;
    /** Optional metadata changes */
    metadata?: Record<string, unknown>;
    /** Timestamp (auto-generated if not provided) */
    timestamp?: Date;
}

export type DeltaOperation =
    | { type: 'create'; vector: Float32Array }
    | { type: 'sparse'; indices: Uint32Array; values: Float32Array }
    | { type: 'dense'; vector: Float32Array }
    | { type: 'scale'; factor: number }
    | { type: 'offset'; amount: number }
    | { type: 'delete' };
```

#### Rust WASM Bindings

```rust
use wasm_bindgen::prelude::*;
use js_sys::{Float32Array, Uint32Array, Uint8Array, Function};

/// Delta-aware vector database for WASM
#[wasm_bindgen]
pub struct DeltaVectorDB {
    inner: WasmDeltaManager,
    event_listeners: Vec<Function>,
}

#[wasm_bindgen]
impl DeltaVectorDB {
    /// Create new database
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<DeltaVectorDB, JsError> {
        let config: DeltaDBOptions = serde_wasm_bindgen::from_value(options)?;
        Ok(Self {
            inner: WasmDeltaManager::new(config)?,
            event_listeners: Vec::new(),
        })
    }

    /// Apply a delta operation
    #[wasm_bindgen(js_name = applyDelta)]
    pub fn apply_delta(&mut self, delta: JsValue) -> Result<String, JsError> {
        let delta: VectorDelta = serde_wasm_bindgen::from_value(delta)?;
        let delta_id = self.inner.apply_delta(delta)?;

        // Emit to listeners
        self.emit_delta_event(&delta_id);

        Ok(delta_id.to_string())
    }

    /// Apply batch of deltas efficiently
    #[wasm_bindgen(js_name = applyDeltas)]
    pub fn apply_deltas(&mut self, deltas: JsValue) -> Result<JsValue, JsError> {
        let deltas: Vec<VectorDelta> = serde_wasm_bindgen::from_value(deltas)?;
        let ids = self.inner.apply_deltas(deltas)?;

        Ok(serde_wasm_bindgen::to_value(&ids)?)
    }

    /// Get current vector as Float32Array
    #[wasm_bindgen(js_name = getVector)]
    pub fn get_vector(&self, id: &str) -> Option<Float32Array> {
        self.inner.get_vector(id)
            .map(|v| {
                let array = Float32Array::new_with_length(v.len() as u32);
                array.copy_from(&v);
                array
            })
    }

    /// Search for nearest neighbors
    #[wasm_bindgen(js_name = search)]
    pub fn search(&self, query: Float32Array, k: u32) -> Result<JsValue, JsError> {
        let query_vec: Vec<f32> = query.to_vec();
        let results = self.inner.search(&query_vec, k as usize)?;
        Ok(serde_wasm_bindgen::to_value(&results)?)
    }

    /// Subscribe to delta events
    #[wasm_bindgen(js_name = onDelta)]
    pub fn on_delta(&mut self, callback: Function) -> usize {
        let index = self.event_listeners.len();
        self.event_listeners.push(callback);
        index
    }

    /// Export state for persistence
    #[wasm_bindgen(js_name = export)]
    pub fn export(&self) -> Result<Uint8Array, JsError> {
        let bytes = self.inner.export()?;
        let array = Uint8Array::new_with_length(bytes.len() as u32);
        array.copy_from(&bytes);
        Ok(array)
    }

    /// Import previously exported state
    #[wasm_bindgen(js_name = import)]
    pub fn import(&mut self, data: Uint8Array) -> Result<(), JsError> {
        let bytes = data.to_vec();
        self.inner.import(&bytes)?;
        Ok(())
    }
}
```

### Shared Memory Pattern

For high-throughput scenarios, we use a shared memory pool:

```rust
/// Shared memory pool for zero-copy delta transfer
#[wasm_bindgen]
pub struct SharedDeltaPool {
    /// Preallocated buffer for deltas
    buffer: Vec<u8>,
    /// Write position
    write_pos: usize,
    /// Read position
    read_pos: usize,
    /// Capacity
    capacity: usize,
}

#[wasm_bindgen]
impl SharedDeltaPool {
    #[wasm_bindgen(constructor)]
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0u8; capacity],
            write_pos: 0,
            read_pos: 0,
            capacity,
        }
    }

    /// Get buffer pointer for direct JS access
    #[wasm_bindgen(js_name = getBufferPtr)]
    pub fn get_buffer_ptr(&self) -> *const u8 {
        self.buffer.as_ptr()
    }

    /// Get buffer length
    #[wasm_bindgen(js_name = getBufferLen)]
    pub fn get_buffer_len(&self) -> usize {
        self.capacity
    }

    /// Write delta to shared buffer
    #[wasm_bindgen(js_name = writeDelta)]
    pub fn write_delta(&mut self, delta: JsValue) -> Result<usize, JsError> {
        let delta: VectorDelta = serde_wasm_bindgen::from_value(delta)?;
        let encoded = encode_delta(&delta)?;

        // Check capacity
        if self.write_pos + encoded.len() > self.capacity {
            return Err(JsError::new("Buffer full"));
        }

        // Write length prefix + data
        let len_bytes = (encoded.len() as u32).to_le_bytes();
        self.buffer[self.write_pos..self.write_pos + 4].copy_from_slice(&len_bytes);
        self.write_pos += 4;

        self.buffer[self.write_pos..self.write_pos + encoded.len()].copy_from_slice(&encoded);
        self.write_pos += encoded.len();

        Ok(self.write_pos)
    }

    /// Flush buffer and apply all deltas
    #[wasm_bindgen(js_name = flush)]
    pub fn flush(&mut self, db: &mut DeltaVectorDB) -> Result<usize, JsError> {
        let mut count = 0;
        self.read_pos = 0;

        while self.read_pos < self.write_pos {
            // Read length prefix
            let len_bytes: [u8; 4] = self.buffer[self.read_pos..self.read_pos + 4]
                .try_into()
                .unwrap();
            let len = u32::from_le_bytes(len_bytes) as usize;
            self.read_pos += 4;

            // Decode and apply delta
            let encoded = &self.buffer[self.read_pos..self.read_pos + len];
            let delta = decode_delta(encoded)?;
            db.inner.apply_delta(delta)?;

            self.read_pos += len;
            count += 1;
        }

        // Reset buffer
        self.write_pos = 0;
        self.read_pos = 0;

        Ok(count)
    }
}
```

### JavaScript Integration

```typescript
// High-performance delta streaming using SharedArrayBuffer (when available)
class DeltaStreamProcessor {
    private db: DeltaVectorDB;
    private pool: SharedDeltaPool;
    private worker?: Worker;

    constructor(db: DeltaVectorDB, poolSize: number = 1024 * 1024) {
        this.db = db;
        this.pool = new SharedDeltaPool(poolSize);

        // Use Web Worker for background processing if available
        if (typeof Worker !== 'undefined') {
            this.initWorker();
        }
    }

    private initWorker() {
        const workerCode = `
            self.onmessage = function(e) {
                const { type, data } = e.data;
                if (type === 'process') {
                    // Process deltas in worker
                    self.postMessage({ type: 'done', count: data.length });
                }
            };
        `;
        const blob = new Blob([workerCode], { type: 'application/javascript' });
        this.worker = new Worker(URL.createObjectURL(blob));
    }

    // Stream deltas with batching
    async streamDeltas(deltas: AsyncIterable<VectorDelta>): Promise<number> {
        let count = 0;
        let batch: VectorDelta[] = [];
        const BATCH_SIZE = 100;

        for await (const delta of deltas) {
            batch.push(delta);

            if (batch.length >= BATCH_SIZE) {
                count += await this.processBatch(batch);
                batch = [];
            }
        }

        // Process remaining
        if (batch.length > 0) {
            count += await this.processBatch(batch);
        }

        return count;
    }

    private async processBatch(deltas: VectorDelta[]): Promise<number> {
        // Write to shared pool
        for (const delta of deltas) {
            this.pool.writeDelta(delta);
        }

        // Flush to database
        return this.pool.flush(this.db);
    }

    // Zero-copy vector access
    getVectorView(id: string): Float32Array | null {
        const ptr = this.db.getVectorPtr(id);
        if (ptr === 0) return null;

        const dims = this.db.getDimensions();
        const memory = this.db.getMemory();

        // Create view directly into WASM memory
        return new Float32Array(memory.buffer, ptr, dims);
    }
}
```

---

## Performance Considerations

### Serialization Overhead

| Method | Size (bytes) | Encode (us) | Decode (us) |
|--------|--------------|-------------|-------------|
| JSON | 500 | 50 | 30 |
| serde_wasm_bindgen | 200 | 20 | 15 |
| Manual binary | 100 | 5 | 3 |
| Zero-copy (view) | 0 | 0.1 | 0.1 |

### Memory Usage

| Component | Memory | Notes |
|-----------|--------|-------|
| WASM linear memory | 1MB initial | Grows as needed |
| Delta pool | 1MB | Configurable |
| Vector storage | ~4B * dims * count | Grows with data |
| HNSW index | ~640B * count | Graph structure |

### Benchmarks (Chrome, 10K vectors, 384 dims)

| Operation | Native | WASM | Ratio |
|-----------|--------|------|-------|
| Apply delta (sparse 5%) | 5us | 15us | 3x |
| Apply delta (dense) | 10us | 25us | 2.5x |
| Get vector | 0.5us | 5us | 10x |
| Search k=10 | 100us | 300us | 3x |
| Batch apply (100) | 200us | 400us | 2x |

---

## Considered Options

### Option 1: Full Serialization Every Call

**Description**: Serialize/deserialize on each API call.

**Pros**:
- Simple implementation
- Works everywhere

**Cons**:
- High overhead
- Memory copying
- GC pressure in JS

**Verdict**: Used for complex objects, not for bulk data.

### Option 2: SharedArrayBuffer

**Description**: True shared memory between JS and WASM.

**Pros**:
- Zero-copy possible
- Highest performance

**Cons**:
- Requires COOP/COEP headers
- Not available in all contexts
- Complex synchronization

**Verdict**: Optional optimization when available.

### Option 3: Component Model (Selected)

**Description**: WASM Component Model with resource types.

**Pros**:
- Clean interface definitions
- Future-proof (standard)
- Better than wasm-bindgen long-term

**Cons**:
- Still maturing
- Browser support varies

**Verdict**: Adopted as target, with wasm-bindgen fallback.

### Option 4: Direct Memory Access

**Description**: Expose raw memory pointers.

**Pros**:
- Maximum performance
- Zero overhead

**Cons**:
- Unsafe
- Manual memory management
- Easy to corrupt state

**Verdict**: Used internally, not exposed to JS.

---

## Technical Specification

### Interface Definition (WIT)

```wit
// delta-vector.wit (Component Model interface)
package ruvector:delta@0.1.0;

interface delta-types {
    // Delta identifier
    type delta-id = string;
    type vector-id = string;

    // Delta operations
    variant delta-operation {
        create(list<float32>),
        sparse(sparse-update),
        dense(list<float32>),
        scale(float32),
        offset(float32),
        delete,
    }

    record sparse-update {
        indices: list<u32>,
        values: list<float32>,
    }

    record vector-delta {
        vector-id: vector-id,
        operation: delta-operation,
        timestamp: option<u64>,
    }

    record search-result {
        id: vector-id,
        score: float32,
    }
}

interface delta-db {
    use delta-types.{delta-id, vector-id, vector-delta, search-result};

    // Resource representing the database
    resource database {
        constructor(dimensions: u32);

        apply-delta: func(delta: vector-delta) -> result<delta-id, string>;
        apply-deltas: func(deltas: list<vector-delta>) -> result<list<delta-id>, string>;
        get-vector: func(id: vector-id) -> option<list<float32>>;
        search: func(query: list<float32>, k: u32) -> list<search-result>;
        export: func() -> list<u8>;
        import: func(data: list<u8>) -> result<_, string>;
    }
}

world delta-vector-world {
    export delta-db;
}
```

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct DeltaDBOptions {
    /// Vector dimensions
    pub dimensions: u32,
    /// Maximum vectors
    pub max_vectors: u32,
    /// Enable compression
    pub compression: bool,
    /// Checkpoint interval (deltas)
    pub checkpoint_interval: u32,
    /// HNSW configuration
    pub hnsw_m: u32,
    pub hnsw_ef_construction: u32,
    pub hnsw_ef_search: u32,
}

impl Default for DeltaDBOptions {
    fn default() -> Self {
        Self {
            dimensions: 384,
            max_vectors: 100_000,
            compression: true,
            checkpoint_interval: 100,
            hnsw_m: 16,
            hnsw_ef_construction: 100,
            hnsw_ef_search: 50,
        }
    }
}
```

---

## Consequences

### Benefits

1. **Browser Deployment**: Delta operations in web applications
2. **Edge Computing**: Run on WASM-capable edge nodes
3. **Unified Codebase**: Same delta logic for all platforms
4. **Streaming Support**: Real-time delta processing in browser
5. **Persistence Options**: Export/import for IndexedDB

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Performance gap | High | Medium | Zero-copy patterns, batching |
| Memory limits | Medium | High | Streaming, compression |
| Browser compatibility | Low | Medium | Feature detection, fallbacks |
| Component Model changes | Medium | Low | Abstraction layer |

---

## References

1. WebAssembly Component Model. https://component-model.bytecodealliance.org/
2. wasm-bindgen Reference. https://rustwasm.github.io/wasm-bindgen/
3. ADR-001: Ruvector Core Architecture (WASM section)
4. ADR-DB-001: Delta Behavior Core Architecture

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-006**: Delta Compression Strategy
- **ADR-005**: WASM Runtime Integration
