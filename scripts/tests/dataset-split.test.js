'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { splitBySession, validatePoseDataset } = require('../dataset-split.js');

test('holds out whole sessions without adjacent-window leakage', () => {
  const samples = [
    { id: 'a1', sessionId: 'session-a', timestamp: 1 },
    { id: 'a2', sessionId: 'session-a', timestamp: 2 },
    { id: 'b1', sessionId: 'session-b', timestamp: 3 },
    { id: 'b2', sessionId: 'session-b', timestamp: 4 },
  ];

  const { trainSet, evalSet, strategy } = splitBySession(samples, 0.25);

  assert.equal(strategy, 'session-disjoint');
  assert.deepEqual(trainSet.map(s => s.id), ['a1', 'a2']);
  assert.deepEqual(evalSet.map(s => s.id), ['b1', 'b2']);
  assert.deepEqual(new Set(trainSet.map(s => s.sessionId)), new Set(['session-a']));
  assert.deepEqual(new Set(evalSet.map(s => s.sessionId)), new Set(['session-b']));
});

test('refuses an unlabelled or single-session evaluation split', () => {
  assert.throws(
    () => splitBySession([{ sessionId: null, timestamp: 1 }], 0.2),
    /at least two labelled sessions/,
  );
  assert.throws(
    () => splitBySession([{ sessionId: 'only', timestamp: 1 }], 0.2),
    /at least two labelled sessions/,
  );
  const twoSessions = [
    { sessionId: 'a', timestamp: 1 },
    { sessionId: 'b', timestamp: 2 },
  ];
  assert.throws(() => splitBySession(twoSessions, Number.NaN), /eval fraction/);
  assert.throws(() => splitBySession(twoSessions, 0), /eval fraction/);
  assert.throws(() => splitBySession(twoSessions, 1), /eval fraction/);
});

test('rejects heterogeneous channel counts and excludes absence from pose data', () => {
  const samples = [
    { csiDim: 2, timeSteps: 20, presence: true },
    { csiDim: 3, timeSteps: 20, presence: true },
  ];
  assert.throws(() => validatePoseDataset(samples, 20), /heterogeneous CSI shape/);

  const validated = validatePoseDataset([
    { csiDim: 2, timeSteps: 20, presence: false },
    { csiDim: 2, timeSteps: 20, presence: true },
  ], 20);
  assert.equal(validated.length, 1);
  assert.equal(validated[0].presence, true);
});

test('orders large sessions without spreading arguments onto the stack', () => {
  const largeSession = Array.from({ length: 150_000 }, (_, index) => ({
    sessionId: 'large',
    timestamp: index + 1,
  }));
  const samples = [...largeSession, { sessionId: 'latest', timestamp: 200_000 }];

  const { trainSet, evalSet } = splitBySession(samples, 0.000_001);

  assert.equal(trainSet.length, largeSession.length);
  assert.deepEqual(evalSet.map(sample => sample.sessionId), ['latest']);
});
