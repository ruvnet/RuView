/**
 * RuVector WASM Bindings - Unit Tests
 *
 * Tests for WASM integration with RuVector vector operations
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  MockWasmVectorIndex,
  MockWasmEmbedder,
  MockWasmRouter,
  createMockRuVectorBindings,
  mockWasmLoader,
  resetWasmMocks
} from '../../mocks/wasm.mock';

describe('WASM Vector Index', () => {
  let vectorIndex: MockWasmVectorIndex;

  beforeEach(() => {
    vectorIndex = new MockWasmVectorIndex(384);
  });

  describe('Basic Operations', () => {
    it('should add vectors', () => {
      const vector = new Float32Array(384).fill(0.5);
      vectorIndex.add('vec-001', vector);

      expect(vectorIndex.size()).toBe(1);
    });

    it('should throw on dimension mismatch when adding', () => {
      const wrongVector = new Float32Array(256).fill(0.5);

      expect(() => vectorIndex.add('vec-001', wrongVector)).toThrow('dimension mismatch');
    });

    it('should delete vectors', () => {
      const vector = new Float32Array(384).fill(0.5);
      vectorIndex.add('vec-001', vector);

      const deleted = vectorIndex.delete('vec-001');

      expect(deleted).toBe(true);
      expect(vectorIndex.size()).toBe(0);
    });

    it('should return false when deleting non-existent vector', () => {
      const deleted = vectorIndex.delete('non-existent');
      expect(deleted).toBe(false);
    });

    it('should clear all vectors', () => {
      const vector = new Float32Array(384).fill(0.5);
      vectorIndex.add('vec-001', vector);
      vectorIndex.add('vec-002', vector);

      vectorIndex.clear();

      expect(vectorIndex.size()).toBe(0);
    });
  });

  describe('Search Operations', () => {
    beforeEach(() => {
      // Add test vectors with known patterns
      const vec1 = new Float32Array(384).fill(0);
      vec1[0] = 1; // Unit vector in first dimension

      const vec2 = new Float32Array(384).fill(0);
      vec2[1] = 1; // Unit vector in second dimension

      const vec3 = new Float32Array(384).fill(0);
      vec3[0] = 0.707;
      vec3[1] = 0.707; // Between first and second

      vectorIndex.add('vec-1', vec1);
      vectorIndex.add('vec-2', vec2);
      vectorIndex.add('vec-3', vec3);
    });

    it('should search for similar vectors', () => {
      const query = new Float32Array(384).fill(0);
      query[0] = 1; // Query similar to vec-1

      const results = vectorIndex.search(query, 2);

      expect(results).toHaveLength(2);
      expect(results[0].id).toBe('vec-1');
      expect(results[0].score).toBeCloseTo(1, 5);
    });

    it('should return results sorted by similarity', () => {
      const query = new Float32Array(384).fill(0);
      query[0] = 0.5;
      query[1] = 0.5;

      const results = vectorIndex.search(query, 3);

      // Results should be sorted by score descending
      for (let i = 1; i < results.length; i++) {
        expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score);
      }
    });

    it('should respect topK limit', () => {
      const query = new Float32Array(384).fill(0.1);

      const results = vectorIndex.search(query, 1);

      expect(results).toHaveLength(1);
    });

    it('should throw on query dimension mismatch', () => {
      const wrongQuery = new Float32Array(256).fill(0.5);

      expect(() => vectorIndex.search(wrongQuery, 5)).toThrow('dimension mismatch');
    });

    it('should include distance in results', () => {
      const query = new Float32Array(384).fill(0);
      query[0] = 1;

      const results = vectorIndex.search(query, 1);

      expect(results[0]).toHaveProperty('distance');
      expect(results[0].distance).toBe(1 - results[0].score);
    });
  });
});

describe('WASM Embedder', () => {
  let embedder: MockWasmEmbedder;

  beforeEach(() => {
    embedder = new MockWasmEmbedder(384);
  });

  describe('Single Embedding', () => {
    it('should embed text into vector', () => {
      const embedding = embedder.embed('Hello, world!');

      expect(embedding).toBeInstanceOf(Float32Array);
      expect(embedding.length).toBe(384);
    });

    it('should return correct dimension', () => {
      expect(embedder.dimension()).toBe(384);
    });

    it('should produce normalized embeddings', () => {
      const embedding = embedder.embed('Test text');

      let norm = 0;
      for (let i = 0; i < embedding.length; i++) {
        norm += embedding[i] * embedding[i];
      }
      norm = Math.sqrt(norm);

      expect(norm).toBeCloseTo(1, 5);
    });

    it('should produce deterministic embeddings for same input', () => {
      const embedding1 = embedder.embed('Same text');
      const embedding2 = embedder.embed('Same text');

      for (let i = 0; i < embedding1.length; i++) {
        expect(embedding1[i]).toBe(embedding2[i]);
      }
    });

    it('should produce different embeddings for different inputs', () => {
      const embedding1 = embedder.embed('Text one');
      const embedding2 = embedder.embed('Text two');

      let identical = true;
      for (let i = 0; i < embedding1.length; i++) {
        if (embedding1[i] !== embedding2[i]) {
          identical = false;
          break;
        }
      }

      expect(identical).toBe(false);
    });
  });

  describe('Batch Embedding', () => {
    it('should embed batch of texts', () => {
      const texts = ['First text', 'Second text', 'Third text'];
      const embeddings = embedder.embedBatch(texts);

      expect(embeddings).toHaveLength(3);
      embeddings.forEach(e => {
        expect(e).toBeInstanceOf(Float32Array);
        expect(e.length).toBe(384);
      });
    });

    it('should handle empty batch', () => {
      const embeddings = embedder.embedBatch([]);
      expect(embeddings).toHaveLength(0);
    });

    it('should be consistent with single embedding', () => {
      const text = 'Consistent text';

      const singleEmbedding = embedder.embed(text);
      const batchEmbedding = embedder.embedBatch([text])[0];

      for (let i = 0; i < singleEmbedding.length; i++) {
        expect(singleEmbedding[i]).toBe(batchEmbedding[i]);
      }
    });
  });
});

describe('WASM Router', () => {
  let router: MockWasmRouter;

  beforeEach(() => {
    router = new MockWasmRouter();
  });

  describe('Route Management', () => {
    it('should add route', () => {
      router.addRoute('code.*', 'coder');

      const result = router.route('code generation request');

      expect(result.handler).toBe('coder');
    });

    it('should remove route', () => {
      router.addRoute('test.*', 'tester');

      const removed = router.removeRoute('test.*');

      expect(removed).toBe(true);
    });

    it('should return false when removing non-existent route', () => {
      const removed = router.removeRoute('non-existent');
      expect(removed).toBe(false);
    });
  });

  describe('Routing', () => {
    beforeEach(() => {
      router.addRoute('generate.*code', 'coder');
      router.addRoute('write.*test', 'tester');
      router.addRoute('review.*code', 'reviewer');
    });

    it('should route to correct handler', () => {
      const result = router.route('generate some code for me');

      expect(result.handler).toBe('coder');
      expect(result.confidence).toBeGreaterThan(0.5);
    });

    it('should fallback to default for unmatched input', () => {
      const result = router.route('random unrelated request');

      expect(result.handler).toBe('default');
      expect(result.confidence).toBe(0.5);
      expect(result.metadata.fallback).toBe(true);
    });

    it('should include context in metadata', () => {
      const context = { userId: 'user-001', sessionId: 'session-001' };

      const result = router.route('generate code', context);

      expect(result.metadata.context).toEqual(context);
    });

    it('should match patterns case-insensitively', () => {
      const result = router.route('GENERATE CODE');

      expect(result.handler).toBe('coder');
    });
  });
});

describe('WASM Loader', () => {
  beforeEach(() => {
    resetWasmMocks();
  });

  it('should load vector index', async () => {
    const index = await mockWasmLoader.loadVectorIndex(384);

    expect(index).toBeInstanceOf(MockWasmVectorIndex);
    expect(mockWasmLoader.loadVectorIndex).toHaveBeenCalledWith(384);
  });

  it('should load embedder', async () => {
    const embedder = await mockWasmLoader.loadEmbedder(768);

    expect(embedder).toBeInstanceOf(MockWasmEmbedder);
    expect(mockWasmLoader.loadEmbedder).toHaveBeenCalledWith(768);
  });

  it('should load router', async () => {
    const router = await mockWasmLoader.loadRouter();

    expect(router).toBeInstanceOf(MockWasmRouter);
    expect(mockWasmLoader.loadRouter).toHaveBeenCalled();
  });

  it('should check WASM support', () => {
    const supported = mockWasmLoader.isWasmSupported();

    expect(supported).toBe(true);
    expect(mockWasmLoader.isWasmSupported).toHaveBeenCalled();
  });

  it('should get WASM memory usage', () => {
    const memory = mockWasmLoader.getWasmMemory();

    expect(memory).toHaveProperty('used');
    expect(memory).toHaveProperty('total');
    expect(memory.used).toBeLessThan(memory.total);
  });
});

describe('RuVector Bindings Integration', () => {
  let bindings: ReturnType<typeof createMockRuVectorBindings>;

  beforeEach(() => {
    bindings = createMockRuVectorBindings();
  });

  describe('High-level API', () => {
    it('should index text and search', async () => {
      await bindings.index('doc-1', 'TypeScript is a typed superset of JavaScript');
      await bindings.index('doc-2', 'Python is a high-level programming language');
      await bindings.index('doc-3', 'JavaScript runs in the browser');

      const results = await bindings.search('TypeScript programming', 2);

      expect(results).toHaveLength(2);
      // doc-1 should be most similar due to "TypeScript"
      expect(results[0].id).toBe('doc-1');
    });

    it('should batch index multiple items', async () => {
      const items = [
        { id: 'doc-1', text: 'First document' },
        { id: 'doc-2', text: 'Second document' },
        { id: 'doc-3', text: 'Third document' }
      ];

      await bindings.batchIndex(items);

      const results = await bindings.search('document', 3);
      expect(results).toHaveLength(3);
    });

    it('should combine embedder and vector index', async () => {
      const text = 'Test document for embedding';

      // Index
      await bindings.index('test-doc', text);

      // Embed same text and search
      const embedding = bindings.embedder.embed(text);
      const results = bindings.vectorIndex.search(embedding, 1);

      expect(results).toHaveLength(1);
      expect(results[0].id).toBe('test-doc');
      expect(results[0].score).toBeCloseTo(1, 5);
    });
  });

  describe('Component Access', () => {
    it('should expose vector index', () => {
      expect(bindings.vectorIndex).toBeInstanceOf(MockWasmVectorIndex);
    });

    it('should expose embedder', () => {
      expect(bindings.embedder).toBeInstanceOf(MockWasmEmbedder);
    });

    it('should expose router', () => {
      expect(bindings.router).toBeInstanceOf(MockWasmRouter);
    });
  });
});

describe('WASM Performance Simulation', () => {
  let vectorIndex: MockWasmVectorIndex;
  let embedder: MockWasmEmbedder;

  beforeEach(() => {
    vectorIndex = new MockWasmVectorIndex(384);
    embedder = new MockWasmEmbedder(384);
  });

  it('should handle large number of vectors', () => {
    const count = 1000;

    for (let i = 0; i < count; i++) {
      const embedding = embedder.embed(`Document ${i}`);
      vectorIndex.add(`doc-${i}`, embedding);
    }

    expect(vectorIndex.size()).toBe(count);

    // Search should still work
    const query = embedder.embed('Document 500');
    const results = vectorIndex.search(query, 10);

    expect(results).toHaveLength(10);
  });

  it('should search efficiently in large index', () => {
    // Pre-populate with vectors
    for (let i = 0; i < 500; i++) {
      const embedding = embedder.embed(`Content ${i}`);
      vectorIndex.add(`doc-${i}`, embedding);
    }

    const query = embedder.embed('Content 250');

    const start = performance.now();
    const results = vectorIndex.search(query, 10);
    const duration = performance.now() - start;

    expect(results).toHaveLength(10);
    expect(duration).toBeLessThan(100); // Should complete in <100ms
  });

  it('should batch embed efficiently', () => {
    const texts = Array.from({ length: 100 }, (_, i) => `Text number ${i}`);

    const start = performance.now();
    const embeddings = embedder.embedBatch(texts);
    const duration = performance.now() - start;

    expect(embeddings).toHaveLength(100);
    expect(duration).toBeLessThan(50); // Should complete quickly
  });
});
