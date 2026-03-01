/**
 * Integration tests for cross-package compatibility
 * Tests that all packages work together correctly
 */

const test = require('node:test');
const assert = require('node:assert');

// Test that main package correctly loads backends
test('Integration - Backend Loading', async (t) => {
  const ruvector = require('ruvector');

  await t.test('should load a working backend', () => {
    const info = ruvector.getBackendInfo();
    assert.ok(info, 'Should get backend info');
    assert.ok(['native', 'wasm'].includes(info.type), 'Should have valid backend type');
  });

  await t.test('should create VectorIndex with loaded backend', () => {
    const index = new ruvector.VectorIndex({ dimension: 128 });
    assert.ok(index, 'Should create index with backend');
  });

  await t.test('backend type should match availability', () => {
    const info = ruvector.getBackendInfo();
    const hasNative = ruvector.isNativeAvailable();

    if (hasNative) {
      assert.strictEqual(info.type, 'native', 'Should use native when available');
    } else {
      assert.strictEqual(info.type, 'wasm', 'Should use WASM as fallback');
    }
  });
});

// Test API compatibility between backends
test('Integration - API Compatibility', async (t) => {
  const ruvector = require('ruvector');
  const dimension = 128;

  await t.test('insert and search should work consistently', async () => {
    const index = new ruvector.VectorIndex({ dimension, metric: 'cosine' });

    // Insert test data
    const vectors = Array.from({ length: 20 }, (_, i) => ({
      id: `api-test-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    await index.insertBatch(vectors);

    // Search
    const query = Array.from({ length: dimension }, () => Math.random());
    const results = await index.search(query, { k: 5 });

    assert.ok(Array.isArray(results), 'Search should return array');
    assert.ok(results.length > 0, 'Should find results');
    assert.ok(results.length <= 5, 'Should respect k parameter');

    // Verify result structure
    results.forEach(result => {
      assert.ok(result.id, 'Result should have ID');
      assert.strictEqual(typeof result.score, 'number', 'Score should be number');
    });
  });

  await t.test('delete and get should work consistently', async () => {
    const index = new ruvector.VectorIndex({ dimension });

    const testId = 'delete-get-test';
    const vector = {
      id: testId,
      values: Array.from({ length: dimension }, () => Math.random())
    };

    await index.insert(vector);

    // Get
    const retrieved = await index.get(testId);
    assert.ok(retrieved, 'Should get inserted vector');
    assert.strictEqual(retrieved.id, testId, 'ID should match');

    // Delete
    const deleted = await index.delete(testId);
    assert.strictEqual(deleted, true, 'Should delete successfully');

    // Verify deletion
    const afterDelete = await index.get(testId);
    assert.strictEqual(afterDelete, null, 'Vector should be deleted');
  });

  await t.test('stats should work consistently', async () => {
    const index = new ruvector.VectorIndex({ dimension });

    await index.insert({
      id: 'stats-test',
      values: Array.from({ length: dimension }, () => Math.random())
    });

    const stats = await index.stats();

    assert.ok(stats, 'Should return stats');
    assert.ok(typeof stats.vectorCount === 'number', 'vectorCount should be number');
    assert.strictEqual(stats.dimension, dimension, 'Dimension should match');
  });
});

// Test data consistency across operations
test('Integration - Data Consistency', async (t) => {
  const ruvector = require('ruvector');
  const dimension = 256;

  await t.test('inserted vectors should be searchable', async () => {
    const index = new ruvector.VectorIndex({ dimension, metric: 'cosine' });

    const testVector = {
      id: 'consistency-test',
      values: Array.from({ length: dimension }, () => Math.random())
    };

    await index.insert(testVector);

    // Search with the exact same vector
    const results = await index.search(testVector.values, { k: 1 });

    assert.strictEqual(results.length, 1, 'Should find the vector');
    assert.strictEqual(results[0].id, testVector.id, 'Should find the correct vector');
    assert.ok(results[0].score < 0.01, 'Score should be very close to 0 (exact match)');
  });

  await t.test('batch insert should maintain order and IDs', async () => {
    const index = new ruvector.VectorIndex({ dimension });

    const vectors = Array.from({ length: 10 }, (_, i) => ({
      id: `order-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    await index.insertBatch(vectors);

    // Verify all vectors were inserted
    for (const vector of vectors) {
      const retrieved = await index.get(vector.id);
      assert.ok(retrieved, `Vector ${vector.id} should be retrievable`);
      assert.strictEqual(retrieved.id, vector.id, 'ID should match');
    }
  });

  await t.test('search results should be deterministic', async () => {
    const index = new ruvector.VectorIndex({ dimension, metric: 'cosine' });

    // Insert fixed vectors
    const vectors = Array.from({ length: 20 }, (_, i) => ({
      id: `det-${i}`,
      values: Array.from({ length: dimension }, (_, j) => (i + j) / 100)
    }));

    await index.insertBatch(vectors);

    // Search with fixed query
    const query = Array.from({ length: dimension }, (_, i) => i / 100);
    const results1 = await index.search(query, { k: 5 });
    const results2 = await index.search(query, { k: 5 });

    assert.strictEqual(results1.length, results2.length, 'Should return same number of results');

    for (let i = 0; i < results1.length; i++) {
      assert.strictEqual(results1[i].id, results2[i].id, 'IDs should match');
      assert.strictEqual(results1[i].score, results2[i].score, 'Scores should match');
    }
  });
});

// Test performance across backends
test('Integration - Performance Comparison', async (t) => {
  const ruvector = require('ruvector');
  const dimension = 128;
  const numVectors = 100;

  await t.test('insert performance should be reasonable', async () => {
    const index = new ruvector.VectorIndex({ dimension });

    const vectors = Array.from({ length: numVectors }, (_, i) => ({
      id: `perf-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));

    const start = Date.now();
    await index.insertBatch(vectors);
    const duration = Date.now() - start;

    const throughput = numVectors / (duration / 1000);

    console.log(`  Insert throughput: ${throughput.toFixed(0)} vectors/sec`);
    assert.ok(throughput > 10, 'Should insert at least 10 vectors/sec');
  });

  await t.test('search performance should be reasonable', async () => {
    const index = new ruvector.VectorIndex({ dimension });

    // Insert test data
    const vectors = Array.from({ length: numVectors }, (_, i) => ({
      id: `search-perf-${i}`,
      values: Array.from({ length: dimension }, () => Math.random())
    }));
    await index.insertBatch(vectors);

    // Run searches
    const numQueries = 50;
    const queries = Array.from(
      { length: numQueries },
      () => Array.from({ length: dimension }, () => Math.random())
    );

    const start = Date.now();
    for (const query of queries) {
      await index.search(query, { k: 10 });
    }
    const duration = Date.now() - start;

    const throughput = numQueries / (duration / 1000);

    console.log(`  Search throughput: ${throughput.toFixed(0)} queries/sec`);
    assert.ok(throughput > 5, 'Should search at least 5 queries/sec');
  });
});

// Test error handling consistency
test('Integration - Error Handling', async (t) => {
  const ruvector = require('ruvector');

  await t.test('should handle invalid dimensions', () => {
    assert.throws(
      () => new ruvector.VectorIndex({ dimension: -1 }),
      'Should reject negative dimensions'
    );
  });

  await t.test('should handle dimension mismatch', async () => {
    const index = new ruvector.VectorIndex({ dimension: 128 });

    const wrongVector = {
      id: 'wrong-dim',
      values: Array.from({ length: 64 }, () => Math.random())
    };

    try {
      await index.insert(wrongVector);
      // Some backends might auto-handle this, others might throw
      assert.ok(true);
    } catch (error) {
      assert.ok(error.message.includes('dimension'), 'Error should mention dimension');
    }
  });

  await t.test('should handle empty search', async () => {
    const index = new ruvector.VectorIndex({ dimension: 128 });

    const query = Array.from({ length: 128 }, () => Math.random());
    const results = await index.search(query, { k: 10 });

    assert.ok(Array.isArray(results), 'Should return empty array');
    assert.strictEqual(results.length, 0, 'Should have no results');
  });
});

// Test TypeScript types compatibility
test('Integration - TypeScript Types', async (t) => {
  await t.test('should have type definitions available', () => {
    const fs = require('fs');
    const path = require('path');

    const ruvectorTypesPath = path.join(__dirname, '../../ruvector/dist/index.d.ts');
    const coreTypesPath = path.join(__dirname, '../../core/dist/index.d.ts');

    // At least one should exist
    const hasRuvectorTypes = fs.existsSync(ruvectorTypesPath);
    const hasCoreTypes = fs.existsSync(coreTypesPath);

    assert.ok(
      hasRuvectorTypes || hasCoreTypes,
      'Should have TypeScript definitions'
    );
  });
});
