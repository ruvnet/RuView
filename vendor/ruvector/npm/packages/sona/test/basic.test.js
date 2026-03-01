/**
 * Basic NAPI tests for SONA
 */

const test = require('node:test');
const assert = require('node:assert');
const { SonaEngine } = require('../index.js');

test('SonaEngine creation', () => {
  const engine = new SonaEngine(128);
  assert.ok(engine, 'Engine should be created');
  assert.strictEqual(engine.isEnabled(), true, 'Engine should be enabled by default');
});

test('SonaEngine with custom config', () => {
  const engine = SonaEngine.withConfig({
    hiddenDim: 256,
    microLoraRank: 2,
    baseLoraRank: 8,
  });
  assert.ok(engine, 'Engine should be created with custom config');
});

test('Trajectory recording', () => {
  const engine = new SonaEngine(64);
  const queryEmbedding = Array(64).fill(0.1);

  const builder = engine.beginTrajectory(queryEmbedding);
  assert.ok(builder, 'TrajectoryBuilder should be created');

  builder.addStep(Array(64).fill(0.5), Array(32).fill(0.4), 0.8);
  builder.setRoute('test_route');
  builder.addContext('test_context');

  engine.endTrajectory(builder, 0.85);
});

test('Micro-LoRA application', () => {
  const engine = new SonaEngine(64);
  const input = Array(64).fill(1.0);

  const output = engine.applyMicroLora(input);
  assert.ok(Array.isArray(output), 'Output should be an array');
  assert.strictEqual(output.length, 64, 'Output should have same dimension as input');
});

test('Base-LoRA application', () => {
  const engine = new SonaEngine(64);
  const input = Array(64).fill(1.0);

  const output = engine.applyBaseLora(0, input);
  assert.ok(Array.isArray(output), 'Output should be an array');
  assert.strictEqual(output.length, 64, 'Output should have same dimension as input');
});

test('Pattern finding', () => {
  const engine = new SonaEngine(64);

  // Record some trajectories first
  for (let i = 0; i < 10; i++) {
    const builder = engine.beginTrajectory(Array(64).fill(Math.random()));
    builder.addStep(Array(64).fill(0.5), Array(32).fill(0.4), 0.8);
    engine.endTrajectory(builder, 0.8);
  }

  // Force learning to extract patterns
  engine.forceLearn();

  // Find patterns
  const patterns = engine.findPatterns(Array(64).fill(0.5), 5);
  assert.ok(Array.isArray(patterns), 'Patterns should be an array');
});

test('Enable/disable engine', () => {
  const engine = new SonaEngine(64);

  assert.strictEqual(engine.isEnabled(), true);
  engine.setEnabled(false);
  assert.strictEqual(engine.isEnabled(), false);
  engine.setEnabled(true);
  assert.strictEqual(engine.isEnabled(), true);
});

test('Force learning', () => {
  const engine = new SonaEngine(64);

  // Record trajectories
  for (let i = 0; i < 5; i++) {
    const builder = engine.beginTrajectory(Array(64).fill(Math.random()));
    builder.addStep(Array(64).fill(0.5), Array(32).fill(0.4), 0.8);
    engine.endTrajectory(builder, 0.8);
  }

  const result = engine.forceLearn();
  assert.ok(typeof result === 'string', 'Result should be a string');
  assert.ok(result.length > 0, 'Result should not be empty');
});

test('Get statistics', () => {
  const engine = new SonaEngine(64);

  const stats = engine.getStats();
  assert.ok(typeof stats === 'string', 'Stats should be a string');
  assert.ok(stats.length > 0, 'Stats should not be empty');
});

test('Flush instant updates', () => {
  const engine = new SonaEngine(64);

  // Should not throw
  assert.doesNotThrow(() => {
    engine.flush();
  });
});

test('Tick background learning', () => {
  const engine = new SonaEngine(64);

  // May or may not return a message depending on timing
  const result = engine.tick();
  assert.ok(result === null || typeof result === 'string');
});
