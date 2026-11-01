import test from 'node:test';
import assert from 'node:assert/strict';
import { createRuView } from '../src/sdk.js';
import { trainingGate } from '../src/training.js';
const report = { model_score: 0.7, baseline_score: 0.5, split: 'chronological', n_test: 200, data: 'synthetic' };

test('public SDK accepts epoch-zero ISO chronology boundary', async () => {
  const result = await createRuView().training.gate({ ...report, train_end: '1970-01-01T00:00:00.000Z', test_start: '1970-01-01T00:00:00.030Z' });
  assert.equal(result.verdict, 'PASS', JSON.stringify(result));
  assert.equal(result.evidence, 'SYNTHETIC');
});

test('public training export treats numeric timestamps as milliseconds', () => {
  const result = trainingGate({ ...report, train_end: 5, test_start: 30 });
  assert.equal(result.verdict, 'PASS', JSON.stringify(result));
});

test('equal chronology and invalid date controls remain refused', async () => {
  for (const [end, start] of [['1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z'], ['invalid', '2026-01-02T00:00:00Z']]) {
    assert.equal((await createRuView().training.gate({ ...report, train_end: end, test_start: start })).verdict, 'FAIL');
  }
});
