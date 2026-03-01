/**
 * Tests for Multi-Model Benchmarking
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { MultiModelBenchmark } from '../../src/dspy/benchmark.js';
import { ModelProvider } from '../../src/types/index.js';
import type { BenchmarkConfig } from '../../src/dspy/benchmark.js';

describe('MultiModelBenchmark', () => {
  let config: BenchmarkConfig;

  beforeEach(() => {
    config = {
      models: [
        {
          provider: ModelProvider.GEMINI,
          model: 'gemini-2.0-flash-exp',
          apiKey: 'test-key-1'
        },
        {
          provider: ModelProvider.CLAUDE,
          model: 'claude-sonnet-4',
          apiKey: 'test-key-2'
        }
      ],
      tasks: ['code-generation', 'text-summarization'],
      iterations: 3
    };
  });

  describe('Initialization', () => {
    it('should create benchmark with valid config', () => {
      const benchmark = new MultiModelBenchmark(config);
      expect(benchmark).toBeDefined();
    });

    it('should accept timeout option', () => {
      const benchmarkWithTimeout = new MultiModelBenchmark({
        ...config,
        timeout: 5000
      });
      expect(benchmarkWithTimeout).toBeDefined();
    });
  });

  describe('Benchmark Execution', () => {
    it('should run complete benchmark and return results', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      expect(result.results).toBeDefined();
      expect(result.results.length).toBeGreaterThan(0);
      expect(result.bestModel).toBeDefined();
      expect(result.bestProvider).toBeDefined();
      expect(result.summary).toBeDefined();
    });

    it('should test all model and task combinations', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      // 2 models × 2 tasks × 3 iterations = 12 results
      expect(result.results.length).toBe(12);

      // Verify all tasks are covered
      const tasks = new Set(result.results.map(r => r.task));
      expect(tasks.size).toBe(2);
      expect(tasks.has('code-generation')).toBe(true);
      expect(tasks.has('text-summarization')).toBe(true);

      // Verify all models are covered
      const providers = new Set(result.results.map(r => r.provider));
      expect(providers.size).toBe(2);
    });

    it('should run multiple iterations per task', async () => {
      const benchmark = new MultiModelBenchmark({
        ...config,
        iterations: 5
      });
      const result = await benchmark.run();

      // 2 models × 2 tasks × 5 iterations = 20 results
      expect(result.results.length).toBe(20);
    });
  });

  describe('Performance Metrics', () => {
    it('should track latency for each test', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      result.results.forEach(r => {
        expect(r.latency).toBeGreaterThan(0);
        expect(r.latency).toBeLessThan(2000); // Reasonable latency limit
      });
    });

    it('should track cost for each test', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      result.results.forEach(r => {
        expect(r.cost).toBeGreaterThanOrEqual(0);
      });

      expect(result.summary.totalCost).toBeGreaterThan(0);
    });

    it('should track tokens used', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      result.results.forEach(r => {
        expect(r.tokensUsed).toBeGreaterThanOrEqual(0);
      });
    });

    it('should calculate quality scores', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      result.results.forEach(r => {
        expect(r.score).toBeGreaterThanOrEqual(0);
        expect(r.score).toBeLessThanOrEqual(1);
      });
    });
  });

  describe('Result Aggregation', () => {
    it('should generate summary statistics', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      expect(result.summary.totalTests).toBe(12);
      expect(result.summary.avgScore).toBeGreaterThan(0);
      expect(result.summary.avgLatency).toBeGreaterThan(0);
      expect(result.summary.totalCost).toBeGreaterThan(0);
      expect(result.summary.successRate).toBeGreaterThan(0);
      expect(result.summary.successRate).toBeLessThanOrEqual(1);
    });

    it('should include model comparison in summary', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      expect(result.summary.modelComparison).toBeDefined();
      expect(Array.isArray(result.summary.modelComparison)).toBe(true);
      expect(result.summary.modelComparison.length).toBe(2); // 2 models

      result.summary.modelComparison.forEach((comparison: any) => {
        expect(comparison.model).toBeDefined();
        expect(comparison.avgScore).toBeDefined();
        expect(comparison.minScore).toBeDefined();
        expect(comparison.maxScore).toBeDefined();
      });
    });

    it('should identify best performing model', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      expect(result.bestModel).toBeDefined();
      expect(result.bestProvider).toBeDefined();
      expect([ModelProvider.GEMINI, ModelProvider.CLAUDE]).toContain(result.bestProvider);

      // Verify the best model actually performed best
      const bestModelResults = result.results.filter(
        r => r.model === result.bestModel && r.provider === result.bestProvider
      );
      const avgBestScore = bestModelResults.reduce((sum, r) => sum + r.score, 0) / bestModelResults.length;

      // Best model should have above-average score
      expect(avgBestScore).toBeGreaterThanOrEqual(result.summary.avgScore * 0.9);
    });
  });

  describe('Model Comparison', () => {
    it('should directly compare two models', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.compare(
        config.models[0],
        config.models[1],
        'code-generation'
      );

      expect(result.winner).toBeDefined();
      expect([ModelProvider.GEMINI, ModelProvider.CLAUDE]).toContain(result.winner);
      expect(result.model1Results.length).toBe(3); // 3 iterations
      expect(result.model2Results.length).toBe(3);
      expect(result.comparison).toBeDefined();
      expect(result.comparison.scoreImprovement).toBeGreaterThanOrEqual(0);
    });

    it('should calculate score improvement in comparison', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.compare(
        config.models[0],
        config.models[1],
        'text-summarization'
      );

      expect(result.comparison.model1Avg).toBeGreaterThan(0);
      expect(result.comparison.model2Avg).toBeGreaterThan(0);
      expect(typeof result.comparison.scoreImprovement).toBe('number');
    });
  });

  describe('Error Handling', () => {
    it('should handle API failures gracefully', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      // Some tests might fail (simulated 5% failure rate)
      const failedTests = result.results.filter(r => r.score === 0);
      const successRate = result.summary.successRate;

      expect(successRate).toBeGreaterThan(0.8); // At least 80% success
      expect(successRate).toBeLessThanOrEqual(1.0);
    });

    it('should continue after individual test failures', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      // Should complete all tests even if some fail
      expect(result.results.length).toBe(12);
    });

    it('should handle timeout scenarios', async () => {
      const benchmark = new MultiModelBenchmark({
        ...config,
        timeout: 100 // Very short timeout
      });

      const result = await benchmark.run();
      expect(result.results).toBeDefined();
      // Tests should complete or fail, but not hang
    });
  });

  describe('Task Variations', () => {
    it('should handle single task benchmark', async () => {
      const benchmark = new MultiModelBenchmark({
        ...config,
        tasks: ['code-generation']
      });
      const result = await benchmark.run();

      expect(result.results.length).toBe(6); // 2 models × 1 task × 3 iterations
      expect(result.results.every(r => r.task === 'code-generation')).toBe(true);
    });

    it('should handle multiple task types', async () => {
      const benchmark = new MultiModelBenchmark({
        ...config,
        tasks: ['code-generation', 'text-summarization', 'data-analysis', 'creative-writing']
      });
      const result = await benchmark.run();

      // 2 models × 4 tasks × 3 iterations = 24 results
      expect(result.results.length).toBe(24);

      const tasks = new Set(result.results.map(r => r.task));
      expect(tasks.size).toBe(4);
    });
  });

  describe('Model Variations', () => {
    it('should handle single model benchmark', async () => {
      const benchmark = new MultiModelBenchmark({
        ...config,
        models: [config.models[0]]
      });
      const result = await benchmark.run();

      expect(result.results.length).toBe(6); // 1 model × 2 tasks × 3 iterations
      expect(result.results.every(r => r.provider === ModelProvider.GEMINI)).toBe(true);
    });

    it('should handle three or more models', async () => {
      const benchmark = new MultiModelBenchmark({
        ...config,
        models: [
          ...config.models,
          {
            provider: ModelProvider.GPT4,
            model: 'gpt-4-turbo',
            apiKey: 'test-key-3'
          }
        ]
      });
      const result = await benchmark.run();

      // 3 models × 2 tasks × 3 iterations = 18 results
      expect(result.results.length).toBe(18);

      const providers = new Set(result.results.map(r => r.provider));
      expect(providers.size).toBe(3);
    });
  });

  describe('Performance Analysis', () => {
    it('should track consistency across iterations', async () => {
      const benchmark = new MultiModelBenchmark({
        ...config,
        iterations: 10 // More iterations for consistency check
      });
      const result = await benchmark.run();

      // Group results by model and task
      const groupedResults = result.results.reduce((acc, r) => {
        const key = `${r.provider}:${r.task}`;
        if (!acc[key]) acc[key] = [];
        acc[key].push(r.score);
        return acc;
      }, {} as Record<string, number[]>);

      // Check variance isn't too high (scores should be relatively consistent)
      Object.values(groupedResults).forEach(scores => {
        const mean = scores.reduce((a, b) => a + b, 0) / scores.length;
        const variance = scores.reduce((sum, score) => sum + Math.pow(score - mean, 2), 0) / scores.length;
        const stdDev = Math.sqrt(variance);

        // Standard deviation should be reasonable (not random)
        expect(stdDev).toBeLessThan(0.3);
      });
    });

    it('should identify performance patterns', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      // Verify we can identify which model is better for which task
      const taskPerformance = result.results.reduce((acc, r) => {
        if (!acc[r.task]) acc[r.task] = {};
        if (!acc[r.task][r.provider]) acc[r.task][r.provider] = [];
        acc[r.task][r.provider].push(r.score);
        return acc;
      }, {} as Record<string, Record<string, number[]>>);

      // Each task should have results from both models
      Object.keys(taskPerformance).forEach(task => {
        expect(Object.keys(taskPerformance[task]).length).toBe(2);
      });
    });
  });

  describe('Cost Analysis', () => {
    it('should calculate total cost accurately', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      const manualTotal = result.results.reduce((sum, r) => sum + r.cost, 0);
      expect(result.summary.totalCost).toBeCloseTo(manualTotal, 2);
    });

    it('should track cost per model', async () => {
      const benchmark = new MultiModelBenchmark(config);
      const result = await benchmark.run();

      const costByModel = result.results.reduce((acc, r) => {
        const key = `${r.provider}:${r.model}`;
        acc[key] = (acc[key] || 0) + r.cost;
        return acc;
      }, {} as Record<string, number>);

      // Both models should have incurred costs
      expect(Object.keys(costByModel).length).toBe(2);
      Object.values(costByModel).forEach(cost => {
        expect(cost).toBeGreaterThan(0);
      });
    });
  });
});
