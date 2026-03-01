# @ruvector/core

High-performance Rust vector database for Node.js with HNSW indexing and SIMD optimizations.

## Features

- 🚀 **Blazing Fast**: Rust + SIMD optimizations for maximum performance
- 🎯 **HNSW Indexing**: State-of-the-art approximate nearest neighbor search
- 📦 **Zero-Copy**: Efficient buffer sharing between Rust and Node.js
- 🔍 **Multiple Distance Metrics**: Euclidean, Cosine, Dot Product, Manhattan
- 💾 **Persistent Storage**: Optional disk-based storage with memory mapping
- 🔧 **Quantization**: Scalar, Product, and Binary quantization support
- 📊 **TypeScript**: Full type definitions included
- 🌍 **Cross-Platform**: Linux, macOS, and Windows support

## Installation

```bash
npm install @ruvector/core
```

The package will automatically install the correct native binding for your platform:
- Linux x64 (GNU)
- Linux ARM64 (GNU)
- macOS x64 (Intel)
- macOS ARM64 (Apple Silicon)
- Windows x64 (MSVC)

## Quick Start

```typescript
import { VectorDB, DistanceMetric } from '@ruvector/core';

// Create a database
const db = new VectorDB({
  dimensions: 384,
  distanceMetric: DistanceMetric.Cosine,
  storagePath: './vectors.db',
  hnswConfig: {
    m: 32,
    efConstruction: 200,
    efSearch: 100
  }
});

// Insert vectors
const id = await db.insert({
  vector: new Float32Array([1.0, 2.0, 3.0, ...])
});

// Search for similar vectors
const results = await db.search({
  vector: new Float32Array([1.0, 2.0, 3.0, ...]),
  k: 10
});

console.log(results);
// [{ id: 'vector-id', score: 0.95 }, ...]
```

## API Reference

### VectorDB

#### Constructor

```typescript
new VectorDB(options: DbOptions)
```

Creates a new vector database with the specified options.

**Options:**
- `dimensions` (number, required): Vector dimensions
- `distanceMetric` (DistanceMetric, optional): Distance metric (default: Cosine)
- `storagePath` (string, optional): Path for persistent storage (default: './ruvector.db')
- `hnswConfig` (HnswConfig, optional): HNSW index configuration
- `quantization` (QuantizationConfig, optional): Quantization configuration

#### Static Methods

```typescript
VectorDB.withDimensions(dimensions: number): VectorDB
```

Creates a vector database with default options.

#### Instance Methods

##### insert(entry: VectorEntry): Promise<string>

Inserts a vector into the database.

```typescript
const id = await db.insert({
  id: 'optional-id',
  vector: new Float32Array([1, 2, 3])
});
```

##### insertBatch(entries: VectorEntry[]): Promise<string[]>

Inserts multiple vectors in a batch.

```typescript
const ids = await db.insertBatch([
  { vector: new Float32Array([1, 2, 3]) },
  { vector: new Float32Array([4, 5, 6]) }
]);
```

##### search(query: SearchQuery): Promise<SearchResult[]>

Searches for similar vectors.

```typescript
const results = await db.search({
  vector: new Float32Array([1, 2, 3]),
  k: 10,
  efSearch: 100
});
```

##### delete(id: string): Promise<boolean>

Deletes a vector by ID.

```typescript
const deleted = await db.delete('vector-id');
```

##### get(id: string): Promise<VectorEntry | null>

Retrieves a vector by ID.

```typescript
const entry = await db.get('vector-id');
```

##### len(): Promise<number>

Returns the number of vectors in the database.

```typescript
const count = await db.len();
```

##### isEmpty(): Promise<boolean>

Checks if the database is empty.

```typescript
const empty = await db.isEmpty();
```

### Types

#### DistanceMetric

```typescript
enum DistanceMetric {
  Euclidean = 'Euclidean',
  Cosine = 'Cosine',
  DotProduct = 'DotProduct',
  Manhattan = 'Manhattan'
}
```

#### DbOptions

```typescript
interface DbOptions {
  dimensions: number;
  distanceMetric?: DistanceMetric;
  storagePath?: string;
  hnswConfig?: HnswConfig;
  quantization?: QuantizationConfig;
}
```

#### HnswConfig

```typescript
interface HnswConfig {
  m?: number;
  efConstruction?: number;
  efSearch?: number;
  maxElements?: number;
}
```

#### QuantizationConfig

```typescript
interface QuantizationConfig {
  type: 'none' | 'scalar' | 'product' | 'binary';
  subspaces?: number;
  k?: number;
}
```

## Performance

rUvector delivers exceptional performance:

- **150x faster** than pure JavaScript implementations
- **1M+ vectors/second** insertion rate
- **Sub-millisecond** search latency
- **4-32x memory reduction** with quantization

## Platform Support

| Platform | Architecture | Package |
|----------|-------------|---------|
| Linux | x64 | @ruvector/core-linux-x64-gnu |
| Linux | ARM64 | @ruvector/core-linux-arm64-gnu |
| macOS | x64 (Intel) | @ruvector/core-darwin-x64 |
| macOS | ARM64 (Apple Silicon) | @ruvector/core-darwin-arm64 |
| Windows | x64 | @ruvector/core-win32-x64-msvc |

## License

MIT

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Documentation](https://github.com/ruvnet/ruvector#readme)
- [Issue Tracker](https://github.com/ruvnet/ruvector/issues)
