/**
 * Tests for @ruvector/rudag
 */

import { test, describe, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { RuDag, DagOperator, AttentionMechanism, MemoryStorage, createStorage } from './index';

describe('RuDag', () => {
  let dag: RuDag;

  beforeEach(async () => {
    dag = new RuDag({ storage: new MemoryStorage(), autoSave: false });
    await dag.init();
  });

  afterEach(() => {
    dag.dispose();
  });

  test('should create empty DAG', () => {
    assert.strictEqual(dag.nodeCount, 0);
    assert.strictEqual(dag.edgeCount, 0);
  });

  test('should add nodes', () => {
    const id1 = dag.addNode(DagOperator.SCAN, 10.0);
    const id2 = dag.addNode(DagOperator.FILTER, 2.0);

    assert.strictEqual(id1, 0);
    assert.strictEqual(id2, 1);
    assert.strictEqual(dag.nodeCount, 2);
  });

  test('should add edges', () => {
    const n1 = dag.addNode(DagOperator.SCAN, 10.0);
    const n2 = dag.addNode(DagOperator.FILTER, 2.0);

    const success = dag.addEdge(n1, n2);
    assert.strictEqual(success, true);
    assert.strictEqual(dag.edgeCount, 1);
  });

  test('should reject cycles', () => {
    const n1 = dag.addNode(DagOperator.SCAN, 1.0);
    const n2 = dag.addNode(DagOperator.FILTER, 1.0);
    const n3 = dag.addNode(DagOperator.PROJECT, 1.0);

    dag.addEdge(n1, n2);
    dag.addEdge(n2, n3);

    // This should fail - would create cycle
    const success = dag.addEdge(n3, n1);
    assert.strictEqual(success, false);
  });

  test('should compute topological sort', () => {
    const n1 = dag.addNode(DagOperator.SCAN, 1.0);
    const n2 = dag.addNode(DagOperator.FILTER, 1.0);
    const n3 = dag.addNode(DagOperator.PROJECT, 1.0);

    dag.addEdge(n1, n2);
    dag.addEdge(n2, n3);

    const topo = dag.topoSort();
    assert.deepStrictEqual(topo, [0, 1, 2]);
  });

  test('should find critical path', () => {
    const n1 = dag.addNode(DagOperator.SCAN, 10.0);
    const n2 = dag.addNode(DagOperator.FILTER, 2.0);
    const n3 = dag.addNode(DagOperator.PROJECT, 1.0);

    dag.addEdge(n1, n2);
    dag.addEdge(n2, n3);

    const result = dag.criticalPath();
    assert.deepStrictEqual(result.path, [0, 1, 2]);
    assert.strictEqual(result.cost, 13); // 10 + 2 + 1
  });

  test('should compute attention scores', () => {
    dag.addNode(DagOperator.SCAN, 1.0);
    dag.addNode(DagOperator.FILTER, 2.0);
    dag.addNode(DagOperator.PROJECT, 3.0);

    const uniform = dag.attention(AttentionMechanism.UNIFORM);
    assert.strictEqual(uniform.length, 3);
    // All should be approximately 0.333
    assert.ok(Math.abs(uniform[0] - 0.333) < 0.01);

    const topo = dag.attention(AttentionMechanism.TOPOLOGICAL);
    assert.strictEqual(topo.length, 3);

    const critical = dag.attention(AttentionMechanism.CRITICAL_PATH);
    assert.strictEqual(critical.length, 3);
  });

  test('should serialize to JSON', () => {
    dag.addNode(DagOperator.SCAN, 1.0);
    dag.addNode(DagOperator.FILTER, 2.0);
    dag.addEdge(0, 1);

    const json = dag.toJSON();
    assert.ok(json.includes('nodes'));
    assert.ok(json.includes('edges'));
  });

  test('should serialize to bytes', () => {
    dag.addNode(DagOperator.SCAN, 1.0);
    dag.addNode(DagOperator.FILTER, 2.0);
    dag.addEdge(0, 1);

    const bytes = dag.toBytes();
    assert.ok(bytes instanceof Uint8Array);
    assert.ok(bytes.length > 0);
  });

  test('should round-trip through JSON', async () => {
    const n1 = dag.addNode(DagOperator.SCAN, 10.0);
    const n2 = dag.addNode(DagOperator.FILTER, 2.0);
    dag.addEdge(n1, n2);

    const json = dag.toJSON();
    const restored = await RuDag.fromJSON(json, { storage: null });

    assert.strictEqual(restored.nodeCount, 2);
    assert.strictEqual(restored.edgeCount, 1);

    restored.dispose();
  });

  test('should round-trip through bytes', async () => {
    const n1 = dag.addNode(DagOperator.SCAN, 10.0);
    const n2 = dag.addNode(DagOperator.FILTER, 2.0);
    dag.addEdge(n1, n2);

    const bytes = dag.toBytes();
    const restored = await RuDag.fromBytes(bytes, { storage: null });

    assert.strictEqual(restored.nodeCount, 2);
    assert.strictEqual(restored.edgeCount, 1);

    restored.dispose();
  });
});

describe('MemoryStorage', () => {
  let storage: MemoryStorage;

  beforeEach(async () => {
    storage = new MemoryStorage();
    await storage.init();
  });

  test('should save and retrieve DAG', async () => {
    const data = new Uint8Array([1, 2, 3, 4]);
    await storage.save('test-dag', data, { name: 'Test DAG' });

    const retrieved = await storage.get('test-dag');
    assert.ok(retrieved);
    assert.strictEqual(retrieved.id, 'test-dag');
    assert.strictEqual(retrieved.name, 'Test DAG');
    assert.deepStrictEqual(Array.from(retrieved.data), [1, 2, 3, 4]);
  });

  test('should list all DAGs', async () => {
    await storage.save('dag-1', new Uint8Array([1]));
    await storage.save('dag-2', new Uint8Array([2]));

    const list = await storage.list();
    assert.strictEqual(list.length, 2);
  });

  test('should delete DAG', async () => {
    await storage.save('to-delete', new Uint8Array([1]));
    assert.ok(await storage.get('to-delete'));

    await storage.delete('to-delete');
    assert.strictEqual(await storage.get('to-delete'), null);
  });

  test('should find by name', async () => {
    await storage.save('dag-1', new Uint8Array([1]), { name: 'query' });
    await storage.save('dag-2', new Uint8Array([2]), { name: 'query' });
    await storage.save('dag-3', new Uint8Array([3]), { name: 'other' });

    const results = await storage.findByName('query');
    assert.strictEqual(results.length, 2);
  });

  test('should calculate stats', async () => {
    await storage.save('dag-1', new Uint8Array(100));
    await storage.save('dag-2', new Uint8Array(200));

    const stats = await storage.stats();
    assert.strictEqual(stats.count, 2);
    assert.strictEqual(stats.totalSize, 300);
  });

  test('should clear all', async () => {
    await storage.save('dag-1', new Uint8Array([1]));
    await storage.save('dag-2', new Uint8Array([2]));

    await storage.clear();
    const list = await storage.list();
    assert.strictEqual(list.length, 0);
  });
});

describe('createStorage', () => {
  test('should create MemoryStorage in Node.js', () => {
    const storage = createStorage();
    assert.ok(storage instanceof MemoryStorage);
  });
});
