'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const TRAINER = path.resolve(__dirname, '..', 'train-wiflow-supervised.js');

function runTrainer(samples) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ruview-trainer-'));
  const data = path.join(dir, 'paired.jsonl');
  fs.writeFileSync(data, samples.map(JSON.stringify).join('\n') + '\n');
  return spawnSync(process.execPath, [
    TRAINER,
    '--data', data,
    '--output', path.join(dir, 'model'),
    '--epochs', '0',
    '--skip-contrastive',
  ], { encoding: 'utf8' });
}

function sample(sessionId, csiDim = 2) {
  return {
    csi: Array(csiDim * 20).fill(1),
    csi_shape: [csiDim, 20],
    kp: Array.from({ length: 17 }, () => [0.1, 0.2]),
    conf: 0.9,
    session_id: sessionId,
    presence: true,
  };
}

test('trainer refuses evaluation from a single capture session', () => {
  const result = runTrainer([sample('only-session'), sample('only-session')]);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Cannot create leakage-free train\/eval split/);
});

test('trainer rejects heterogeneous CSI shapes before preprocessing', () => {
  const result = runTrainer([sample('session-a', 2), sample('session-b', 3)]);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Invalid pose dataset: heterogeneous CSI shape/);
});
