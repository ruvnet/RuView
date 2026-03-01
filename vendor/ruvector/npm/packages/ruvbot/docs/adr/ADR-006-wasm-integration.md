# ADR-006: WASM Integration

**Status:** Accepted
**Date:** 2026-01-27
**Decision Makers:** RuVector Architecture Team
**Technical Area:** Runtime, Performance

---

## Context and Problem Statement

RuvBot requires high-performance vector operations and ML inference for:

1. **Embedding generation** for memory storage and retrieval
2. **HNSW search** for semantic memory recall
3. **Pattern matching** for learned response optimization
4. **Quantization** for memory-efficient vector storage

The runtime must support:

- **Server-side Node.js** for API workloads
- **Edge deployments** (Cloudflare Workers, Vercel Edge)
- **Browser execution** for client-side features
- **Fallback paths** when WASM is unavailable

---

## Decision Drivers

### Performance Requirements

| Operation | Target Latency | Environment |
|-----------|----------------|-------------|
| Embed single text | < 10ms | WASM |
| Embed batch (32) | < 100ms | WASM |
| HNSW search k=10 | < 5ms | Native/WASM |
| Quantize vector | < 1ms | WASM |
| Pattern match | < 20ms | WASM |

### Compatibility Requirements

| Environment | WASM Support | Native Support |
|-------------|--------------|----------------|
| Node.js 18+ | Full | Full (NAPI) |
| Node.js 14-17 | Partial | Full (NAPI) |
| Cloudflare Workers | Full | None |
| Vercel Edge | Full | None |
| Browser (Chrome/FF/Safari) | Full | None |
| Deno | Full | Partial |

---

## Decision Outcome

### Adopt Hybrid WASM/Native Runtime with Automatic Detection

We implement a runtime abstraction that:

1. **Detects environment** at initialization
2. **Prefers native bindings** when available (2-5x faster)
3. **Falls back to WASM** universally
4. **Provides consistent API** regardless of backend

```
+-----------------------------------------------------------------------------+
|                           WASM INTEGRATION LAYER                             |
+-----------------------------------------------------------------------------+

                    +---------------------------+
                    |    Runtime Detector       |
                    +-------------+-------------+
                                  |
            +---------------------+---------------------+
            |                                           |
+-----------v-----------+                   +-----------v-----------+
|    Native Backend     |                   |     WASM Backend      |
|    (NAPI-RS)          |                   |    (wasm-bindgen)     |
|-----------------------|                   |-----------------------|
| - @ruvector/core      |                   | - @ruvector/wasm      |
| - @ruvector/ruvllm    |                   | - @ruvllm-wasm        |
| - @ruvector/sona      |                   | - @sona-wasm          |
+-----------+-----------+                   +-----------+-----------+
            |                                           |
            +---------------------+---------------------+
                                  |
                    +-------------v-------------+
                    |   Unified API Surface     |
                    |   (RuVectorRuntime)       |
                    +---------------------------+
```

---

## WASM Module Architecture

### Module Organization

```typescript
// WASM module types available
interface WasmModules {
  // Vector operations
  vectorOps: {
    distance: (a: Float32Array, b: Float32Array, metric: DistanceMetric) => number;
    batchDistance: (query: Float32Array, vectors: Float32Array[], metric: DistanceMetric) => Float32Array;
    normalize: (vector: Float32Array) => Float32Array;
    quantize: (vector: Float32Array, config: QuantizationConfig) => Uint8Array;
    dequantize: (quantized: Uint8Array, config: QuantizationConfig) => Float32Array;
  };

  // HNSW index
  hnsw: {
    create: (config: HnswConfig) => HnswIndexHandle;
    insert: (handle: HnswIndexHandle, id: string, vector: Float32Array) => void;
    search: (handle: HnswIndexHandle, query: Float32Array, k: number) => SearchResult[];
    delete: (handle: HnswIndexHandle, id: string) => boolean;
    serialize: (handle: HnswIndexHandle) => Uint8Array;
    deserialize: (data: Uint8Array) => HnswIndexHandle;
    free: (handle: HnswIndexHandle) => void;
  };

  // Embeddings
  embeddings: {
    loadModel: (modelPath: string) => EmbeddingModelHandle;
    embed: (handle: EmbeddingModelHandle, text: string) => Float32Array;
    embedBatch: (handle: EmbeddingModelHandle, texts: string[]) => Float32Array[];
    unloadModel: (handle: EmbeddingModelHandle) => void;
  };

  // Learning
  learning: {
    createPattern: (embedding: Float32Array, metadata: unknown) => PatternHandle;
    matchPatterns: (query: Float32Array, patterns: PatternHandle[], threshold: number) => PatternMatch[];
    trainLoRA: (trajectories: Trajectory[], config: LoRAConfig) => LoRAWeights;
    applyEWC: (weights: ModelWeights, fisher: FisherMatrix, lambda: number) => ModelWeights;
  };
}
```

