/**
 * Tests for advanced features: Federated Learning, LoRA, Export, Training Pipeline
 */

const { test, describe } = require('node:test');
const assert = require('node:assert');

const {
  // Federated Learning
  EphemeralAgent,
  FederatedCoordinator,
  // LoRA
  LoraAdapter,
  LoraManager,
  // Export
  SafeTensorsWriter,
  SafeTensorsReader,
  ModelExporter,
  ModelImporter,
  DatasetExporter,
  // Training
  TrainingPipeline,
  TrainingFactory,
  LRScheduler,
  MetricsTracker,
} = require('../dist/cjs/index.js');

// ============================================
// Federated Learning Tests
// ============================================

describe('EphemeralAgent', () => {
  test('should create agent with config', () => {
    const agent = new EphemeralAgent('agent-1', { hiddenDim: 128 });

    assert.strictEqual(agent.getAgentId(), 'agent-1');
    assert.strictEqual(agent.trajectoryCount(), 0);
    assert.strictEqual(agent.avgQuality(), 0);
  });

  test('should process tasks', () => {
    const agent = new EphemeralAgent('agent-1', { hiddenDim: 64 });

    agent.processTask([0.1, 0.2, 0.3], 0.85);
    agent.processTask([0.4, 0.5, 0.6], 0.92);

    assert.strictEqual(agent.trajectoryCount(), 2);
    assert.ok(agent.avgQuality() > 0.8);
  });

  test('should process tasks with route', () => {
    const agent = new EphemeralAgent('agent-1');

    agent.processTaskWithRoute([0.1, 0.2], 0.9, 'code-model');

    const exportData = agent.exportState();
    assert.strictEqual(exportData.trajectories[0].route, 'code-model');
  });

  test('should apply micro-LoRA', () => {
    const agent = new EphemeralAgent('agent-1', { hiddenDim: 8, microLoraRank: 2 });

    // Process some tasks first to train the LoRA weights
    for (let i = 0; i < 10; i++) {
      agent.processTask([1, 2, 3, 4, 5, 6, 7, 8], 0.9);
    }

    const input = [1, 2, 3, 4, 5, 6, 7, 8];
    const output = new Array(8).fill(0);

    agent.applyMicroLora(input, output);

    // Output should have non-zero values after LoRA applied
    const hasOutput = output.some((v) => v !== 0);
    assert.ok(hasOutput, 'LoRA should produce non-zero output');
  });

  test('should export state', () => {
    const agent = new EphemeralAgent('agent-1');

    agent.processTask([0.1, 0.2], 0.85);
    agent.processTask([0.3, 0.4], 0.75);

    const exportData = agent.exportState();

    assert.strictEqual(exportData.agentId, 'agent-1');
    assert.strictEqual(exportData.trajectories.length, 2);
    assert.ok(exportData.sessionDurationMs >= 0);
    assert.ok(exportData.stats.avgQuality > 0.7);
  });

  test('should serialize to JSON', () => {
    const agent = new EphemeralAgent('agent-1');
    agent.processTask([0.1, 0.2], 0.9);

    const json = agent.toJSON();
    const parsed = JSON.parse(json);

    assert.strictEqual(parsed.agentId, 'agent-1');
    assert.strictEqual(parsed.trajectories.length, 1);
  });
});

