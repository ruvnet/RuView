/**
 * Integration tests for Ruvector adapter
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { RuvectorAdapter } from '../../src/adapters/ruvector.js';
import { DataGenerator } from '../../src/generators/data-generator.js';

describe('Ruvector Integration', () => {
  let adapter;
  let generator;

  beforeEach(async () => {
    adapter = new RuvectorAdapter({
      dimensions: 128
    });

    generator = new DataGenerator({
      schema: {
        text: { type: 'string', length: 50 },
        embedding: { type: 'vector', dimensions: 128 }
      }
    });

    await adapter.initialize();
  });

  afterEach(() => {
    // Cleanup
  });

  describe('initialization', () => {
    it('should initialize with custom dimensions', async () => {
      const customAdapter = new RuvectorAdapter({ dimensions: 256 });
      await customAdapter.initialize();

      expect(customAdapter.dimensions).toBe(256);
      expect(customAdapter.initialized).toBe(true);
    });

    it('should use default dimensions', async () => {
      const defaultAdapter = new RuvectorAdapter();
      await defaultAdapter.initialize();

      expect(defaultAdapter.dimensions).toBe(128);
    });
  });

  describe('vector insertion', () => {
    it('should insert single vector', async () => {
      const vectors = [{
        id: 'vec1',
        vector: new Array(128).fill(0).map(() => Math.random())
      }];

      const results = await adapter.insert(vectors);

      expect(results).toHaveLength(1);
      expect(results[0].id).toBe('vec1');
      expect(results[0].status).toBe('inserted');
    });

    it('should insert multiple vectors', async () => {
      const vectors = Array.from({ length: 10 }, (_, i) => ({
        id: `vec${i}`,
        vector: new Array(128).fill(0).map(() => Math.random())
      }));

      const results = await adapter.insert(vectors);

      expect(results).toHaveLength(10);
    });

    it('should throw error when not initialized', async () => {
      const uninitializedAdapter = new RuvectorAdapter();

      await expect(uninitializedAdapter.insert([]))
        .rejects.toThrow('RuVector adapter not initialized');
    });

    it('should validate vector format', async () => {
      await expect(adapter.insert('not an array')).rejects.toThrow('Vectors must be an array');
    });

    it('should validate vector structure', async () => {
      const invalidVectors = [{ id: 'test' }]; // Missing vector field

      await expect(adapter.insert(invalidVectors))
        .rejects.toThrow('Each vector must have id and vector fields');
    });

    it('should validate vector dimensions', async () => {
      const wrongDimensions = [{
        id: 'test',
        vector: new Array(64).fill(0) // Wrong dimension
      }];

      await expect(adapter.insert(wrongDimensions))
        .rejects.toThrow('Vector dimension mismatch');
    });
  });

  describe('vector search', () => {
    beforeEach(async () => {
      // Insert some test vectors
      const vectors = Array.from({ length: 20 }, (_, i) => ({
        id: `vec${i}`,
        vector: new Array(128).fill(0).map(() => Math.random())
      }));
      await adapter.insert(vectors);
    });

    it('should search for similar vectors', async () => {
      const query = new Array(128).fill(0).map(() => Math.random());
      const results = await adapter.search(query, 5);

      expect(results).toHaveLength(5);
      results.forEach(result => {
        expect(result).toHaveProperty('id');
        expect(result).toHaveProperty('score');
      });
    });

    it('should return results sorted by score', async () => {
      const query = new Array(128).fill(0).map(() => Math.random());
      const results = await adapter.search(query, 10);

      // Check descending order
      for (let i = 1; i < results.length; i++) {
        expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score);
      }
    });

    it('should respect k parameter', async () => {
      const query = new Array(128).fill(0).map(() => Math.random());

      const results3 = await adapter.search(query, 3);
      expect(results3).toHaveLength(3);

      const results10 = await adapter.search(query, 10);
      expect(results10).toHaveLength(10);
    });

    it('should validate query format', async () => {
      await expect(adapter.search('not an array', 5))
        .rejects.toThrow('Query must be an array');
    });

    it('should validate query dimensions', async () => {
      const wrongQuery = new Array(64).fill(0);

      await expect(adapter.search(wrongQuery, 5))
        .rejects.toThrow('Query dimension mismatch');
    });

    it('should throw error when not initialized', async () => {
      const uninitializedAdapter = new RuvectorAdapter();
      const query = new Array(128).fill(0);

      await expect(uninitializedAdapter.search(query, 5))
        .rejects.toThrow('RuVector adapter not initialized');
    });
  });

  describe('vector retrieval', () => {
    beforeEach(async () => {
      const testVector = {
        id: 'test-vec',
        vector: new Array(128).fill(0.5)
      };
      await adapter.insert([testVector]);
    });

    it('should get vector by ID', async () => {
      const result = await adapter.get('test-vec');

      expect(result).toBeDefined();
      expect(result.id).toBe('test-vec');
      expect(result.vector).toHaveLength(128);
    });

    it('should return null for non-existent ID', async () => {
      const result = await adapter.get('nonexistent');
      expect(result).toBeNull();
    });

    it('should throw error when not initialized', async () => {
      const uninitializedAdapter = new RuvectorAdapter();

      await expect(uninitializedAdapter.get('test'))
        .rejects.toThrow('RuVector adapter not initialized');
    });
  });

  describe('end-to-end workflow', () => {
    it('should generate embeddings and perform similarity search', async () => {
      // Generate synthetic data with embeddings
      const data = generator.generate(50);

      // Insert into Ruvector
      const vectors = data.map(item => ({
        id: `doc${item.id}`,
        vector: item.embedding
      }));
      await adapter.insert(vectors);

      // Search for similar vectors
      const queryVector = data[0].embedding;
      const results = await adapter.search(queryVector, 10);

      expect(results).toHaveLength(10);

      // First result should be the query itself (highest similarity)
      expect(results[0].id).toBe('doc0');
      expect(results[0].score).toBeGreaterThan(0.9);
    });

    it('should handle large-scale insertion and search', async () => {
      // Generate large dataset
      const largeData = generator.generate(1000);
      const vectors = largeData.map(item => ({
        id: `doc${item.id}`,
        vector: item.embedding
      }));

      // Insert in batches
      const batchSize = 100;
      for (let i = 0; i < vectors.length; i += batchSize) {
        const batch = vectors.slice(i, i + batchSize);
        await adapter.insert(batch);
      }

      // Perform searches
      const query = largeData[0].embedding;
      const results = await adapter.search(query, 20);

      expect(results).toHaveLength(20);
    });
  });

  describe('performance', () => {
    it('should insert 1000 vectors quickly', async () => {
      const vectors = Array.from({ length: 1000 }, (_, i) => ({
        id: `vec${i}`,
        vector: new Array(128).fill(0).map(() => Math.random())
      }));

      const start = Date.now();
      await adapter.insert(vectors);
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(1000); // Less than 1 second
    });

    it('should perform search quickly', async () => {
      // Insert test data
      const vectors = Array.from({ length: 1000 }, (_, i) => ({
        id: `vec${i}`,
        vector: new Array(128).fill(0).map(() => Math.random())
      }));
      await adapter.insert(vectors);

      // Measure search time
      const query = new Array(128).fill(0).map(() => Math.random());

      const start = Date.now();
      await adapter.search(query, 10);
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(100); // Less than 100ms
    });

    it('should handle concurrent searches', async () => {
      // Insert test data
      const vectors = Array.from({ length: 100 }, (_, i) => ({
        id: `vec${i}`,
        vector: new Array(128).fill(0).map(() => Math.random())
      }));
      await adapter.insert(vectors);

      // Perform concurrent searches
      const queries = Array.from({ length: 50 }, () =>
        new Array(128).fill(0).map(() => Math.random())
      );

      const start = Date.now();
      await Promise.all(queries.map(q => adapter.search(q, 5)));
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(500);
    });
  });

  describe('accuracy', () => {
    it('should find exact match with highest score', async () => {
      const exactVector = new Array(128).fill(0.5);
      await adapter.insert([{ id: 'exact', vector: exactVector }]);

      const results = await adapter.search(exactVector, 1);

      expect(results[0].id).toBe('exact');
      expect(results[0].score).toBeCloseTo(1.0, 5);
    });

    it('should rank similar vectors correctly', async () => {
      const baseVector = new Array(128).fill(0.5);

      // Create slightly different vectors
      const similar = baseVector.map(v => v + 0.01);
      const different = new Array(128).fill(0).map(() => Math.random());

      await adapter.insert([
        { id: 'base', vector: baseVector },
        { id: 'similar', vector: similar },
        { id: 'different', vector: different }
      ]);

      const results = await adapter.search(baseVector, 3);

      // Base should be first, similar second
      expect(results[0].id).toBe('base');
      expect(results[1].id).toBe('similar');
    });
  });
});