### Runtime Detection

```typescript
// Automatic runtime detection and initialization
class RuVectorRuntime {
  private static instance: RuVectorRuntime | null = null;
  private backend: 'native' | 'wasm' | 'js-fallback';
  private modules: WasmModules | NativeModules;

  private constructor() {}

  static async initialize(): Promise<RuVectorRuntime> {
    if (this.instance) return this.instance;

    const runtime = new RuVectorRuntime();
    await runtime.detectAndLoad();
    this.instance = runtime;
    return runtime;
  }

  private async detectAndLoad(): Promise<void> {
    // Try native first (best performance)
    if (await this.tryNative()) {
      this.backend = 'native';
      console.log('RuVector: Using native NAPI backend');
      return;
    }

    // Try WASM
    if (await this.tryWasm()) {
      this.backend = 'wasm';
      console.log('RuVector: Using WASM backend');
      return;
    }

    // Fall back to pure JS (limited functionality)
    this.backend = 'js-fallback';
    console.warn('RuVector: Using JS fallback (limited performance)');
    await this.loadJsFallback();
  }

  private async tryNative(): Promise<boolean> {
    // Native only available in Node.js
    if (typeof process === 'undefined' || !process.versions?.node) {
      return false;
    }

    try {
      const nativeModule = await import('@ruvector/core');
      if (typeof nativeModule.isNativeAvailable === 'function' &&
          nativeModule.isNativeAvailable()) {
        this.modules = nativeModule;
        return true;
      }
    } catch (e) {
      console.debug('Native module not available:', e);
    }

    return false;
  }

  private async tryWasm(): Promise<boolean> {
    try {
      // Check WebAssembly support
      if (typeof WebAssembly !== 'object') {
        return false;
      }

      // Load WASM modules
      const [vectorOps, hnsw, embeddings, learning] = await Promise.all([
        import('@ruvector/wasm'),
        import('@ruvector/wasm/hnsw'),
        import('@ruvector/wasm/embeddings'),
        import('@ruvector/wasm/learning'),
      ]);

      // Initialize WASM modules
      await Promise.all([
        vectorOps.default(),
        hnsw.default(),
        embeddings.default(),
        learning.default(),
      ]);

      this.modules = {
        vectorOps,
        hnsw,
        embeddings,
        learning,
      };

      return true;
    } catch (e) {
      console.debug('WASM modules not available:', e);
      return false;
    }
  }

  private async loadJsFallback(): Promise<void> {
    // Pure JS implementations (slower but always work)
    const { JsFallbackModules } = await import('./js-fallback');
    this.modules = new JsFallbackModules();
  }

  getBackend(): 'native' | 'wasm' | 'js-fallback' {
    return this.backend;
  }

  getModules(): WasmModules | NativeModules {
    if (!this.modules) {
      throw new Error('RuVector runtime not initialized');
    }
    return this.modules;
  }
}
```

---

## Embedding Engine

### WASM Embedder

