'use strict';
/**
 * Tests for NodeBackend string ID ↔ numeric label mapping (issue #114 fix).
 *
 * These tests exercise the mapping logic directly without requiring the
 * native @ruvector/rvf-node addon, by using a lightweight mock.
 */

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const os = require('os');

// ---------------------------------------------------------------------------
// Mock the native module so tests work without the N-API addon
// ---------------------------------------------------------------------------
class MockNativeHandle {
  constructor() {
    this.vectors = new Map(); // label → Float32Array
  }
  ingestBatch(flat, ids) {
    const dim = flat.length / ids.length;
    let accepted = 0;
    for (let i = 0; i < ids.length; i++) {
      const label = ids[i];
      // Mimic native behavior: NaN labels are silently ignored
      if (Number.isNaN(label) || label === undefined) continue;
      this.vectors.set(label, flat.slice(i * dim, (i + 1) * dim));
      accepted++;
    }
    return { accepted, rejected: ids.length - accepted, epoch: 1 };
  }
  query(vector, k) {
    const results = [];
    for (const [id, vec] of this.vectors) {
      let dist = 0;
      for (let i = 0; i < vector.length; i++) dist += (vector[i] - vec[i]) ** 2;
      results.push({ id, distance: Math.sqrt(dist) });
    }
    results.sort((a, b) => a.distance - b.distance);
    return results.slice(0, k);
  }
  delete(numIds) {
    let deleted = 0;
    for (const id of numIds) {
      if (this.vectors.delete(id)) deleted++;
    }
    return { deleted, epoch: 1 };
  }
  status() { return { total_vectors: this.vectors.size, total_segments: 1, file_size: 0, current_epoch: 0, profile_id: 0, compaction_state: 'idle', dead_space_ratio: 0, read_only: false }; }
  close() { this.vectors.clear(); }
  dimension() { return 4; }
}

// ---------------------------------------------------------------------------
// We test NodeBackend by patching its loadNative to use our mock.
// ---------------------------------------------------------------------------
const { NodeBackend } = require('../dist/backend');

async function createTestBackend(tmpDir) {
  const storePath = path.join(tmpDir, 'test.rvf');
  const backend = new NodeBackend();
  // Patch internals: skip native loading, use mock handle
  backend['native'] = { create: () => new MockNativeHandle(), open: () => new MockNativeHandle() };
  backend['handle'] = new MockNativeHandle();
  backend['storePath'] = storePath;
  backend['idToLabel'] = new Map();
  backend['labelToId'] = new Map();
  backend['nextLabel'] = 1;
  return { backend, storePath };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
let passed = 0, failed = 0;

async function test(name, fn) {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'rvf-id-test-'));
  try {
    await fn(tmpDir);
    console.log(`  PASS  ${name}`);
    passed++;
  } catch (err) {
    console.log(`  FAIL  ${name}: ${err.message}`);
    failed++;
  } finally {
    // Cleanup
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch {}
  }
}

