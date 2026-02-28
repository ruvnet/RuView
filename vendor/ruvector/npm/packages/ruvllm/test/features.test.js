/**
 * Tests for new features: Sessions, Streaming, SONA
 */

const { test, describe } = require('node:test');
const assert = require('node:assert');

const {
  RuvLLM,
  SessionManager,
  StreamingGenerator,
  SonaCoordinator,
  TrajectoryBuilder,
  ReasoningBank,
  EwcManager,
} = require('../dist/cjs/index.js');

describe('SessionManager', () => {
  test('should create session', () => {
    const llm = new RuvLLM();
    const sessions = new SessionManager(llm);

    const session = sessions.create({ userId: 'test' });

    assert.ok(session.id.startsWith('session-'));
    assert.strictEqual(session.messageCount, 0);
    assert.deepStrictEqual(session.metadata, { userId: 'test' });
  });

  test('should chat with context', () => {
    const llm = new RuvLLM();
    const sessions = new SessionManager(llm);

    const session = sessions.create();
    const response1 = sessions.chat(session.id, 'Hello');
    const response2 = sessions.chat(session.id, 'How are you?');

    assert.strictEqual(session.messages.length, 4); // 2 user + 2 assistant
    assert.ok(response1.text);
    assert.ok(response2.text);
  });

  test('should get history', () => {
    const llm = new RuvLLM();
    const sessions = new SessionManager(llm);

    const session = sessions.create();
    sessions.chat(session.id, 'Message 1');
    sessions.chat(session.id, 'Message 2');

    const history = sessions.getHistory(session.id);
    assert.strictEqual(history.length, 4);

    const limited = sessions.getHistory(session.id, 2);
    assert.strictEqual(limited.length, 2);
  });

  test('should export and import session', () => {
    const llm = new RuvLLM();
    const sessions = new SessionManager(llm);

    const session = sessions.create({ key: 'value' });
    sessions.chat(session.id, 'Test message');

    const exported = sessions.export(session.id);
    assert.ok(exported);

    const imported = sessions.import(exported);
    assert.strictEqual(imported.id, session.id);
    assert.strictEqual(imported.messages.length, 2);
  });

  test('should end session', () => {
    const llm = new RuvLLM();
    const sessions = new SessionManager(llm);

    const session = sessions.create();
    assert.ok(sessions.get(session.id));

    sessions.end(session.id);
    assert.strictEqual(sessions.get(session.id), undefined);
  });
});

describe('StreamingGenerator', () => {
  test('should stream response', async () => {
    const llm = new RuvLLM();
    const streamer = new StreamingGenerator(llm);

    const chunks = [];
    for await (const chunk of streamer.stream('Test prompt')) {
      chunks.push(chunk);
    }

    assert.ok(chunks.length > 0);
    assert.ok(chunks[chunks.length - 1].done);
  });

  test('should collect stream', async () => {
    const llm = new RuvLLM();
    const streamer = new StreamingGenerator(llm);

    const result = await streamer.collect('Test prompt');
    assert.ok(typeof result === 'string');
  });

  test('should use callbacks', async () => {
    const llm = new RuvLLM();
    const streamer = new StreamingGenerator(llm);

    let chunkCount = 0;
    let completed = false;

    await streamer.streamWithCallbacks('Test', {
      onChunk: () => chunkCount++,
      onComplete: () => { completed = true; },
    });

    assert.ok(chunkCount > 0);
    assert.ok(completed);
  });
});

describe('TrajectoryBuilder', () => {
  test('should build trajectory', () => {
    const builder = new TrajectoryBuilder();

    const trajectory = builder
      .startStep('query', 'What is AI?')
      .endStep('AI is...', 0.95)
      .startStep('memory', 'searching')
      .endStep('found 3 results', 0.88)
      .complete('success');

    assert.ok(trajectory.id.startsWith('traj-'));
    assert.strictEqual(trajectory.steps.length, 2);
    assert.strictEqual(trajectory.outcome, 'success');
    assert.ok(trajectory.durationMs >= 0);
  });

  test('should track step durations', () => {
    const builder = new TrajectoryBuilder();

    builder.startStep('query', 'input');
    // Small delay
    const start = Date.now();
    while (Date.now() - start < 5) { /* wait */ }
    builder.endStep('output', 0.9);

    const trajectory = builder.complete('success');
    assert.ok(trajectory.steps[0].durationMs >= 0);
  });
});