```typescript
// WASM-based embedding engine
class WasmEmbedder {
  private modelHandle: EmbeddingModelHandle | null = null;
  private modelPath: string;
  private dimensions: number;
  private runtime: RuVectorRuntime;

  constructor(config: EmbedderConfig) {
    this.modelPath = config.modelPath;
    this.dimensions = config.dimensions ?? 384;
  }

  async initialize(): Promise<void> {
    this.runtime = await RuVectorRuntime.initialize();
    const { embeddings } = this.runtime.getModules();

    // Load model (downloads and caches if needed)
    const modelData = await this.loadModelData();
    this.modelHandle = embeddings.loadModel(modelData);
  }

  async embed(text: string): Promise<Float32Array> {
    if (!this.modelHandle) {
      throw new Error('Embedder not initialized');
    }

    const { embeddings } = this.runtime.getModules();
    return embeddings.embed(this.modelHandle, text);
  }

  async embedBatch(texts: string[]): Promise<Float32Array[]> {
    if (!this.modelHandle) {
      throw new Error('Embedder not initialized');
    }

    const { embeddings } = this.runtime.getModules();

    // Process in chunks to avoid OOM
    const chunkSize = 32;
    const results: Float32Array[] = [];

    for (let i = 0; i < texts.length; i += chunkSize) {
      const chunk = texts.slice(i, i + chunkSize);
      const chunkResults = embeddings.embedBatch(this.modelHandle, chunk);
      results.push(...chunkResults);
    }

    return results;
  }

  getDimensions(): number {
    return this.dimensions;
  }

  async dispose(): Promise<void> {
    if (this.modelHandle) {
      const { embeddings } = this.runtime.getModules();
      embeddings.unloadModel(this.modelHandle);
      this.modelHandle = null;
    }
  }

  private async loadModelData(): Promise<Uint8Array> {
    // Check cache first
    const cached = await this.modelCache.get(this.modelPath);
    if (cached) return cached;

    // Download model
    const response = await fetch(this.modelPath);
    const buffer = await response.arrayBuffer();
    const data = new Uint8Array(buffer);

    // Cache for future use
    await this.modelCache.set(this.modelPath, data);

    return data;
  }
}
```

### Model Cache

```typescript
// Cross-environment model cache
class ModelCache {
  private memoryCache: Map<string, Uint8Array> = new Map();

  async get(key: string): Promise<Uint8Array | null> {
    // Check memory cache first
    if (this.memoryCache.has(key)) {
      return this.memoryCache.get(key)!;
    }

    // Try persistent cache (environment-specific)
    if (typeof caches !== 'undefined') {
      // Browser/Cloudflare Cache API
      return this.getFromCacheAPI(key);
    } else if (typeof process !== 'undefined' && process.versions?.node) {
      // Node.js file system cache
      return this.getFromFileCache(key);
    }

    return null;
  }

  async set(key: string, data: Uint8Array): Promise<void> {
    // Always store in memory
    this.memoryCache.set(key, data);

    // Persist to appropriate cache
    if (typeof caches !== 'undefined') {
      await this.setToCacheAPI(key, data);
    } else if (typeof process !== 'undefined' && process.versions?.node) {
      await this.setToFileCache(key, data);
    }
  }

  private async getFromCacheAPI(key: string): Promise<Uint8Array | null> {
    try {
      const cache = await caches.open('ruvector-models');
      const response = await cache.match(key);
      if (response) {
        const buffer = await response.arrayBuffer();
        return new Uint8Array(buffer);
      }
    } catch (e) {
      console.debug('Cache API error:', e);
    }
    return null;
  }

  private async setToCacheAPI(key: string, data: Uint8Array): Promise<void> {
    try {
      const cache = await caches.open('ruvector-models');
      const response = new Response(data, {
        headers: { 'Content-Type': 'application/octet-stream' },
      });
      await cache.put(key, response);
    } catch (e) {
      console.debug('Cache API error:', e);
    }
  }

  private async getFromFileCache(key: string): Promise<Uint8Array | null> {
    const fs = await import('fs/promises');
    const path = await import('path');
    const os = await import('os');

    const cacheDir = path.join(os.homedir(), '.ruvector', 'models');
    const cachePath = path.join(cacheDir, this.keyToFilename(key));

    try {
      const data = await fs.readFile(cachePath);
      return new Uint8Array(data);
    } catch (e) {
      return null;
    }
  }

  private async setToFileCache(key: string, data: Uint8Array): Promise<void> {
    const fs = await import('fs/promises');
    const path = await import('path');
    const os = await import('os');

    const cacheDir = path.join(os.homedir(), '.ruvector', 'models');
    await fs.mkdir(cacheDir, { recursive: true });

    const cachePath = path.join(cacheDir, this.keyToFilename(key));
    await fs.writeFile(cachePath, data);
  }

  private keyToFilename(key: string): string {
    const crypto = require('crypto');
    return crypto.createHash('sha256').update(key).digest('hex').slice(0, 32);
  }
}
```

---

## HNSW Index WASM Wrapper

