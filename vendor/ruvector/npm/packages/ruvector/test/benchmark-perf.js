const { LRUCache, Float32BufferPool, VectorOps, ParallelBatchProcessor, OptimizedMemoryStore } = require('../dist/core/neural-perf.js');

// Benchmark utilities
function benchmark(name, fn, iterations = 10000) {
  // Warmup
  for (let i = 0; i < 100; i++) fn();

  const start = performance.now();
  for (let i = 0; i < iterations; i++) fn();
  const elapsed = performance.now() - start;

  return { name, iterations, totalMs: elapsed, perOpUs: (elapsed / iterations) * 1000 };
}

function formatResult(result) {
  const name = result.name.padEnd(40);
  const us = result.perOpUs.toFixed(3).padStart(10);
  return name + ' ' + us + ' us/op  (' + result.iterations + ' ops in ' + result.totalMs.toFixed(1) + 'ms)';
}

console.log('\n═══════════════════════════════════════════════════════════════════');
console.log('  RUVECTOR PERFORMANCE BENCHMARKS');
console.log('═══════════════════════════════════════════════════════════════════\n');

// ============================================================================
// 1. LRU Cache: O(1) vs Map (simulated O(n) eviction)
// ============================================================================
console.log('┌─────────────────────────────────────────────────────────────────┐');
console.log('│  LRU CACHE BENCHMARK                                            │');
console.log('└─────────────────────────────────────────────────────────────────┘');

const lruCache = new LRUCache(1000);
const naiveCache = new Map();
const CACHE_SIZE = 1000;

// Pre-fill caches
for (let i = 0; i < CACHE_SIZE; i++) {
  lruCache.set('key' + i, { data: i });
  naiveCache.set('key' + i, { data: i });
}

// Benchmark LRU get (O(1))
const lruGet = benchmark('LRU Cache get (O(1))', () => {
  lruCache.get('key' + Math.floor(Math.random() * CACHE_SIZE));
}, 100000);

// Benchmark LRU set with eviction (O(1))
let lruSetCounter = CACHE_SIZE;
const lruSet = benchmark('LRU Cache set+evict (O(1))', () => {
  lruCache.set('newkey' + lruSetCounter++, { data: lruSetCounter });
}, 50000);

// Simulate naive O(n) eviction
const naiveEvict = benchmark('Naive Map eviction (O(n))', () => {
  // Simulate finding oldest entry (O(n) scan)
  let oldest = null;
  for (const [k, v] of naiveCache.entries()) {
    oldest = k;
    break; // Just get first (simulating finding oldest)
  }
  naiveCache.delete(oldest);
  naiveCache.set('key' + Math.random(), { data: 1 });
}, 50000);

console.log(formatResult(lruGet));
console.log(formatResult(lruSet));
console.log(formatResult(naiveEvict));
console.log('  → LRU Speedup: ' + (naiveEvict.perOpUs / lruSet.perOpUs).toFixed(1) + 'x faster\n');

// ============================================================================
// 2. Buffer Pool vs Fresh Allocation
// ============================================================================
console.log('┌─────────────────────────────────────────────────────────────────┐');
console.log('│  BUFFER POOL BENCHMARK                                          │');
console.log('└─────────────────────────────────────────────────────────────────┘');

const bufferPool = new Float32BufferPool(64);
bufferPool.prewarm([384], 32);

// Pooled allocation
const pooledAlloc = benchmark('Buffer Pool acquire+release', () => {
  const buf = bufferPool.acquire(384);
  buf[0] = 1.0; // Use it
  bufferPool.release(buf);
}, 100000);

// Fresh allocation
const freshAlloc = benchmark('Fresh Float32Array allocation', () => {
  const buf = new Float32Array(384);
  buf[0] = 1.0; // Use it
  // Let GC handle it
}, 100000);

console.log(formatResult(pooledAlloc));
console.log(formatResult(freshAlloc));
console.log('  → Pool Speedup: ' + (freshAlloc.perOpUs / pooledAlloc.perOpUs).toFixed(1) + 'x faster');
const poolStats = bufferPool.getStats();
console.log('  → Pool Stats: reuse=' + (poolStats.reuseRate * 100).toFixed(1) + '%, pooled=' + poolStats.pooledBuffers + '\n');

// ============================================================================
// 3. Vector Ops: Unrolled vs Standard
// ============================================================================
console.log('┌─────────────────────────────────────────────────────────────────┐');
console.log('│  VECTOR OPERATIONS BENCHMARK (384-dim)                          │');
console.log('└─────────────────────────────────────────────────────────────────┘');

const vecA = new Float32Array(384);
const vecB = new Float32Array(384);
for (let i = 0; i < 384; i++) {
  vecA[i] = Math.random();
  vecB[i] = Math.random();
}

// Unrolled cosine
const unrolledCosine = benchmark('VectorOps.cosine (8x unrolled)', () => {
  VectorOps.cosine(vecA, vecB);
}, 100000);

// Standard cosine
function standardCosine(a, b) {
  let dot = 0, normA = 0, normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  return dot / Math.sqrt(normA * normB);
}

