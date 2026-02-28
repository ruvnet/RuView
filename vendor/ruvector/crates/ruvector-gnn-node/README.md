# @ruvector/gnn - Graph Neural Network Node.js Bindings

High-performance Graph Neural Network (GNN) capabilities for Ruvector, powered by Rust and NAPI-RS.

[![npm version](https://img.shields.io/npm/v/@ruvector/gnn.svg)](https://www.npmjs.com/package/@ruvector/gnn)
[![CI](https://github.com/ruvnet/ruvector/actions/workflows/build-gnn.yml/badge.svg)](https://github.com/ruvnet/ruvector/actions/workflows/build-gnn.yml)

## Features

- **GNN Layers**: Multi-head attention, layer normalization, GRU cells
- **Tensor Compression**: Adaptive compression with 5 levels (None, Half, PQ8, PQ4, Binary)
- **Differentiable Search**: Soft attention-based search with temperature scaling
- **Hierarchical Processing**: Multi-layer GNN forward pass
- **Zero-copy**: Efficient data transfer between JavaScript and Rust
- **TypeScript Support**: Full type definitions included

## Installation

```bash
npm install @ruvector/gnn
```

## Quick Start

### Creating a GNN Layer

```javascript
const { RuvectorLayer } = require('@ruvector/gnn');

// Create a GNN layer with:
// - Input dimension: 128
// - Hidden dimension: 256
// - Attention heads: 4
// - Dropout rate: 0.1
const layer = new RuvectorLayer(128, 256, 4, 0.1);

// Forward pass
const nodeEmbedding = new Array(128).fill(0).map(() => Math.random());
const neighborEmbeddings = [
  new Array(128).fill(0).map(() => Math.random()),
  new Array(128).fill(0).map(() => Math.random()),
];
const edgeWeights = [0.7, 0.3];

const output = layer.forward(nodeEmbedding, neighborEmbeddings, edgeWeights);
console.log('Output dimension:', output.length); // 256
```

### Tensor Compression

```javascript
const { TensorCompress, getCompressionLevel } = require('@ruvector/gnn');

const compressor = new TensorCompress();
const embedding = new Array(128).fill(0).map(() => Math.random());

// Adaptive compression based on access frequency
const accessFreq = 0.5; // 50% access rate
console.log('Selected level:', getCompressionLevel(accessFreq)); // "half"

const compressed = compressor.compress(embedding, accessFreq);
const decompressed = compressor.decompress(compressed);

console.log('Original size:', embedding.length);
console.log('Compression ratio:', compressed.length / JSON.stringify(embedding).length);

// Explicit compression level
const level = {
  level_type: 'pq8',
  subvectors: 8,
  centroids: 16
};
const compressedPQ = compressor.compressWithLevel(embedding, level);
```

### Differentiable Search

```javascript
const { differentiableSearch } = require('@ruvector/gnn');

const query = [1.0, 0.0, 0.0];
const candidates = [
  [1.0, 0.0, 0.0],  // Perfect match
  [0.9, 0.1, 0.0],  // Close match
  [0.0, 1.0, 0.0],  // Orthogonal
];

const result = differentiableSearch(query, candidates, 2, 1.0);
console.log('Top-2 indices:', result.indices);    // [0, 1]
console.log('Soft weights:', result.weights);     // [0.x, 0.y]
```

### Hierarchical Forward Pass

```javascript
const { hierarchicalForward, RuvectorLayer } = require('@ruvector/gnn');

const query = [1.0, 0.0];

// Layer embeddings (organized by HNSW layers)
const layerEmbeddings = [
  [[1.0, 0.0], [0.0, 1.0]],  // Layer 0 embeddings
];

// Create and serialize GNN layers
const layer1 = new RuvectorLayer(2, 2, 1, 0.0);
const layers = [layer1.toJson()];

// Hierarchical processing
const result = hierarchicalForward(query, layerEmbeddings, layers);
console.log('Final embedding:', result);
```

## API Reference

### RuvectorLayer

#### Constructor

```typescript
new RuvectorLayer(
  inputDim: number,
  hiddenDim: number,
  heads: number,
  dropout: number
): RuvectorLayer
```

#### Methods

- `forward(nodeEmbedding: number[], neighborEmbeddings: number[][], edgeWeights: number[]): number[]`
- `toJson(): string` - Serialize layer to JSON
- `fromJson(json: string): RuvectorLayer` - Deserialize layer from JSON

### TensorCompress

#### Constructor

```typescript
new TensorCompress(): TensorCompress
```

#### Methods

- `compress(embedding: number[], accessFreq: number): string` - Adaptive compression
- `compressWithLevel(embedding: number[], level: CompressionLevelConfig): string` - Explicit level
- `decompress(compressedJson: string): number[]` - Decompress tensor

#### CompressionLevelConfig

```typescript
interface CompressionLevelConfig {
  level_type: 'none' | 'half' | 'pq8' | 'pq4' | 'binary';
  scale?: number;           // For 'half'
  subvectors?: number;      // For 'pq8', 'pq4'
  centroids?: number;       // For 'pq8'
  outlier_threshold?: number; // For 'pq4'
  threshold?: number;       // For 'binary'
}
```

### Search Functions

#### differentiableSearch

```typescript
function differentiableSearch(
  query: number[],
  candidateEmbeddings: number[][],
  k: number,
  temperature: number
): { indices: number[], weights: number[] }
```

#### hierarchicalForward

```typescript
function hierarchicalForward(
  query: number[],
  layerEmbeddings: number[][][],
  gnnLayersJson: string[]
): number[]
```

### Utility Functions

#### getCompressionLevel

```typescript
function getCompressionLevel(accessFreq: number): string
```

Returns the compression level that would be selected for the given access frequency:
- `accessFreq > 0.8`: "none" (hot data)
- `accessFreq > 0.4`: "half" (warm data)
- `accessFreq > 0.1`: "pq8" (cool data)
- `accessFreq > 0.01`: "pq4" (cold data)
- `accessFreq <= 0.01`: "binary" (archive)

## Compression Levels

### None
Full precision, no compression. Best for frequently accessed data.

### Half Precision
~50% space savings with minimal quality loss. Good for warm data.

### PQ8 (8-bit Product Quantization)
~8x compression using 8-bit codes. Suitable for cool data.

### PQ4 (4-bit Product Quantization)
~16x compression with outlier handling. For cold data.

### Binary
~32x compression, values become +1/-1. For archival data.

## Performance

- **Zero-copy operations** where possible
- **SIMD optimizations** for vector operations
- **Parallel processing** with Rayon
- **Native performance** with Rust backend

## Building from Source

```bash
# Install dependencies
npm install

# Build debug
npm run build:debug

# Build release
npm run build

# Run tests
npm test
```

## License

MIT - See LICENSE file for details

## Contributing

Contributions are welcome! Please see the main Ruvector repository for guidelines.

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Documentation](https://docs.ruvector.io)
- [Issues](https://github.com/ruvnet/ruvector/issues)
