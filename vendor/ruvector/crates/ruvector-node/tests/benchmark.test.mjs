import test from 'ava';
import { VectorDB } from '../index.js';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

// Helper to create temp directory
function createTempDir() {
  return mkdtempSync(join(tmpdir(), 'ruvector-bench-'));
}

// Helper to cleanup temp directory
function cleanupTempDir(dir) {
  try {
    rmSync(dir, { recursive: true, force: true });
  } catch (e) {
    console.warn('Failed to cleanup temp dir:', e.message);
  }
}

// Performance measurement helper
function measure(name, fn) {
  const start = process.hrtime.bigint();
  const result = fn();
  const end = process.hrtime.bigint();
  const durationMs = Number(end - start) / 1_000_000;
  console.log(`${name}: ${durationMs.toFixed(2)}ms`);
  return { result, durationMs };
}

async function measureAsync(name, fn) {
  const start = process.hrtime.bigint();
  const result = await fn();
  const end = process.hrtime.bigint();
  const durationMs = Number(end - start) / 1_000_000;
  console.log(`${name}: ${durationMs.toFixed(2)}ms`);
  return { result, durationMs };
}

test('Benchmark - batch insert performance', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 128,
    storagePath: join(tempDir, 'bench.db'),
  });

  const vectors = Array.from({ length: 1000 }, () => ({
    vector: new Float32Array(128).fill(0).map(() => Math.random()),
  }));

  const { durationMs } = await measureAsync(
    'Insert 1000 vectors (batch)',
    async () => {
      return await db.insertBatch(vectors);
    }
  );

  // Should complete in reasonable time (< 1 second for 1000 vectors)
  t.true(durationMs < 1000);
  t.is(await db.len(), 1000);

  const throughput = (1000 / durationMs) * 1000;
  console.log(`Throughput: ${throughput.toFixed(0)} vectors/sec`);
});

test('Benchmark - search performance', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 128,
    storagePath: join(tempDir, 'bench.db'),
    hnswConfig: {
      m: 32,
      efConstruction: 200,
      efSearch: 100,
    },
  });

  // Insert 10k vectors
  const batchSize = 1000;
  const totalVectors = 10000;

  console.log(`Inserting ${totalVectors} vectors...`);
  for (let i = 0; i < totalVectors / batchSize; i++) {
    const batch = Array.from({ length: batchSize }, () => ({
      vector: new Float32Array(128).fill(0).map(() => Math.random()),
    }));
    await db.insertBatch(batch);
  }

  t.is(await db.len(), totalVectors);

  // Benchmark search
  const queryVector = new Float32Array(128).fill(0).map(() => Math.random());

  const { durationMs } = await measureAsync('Search 10k vectors (k=10)', async () => {
    return await db.search({
      vector: queryVector,
      k: 10,
    });
  });

  // Should complete in < 10ms for 10k vectors
  t.true(durationMs < 100);
  console.log(`Search latency: ${durationMs.toFixed(2)}ms`);

  // Multiple searches
  const numQueries = 100;
  const { durationMs: totalDuration } = await measureAsync(
    `${numQueries} searches`,
    async () => {
      const promises = Array.from({ length: numQueries }, () =>
        db.search({
          vector: new Float32Array(128).fill(0).map(() => Math.random()),
          k: 10,
        })
      );
      return await Promise.all(promises);
    }
  );

  const avgLatency = totalDuration / numQueries;
  const qps = (numQueries / totalDuration) * 1000;
  console.log(`Average latency: ${avgLatency.toFixed(2)}ms`);
  console.log(`QPS: ${qps.toFixed(0)} queries/sec`);

  t.pass();
});

test('Benchmark - concurrent insert and search', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 64,
    storagePath: join(tempDir, 'bench.db'),
  });

  // Initial data
  await db.insertBatch(
    Array.from({ length: 1000 }, () => ({
      vector: new Float32Array(64).fill(0).map(() => Math.random()),
    }))
  );

  // Mix of operations
  const operations = [];

  // Add insert operations
  for (let i = 0; i < 50; i++) {
    operations.push(
      db.insert({
        vector: new Float32Array(64).fill(0).map(() => Math.random()),
      })
    );
  }

  // Add search operations
  for (let i = 0; i < 50; i++) {
    operations.push(
      db.search({
        vector: new Float32Array(64).fill(0).map(() => Math.random()),
        k: 10,
      })
    );
  }

  const { durationMs } = await measureAsync(
    '50 inserts + 50 searches (concurrent)',
    async () => {
      return await Promise.all(operations);
    }
  );

  t.true(durationMs < 2000);
  console.log(`Mixed workload: ${durationMs.toFixed(2)}ms`);
});

test('Benchmark - memory efficiency', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 384,
    storagePath: join(tempDir, 'bench.db'),
    quantization: {
      type: 'scalar',
    },
  });

  const memBefore = process.memoryUsage();

  // Insert 5k vectors
  const batchSize = 500;
  const totalVectors = 5000;

  for (let i = 0; i < totalVectors / batchSize; i++) {
    const batch = Array.from({ length: batchSize }, () => ({
      vector: new Float32Array(384).fill(0).map(() => Math.random()),
    }));
    await db.insertBatch(batch);
  }

  const memAfter = process.memoryUsage();
  const heapUsed = (memAfter.heapUsed - memBefore.heapUsed) / 1024 / 1024;

  console.log(`Heap used for ${totalVectors} 384D vectors: ${heapUsed.toFixed(2)}MB`);
  console.log(`Per-vector memory: ${((heapUsed / totalVectors) * 1024).toFixed(2)}KB`);

  t.is(await db.len(), totalVectors);
  t.pass();
});

test('Benchmark - different vector dimensions', async (t) => {
  const dimensions = [128, 384, 768, 1536];
  const numVectors = 1000;

  for (const dim of dimensions) {
    const tempDir = createTempDir();

    const db = new VectorDB({
      dimensions: dim,
      storagePath: join(tempDir, 'bench.db'),
    });

    const vectors = Array.from({ length: numVectors }, () => ({
      vector: new Float32Array(dim).fill(0).map(() => Math.random()),
    }));

    const { durationMs: insertTime } = await measureAsync(
      `Insert ${numVectors} ${dim}D vectors`,
      async () => {
        return await db.insertBatch(vectors);
      }
    );

    const { durationMs: searchTime } = await measureAsync(
      `Search ${dim}D vectors`,
      async () => {
        return await db.search({
          vector: new Float32Array(dim).fill(0).map(() => Math.random()),
          k: 10,
        });
      }
    );

    console.log(
      `${dim}D - Insert: ${insertTime.toFixed(2)}ms, Search: ${searchTime.toFixed(2)}ms`
    );

    cleanupTempDir(tempDir);
  }

  t.pass();
});
