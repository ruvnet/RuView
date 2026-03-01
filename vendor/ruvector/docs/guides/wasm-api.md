# Ruvector WASM API Documentation

## Overview

Ruvector WASM provides a high-performance vector database for browser and Node.js environments. It leverages Rust's speed and safety with WebAssembly for near-native performance.

## Features

- ✅ **Full VectorDB API**: Insert, search, delete, batch operations
- ✅ **SIMD Acceleration**: Automatic detection and use of SIMD instructions when available
- ✅ **Web Workers**: Parallel operations across multiple worker threads
- ✅ **IndexedDB Persistence**: Save and load database state
- ✅ **LRU Cache**: Efficient caching for hot vectors
- ✅ **Zero-Copy Transfers**: Transferable objects for optimal performance
- ✅ **Multiple Distance Metrics**: Euclidean, Cosine, Dot Product, Manhattan

## Installation

```bash
npm install @ruvector/wasm
```

Or build from source:

```bash
cd crates/ruvector-wasm
npm run build
```

## Basic Usage

### Vanilla JavaScript

```javascript
import init, { VectorDB } from '@ruvector/wasm';

// Initialize WASM module
await init();

// Create database
const db = new VectorDB(384, 'cosine', true);

// Insert vector
const vector = new Float32Array(384).map(() => Math.random());
const id = db.insert(vector, 'vec_1', { label: 'example' });

// Search
const query = new Float32Array(384).map(() => Math.random());
const results = db.search(query, 10);

console.log(results);
// [{ id: 'vec_1', score: 0.123, metadata: { label: 'example' } }, ...]
```

### With Web Workers

```javascript
import { WorkerPool } from '@ruvector/wasm/worker-pool';

const pool = new WorkerPool(
  '/worker.js',
  '/pkg/ruvector_wasm.js',
  {
    poolSize: 4,
    dimensions: 384,
    metric: 'cosine'
  }
);

await pool.init();

// Parallel insert
const entries = Array(1000).fill(0).map((_, i) => ({
  vector: Array(384).fill(0).map(() => Math.random()),
  id: `vec_${i}`,
  metadata: { index: i }
}));

const ids = await pool.insertBatch(entries);

// Parallel search
const results = await pool.search(query, 10);

// Cleanup
pool.terminate();
```

### With IndexedDB Persistence

```javascript
import { IndexedDBPersistence } from '@ruvector/wasm/indexeddb';

const persistence = new IndexedDBPersistence('my_database');
await persistence.open();

// Save vectors
await persistence.saveBatch(entries);

// Load with progress callback
await persistence.loadAll((progress) => {
  console.log(`Loaded ${progress.loaded} vectors`);

  // Insert into database
  if (progress.vectors.length > 0) {
    db.insertBatch(progress.vectors);
  }
});

// Get stats
const stats = await persistence.getStats();
console.log(`Total vectors: ${stats.totalVectors}`);
console.log(`Cache size: ${stats.cacheSize}`);
```

## API Reference

### VectorDB

#### Constructor

```typescript
new VectorDB(
  dimensions: number,
  metric?: 'euclidean' | 'cosine' | 'dotproduct' | 'manhattan',
  useHnsw?: boolean
): VectorDB
```

Creates a new VectorDB instance.

**Parameters:**
- `dimensions`: Vector dimensions (required)
- `metric`: Distance metric (default: 'cosine')
- `useHnsw`: Use HNSW index for faster search (default: true)

#### Methods

##### insert

```typescript
insert(
  vector: Float32Array,
  id?: string,
  metadata?: object
): string
```

Insert a single vector.

**Returns:** Vector ID

##### insertBatch

```typescript
insertBatch(entries: Array<{
  vector: Float32Array,
  id?: string,
  metadata?: object
}>): string[]
```

Insert multiple vectors in a batch (more efficient).

**Returns:** Array of vector IDs

##### search

```typescript
search(
  query: Float32Array,
  k: number,
  filter?: object
): Array<{
  id: string,
  score: number,
  vector?: Float32Array,
  metadata?: object
}>
```

Search for similar vectors.

**Parameters:**
- `query`: Query vector
- `k`: Number of results to return
- `filter`: Optional metadata filter

**Returns:** Array of search results sorted by similarity

##### delete

```typescript
delete(id: string): boolean
```

Delete a vector by ID.

**Returns:** True if deleted, false if not found

##### get

```typescript
get(id: string): {
  id: string,
  vector: Float32Array,
  metadata?: object
} | null
```

Get a vector by ID.

**Returns:** Vector entry or null if not found

##### len

```typescript
len(): number
```

Get the number of vectors in the database.

##### isEmpty

```typescript
isEmpty(): boolean
```

Check if the database is empty.

### WorkerPool

#### Constructor

