# @ruvector/rvf-node

Native Node.js bindings for the [RuVector Format](https://github.com/ruvnet/ruvector/tree/main/crates/rvf) (RVF) vector database. Built with Rust via N-API for native speed with zero serialization overhead.

## Install

```bash
npm install @ruvector/rvf-node
```

## Features

- **Native Rust performance** via N-API (napi-rs), no FFI marshaling
- **Single-file vector database** — crash-safe, no WAL, append-only
- **k-NN search** with HNSW progressive indexing (recall 0.70 → 0.95)
- **Metadata filtering** — Eq, Ne, Lt, Gt, Range, In, And, Or, Not
- **Lineage tracking** — DNA-style parent/child derivation chains
- **Kernel & eBPF embedding** — embed compute alongside vector data
- **Segment inspection** — enumerate all segments in the file
- **Cross-platform** — Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), Windows (x86_64)
- **AGI methods** — HNSW index stats, witness chain verification, state freeze, metric introspection

## AGI Methods

Four introspection and integrity methods for advanced use cases.

### `db.indexStats()` → `RvfIndexStats`

Returns HNSW index statistics.

| Field | Type | Description |
|-------|------|-------------|
| `indexedVectors` | `number` | Number of indexed vectors |
| `layers` | `number` | Number of HNSW layers |
| `m` | `number` | M parameter (max edges per node per layer) |
| `efConstruction` | `number` | ef_construction parameter |
| `needsRebuild` | `boolean` | Whether index needs rebuilding (dead_space_ratio > 0.3) |

### `db.verifyWitness()` → `RvfWitnessResult`

Verifies SHAKE-256 witness chain integrity.

| Field | Type | Description |
|-------|------|-------------|
| `valid` | `boolean` | Whether the chain is valid |
| `entries` | `number` | Number of entries in the chain |
| `error` | `string?` | Error message if invalid |

### `db.freeze()` → `number`

Snapshot-freeze current state. Returns the epoch number.

### `db.metric()` → `string`

Returns the distance metric name (`"l2"`, `"cosine"`, or `"inner_product"`).

### Usage Example

```javascript
const stats = db.indexStats();
console.log(`Indexed: ${stats.indexedVectors}, HNSW layers: ${stats.layers}`);

const witness = db.verifyWitness();
console.log(`Witness chain: ${witness.entries} entries, valid: ${witness.valid}`);

console.log(`Distance metric: ${db.metric()}`);

const epoch = db.freeze();
console.log(`State frozen at epoch ${epoch}`);
```

## Quick Start

```javascript
const { RvfDatabase } = require('@ruvector/rvf-node');

// Create a store
const db = RvfDatabase.create('vectors.rvf', {
  dimension: 384,
  metric: 'cosine',
});

// Insert vectors
const vectors = new Float32Array(384 * 2); // 2 vectors, 384 dims each
vectors.fill(0.1);
db.ingestBatch(vectors, [1, 2]);

// Query nearest neighbors
const query = new Float32Array(384);
query.fill(0.15);
const results = db.query(query, 5);
// [{ id: 1, distance: 0.002 }, { id: 2, distance: 0.002 }]

db.close();
```

## API Reference

### Store Lifecycle

```typescript
// Create a new store
const db = RvfDatabase.create(path: string, options: RvfOptions);

// Open existing store (read-write, acquires writer lock)
const db = RvfDatabase.open(path: string);

// Open read-only (no lock, concurrent readers allowed)
const db = RvfDatabase.openReadonly(path: string);

// Close and flush
db.close();
```

**RvfOptions:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `dimension` | `number` | required | Vector dimensionality |
| `metric` | `string` | `"l2"` | `"l2"`, `"cosine"`, or `"inner_product"` |
| `profile` | `number` | `0` | Hardware profile: 0=Generic, 1=Core, 2=Hot, 3=Full |
| `signing` | `boolean` | `false` | Enable segment signing |
| `m` | `number` | `16` | HNSW M parameter (neighbor count) |
| `efConstruction` | `number` | `200` | HNSW index build quality |

### Ingest Vectors

```typescript
const result = db.ingestBatch(
  vectors: Float32Array,  // flat array of n * dimension floats
  ids: number[],          // vector IDs
  metadata?: RvfMetadataEntry[]  // optional metadata per vector
);
// Returns: { accepted: number, rejected: number, epoch: number }
```

**Metadata entry format:**

```typescript
{ fieldId: 0, valueType: 'string', value: 'category_a' }
{ fieldId: 1, valueType: 'f64',    value: '0.95' }
{ fieldId: 2, valueType: 'u64',    value: '42' }
```

### Query

```typescript
const results = db.query(
  vector: Float32Array,      // query vector
  k: number,                 // number of neighbors
  options?: RvfQueryOptions   // optional search parameters
);
// Returns: [{ id: number, distance: number }, ...]
```

**RvfQueryOptions:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `efSearch` | `number` | `100` | HNSW search quality (higher = better recall, slower) |
| `filter` | `string` | — | Filter expression as JSON string |
| `timeoutMs` | `number` | `0` | Query timeout in ms (0 = no timeout) |

### Filter Expressions

Filters are passed as JSON strings. All leaf filters require `fieldId`, `valueType`, and `value`:

