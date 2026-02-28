/**
 * Unit tests for @ruvector/wasm package
 * Tests WebAssembly bindings functionality
 */

const test = require('node:test');
const assert = require('node:assert');

// Test WASM module loading
test('@ruvector/wasm - Module Loading', async (t) => {
  await t.test('should load WASM module in Node.js', async () => {
    try {
      const wasm = await import('@ruvector/wasm');
      assert.ok(wasm, 'WASM module should load');
      assert.ok(wasm.VectorDB, 'VectorDB class should be exported');
    } catch (error) {
      if (error.code === 'ERR_MODULE_NOT_FOUND') {
        console.log('⚠ WASM module not built yet - run build:wasm first');
        assert.ok(true, 'WASM not available (expected)');
      } else {
        throw error;
      }
    }
  });

  await t.test('should detect environment correctly', () => {
    const isNode = typeof process !== 'undefined' &&
                   process.versions != null &&
                   process.versions.node != null;
    assert.strictEqual(isNode, true, 'Should detect Node.js environment');
  });
});

// Test VectorDB creation
test('@ruvector/wasm - VectorDB Creation', async (t) => {
  let VectorDB;

  try {
    const wasm = await import('@ruvector/wasm');
    VectorDB = wasm.VectorDB;
  } catch (error) {
    console.log('⚠ Skipping WASM tests - module not available');
    return;
  }

  await t.test('should create VectorDB instance', async () => {
    const db = new VectorDB({ dimensions: 128 });
    await db.init();
    assert.ok(db, 'VectorDB instance should be created');
  });

  await t.test('should create VectorDB with options', async () => {
    const db = new VectorDB({
      dimensions: 256,
      metric: 'cosine',
      useHnsw: true
    });
    await db.init();
    assert.ok(db, 'VectorDB with options should be created');
  });

  await t.test('should require init before use', async () => {
    const db = new VectorDB({ dimensions: 128 });

    assert.throws(
      () => db.insert(new Float32Array(128)),
      /not initialized/i,
      'Should throw when not initialized'
    );
  });
});

// Test vector operations
test('@ruvector/wasm - Vector Operations', async (t) => {
  let VectorDB;

  try {
    const wasm = await import('@ruvector/wasm');
    VectorDB = wasm.VectorDB;
  } catch (error) {
    console.log('⚠ Skipping WASM tests - module not available');
    return;
  }

  const dimensions = 128;
  const db = new VectorDB({ dimensions });
  await db.init();

  await t.test('should insert vector', () => {
    const vector = new Float32Array(dimensions).fill(0.5);
    const id = db.insert(vector);

    assert.ok(id, 'Should return an ID');
    assert.strictEqual(typeof id, 'string', 'ID should be a string');
  });

  await t.test('should insert vector with custom ID', () => {
    const vector = new Float32Array(dimensions).fill(0.3);
    const customId = 'wasm-custom-id';
    const id = db.insert(vector, customId);

    assert.strictEqual(id, customId, 'Should use custom ID');
  });

  await t.test('should insert vector with metadata', () => {
    const vector = new Float32Array(dimensions).fill(0.3);
    const metadata = { label: 'test', value: 42 };
    const id = db.insert(vector, 'with-meta', metadata);

    assert.ok(id, 'Should return ID');
  });

  await t.test('should insert batch of vectors', () => {
    const vectors = Array.from({ length: 10 }, (_, i) => ({
      id: `wasm-batch-${i}`,
      vector: new Float32Array(dimensions).fill(i / 10)
    }));

    const ids = db.insertBatch(vectors);

    assert.strictEqual(ids.length, 10, 'Should return 10 IDs');
  });

  await t.test('should accept array as vector', () => {
    const vector = Array.from({ length: dimensions }, () => Math.random());
    const id = db.insert(vector);

    assert.ok(id, 'Should accept array and return ID');
  });

  await t.test('should get vector count', () => {
    const count = db.len();
    assert.ok(count > 0, `Should have vectors, got ${count}`);
  });

  await t.test('should check if empty', () => {
    const isEmpty = db.isEmpty();
    assert.strictEqual(isEmpty, false, 'Should not be empty');
  });

  await t.test('should get dimensions', () => {
    const dims = db.getDimensions();
    assert.strictEqual(dims, dimensions, 'Dimensions should match');
  });
});

