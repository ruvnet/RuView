/**
 * Unit tests for @ruvector/core package
 * Tests native bindings functionality
 */

const test = require('node:test');
const assert = require('node:assert');

// Test platform detection and loading
test('@ruvector/core - Platform Detection', async (t) => {
  await t.test('should detect current platform correctly', () => {
    const os = require('node:os');
    const platform = os.platform();
    const arch = os.arch();

    assert.ok(['linux', 'darwin', 'win32'].includes(platform),
      `Platform ${platform} should be supported`);
    assert.ok(['x64', 'arm64'].includes(arch),
      `Architecture ${arch} should be supported`);
  });

  await t.test('should load native binding for current platform', () => {
    try {
      const core = require('@ruvector/core');
      assert.ok(core, 'Core module should load');
      assert.ok(core.VectorDB, 'VectorDB class should be exported');
      assert.ok(typeof core.version === 'function', 'version function should be exported');
      assert.ok(typeof core.hello === 'function', 'hello function should be exported');
    } catch (error) {
      if (error.code === 'MODULE_NOT_FOUND') {
        assert.ok(true, 'Native binding not available (expected in some environments)');
      } else {
        throw error;
      }
    }
  });
});

// Test VectorDB creation and basic operations
test('@ruvector/core - VectorDB Creation', async (t) => {
  let core;

  try {
    core = require('@ruvector/core');
  } catch (error) {
    console.log('⚠ Skipping core tests - native binding not available');
    return;
  }

  await t.test('should create VectorDB with dimensions', () => {
    const db = new core.VectorDB({ dimensions: 128 });
    assert.ok(db, 'VectorDB instance should be created');
  });

  await t.test('should create VectorDB with full options', () => {
    const db = new core.VectorDB({
      dimensions: 256,
      distanceMetric: 'Cosine',
      hnswConfig: {
        m: 16,
        efConstruction: 200,
        efSearch: 100
      }
    });
    assert.ok(db, 'VectorDB with full config should be created');
  });

  await t.test('should reject invalid dimensions', () => {
    assert.throws(
      () => new core.VectorDB({ dimensions: 0 }),
      /invalid.*dimension/i,
      'Should throw on zero dimensions'
    );
  });
});

// Test vector operations
test('@ruvector/core - Vector Operations', async (t) => {
  let core;

  try {
    core = require('@ruvector/core');
  } catch (error) {
    console.log('⚠ Skipping core tests - native binding not available');
    return;
  }

  const dimensions = 128;
  const db = new core.VectorDB({ dimensions });

  await t.test('should insert vector and return ID', async () => {
    const vector = new Float32Array(dimensions).fill(0.5);
    const id = await db.insert({ vector });

    assert.ok(id, 'Should return an ID');
    assert.strictEqual(typeof id, 'string', 'ID should be a string');
  });

  await t.test('should insert vector with custom ID', async () => {
    const vector = new Float32Array(dimensions).fill(0.3);
    const customId = 'custom-id-123';
    const id = await db.insert({ id: customId, vector });

    assert.strictEqual(id, customId, 'Should use custom ID');
  });

  await t.test('should insert batch of vectors', async () => {
    const vectors = Array.from({ length: 10 }, (_, i) => ({
      id: `batch-${i}`,
      vector: new Float32Array(dimensions).fill(i / 10)
    }));

    const ids = await db.insertBatch(vectors);

    assert.strictEqual(ids.length, 10, 'Should return 10 IDs');
    assert.deepStrictEqual(ids, vectors.map(v => v.id), 'IDs should match');
  });

  await t.test('should get vector count', async () => {
    const count = await db.len();
    assert.ok(count >= 12, `Should have at least 12 vectors, got ${count}`);
  });

  await t.test('should check if empty', async () => {
    const isEmpty = await db.isEmpty();
    assert.strictEqual(isEmpty, false, 'Should not be empty');
  });
});