```javascript
// Equality
db.query(vec, 10, {
  filter: '{"op":"eq","fieldId":0,"valueType":"string","value":"science"}'
});

// Range
db.query(vec, 10, {
  filter: '{"op":"range","fieldId":1,"valueType":"f64","low":"0.5","high":"1.0"}'
});

// In-set
db.query(vec, 10, {
  filter: '{"op":"in","fieldId":0,"valueType":"u64","values":["1","2","5"]}'
});

// Boolean combinations
db.query(vec, 10, {
  filter: JSON.stringify({
    op: 'and',
    children: [
      { op: 'eq', fieldId: 0, valueType: 'string', value: 'science' },
      { op: 'gt', fieldId: 1, valueType: 'f64', value: '0.8' }
    ]
  })
});

// Negation
db.query(vec, 10, {
  filter: '{"op":"not","child":{"op":"eq","fieldId":0,"valueType":"string","value":"spam"}}'
});
```

**Supported operators:** `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in`, `range`, `and`, `or`, `not`

**Supported value types:** `u64`, `i64`, `f64`, `string`, `bool`

### Delete

```typescript
// Delete by ID
const result = db.delete([1, 2, 3]);
// Returns: { deleted: number, epoch: number }

// Delete by filter
const result = db.deleteByFilter(
  '{"op":"gt","fieldId":1,"valueType":"f64","value":"0.9"}'
);
```

### Compact

Reclaims space from deleted vectors:

```typescript
const result = db.compact();
// Returns: { segmentsCompacted: number, bytesReclaimed: number, epoch: number }
```

### Status

```typescript
const status = db.status();
// {
//   totalVectors: number,
//   totalSegments: number,
//   fileSize: number,
//   currentEpoch: number,
//   profileId: number,
//   compactionState: 'idle' | 'running' | 'emergency',
//   deadSpaceRatio: number,
//   readOnly: boolean
// }
```

### Lineage & Derivation

RVF tracks parent/child relationships with cryptographic hashes:

```typescript
db.fileId();        // hex string — unique file identifier
db.parentId();      // hex string — parent's ID (zeros if root)
db.lineageDepth();  // 0 for root files

// Derive a child store (inherits dimensions and options)
const child = db.derive('/tmp/child.rvf');
child.lineageDepth(); // 1
child.parentId();     // matches parent's fileId()
```

### Kernel & eBPF Embedding

Embed compute segments alongside vector data:

```typescript
// Embed a Linux microkernel
db.embedKernel(
  1,                           // arch: 0=x86_64, 1=aarch64
  0,                           // kernel type
  0,                           // flags
  Buffer.from(kernelImage),    // kernel binary
  8080,                        // API port
  'console=ttyS0 quiet'       // kernel cmdline (optional)
);

// Extract kernel
const kernel = db.extractKernel();
if (kernel) {
  console.log(kernel.header);  // Buffer: 128-byte KernelHeader
  console.log(kernel.image);   // Buffer: kernel image bytes
}

// Embed an eBPF XDP program
db.embedEbpf(
  1,                          // program type (XDP distance)
  2,                          // attach type (XDP ingress)
  384,                        // max vector dimension
  Buffer.from(bytecode),      // BPF ELF object
  Buffer.from(btf)            // optional BTF section
);

// Extract eBPF
const ebpf = db.extractEbpf();
if (ebpf) {
  console.log(ebpf.header);   // Buffer: 64-byte EbpfHeader
  console.log(ebpf.payload);  // Buffer: bytecode + BTF
}
```

### Segment Inspection

```typescript
const segments = db.segments();
// [{ id: 1, offset: 0, payloadLength: 4096, segType: 'manifest' },
//  { id: 2, offset: 4160, payloadLength: 51200, segType: 'vec' },
//  { id: 3, offset: 55424, payloadLength: 12288, segType: 'index' }]

db.dimension(); // 384
```

## Self-Booting RVF

An `.rvf` file can embed a Linux kernel, eBPF programs, and SSH keys alongside vector data — producing a single file that boots as a microservice.

The Claude Code Appliance example builds a complete AI dev environment:

```bash
cd examples/rvf
cargo run --example claude_code_appliance
```

```
claude_code_appliance.rvf
  ├── KERNEL_SEG    Linux 6.8.12 bzImage (5.2 MB, x86_64)
  ├── EBPF_SEG      Socket filter — ports 2222, 8080 only
  ├── VEC_SEG       20 package embeddings (128-dim)
  ├── INDEX_SEG     HNSW graph for package search
  ├── WITNESS_SEG   6-entry tamper-evident audit trail
  └── CRYPTO_SEG    3 Ed25519 SSH user keys
```

Final file: **5.1 MB single `.rvf`** — boots Linux, serves queries, runs Claude Code.

See the [full RVF documentation](https://github.com/ruvnet/ruvector/tree/main/crates/rvf) for details.

## Build from Source

```bash
# Prerequisites: Rust 1.87+, Node.js 18+
cd crates/rvf/rvf-node
npm install
npm run build
```

## Related Packages

| Package | Description |
|---------|-------------|
| [`@ruvector/rvf`](https://www.npmjs.com/package/@ruvector/rvf) | Unified TypeScript SDK |
| [`@ruvector/rvf-wasm`](https://www.npmjs.com/package/@ruvector/rvf-wasm) | Browser WASM package |
| [`@ruvector/rvf-mcp-server`](https://www.npmjs.com/package/@ruvector/rvf-mcp-server) | MCP server for AI agents |
| [`rvf-runtime`](https://crates.io/crates/rvf-runtime) | Rust runtime (powers this package) |

## License

MIT OR Apache-2.0
