import test from 'ava';
import { VectorDB } from '../index.js';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

// Helper to create temp directory
function createTempDir() {
  return mkdtempSync(join(tmpdir(), 'ruvector-test-'));
}

// Helper to cleanup temp directory
function cleanupTempDir(dir) {
  try {
    rmSync(dir, { recursive: true, force: true });
  } catch (e) {
    console.warn('Failed to cleanup temp dir:', e.message);
  }
}

test('VectorDB - version check', (t) => {
  const { version } = require('../index.js');
  t.is(typeof version, 'function');
  t.is(typeof version(), 'string');
  t.regex(version(), /^\d+\.\d+\.\d+/);
});

test('VectorDB - hello function', (t) => {
  const { hello } = require('../index.js');
  t.is(typeof hello, 'function');
  t.is(hello(), 'Hello from Ruvector Node.js bindings!');
});

test('VectorDB - constructor with options', (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    distanceMetric: 'Euclidean',
    storagePath: join(tempDir, 'test.db'),
  });

  t.truthy(db);
  t.is(typeof db.insert, 'function');
  t.is(typeof db.search, 'function');
});

test('VectorDB - withDimensions factory', (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = VectorDB.withDimensions(128);
  t.truthy(db);
});

test('VectorDB - insert single vector', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  const id = await db.insert({
    vector: new Float32Array([1.0, 2.0, 3.0]),
    metadata: { text: 'test vector' },
  });

  t.is(typeof id, 'string');
  t.truthy(id.length > 0);
});

test('VectorDB - insert with custom ID', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  const customId = 'custom-vector-123';
  const id = await db.insert({
    id: customId,
    vector: new Float32Array([1.0, 2.0, 3.0]),
  });

  t.is(id, customId);
});

test('VectorDB - insert batch', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  const ids = await db.insertBatch([
    { vector: new Float32Array([1.0, 0.0, 0.0]) },
    { vector: new Float32Array([0.0, 1.0, 0.0]) },
    { vector: new Float32Array([0.0, 0.0, 1.0]) },
  ]);

  t.is(ids.length, 3);
  t.truthy(ids.every((id) => typeof id === 'string' && id.length > 0));
});

test('VectorDB - search exact match', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    distanceMetric: 'Euclidean',
    storagePath: join(tempDir, 'test.db'),
    hnswConfig: null, // Use flat index for testing
  });

  await db.insert({
    id: 'v1',
    vector: new Float32Array([1.0, 0.0, 0.0]),
  });

  await db.insert({
    id: 'v2',
    vector: new Float32Array([0.0, 1.0, 0.0]),
  });

  const results = await db.search({
    vector: new Float32Array([1.0, 0.0, 0.0]),
    k: 2,
  });

  t.truthy(Array.isArray(results));
  t.truthy(results.length >= 1);
  t.is(results[0].id, 'v1');
  t.true(results[0].score < 0.01);
});

test('VectorDB - search with metadata filter', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  await db.insert({
    vector: new Float32Array([1.0, 0.0, 0.0]),
    metadata: { category: 'A' },
  });

  await db.insert({
    vector: new Float32Array([0.9, 0.1, 0.0]),
    metadata: { category: 'B' },
  });

  const results = await db.search({
    vector: new Float32Array([1.0, 0.0, 0.0]),
    k: 10,
    filter: { category: 'A' },
  });

  t.truthy(results.length >= 1);
  t.is(results[0].metadata?.category, 'A');
});

test('VectorDB - get by ID', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  const id = await db.insert({
    vector: new Float32Array([1.0, 2.0, 3.0]),
    metadata: { text: 'test' },
  });

  const entry = await db.get(id);
  t.truthy(entry);
  t.deepEqual(Array.from(entry.vector), [1.0, 2.0, 3.0]);
  t.is(entry.metadata?.text, 'test');
});

test('VectorDB - get non-existent ID', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  const entry = await db.get('non-existent-id');
  t.is(entry, null);
});

test('VectorDB - delete', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  const id = await db.insert({
    vector: new Float32Array([1.0, 2.0, 3.0]),
  });

  const deleted = await db.delete(id);
  t.true(deleted);

  const entry = await db.get(id);
  t.is(entry, null);
});

test('VectorDB - delete non-existent', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  const deleted = await db.delete('non-existent-id');
  t.false(deleted);
});

test('VectorDB - len and isEmpty', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  t.true(await db.isEmpty());
  t.is(await db.len(), 0);

  await db.insert({ vector: new Float32Array([1, 2, 3]) });
  t.false(await db.isEmpty());
  t.is(await db.len(), 1);

  await db.insert({ vector: new Float32Array([4, 5, 6]) });
  t.is(await db.len(), 2);
});

test('VectorDB - cosine similarity', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    distanceMetric: 'Cosine',
    storagePath: join(tempDir, 'test.db'),
  });

  await db.insert({
    id: 'v1',
    vector: new Float32Array([1.0, 0.0, 0.0]),
  });

  await db.insert({
    id: 'v2',
    vector: new Float32Array([0.5, 0.5, 0.0]),
  });

  const results = await db.search({
    vector: new Float32Array([1.0, 0.0, 0.0]),
    k: 2,
  });

  t.truthy(results.length >= 1);
  t.is(results[0].id, 'v1');
});

test('VectorDB - HNSW index configuration', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 128,
    storagePath: join(tempDir, 'test.db'),
    hnswConfig: {
      m: 16,
      efConstruction: 100,
      efSearch: 50,
      maxElements: 10000,
    },
  });

  // Insert some vectors
  const vectors = Array.from({ length: 10 }, (_, i) =>
    new Float32Array(128).fill(0).map((_, j) => (i + j) * 0.01)
  );

  const ids = await db.insertBatch(
    vectors.map((vector) => ({ vector }))
  );

  t.is(ids.length, 10);

  const results = await db.search({
    vector: vectors[0],
    k: 5,
  });

  t.truthy(results.length >= 1);
});

test('VectorDB - memory stress test', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 128,
    storagePath: join(tempDir, 'test.db'),
  });

  // Insert 1000 vectors in batches
  const batchSize = 100;
  const totalVectors = 1000;

  for (let i = 0; i < totalVectors / batchSize; i++) {
    const batch = Array.from({ length: batchSize }, (_, j) => ({
      vector: new Float32Array(128).fill(0).map((_, k) => Math.random()),
    }));

    await db.insertBatch(batch);
  }

  const count = await db.len();
  t.is(count, totalVectors);

  // Search should still work
  const results = await db.search({
    vector: new Float32Array(128).fill(0).map(() => Math.random()),
    k: 10,
  });

  t.is(results.length, 10);
});

test('VectorDB - concurrent operations', async (t) => {
  const tempDir = createTempDir();
  t.teardown(() => cleanupTempDir(tempDir));

  const db = new VectorDB({
    dimensions: 3,
    storagePath: join(tempDir, 'test.db'),
  });

  // Insert vectors concurrently
  const promises = Array.from({ length: 50 }, (_, i) =>
    db.insert({
      vector: new Float32Array([i, i + 1, i + 2]),
    })
  );

  const ids = await Promise.all(promises);
  t.is(ids.length, 50);
  t.is(new Set(ids).size, 50); // All IDs should be unique

  // Search concurrently
  const searchPromises = Array.from({ length: 10 }, () =>
    db.search({
      vector: new Float32Array([1, 2, 3]),
      k: 5,
    })
  );

  const results = await Promise.all(searchPromises);
  t.is(results.length, 10);
  results.forEach((r) => t.truthy(r.length >= 1));
});