```typescript
new WorkerPool(
  workerUrl: string,
  wasmUrl: string,
  options: {
    poolSize?: number,
    dimensions: number,
    metric?: string,
    useHnsw?: boolean
  }
): WorkerPool
```

Creates a worker pool for parallel operations.

**Parameters:**
- `workerUrl`: URL to worker.js
- `wasmUrl`: URL to WASM module
- `options.poolSize`: Number of workers (default: CPU cores)
- `options.dimensions`: Vector dimensions
- `options.metric`: Distance metric
- `options.useHnsw`: Use HNSW index

#### Methods

##### init

```typescript
async init(): Promise<void>
```

Initialize the worker pool.

##### insert

```typescript
async insert(
  vector: number[],
  id?: string,
  metadata?: object
): Promise<string>
```

Insert vector via worker pool.

##### insertBatch

```typescript
async insertBatch(entries: Array<{
  vector: number[],
  id?: string,
  metadata?: object
}>): Promise<string[]>
```

Insert batch via worker pool (distributed across workers).

##### search

```typescript
async search(
  query: number[],
  k?: number,
  filter?: object
): Promise<Array<{
  id: string,
  score: number,
  metadata?: object
}>>
```

Search via worker pool.

##### searchBatch

```typescript
async searchBatch(
  queries: number[][],
  k?: number,
  filter?: object
): Promise<Array<Array<SearchResult>>>
```

Parallel search across multiple queries.

##### terminate

```typescript
terminate(): void
```

Terminate all workers.

##### getStats

```typescript
getStats(): {
  poolSize: number,
  busyWorkers: number,
  idleWorkers: number,
  pendingRequests: number
}
```

Get pool statistics.

### IndexedDBPersistence

#### Constructor

```typescript
new IndexedDBPersistence(dbName?: string): IndexedDBPersistence
```

Creates IndexedDB persistence manager.

#### Methods

##### open

```typescript
async open(): Promise<IDBDatabase>
```

Open IndexedDB connection.

##### saveVector

```typescript
async saveVector(
  id: string,
  vector: Float32Array,
  metadata?: object
): Promise<string>
```

Save a single vector.

##### saveBatch

```typescript
async saveBatch(
  entries: Array<{
    id: string,
    vector: Float32Array,
    metadata?: object
  }>,
  batchSize?: number
): Promise<number>
```

Save vectors in batch.

##### loadVector

```typescript
async loadVector(id: string): Promise<{
  id: string,
  vector: Float32Array,
  metadata?: object,
  timestamp: number
} | null>
```

Load a single vector.

##### loadAll

```typescript
async loadAll(
  onProgress?: (progress: {
    loaded: number,
    vectors: Array<any>,
    complete?: boolean
  }) => void,
  batchSize?: number
): Promise<{ count: number, complete: boolean }>
```

Load all vectors with progressive loading.

##### deleteVector

```typescript
async deleteVector(id: string): Promise<boolean>
```

Delete a vector.

##### clear

```typescript
async clear(): Promise<void>
```

Clear all vectors.

##### getStats

```typescript
async getStats(): Promise<{
  totalVectors: number,
  cacheSize: number,
  cacheHitRate: number
}>
```

Get database statistics.

## Utility Functions

### detectSIMD

```typescript
detectSIMD(): boolean
```

Detect if SIMD is supported in the current environment.

### version

```typescript
version(): string
```

Get Ruvector version.

### benchmark

```typescript
benchmark(
  name: string,
  iterations: number,
  dimensions: number
): number
```

Run performance benchmark.

**Returns:** Operations per second

## Performance Tips

1. **Use Batch Operations**: `insertBatch` is significantly faster than multiple `insert` calls
2. **Enable SIMD**: Build with SIMD feature for 2-4x speedup on supported hardware
3. **Use Web Workers**: Distribute operations across workers for parallel processing
4. **Use LRU Cache**: Keep hot vectors in memory via IndexedDB cache
5. **Optimize Vector Size**: Smaller dimensions = faster operations
6. **Use Appropriate Metric**: Dot product is fastest, Euclidean is slowest

## Browser Support

- Chrome 91+ (with SIMD)
- Firefox 89+ (with SIMD)
- Safari 16.4+ (limited SIMD)
- Edge 91+

## Size Optimization

The WASM binary is optimized for size:
- Base build: ~450KB gzipped
- With SIMD: ~480KB gzipped

Build size can be further reduced with:

```bash
npm run optimize
```

## Examples

See:
- `/examples/wasm-vanilla/` - Vanilla JavaScript example
- `/examples/wasm-react/` - React with Web Workers example

## Troubleshooting

### SIMD not working

Ensure your browser supports SIMD and you're using the SIMD build:

```javascript
import init from '@ruvector/wasm-simd';
```

### Workers not starting

Check CORS headers and ensure worker.js is served from the same origin.

### IndexedDB errors

Ensure your browser supports IndexedDB and you have sufficient storage quota.

## License

MIT