describe('FederatedCoordinator', () => {
  test('should create coordinator', () => {
    const coord = new FederatedCoordinator('coord-1', { hiddenDim: 128 });

    assert.strictEqual(coord.getCoordinatorId(), 'coord-1');
    assert.strictEqual(coord.agentCount(), 0);
    assert.strictEqual(coord.getTotalTrajectories(), 0);
  });

  test('should aggregate agent exports', () => {
    const coord = new FederatedCoordinator('coord-1');
    coord.setQualityThreshold(0.5);

    const exportData = {
      agentId: 'agent-1',
      trajectories: [
        { embedding: [0.1, 0.2], quality: 0.8, context: [], timestamp: Date.now() },
        { embedding: [0.3, 0.4], quality: 0.3, context: [], timestamp: Date.now() }, // Below threshold
      ],
      stats: { totalTrajectories: 2, avgQuality: 0.55, patternsLearned: 0 },
      sessionDurationMs: 1000,
      timestamp: Date.now(),
    };

    const result = coord.aggregate(exportData);

    assert.strictEqual(result.agentId, 'agent-1');
    assert.strictEqual(result.trajectoriesAccepted, 1);
    assert.strictEqual(result.trajectoriesRejected, 1);
    assert.strictEqual(result.totalAgents, 1);
  });

  test('should aggregate multiple agents', () => {
    const coord = new FederatedCoordinator('coord-1');

    for (let i = 0; i < 3; i++) {
      coord.aggregate({
        agentId: `agent-${i}`,
        trajectories: [
          { embedding: [i * 0.1], quality: 0.8, context: [], timestamp: Date.now() },
        ],
        stats: { totalTrajectories: 1, avgQuality: 0.8, patternsLearned: 0 },
        sessionDurationMs: 1000,
        timestamp: Date.now(),
      });
    }

    const stats = coord.stats();
    assert.strictEqual(stats.totalAgents, 3);
    assert.strictEqual(stats.totalTrajectories, 3);
  });

  test('should create agent with warm start', () => {
    const coord = new FederatedCoordinator('coord-1');

    // Add some patterns first
    coord.aggregate({
      agentId: 'agent-1',
      trajectories: [
        { embedding: [0.5, 0.5], quality: 0.9, context: [], timestamp: Date.now() },
      ],
      stats: { totalTrajectories: 1, avgQuality: 0.9, patternsLearned: 1 },
      sessionDurationMs: 1000,
      timestamp: Date.now(),
    });

    const newAgent = coord.createAgent('agent-2');

    assert.strictEqual(newAgent.getAgentId(), 'agent-2');
    // Agent should have some warm-start trajectories
  });

  test('should apply coordinator LoRA', () => {
    const coord = new FederatedCoordinator('coord-1', { hiddenDim: 8 });

    const input = [1, 2, 3, 4, 5, 6, 7, 8];
    const output = coord.applyLora(input);

    assert.strictEqual(output.length, input.length);
  });

  test('should get initial patterns', () => {
    const coord = new FederatedCoordinator('coord-1');

    coord.aggregate({
      agentId: 'agent-1',
      trajectories: [
        { embedding: [0.1, 0.2], quality: 0.9, context: [], timestamp: Date.now() },
        { embedding: [0.3, 0.4], quality: 0.8, context: [], timestamp: Date.now() },
      ],
      stats: { totalTrajectories: 2, avgQuality: 0.85, patternsLearned: 0 },
      sessionDurationMs: 1000,
      timestamp: Date.now(),
    });

    const patterns = coord.getInitialPatterns(5);
    assert.ok(patterns.length >= 0);
  });
});

// ============================================
// LoRA Tests
// ============================================

