/**
 * Unit tests for ruvector main package
 * Tests platform detection, fallback logic, and TypeScript types
 */

const test = require('node:test');
const assert = require('node:assert');

// Test module loading and backend detection
test('ruvector - Backend Detection', async (t) => {
  await t.test('should load ruvector module', () => {
    const ruvector = require('ruvector');
    assert.ok(ruvector, 'Module should load');
    assert.ok(ruvector.VectorIndex, 'VectorIndex should be exported');
    assert.ok(ruvector.getBackendInfo, 'getBackendInfo should be exported');
    assert.ok(ruvector.isNativeAvailable, 'isNativeAvailable should be exported');
    assert.ok(ruvector.Utils, 'Utils should be exported');
  });

  await t.test('should detect backend type', () => {
    const { getBackendInfo } = require('ruvector');
    const info = getBackendInfo();

    assert.ok(info, 'Should return backend info');
    assert.ok(['native', 'wasm'].includes(info.type), 'Backend type should be native or wasm');
    assert.ok(info.version, 'Should have version');
    assert.ok(Array.isArray(info.features), 'Features should be an array');
  });

  await t.test('should check native availability', () => {
    const { isNativeAvailable } = require('ruvector');
    const hasNative = isNativeAvailable();

    assert.strictEqual(typeof hasNative, 'boolean', 'Should return boolean');
  });

  await t.test('should prioritize native over WASM when available', () => {
    const { getBackendInfo, isNativeAvailable } = require('ruvector');
    const info = getBackendInfo();
    const hasNative = isNativeAvailable();

    if (hasNative) {
      assert.strictEqual(info.type, 'native', 'Should use native when available');
      assert.ok(
        info.features.includes('SIMD') || info.features.includes('Multi-threading'),
        'Native should have performance features'
      );
    } else {
      assert.strictEqual(info.type, 'wasm', 'Should fallback to WASM');
      assert.ok(
        info.features.includes('Browser-compatible'),
        'WASM should have browser compatibility'
      );
    }
  });
});

// Test VectorIndex creation
test('ruvector - VectorIndex Creation', async (t) => {
  const { VectorIndex } = require('ruvector');

  await t.test('should create VectorIndex with options', () => {
    const index = new VectorIndex({
      dimension: 128,
      metric: 'cosine',
      indexType: 'hnsw'
    });

    assert.ok(index, 'VectorIndex should be created');
  });

  await t.test('should create VectorIndex with minimal options', () => {
    const index = new VectorIndex({
      dimension: 64
    });

    assert.ok(index, 'VectorIndex with minimal options should be created');
  });

  await t.test('should accept various index types', () => {
    const flatIndex = new VectorIndex({
      dimension: 128,
      indexType: 'flat'
    });

    const hnswIndex = new VectorIndex({
      dimension: 128,
      indexType: 'hnsw'
    });

    assert.ok(flatIndex, 'Flat index should be created');
    assert.ok(hnswIndex, 'HNSW index should be created');
  });
});