```typescript
// WASM-based HNSW index
class WasmHnswIndex {
  private handle: HnswIndexHandle | null = null;
  private runtime: RuVectorRuntime;
  private config: HnswConfig;
  private vectorCount = 0;

  constructor(config: HnswConfig) {
    this.config = config;
  }

  async initialize(): Promise<void> {
    this.runtime = await RuVectorRuntime.initialize();
    const { hnsw } = this.runtime.getModules();
    this.handle = hnsw.create(this.config);
  }

  async insert(id: string, vector: Float32Array): Promise<void> {
    if (!this.handle) throw new Error('Index not initialized');

    // Validate dimensions
    if (vector.length !== this.config.dimensions) {
      throw new Error(`Vector dimension mismatch: ${vector.length} vs ${this.config.dimensions}`);
    }

    const { hnsw } = this.runtime.getModules();
    hnsw.insert(this.handle, id, vector);
    this.vectorCount++;
  }

  async insertBatch(entries: Array<{ id: string; vector: Float32Array }>): Promise<void> {
    if (!this.handle) throw new Error('Index not initialized');

    const { hnsw } = this.runtime.getModules();

    for (const entry of entries) {
      if (entry.vector.length !== this.config.dimensions) {
        throw new Error(`Vector dimension mismatch for ${entry.id}`);
      }
      hnsw.insert(this.handle, entry.id, entry.vector);
      this.vectorCount++;
    }
  }

  async search(query: Float32Array, k: number): Promise<SearchResult[]> {
    if (!this.handle) throw new Error('Index not initialized');

    if (query.length !== this.config.dimensions) {
      throw new Error(`Query dimension mismatch: ${query.length}`);
    }

    const { hnsw } = this.runtime.getModules();
    return hnsw.search(this.handle, query, Math.min(k, this.vectorCount));
  }

  async delete(id: string): Promise<boolean> {
    if (!this.handle) throw new Error('Index not initialized');

    const { hnsw } = this.runtime.getModules();
    const deleted = hnsw.delete(this.handle, id);
    if (deleted) this.vectorCount--;
    return deleted;
  }

  async serialize(): Promise<Uint8Array> {
    if (!this.handle) throw new Error('Index not initialized');

    const { hnsw } = this.runtime.getModules();
    return hnsw.serialize(this.handle);
  }

  async deserialize(data: Uint8Array): Promise<void> {
    const { hnsw } = this.runtime.getModules();

    // Free existing handle if any
    if (this.handle) {
      hnsw.free(this.handle);
    }

    this.handle = hnsw.deserialize(data);
  }

  getStats(): IndexStats {
    return {
      vectorCount: this.vectorCount,
      dimensions: this.config.dimensions,
      m: this.config.m,
      efConstruction: this.config.efConstruction,
      efSearch: this.config.efSearch,
      backend: this.runtime.getBackend(),
    };
  }

  async dispose(): Promise<void> {
    if (this.handle) {
      const { hnsw } = this.runtime.getModules();
      hnsw.free(this.handle);
      this.handle = null;
    }
  }
}

interface HnswConfig {
  dimensions: number;
  m: number;              // Max connections per node per layer
  efConstruction: number; // Build-time exploration factor
  efSearch: number;       // Query-time exploration factor
  distanceMetric: 'cosine' | 'euclidean' | 'dot_product';
}

interface SearchResult {
  id: string;
  score: number;
}

interface IndexStats {
  vectorCount: number;
  dimensions: number;
  m: number;
  efConstruction: number;
  efSearch: number;
  backend: 'native' | 'wasm' | 'js-fallback';
}
```

---

## Memory Management

### WASM Memory Pooling

