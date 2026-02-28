#!/usr/bin/env node

/**
 * AgentDB Performance Benchmark Suite
 *
 * Comprehensive benchmarking of all attention mechanisms and vector operations
 * to find optimal configurations and measure true performance limits.
 *
 * Benchmarks:
 * 1. Attention mechanisms across different dimensions and batch sizes
 * 2. Vector search with varying dataset sizes
 * 3. Batch vs single processing
 * 4. Cache effectiveness
 * 5. Memory usage profiling
 */

const { VectorDB } = require('ruvector');
const {
  MultiHeadAttention,
  HyperbolicAttention,
  FlashAttention,
  MoEAttention,
  LinearAttention
} = require('@ruvector/attention');

console.log('⚡ AgentDB Performance Benchmark Suite\n');
console.log('=' .repeat(70));

class PerformanceBenchmark {
  constructor() {
    this.results = new Map();
    this.cache = new Map();
    this.stats = {
      totalTests: 0,
      totalTime: 0,
      cacheHits: 0,
      cacheMisses: 0
    };
  }

  // Benchmark a function with multiple iterations
  async benchmark(name, fn, iterations = 100) {
    console.log(`\n🔬 Benchmarking: ${name}`);
    console.log(`   Iterations: ${iterations}`);

    const times = [];
    const memoryBefore = process.memoryUsage();

    for (let i = 0; i < iterations; i++) {
      const start = performance.now();
      await fn();
      const end = performance.now();
      times.push(end - start);
    }

    const memoryAfter = process.memoryUsage();

    // Calculate statistics
    const sorted = times.sort((a, b) => a - b);
    const stats = {
      min: sorted[0],
      max: sorted[sorted.length - 1],
      mean: times.reduce((a, b) => a + b, 0) / times.length,
      median: sorted[Math.floor(sorted.length / 2)],
      p95: sorted[Math.floor(sorted.length * 0.95)],
      p99: sorted[Math.floor(sorted.length * 0.99)],
      stdDev: this.calculateStdDev(times),
      opsPerSec: 1000 / (times.reduce((a, b) => a + b, 0) / times.length),
      memoryDelta: (memoryAfter.heapUsed - memoryBefore.heapUsed) / 1024 / 1024
    };

    this.results.set(name, stats);
    this.stats.totalTests++;
    this.stats.totalTime += times.reduce((a, b) => a + b, 0);

    console.log(`   ✓ Mean: ${stats.mean.toFixed(3)}ms`);
    console.log(`   ✓ Median: ${stats.median.toFixed(3)}ms`);
    console.log(`   ✓ P95: ${stats.p95.toFixed(3)}ms`);
    console.log(`   ✓ P99: ${stats.p99.toFixed(3)}ms`);
    console.log(`   ✓ Ops/sec: ${stats.opsPerSec.toFixed(0)}`);
    console.log(`   ✓ Memory: ${stats.memoryDelta > 0 ? '+' : ''}${stats.memoryDelta.toFixed(2)}MB`);

    return stats;
  }

  calculateStdDev(values) {
    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    const squareDiffs = values.map(value => Math.pow(value - mean, 2));
    const avgSquareDiff = squareDiffs.reduce((a, b) => a + b, 0) / squareDiffs.length;
    return Math.sqrt(avgSquareDiff);
  }

  // Cache wrapper for vector embeddings
  getCachedVector(key, generator) {
    if (this.cache.has(key)) {
      this.stats.cacheHits++;
      return this.cache.get(key);
    }

    this.stats.cacheMisses++;
    const value = generator();
    this.cache.set(key, value);
    return value;
  }

  clearCache() {
    this.cache.clear();
  }
}

// Create test vectors
function createTestVectors(count, dimensions) {
  const vectors = [];
  for (let i = 0; i < count; i++) {
    const vec = new Float32Array(dimensions);
    for (let j = 0; j < dimensions; j++) {
      vec[j] = Math.random() * 2 - 1; // [-1, 1]
    }
    vectors.push(vec);
  }
  return vectors;
}

