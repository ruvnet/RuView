# Router WASM

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![npm version](https://img.shields.io/npm/v/router-wasm.svg)](https://www.npmjs.com/package/router-wasm)
[![Bundle Size](https://img.shields.io/bundlephobia/minzip/router-wasm)](https://bundlephobia.com/package/router-wasm)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-✓-654FF0.svg)](https://webassembly.org/)

**WebAssembly bindings for intelligent neural routing and vector search in the browser.**

> Bring powerful vector database capabilities to the client-side. Run sub-millisecond vector search entirely in the browser with **zero server dependencies**. Perfect for edge computing, offline AI, and privacy-first applications.

## 🌟 Why Router WASM?

Traditional vector databases require backend infrastructure and constant network connectivity. **Router WASM changes that.**

### The Browser-First Advantage

- ⚡ **Zero Latency**: No network roundtrips—search happens entirely in the browser
- 🔒 **Privacy First**: User data never leaves the device
- 🌐 **Offline Capable**: Full functionality without internet connection
- 💰 **Cost Effective**: Eliminate backend infrastructure and API costs
- 🚀 **Edge Computing**: Deploy intelligent routing to CDN edge nodes
- 📦 **Small Bundle**: Optimized WASM binary for fast page loads

## 🚀 Features

### Core Capabilities

- **Client-Side Vector Search**: Sub-millisecond similarity search in the browser
- **Neural Routing**: Intelligent request routing and pattern matching
- **Multiple Distance Metrics**: Euclidean, Cosine, Dot Product, Manhattan
- **HNSW Indexing**: Fast approximate nearest neighbor search
- **Memory Efficient**: Optimized for browser memory constraints
- **TypeScript Support**: Full type definitions included
- **Framework Agnostic**: Works with React, Vue, Svelte, vanilla JS
- **Web Worker Ready**: Run computations off the main thread

### Browser-Specific Optimizations

- **SIMD Acceleration**: Hardware-accelerated vector operations where available
- **Progressive Loading**: Load and initialize asynchronously
- **Lazy Initialization**: Initialize only when needed
- **Small Footprint**: <100KB gzipped WASM binary
- **Memory Pooling**: Efficient memory management for long-running sessions
- **IndexedDB Integration**: Persist vector data locally

## 📦 Installation

### NPM/Yarn

```bash
# Using npm
npm install router-wasm

# Using yarn
yarn add router-wasm

# Using pnpm
pnpm add router-wasm
```

### CDN (Unpkg)

```html
<script type="module">
  import init, { VectorDB } from 'https://unpkg.com/router-wasm/router_wasm.js';

  await init();
  const db = new VectorDB(128);
</script>
```

## ⚡ Quick Start

### Basic Usage (ES Modules)

```javascript
import init, { VectorDB, DistanceMetric } from 'router-wasm';

// Initialize WASM module (only once)
await init();

// Create a vector database with 128 dimensions
const db = new VectorDB(128);

// Insert vectors
db.insert('doc1', new Float32Array([0.1, 0.2, 0.3, /* ... 125 more */]));
db.insert('doc2', new Float32Array([0.4, 0.5, 0.6, /* ... 125 more */]));
db.insert('doc3', new Float32Array([0.7, 0.8, 0.9, /* ... 125 more */]));

// Search for similar vectors
const query = new Float32Array([0.15, 0.25, 0.35, /* ... 125 more */]);
const results = db.search(query, 5);  // Top 5 results

// Process results
for (const result of results) {
  console.log(`ID: ${result.id}, Score: ${result.score}`);
}

// Get collection size
console.log(`Total vectors: ${db.count()}`);

// Delete a vector
db.delete('doc2');
```

### TypeScript Support

```typescript
import init, { VectorDB, DistanceMetric } from 'router-wasm';

interface SearchResult {
  id: string;
  score: number;
}

async function initializeVectorSearch(): Promise<VectorDB> {
  // Initialize WASM
  await init();

  // Create database with 384 dimensions (e.g., for sentence embeddings)
  const db = new VectorDB(384);

  return db;
}

async function semanticSearch(
  db: VectorDB,
  queryEmbedding: Float32Array,
  topK: number = 10
): Promise<SearchResult[]> {
  const results = db.search(queryEmbedding, topK);
  return results;
}
```

### React Integration

```jsx
import React, { useState, useEffect } from 'react';
import init, { VectorDB } from 'router-wasm';

function VectorSearchApp() {
  const [db, setDb] = useState(null);
  const [loading, setLoading] = useState(true);
  const [results, setResults] = useState([]);

  useEffect(() => {
    async function initialize() {
      await init();
      const vectorDb = new VectorDB(128);

      // Populate with sample data
      vectorDb.insert('item1', new Float32Array(128).fill(0.1));
      vectorDb.insert('item2', new Float32Array(128).fill(0.5));

      setDb(vectorDb);
      setLoading(false);
    }

    initialize();
  }, []);

  const handleSearch = async (queryVector) => {
    if (!db) return;

    const searchResults = db.search(queryVector, 10);
    setResults(searchResults);
  };

  if (loading) return <div>Loading vector database...</div>;

  return (
    <div>
      <h1>Client-Side Vector Search</h1>
      <button onClick={() => handleSearch(new Float32Array(128).fill(0.2))}>
        Search
      </button>
      <ul>
        {results.map(r => (
          <li key={r.id}>
            {r.id}: {r.score.toFixed(4)}
          </li>
        ))}
      </ul>
    </div>
  );
}

export default VectorSearchApp;
```

### Vue 3 Integration

```vue
<template>
  <div>
    <h1>Vector Search</h1>
    <input v-model="searchQuery" @input="handleSearch" placeholder="Search..." />
    <ul>
      <li v-for="result in results" :key="result.id">
        {{ result.id }}: {{ result.score.toFixed(4) }}
      </li>
    </ul>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import init, { VectorDB } from 'router-wasm';

const db = ref(null);
const searchQuery = ref('');
const results = ref([]);

onMounted(async () => {
  await init();
  db.value = new VectorDB(128);

  // Populate database
  db.value.insert('doc1', new Float32Array(128).fill(0.1));
  db.value.insert('doc2', new Float32Array(128).fill(0.5));
});

const handleSearch = () => {
  if (!db.value || !searchQuery.value) return;

  // Convert query to embedding (simplified example)
  const queryVector = new Float32Array(128).fill(parseFloat(searchQuery.value) || 0);
  results.value = db.value.search(queryVector, 5);
};
</script>
```

## 🎯 Use Cases

### Client-Side AI Applications

**Semantic Search in the Browser**
```javascript
// RAG (Retrieval Augmented Generation) in the browser
import init, { VectorDB } from 'router-wasm';
import { generateEmbedding } from './embeddings';  // Your embedding model

await init();
const knowledgeBase = new VectorDB(384);

// Index documents
const docs = [
  { id: 'doc1', text: 'Rust is a systems programming language' },
  { id: 'doc2', text: 'WebAssembly enables near-native performance' },
  { id: 'doc3', text: 'Vector databases power semantic search' }
];

for (const doc of docs) {
  const embedding = await generateEmbedding(doc.text);
  knowledgeBase.insert(doc.id, embedding);
}

// Query with natural language
const queryEmbedding = await generateEmbedding('What is WASM?');
const relevantDocs = knowledgeBase.search(queryEmbedding, 3);
```

**Offline Recommender System**
```javascript
// Product recommendations without backend
const productDb = new VectorDB(256);

// Index product features
products.forEach(product => {
  const featureVector = extractFeatures(product);
  productDb.insert(product.id, featureVector);
});

// Get recommendations based on user preferences
const userPreferences = getUserPreferenceVector();
const recommendations = productDb.search(userPreferences, 10);
```

**Privacy-First Search**
```javascript
// Search user data without sending to server
const privateDb = new VectorDB(512);

// User data stays in browser
userDocuments.forEach(doc => {
  const embedding = embedDocument(doc);
  privateDb.insert(doc.id, embedding);
});

// All searches happen locally
const results = privateDb.search(queryEmbedding, 20);
```

### Edge Computing & CDN

**Cloudflare Workers**
```javascript
// Deploy to Cloudflare Workers
import init, { VectorDB } from 'router-wasm';

export default {
  async fetch(request, env, ctx) {
    await init();

    const db = new VectorDB(128);
    // Load pre-computed vectors from KV store
    const vectors = await env.VECTORS.get('index', 'json');

    for (const [id, vector] of Object.entries(vectors)) {
      db.insert(id, new Float32Array(vector));
    }

    // Handle search at edge
    const { query } = await request.json();
    const results = db.search(new Float32Array(query), 10);

    return new Response(JSON.stringify(results), {
      headers: { 'content-type': 'application/json' }
    });
  }
};
```

**Deno Deploy**
```typescript
// Edge function with vector search
import init, { VectorDB } from 'https://esm.sh/router-wasm';

Deno.serve(async (req) => {
  await init();

  const db = new VectorDB(256);
  // Your edge routing logic

  return new Response('OK');
});
```

### Web Workers

```javascript
// worker.js - Run vector search off main thread
import init, { VectorDB } from 'router-wasm';

let db = null;

self.addEventListener('message', async (e) => {
  const { type, payload } = e.data;

  if (type === 'init') {
    await init();
    db = new VectorDB(payload.dimensions);
    self.postMessage({ type: 'ready' });
  }

  if (type === 'insert') {
    db.insert(payload.id, new Float32Array(payload.vector));
    self.postMessage({ type: 'inserted', id: payload.id });
  }

  if (type === 'search') {
    const results = db.search(new Float32Array(payload.query), payload.k);
    self.postMessage({ type: 'results', data: results });
  }
});
```

```javascript
// main.js - Use the worker
const worker = new Worker('worker.js', { type: 'module' });

worker.postMessage({ type: 'init', payload: { dimensions: 128 } });

worker.addEventListener('message', (e) => {
  if (e.data.type === 'ready') {
    console.log('Vector DB ready in worker');

    // Insert data
    worker.postMessage({
      type: 'insert',
      payload: { id: 'doc1', vector: new Array(128).fill(0.1) }
    });

    // Search
    worker.postMessage({
      type: 'search',
      payload: { query: new Array(128).fill(0.2), k: 5 }
    });
  }

  if (e.data.type === 'results') {
    console.log('Search results:', e.data.data);
  }
});
```

## 🔧 Advanced Features

### Persistent Storage (IndexedDB)

```javascript
import init, { VectorDB } from 'router-wasm';

// Initialize with persistent storage path
await init();
const db = new VectorDB(128, 'my-vector-store');

// Data persists across sessions
db.insert('doc1', new Float32Array(128));

// Reload in future session
const db2 = new VectorDB(128, 'my-vector-store');
console.log(db2.count());  // Previously inserted data is available
```

### Distance Metrics

```javascript
import { VectorDB, DistanceMetric } from 'router-wasm';

const db = new VectorDB(128);

// Different similarity measures available:
// - DistanceMetric.Euclidean (L2 distance)
// - DistanceMetric.Cosine (cosine similarity)
// - DistanceMetric.DotProduct (dot product)
// - DistanceMetric.Manhattan (L1 distance)

// Note: Distance metric is set at index build time in router-core
```

### Batch Operations

```javascript
// Efficient bulk insertion
const vectors = [
  { id: 'doc1', vector: new Float32Array(128).fill(0.1) },
  { id: 'doc2', vector: new Float32Array(128).fill(0.2) },
  { id: 'doc3', vector: new Float32Array(128).fill(0.3) },
];

vectors.forEach(({ id, vector }) => db.insert(id, vector));

// Batch search (multiple queries)
const queries = [
  new Float32Array(128).fill(0.15),
  new Float32Array(128).fill(0.25),
];

const allResults = queries.map(query => db.search(query, 5));
```

### Memory Management

```javascript
// Check collection size
const count = db.count();
console.log(`Vectors in database: ${count}`);

// Clean up when done (especially important in SPAs)
// Note: Drop the reference and let garbage collector handle it
db = null;

// For explicit cleanup in long-running apps
function cleanupVectorDb(db) {
  const ids = getAllIds();  // Your tracking logic
  ids.forEach(id => db.delete(id));
}
```

## 📊 Performance Optimization

### Bundle Size Optimization

**Tree Shaking**
```javascript
// Import only what you need
import init, { VectorDB } from 'router-wasm';
// Don't import unused distance metrics or types
```

**Code Splitting**
```javascript
// Lazy load WASM module
const loadVectorDB = async () => {
  const { default: init, VectorDB } = await import('router-wasm');
  await init();
  return VectorDB;
};

// Use when needed
button.addEventListener('click', async () => {
  const VectorDB = await loadVectorDB();
  const db = new VectorDB(128);
});
```

**Webpack Configuration**
```javascript
// webpack.config.js
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
  optimization: {
    splitChunks: {
      chunks: 'all',
    },
  },
};
```

### Runtime Performance

**Pre-compute Embeddings**
```javascript
// Generate embeddings server-side or during build
// Ship pre-computed vectors to reduce client computation
const precomputedVectors = await fetch('/vectors.json').then(r => r.json());

await init();
const db = new VectorDB(128);

for (const [id, vector] of Object.entries(precomputedVectors)) {
  db.insert(id, new Float32Array(vector));
}
```

**Dimension Reduction**
```javascript
// Use lower dimensions for faster search
// 128 or 256 dimensions often sufficient for many use cases
const db = new VectorDB(128);  // Instead of 384 or 768

// Consider PCA or other dimensionality reduction techniques
```

**Limit Result Sets**
```javascript
// Request only what you need
const results = db.search(query, 10);  // Top 10, not 100

// Implement pagination if needed
function paginatedSearch(query, page = 0, pageSize = 10) {
  const allResults = db.search(query, (page + 1) * pageSize);
  return allResults.slice(page * pageSize, (page + 1) * pageSize);
}
```

## 🔨 Building from Source

### Prerequisites

- **Rust**: 1.77 or higher
- **wasm-pack**: `cargo install wasm-pack`
- **Node.js**: 18.0 or higher (for testing)

### Build Commands

```bash
# Clone repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/crates/router-wasm

# Build for web (ES modules)
wasm-pack build --target web --release

# Build for Node.js
wasm-pack build --target nodejs --release

# Build for bundlers (webpack, etc.)
wasm-pack build --target bundler --release

# Build with optimizations
wasm-pack build --target web --release -- --features simd

# Run tests
wasm-pack test --headless --chrome
```

### Build Output

After building, the `pkg/` directory contains:

```
pkg/
├── router_wasm.js          # JavaScript bindings
├── router_wasm.d.ts        # TypeScript definitions
├── router_wasm_bg.wasm     # WebAssembly binary
├── router_wasm_bg.wasm.d.ts
└── package.json            # NPM package metadata
```

### Custom Build Profiles

```toml
# Cargo.toml - Already optimized for size
[profile.release]
opt-level = "z"              # Optimize for size
lto = true                   # Link-time optimization
codegen-units = 1            # Better optimization
panic = "abort"              # Smaller binary
```

## 🌐 Browser Compatibility

| Browser | Version | WASM | SIMD | Notes |
|---------|---------|------|------|-------|
| **Chrome** | 87+ | ✅ | ✅ | Full support |
| **Firefox** | 89+ | ✅ | ✅ | Full support |
| **Safari** | 15+ | ✅ | ⚠️ | WASM SIMD in 16.4+ |
| **Edge** | 87+ | ✅ | ✅ | Full support |
| **Opera** | 73+ | ✅ | ✅ | Full support |
| **Mobile Safari** | 15+ | ✅ | ⚠️ | Limited SIMD |
| **Mobile Chrome** | 87+ | ✅ | ✅ | Full support |

**Notes**:
- ✅ Full support
- ⚠️ Partial support (SIMD acceleration may not be available)
- All modern browsers support WebAssembly
- SIMD provides 2-4x performance boost where available

## 🔗 Integration with Ruvector Ecosystem

### With ruvector-wasm

```javascript
import initRouter, { VectorDB as RouterDB } from 'router-wasm';
import initRuvector, { VectorDB } from 'ruvector-wasm';

// Initialize both modules
await Promise.all([initRouter(), initRuvector()]);

// Router WASM: Intelligent routing and pattern matching
const router = new RouterDB(128);

// Ruvector WASM: Full-featured vector database
const vectorDb = new VectorDB(128);

// Use together for advanced use cases
```

### With Node.js Backend

```javascript
// Frontend (router-wasm)
import init, { VectorDB } from 'router-wasm';
await init();
const clientDb = new VectorDB(128);

// Backend (ruvector Node.js bindings)
const { VectorDB } = require('ruvector');
const serverDb = new VectorDB();

// Hybrid architecture: Local search + server sync
```

## 📚 API Reference

### VectorDB

```typescript
class VectorDB {
  /**
   * Create a new vector database
   * @param dimensions - Vector dimensionality (e.g., 128, 256, 384, 768)
   * @param storage_path - Optional persistent storage path
   */
  constructor(dimensions: number, storage_path?: string);

  /**
   * Insert a vector into the database
   * @param id - Unique identifier
   * @param vector - Float32Array of specified dimensions
   * @returns The inserted ID
   */
  insert(id: string, vector: Float32Array): string;

  /**
   * Search for similar vectors
   * @param vector - Query vector
   * @param k - Number of results to return
   * @returns Array of search results with id and score
   */
  search(vector: Float32Array, k: number): SearchResult[];

  /**
   * Delete a vector by ID
   * @param id - ID to delete
   * @returns true if deleted, false if not found
   */
  delete(id: string): boolean;

  /**
   * Get total number of vectors
   * @returns Vector count
   */
  count(): number;
}
```

### Types

```typescript
interface SearchResult {
  id: string;
  score: number;
}

enum DistanceMetric {
  Euclidean,
  Cosine,
  DotProduct,
  Manhattan
}
```

## 🎓 Examples

### Complete RAG Application

See [examples/browser-rag](../../examples/browser-rag/) for a full-featured Retrieval Augmented Generation application running entirely in the browser.

### Product Search

See [examples/product-search](../../examples/product-search/) for an offline product recommendation system.

### Edge Routing

See [examples/edge-routing](../../examples/edge-routing/) for Cloudflare Workers integration.

## 🐛 Troubleshooting

### WASM Module Not Loading

```javascript
// Ensure init() is called before creating VectorDB
import init, { VectorDB } from 'router-wasm';

// ❌ Wrong
const db = new VectorDB(128);  // Error: WASM not initialized

// ✅ Correct
await init();
const db = new VectorDB(128);
```

### Large Bundle Size

```javascript
// Use dynamic imports for code splitting
const { default: init, VectorDB } = await import('router-wasm');
await init();
```

### Memory Errors in Browser

```javascript
// Reduce dimensions or limit database size
const db = new VectorDB(128);  // Instead of 768

// Clear vectors periodically in long-running apps
if (db.count() > 10000) {
  // Implement your pruning logic
  oldIds.forEach(id => db.delete(id));
}
```

### TypeScript Errors

```typescript
// Ensure TypeScript can find declarations
// tsconfig.json
{
  "compilerOptions": {
    "moduleResolution": "node",
    "types": ["router-wasm"]
  }
}
```

## 📖 Documentation

- **[Quick Start Guide](../../docs/guide/GETTING_STARTED.md)** - Get started in 5 minutes
- **[WASM API Reference](../../docs/getting-started/wasm-api.md)** - Complete API documentation
- **[Performance Tuning](../../docs/optimization/PERFORMANCE_TUNING_GUIDE.md)** - Optimization strategies
- **[Main README](../../README.md)** - Ruvector ecosystem overview

## 🤝 Contributing

Contributions are welcome! See [Contributing Guidelines](../../docs/development/CONTRIBUTING.md).

### Development Setup

```bash
# Clone and setup
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/crates/router-wasm

# Build
wasm-pack build --target web

# Test
wasm-pack test --headless --chrome --firefox

# Format
cargo fmt

# Lint
cargo clippy -- -D warnings
```

## 📜 License

**MIT License** - see [LICENSE](../../LICENSE) for details.

## 🙏 Acknowledgments

Built with:
- **wasm-bindgen**: Rust/JavaScript interop
- **router-core**: High-performance vector routing engine
- **HNSW**: Fast approximate nearest neighbor search
- **SIMD**: Hardware-accelerated vector operations

## 🌐 Links

- **GitHub**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **NPM**: [npmjs.com/package/router-wasm](https://www.npmjs.com/package/router-wasm)
- **Documentation**: [ruvector docs](../../docs/README.md)
- **Discord**: [Join community](https://discord.gg/ruvnet)
- **Website**: [ruv.io](https://ruv.io)

---

<div align="center">

**Built by [rUv](https://ruv.io) • Part of [Ruvector](../../README.md) • MIT Licensed**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)
[![Follow @ruvnet](https://img.shields.io/twitter/follow/ruvnet?style=social)](https://twitter.com/ruvnet)

**Browser-First Vector Search** | **Zero Backend Required** | **Privacy First**

[Get Started](../../docs/guide/GETTING_STARTED.md) • [Documentation](../../docs/README.md) • [Examples](../../examples/)

</div>