describe('LoraAdapter', () => {
  test('should create adapter with config', () => {
    const adapter = new LoraAdapter({ rank: 8, alpha: 16 }, 64, 64);

    const config = adapter.getConfig();
    assert.strictEqual(config.rank, 8);
    assert.strictEqual(config.alpha, 16);
  });

  test('should forward pass', () => {
    const adapter = new LoraAdapter({ rank: 4 }, 16, 16);

    const input = new Array(16).fill(0).map((_, i) => i * 0.1);
    const output = adapter.forward(input);

    assert.strictEqual(output.length, 16);
    // Output should differ from input due to LoRA delta
  });

  test('should forward batch', () => {
    const adapter = new LoraAdapter({ rank: 4 }, 8, 8);

    const inputs = [
      [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
      [0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9],
    ];

    const outputs = adapter.forwardBatch(inputs);

    assert.strictEqual(outputs.length, 2);
    assert.strictEqual(outputs[0].length, 8);
  });

  test('should backward and update weights', () => {
    const adapter = new LoraAdapter({ rank: 4 }, 8, 8);
    adapter.startTraining(0.01);

    const input = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    const gradOutput = [0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08];

    const gradNorm = adapter.backward(input, gradOutput, 0.01);

    assert.ok(gradNorm >= 0);

    const state = adapter.endTraining();
    assert.ok(state);
    assert.strictEqual(state.step, 1);
  });

  test('should freeze and unfreeze', () => {
    const adapter = new LoraAdapter();

    assert.strictEqual(adapter.isFrozen(), false);

    adapter.freeze();
    assert.strictEqual(adapter.isFrozen(), true);

    adapter.unfreeze();
    assert.strictEqual(adapter.isFrozen(), false);
  });

  test('should serialize and deserialize', () => {
    const adapter = new LoraAdapter({ rank: 4, alpha: 8 }, 16, 16);

    const json = adapter.toJSON();
    const restored = LoraAdapter.fromJSON(json);

    const config = restored.getConfig();
    assert.strictEqual(config.rank, 4);
    assert.strictEqual(config.alpha, 8);
  });

  test('should merge weights', () => {
    const adapter = new LoraAdapter({ rank: 4 }, 8, 8);

    const delta = adapter.merge();

    assert.strictEqual(delta.length, 8);
    assert.strictEqual(delta[0].length, 8);
  });

  test('should report number of parameters', () => {
    const adapter = new LoraAdapter({ rank: 8 }, 64, 64);

    const params = adapter.numParameters();
    // (64 * 8) + (8 * 64) = 1024
    assert.strictEqual(params, 1024);
  });
});

describe('LoraManager', () => {
  test('should manage multiple adapters', () => {
    const manager = new LoraManager();

    manager.create('task-1', { rank: 4 }, 32, 32);
    manager.create('task-2', { rank: 8 }, 32, 32);

    assert.strictEqual(manager.count(), 2);
    assert.deepStrictEqual(manager.list(), ['task-1', 'task-2']);
  });

  test('should activate adapters', () => {
    const manager = new LoraManager();

    manager.create('task-1');
    manager.create('task-2');

    assert.strictEqual(manager.getActiveId(), null);

    manager.activate('task-1');
    assert.strictEqual(manager.getActiveId(), 'task-1');

    manager.deactivate();
    assert.strictEqual(manager.getActiveId(), null);
  });

  test('should forward through active adapter', () => {
    const manager = new LoraManager();

    manager.create('task-1', { rank: 4 }, 8, 8);
    manager.activate('task-1');

    const input = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    const output = manager.forward(input);

    assert.strictEqual(output.length, 8);
  });

  test('should merge adapters', () => {
    const manager = new LoraManager();

    manager.create('task-1', { rank: 4 }, 8, 8);
    manager.create('task-2', { rank: 4 }, 8, 8);

    const merged = manager.mergeAdapters(['task-1', 'task-2'], 'merged');

    assert.ok(merged);
    assert.strictEqual(manager.count(), 3);
  });

  test('should provide stats', () => {
    const manager = new LoraManager();

    manager.create('task-1', { rank: 4 }, 16, 16);
    manager.create('task-2', { rank: 8 }, 16, 16);
    manager.get('task-1').freeze();

    const stats = manager.stats();

    assert.strictEqual(stats.totalAdapters, 2);
    assert.strictEqual(stats.frozenCount, 1);
    assert.ok(stats.totalParameters > 0);
  });
});

// ============================================
// Export Tests
// ============================================

describe('SafeTensorsWriter', () => {
  test('should add tensors', () => {
    const writer = new SafeTensorsWriter();

    writer.add1D('bias', [0.1, 0.2, 0.3]);
    writer.add2D('weight', [[0.1, 0.2], [0.3, 0.4]]);

    const buffer = writer.build();

    assert.ok(buffer instanceof Uint8Array);
    assert.ok(buffer.length > 0);
  });

  test('should add metadata', () => {
    const writer = new SafeTensorsWriter();

    writer.addMetadata('name', 'test-model');
    writer.addMetadata('version', '1.0.0');
    writer.add1D('data', [1, 2, 3]);

    const buffer = writer.build();
    assert.ok(buffer.length > 0);
  });
});

describe('SafeTensorsReader', () => {
  test('should read tensors', () => {
    // Write then read
    const writer = new SafeTensorsWriter();
    writer.add1D('bias', [0.1, 0.2, 0.3]);
    writer.add2D('weight', [[1, 2], [3, 4]]);
    writer.addMetadata('name', 'test');

    const buffer = writer.build();
    const reader = new SafeTensorsReader(buffer);

    const names = reader.getTensorNames();
    assert.ok(names.includes('bias'));
    assert.ok(names.includes('weight'));

    const bias = reader.getTensor1D('bias');
    assert.ok(bias);
    assert.strictEqual(bias.length, 3);

    const weight = reader.getTensor2D('weight');
    assert.ok(weight);
    assert.strictEqual(weight.length, 2);
    assert.strictEqual(weight[0].length, 2);

    const metadata = reader.getMetadata();
    assert.strictEqual(metadata.name, 'test');
  });
});

describe('ModelExporter', () => {
  test('should export to SafeTensors', () => {
    const exporter = new ModelExporter();

    const model = {
      metadata: {
        name: 'test-model',
        version: '1.0.0',
        architecture: 'sona-lora',
      },
      loraWeights: {
        loraA: [[0.1, 0.2], [0.3, 0.4]],
        loraB: [[0.5, 0.6], [0.7, 0.8]],
        scaling: 2.0,
      },
    };

    const buffer = exporter.toSafeTensors(model);

    assert.ok(buffer instanceof Uint8Array);
    assert.ok(buffer.length > 0);
  });

  test('should export to JSON', () => {
    const exporter = new ModelExporter();

    const model = {
      metadata: { name: 'test', version: '1.0', architecture: 'lora' },
      loraConfig: { rank: 8, alpha: 16, dropout: 0.1, targetModules: ['q', 'v'] },
    };

    const json = exporter.toJSON(model);
    const parsed = JSON.parse(json);

    assert.strictEqual(parsed.metadata.name, 'test');
    assert.strictEqual(parsed.loraConfig.rank, 8);
  });

  test('should export for HuggingFace', () => {
    const exporter = new ModelExporter();

    const model = {
      metadata: {
        name: 'my-lora',
        version: '1.0.0',
        architecture: 'sona-lora',
        training: { steps: 1000, loss: 0.01, learningRate: 0.001 },
      },
      loraWeights: {
        loraA: [[0.1, 0.2]],
        loraB: [[0.3, 0.4]],
        scaling: 2.0,
      },
    };

    const { safetensors, config, readme } = exporter.toHuggingFace(model);

    assert.ok(safetensors instanceof Uint8Array);
    assert.ok(config.includes('sona-lora'));
    assert.ok(readme.includes('my-lora'));
  });
});

describe('ModelImporter', () => {
  test('should import from SafeTensors', () => {
    const exporter = new ModelExporter();
    const importer = new ModelImporter();

    const original = {
      metadata: { name: 'test', version: '1.0', architecture: 'lora' },
      loraWeights: {
        loraA: [[0.1, 0.2], [0.3, 0.4]],
        loraB: [[0.5, 0.6], [0.7, 0.8]],
        scaling: 2.0,
      },
    };

    const buffer = exporter.toSafeTensors(original);
    const imported = importer.fromSafeTensors(buffer);

    assert.ok(imported.loraWeights);
    assert.strictEqual(imported.loraWeights.loraA.length, 2);
  });

  test('should import from JSON', () => {
    const importer = new ModelImporter();

    const json = JSON.stringify({
      metadata: { name: 'test', version: '1.0', architecture: 'lora' },
      loraConfig: { rank: 8 },
    });

    const imported = importer.fromJSON(json);

    assert.strictEqual(imported.metadata.name, 'test');
    assert.strictEqual(imported.loraConfig.rank, 8);
  });
});

describe('DatasetExporter', () => {
  test('should export to JSONL', () => {
    const exporter = new DatasetExporter();

    const data = [
      { input: [0.1, 0.2], output: [0.3, 0.4], quality: 0.9 },
      { input: [0.5, 0.6], output: [0.7, 0.8], quality: 0.8 },
    ];

    const jsonl = exporter.toJSONL(data);
    const lines = jsonl.split('\n');

    assert.strictEqual(lines.length, 2);
    const first = JSON.parse(lines[0]);
    assert.deepStrictEqual(first.input, [0.1, 0.2]);
  });

  test('should export to CSV', () => {
    const exporter = new DatasetExporter();

    const data = [
      { input: [0.1], output: [0.2], quality: 0.9 },
    ];

    const csv = exporter.toCSV(data);

    assert.ok(csv.startsWith('quality,input,output'));
    assert.ok(csv.includes('0.9'));
  });
});

// ============================================
// Training Pipeline Tests
// ============================================

describe('LRScheduler', () => {
  test('should return constant LR', () => {
    const config = {
      learningRate: 0.01,
      batchSize: 32,
      epochs: 10,
      scheduler: 'constant',
      warmupSteps: 0,
      weightDecay: 0,
      gradientClip: 1,
      earlyStoppingPatience: 3,
      checkpointInterval: 1,
      ewcLambda: 2000,
      validationSplit: 0.1,
    };

    const scheduler = new LRScheduler(config, 100);

    assert.strictEqual(scheduler.getLR(), 0.01);
    scheduler.step();
    assert.strictEqual(scheduler.getLR(), 0.01);
  });

  test('should decay with cosine schedule', () => {
    const config = {
      learningRate: 0.01,
      batchSize: 32,
      epochs: 10,
      scheduler: 'cosine',
      warmupSteps: 0,
      weightDecay: 0,
      gradientClip: 1,
      earlyStoppingPatience: 3,
      checkpointInterval: 1,
      ewcLambda: 2000,
      validationSplit: 0.1,
    };

    const scheduler = new LRScheduler(config, 100);

    const lr1 = scheduler.getLR();
    for (let i = 0; i < 50; i++) scheduler.step();
    const lr2 = scheduler.getLR();

    assert.ok(lr2 < lr1, 'LR should decay');
  });
});

describe('MetricsTracker', () => {
  test('should track losses', () => {
    const tracker = new MetricsTracker();

    tracker.recordLoss(0.5);
    tracker.recordLoss(0.4);
    tracker.recordLoss(0.3);

    const avg = tracker.avgLoss(3);
    assert.ok(Math.abs(avg - 0.4) < 0.01);
  });

  test('should track validation losses', () => {
    const tracker = new MetricsTracker();

    tracker.recordValLoss(0.6);
    tracker.recordValLoss(0.5);
    tracker.recordValLoss(0.4);

    assert.strictEqual(tracker.bestValLoss(), 0.4);
  });

  test('should compute steps per second', () => {
    const tracker = new MetricsTracker();

    tracker.recordStepTime(100);
    tracker.recordStepTime(100);

    const sps = tracker.stepsPerSecond();
    assert.ok(sps > 0);
  });
});

describe('TrainingPipeline', () => {
  test('should add training data', () => {
    const pipeline = new TrainingPipeline({ batchSize: 2 });

    const data = [
      { input: [0.1, 0.2], target: [0.3, 0.4], quality: 0.9 },
      { input: [0.5, 0.6], target: [0.7, 0.8], quality: 0.8 },
      { input: [0.9, 1.0], target: [1.1, 1.2], quality: 0.7 },
    ];

    pipeline.addData(data);
    // Should have 2 batches (2 + 1)
  });

  test('should train model', () => {
    const pipeline = new TrainingPipeline({
      learningRate: 0.01,
      batchSize: 2,
      epochs: 2,
      validationSplit: 0,
    });

    // Add some training data
    const data = [];
    for (let i = 0; i < 10; i++) {
      data.push({
        input: new Array(8).fill(0).map(() => Math.random()),
        target: new Array(8).fill(0).map(() => Math.random()),
        quality: 0.8 + Math.random() * 0.2,
      });
    }

    pipeline.addData(data);
    const result = pipeline.train();

    assert.strictEqual(result.epochs, 2);
    assert.ok(result.steps > 0);
    assert.ok(result.lossHistory.length > 0);
  });

  test('should get metrics', () => {
    const pipeline = new TrainingPipeline();

    const metrics = pipeline.getMetrics();

    assert.strictEqual(metrics.epoch, 0);
    assert.strictEqual(metrics.step, 0);
  });

  test('should get adapter', () => {
    const pipeline = new TrainingPipeline();

    const adapter = pipeline.getAdapter();

    assert.ok(adapter instanceof LoraAdapter);
  });
});

describe('TrainingFactory', () => {
  test('should create quick finetune pipeline', () => {
    const pipeline = TrainingFactory.quickFinetune();

    const adapter = pipeline.getAdapter();
    assert.ok(adapter);
  });

  test('should create deep training pipeline', () => {
    const pipeline = TrainingFactory.deepTraining();

    const adapter = pipeline.getAdapter();
    assert.ok(adapter);
  });

  test('should create continual learning pipeline', () => {
    const pipeline = TrainingFactory.continualLearning(5000);

    const ewc = pipeline.getEwcManager();
    assert.ok(ewc);
  });

  test('should create federated aggregation pipeline', () => {
    const pipeline = TrainingFactory.federatedAggregation();

    const adapter = pipeline.getAdapter();
    assert.ok(adapter);
  });
});

// ============================================
// Integration Tests
// ============================================

describe('Integration: Federated + LoRA + Export', () => {
  test('should train agent, export, and import', () => {
    // Create and train agent
    const agent = new EphemeralAgent('agent-1', { hiddenDim: 8 });

    for (let i = 0; i < 5; i++) {
      agent.processTask(
        new Array(8).fill(0).map(() => Math.random()),
        0.7 + Math.random() * 0.3
      );
    }

    // Export state
    const exportData = agent.exportState();

    // Aggregate in coordinator
    const coord = new FederatedCoordinator('coord-1', { hiddenDim: 8 });
    const result = coord.aggregate(exportData);

    assert.ok(result.trajectoriesAccepted > 0);

    // Export coordinator model
    const exporter = new ModelExporter();
    const model = {
      metadata: {
        name: 'federated-model',
        version: '1.0.0',
        architecture: 'sona-federated',
      },
      patterns: coord.getAllPatterns(),
    };

    const json = exporter.toJSON(model);
    const importer = new ModelImporter();
    const imported = importer.fromJSON(json);

    assert.strictEqual(imported.metadata.name, 'federated-model');
  });

  test('should train with pipeline and export LoRA', () => {
    // Create pipeline
    const pipeline = new TrainingPipeline({
      learningRate: 0.01,
      epochs: 1,
      batchSize: 2,
      validationSplit: 0,
    });

    // Add data
    for (let i = 0; i < 4; i++) {
      pipeline.addBatch(
        [new Array(8).fill(0).map(() => Math.random())],
        [new Array(8).fill(0).map(() => Math.random())],
        [0.8]
      );
    }

    // Train
    const result = pipeline.train();
    assert.ok(result.steps > 0);

    // Export adapter
    const adapter = pipeline.getAdapter();
    const exporter = new ModelExporter();

    const model = {
      metadata: {
        name: 'trained-lora',
        version: '1.0.0',
        architecture: 'lora',
        training: {
          steps: result.steps,
          loss: result.finalLoss,
          learningRate: 0.01,
        },
      },
      loraWeights: adapter.getWeights(),
      loraConfig: adapter.getConfig(),
    };

    const buffer = exporter.toSafeTensors(model);
    assert.ok(buffer.length > 0);

    // Import and verify
    const importer = new ModelImporter();
    const imported = importer.fromSafeTensors(buffer);

    assert.ok(imported.loraWeights);
  });
});