```typescript
// Efficient memory management for WASM
class WasmMemoryPool {
  private pools: Map<number, Float32Array[]> = new Map();
  private maxPoolSize = 100;

  // Get or create a Float32Array of specified length
  acquire(length: number): Float32Array {
    const pool = this.pools.get(length);

    if (pool && pool.length > 0) {
      return pool.pop()!;
    }

    return new Float32Array(length);
  }

  // Return array to pool for reuse
  release(array: Float32Array): void {
    const length = array.length;
    let pool = this.pools.get(length);

    if (!pool) {
      pool = [];
      this.pools.set(length, pool);
    }

    if (pool.length < this.maxPoolSize) {
      // Zero out for security
      array.fill(0);
      pool.push(array);
    }
    // Otherwise let GC handle it
  }

  // Clear pools when memory pressure detected
  clear(): void {
    this.pools.clear();
  }

  getStats(): PoolStats {
    const stats: PoolStats = { totalArrays: 0, totalBytes: 0, pools: {} };

    for (const [length, pool] of this.pools) {
      stats.pools[length] = pool.length;
      stats.totalArrays += pool.length;
      stats.totalBytes += pool.length * length * 4; // 4 bytes per float32
    }

    return stats;
  }
}

// Usage in embedder
class PooledWasmEmbedder extends WasmEmbedder {
  private pool = new WasmMemoryPool();

  async embed(text: string): Promise<Float32Array> {
    const result = await super.embed(text);

    // Copy to pooled array
    const pooled = this.pool.acquire(result.length);
    pooled.set(result);

    return pooled;
  }

  releaseEmbedding(embedding: Float32Array): void {
    this.pool.release(embedding);
  }
}
```

---

## Performance Benchmarks

```typescript
// Benchmark suite for runtime comparison
class WasmBenchmarks {
  async runAll(): Promise<BenchmarkResults> {
    const results: BenchmarkResults = {};

    // Embedding benchmarks
    results.embedSingle = await this.benchmarkEmbedSingle();
    results.embedBatch = await this.benchmarkEmbedBatch();

    // HNSW benchmarks
    results.hnswInsert = await this.benchmarkHnswInsert();
    results.hnswSearch = await this.benchmarkHnswSearch();

    // Vector operations
    results.distance = await this.benchmarkDistance();
    results.quantize = await this.benchmarkQuantize();

    return results;
  }

  private async benchmarkEmbedSingle(): Promise<BenchmarkResult> {
    const embedder = new WasmEmbedder({ modelPath: 'minilm-l6-v2' });
    await embedder.initialize();

    const iterations = 100;
    const texts = Array(iterations).fill('This is a test sentence for embedding.');

    const start = performance.now();
    for (const text of texts) {
      await embedder.embed(text);
    }
    const elapsed = performance.now() - start;

    return {
      operation: 'embed_single',
      iterations,
      totalMs: elapsed,
      avgMs: elapsed / iterations,
      opsPerSecond: (iterations / elapsed) * 1000,
    };
  }

  private async benchmarkHnswSearch(): Promise<BenchmarkResult> {
    const index = new WasmHnswIndex({
      dimensions: 384,
      m: 16,
      efConstruction: 100,
      efSearch: 50,
      distanceMetric: 'cosine',
    });
    await index.initialize();

    // Insert 10k vectors
    for (let i = 0; i < 10000; i++) {
      await index.insert(`vec_${i}`, this.randomVector(384));
    }

    const iterations = 1000;
    const query = this.randomVector(384);

    const start = performance.now();
    for (let i = 0; i < iterations; i++) {
      await index.search(query, 10);
    }
    const elapsed = performance.now() - start;

    return {
      operation: 'hnsw_search_10k',
      iterations,
      totalMs: elapsed,
      avgMs: elapsed / iterations,
      opsPerSecond: (iterations / elapsed) * 1000,
    };
  }

  private randomVector(dim: number): Float32Array {
    const vec = new Float32Array(dim);
    for (let i = 0; i < dim; i++) {
      vec[i] = Math.random() * 2 - 1;
    }
    return vec;
  }
}
```

---

## Consequences

### Benefits

1. **Universal Deployment**: Same code runs everywhere (Node, Edge, Browser)
2. **Performance**: Near-native performance for vector operations
3. **Fallback Safety**: Always works even without WASM support
4. **Memory Efficiency**: Pooling and proper cleanup prevent leaks
5. **Model Portability**: ONNX models run in any environment

### Trade-offs

| Benefit | Trade-off |
|---------|-----------|
| Portability | Slight overhead vs pure native |
| WASM safety | No direct memory access (by design) |
| Model caching | Disk/Cache API storage needed |
| Lazy loading | First-use latency for initialization |

---

## Related Decisions

- **ADR-001**: Architecture Overview
- **ADR-003**: Persistence Layer (vector storage)
- **ADR-007**: Learning System (pattern WASM modules)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-27 | RuVector Architecture Team | Initial version |
