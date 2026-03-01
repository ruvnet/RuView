/**
 * Integration Tests
 * End-to-end workflows and package integration
 */

import { describe, it, expect } from 'vitest';
import { DSPyTrainingSession, MultiModelBenchmark } from '../src/dspy/index.js';
import { SelfLearningGenerator } from '../src/generators/self-learning.js';
import { StockMarketSimulator } from '../src/generators/stock-market.js';
import { ModelProvider } from '../src/types/index.js';

describe('Integration Tests', () => {
  describe('Package Exports', () => {
    it('should export all main classes', () => {
      expect(DSPyTrainingSession).toBeDefined();
      expect(MultiModelBenchmark).toBeDefined();
      expect(SelfLearningGenerator).toBeDefined();
      expect(StockMarketSimulator).toBeDefined();
    });

    it('should export types and enums', () => {
      expect(ModelProvider).toBeDefined();
      expect(ModelProvider.GEMINI).toBe('gemini');
      expect(ModelProvider.CLAUDE).toBe('claude');
      expect(ModelProvider.GPT4).toBe('gpt4');
      expect(ModelProvider.LLAMA).toBe('llama');
    });
  });

  describe('End-to-End Workflows', () => {
    it('should complete full DSPy training workflow', async () => {
      const session = new DSPyTrainingSession({
        models: [
          {
            provider: ModelProvider.GEMINI,
            model: 'gemini-2.0-flash-exp',
            apiKey: 'test-key'
          }
        ],
        optimizationRounds: 2,
        convergenceThreshold: 0.95
      });

      const report = await session.run('Generate test data', {});

      expect(report).toBeDefined();
      expect(report.bestModel).toBeDefined();
      expect(report.totalCost).toBeGreaterThan(0);
      expect(report.results.length).toBe(2); // 2 rounds
    });

    it('should complete self-learning generation workflow', async () => {
      const generator = new SelfLearningGenerator({
        task: 'test-generation',
        learningRate: 0.1,
        iterations: 3
      });

      const result = await generator.generate({
        prompt: 'Generate test content'
      });

      expect(result.output).toBeDefined();
      expect(result.finalQuality).toBeGreaterThan(0);
      expect(result.metrics.length).toBe(3);
    });

    it('should complete stock market simulation workflow', async () => {
      const simulator = new StockMarketSimulator({
        symbols: ['AAPL'],
        startDate: '2024-01-01',
        endDate: '2024-01-05',
        volatility: 'medium'
      });

      const data = await simulator.generate();

      expect(data.length).toBeGreaterThan(0);
      expect(data[0].symbol).toBe('AAPL');
      expect(data[0].open).toBeGreaterThan(0);
    });

    it('should complete benchmark workflow', async () => {
      const benchmark = new MultiModelBenchmark({
        models: [
          {
            provider: ModelProvider.GEMINI,
            model: 'gemini-2.0-flash-exp',
            apiKey: 'test-key'
          }
        ],
        tasks: ['test-task'],
        iterations: 2
      });

      const result = await benchmark.run();

      expect(result.results.length).toBe(2); // 1 model × 1 task × 2 iterations
      expect(result.bestModel).toBeDefined();
      expect(result.summary).toBeDefined();
    });
  });

  describe('Cross-Component Integration', () => {
    it('should use training results in benchmark', async () => {
      // Train models
      const session = new DSPyTrainingSession({
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
        optimizationRounds: 2,
        convergenceThreshold: 0.95
      });

      const trainingReport = await session.run('Test prompt', {});

      // Use trained models in benchmark
      const benchmark = new MultiModelBenchmark({
        models: [
          {
            provider: trainingReport.bestProvider,
            model: trainingReport.bestModel,
            apiKey: 'test-key'
          }
        ],
        tasks: ['validation'],
        iterations: 1
      });

      const benchmarkResult = await benchmark.run();

      expect(benchmarkResult.results.length).toBe(1);
      expect(benchmarkResult.bestProvider).toBe(trainingReport.bestProvider);
    });

    it('should use self-learning with quality metrics', async () => {
      const generator = new SelfLearningGenerator({
        task: 'quality-test',
        learningRate: 0.2,
        iterations: 5,
        qualityThreshold: 0.8
      });

      let improvementEvents = 0;
      generator.on('improvement', () => {
        improvementEvents++;
      });

      const result = await generator.generate({
        prompt: 'Generate with quality tracking',
        tests: [
          (output: any) => output.quality > 0.5,
          (output: any) => output.content.length > 0
        ]
      });

      expect(result.finalQuality).toBeGreaterThan(0);
      expect(improvementEvents).toBeGreaterThan(0);
      expect(result.metrics.every(m => m.testsPassingRate !== undefined)).toBe(true);
    });

    it('should integrate stock market data with statistics', async () => {
      const simulator = new StockMarketSimulator({
        symbols: ['AAPL', 'GOOGL'],
        startDate: '2024-01-01',
        endDate: '2024-01-15',
        volatility: 'high'
      });

      const data = await simulator.generate({
        includeSentiment: true,
        includeNews: true,
        marketConditions: 'bullish'
      });

      expect(data.length).toBeGreaterThan(0);

      // Get statistics for each symbol
      const aaplData = data.filter(d => d.symbol === 'AAPL');
      const googlData = data.filter(d => d.symbol === 'GOOGL');

      const aaplStats = simulator.getStatistics(aaplData);
      const googlStats = simulator.getStatistics(googlData);

      expect(aaplStats.totalDays).toBeGreaterThan(0);
      expect(googlStats.totalDays).toBeGreaterThan(0);
      expect(aaplStats.volatility).toBeGreaterThan(0);
      expect(googlStats.volatility).toBeGreaterThan(0);

      // Check sentiment is included
      expect(data.some(d => d.sentiment !== undefined)).toBe(true);
    });
  });

  describe('Event-Driven Coordination', () => {
    it('should coordinate events across DSPy training', async () => {
      const session = new DSPyTrainingSession({
        models: [
          {
            provider: ModelProvider.GEMINI,
            model: 'gemini-2.0-flash-exp',
            apiKey: 'test-key'
          }
        ],
        optimizationRounds: 3,
        convergenceThreshold: 0.95
      });

      const events: string[] = [];

      session.on('start', () => events.push('start'));
      session.on('round', () => events.push('round'));
      session.on('iteration', () => events.push('iteration'));
      session.on('complete', () => events.push('complete'));

      await session.run('Coordinate events', {});

      expect(events).toContain('start');
      expect(events).toContain('round');
      expect(events).toContain('iteration');
      expect(events).toContain('complete');
      expect(events[0]).toBe('start');
      expect(events[events.length - 1]).toBe('complete');
    });

    it('should coordinate events in self-learning', async () => {
      const generator = new SelfLearningGenerator({
        task: 'event-test',
        learningRate: 0.1,
        iterations: 3
      });

      const events: string[] = [];

      generator.on('start', () => events.push('start'));
      generator.on('improvement', () => events.push('improvement'));
      generator.on('complete', () => events.push('complete'));

      await generator.generate({ prompt: 'Test events' });

      expect(events).toContain('start');
      expect(events).toContain('improvement');
      expect(events).toContain('complete');
      expect(events.filter(e => e === 'improvement').length).toBe(3);
    });
  });

  describe('Error Recovery', () => {
    it('should handle errors gracefully in training', async () => {
      const session = new DSPyTrainingSession({
        models: [], // Invalid: no models
        optimizationRounds: 2,
        convergenceThreshold: 0.95
      });

      await expect(session.run('Test error', {})).rejects.toThrow();
    });

    it('should continue after partial failures in benchmark', async () => {
      const benchmark = new MultiModelBenchmark({
        models: [
          {
            provider: ModelProvider.GEMINI,
            model: 'gemini-2.0-flash-exp',
            apiKey: 'test-key'
          }
        ],
        tasks: ['task1', 'task2'],
        iterations: 3
      });

      const result = await benchmark.run();

      // Should complete even with simulated 5% failure rate
      expect(result.results).toBeDefined();
      expect(result.summary.successRate).toBeGreaterThan(0);
    });
  });

  describe('Performance at Scale', () => {
    it('should handle multiple models and rounds efficiently', async () => {
      const session = new DSPyTrainingSession({
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
          },
          {
            provider: ModelProvider.GPT4,
            model: 'gpt-4-turbo',
            apiKey: 'test-key-3'
          }
        ],
        optimizationRounds: 3,
        convergenceThreshold: 0.95
      });

      const startTime = Date.now();
      const report = await session.run('Scale test', {});
      const duration = Date.now() - startTime;

      expect(report.results.length).toBe(9); // 3 models × 3 rounds
      expect(duration).toBeLessThan(3000); // Should complete quickly with parallel execution
    });

    it('should handle long time series efficiently', async () => {
      const simulator = new StockMarketSimulator({
        symbols: ['AAPL', 'GOOGL', 'MSFT'],
        startDate: '2024-01-01',
        endDate: '2024-12-31',
        volatility: 'medium'
      });

      const startTime = Date.now();
      const data = await simulator.generate();
      const duration = Date.now() - startTime;

      expect(data.length).toBeGreaterThan(500); // ~252 trading days × 3 symbols
      expect(duration).toBeLessThan(2000); // Should generate efficiently
    });

    it('should handle many learning iterations', async () => {
      const generator = new SelfLearningGenerator({
        task: 'scale-test',
        learningRate: 0.05,
        iterations: 20
      });

      const startTime = Date.now();
      const result = await generator.generate({
        prompt: 'Scale test prompt'
      });
      const duration = Date.now() - startTime;

      expect(result.iterations).toBe(20);
      expect(result.metrics.length).toBe(20);
      expect(duration).toBeLessThan(5000); // Should complete in reasonable time
    });
  });

  describe('Data Consistency', () => {
    it('should maintain consistency in training results', async () => {
      const session = new DSPyTrainingSession({
        models: [
          {
            provider: ModelProvider.GEMINI,
            model: 'gemini-2.0-flash-exp',
            apiKey: 'test-key'
          }
        ],
        optimizationRounds: 3,
        convergenceThreshold: 0.95
      });

      const report = await session.run('Consistency test', {});

      // Verify result consistency
      expect(report.results.length).toBe(3);
      expect(report.iterations).toBe(3);
      expect(report.results.every(r => r.modelProvider === ModelProvider.GEMINI)).toBe(true);

      // Verify cost tracking
      const totalCost = report.results.reduce((sum, r) => sum + r.cost, 0);
      expect(Math.abs(totalCost - report.totalCost)).toBeLessThan(0.01);
    });

    it('should maintain data integrity in stock simulation', async () => {
      const simulator = new StockMarketSimulator({
        symbols: ['AAPL'],
        startDate: '2024-01-01',
        endDate: '2024-01-10',
        volatility: 'medium'
      });

      const data = await simulator.generate();

      // Verify sequential dates
      for (let i = 1; i < data.length; i++) {
        const prevDate = data[i - 1].date;
        const currDate = data[i].date;
        expect(currDate.getTime()).toBeGreaterThan(prevDate.getTime());
      }

      // Verify OHLCV consistency
      data.forEach(point => {
        expect(point.high).toBeGreaterThanOrEqual(point.open);
        expect(point.high).toBeGreaterThanOrEqual(point.close);
        expect(point.low).toBeLessThanOrEqual(point.open);
        expect(point.low).toBeLessThanOrEqual(point.close);
      });
    });
  });

  describe('Real-World Scenarios', () => {
    it('should support model selection workflow', async () => {
      // Step 1: Train multiple models
      const session = new DSPyTrainingSession({
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
        optimizationRounds: 2,
        convergenceThreshold: 0.95
      });

      const trainingReport = await session.run('Select best model', {});

      // Step 2: Benchmark the best model
      const benchmark = new MultiModelBenchmark({
        models: [
          {
            provider: trainingReport.bestProvider,
            model: trainingReport.bestModel,
            apiKey: 'test-key'
          }
        ],
        tasks: ['validation', 'production'],
        iterations: 3
      });

      const benchmarkResult = await benchmark.run();

      // Step 3: Verify the selected model performs well
      expect(benchmarkResult.summary.avgScore).toBeGreaterThan(0.5);
      expect(benchmarkResult.summary.successRate).toBeGreaterThan(0.8);
    });

    it('should support data generation for testing', async () => {
      // Generate synthetic financial data
      const simulator = new StockMarketSimulator({
        symbols: ['TEST1', 'TEST2'],
        startDate: '2024-01-01',
        endDate: '2024-01-31',
        volatility: 'low'
      });

      const testData = await simulator.generate({
        includeSentiment: true,
        marketConditions: 'neutral'
      });

      // Use the data for testing purposes
      expect(testData.length).toBeGreaterThan(0);

      // Verify data is suitable for testing
      const stats = simulator.getStatistics(testData.filter(d => d.symbol === 'TEST1'));
      expect(stats.totalDays).toBeGreaterThan(10);
      expect(stats.avgPrice).toBeGreaterThan(0);
      expect(stats.volatility).toBeLessThan(10); // Low volatility
    });

    it('should support iterative improvement workflow', async () => {
      const generator = new SelfLearningGenerator({
        task: 'iterative-improvement',
        learningRate: 0.15,
        iterations: 5,
        qualityThreshold: 0.85
      });

      // Track improvement over multiple generations
      const run1 = await generator.generate({
        prompt: 'Initial generation',
        initialQuality: 0.5
      });

      const run2 = await generator.generate({
        prompt: 'Improved generation',
        initialQuality: run1.finalQuality
      });

      // Second run should start from where first ended
      expect(run2.finalQuality).toBeGreaterThanOrEqual(run1.finalQuality * 0.95);
    });
  });
});
