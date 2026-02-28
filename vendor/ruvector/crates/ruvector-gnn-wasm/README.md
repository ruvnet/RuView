# RuVector GNN WASM

WebAssembly bindings for RuVector Graph Neural Network operations.

## Features

- **GNN Layer Operations**: Multi-head attention, GRU updates, layer normalization
- **Tensor Compression**: Adaptive compression based on access frequency
- **Differentiable Search**: Soft attention-based similarity search
- **Hierarchical Forward**: Multi-layer GNN processing

## Installation

```bash
npm install ruvector-gnn-wasm
```

## Usage

### Initialize

```typescript
import init, {
  JsRuvectorLayer,
  JsTensorCompress,
  differentiableSearch,
  SearchConfig
} from 'ruvector-gnn-wasm';

await init();
```

### GNN Layer

```typescript
// Create a GNN layer
const layer = new JsRuvectorLayer(
  4,    // input dimension
  8,    // hidden dimension
  2,    // number of attention heads
  0.1   // dropout rate
);

// Forward pass
const nodeEmbedding = new Float32Array([1.0, 2.0, 3.0, 4.0]);
const neighbors = [
  new Float32Array([0.5, 1.0, 1.5, 2.0]),
  new Float32Array([2.0, 3.0, 4.0, 5.0])
];
const edgeWeights = new Float32Array([0.3, 0.7]);

const output = layer.forward(nodeEmbedding, neighbors, edgeWeights);
console.log('Output dimension:', layer.outputDim);
```

### Tensor Compression

```typescript
const compressor = new JsTensorCompress();

// Compress based on access frequency
const embedding = new Float32Array(128).fill(0.5);
const compressed = compressor.compress(embedding, 0.5); // 50% access frequency

// Decompress
const decompressed = compressor.decompress(compressed);

// Or specify compression level explicitly
const compressedPQ8 = compressor.compressWithLevel(embedding, "pq8");

// Get compression ratio
const ratio = compressor.getCompressionRatio(0.5); // Returns ~2.0 for half precision
```

### Compression Levels

Access frequency determines compression:
- `f > 0.8`: **Full precision** (no compression) - hot data
- `f > 0.4`: **Half precision** (2x compression) - warm data
- `f > 0.1`: **8-bit PQ** (4x compression) - cool data
- `f > 0.01`: **4-bit PQ** (8x compression) - cold data
- `f <= 0.01`: **Binary** (32x compression) - archive data

### Differentiable Search

```typescript
const query = new Float32Array([1.0, 0.0, 0.0]);
const candidates = [
  new Float32Array([1.0, 0.0, 0.0]),  // Perfect match
  new Float32Array([0.9, 0.1, 0.0]),  // Close match
  new Float32Array([0.0, 1.0, 0.0])   // Orthogonal
];

const config = new SearchConfig(2, 1.0); // k=2, temperature=1.0
const result = differentiableSearch(query, candidates, config);

console.log('Top indices:', result.indices);
console.log('Weights:', result.weights);
```

## API Reference

### `JsRuvectorLayer`

```typescript
class JsRuvectorLayer {
  constructor(
    inputDim: number,
    hiddenDim: number,
    heads: number,
    dropout: number
  );

  forward(
    nodeEmbedding: Float32Array,
    neighborEmbeddings: Float32Array[],
    edgeWeights: Float32Array
  ): Float32Array;

  readonly outputDim: number;
}
```

### `JsTensorCompress`

```typescript
class JsTensorCompress {
  constructor();

  compress(embedding: Float32Array, accessFreq: number): object;
  compressWithLevel(embedding: Float32Array, level: string): object;
  decompress(compressed: object): Float32Array;
  getCompressionRatio(accessFreq: number): number;
}
```

Compression levels: `"none"`, `"half"`, `"pq8"`, `"pq4"`, `"binary"`

### `differentiableSearch`

```typescript
function differentiableSearch(
  query: Float32Array,
  candidateEmbeddings: Float32Array[],
  config: SearchConfig
): { indices: number[], weights: number[] };
```

### `SearchConfig`

```typescript
class SearchConfig {
  constructor(k: number, temperature: number);
  k: number;          // Number of results
  temperature: number; // Softmax temperature (lower = sharper)
}
```

### `cosineSimilarity`

```typescript
function cosineSimilarity(a: Float32Array, b: Float32Array): number;
```

## Building from Source

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build for Node.js
wasm-pack build --target nodejs

# Build for browser
wasm-pack build --target web

# Build for bundler (webpack, etc.)
wasm-pack build --target bundler
```

## Performance

- GNN layers use efficient attention mechanisms
- Compression reduces memory usage by 2-32x
- All operations are optimized for WASM
- No garbage collection during forward passes

## License

MIT
