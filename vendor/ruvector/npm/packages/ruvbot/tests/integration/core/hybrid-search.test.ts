/**
 * HybridSearch Integration Tests
 *
 * Tests the hybrid search implementation combining vector similarity
 * and BM25 keyword search with Reciprocal Rank Fusion.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  HybridSearch,
  createHybridSearch,
  DEFAULT_HYBRID_CONFIG,
  type HybridSearchConfig,
  type HybridSearchResult,
} from '../../../src/learning/search/HybridSearch.js';
import type { Embedder, VectorIndex } from '../../../src/learning/memory/MemoryManager.js';

// Mock vector index for testing
class MockVectorIndex implements VectorIndex {
  private vectors: Map<string, Float32Array> = new Map();

  async add(id: string, embedding: Float32Array): Promise<void> {
    this.vectors.set(id, embedding);
  }

  async remove(id: string): Promise<boolean> {
    return this.vectors.delete(id);
  }

  delete(id: string): boolean {
    return this.vectors.delete(id);
  }

  async search(query: Float32Array, topK: number): Promise<Array<{ id: string; score: number; distance: number }>> {
    const results: Array<{ id: string; score: number; distance: number }> = [];

    for (const [id, vec] of this.vectors.entries()) {
      const score = this.cosineSimilarity(query, vec);
      results.push({ id, score, distance: 1 - score });
    }

    return results
      .sort((a, b) => b.score - a.score)
      .slice(0, topK);
  }

  size(): number {
    return this.vectors.size;
  }

  clear(): void {
    this.vectors.clear();
  }

  private cosineSimilarity(a: Float32Array, b: Float32Array): number {
    let dotProduct = 0;
    let normA = 0;
    let normB = 0;

    for (let i = 0; i < a.length; i++) {
      dotProduct += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }

    const magnitude = Math.sqrt(normA) * Math.sqrt(normB);
    return magnitude === 0 ? 0 : dotProduct / magnitude;
  }
}

// Mock embedder for testing
class MockEmbedder implements Embedder {
  private _dimension = 128;

  async embed(text: string): Promise<Float32Array> {
    // Simple deterministic embedding based on text hash
    const embedding = new Float32Array(this._dimension);
    const hash = this.simpleHash(text);

    for (let i = 0; i < this._dimension; i++) {
      embedding[i] = Math.sin(hash * (i + 1)) * Math.cos(hash / (i + 1));
    }

    // Normalize
    let norm = 0;
    for (let i = 0; i < this._dimension; i++) {
      norm += embedding[i] * embedding[i];
    }
    norm = Math.sqrt(norm);
    for (let i = 0; i < this._dimension; i++) {
      embedding[i] /= norm;
    }

    return embedding;
  }

  async embedBatch(texts: string[]): Promise<Float32Array[]> {
    return Promise.all(texts.map(t => this.embed(t)));
  }

  dimension(): number {
    return this._dimension;
  }

  private simpleHash(str: string): number {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash;
    }
    return hash;
  }
}

describe('HybridSearch Integration Tests', () => {
  let hybridSearch: HybridSearch;
  let vectorIndex: MockVectorIndex;
  let embedder: MockEmbedder;

  beforeEach(() => {
    hybridSearch = createHybridSearch();
    vectorIndex = new MockVectorIndex();
    embedder = new MockEmbedder();
    hybridSearch.initialize(vectorIndex, embedder);
  });

  describe('Initialization', () => {
    it('should initialize with default configuration', () => {
      const search = createHybridSearch();
      expect(search.isInitialized()).toBe(false);

      const stats = search.getStats();
      expect(stats.config.vector.enabled).toBe(DEFAULT_HYBRID_CONFIG.vector.enabled);
      expect(stats.config.keyword.enabled).toBe(DEFAULT_HYBRID_CONFIG.keyword.enabled);
      expect(stats.config.fusion.method).toBe(DEFAULT_HYBRID_CONFIG.fusion.method);
    });

    it('should accept custom configuration', () => {
      const customConfig: Partial<HybridSearchConfig> = {
        vector: { enabled: true, weight: 0.8 },
        keyword: { enabled: true, weight: 0.2, k1: 1.5, b: 0.8 },
        fusion: { method: 'linear', k: 30, candidateMultiplier: 2 },
      };

      const search = createHybridSearch(customConfig);
      const stats = search.getStats();

      expect(stats.config.vector.weight).toBe(0.8);
      expect(stats.config.keyword.weight).toBe(0.2);
      expect(stats.config.fusion.method).toBe('linear');
    });

    it('should track initialization status', () => {
      const search = createHybridSearch();
      expect(search.isInitialized()).toBe(false);

      search.initialize(vectorIndex, embedder);
      expect(search.isInitialized()).toBe(true);
    });
  });

  describe('Document Indexing', () => {
    it('should add documents to both indices', async () => {
      await hybridSearch.add('doc1', 'Machine learning algorithms process data');
      await hybridSearch.add('doc2', 'Deep neural networks learn patterns');

      const stats = hybridSearch.getStats();
      expect(stats.bm25Stats.documentCount).toBe(2);
      expect(stats.vectorIndexSize).toBe(2);
    });

    it('should add documents with pre-computed embeddings', async () => {
      const embedding = await embedder.embed('test content');
      await hybridSearch.add('doc1', 'Test content for indexing', embedding);

      const stats = hybridSearch.getStats();
      expect(stats.bm25Stats.documentCount).toBe(1);
      expect(stats.vectorIndexSize).toBe(1);
    });

    it('should delete documents from both indices', async () => {
      await hybridSearch.add('doc1', 'First document');
      await hybridSearch.add('doc2', 'Second document');

      expect(hybridSearch.getStats().bm25Stats.documentCount).toBe(2);

      const deleted = hybridSearch.delete('doc1');
      expect(deleted).toBe(true);

      const stats = hybridSearch.getStats();
      expect(stats.bm25Stats.documentCount).toBe(1);
      expect(stats.vectorIndexSize).toBe(1);
    });

    it('should clear both indices', async () => {
      await hybridSearch.add('doc1', 'First document');
      await hybridSearch.add('doc2', 'Second document');

      hybridSearch.clear();

      const stats = hybridSearch.getStats();
      expect(stats.bm25Stats.documentCount).toBe(0);
      expect(stats.vectorIndexSize).toBe(0);
    });
  });

  describe('Hybrid Search', () => {
    beforeEach(async () => {
      // Add test corpus
      await hybridSearch.add('ml-doc', 'Machine learning is used for predictive analytics and pattern recognition');
      await hybridSearch.add('dl-doc', 'Deep learning neural networks excel at image and speech recognition');
      await hybridSearch.add('nlp-doc', 'Natural language processing enables text analysis and sentiment detection');
      await hybridSearch.add('cv-doc', 'Computer vision algorithms process visual data from cameras and sensors');
      await hybridSearch.add('ds-doc', 'Data science combines statistics, programming, and domain expertise');
    });

    it('should return fused results from both indices', async () => {
      const results = await hybridSearch.search('machine learning analytics');

      expect(results.length).toBeGreaterThan(0);

      // Results should have scores from both methods
      const firstResult = results[0];
      expect(firstResult).toHaveProperty('id');
      expect(firstResult).toHaveProperty('vectorScore');
      expect(firstResult).toHaveProperty('keywordScore');
      expect(firstResult).toHaveProperty('fusedScore');
    });

    it('should rank results by fused score', async () => {
      const results = await hybridSearch.search('neural networks deep learning', 10);

      // Results should be sorted by fusedScore descending
      for (let i = 1; i < results.length; i++) {
        expect(results[i - 1].fusedScore).toBeGreaterThanOrEqual(results[i].fusedScore);
      }
    });

    it('should respect topK parameter', async () => {
      const results = await hybridSearch.search('learning', { topK: 2 });
      expect(results.length).toBeLessThanOrEqual(2);
    });

    it('should filter by threshold', async () => {
      const results = await hybridSearch.search('learning', { threshold: 0.5 });

      // All results should meet threshold
      for (const result of results) {
        expect(result.fusedScore).toBeGreaterThanOrEqual(0.5);
      }
    });

    it('should support vector-only search', async () => {
      const results = await hybridSearch.search('learning', { vectorOnly: true });

      expect(results.length).toBeGreaterThan(0);
      // In vector-only mode, keyword scores should be 0
      for (const result of results) {
        expect(result.keywordScore).toBe(0);
      }
    });

    it('should support keyword-only search', async () => {
      const results = await hybridSearch.search('learning', { keywordOnly: true });

      expect(results.length).toBeGreaterThan(0);
      // In keyword-only mode, vector scores should be 0
      for (const result of results) {
        expect(result.vectorScore).toBe(0);
      }
    });

    it('should include matched terms from keyword search', async () => {
      const results = await hybridSearch.search('machine learning');

      const mlResult = results.find(r => r.id === 'ml-doc');
      expect(mlResult).toBeDefined();
      expect(mlResult?.matchedTerms).toBeDefined();
    });
  });

  describe('Fusion Methods', () => {
    const setupSearch = async (method: 'rrf' | 'linear' | 'weighted') => {
      const search = createHybridSearch({
        fusion: { method, k: 60, candidateMultiplier: 3 },
      });
      search.initialize(vectorIndex, embedder);

      await search.add('doc1', 'Machine learning algorithms');
      await search.add('doc2', 'Deep learning neural networks');
      await search.add('doc3', 'Natural language processing');

      return search;
    };

    it('should use RRF fusion correctly', async () => {
      const search = await setupSearch('rrf');
      const results = await search.search('machine learning');

      expect(results.length).toBeGreaterThan(0);
      // RRF produces positive scores
      expect(results[0].fusedScore).toBeGreaterThan(0);
    });

    it('should use linear fusion correctly', async () => {
      const search = await setupSearch('linear');
      const results = await search.search('machine learning');

      expect(results.length).toBeGreaterThan(0);
      // Linear fusion produces weighted sum
      expect(results[0].fusedScore).toBeGreaterThanOrEqual(0);
    });

    it('should use weighted fusion correctly', async () => {
      const search = await setupSearch('weighted');
      const results = await search.search('machine learning');

      expect(results.length).toBeGreaterThan(0);
      // Weighted fusion with presence bonus
      expect(results[0].fusedScore).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Weight Configuration', () => {
    it('should apply vector weight', async () => {
      const vectorHeavy = createHybridSearch({
        vector: { enabled: true, weight: 0.9 },
        keyword: { enabled: true, weight: 0.1 },
      });
      vectorHeavy.initialize(vectorIndex, embedder);

      await vectorHeavy.add('doc1', 'test content');
      const results = await vectorHeavy.search('test');

      expect(results.length).toBeGreaterThan(0);
    });

    it('should apply keyword weight', async () => {
      const keywordHeavy = createHybridSearch({
        vector: { enabled: true, weight: 0.1 },
        keyword: { enabled: true, weight: 0.9 },
      });
      keywordHeavy.initialize(vectorIndex, embedder);

      await keywordHeavy.add('doc1', 'test content');
      const results = await keywordHeavy.search('test');

      expect(results.length).toBeGreaterThan(0);
    });
  });

  describe('Disabled Modes', () => {
    it('should work with vector disabled', async () => {
      const keywordOnly = createHybridSearch({
        vector: { enabled: false, weight: 0 },
        keyword: { enabled: true, weight: 1.0 },
      });
      keywordOnly.initialize(vectorIndex, embedder);

      await keywordOnly.add('doc1', 'Machine learning content');
      const results = await keywordOnly.search('machine learning');

      expect(results.length).toBe(1);
      expect(results[0].vectorScore).toBe(0);
    });

    it('should work with keyword disabled', async () => {
      const vectorOnly = createHybridSearch({
        vector: { enabled: true, weight: 1.0 },
        keyword: { enabled: false, weight: 0 },
      });
      vectorOnly.initialize(vectorIndex, embedder);

      await vectorOnly.add('doc1', 'Machine learning content');
      const results = await vectorOnly.search('machine learning');

      expect(results.length).toBe(1);
      expect(results[0].keywordScore).toBe(0);
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty queries', async () => {
      await hybridSearch.add('doc1', 'Some content');
      const results = await hybridSearch.search('');

      expect(results.length).toBe(0);
    });

    it('should handle queries with no matches', async () => {
      await hybridSearch.add('doc1', 'Machine learning content');
      const results = await hybridSearch.search('cryptocurrency blockchain', { keywordOnly: true });

      expect(results.length).toBe(0);
    });

    it('should handle search without initialization', async () => {
      const uninitSearch = createHybridSearch();
      // Add to BM25 only since not initialized
      await uninitSearch.add('doc1', 'Test content');

      // Should still work for keyword search
      const results = await uninitSearch.search('test', { keywordOnly: true });
      expect(results.length).toBe(1);
    });

    it('should handle concurrent additions', async () => {
      const promises = [];
      for (let i = 0; i < 10; i++) {
        promises.push(hybridSearch.add(`doc-${i}`, `Content number ${i} with words`));
      }

      await Promise.all(promises);

      const stats = hybridSearch.getStats();
      expect(stats.bm25Stats.documentCount).toBe(10);
    });
  });

  describe('Statistics', () => {
    it('should return accurate statistics', async () => {
      await hybridSearch.add('doc1', 'First document');
      await hybridSearch.add('doc2', 'Second document');

      const stats = hybridSearch.getStats();

      expect(stats.config).toBeDefined();
      expect(stats.bm25Stats).toBeDefined();
      expect(stats.vectorIndexSize).toBe(2);
      expect(stats.bm25Stats.documentCount).toBe(2);
    });
  });
});
