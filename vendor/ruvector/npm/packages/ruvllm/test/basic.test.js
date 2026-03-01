/**
 * Basic tests for @ruvector/ruvllm
 */

const { test, describe } = require('node:test');
const assert = require('node:assert');

// We test against the source for now
// In production, tests would run against dist/
const { RuvLLM, SimdOps, version, hasSimdSupport } = require('../dist/cjs/index.js');

describe('RuvLLM', () => {
  test('should create instance', () => {
    const llm = new RuvLLM();
    assert.ok(llm);
  });

  test('should create instance with config', () => {
    const llm = new RuvLLM({
      embeddingDim: 384,
      learningEnabled: false,
    });
    assert.ok(llm);
  });

  test('should query and get response', () => {
    const llm = new RuvLLM();
    const response = llm.query('test query');

    assert.ok(response.text);
    assert.ok(typeof response.confidence === 'number');
    assert.ok(response.model);
    assert.ok(response.requestId);
  });

  test('should generate text', () => {
    const llm = new RuvLLM();
    const text = llm.generate('test prompt');

    assert.ok(typeof text === 'string');
    assert.ok(text.length > 0);
  });

  test('should route queries', () => {
    const llm = new RuvLLM();
    const decision = llm.route('test query');

    assert.ok(decision.model);
    assert.ok(typeof decision.contextSize === 'number');
    assert.ok(typeof decision.temperature === 'number');
    assert.ok(typeof decision.confidence === 'number');
  });

  test('should add and search memory', () => {
    const llm = new RuvLLM();

    const id = llm.addMemory('test content', { type: 'test' });
    assert.ok(typeof id === 'number');

    const results = llm.searchMemory('test', 5);
    assert.ok(Array.isArray(results));
  });

  test('should compute embeddings', () => {
    const llm = new RuvLLM({ embeddingDim: 768 });
    const embedding = llm.embed('test text');

    assert.ok(Array.isArray(embedding));
    assert.strictEqual(embedding.length, 768);
  });

  test('should compute similarity', () => {
    const llm = new RuvLLM();
    const similarity = llm.similarity('hello', 'hello');

    assert.ok(typeof similarity === 'number');
    assert.ok(similarity >= 0 && similarity <= 1);
  });

  test('should return stats', () => {
    const llm = new RuvLLM();
    const stats = llm.stats();

    assert.ok(typeof stats.totalQueries === 'number');
    assert.ok(typeof stats.memoryNodes === 'number');
    assert.ok(typeof stats.avgLatencyMs === 'number');
  });

  test('should handle batch queries', () => {
    const llm = new RuvLLM();
    const response = llm.batchQuery({
      queries: ['query 1', 'query 2', 'query 3'],
    });

    assert.strictEqual(response.responses.length, 3);
    assert.ok(typeof response.totalLatencyMs === 'number');
  });
});

describe('SimdOps', () => {
  test('should create instance', () => {
    const simd = new SimdOps();
    assert.ok(simd);
  });

  test('should compute dot product', () => {
    const simd = new SimdOps();
    const result = simd.dotProduct([1, 2, 3], [4, 5, 6]);

    assert.strictEqual(result, 32); // 1*4 + 2*5 + 3*6 = 32
  });

  test('should compute cosine similarity', () => {
    const simd = new SimdOps();

    // Same vector should have similarity 1
    const same = simd.cosineSimilarity([1, 0, 0], [1, 0, 0]);
    assert.ok(Math.abs(same - 1) < 0.0001);

    // Orthogonal vectors should have similarity 0
    const ortho = simd.cosineSimilarity([1, 0, 0], [0, 1, 0]);
    assert.ok(Math.abs(ortho) < 0.0001);
  });

  test('should compute L2 distance', () => {
    const simd = new SimdOps();
    const result = simd.l2Distance([0, 0], [3, 4]);

    assert.strictEqual(result, 5); // sqrt(9 + 16) = 5
  });

  test('should compute softmax', () => {
    const simd = new SimdOps();
    const result = simd.softmax([1, 2, 3]);

    // Sum should be 1
    const sum = result.reduce((a, b) => a + b, 0);
    assert.ok(Math.abs(sum - 1) < 0.0001);

    // Should be monotonically increasing
    assert.ok(result[0] < result[1]);
    assert.ok(result[1] < result[2]);
  });

  test('should compute ReLU', () => {
    const simd = new SimdOps();
    const result = simd.relu([-1, 0, 1, 2]);

    assert.deepStrictEqual(result, [0, 0, 1, 2]);
  });

  test('should normalize vectors', () => {
    const simd = new SimdOps();
    const result = simd.normalize([3, 4]);

    // Should have unit length
    const norm = Math.sqrt(result[0] ** 2 + result[1] ** 2);
    assert.ok(Math.abs(norm - 1) < 0.0001);
  });

  test('should report capabilities', () => {
    const simd = new SimdOps();
    const caps = simd.capabilities();

    assert.ok(Array.isArray(caps));
    assert.ok(caps.length > 0);
  });
});

describe('Module exports', () => {
  test('should export version', () => {
    assert.ok(typeof version === 'function');
    const v = version();
    assert.ok(typeof v === 'string');
  });

  test('should export hasSimdSupport', () => {
    assert.ok(typeof hasSimdSupport === 'function');
    const has = hasSimdSupport();
    assert.ok(typeof has === 'boolean');
  });
});