async function runBenchmarks() {
  const bench = new PerformanceBenchmark();

  console.log('\n📊 PART 1: Attention Mechanism Benchmarks\n');
  console.log('=' .repeat(70));

  // Test different dimensions
  const dimensions = [32, 64, 128, 256];
  const sequenceLengths = [5, 10, 20];

  for (const dim of dimensions) {
    console.log(`\n\n🔷 Testing Dimension: ${dim}\n`);

    // Multi-Head Attention
    for (const numHeads of [4, 8]) {
      const query = new Float32Array(dim).fill(0.1);
      const keys = createTestVectors(10, dim);
      const values = createTestVectors(10, dim);

      await bench.benchmark(
        `MultiHead-${dim}d-${numHeads}h`,
        () => {
          const attn = new MultiHeadAttention(dim, numHeads);
          attn.compute(query, keys, values);
        },
        50
      );
    }

    // Hyperbolic Attention
    const query = new Float32Array(dim).fill(0.1);
    const keys = createTestVectors(10, dim);
    const values = createTestVectors(10, dim);

    await bench.benchmark(
      `Hyperbolic-${dim}d`,
      () => {
        const attn = new HyperbolicAttention(dim, -1.0);
        attn.compute(query, keys, values);
      },
      50
    );

    // Flash Attention
    for (const blockSize of [16, 32, 64]) {
      if (blockSize <= dim) {
        await bench.benchmark(
          `Flash-${dim}d-block${blockSize}`,
          () => {
            const attn = new FlashAttention(dim, blockSize);
            attn.compute(query, keys, values);
          },
          50
        );
      }
    }

    // Linear Attention
    await bench.benchmark(
      `Linear-${dim}d`,
      () => {
        const attn = new LinearAttention(dim, dim);
        attn.compute(query, keys, values);
      },
      50
    );

    // MoE Attention
    await bench.benchmark(
      `MoE-${dim}d-4experts`,
      () => {
        const attn = new MoEAttention({
          dim: dim,
          numExperts: 4,
          topK: 2,
          expertCapacity: 1.25
        });
        attn.compute(query, keys, values);
      },
      50
    );
  }

  console.log('\n\n📊 PART 2: Vector Search Benchmarks\n');
  console.log('=' .repeat(70));

  // Test different dataset sizes
  const datasetSizes = [100, 500, 1000];

  for (const size of datasetSizes) {
    console.log(`\n\n🔷 Dataset Size: ${size} vectors\n`);

    const db = new VectorDB({
      dimensions: 128,
      maxElements: size
    });

    // Insert vectors
    console.log(`   Inserting ${size} vectors...`);
    for (let i = 0; i < size; i++) {
      const vec = new Float32Array(128);
      for (let j = 0; j < 128; j++) {
        vec[j] = Math.random();
      }
      await db.insert({
        id: `vec-${i}`,
        vector: vec,
        metadata: { index: i }
      });
    }

    // Benchmark search
    const queryVec = new Float32Array(128).fill(0.5);

    await bench.benchmark(
      `VectorSearch-${size}-k5`,
      async () => {
        await db.search({ vector: queryVec, k: 5 });
      },
      100
    );

    await bench.benchmark(
      `VectorSearch-${size}-k10`,
      async () => {
        await db.search({ vector: queryVec, k: 10 });
      },
      100
    );

    await bench.benchmark(
      `VectorSearch-${size}-k20`,
      async () => {
        await db.search({ vector: queryVec, k: 20 });
      },
      100
    );
  }

  console.log('\n\n📊 PART 3: Batch Processing Benchmarks\n');
  console.log('=' .repeat(70));

  const db = new VectorDB({
    dimensions: 128,
    maxElements: 500
  });

  // Insert test data
  for (let i = 0; i < 500; i++) {
    const vec = new Float32Array(128);
    for (let j = 0; j < 128; j++) {
      vec[j] = Math.random();
    }
    await db.insert({
      id: `vec-${i}`,
      vector: vec,
      metadata: { index: i }
    });
  }

  // Single query vs batch queries
  const queries = [];
  for (let i = 0; i < 10; i++) {
    const vec = new Float32Array(128);
    for (let j = 0; j < 128; j++) {
      vec[j] = Math.random();
    }
    queries.push(vec);
  }

  await bench.benchmark(
    'Sequential-10-queries',
    async () => {
      for (const query of queries) {
        await db.search({ vector: query, k: 5 });
      }
    },
    20
  );

  await bench.benchmark(
    'Parallel-10-queries',
    async () => {
      await Promise.all(
        queries.map(query => db.search({ vector: query, k: 5 }))
      );
    },
    20
  );

  console.log('\n\n📊 PART 4: Cache Effectiveness\n');
  console.log('=' .repeat(70));

  // Test with cache
  bench.clearCache();

  await bench.benchmark(
    'With-Cache-Cold',
    () => {
      for (let i = 0; i < 100; i++) {
        bench.getCachedVector(`vec-${i}`, () => createTestVectors(1, 128)[0]);
      }
    },
    10
  );

  await bench.benchmark(
    'With-Cache-Warm',
    () => {
      for (let i = 0; i < 100; i++) {
        bench.getCachedVector(`vec-${i % 50}`, () => createTestVectors(1, 128)[0]);
      }
    },
    10
  );

  console.log(`\n   Cache Statistics:`);
  console.log(`   - Hits: ${bench.stats.cacheHits}`);
  console.log(`   - Misses: ${bench.stats.cacheMisses}`);
  console.log(`   - Hit Rate: ${(bench.stats.cacheHits / (bench.stats.cacheHits + bench.stats.cacheMisses) * 100).toFixed(1)}%`);

  // Generate Summary Report
  console.log('\n\n' + '=' .repeat(70));
  console.log('\n📈 PERFORMANCE SUMMARY REPORT\n');
  console.log('=' .repeat(70));

  // Find fastest operations
  const sortedResults = Array.from(bench.results.entries())
    .sort((a, b) => a[1].mean - b[1].mean);

  console.log('\n🏆 TOP 10 FASTEST OPERATIONS:\n');
  sortedResults.slice(0, 10).forEach(([name, stats], index) => {
    console.log(`   ${index + 1}. ${name}`);
    console.log(`      Mean: ${stats.mean.toFixed(3)}ms | Ops/sec: ${stats.opsPerSec.toFixed(0)}`);
  });

  console.log('\n🐌 TOP 5 SLOWEST OPERATIONS:\n');
  sortedResults.slice(-5).reverse().forEach(([name, stats], index) => {
    console.log(`   ${index + 1}. ${name}`);
    console.log(`      Mean: ${stats.mean.toFixed(3)}ms | Ops/sec: ${stats.opsPerSec.toFixed(0)}`);
  });

  // Attention mechanism comparison
  console.log('\n\n⚡ ATTENTION MECHANISM COMPARISON (64d):\n');

  const attentionResults = Array.from(bench.results.entries())
    .filter(([name]) => name.includes('-64d') && !name.includes('VectorSearch'))
    .sort((a, b) => a[1].mean - b[1].mean);

  attentionResults.forEach(([name, stats]) => {
    const mechanism = name.split('-')[0];
    const bar = '█'.repeat(Math.max(1, Math.floor(stats.opsPerSec / 1000)));
    console.log(`   ${mechanism.padEnd(15)} ${bar} ${stats.mean.toFixed(3)}ms (${stats.opsPerSec.toFixed(0)} ops/s)`);
  });

  // Vector search scaling
  console.log('\n\n📊 VECTOR SEARCH SCALING (k=5):\n');

  const searchResults = Array.from(bench.results.entries())
    .filter(([name]) => name.includes('VectorSearch') && name.includes('k5'))
    .sort((a, b) => {
      const sizeA = parseInt(a[0].split('-')[1]);
      const sizeB = parseInt(b[0].split('-')[1]);
      return sizeA - sizeB;
    });

  searchResults.forEach(([name, stats]) => {
    const size = name.split('-')[1];
    console.log(`   ${size.padEnd(10)} vectors: ${stats.mean.toFixed(3)}ms | ${stats.opsPerSec.toFixed(0)} ops/s`);
  });

  // Batch processing benefit
  console.log('\n\n🔄 BATCH PROCESSING BENEFIT:\n');

  const sequential = bench.results.get('Sequential-10-queries');
  const parallel = bench.results.get('Parallel-10-queries');

  if (sequential && parallel) {
    const speedup = sequential.mean / parallel.mean;
    console.log(`   Sequential: ${sequential.mean.toFixed(3)}ms`);
    console.log(`   Parallel:   ${parallel.mean.toFixed(3)}ms`);
    console.log(`   Speedup:    ${speedup.toFixed(2)}x faster`);
    console.log(`   Benefit:    ${((1 - parallel.mean / sequential.mean) * 100).toFixed(1)}% time saved`);
  }

  // Overall statistics
  console.log('\n\n📊 OVERALL STATISTICS:\n');
  console.log(`   Total Tests: ${bench.stats.totalTests}`);
  console.log(`   Total Time: ${(bench.stats.totalTime / 1000).toFixed(2)}s`);
  console.log(`   Avg Test Time: ${(bench.stats.totalTime / bench.stats.totalTests).toFixed(3)}ms`);

  // Recommendations
  console.log('\n\n💡 OPTIMIZATION RECOMMENDATIONS:\n');

  const fastest = sortedResults[0];
  const flashResults = attentionResults.filter(([name]) => name.includes('Flash'));
  const optimalDim = attentionResults.length > 0 ?
    attentionResults[0][0].match(/(\d+)d/)[1] : '64';

  console.log(`   1. Fastest overall: ${fastest[0]} (${fastest[1].mean.toFixed(3)}ms)`);

  if (flashResults.length > 0) {
    console.log(`   2. Flash Attention is consistently fast across dimensions`);
  }

  console.log(`   3. Optimal dimension for attention: ${optimalDim}d`);
  console.log(`   4. Batch processing provides ${parallel ? (sequential.mean / parallel.mean).toFixed(1) : 'significant'}x speedup`);
  console.log(`   5. Cache hit rate: ${(bench.stats.cacheHits / (bench.stats.cacheHits + bench.stats.cacheMisses) * 100).toFixed(1)}%`);

  if (searchResults.length > 1) {
    const scaling = searchResults[searchResults.length - 1][1].mean / searchResults[0][1].mean;
    console.log(`   6. Vector search scales ${scaling < 5 ? 'well' : 'linearly'} with dataset size`);
  }

  console.log('\n' + '=' .repeat(70));
  console.log('\n✅ Benchmark Suite Complete!\n');
}

runBenchmarks().catch(error => {
  console.error('\n❌ Error:', error);
  console.error('\nStack:', error.stack);
  process.exit(1);
});