// Test vector operations
test('ruvector - Vector Operations', async (t) => {
  const { VectorIndex } = require('ruvector');
  const dimension = 128;
  const index = new VectorIndex({ dimension, metric: 'cosine' });

  await t.test('should insert vector', async () => {
    await index.insert({
      id: 'test-1',
      values: Array.from({ length: dimension }, () => Math.random())
    });

    const stats = await index.stats();
    assert.ok(stats.vectorCount > 0, 'Should have vectors after insert');
  });

  await t.test('should insert batch of vectors', async () => {
    const vectors = Array.from({ length: 10 }, (_, i) => ({
      id: `batch-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    await index.insertBatch(vectors);

    const stats = await index.stats();
    assert.ok(stats.vectorCount >= 10, 'Should have at least 10 vectors');
  });

  await t.test('should insert batch with progress callback', async () => {
    const vectors = Array.from({ length: 20 }, (_, i) => ({
      id: `progress-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    let progressCalled = false;
    await index.insertBatch(vectors, {
      batchSize: 5,
      progressCallback: (progress) => {
        progressCalled = true;
        assert.ok(progress >= 0 && progress <= 1, 'Progress should be between 0 and 1');
      }
    });

    assert.ok(progressCalled, 'Progress callback should be called');
  });
});

// Test search operations
test('ruvector - Search Operations', async (t) => {
  const { VectorIndex } = require('ruvector');
  const dimension = 128;
  const index = new VectorIndex({ dimension, metric: 'cosine' });

  // Insert test data
  const testVectors = Array.from({ length: 50 }, (_, i) => ({
    id: `search-test-${i}`,
    values: Array.from({ length: dimension }, () => Math.random())
  }));
  await index.insertBatch(testVectors);

  await t.test('should search vectors', async () => {
    const query = Array.from({ length: dimension }, () => Math.random());
    const results = await index.search(query, { k: 10 });

    assert.ok(Array.isArray(results), 'Results should be an array');
    assert.ok(results.length > 0, 'Should return results');
    assert.ok(results.length <= 10, 'Should return at most k results');
  });

  await t.test('should return results with correct structure', async () => {
    const query = Array.from({ length: dimension }, () => Math.random());
    const results = await index.search(query, { k: 5 });

    results.forEach(result => {
      assert.ok(result.id, 'Result should have ID');
      assert.strictEqual(typeof result.score, 'number', 'Score should be a number');
    });
  });

  await t.test('should respect k parameter', async () => {
    const query = Array.from({ length: dimension }, () => Math.random());
    const results = await index.search(query, { k: 3 });

    assert.ok(results.length <= 3, 'Should return at most 3 results');
  });
});

// Test delete and get operations
test('ruvector - Delete and Get Operations', async (t) => {
  const { VectorIndex } = require('ruvector');
  const dimension = 128;
  const index = new VectorIndex({ dimension });

  await t.test('should get vector by ID', async () => {
    const vector = {
      id: 'get-test',
      values: Array.from({ length: dimension }, () => Math.random())
    };
    await index.insert(vector);

    const retrieved = await index.get('get-test');
    assert.ok(retrieved, 'Should retrieve vector');
    assert.strictEqual(retrieved.id, 'get-test', 'ID should match');
  });

  await t.test('should return null for non-existent ID', async () => {
    const retrieved = await index.get('non-existent');
    assert.strictEqual(retrieved, null, 'Should return null for non-existent ID');
  });

  await t.test('should delete vector', async () => {
    const vector = {
      id: 'delete-test',
      values: Array.from({ length: dimension }, () => Math.random())
    };
    await index.insert(vector);

    const deleted = await index.delete('delete-test');
    assert.strictEqual(deleted, true, 'Should return true for deleted vector');

    const retrieved = await index.get('delete-test');
    assert.strictEqual(retrieved, null, 'Deleted vector should not be retrievable');
  });
});

// Test stats and utility operations
test('ruvector - Stats and Utilities', async (t) => {
  const { VectorIndex } = require('ruvector');
  const dimension = 128;
  const index = new VectorIndex({ dimension });

  await t.test('should return stats', async () => {
    const stats = await index.stats();

    assert.ok(stats, 'Should return stats');
    assert.ok('vectorCount' in stats, 'Stats should have vectorCount');
    assert.ok('dimension' in stats, 'Stats should have dimension');
    assert.strictEqual(stats.dimension, dimension, 'Dimension should match');
  });

  await t.test('should clear index', async () => {
    await index.insert({
      id: 'clear-test',
      values: Array.from({ length: dimension }, () => Math.random())
    });

    await index.clear();

    const stats = await index.stats();
    assert.strictEqual(stats.vectorCount, 0, 'Index should be empty after clear');
  });

  await t.test('should optimize index', async () => {
    // Insert some vectors
    const vectors = Array.from({ length: 10 }, (_, i) => ({
      id: `opt-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));
    await index.insertBatch(vectors);

    // Should not throw
    await index.optimize();
    assert.ok(true, 'Optimize should complete without error');
  });
});

// Test Utils
test('ruvector - Utils', async (t) => {
  const { Utils } = require('ruvector');

  await t.test('should calculate cosine similarity', () => {
    const a = [1, 0, 0];
    const b = [1, 0, 0];
    const similarity = Utils.cosineSimilarity(a, b);

    assert.strictEqual(similarity, 1, 'Identical vectors should have similarity 1');
  });

  await t.test('should calculate cosine similarity for orthogonal vectors', () => {
    const a = [1, 0, 0];
    const b = [0, 1, 0];
    const similarity = Utils.cosineSimilarity(a, b);

    assert.ok(Math.abs(similarity) < 0.001, 'Orthogonal vectors should have similarity ~0');
  });

  await t.test('should throw on dimension mismatch for cosine', () => {
    assert.throws(
      () => Utils.cosineSimilarity([1, 2], [1, 2, 3]),
      /same dimension/i,
      'Should throw on dimension mismatch'
    );
  });

  await t.test('should calculate euclidean distance', () => {
    const a = [0, 0, 0];
    const b = [3, 4, 0];
    const distance = Utils.euclideanDistance(a, b);

    assert.strictEqual(distance, 5, 'Distance should be 5');
  });

  await t.test('should throw on dimension mismatch for euclidean', () => {
    assert.throws(
      () => Utils.euclideanDistance([1, 2], [1, 2, 3]),
      /same dimension/i,
      'Should throw on dimension mismatch'
    );
  });

  await t.test('should normalize vector', () => {
    const vector = [3, 4];
    const normalized = Utils.normalize(vector);

    assert.strictEqual(normalized[0], 0.6, 'First component should be 0.6');
    assert.strictEqual(normalized[1], 0.8, 'Second component should be 0.8');

    // Check magnitude is 1
    const magnitude = Math.sqrt(normalized[0] ** 2 + normalized[1] ** 2);
    assert.ok(Math.abs(magnitude - 1) < 0.001, 'Normalized vector should have magnitude 1');
  });

  await t.test('should generate random vector', () => {
    const dimension = 128;
    const vector = Utils.randomVector(dimension);

    assert.strictEqual(vector.length, dimension, 'Should have correct dimension');

    // Check it's normalized
    const magnitude = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
    assert.ok(Math.abs(magnitude - 1) < 0.001, 'Random vector should be normalized');
  });
});