describe('ReasoningBank', () => {
  test('should store and retrieve patterns', () => {
    const bank = new ReasoningBank(0.5); // Lower threshold for testing

    const embedding = [0.1, 0.2, 0.3, 0.4, 0.5];
    const id = bank.store('query_response', embedding);

    assert.ok(id.startsWith('pat-'));

    const pattern = bank.get(id);
    assert.ok(pattern);
    assert.strictEqual(pattern.type, 'query_response');
    assert.strictEqual(pattern.successRate, 1.0);
  });

  test('should find similar patterns', () => {
    const bank = new ReasoningBank(0.5);

    const emb1 = [1, 0, 0, 0, 0];
    const emb2 = [0.9, 0.1, 0, 0, 0]; // Similar to emb1

    bank.store('query_response', emb1);
    bank.store('routing', emb2);

    const similar = bank.findSimilar([1, 0, 0, 0, 0], 5);
    assert.ok(similar.length >= 1);
  });

  test('should track usage', () => {
    const bank = new ReasoningBank();

    const embedding = [0.1, 0.2, 0.3];
    const id = bank.store('query_response', embedding);

    bank.recordUsage(id, true);
    bank.recordUsage(id, true);
    bank.recordUsage(id, false);

    const pattern = bank.get(id);
    assert.strictEqual(pattern.useCount, 3);
    assert.ok(pattern.successRate < 1.0);
  });

  test('should provide stats', () => {
    const bank = new ReasoningBank();

    bank.store('query_response', [0.1, 0.2]);
    bank.store('routing', [0.3, 0.4]);

    const stats = bank.stats();
    assert.strictEqual(stats.totalPatterns, 2);
    assert.strictEqual(stats.byType['query_response'], 1);
    assert.strictEqual(stats.byType['routing'], 1);
  });
});

describe('EwcManager', () => {
  test('should register tasks', () => {
    const ewc = new EwcManager(1000);

    ewc.registerTask('task1', [0.1, 0.2, 0.3]);
    ewc.registerTask('task2', [0.4, 0.5, 0.6]);

    const stats = ewc.stats();
    assert.strictEqual(stats.tasksLearned, 2);
    assert.strictEqual(stats.fisherComputed, true);
  });

  test('should compute penalty', () => {
    const ewc = new EwcManager(1000);

    ewc.registerTask('task1', [0.5, 0.5, 0.5]);

    // Weights that differ from optimal should have higher penalty
    const penalty1 = ewc.computePenalty([0.5, 0.5, 0.5]);
    const penalty2 = ewc.computePenalty([1.0, 1.0, 1.0]);

    assert.ok(penalty2 > penalty1);
  });
});

describe('SonaCoordinator', () => {
  test('should create with config', () => {
    const sona = new SonaCoordinator({
      instantLoopEnabled: true,
      ewcLambda: 5000,
    });

    assert.ok(sona);
    const stats = sona.stats();
    assert.ok(stats.patterns);
    assert.ok(stats.ewc);
  });

  test('should record signals', () => {
    const sona = new SonaCoordinator();

    sona.recordSignal({
      requestId: 'req-123',
      quality: 0.9,
      type: 'positive',
      timestamp: new Date(),
    });

    const stats = sona.stats();
    assert.strictEqual(stats.signalsReceived, 1);
  });

  test('should record trajectories', () => {
    const sona = new SonaCoordinator();

    const builder = new TrajectoryBuilder();
    const trajectory = builder
      .startStep('query', 'test')
      .endStep('response', 0.95)
      .complete('success');

    sona.recordTrajectory(trajectory);

    const stats = sona.stats();
    assert.strictEqual(stats.trajectoriesBuffered, 1);
  });

  test('should run background loop', () => {
    const sona = new SonaCoordinator();

    // Add some trajectories
    for (let i = 0; i < 3; i++) {
      const builder = new TrajectoryBuilder();
      const trajectory = builder
        .startStep('query', `test ${i}`)
        .endStep(`response ${i}`, 0.95)
        .complete('success');
      sona.recordTrajectory(trajectory);
    }

    const result = sona.runBackgroundLoop();
    assert.strictEqual(result.trajectoriesProcessed, 3);
  });
});
