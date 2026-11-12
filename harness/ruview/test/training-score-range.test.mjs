import test from 'node:test';
import assert from 'node:assert/strict';
import { trainingGate } from '../src/training.js';

const valid = {
  metric: 'PCK@20', model_score: 0.7, baseline_score: 0.5,
  split: 'chronological', train_end: '2026-01-01', test_start: '2026-01-02',
  n_test: 200, reproducer: 'node evaluation.mjs', data: 'synthetic',
};

test('direct training gate refuses a model score outside its percentage contract', () => {
  for (const model_score of [-1, 100.01, 200]) {
    const result = trainingGate({ ...valid, model_score });
    assert.equal(result.verdict, 'FAIL', String(model_score));
    assert.equal(result.statement, null);
    assert.equal(result.evidence, null);
    assert.equal(result.deltaPp, null);
    assert.ok(result.findings.some(f => f.code === 'model_score_out_of_range'));
  }
});

test('direct training gate refuses an out-of-range baseline before computing lift', () => {
  for (const baseline_score of [-1, 100.01, 200]) {
    const result = trainingGate({ ...valid, baseline_score });
    assert.equal(result.verdict, 'FAIL', String(baseline_score));
    assert.equal(result.statement, null);
    assert.equal(result.evidence, null);
    assert.equal(result.deltaPp, null);
    assert.ok(result.findings.some(f => f.code === 'baseline_out_of_range'));
  }
});

test('fraction and percentage reports retain their baseline-relative result', () => {
  for (const [model_score, baseline_score, delta] of [[0.7, 0.5, 20], [70, 50, 20], [100, 0, 100]]) {
    const result = trainingGate({ ...valid, model_score, baseline_score });
    assert.equal(result.verdict, 'PASS');
    assert.equal(result.deltaPp, delta);
    assert.equal(result.evidence, 'SYNTHETIC');
  }
});
