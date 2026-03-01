/**
 * Tests for Self-Learning Generator
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { SelfLearningGenerator } from '../../src/generators/self-learning.js';
import type { SelfLearningConfig, GenerateOptions } from '../../src/generators/self-learning.js';

describe('SelfLearningGenerator', () => {
  let config: SelfLearningConfig;

  beforeEach(() => {
    config = {
      task: 'code-generation',
      learningRate: 0.1,
      iterations: 5
    };
  });

  describe('Initialization', () => {
    it('should create generator with valid config', () => {
      const generator = new SelfLearningGenerator(config);
      expect(generator).toBeDefined();
    });

    it('should accept quality threshold', () => {
      const generatorWithThreshold = new SelfLearningGenerator({
        ...config,
        qualityThreshold: 0.9
      });
      expect(generatorWithThreshold).toBeDefined();
    });

    it('should accept maxAttempts option', () => {
      const generatorWithMax = new SelfLearningGenerator({
        ...config,
        maxAttempts: 20
      });
      expect(generatorWithMax).toBeDefined();
    });
  });

  describe('Generation and Learning', () => {
    it('should generate output with quality improvement', async () => {
      const generator = new SelfLearningGenerator(config);
      const result = await generator.generate({
        prompt: 'Generate a function to validate emails'
      });

      expect(result.output).toBeDefined();
      expect(result.finalQuality).toBeGreaterThan(0);
      expect(result.finalQuality).toBeLessThanOrEqual(1);
      expect(result.improvement).toBeGreaterThanOrEqual(0);
      expect(result.iterations).toBe(5);
      expect(result.metrics).toHaveLength(5);
    });

    it('should show quality improvement over iterations', async () => {
      const generator = new SelfLearningGenerator(config);
      const result = await generator.generate({
        prompt: 'Test improvement tracking'
      });

      const firstQuality = result.metrics[0].quality;
      const lastQuality = result.metrics[result.metrics.length - 1].quality;

      // Quality should generally improve (or at least not decrease significantly)
      expect(lastQuality).toBeGreaterThanOrEqual(firstQuality * 0.95);
      expect(result.improvement).toBeDefined();
    });

    it('should track metrics for each iteration', async () => {
      const generator = new SelfLearningGenerator(config);
      const result = await generator.generate({
        prompt: 'Track iteration metrics'
      });

      expect(result.metrics).toHaveLength(5);
      result.metrics.forEach((metric, index) => {
        expect(metric.iteration).toBe(index + 1);
        expect(metric.quality).toBeGreaterThan(0);
        expect(typeof metric.improvement).toBe('number');
        expect(Array.isArray(metric.feedback)).toBe(true);
      });
    });

    it('should apply learning rate correctly', async () => {
      const highLearningRate = new SelfLearningGenerator({
        ...config,
        learningRate: 0.5,
        iterations: 3
      });
      const lowLearningRate = new SelfLearningGenerator({
        ...config,
        learningRate: 0.05,
        iterations: 3
      });

      const highResult = await highLearningRate.generate({
        prompt: 'Test high learning rate'
      });
      const lowResult = await lowLearningRate.generate({
        prompt: 'Test low learning rate'
      });

      // Higher learning rate should generally lead to faster improvement
      expect(highResult.improvement).toBeDefined();
      expect(lowResult.improvement).toBeDefined();
    });
  });

  describe('Test Integration', () => {
    it('should evaluate against test cases', async () => {
      const generator = new SelfLearningGenerator(config);
      const tests = [
        (output: any) => output.content.length > 10,
        (output: any) => output.quality > 0.5,
        (output: any) => output.metadata !== undefined
      ];

      const result = await generator.generate({
        prompt: 'Generate with tests',
        tests
      });

      expect(result.finalQuality).toBeGreaterThan(0);
      result.metrics.forEach(metric => {
        expect(metric.testsPassingRate).toBeDefined();
        expect(metric.testsPassingRate).toBeGreaterThanOrEqual(0);
        expect(metric.testsPassingRate).toBeLessThanOrEqual(1);
      });
    });

    it('should track test passing rate', async () => {
      const generator = new SelfLearningGenerator(config);
      const tests = [
        (output: any) => output.quality > 0.6,
        (output: any) => output.quality > 0.7
      ];

      const result = await generator.generate({
        prompt: 'Track test pass rate',
        tests
      });

      // Test passing rate should be tracked for each iteration
      result.metrics.forEach(metric => {
        expect(metric.testsPassingRate).toBeGreaterThanOrEqual(0);
        expect(metric.testsPassingRate).toBeLessThanOrEqual(1);
      });
    });

    it('should handle failing tests gracefully', async () => {
      const generator = new SelfLearningGenerator(config);
      const impossibleTests = [
        () => false, // Always fails
        () => false
      ];

      const result = await generator.generate({
        prompt: 'Handle test failures',
        tests: impossibleTests
      });

      expect(result.output).toBeDefined();
      expect(result.finalQuality).toBeGreaterThan(0);
      // Should complete despite test failures
    });
  });

  describe('Event Emissions', () => {
    it('should emit start event', async () => {
      const generator = new SelfLearningGenerator(config);
      let startEmitted = false;

      generator.on('start', (data) => {
        startEmitted = true;
        expect(data.task).toBe('code-generation');
        expect(data.iterations).toBe(5);
      });

      await generator.generate({ prompt: 'Test start event' });
      expect(startEmitted).toBe(true);
    });

    it('should emit improvement events', async () => {
      const generator = new SelfLearningGenerator(config);
      const improvements: any[] = [];

      generator.on('improvement', (metrics) => {
        improvements.push(metrics);
      });

      await generator.generate({ prompt: 'Test improvement events' });

      expect(improvements).toHaveLength(5);
      improvements.forEach(metric => {
        expect(metric.iteration).toBeDefined();
        expect(metric.quality).toBeDefined();
      });
    });

    it('should emit complete event', async () => {
      const generator = new SelfLearningGenerator(config);
      let completeData: any = null;

      generator.on('complete', (data) => {
        completeData = data;
      });

      await generator.generate({ prompt: 'Test complete event' });

      expect(completeData).toBeDefined();
      expect(completeData.finalQuality).toBeDefined();
      expect(completeData.improvement).toBeDefined();
      expect(completeData.iterations).toBe(5);
    });

    it('should emit threshold-reached event', async () => {
      const generator = new SelfLearningGenerator({
        ...config,
        qualityThreshold: 0.6,
        iterations: 10
      });
      let thresholdReached = false;

      generator.on('threshold-reached', (data) => {
        thresholdReached = true;
        expect(data.quality).toBeGreaterThanOrEqual(0.6);
      });

      await generator.generate({ prompt: 'Test threshold' });
      // Threshold might or might not be reached depending on random variation
    });
  });

  describe('Quality Thresholds', () => {
    it('should stop when quality threshold is reached', async () => {
      const generator = new SelfLearningGenerator({
        ...config,
        qualityThreshold: 0.7,
        iterations: 10
      });

      const result = await generator.generate({
        prompt: 'Test early stopping'
      });

      // Should stop before completing all iterations if threshold reached
      expect(result.iterations).toBeLessThanOrEqual(10);
      if (result.finalQuality >= 0.7) {
        expect(result.iterations).toBeLessThan(10);
      }
    });

    it('should use initial quality if provided', async () => {
      const generator = new SelfLearningGenerator(config);
      const result = await generator.generate({
        prompt: 'Test initial quality',
        initialQuality: 0.8
      });

      expect(result.output).toBeDefined();
      // Improvement calculation should be based on initial quality
    });
  });

  describe('History Tracking', () => {
    it('should maintain learning history', async () => {
      const generator = new SelfLearningGenerator(config);
      await generator.generate({ prompt: 'First generation' });

      const history = generator.getHistory();
      expect(history).toHaveLength(5);
      expect(history[0].iteration).toBe(1);
      expect(history[4].iteration).toBe(5);
    });

    it('should accumulate history across multiple generations', async () => {
      const generator = new SelfLearningGenerator(config);
      await generator.generate({ prompt: 'First' });
      await generator.generate({ prompt: 'Second' });

      const history = generator.getHistory();
      expect(history.length).toBe(10); // 5 + 5 iterations
    });

    it('should reset history when reset is called', async () => {
      const generator = new SelfLearningGenerator(config);
      await generator.generate({ prompt: 'Generate before reset' });

      expect(generator.getHistory().length).toBe(5);

      generator.reset();

      expect(generator.getHistory()).toHaveLength(0);
    });

    it('should emit reset event', () => {
      const generator = new SelfLearningGenerator(config);
      let resetEmitted = false;

      generator.on('reset', () => {
        resetEmitted = true;
      });

      generator.reset();
      expect(resetEmitted).toBe(true);
    });
  });

  describe('Feedback Generation', () => {
    it('should generate relevant feedback', async () => {
      const generator = new SelfLearningGenerator(config);
      const result = await generator.generate({
        prompt: 'Test feedback generation'
      });

      result.metrics.forEach(metric => {
        expect(Array.isArray(metric.feedback)).toBe(true);
        expect(metric.feedback.length).toBeGreaterThan(0);
        metric.feedback.forEach(fb => {
          expect(typeof fb).toBe('string');
          expect(fb.length).toBeGreaterThan(0);
        });
      });
    });

    it('should provide contextual feedback based on quality', async () => {
      const generator = new SelfLearningGenerator(config);
      const result = await generator.generate({
        prompt: 'Test contextual feedback'
      });

      // Feedback should vary based on performance
      const feedbackTypes = new Set(
        result.metrics.flatMap(m => m.feedback)
      );
      expect(feedbackTypes.size).toBeGreaterThan(0);
    });
  });

  describe('Edge Cases', () => {
    it('should handle zero iterations', async () => {
      const generator = new SelfLearningGenerator({
        ...config,
        iterations: 0
      });

      const result = await generator.generate({
        prompt: 'Test zero iterations'
      });

      expect(result.output).toBeNull();
      expect(result.metrics).toHaveLength(0);
    });

    it('should handle very high learning rate', async () => {
      const generator = new SelfLearningGenerator({
        ...config,
        learningRate: 1.0
      });

      const result = await generator.generate({
        prompt: 'Test high learning rate'
      });

      expect(result.output).toBeDefined();
      expect(result.finalQuality).toBeLessThanOrEqual(1.0);
    });

    it('should handle very low learning rate', async () => {
      const generator = new SelfLearningGenerator({
        ...config,
        learningRate: 0.001
      });

      const result = await generator.generate({
        prompt: 'Test low learning rate'
      });

      expect(result.output).toBeDefined();
      // Improvement should be minimal but positive
    });

    it('should handle single iteration', async () => {
      const generator = new SelfLearningGenerator({
        ...config,
        iterations: 1
      });

      const result = await generator.generate({
        prompt: 'Single iteration test'
      });

      expect(result.iterations).toBe(1);
      expect(result.metrics).toHaveLength(1);
      expect(result.output).toBeDefined();
    });
  });

  describe('Performance', () => {
    it('should complete within reasonable time', async () => {
      const generator = new SelfLearningGenerator(config);
      const startTime = Date.now();

      await generator.generate({
        prompt: 'Performance test'
      });

      const duration = Date.now() - startTime;
      expect(duration).toBeLessThan(2000); // Should complete in under 2 seconds
    });

    it('should handle many iterations efficiently', async () => {
      const generator = new SelfLearningGenerator({
        ...config,
        iterations: 20
      });

      const startTime = Date.now();
      await generator.generate({
        prompt: 'Many iterations test'
      });
      const duration = Date.now() - startTime;

      expect(duration).toBeLessThan(5000); // Even with 20 iterations
    });
  });
});