const stdCosine = benchmark('Standard cosine (no unroll)', () => {
  standardCosine(vecA, vecB);
}, 100000);

console.log(formatResult(unrolledCosine));
console.log(formatResult(stdCosine));
console.log('  → Unroll Speedup: ' + (stdCosine.perOpUs / unrolledCosine.perOpUs).toFixed(2) + 'x faster\n');

// Unrolled dot product
const unrolledDot = benchmark('VectorOps.dot (8x unrolled)', () => {
  VectorOps.dot(vecA, vecB);
}, 100000);

function standardDot(a, b) {
  let sum = 0;
  for (let i = 0; i < a.length; i++) sum += a[i] * b[i];
  return sum;
}

const stdDot = benchmark('Standard dot product', () => {
  standardDot(vecA, vecB);
}, 100000);

console.log(formatResult(unrolledDot));
console.log(formatResult(stdDot));
console.log('  → Unroll Speedup: ' + (stdDot.perOpUs / unrolledDot.perOpUs).toFixed(2) + 'x faster\n');

// Unrolled distance
const unrolledDist = benchmark('VectorOps.distance (8x unrolled)', () => {
  VectorOps.distance(vecA, vecB);
}, 100000);

function standardDistance(a, b) {
  let sum = 0;
  for (let i = 0; i < a.length; i++) {
    const d = a[i] - b[i];
    sum += d * d;
  }
  return Math.sqrt(sum);
}

const stdDist = benchmark('Standard distance', () => {
  standardDistance(vecA, vecB);
}, 100000);

console.log(formatResult(unrolledDist));
console.log(formatResult(stdDist));
console.log('  → Unroll Speedup: ' + (stdDist.perOpUs / unrolledDist.perOpUs).toFixed(2) + 'x faster\n');

// ============================================================================
// 4. Batch Processing
// ============================================================================
console.log('┌─────────────────────────────────────────────────────────────────┐');
console.log('│  BATCH SIMILARITY SEARCH                                        │');
console.log('└─────────────────────────────────────────────────────────────────┘');

const corpus = [];
for (let i = 0; i < 1000; i++) {
  const v = new Float32Array(384);
  for (let j = 0; j < 384; j++) v[j] = Math.random();
  corpus.push(v);
}

const queries = [];
for (let i = 0; i < 100; i++) {
  const v = new Float32Array(384);
  for (let j = 0; j < 384; j++) v[j] = Math.random();
  queries.push(v);
}

const batchProcessor = new ParallelBatchProcessor({ batchSize: 32 });

const batchSearch = benchmark('Batch similarity (10 queries x 100 corpus)', () => {
  batchProcessor.batchSimilarity(queries.slice(0, 10), corpus.slice(0, 100), 5);
}, 100);

console.log(formatResult(batchSearch));

// ============================================================================
// 5. OptimizedMemoryStore
// ============================================================================
console.log('\n┌─────────────────────────────────────────────────────────────────┐');
console.log('│  OPTIMIZED MEMORY STORE                                         │');
console.log('└─────────────────────────────────────────────────────────────────┘');

const store = new OptimizedMemoryStore({ cacheSize: 1000, dimension: 384 });

// Pre-fill
for (let i = 0; i < 1000; i++) {
  const emb = new Float32Array(384);
  for (let j = 0; j < 384; j++) emb[j] = Math.random();
  store.store('mem' + i, emb, 'content ' + i);
}

const storeGet = benchmark('OptimizedMemoryStore.get (O(1))', () => {
  store.get('mem' + Math.floor(Math.random() * 1000));
}, 100000);

const queryEmb = new Float32Array(384);
for (let j = 0; j < 384; j++) queryEmb[j] = Math.random();

const storeSearch = benchmark('OptimizedMemoryStore.search (k=5)', () => {
  store.search(queryEmb, 5);
}, 1000);

console.log(formatResult(storeGet));
console.log(formatResult(storeSearch));
const storeStats = store.getStats();
console.log('  → Cache: hits=' + storeStats.cache.hits + ', hitRate=' + (storeStats.cache.hitRate * 100).toFixed(1) + '%\n');

// ============================================================================
// Summary
// ============================================================================
console.log('═══════════════════════════════════════════════════════════════════');
console.log('  SUMMARY');
console.log('═══════════════════════════════════════════════════════════════════');
console.log('  ✓ LRU Cache:     O(1) operations, ' + (naiveEvict.perOpUs / lruSet.perOpUs).toFixed(0) + 'x faster than naive eviction');
console.log('  ✓ Buffer Pool:   ' + (freshAlloc.perOpUs / pooledAlloc.perOpUs).toFixed(1) + 'x faster, ' + (poolStats.reuseRate * 100).toFixed(0) + '% reuse rate');
console.log('  ✓ Vector Ops:    ' + (stdCosine.perOpUs / unrolledCosine.perOpUs).toFixed(1) + 'x faster with 8x unrolling');
console.log('  ✓ Memory Store:  O(1) lookup at ' + storeGet.perOpUs.toFixed(3) + ' µs/op');
console.log('═══════════════════════════════════════════════════════════════════\n');
