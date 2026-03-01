/**
 * RuVector WASM Bindings - Integration Tests
 *
 * Tests for RuVector vector database integration with WASM bindings
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  createMockRuVectorBindings,
  MockWasmVectorIndex,
  MockWasmEmbedder,
  mockWasmLoader
} from '../../mocks/wasm.mock';

describe('RuVector WASM Integration', () => {
  let ruvector: ReturnType<typeof createMockRuVectorBindings>;

  beforeEach(() => {
    ruvector = createMockRuVectorBindings();
  });

  describe('Document Indexing', () => {
    it('should index single document', async () => {
      await ruvector.index('doc-1', 'This is a test document about programming');

      expect(ruvector.vectorIndex.size()).toBe(1);
    });

    it('should index multiple documents', async () => {
      await ruvector.index('doc-1', 'React component patterns');
      await ruvector.index('doc-2', 'Vue.js best practices');
      await ruvector.index('doc-3', 'Angular architecture guide');

      expect(ruvector.vectorIndex.size()).toBe(3);
    });

    it('should batch index documents', async () => {
      const documents = [
        { id: 'doc-1', text: 'JavaScript fundamentals' },
        { id: 'doc-2', text: 'TypeScript advanced types' },
        { id: 'doc-3', text: 'Node.js performance tuning' },
        { id: 'doc-4', text: 'Deno runtime overview' }
      ];

      await ruvector.batchIndex(documents);

      expect(ruvector.vectorIndex.size()).toBe(4);
    });

    it('should handle empty documents', async () => {
      await ruvector.index('empty-doc', '');

      expect(ruvector.vectorIndex.size()).toBe(1);
    });

    it('should handle very long documents', async () => {
      const longText = 'word '.repeat(10000);

      await ruvector.index('long-doc', longText);

      expect(ruvector.vectorIndex.size()).toBe(1);
    });
  });

  describe('Semantic Search', () => {
    beforeEach(async () => {
      await ruvector.batchIndex([
        { id: 'react-hooks', text: 'React hooks provide a way to use state and lifecycle in functional components' },
        { id: 'vue-composition', text: 'Vue composition API offers reactive state management' },
        { id: 'angular-rxjs', text: 'Angular uses RxJS for reactive programming patterns' },
        { id: 'svelte-stores', text: 'Svelte stores provide simple state management' },
        { id: 'solid-signals', text: 'SolidJS signals offer fine-grained reactivity' }
      ]);
    });

    it('should find semantically similar documents', async () => {
      const results = await ruvector.search('React state management', 3);

      expect(results).toHaveLength(3);
      expect(results[0].score).toBeGreaterThan(0);
    });

    it('should rank results by similarity', async () => {
      const results = await ruvector.search('React hooks', 5);

      // Results should be sorted by score descending
      for (let i = 1; i < results.length; i++) {
        expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score);
      }
    });

    it('should respect topK limit', async () => {
      const results = await ruvector.search('state management', 2);

      expect(results).toHaveLength(2);
    });

    it('should handle queries with no good matches', async () => {
      const results = await ruvector.search('quantum computing algorithms', 3);

      // Should still return results, just with lower scores
      expect(results.length).toBeGreaterThan(0);
      // Scores should be lower for unrelated queries
      expect(results[0].score).toBeLessThan(0.9);
    });
  });

  describe('Embedding Operations', () => {
    it('should generate consistent embeddings', () => {
      const text = 'Consistent embedding test';

      const embedding1 = ruvector.embedder.embed(text);
      const embedding2 = ruvector.embedder.embed(text);

      expect(embedding1.length).toBe(embedding2.length);
      for (let i = 0; i < embedding1.length; i++) {
        expect(embedding1[i]).toBe(embedding2[i]);
      }
    });

    it('should generate different embeddings for different texts', () => {
      const embedding1 = ruvector.embedder.embed('First text');
      const embedding2 = ruvector.embedder.embed('Second completely different text');

      let identical = true;
      for (let i = 0; i < embedding1.length; i++) {
        if (embedding1[i] !== embedding2[i]) {
          identical = false;
          break;
        }
      }

      expect(identical).toBe(false);
    });

    it('should return correct dimension', () => {
      expect(ruvector.embedder.dimension()).toBe(384);
    });

    it('should handle batch embedding', () => {
      const texts = ['Text 1', 'Text 2', 'Text 3'];
      const embeddings = ruvector.embedder.embedBatch(texts);

      expect(embeddings).toHaveLength(3);
      embeddings.forEach(e => {
        expect(e.length).toBe(384);
      });
    });
  });

  describe('Vector Index Operations', () => {
    it('should add and retrieve vectors', () => {
      const embedding = ruvector.embedder.embed('Test document');
      ruvector.vectorIndex.add('test-id', embedding);

      expect(ruvector.vectorIndex.size()).toBe(1);
    });

    it('should delete vectors', () => {
      const embedding = ruvector.embedder.embed('To delete');
      ruvector.vectorIndex.add('delete-id', embedding);

      const deleted = ruvector.vectorIndex.delete('delete-id');

      expect(deleted).toBe(true);
      expect(ruvector.vectorIndex.size()).toBe(0);
    });

    it('should clear all vectors', async () => {
      await ruvector.batchIndex([
        { id: 'doc-1', text: 'Text 1' },
        { id: 'doc-2', text: 'Text 2' }
      ]);

      ruvector.vectorIndex.clear();

      expect(ruvector.vectorIndex.size()).toBe(0);
    });

    it('should handle search on empty index', () => {
      const embedding = ruvector.embedder.embed('Query');
      const results = ruvector.vectorIndex.search(embedding, 10);

      expect(results).toHaveLength(0);
    });
  });

  describe('Routing', () => {
    beforeEach(() => {
      ruvector.router.addRoute('generate.*code', 'coder');
      ruvector.router.addRoute('write.*test', 'tester');
      ruvector.router.addRoute('review.*pull', 'reviewer');
    });

    it('should route to correct handler', () => {
      const result = ruvector.router.route('generate some code for me');

      expect(result.handler).toBe('coder');
      expect(result.confidence).toBeGreaterThan(0.5);
    });

    it('should fallback for unmatched queries', () => {
      const result = ruvector.router.route('random unrelated request');

      expect(result.handler).toBe('default');
      expect(result.metadata.fallback).toBe(true);
    });
  });
});

describe('RuVector Performance', () => {
  let ruvector: ReturnType<typeof createMockRuVectorBindings>;

  beforeEach(() => {
    ruvector = createMockRuVectorBindings();
  });

  describe('Large Scale Operations', () => {
    it('should handle 1000 documents', async () => {
      const documents = Array.from({ length: 1000 }, (_, i) => ({
        id: `doc-${i}`,
        text: `Document ${i} containing text about topic ${i % 10}`
      }));

      const startIndex = performance.now();
      await ruvector.batchIndex(documents);
      const indexTime = performance.now() - startIndex;

      expect(ruvector.vectorIndex.size()).toBe(1000);
      expect(indexTime).toBeLessThan(5000); // Should complete in <5 seconds
    });

    it('should search efficiently in large index', async () => {
      // Pre-populate index
      const documents = Array.from({ length: 500 }, (_, i) => ({
        id: `doc-${i}`,
        text: `Content about subject ${i} with details`
      }));
      await ruvector.batchIndex(documents);

      const startSearch = performance.now();
      const results = await ruvector.search('subject 250', 10);
      const searchTime = performance.now() - startSearch;

      expect(results).toHaveLength(10);
      expect(searchTime).toBeLessThan(100); // Should complete in <100ms
    });
  });

  describe('Memory Efficiency', () => {
    it('should report memory usage', () => {
      const memory = mockWasmLoader.getWasmMemory();

      expect(memory.used).toBeDefined();
      expect(memory.total).toBeDefined();
      expect(memory.used).toBeLessThan(memory.total);
    });
  });
});

describe('RuVector Error Handling', () => {
  let ruvector: ReturnType<typeof createMockRuVectorBindings>;

  beforeEach(() => {
    ruvector = createMockRuVectorBindings();
  });

  describe('Dimension Validation', () => {
    it('should reject mismatched embedding dimensions', () => {
      const wrongDimension = new Float32Array(256).fill(0.5);

      expect(() => {
        ruvector.vectorIndex.add('wrong', wrongDimension);
      }).toThrow('dimension mismatch');
    });

    it('should reject mismatched query dimensions', async () => {
      await ruvector.index('doc-1', 'Test document');

      const wrongQuery = new Float32Array(256).fill(0.5);

      expect(() => {
        ruvector.vectorIndex.search(wrongQuery, 10);
      }).toThrow('dimension mismatch');
    });
  });
});

describe('RuVector WASM Loader', () => {
  it('should check WASM support', () => {
    const supported = mockWasmLoader.isWasmSupported();
    expect(typeof supported).toBe('boolean');
  });

  it('should load vector index', async () => {
    const index = await mockWasmLoader.loadVectorIndex(768);

    expect(index).toBeInstanceOf(MockWasmVectorIndex);
  });

  it('should load embedder', async () => {
    const embedder = await mockWasmLoader.loadEmbedder(768);

    expect(embedder).toBeInstanceOf(MockWasmEmbedder);
  });
});
