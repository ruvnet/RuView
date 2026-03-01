/**
 * Unit tests for ContextCache
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ContextCache } from '../../../src/cache/context-cache.js';

describe('ContextCache', () => {
  let cache;

  beforeEach(() => {
    cache = new ContextCache({
      maxSize: 5,
      ttl: 1000 // 1 second for testing
    });
  });

  describe('constructor', () => {
    it('should create cache with default options', () => {
      const defaultCache = new ContextCache();
      expect(defaultCache.maxSize).toBe(100);
      expect(defaultCache.ttl).toBe(3600000);
    });

    it('should accept custom options', () => {
      expect(cache.maxSize).toBe(5);
      expect(cache.ttl).toBe(1000);
    });

    it('should initialize empty cache', () => {
      expect(cache.cache.size).toBe(0);
    });

    it('should initialize stats', () => {
      const stats = cache.getStats();
      expect(stats.hits).toBe(0);
      expect(stats.misses).toBe(0);
      expect(stats.evictions).toBe(0);
    });
  });

  describe('set and get', () => {
    it('should store and retrieve value', () => {
      cache.set('key1', 'value1');
      const result = cache.get('key1');
      expect(result).toBe('value1');
    });

    it('should return null for non-existent key', () => {
      const result = cache.get('nonexistent');
      expect(result).toBeNull();
    });

    it('should update existing key', () => {
      cache.set('key1', 'value1');
      cache.set('key1', 'value2');
      expect(cache.get('key1')).toBe('value2');
      expect(cache.cache.size).toBe(1);
    });

    it('should store complex objects', () => {
      const obj = { nested: { data: [1, 2, 3] } };
      cache.set('complex', obj);
      expect(cache.get('complex')).toEqual(obj);
    });
  });

  describe('TTL (Time To Live)', () => {
    it('should return null for expired entries', async () => {
      cache.set('key1', 'value1');

      // Wait for TTL to expire
      await new Promise(resolve => setTimeout(resolve, 1100));

      const result = cache.get('key1');
      expect(result).toBeNull();
    });

    it('should not return expired entries in has()', async () => {
      cache.set('key1', 'value1');
      expect(cache.has('key1')).toBe(true);

      await new Promise(resolve => setTimeout(resolve, 1100));

      expect(cache.has('key1')).toBe(false);
    });

    it('should delete expired entries', async () => {
      cache.set('key1', 'value1');
      expect(cache.cache.size).toBe(1);

      await new Promise(resolve => setTimeout(resolve, 1100));
      cache.get('key1'); // Triggers cleanup

      expect(cache.cache.size).toBe(0);
    });
  });

  describe('eviction', () => {
    it('should evict LRU entry when at capacity', () => {
      // Fill cache to capacity
      for (let i = 0; i < 5; i++) {
        cache.set(`key${i}`, `value${i}`);
      }

      expect(cache.cache.size).toBe(5);

      // Access key1 to make key0 the LRU
      cache.get('key1');

      // Add new entry, should evict key0
      cache.set('key5', 'value5');

      expect(cache.cache.size).toBe(5);
      expect(cache.get('key0')).toBeNull();
      expect(cache.get('key5')).toBe('value5');
    });

    it('should track eviction stats', () => {
      for (let i = 0; i < 6; i++) {
        cache.set(`key${i}`, `value${i}`);
      }

      const stats = cache.getStats();
      expect(stats.evictions).toBeGreaterThan(0);
    });
  });

  describe('has', () => {
    it('should return true for existing key', () => {
      cache.set('key1', 'value1');
      expect(cache.has('key1')).toBe(true);
    });

    it('should return false for non-existent key', () => {
      expect(cache.has('nonexistent')).toBe(false);
    });
  });

  describe('clear', () => {
    it('should remove all entries', () => {
      cache.set('key1', 'value1');
      cache.set('key2', 'value2');

      cache.clear();

      expect(cache.cache.size).toBe(0);
      expect(cache.get('key1')).toBeNull();
    });

    it('should reset statistics', () => {
      cache.set('key1', 'value1');
      cache.get('key1');
      cache.get('nonexistent');

      cache.clear();

      const stats = cache.getStats();
      expect(stats.hits).toBe(0);
      expect(stats.misses).toBe(0);
    });
  });

  describe('getStats', () => {
    it('should track cache hits', () => {
      cache.set('key1', 'value1');
      cache.get('key1');
      cache.get('key1');

      const stats = cache.getStats();
      expect(stats.hits).toBe(2);
    });

    it('should track cache misses', () => {
      cache.get('nonexistent1');
      cache.get('nonexistent2');

      const stats = cache.getStats();
      expect(stats.misses).toBe(2);
    });

    it('should calculate hit rate', () => {
      cache.set('key1', 'value1');
      cache.get('key1'); // hit
      cache.get('key1'); // hit
      cache.get('nonexistent'); // miss

      const stats = cache.getStats();
      expect(stats.hitRate).toBeCloseTo(0.666, 2);
    });

    it('should include cache size', () => {
      cache.set('key1', 'value1');
      cache.set('key2', 'value2');

      const stats = cache.getStats();
      expect(stats.size).toBe(2);
    });

    it('should handle zero hit rate', () => {
      cache.get('nonexistent');

      const stats = cache.getStats();
      expect(stats.hitRate).toBe(0);
    });
  });

  describe('access tracking', () => {
    it('should update access count', () => {
      cache.set('key1', 'value1');
      cache.get('key1');
      cache.get('key1');

      const entry = cache.cache.get('key1');
      expect(entry.accessCount).toBe(2);
    });

    it('should update last access time', () => {
      cache.set('key1', 'value1');
      const initialAccess = cache.cache.get('key1').lastAccess;

      // Small delay
      setTimeout(() => {
        cache.get('key1');
        const laterAccess = cache.cache.get('key1').lastAccess;
        expect(laterAccess).toBeGreaterThan(initialAccess);
      }, 10);
    });
  });

  describe('performance', () => {
    it('should handle 1000 operations quickly', () => {
      const start = Date.now();

      for (let i = 0; i < 1000; i++) {
        cache.set(`key${i}`, `value${i}`);
        cache.get(`key${i}`);
      }

      const duration = Date.now() - start;
      expect(duration).toBeLessThan(100); // Less than 100ms
    });

    it('should maintain performance with large values', () => {
      const largeValue = { data: new Array(1000).fill('x'.repeat(100)) };

      const start = Date.now();
      for (let i = 0; i < 100; i++) {
        cache.set(`key${i}`, largeValue);
      }
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(100);
    });
  });
});
