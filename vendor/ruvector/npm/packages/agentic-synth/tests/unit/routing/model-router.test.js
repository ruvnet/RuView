/**
 * Unit tests for ModelRouter
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { ModelRouter } from '../../../src/routing/model-router.js';

describe('ModelRouter', () => {
  let router;
  let models;

  beforeEach(() => {
    models = [
      { id: 'model-1', endpoint: 'http://api1.com', capabilities: ['general', 'code'] },
      { id: 'model-2', endpoint: 'http://api2.com', capabilities: ['general'] },
      { id: 'model-3', endpoint: 'http://api3.com', capabilities: ['math', 'reasoning'] }
    ];

    router = new ModelRouter({
      models,
      strategy: 'round-robin'
    });
  });

  describe('constructor', () => {
    it('should create router with default options', () => {
      const defaultRouter = new ModelRouter();
      expect(defaultRouter.models).toEqual([]);
      expect(defaultRouter.strategy).toBe('round-robin');
    });

    it('should accept custom options', () => {
      expect(router.models).toEqual(models);
      expect(router.strategy).toBe('round-robin');
    });

    it('should initialize model stats', () => {
      models.forEach(model => {
        const stats = router.getStats(model.id);
        expect(stats).toBeDefined();
        expect(stats.requests).toBe(0);
        expect(stats.errors).toBe(0);
      });
    });
  });

  describe('registerModel', () => {
    it('should register new model', () => {
      const newModel = { id: 'model-4', endpoint: 'http://api4.com' };
      router.registerModel(newModel);

      expect(router.models).toContain(newModel);
      expect(router.getStats('model-4')).toBeDefined();
    });

    it('should throw error for invalid model', () => {
      expect(() => router.registerModel({})).toThrow('Model must have id and endpoint');
      expect(() => router.registerModel({ id: 'test' })).toThrow('Model must have id and endpoint');
    });

    it('should initialize stats for new model', () => {
      const newModel = { id: 'model-4', endpoint: 'http://api4.com' };
      router.registerModel(newModel);

      const stats = router.getStats('model-4');
      expect(stats.requests).toBe(0);
      expect(stats.errors).toBe(0);
      expect(stats.avgLatency).toBe(0);
    });
  });

  describe('route - round-robin', () => {
    it('should distribute requests evenly', () => {
      const results = [];
      for (let i = 0; i < 6; i++) {
        results.push(router.route({}));
      }

      expect(results[0]).toBe('model-1');
      expect(results[1]).toBe('model-2');
      expect(results[2]).toBe('model-3');
      expect(results[3]).toBe('model-1');
      expect(results[4]).toBe('model-2');
      expect(results[5]).toBe('model-3');
    });

    it('should wrap around after reaching end', () => {
      for (let i = 0; i < 3; i++) {
        router.route({});
      }

      expect(router.route({})).toBe('model-1');
    });
  });

  describe('route - least-latency', () => {
    beforeEach(() => {
      router.strategy = 'least-latency';

      // Record some metrics
      router.recordMetrics('model-1', 100);
      router.recordMetrics('model-2', 50);
      router.recordMetrics('model-3', 150);
    });

    it('should route to model with lowest latency', () => {
      const modelId = router.route({});
      expect(modelId).toBe('model-2');
    });

    it('should update as latencies change', () => {
      router.recordMetrics('model-1', 20);
      router.recordMetrics('model-1', 20);

      const modelId = router.route({});
      expect(modelId).toBe('model-1');
    });
  });

  describe('route - cost-optimized', () => {
    beforeEach(() => {
      router.strategy = 'cost-optimized';
    });

    it('should route small requests to first model', () => {
      const smallRequest = { data: 'test' };
      const modelId = router.route(smallRequest);
      expect(modelId).toBe('model-1');
    });

    it('should route large requests to last model', () => {
      const largeRequest = { data: 'x'.repeat(2000) };
      const modelId = router.route(largeRequest);
      expect(modelId).toBe('model-3');
    });
  });

  describe('route - capability-based', () => {
    beforeEach(() => {
      router.strategy = 'capability-based';
    });

    it('should route to model with required capability', () => {
      const request = { capability: 'code' };
      const modelId = router.route(request);
      expect(modelId).toBe('model-1');
    });

    it('should route math requests to capable model', () => {
      const request = { capability: 'math' };
      const modelId = router.route(request);
      expect(modelId).toBe('model-3');
    });

    it('should fallback to first model if no match', () => {
      const request = { capability: 'unsupported' };
      const modelId = router.route(request);
      expect(modelId).toBe('model-1');
    });
  });

  describe('route - error handling', () => {
    it('should throw error when no models available', () => {
      const emptyRouter = new ModelRouter();
      expect(() => emptyRouter.route({})).toThrow('No models available for routing');
    });
  });

  describe('recordMetrics', () => {
    it('should record successful requests', () => {
      router.recordMetrics('model-1', 100, true);

      const stats = router.getStats('model-1');
      expect(stats.requests).toBe(1);
      expect(stats.errors).toBe(0);
      expect(stats.avgLatency).toBe(100);
    });

    it('should record failed requests', () => {
      router.recordMetrics('model-1', 100, false);

      const stats = router.getStats('model-1');
      expect(stats.requests).toBe(1);
      expect(stats.errors).toBe(1);
    });

    it('should calculate average latency', () => {
      router.recordMetrics('model-1', 100);
      router.recordMetrics('model-1', 200);
      router.recordMetrics('model-1', 300);

      const stats = router.getStats('model-1');
      expect(stats.avgLatency).toBe(200);
    });

    it('should handle non-existent model gracefully', () => {
      router.recordMetrics('nonexistent', 100);
      expect(router.getStats('nonexistent')).toBeUndefined();
    });
  });

  describe('getStats', () => {
    it('should return stats for specific model', () => {
      router.recordMetrics('model-1', 100);

      const stats = router.getStats('model-1');
      expect(stats).toHaveProperty('requests');
      expect(stats).toHaveProperty('errors');
      expect(stats).toHaveProperty('avgLatency');
    });

    it('should return all stats when no model specified', () => {
      const allStats = router.getStats();
      expect(allStats).toHaveProperty('model-1');
      expect(allStats).toHaveProperty('model-2');
      expect(allStats).toHaveProperty('model-3');
    });

    it('should track multiple models independently', () => {
      router.recordMetrics('model-1', 100);
      router.recordMetrics('model-2', 200);

      expect(router.getStats('model-1').avgLatency).toBe(100);
      expect(router.getStats('model-2').avgLatency).toBe(200);
    });
  });

  describe('performance', () => {
    it('should handle 1000 routing decisions quickly', () => {
      const start = Date.now();

      for (let i = 0; i < 1000; i++) {
        router.route({});
      }

      const duration = Date.now() - start;
      expect(duration).toBeLessThan(100); // Less than 100ms
    });

    it('should efficiently handle many models', () => {
      const manyModels = Array.from({ length: 100 }, (_, i) => ({
        id: `model-${i}`,
        endpoint: `http://api${i}.com`
      }));

      const largeRouter = new ModelRouter({ models: manyModels });

      const start = Date.now();
      for (let i = 0; i < 1000; i++) {
        largeRouter.route({});
      }
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(200);
    });
  });
});
