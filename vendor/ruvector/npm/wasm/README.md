# @ruvector/wasm

WebAssembly bindings for Ruvector - High-performance vector database for browsers and Node.js.

## Features

- 🚀 **High Performance**: SIMD-accelerated vector operations
- 🌐 **Universal**: Works in browsers and Node.js
- 🎯 **Multiple Distance Metrics**: Cosine, Euclidean, Dot Product, Manhattan
- 🔍 **Fast Search**: HNSW indexing for approximate nearest neighbor search
- 💾 **Persistent Storage**: IndexedDB (browser) and file system (Node.js)
- 🦀 **Rust-powered**: Built with Rust and WebAssembly

## Installation

```bash
npm install @ruvector/wasm
```

## Quick Start

### Browser

```javascript
import { VectorDB } from '@ruvector/wasm/browser';

// Create database
const db = new VectorDB({ dimensions: 128 });
await db.init();

// Insert vectors
const vector = new Float32Array(128).fill(0.5);
const id = db.insert(vector, 'my-vector', { label: 'example' });

// Search
const results = db.search(vector, 10);
console.log(results);

// Save to IndexedDB
await db.saveToIndexedDB();
```

### Node.js

```javascript
import { VectorDB } from '@ruvector/wasm/node';

// Create database
const db = new VectorDB({ dimensions: 128 });
await db.init();

// Insert vectors
const vector = new Float32Array(128).fill(0.5);
const id = db.insert(vector, 'my-vector', { label: 'example' });

// Search
const results = db.search(vector, 10);
console.log(results);
```

### Universal (Auto-detect)

```javascript
import { VectorDB } from '@ruvector/wasm';

// Works in both browser and Node.js
const db = new VectorDB({ dimensions: 128 });
await db.init();

const vector = new Float32Array(128).fill(0.5);
const id = db.insert(vector);
const results = db.search(vector, 10);
```

## API Reference

### VectorDB

#### Constructor

```typescript
new VectorDB(options: DbOptions)
```

Options:
- `dimensions: number` - Vector dimensions (required)
- `metric?: 'euclidean' | 'cosine' | 'dotproduct' | 'manhattan'` - Distance metric (default: 'cosine')
- `useHnsw?: boolean` - Use HNSW index (default: true)

#### Methods

##### init()

Initialize the database (must be called before use).

```typescript
await db.init(): Promise<void>
```

##### insert()

Insert a single vector.

```typescript
db.insert(
  vector: Float32Array | number[],
  id?: string,
  metadata?: Record<string, any>
): string
```

##### insertBatch()

Insert multiple vectors efficiently.

```typescript
db.insertBatch(entries: VectorEntry[]): string[]
```

##### search()

Search for similar vectors.

```typescript
db.search(
  query: Float32Array | number[],
  k: number,
  filter?: Record<string, any>
): SearchResult[]
```

##### delete()

Delete a vector by ID.

```typescript
db.delete(id: string): boolean
```

##### get()

Get a vector by ID.

```typescript
db.get(id: string): VectorEntry | null
```

##### len()

Get the number of vectors.

```typescript
db.len(): number
```

##### isEmpty()

Check if database is empty.

```typescript
db.isEmpty(): boolean
```

##### getDimensions()

Get vector dimensions.

```typescript
db.getDimensions(): number
```

##### save()

Save database to persistent storage.

```typescript
await db.save(path?: string): Promise<void>
```

### Utility Functions

#### detectSIMD()

Check if SIMD is supported.

```typescript
const hasSIMD = await detectSIMD();
```

#### version()

Get library version.

```typescript
const ver = await version();
```

#### benchmark()

Run performance benchmark.

```typescript
const opsPerSec = await benchmark('insert', 1000, 128);
```

## Types

### VectorEntry

```typescript
interface VectorEntry {
  id?: string;
  vector: Float32Array | number[];
  metadata?: Record<string, any>;
}
```

### SearchResult

```typescript
interface SearchResult {
  id: string;
  score: number;
  vector?: Float32Array;
  metadata?: Record<string, any>;
}
```

### DbOptions

```typescript
interface DbOptions {
  dimensions: number;
  metric?: 'euclidean' | 'cosine' | 'dotproduct' | 'manhattan';
  useHnsw?: boolean;
}
```

## Performance

Ruvector WASM delivers exceptional performance:

- **SIMD Acceleration**: Up to 4x faster with WebAssembly SIMD
- **HNSW Index**: Sub-linear search complexity
- **Zero-copy**: Efficient memory usage with transferable objects
- **Batch Operations**: Optimized bulk inserts

## Browser Compatibility

- Chrome 91+ (SIMD support)
- Firefox 89+ (SIMD support)
- Safari 16.4+ (SIMD support)
- Edge 91+ (SIMD support)

## License

MIT

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Documentation](https://github.com/ruvnet/ruvector#readme)
- [Issues](https://github.com/ruvnet/ruvector/issues)
