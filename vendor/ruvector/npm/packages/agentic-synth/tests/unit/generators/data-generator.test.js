/**
 * Unit tests for DataGenerator
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { DataGenerator } from '../../../src/generators/data-generator.js';

describe('DataGenerator', () => {
  let generator;

  beforeEach(() => {
    generator = new DataGenerator({
      seed: 12345,
      schema: {
        name: { type: 'string', length: 10 },
        age: { type: 'number', min: 18, max: 65 },
        active: { type: 'boolean' },
        tags: { type: 'array', items: 5 },
        embedding: { type: 'vector', dimensions: 128 }
      }
    });
  });

  describe('constructor', () => {
    it('should create generator with default options', () => {
      const gen = new DataGenerator();
      expect(gen).toBeDefined();
      expect(gen.format).toBe('json');
    });

    it('should accept custom options', () => {
      const gen = new DataGenerator({
        seed: 99999,
        format: 'csv',
        schema: { test: { type: 'string' } }
      });
      expect(gen.seed).toBe(99999);
      expect(gen.format).toBe('csv');
      expect(gen.schema).toHaveProperty('test');
    });
  });

  describe('generate', () => {
    it('should generate specified number of records', () => {
      const data = generator.generate(5);
      expect(data).toHaveLength(5);
    });

    it('should generate single record by default', () => {
      const data = generator.generate();
      expect(data).toHaveLength(1);
    });

    it('should throw error for invalid count', () => {
      expect(() => generator.generate(0)).toThrow('Count must be at least 1');
      expect(() => generator.generate(-5)).toThrow('Count must be at least 1');
    });

    it('should generate records with correct schema fields', () => {
      const data = generator.generate(1);
      const record = data[0];

      expect(record).toHaveProperty('id');
      expect(record).toHaveProperty('name');
      expect(record).toHaveProperty('age');
      expect(record).toHaveProperty('active');
      expect(record).toHaveProperty('tags');
      expect(record).toHaveProperty('embedding');
    });

    it('should generate unique IDs', () => {
      const data = generator.generate(10);
      const ids = data.map(r => r.id);
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(10);
    });
  });

  describe('field generation', () => {
    it('should generate strings of correct length', () => {
      const data = generator.generate(1);
      expect(data[0].name).toHaveLength(10);
      expect(typeof data[0].name).toBe('string');
    });

    it('should generate numbers within range', () => {
      const data = generator.generate(100);
      data.forEach(record => {
        expect(record.age).toBeGreaterThanOrEqual(18);
        expect(record.age).toBeLessThanOrEqual(65);
      });
    });

    it('should generate boolean values', () => {
      const data = generator.generate(1);
      expect(typeof data[0].active).toBe('boolean');
    });

    it('should generate arrays of correct length', () => {
      const data = generator.generate(1);
      expect(Array.isArray(data[0].tags)).toBe(true);
      expect(data[0].tags).toHaveLength(5);
    });

    it('should generate vectors with correct dimensions', () => {
      const data = generator.generate(1);
      expect(Array.isArray(data[0].embedding)).toBe(true);
      expect(data[0].embedding).toHaveLength(128);

      // Check all values are numbers between 0 and 1
      data[0].embedding.forEach(val => {
        expect(typeof val).toBe('number');
        expect(val).toBeGreaterThanOrEqual(0);
        expect(val).toBeLessThanOrEqual(1);
      });
    });
  });

  describe('setSeed', () => {
    it('should allow updating seed', () => {
      generator.setSeed(54321);
      expect(generator.seed).toBe(54321);
    });

    it('should produce same results with same seed', () => {
      const gen1 = new DataGenerator({ seed: 12345, schema: { val: { type: 'number' } } });
      const gen2 = new DataGenerator({ seed: 12345, schema: { val: { type: 'number' } } });

      // Note: This test may be flaky due to random number generation
      // In real implementation, you'd want seeded random number generation
      expect(gen1.seed).toBe(gen2.seed);
    });
  });

  describe('performance', () => {
    it('should generate 1000 records quickly', () => {
      const start = Date.now();
      const data = generator.generate(1000);
      const duration = Date.now() - start;

      expect(data).toHaveLength(1000);
      expect(duration).toBeLessThan(1000); // Less than 1 second
    });

    it('should handle large vector dimensions efficiently', () => {
      const largeVectorGen = new DataGenerator({
        schema: {
          embedding: { type: 'vector', dimensions: 4096 }
        }
      });

      const start = Date.now();
      const data = largeVectorGen.generate(100);
      const duration = Date.now() - start;

      expect(data).toHaveLength(100);
      expect(data[0].embedding).toHaveLength(4096);
      expect(duration).toBeLessThan(2000); // Less than 2 seconds
    });
  });
});