// Test search operations
test('@ruvector/core - Search Operations', async (t) => {
  let core;

  try {
    core = require('@ruvector/core');
  } catch (error) {
    console.log('⚠ Skipping core tests - native binding not available');
    return;
  }

  const dimensions = 128;
  const db = new core.VectorDB({
    dimensions,
    distanceMetric: 'Cosine'
  });

  // Insert test vectors
  const testVectors = Array.from({ length: 100 }, (_, i) => ({
    id: `vec-${i}`,
    vector: new Float32Array(dimensions).map(() => Math.random())
  }));
  await db.insertBatch(testVectors);

  await t.test('should search and return results', async () => {
    const query = new Float32Array(dimensions).fill(0.5);
    const results = await db.search({ vector: query, k: 10 });

    assert.ok(Array.isArray(results), 'Results should be an array');
    assert.ok(results.length > 0, 'Should return results');
    assert.ok(results.length <= 10, 'Should return at most k results');
  });

  await t.test('search results should have correct structure', async () => {
    const query = new Float32Array(dimensions).fill(0.5);
    const results = await db.search({ vector: query, k: 5 });

    results.forEach(result => {
      assert.ok(result.id, 'Result should have ID');
      assert.strictEqual(typeof result.score, 'number', 'Score should be a number');
      assert.ok(result.score >= 0, 'Score should be non-negative');
    });
  });

  await t.test('should respect k parameter', async () => {
    const query = new Float32Array(dimensions).fill(0.5);
    const results = await db.search({ vector: query, k: 3 });

    assert.ok(results.length <= 3, 'Should return at most 3 results');
  });

  await t.test('results should be sorted by score', async () => {
    const query = new Float32Array(dimensions).fill(0.5);
    const results = await db.search({ vector: query, k: 10 });

    for (let i = 0; i < results.length - 1; i++) {
      assert.ok(
        results[i].score <= results[i + 1].score,
        'Results should be sorted by increasing distance'
      );
    }
  });
});

// Test delete operations
test('@ruvector/core - Delete Operations', async (t) => {
  let core;

  try {
    core = require('@ruvector/core');
  } catch (error) {
    console.log('⚠ Skipping core tests - native binding not available');
    return;
  }

  const dimensions = 128;
  const db = new core.VectorDB({ dimensions });

  await t.test('should delete existing vector', async () => {
    const vector = new Float32Array(dimensions).fill(0.5);
    const id = await db.insert({ id: 'to-delete', vector });

    const deleted = await db.delete(id);
    assert.strictEqual(deleted, true, 'Should return true for deleted vector');
  });

  await t.test('should return false for non-existent vector', async () => {
    const deleted = await db.delete('non-existent-id');
    assert.strictEqual(deleted, false, 'Should return false for non-existent vector');
  });
});

// Test get operations
test('@ruvector/core - Get Operations', async (t) => {
  let core;

  try {
    core = require('@ruvector/core');
  } catch (error) {
    console.log('⚠ Skipping core tests - native binding not available');
    return;
  }

  const dimensions = 128;
  const db = new core.VectorDB({ dimensions });

  await t.test('should get existing vector', async () => {
    const vector = new Float32Array(dimensions).fill(0.7);
    const id = await db.insert({ id: 'get-test', vector });

    const entry = await db.get(id);
    assert.ok(entry, 'Should return entry');
    assert.strictEqual(entry.id, id, 'ID should match');
    assert.ok(entry.vector, 'Should have vector');
  });

  await t.test('should return null for non-existent vector', async () => {
    const entry = await db.get('non-existent-id');
    assert.strictEqual(entry, null, 'Should return null for non-existent vector');
  });
});

// Test version and utility functions
test('@ruvector/core - Utility Functions', async (t) => {
  let core;

  try {
    core = require('@ruvector/core');
  } catch (error) {
    console.log('⚠ Skipping core tests - native binding not available');
    return;
  }

  await t.test('version should return string', () => {
    const version = core.version();
    assert.strictEqual(typeof version, 'string', 'Version should be a string');
    assert.ok(version.length > 0, 'Version should not be empty');
  });

  await t.test('hello should return string', () => {
    const greeting = core.hello();
    assert.strictEqual(typeof greeting, 'string', 'Hello should return a string');
    assert.ok(greeting.length > 0, 'Greeting should not be empty');
  });
});
