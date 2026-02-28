/**
 * DSPy Learning Session - Unit Tests
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  DSPyTrainingSession,
  ModelProvider,
  TrainingPhase,
  ClaudeSonnetAgent,
  GPT4Agent,
  GeminiAgent,
  LlamaAgent,
  OptimizationEngine,
  BenchmarkCollector,
  type ModelConfig,
  type DSPySignature,
  type IterationResult,
  type QualityMetrics,
  type PerformanceMetrics
} from '../training/dspy-learning-session.js';

describe('DSPyTrainingSession', () => {
  let config: any;

  beforeEach(() => {
    config = {
      models: [
        {
          provider: ModelProvider.GEMINI,
          model: 'gemini-2.0-flash-exp',
          apiKey: 'test-key-gemini'
        },
        {
          provider: ModelProvider.CLAUDE,
          model: 'claude-sonnet-4',
          apiKey: 'test-key-claude'
        }
      ],
      optimizationRounds: 2,
      convergenceThreshold: 0.9,
      maxConcurrency: 2,
      enableCrossLearning: true,
      enableHooksIntegration: false,
      costBudget: 1.0,
      timeoutPerIteration: 5000,
      baselineIterations: 2,
      benchmarkSamples: 5
    };
  });

  describe('Constructor', () => {
    it('should create a training session with valid config', () => {
      const session = new DSPyTrainingSession(config);
      expect(session).toBeDefined();
      expect(session.getStatistics()).toBeDefined();
    });

    it('should throw error with invalid config', () => {
      const invalidConfig = { ...config, models: [] };
      expect(() => new DSPyTrainingSession(invalidConfig)).toThrow();
    });

    it('should initialize with default values', () => {
      const minimalConfig = {
        models: [
          {
            provider: ModelProvider.GEMINI,
            model: 'gemini-2.0-flash-exp',
            apiKey: 'test-key'
          }
        ]
      };

      const session = new DSPyTrainingSession(minimalConfig);
      const stats = session.getStatistics();

      expect(stats.currentPhase).toBe(TrainingPhase.BASELINE);
      expect(stats.totalCost).toBe(0);
    });
  });

  describe('Event System', () => {
    it('should emit start event', async () => {
      const session = new DSPyTrainingSession(config);

      await new Promise<void>((resolve) => {
        session.on('start', (data) => {
          expect(data.phase).toBe(TrainingPhase.BASELINE);
          resolve();
        });

        const optimizer = new OptimizationEngine();
        const signature = optimizer.createSignature('test', 'input', 'output');

        session.run('test prompt', signature);
      });
    });

    it('should emit phase transitions', async () => {
      const session = new DSPyTrainingSession(config);
      const phases: TrainingPhase[] = [];

      await new Promise<void>((resolve) => {
        session.on('phase', (phase) => {
          phases.push(phase);
        });

        session.on('complete', () => {
          expect(phases.length).toBeGreaterThan(0);
          expect(phases).toContain(TrainingPhase.BASELINE);
          resolve();
        });

        const optimizer = new OptimizationEngine();
        const signature = optimizer.createSignature('test', 'input', 'output');

        session.run('test prompt', signature);
      });
    });

    it('should emit iteration events', async () => {
      const session = new DSPyTrainingSession(config);
      let iterationCount = 0;

      await new Promise<void>((resolve) => {
        session.on('iteration', (result) => {
          iterationCount++;
          expect(result).toBeDefined();
          expect(result.modelProvider).toBeDefined();
          expect(result.quality).toBeDefined();
          expect(result.performance).toBeDefined();
        });

        session.on('complete', () => {
          expect(iterationCount).toBeGreaterThan(0);
          resolve();
        });

        const optimizer = new OptimizationEngine();
        const signature = optimizer.createSignature('test', 'input', 'output');

        session.run('test prompt', signature);
      });
    });
  });

  describe('Statistics', () => {
    it('should track session statistics', () => {
      const session = new DSPyTrainingSession(config);
      const initialStats = session.getStatistics();

      expect(initialStats.currentPhase).toBe(TrainingPhase.BASELINE);
      expect(initialStats.totalCost).toBe(0);
      expect(initialStats.duration).toBeGreaterThanOrEqual(0);
    });

    it('should update cost during training', async () => {
      const session = new DSPyTrainingSession(config);

      await new Promise<void>((resolve) => {
        session.on('complete', () => {
          const stats = session.getStatistics();
          expect(stats.totalCost).toBeGreaterThan(0);
          resolve();
        });

        const optimizer = new OptimizationEngine();
        const signature = optimizer.createSignature('test', 'input', 'output');

        session.run('test prompt', signature);
      });
    });
  });

  describe('Stop Functionality', () => {
    it('should stop training session', async () => {
      const session = new DSPyTrainingSession(config);

      await new Promise<void>((resolve) => {
        session.on('stopped', (stats) => {
          expect(stats).toBeDefined();
          expect(stats.currentPhase).toBeDefined();
          resolve();
        });

        setTimeout(() => {
          session.stop();
        }, 100);

        const optimizer = new OptimizationEngine();
        const signature = optimizer.createSignature('test', 'input', 'output');

        session.run('test prompt', signature);
      });
    });
  });
});

describe('Model Agents', () => {
  describe('ClaudeSonnetAgent', () => {
    let agent: ClaudeSonnetAgent;
    let config: ModelConfig;

    beforeEach(() => {
      config = {
        provider: ModelProvider.CLAUDE,
        model: 'claude-sonnet-4',
        apiKey: 'test-key',
        temperature: 0.7
      };
      agent = new ClaudeSonnetAgent(config);
    });

    it('should execute and return result', async () => {
      const signature: DSPySignature = {
        input: 'test input',
        output: 'test output'
      };

      const result = await agent.execute('test prompt', signature);

      expect(result).toBeDefined();
      expect(result.modelProvider).toBe(ModelProvider.CLAUDE);
      expect(result.quality).toBeDefined();
      expect(result.performance).toBeDefined();
      expect(result.quality.score).toBeGreaterThanOrEqual(0);
      expect(result.quality.score).toBeLessThanOrEqual(1);
    });

    it('should track results', async () => {
      const signature: DSPySignature = {
        input: 'test input',
        output: 'test output'
      };

      await agent.execute('test prompt 1', signature);
      await agent.execute('test prompt 2', signature);

      const results = agent.getResults();
      expect(results.length).toBe(2);
    });

    it('should track total cost', async () => {
      const signature: DSPySignature = {
        input: 'test input',
        output: 'test output'
      };

      await agent.execute('test prompt', signature);

      const cost = agent.getTotalCost();
      expect(cost).toBeGreaterThan(0);
    });
  });

  describe('GPT4Agent', () => {
    it('should execute with correct provider', async () => {
      const config: ModelConfig = {
        provider: ModelProvider.GPT4,
        model: 'gpt-4-turbo',
        apiKey: 'test-key'
      };
      const agent = new GPT4Agent(config);
      const signature: DSPySignature = {
        input: 'test',
        output: 'test'
      };

      const result = await agent.execute('test', signature);

      expect(result.modelProvider).toBe(ModelProvider.GPT4);
    });
  });

  describe('GeminiAgent', () => {
    it('should execute with correct provider', async () => {
      const config: ModelConfig = {
        provider: ModelProvider.GEMINI,
        model: 'gemini-2.0-flash-exp',
        apiKey: 'test-key'
      };
      const agent = new GeminiAgent(config);
      const signature: DSPySignature = {
        input: 'test',
        output: 'test'
      };

      const result = await agent.execute('test', signature);

      expect(result.modelProvider).toBe(ModelProvider.GEMINI);
    });
  });

  describe('LlamaAgent', () => {
    it('should execute with correct provider', async () => {
      const config: ModelConfig = {
        provider: ModelProvider.LLAMA,
        model: 'llama-3.1-70b',
        apiKey: 'test-key'
      };
      const agent = new LlamaAgent(config);
      const signature: DSPySignature = {
        input: 'test',
        output: 'test'
      };

      const result = await agent.execute('test', signature);

      expect(result.modelProvider).toBe(ModelProvider.LLAMA);
    });
  });
});

describe('OptimizationEngine', () => {
  let optimizer: OptimizationEngine;

  beforeEach(() => {
    optimizer = new OptimizationEngine();
  });

  describe('Signature Creation', () => {
    it('should create basic signature', () => {
      const signature = optimizer.createSignature(
        'test',
        'input',
        'output'
      );

      expect(signature).toBeDefined();
      expect(signature.input).toBe('input');
      expect(signature.output).toBe('output');
      expect(signature.examples).toEqual([]);
      expect(signature.constraints).toEqual([]);
      expect(signature.objectives).toEqual([]);
    });

    it('should create signature with options', () => {
      const signature = optimizer.createSignature(
        'test',
        'input',
        'output',
        {
          examples: [{ input: 'ex1', output: 'ex1' }],
          constraints: ['min_length:10'],
          objectives: ['maximize quality']
        }
      );

      expect(signature.examples?.length).toBe(1);
      expect(signature.constraints?.length).toBe(1);
      expect(signature.objectives?.length).toBe(1);
    });
  });

  describe('Prompt Optimization', () => {
    it('should optimize prompt based on results', async () => {
      const signature: DSPySignature = {
        input: 'test input',
        output: 'test output',
        examples: [{ input: 'example', output: 'example output' }],
        constraints: ['min_length:10'],
        objectives: ['high quality']
      };

      const results: IterationResult[] = [
        {
          iteration: 1,
          phase: TrainingPhase.BASELINE,
          modelProvider: ModelProvider.GEMINI,
          quality: {
            score: 0.5,
            accuracy: 0.5,
            coherence: 0.5,
            relevance: 0.5,
            diversity: 0.5,
            creativity: 0.5
          },
          performance: {
            latency: 100,
            throughput: 10,
            tokensUsed: 100,
            cost: 0.01,
            memoryUsage: 50,
            errorRate: 0
          },
          timestamp: new Date(),
          prompt: 'base prompt',
          output: 'base output',
          optimizations: []
        }
      ];

      const optimized = await optimizer.optimizePrompt(
        'base prompt',
        results,
        signature
      );

      expect(optimized).toBeDefined();
      expect(optimized.length).toBeGreaterThan('base prompt'.length);
    });
  });

  describe('Cross-Model Optimization', () => {
    it('should perform cross-model optimization', async () => {
      const allResults = new Map<ModelProvider, IterationResult[]>();

      const result1: IterationResult = {
        iteration: 1,
        phase: TrainingPhase.BASELINE,
        modelProvider: ModelProvider.GEMINI,
        quality: {
          score: 0.9,
          accuracy: 0.9,
          coherence: 0.9,
          relevance: 0.9,
          diversity: 0.9,
          creativity: 0.9
        },
        performance: {
          latency: 100,
          throughput: 10,
          tokensUsed: 100,
          cost: 0.01,
          memoryUsage: 50,
          errorRate: 0
        },
        timestamp: new Date(),
        prompt: 'good prompt',
        output: 'good output',
        optimizations: []
      };

      const result2: IterationResult = {
        ...result1,
        modelProvider: ModelProvider.CLAUDE,
        quality: {
          score: 0.5,
          accuracy: 0.5,
          coherence: 0.5,
          relevance: 0.5,
          diversity: 0.5,
          creativity: 0.5
        },
        prompt: 'poor prompt'
      };

      allResults.set(ModelProvider.GEMINI, [result1]);
      allResults.set(ModelProvider.CLAUDE, [result2]);

      const optimized = await optimizer.crossModelOptimization(allResults);

      expect(optimized).toBeDefined();
      expect(optimized.size).toBeGreaterThan(0);
    });
  });
});

describe('BenchmarkCollector', () => {
  let collector: BenchmarkCollector;

  beforeEach(() => {
    collector = new BenchmarkCollector();
  });

  describe('Result Collection', () => {
    it('should add results', () => {
      const result: IterationResult = {
        iteration: 1,
        phase: TrainingPhase.BASELINE,
        modelProvider: ModelProvider.GEMINI,
        quality: {
          score: 0.8,
          accuracy: 0.8,
          coherence: 0.8,
          relevance: 0.8,
          diversity: 0.8,
          creativity: 0.8
        },
        performance: {
          latency: 100,
          throughput: 10,
          tokensUsed: 100,
          cost: 0.01,
          memoryUsage: 50,
          errorRate: 0
        },
        timestamp: new Date(),
        prompt: 'test',
        output: 'test',
        optimizations: []
      };

      collector.addResult(result);

      const metrics = collector.getModelMetrics(ModelProvider.GEMINI);
      expect(metrics.length).toBe(1);
      expect(metrics[0]).toEqual(result);
    });

    it('should get metrics for specific model', () => {
      const result1: IterationResult = {
        iteration: 1,
        phase: TrainingPhase.BASELINE,
        modelProvider: ModelProvider.GEMINI,
        quality: {
          score: 0.8,
          accuracy: 0.8,
          coherence: 0.8,
          relevance: 0.8,
          diversity: 0.8,
          creativity: 0.8
        },
        performance: {
          latency: 100,
          throughput: 10,
          tokensUsed: 100,
          cost: 0.01,
          memoryUsage: 50,
          errorRate: 0
        },
        timestamp: new Date(),
        prompt: 'test',
        output: 'test',
        optimizations: []
      };

      const result2 = { ...result1, modelProvider: ModelProvider.CLAUDE };

      collector.addResult(result1);
      collector.addResult(result2);

      const geminiMetrics = collector.getModelMetrics(ModelProvider.GEMINI);
      const claudeMetrics = collector.getModelMetrics(ModelProvider.CLAUDE);

      expect(geminiMetrics.length).toBe(1);
      expect(claudeMetrics.length).toBe(1);
    });
  });

  describe('Statistics', () => {
    it('should calculate aggregate statistics', () => {
      const results: IterationResult[] = [
        {
          iteration: 1,
          phase: TrainingPhase.BASELINE,
          modelProvider: ModelProvider.GEMINI,
          quality: {
            score: 0.7,
            accuracy: 0.7,
            coherence: 0.7,
            relevance: 0.7,
            diversity: 0.7,
            creativity: 0.7
          },
          performance: {
            latency: 100,
            throughput: 10,
            tokensUsed: 100,
            cost: 0.01,
            memoryUsage: 50,
            errorRate: 0
          },
          timestamp: new Date(),
          prompt: 'test',
          output: 'test',
          optimizations: []
        },
        {
          iteration: 2,
          phase: TrainingPhase.OPTIMIZATION,
          modelProvider: ModelProvider.GEMINI,
          quality: {
            score: 0.9,
            accuracy: 0.9,
            coherence: 0.9,
            relevance: 0.9,
            diversity: 0.9,
            creativity: 0.9
          },
          performance: {
            latency: 120,
            throughput: 8,
            tokensUsed: 120,
            cost: 0.012,
            memoryUsage: 55,
            errorRate: 0
          },
          timestamp: new Date(),
          prompt: 'test',
          output: 'test',
          optimizations: []
        }
      ];

      results.forEach(r => collector.addResult(r));

      const stats = collector.getAggregateStats(ModelProvider.GEMINI);

      expect(stats).toBeDefined();
      expect(stats?.totalIterations).toBe(2);
      expect(stats?.avgQualityScore).toBeCloseTo(0.8, 1);
      expect(stats?.avgLatency).toBeCloseTo(110, 0);
      expect(stats?.totalCost).toBeCloseTo(0.022, 3);
    });

    it('should identify best model', () => {
      const geminiResult: IterationResult = {
        iteration: 1,
        phase: TrainingPhase.BASELINE,
        modelProvider: ModelProvider.GEMINI,
        quality: {
          score: 0.9,
          accuracy: 0.9,
          coherence: 0.9,
          relevance: 0.9,
          diversity: 0.9,
          creativity: 0.9
        },
        performance: {
          latency: 100,
          throughput: 10,
          tokensUsed: 100,
          cost: 0.01,
          memoryUsage: 50,
          errorRate: 0
        },
        timestamp: new Date(),
        prompt: 'test',
        output: 'test',
        optimizations: []
      };

      const claudeResult = {
        ...geminiResult,
        modelProvider: ModelProvider.CLAUDE,
        quality: {
          score: 0.7,
          accuracy: 0.7,
          coherence: 0.7,
          relevance: 0.7,
          diversity: 0.7,
          creativity: 0.7
        }
      };

      collector.addResult(geminiResult);
      collector.addResult(claudeResult);

      const bestModel = collector.getBestModel();
      expect(bestModel).toBe(ModelProvider.GEMINI);
    });
  });

  describe('Report Generation', () => {
    it('should generate comprehensive report', () => {
      const result: IterationResult = {
        iteration: 1,
        phase: TrainingPhase.BASELINE,
        modelProvider: ModelProvider.GEMINI,
        quality: {
          score: 0.8,
          accuracy: 0.8,
          coherence: 0.8,
          relevance: 0.8,
          diversity: 0.8,
          creativity: 0.8
        },
        performance: {
          latency: 100,
          throughput: 10,
          tokensUsed: 100,
          cost: 0.01,
          memoryUsage: 50,
          errorRate: 0
        },
        timestamp: new Date(),
        prompt: 'test',
        output: 'test',
        optimizations: []
      };

      collector.addResult(result);

      const report = collector.generateReport();

      expect(report).toContain('DSPy Training Session Report');
      expect(report).toContain('Best Performing Model');
      expect(report).toContain('Model Comparison');
      expect(report).toContain('gemini');
    });

    it('should generate comparison data', () => {
      const geminiResult: IterationResult = {
        iteration: 1,
        phase: TrainingPhase.BASELINE,
        modelProvider: ModelProvider.GEMINI,
        quality: {
          score: 0.8,
          accuracy: 0.8,
          coherence: 0.8,
          relevance: 0.8,
          diversity: 0.8,
          creativity: 0.8
        },
        performance: {
          latency: 100,
          throughput: 10,
          tokensUsed: 100,
          cost: 0.01,
          memoryUsage: 50,
          errorRate: 0
        },
        timestamp: new Date(),
        prompt: 'test',
        output: 'test',
        optimizations: []
      };

      const claudeResult = { ...geminiResult, modelProvider: ModelProvider.CLAUDE };

      collector.addResult(geminiResult);
      collector.addResult(claudeResult);

      const comparison = collector.getComparison();

      expect(comparison).toBeDefined();
      expect(comparison[ModelProvider.GEMINI]).toBeDefined();
      expect(comparison[ModelProvider.CLAUDE]).toBeDefined();
    });
  });
});

describe('Quality Metrics Calculation', () => {
  it('should calculate quality scores correctly', async () => {
    const config: ModelConfig = {
      provider: ModelProvider.GEMINI,
      model: 'gemini-2.0-flash-exp',
      apiKey: 'test-key'
    };
    const agent = new GeminiAgent(config);

    const signature: DSPySignature = {
      input: 'test input with keywords',
      output: 'test output',
      constraints: ['min_length:10']
    };

    const result = await agent.execute('test prompt', signature);

    expect(result.quality.score).toBeGreaterThanOrEqual(0);
    expect(result.quality.score).toBeLessThanOrEqual(1);
    expect(result.quality.accuracy).toBeGreaterThanOrEqual(0);
    expect(result.quality.coherence).toBeGreaterThanOrEqual(0);
    expect(result.quality.relevance).toBeGreaterThanOrEqual(0);
    expect(result.quality.diversity).toBeGreaterThanOrEqual(0);
    expect(result.quality.creativity).toBeGreaterThanOrEqual(0);
  });
});

describe('Performance Metrics Calculation', () => {
  it('should track latency correctly', async () => {
    const config: ModelConfig = {
      provider: ModelProvider.GEMINI,
      model: 'gemini-2.0-flash-exp',
      apiKey: 'test-key'
    };
    const agent = new GeminiAgent(config);

    const signature: DSPySignature = {
      input: 'test',
      output: 'test'
    };

    const result = await agent.execute('test', signature);

    expect(result.performance.latency).toBeGreaterThan(0);
    expect(result.performance.throughput).toBeGreaterThan(0);
  });

  it('should calculate cost correctly', async () => {
    const config: ModelConfig = {
      provider: ModelProvider.GEMINI,
      model: 'gemini-2.0-flash-exp',
      apiKey: 'test-key'
    };
    const agent = new GeminiAgent(config);

    const signature: DSPySignature = {
      input: 'test',
      output: 'test'
    };

    const result = await agent.execute('test prompt', signature);

    expect(result.performance.cost).toBeGreaterThan(0);
    expect(result.performance.tokensUsed).toBeGreaterThan(0);
  });
});

describe('Integration Tests', () => {
  it('should complete full training pipeline', async () => {
    const config = {
      models: [
        {
          provider: ModelProvider.GEMINI,
          model: 'gemini-2.0-flash-exp',
          apiKey: 'test-key'
        }
      ],
      optimizationRounds: 1,
      baselineIterations: 1,
      benchmarkSamples: 2,
      enableCrossLearning: false,
      enableHooksIntegration: false
    };

    const session = new DSPyTrainingSession(config);

    const phases: TrainingPhase[] = [];
    session.on('phase', (phase) => phases.push(phase));

    await new Promise<void>((resolve) => {
      session.on('complete', () => {
        expect(phases.length).toBeGreaterThan(0);
        resolve();
      });

      const optimizer = new OptimizationEngine();
      const signature = optimizer.createSignature('test', 'input', 'output');

      session.run('test prompt', signature);
    });
  }, 10000);
});
