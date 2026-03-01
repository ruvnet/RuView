/**
 * Performance benchmarks for ruvector packages
 * Measures throughput, latency, and resource usage
 */

const test = require('node:test');
const assert = require('node:assert');

// Helper to format numbers
function formatNumber(num) {
  if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(2)}M`;
  if (num >= 1_000) return `${(num / 1_000).toFixed(2)}K`;
  return num.toFixed(0);
}

// Helper to format duration
function formatDuration(ms) {
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)}s`;
  return `${ms.toFixed(2)}ms`;
}

// Test insert performance
test('Performance - Insert Operations', async (t) => {
  const ruvector = require('ruvector');
  const dimension = 384;

  await t.test('single insert throughput', async () => {
    const index = new ruvector.VectorIndex({ dimension });
    const numVectors = 1000;

    const start = Date.now();

    for (let i = 0; i < numVectors; i++) {
      await index.insert({
        id: `single-${i}`,
        values: Array.from({ length: dimension }, () => Math.random())
      });
    }

    const duration = Date.now() - start;
    const throughput = numVectors / (duration / 1000);

    console.log(`    Single insert: ${formatNumber(throughput)} vectors/sec (${formatDuration(duration)})`);

    assert.ok(throughput > 0, 'Should complete inserts');
  });

  await t.test('batch insert throughput', async () => {
    const index = new ruvector.VectorIndex({ dimension });
    const numVectors = 10000;
    const batchSize = 1000;

    const vectors = Array.from({ length: numVectors }, (_, i) => ({
      id: `batch-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    const start = Date.now();

    await index.insertBatch(vectors, { batchSize });

    const duration = Date.now() - start;
    const throughput = numVectors / (duration / 1000);

    console.log(`    Batch insert: ${formatNumber(throughput)} vectors/sec (${formatDuration(duration)})`);

    const stats = await index.stats();
    assert.strictEqual(stats.vectorCount, numVectors, 'All vectors should be inserted');
  });

  await t.test('large batch insert', async () => {
    const index = new ruvector.VectorIndex({ dimension });
    const numVectors = 50000;

    const vectors = Array.from({ length: numVectors }, (_, i) => ({
      id: `large-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    const start = Date.now();

    await index.insertBatch(vectors, { batchSize: 5000 });

    const duration = Date.now() - start;
    const throughput = numVectors / (duration / 1000);

    console.log(`    Large batch (50K): ${formatNumber(throughput)} vectors/sec (${formatDuration(duration)})`);

    assert.ok(duration < 120000, 'Should complete within 2 minutes');
  });
});

// Test search performance
test('Performance - Search Operations', async (t) => {
  const ruvector = require('ruvector');
  const dimension = 384;
  const numVectors = 10000;

  // Setup: create index with data
  const index = new ruvector.VectorIndex({ dimension, metric: 'cosine', indexType: 'hnsw' });
  const vectors = Array.from({ length: numVectors }, (_, i) => ({
    id: `search-perf-${i}`,
    values: Array.from({ length: dimension }, () => Math.random())
  }));

  console.log('    Setting up test data...');
  await index.insertBatch(vectors, { batchSize: 5000 });

  await t.test('search latency (k=10)', async () => {
    const numQueries = 100;
    const queries = Array.from(
      { length: numQueries },
      () => Array.from({ length: dimension }, () => Math.random())
    );

    const latencies = [];

    for (const query of queries) {
      const start = Date.now();
      await index.search(query, { k: 10 });
      latencies.push(Date.now() - start);
    }

    const avgLatency = latencies.reduce((a, b) => a + b) / latencies.length;
    const p95Latency = latencies.sort((a, b) => a - b)[Math.floor(latencies.length * 0.95)];
    const throughput = numQueries / (latencies.reduce((a, b) => a + b) / 1000);

    console.log(`    Search (k=10): ${formatNumber(throughput)} qps`);
    console.log(`    Avg latency: ${formatDuration(avgLatency)}`);
    console.log(`    P95 latency: ${formatDuration(p95Latency)}`);

    assert.ok(avgLatency < 1000, 'Average latency should be under 1 second');
  });

  await t.test('search latency (k=100)', async () => {
    const numQueries = 100;
    const queries = Array.from(
      { length: numQueries },
      () => Array.from({ length: dimension }, () => Math.random())
    );

    const latencies = [];

    for (const query of queries) {
      const start = Date.now();
      await index.search(query, { k: 100 });
      latencies.push(Date.now() - start);
    }

    const avgLatency = latencies.reduce((a, b) => a + b) / latencies.length;
    const throughput = numQueries / (latencies.reduce((a, b) => a + b) / 1000);

    console.log(`    Search (k=100): ${formatNumber(throughput)} qps (avg: ${formatDuration(avgLatency)})`);

    assert.ok(throughput > 0, 'Should complete searches');
  });

  await t.test('concurrent search throughput', async () => {
    const numQueries = 50;
    const queries = Array.from(
      { length: numQueries },
      () => Array.from({ length: dimension }, () => Math.random())
    );

    const start = Date.now();

    // Execute searches in parallel
    await Promise.all(queries.map(query => index.search(query, { k: 10 })));

    const duration = Date.now() - start;
    const throughput = numQueries / (duration / 1000);

    console.log(`    Concurrent search: ${formatNumber(throughput)} qps (${formatDuration(duration)})`);

    assert.ok(throughput > 0, 'Should handle concurrent searches');
  });
});

// Test different dimensions
test('Performance - Dimension Scaling', async (t) => {
  const ruvector = require('ruvector');
  const numVectors = 1000;
  const numQueries = 50;

  for (const dimension of [128, 384, 768, 1536]) {
    await t.test(`dimension ${dimension}`, async () => {
      const index = new ruvector.VectorIndex({ dimension, metric: 'cosine' });

      // Insert
      const vectors = Array.from({ length: numVectors }, (_, i) => ({
        id: `dim-${dimension}-${i}`,
        values: Array.from({ length: dimension }, () => Math.random())
      }));

      const insertStart = Date.now();
      await index.insertBatch(vectors, { batchSize: 500 });
      const insertDuration = Date.now() - insertStart;
      const insertThroughput = numVectors / (insertDuration / 1000);

      // Search
      const queries = Array.from(
        { length: numQueries },
        () => Array.from({ length: dimension }, () => Math.random())
      );

      const searchStart = Date.now();
      for (const query of queries) {
        await index.search(query, { k: 10 });
      }
      const searchDuration = Date.now() - searchStart;
      const searchThroughput = numQueries / (searchDuration / 1000);

      console.log(`    Dim ${dimension}: Insert ${formatNumber(insertThroughput)} v/s, Search ${formatNumber(searchThroughput)} q/s`);

      assert.ok(insertThroughput > 0, 'Insert should complete');
      assert.ok(searchThroughput > 0, 'Search should complete');
    });
  }
});

// Test memory usage
test('Performance - Memory Usage', async (t) => {
  const ruvector = require('ruvector');

  await t.test('memory usage for large index', async () => {
    const dimension = 384;
    const numVectors = 10000;

    const initialMemory = process.memoryUsage().heapUsed;

    const index = new ruvector.VectorIndex({ dimension });

    const vectors = Array.from({ length: numVectors }, (_, i) => ({
      id: `mem-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    await index.insertBatch(vectors, { batchSize: 5000 });

    // Force garbage collection if available
    if (global.gc) {
      global.gc();
    }

    const finalMemory = process.memoryUsage().heapUsed;
    const memoryIncrease = finalMemory - initialMemory;
    const bytesPerVector = memoryIncrease / numVectors;

    console.log(`    Memory increase: ${(memoryIncrease / 1024 / 1024).toFixed(2)} MB`);
    console.log(`    Per vector: ${bytesPerVector.toFixed(0)} bytes`);

    // Rough estimate: each vector should be ~1.5-3KB (dimension * 4 bytes + overhead)
    const expectedBytes = dimension * 4 * 2; // 2x for overhead
    assert.ok(
      bytesPerVector < expectedBytes * 5,
      `Memory per vector (${bytesPerVector}) should be reasonable`
    );
  });
});

// Test backend comparison
test('Performance - Backend Comparison', async (t) => {
  const ruvector = require('ruvector');
  const info = ruvector.getBackendInfo();

  console.log(`\n  Backend: ${info.type}`);
  console.log(`  Features: ${info.features.join(', ')}`);

  await t.test('backend performance characteristics', async () => {
    const dimension = 384;
    const numVectors = 5000;
    const numQueries = 100;

    const index = new ruvector.VectorIndex({ dimension, metric: 'cosine' });

    // Benchmark insert
    const vectors = Array.from({ length: numVectors }, (_, i) => ({
      id: `backend-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    const insertStart = Date.now();
    await index.insertBatch(vectors);
    const insertDuration = Date.now() - insertStart;

    // Benchmark search
    const queries = Array.from(
      { length: numQueries },
      () => Array.from({ length: dimension }, () => Math.random())
    );

    const searchStart = Date.now();
    for (const query of queries) {
      await index.search(query, { k: 10 });
    }
    const searchDuration = Date.now() - searchStart;

    console.log(`\n  ${info.type} Backend Performance:`);
    console.log(`    Insert: ${formatNumber(numVectors / (insertDuration / 1000))} vectors/sec`);
    console.log(`    Search: ${formatNumber(numQueries / (searchDuration / 1000))} queries/sec`);

    assert.ok(true, 'Performance benchmark completed');
  });
});

// Test Utils performance
test('Performance - Utils Functions', async (t) => {
  const { Utils } = require('ruvector');
  const dimension = 1536;
  const iterations = 10000;

  await t.test('cosine similarity performance', () => {
    const a = Array.from({ length: dimension }, () => Math.random());
    const b = Array.from({ length: dimension }, () => Math.random());

    const start = Date.now();

    for (let i = 0; i < iterations; i++) {
      Utils.cosineSimilarity(a, b);
    }

    const duration = Date.now() - start;
    const throughput = iterations / (duration / 1000);

    console.log(`    Cosine similarity: ${formatNumber(throughput)} ops/sec`);

    assert.ok(throughput > 100, 'Should compute at least 100 ops/sec');
  });

  await t.test('euclidean distance performance', () => {
    const a = Array.from({ length: dimension }, () => Math.random());
    const b = Array.from({ length: dimension }, () => Math.random());

    const start = Date.now();

    for (let i = 0; i < iterations; i++) {
      Utils.euclideanDistance(a, b);
    }

    const duration = Date.now() - start;
    const throughput = iterations / (duration / 1000);

    console.log(`    Euclidean distance: ${formatNumber(throughput)} ops/sec`);

    assert.ok(throughput > 100, 'Should compute at least 100 ops/sec');
  });

  await t.test('normalization performance', () => {
    const vectors = Array.from(
      { length: iterations },
      () => Array.from({ length: dimension }, () => Math.random())
    );

    const start = Date.now();

    for (const vector of vectors) {
      Utils.normalize(vector);
    }

    const duration = Date.now() - start;
    const throughput = iterations / (duration / 1000);

    console.log(`    Normalization: ${formatNumber(throughput)} ops/sec`);

    assert.ok(throughput > 100, 'Should normalize at least 100 vectors/sec');
  });
});