(async () => {
  console.log('--- NodeBackend ID Mapping (Issue #114) ---');

  await test('string IDs are mapped to numeric labels', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    const vec = new Float32Array([1, 2, 3, 4]);
    await backend.ingestBatch([
      { id: 'chunk_0', vector: vec },
      { id: 'uuid-abc-123', vector: vec },
      { id: 'da003664_2b0f6ff3747e', vector: vec },
    ]);
    const handle = backend['handle'];
    // All 3 should have been accepted (no NaN labels)
    assert.strictEqual(handle.vectors.size, 3, `Expected 3 vectors, got ${handle.vectors.size}`);
  });

  await test('numeric labels are sequential starting at 1', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    const vec = new Float32Array([1, 2, 3, 4]);
    await backend.ingestBatch([
      { id: 'alpha', vector: vec },
      { id: 'beta', vector: vec },
      { id: 'gamma', vector: vec },
    ]);
    assert.strictEqual(backend['idToLabel'].get('alpha'), 1);
    assert.strictEqual(backend['idToLabel'].get('beta'), 2);
    assert.strictEqual(backend['idToLabel'].get('gamma'), 3);
    assert.strictEqual(backend['nextLabel'], 4);
  });

  await test('duplicate IDs reuse the same label', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    const vec = new Float32Array([1, 2, 3, 4]);
    await backend.ingestBatch([{ id: 'dup', vector: vec }]);
    await backend.ingestBatch([{ id: 'dup', vector: vec }]);
    assert.strictEqual(backend['idToLabel'].get('dup'), 1);
    assert.strictEqual(backend['nextLabel'], 2); // Only 1 unique label allocated
  });

  await test('query returns original string IDs, not numeric labels', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    await backend.ingestBatch([
      { id: 'doc_hello', vector: new Float32Array([1, 0, 0, 0]) },
      { id: 'doc_world', vector: new Float32Array([0, 1, 0, 0]) },
    ]);
    const results = await backend.query(new Float32Array([1, 0, 0, 0]), 2);
    const ids = results.map((r) => r.id);
    assert.ok(ids.includes('doc_hello'), `Expected doc_hello in results, got ${ids}`);
    assert.ok(ids.includes('doc_world'), `Expected doc_world in results, got ${ids}`);
  });

  await test('delete resolves string IDs to labels', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    await backend.ingestBatch([
      { id: 'to_keep', vector: new Float32Array([1, 0, 0, 0]) },
      { id: 'to_delete', vector: new Float32Array([0, 1, 0, 0]) },
    ]);
    const result = await backend.delete(['to_delete']);
    assert.strictEqual(result.deleted, 1);
    assert.strictEqual(backend['handle'].vectors.size, 1);
    // Mapping should be cleaned up
    assert.strictEqual(backend['idToLabel'].has('to_delete'), false);
    assert.strictEqual(backend['idToLabel'].has('to_keep'), true);
  });

  await test('delete of unknown ID returns 0', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    await backend.ingestBatch([{ id: 'exists', vector: new Float32Array([1, 0, 0, 0]) }]);
    const result = await backend.delete(['nonexistent']);
    assert.strictEqual(result.deleted, 0);
  });

  await test('mappings are persisted to sidecar JSON file', async (tmp) => {
    const { backend, storePath } = await createTestBackend(tmp);
    // Create a dummy file so the path directory exists
    fs.writeFileSync(storePath, '');
    await backend.ingestBatch([
      { id: 'persist_a', vector: new Float32Array([1, 0, 0, 0]) },
      { id: 'persist_b', vector: new Float32Array([0, 1, 0, 0]) },
    ]);
    const mapFile = storePath + '.idmap.json';
    assert.ok(fs.existsSync(mapFile), `Mapping file not created at ${mapFile}`);
    const data = JSON.parse(fs.readFileSync(mapFile, 'utf-8'));
    assert.strictEqual(data.idToLabel['persist_a'], 1);
    assert.strictEqual(data.idToLabel['persist_b'], 2);
    assert.strictEqual(data.labelToId['1'], 'persist_a');
    assert.strictEqual(data.labelToId['2'], 'persist_b');
    assert.strictEqual(data.nextLabel, 3);
  });

  await test('mappings are restored from sidecar JSON on open', async (tmp) => {
    const storePath = path.join(tmp, 'restore.rvf');
    fs.writeFileSync(storePath, '');
    // Write a sidecar mapping file manually
    const mapData = {
      idToLabel: { 'restored_x': 10, 'restored_y': 20 },
      labelToId: { '10': 'restored_x', '20': 'restored_y' },
      nextLabel: 21,
    };
    fs.writeFileSync(storePath + '.idmap.json', JSON.stringify(mapData));

    const backend = new NodeBackend();
    backend['native'] = { open: () => new MockNativeHandle() };
    backend['handle'] = new MockNativeHandle();
    backend['storePath'] = storePath;
    // Simulate loadMappings
    await backend['loadMappings']();

    assert.strictEqual(backend['idToLabel'].get('restored_x'), 10);
    assert.strictEqual(backend['idToLabel'].get('restored_y'), 20);
    assert.strictEqual(backend['labelToId'].get(10), 'restored_x');
    assert.strictEqual(backend['labelToId'].get(20), 'restored_y');
    assert.strictEqual(backend['nextLabel'], 21);
  });

  await test('purely numeric string IDs still work correctly', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    await backend.ingestBatch([
      { id: '42', vector: new Float32Array([1, 0, 0, 0]) },
      { id: '99', vector: new Float32Array([0, 1, 0, 0]) },
    ]);
    // They get mapped labels, not passed through as raw numbers
    assert.strictEqual(backend['idToLabel'].get('42'), 1);
    assert.strictEqual(backend['idToLabel'].get('99'), 2);
    const results = await backend.query(new Float32Array([1, 0, 0, 0]), 2);
    const ids = results.map((r) => r.id);
    assert.ok(ids.includes('42'), `Expected '42' in results`);
    assert.ok(ids.includes('99'), `Expected '99' in results`);
  });

  await test('mixed numeric and string IDs coexist', async (tmp) => {
    const { backend } = await createTestBackend(tmp);
    await backend.ingestBatch([
      { id: '1', vector: new Float32Array([1, 0, 0, 0]) },
      { id: 'uuid-abc', vector: new Float32Array([0, 1, 0, 0]) },
      { id: '999', vector: new Float32Array([0, 0, 1, 0]) },
      { id: 'chunk_42', vector: new Float32Array([0, 0, 0, 1]) },
    ]);
    assert.strictEqual(backend['handle'].vectors.size, 4);
    const results = await backend.query(new Float32Array([1, 0, 0, 0]), 4);
    const ids = new Set(results.map((r) => r.id));
    assert.ok(ids.has('1'));
    assert.ok(ids.has('uuid-abc'));
    assert.ok(ids.has('999'));
    assert.ok(ids.has('chunk_42'));
  });

  // Print results
  console.log(`\n${'='.repeat(60)}`);
  console.log(`Results: ${passed} passed, ${failed} failed, ${passed + failed} total`);
  console.log('='.repeat(60));
  process.exit(failed > 0 ? 1 : 0);
})();