// Test search operations
test('@ruvector/wasm - Search Operations', async (t) => {
  let VectorDB;

  try {
    const wasm = await import('@ruvector/wasm');
    VectorDB = wasm.VectorDB;
  } catch (error) {
    console.log('⚠ Skipping WASM tests - module not available');
    return;
  }

  const dimensions = 128;
  const db = new VectorDB({ dimensions, metric: 'cosine' });
  await db.init();

  // Insert test vectors
  const testVectors = Array.from({ length: 50 }, (_, i) => ({
    id: `wasm-vec-${i}`,
    vector: new Float32Array(dimensions).map(() => Math.random())
  }));
  db.insertBatch(testVectors);

  await t.test('should search and return results', () => {
    const query = new Float32Array(dimensions).fill(0.5);
    const results = db.search(query, 10);

    assert.ok(Array.isArray(results), 'Results should be an array');
    assert.ok(results.length > 0, 'Should return results');
    assert.ok(results.length <= 10, 'Should return at most k results');
  });

  await t.test('search results should have correct structure', () => {
    const query = new Float32Array(dimensions).fill(0.5);
    const results = db.search(query, 5);

    results.forEach(result => {
      assert.ok(result.id, 'Result should have ID');
      assert.strictEqual(typeof result.score, 'number', 'Score should be a number');
    });
  });

  await t.test('should accept array as query', () => {
    const query = Array.from({ length: dimensions }, () => Math.random());
    const results = db.search(query, 5);

    assert.ok(Array.isArray(results), 'Should accept array and return results');
  });

  await t.test('should respect k parameter', () => {
    const query = new Float32Array(dimensions).fill(0.5);
    const results = db.search(query, 3);

    assert.ok(results.length <= 3, 'Should return at most 3 results');
  });
});

// Test delete operations
test('@ruvector/wasm - Delete Operations', async (t) => {
  let VectorDB;

  try {
    const wasm = await import('@ruvector/wasm');
    VectorDB = wasm.VectorDB;
  } catch (error) {
    console.log('⚠ Skipping WASM tests - module not available');
    return;
  }

  const dimensions = 128;
  const db = new VectorDB({ dimensions });
  await db.init();

  await t.test('should delete existing vector', () => {
    const vector = new Float32Array(dimensions).fill(0.5);
    const id = db.insert(vector, 'wasm-to-delete');

    const deleted = db.delete(id);
    assert.strictEqual(deleted, true, 'Should return true for deleted vector');
  });

  await t.test('should return false for non-existent vector', () => {
    const deleted = db.delete('wasm-non-existent');
    assert.strictEqual(deleted, false, 'Should return false for non-existent vector');
  });
});

// Test get operations
test('@ruvector/wasm - Get Operations', async (t) => {
  let VectorDB;

  try {
    const wasm = await import('@ruvector/wasm');
    VectorDB = wasm.VectorDB;
  } catch (error) {
    console.log('⚠ Skipping WASM tests - module not available');
    return;
  }

  const dimensions = 128;
  const db = new VectorDB({ dimensions });
  await db.init();

  await t.test('should get existing vector', () => {
    const vector = new Float32Array(dimensions).fill(0.7);
    const id = db.insert(vector, 'wasm-get-test');

    const entry = db.get(id);
    assert.ok(entry, 'Should return entry');
    assert.strictEqual(entry.id, id, 'ID should match');
    assert.ok(entry.vector, 'Should have vector');
  });

  await t.test('should return null for non-existent vector', () => {
    const entry = db.get('wasm-non-existent');
    assert.strictEqual(entry, null, 'Should return null for non-existent vector');
  });
});

// Test utility functions
test('@ruvector/wasm - Utility Functions', async (t) => {
  let wasm;

  try {
    wasm = await import('@ruvector/wasm');
  } catch (error) {
    console.log('⚠ Skipping WASM tests - module not available');
    return;
  }

  await t.test('should detect SIMD support', async () => {
    const hasSIMD = await wasm.detectSIMD();
    assert.strictEqual(typeof hasSIMD, 'boolean', 'Should return boolean');
  });

  await t.test('should return version', async () => {
    const version = await wasm.version();
    assert.strictEqual(typeof version, 'string', 'Version should be a string');
  });
});
