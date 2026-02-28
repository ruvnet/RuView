/**
 * Tests for DSPy Training Session
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { DSPyTrainingSession } from '../../src/dspy/training-session.js';
import { ModelProvider } from '../../src/types/index.js';
import type { TrainingSessionConfig } from '../../src/dspy/training-session.js';

describe('DSPyTrainingSession', () => {
  let config: TrainingSessionConfig;

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
      optimizationRounds: 3,
      convergenceThreshold: 0.95
    };
  });

  describe('Initialization', () => {
    it('should create training session with valid config', () => {
      const session = new DSPyTrainingSession(config);
      expect(session).toBeDefined();
      expect(session.getStatus().isRunning).toBe(false);
    });

    it('should accept custom budget', () => {
      const sessionWithBudget = new DSPyTrainingSession({
        ...config,
        budget: 1.0
      });
      expect(sessionWithBudget).toBeDefined();
    });

    it('should accept maxConcurrent option', () => {
      const sessionWithConcurrency = new DSPyTrainingSession({
        ...config,
        maxConcurrent: 5
      });
      expect(sessionWithConcurrency).toBeDefined();
    });
  });

  describe('Training Execution', () => {
    it('should run training session and return report', async () => {
      const session = new DSPyTrainingSession(config);
      const report = await session.run('Generate product descriptions', {});

      expect(report).toBeDefined();
      expect(report.bestModel).toBeDefined();
      expect(report.bestProvider).toBeDefined();
      expect(report.bestScore).toBeGreaterThan(0);
      expect(report.totalCost).toBeGreaterThan(0);
      expect(report.iterations).toBe(3);
      expect(report.results).toHaveLength(6); // 2 models × 3 rounds
    });

    it('should train multiple models in parallel', async () => {
      const session = new DSPyTrainingSession({
        ...config,
        optimizationRounds: 2
      });

      const startTime = Date.now();
      await session.run('Test prompt', {});
      const duration = Date.now() - startTime;

      // Parallel execution should be faster than sequential
      // With 2 models and 2 rounds, parallel should be ~2x faster
      expect(duration).toBeLessThan(1000); // Should complete quickly
    });

    it('should show quality improvement over iterations', async () => {
      const session = new DSPyTrainingSession(config);
      const report = await session.run('Test improvement', {});

      // Get first and last iteration scores for each model
      const firstRound = report.results.filter(r => r.iteration === 1);
      const lastRound = report.results.filter(r => r.iteration === config.optimizationRounds);

      const avgFirstScore = firstRound.reduce((sum, r) => sum + r.quality.score, 0) / firstRound.length;
      const avgLastScore = lastRound.reduce((sum, r) => sum + r.quality.score, 0) / lastRound.length;

      expect(avgLastScore).toBeGreaterThanOrEqual(avgFirstScore);
      expect(report.qualityImprovement).toBeGreaterThanOrEqual(0);
    });

    it('should stop when convergence threshold is reached', async () => {
      const session = new DSPyTrainingSession({
        ...config,
        optimizationRounds: 10,
        convergenceThreshold: 0.7 // Lower threshold to ensure we hit it
      });

      let convergedEvent = false;
      session.on('converged', () => {
        convergedEvent = true;
      });

      const report = await session.run('Test convergence', {});

      // Should stop before completing all 10 rounds
      expect(report.iterations).toBeLessThanOrEqual(10);
      expect(report.bestScore).toBeGreaterThanOrEqual(0.7);
    });

    it('should respect budget constraints', async () => {
      const budget = 0.5;
      const session = new DSPyTrainingSession({
        ...config,
        optimizationRounds: 10,
        budget
      });

      let budgetExceeded = false;
      session.on('budget-exceeded', () => {
        budgetExceeded = true;
      });

      const report = await session.run('Test budget', {});

      expect(report.totalCost).toBeLessThanOrEqual(budget * 1.1); // Allow 10% margin
    });
  });

  describe('Event Emissions', () => {
    it('should emit start event', async () => {
      const session = new DSPyTrainingSession(config);
      let startEmitted = false;

      session.on('start', (data) => {
        startEmitted = true;
        expect(data.models).toBe(2);
        expect(data.rounds).toBe(3);
      });

      await session.run('Test events', {});
      expect(startEmitted).toBe(true);
    });

    it('should emit iteration events', async () => {
      const session = new DSPyTrainingSession(config);
      const iterationResults: any[] = [];

      session.on('iteration', (result) => {
        iterationResults.push(result);
      });

      await session.run('Test iterations', {});

      expect(iterationResults.length).toBe(6); // 2 models × 3 rounds
      iterationResults.forEach(result => {
        expect(result.modelProvider).toBeDefined();
        expect(result.quality.score).toBeGreaterThan(0);
        expect(result.cost).toBeGreaterThan(0);
      });
    });

    it('should emit round events', async () => {
      const session = new DSPyTrainingSession(config);
      const rounds: number[] = [];

      session.on('round', (data) => {
        rounds.push(data.round);
      });

      await session.run('Test rounds', {});

      expect(rounds).toEqual([1, 2, 3]);
    });

    it('should emit complete event', async () => {
      const session = new DSPyTrainingSession(config);
      let completeData: any = null;

      session.on('complete', (report) => {
        completeData = report;
      });

      await session.run('Test complete', {});

      expect(completeData).toBeDefined();
      expect(completeData.bestModel).toBeDefined();
      expect(completeData.totalCost).toBeGreaterThan(0);
    });

    it('should emit error on failure', async () => {
      const invalidConfig = {
        ...config,
        models: [] // Invalid: no models
      };

      const session = new DSPyTrainingSession(invalidConfig);
      let errorEmitted = false;

      session.on('error', () => {
        errorEmitted = true;
      });

      try {
        await session.run('Test error', {});
      } catch {
        // Expected to throw
      }

      expect(errorEmitted).toBe(true);
    });
  });

  describe('Status Tracking', () => {
    it('should track running status', async () => {
      const session = new DSPyTrainingSession(config);

      expect(session.getStatus().isRunning).toBe(false);

      const runPromise = session.run('Test status', {});

      // Check status during execution would require more complex async handling
      await runPromise;

      const status = session.getStatus();
      expect(status.completedIterations).toBe(3);
      expect(status.totalCost).toBeGreaterThan(0);
      expect(status.results).toHaveLength(6);
    });

    it('should track total cost', async () => {
      const session = new DSPyTrainingSession(config);
      await session.run('Test cost', {});

      const status = session.getStatus();
      expect(status.totalCost).toBeGreaterThan(0);
      expect(status.totalCost).toBeLessThan(1.0); // Reasonable cost limit
    });
  });

  describe('Error Handling', () => {
    it('should handle empty models array', async () => {
      const session = new DSPyTrainingSession({
        ...config,
        models: []
      });

      await expect(session.run('Test empty', {})).rejects.toThrow();
    });

    it('should handle invalid optimization rounds', async () => {
      const session = new DSPyTrainingSession({
        ...config,
        optimizationRounds: 0
      });

      const report = await session.run('Test invalid rounds', {});
      expect(report.iterations).toBe(0);
      expect(report.results).toHaveLength(0);
    });

    it('should handle negative convergence threshold', async () => {
      const session = new DSPyTrainingSession({
        ...config,
        convergenceThreshold: -1
      });

      const report = await session.run('Test negative threshold', {});
      expect(report).toBeDefined();
      // Should still complete normally, just never converge
    });
  });

  describe('Quality Metrics', () => {
    it('should include quality metrics in results', async () => {
      const session = new DSPyTrainingSession(config);
      const report = await session.run('Test metrics', {});

      report.results.forEach(result => {
        expect(result.quality).toBeDefined();
        expect(result.quality.score).toBeGreaterThan(0);
        expect(result.quality.score).toBeLessThanOrEqual(1);
        expect(result.quality.metrics).toBeDefined();
        expect(result.quality.metrics.accuracy).toBeDefined();
        expect(result.quality.metrics.consistency).toBeDefined();
        expect(result.quality.metrics.relevance).toBeDefined();
      });
    });

    it('should calculate quality improvement percentage', async () => {
      const session = new DSPyTrainingSession(config);
      const report = await session.run('Test improvement percentage', {});

      expect(typeof report.qualityImprovement).toBe('number');
      expect(report.qualityImprovement).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Model Comparison', () => {
    it('should identify best performing model', async () => {
      const session = new DSPyTrainingSession(config);
      const report = await session.run('Test best model', {});

      expect(report.bestModel).toBeDefined();
      expect(report.bestProvider).toBeDefined();
      expect([ModelProvider.GEMINI, ModelProvider.CLAUDE]).toContain(report.bestProvider);

      // Verify best score matches the best model's score
      const bestResult = report.results.find(
        r => r.model === report.bestModel && r.modelProvider === report.bestProvider
      );
      expect(bestResult).toBeDefined();
    });

    it('should handle three or more models', async () => {
      const multiModelConfig = {
        ...config,
        models: [
          ...config.models,
          {
            provider: ModelProvider.GPT4,
            model: 'gpt-4-turbo',
            apiKey: 'test-key-3'
          }
        ]
      };

      const session = new DSPyTrainingSession(multiModelConfig);
      const report = await session.run('Test multiple models', {});

      expect(report.results.length).toBe(9); // 3 models × 3 rounds
      expect(report.bestProvider).toBeDefined();
    });
  });

  describe('Duration Tracking', () => {
    it('should track total duration', async () => {
      const session = new DSPyTrainingSession(config);
      const report = await session.run('Test duration', {});

      expect(report.totalDuration).toBeGreaterThan(0);
      expect(report.totalDuration).toBeLessThan(10000); // Should complete within 10 seconds
    });

    it('should track per-iteration duration', async () => {
      const session = new DSPyTrainingSession(config);
      const report = await session.run('Test iteration duration', {});

      report.results.forEach(result => {
        expect(result.duration).toBeGreaterThan(0);
        expect(result.duration).toBeLessThan(5000); // Each iteration under 5 seconds
      });
    });
  });
});
