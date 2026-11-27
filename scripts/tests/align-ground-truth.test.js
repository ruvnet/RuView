'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const ALIGN = path.resolve(__dirname, '..', 'align-ground-truth.js');

function jsonl(file, records) {
  fs.writeFileSync(file, records.map(JSON.stringify).join('\n') + '\n');
}

function runAlignment(gtRecords, csiRecords) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ruview-align-'));
  const gt = path.join(dir, 'camera.jsonl');
  const csi = path.join(dir, 'session.csi.jsonl');
  const output = path.join(dir, 'paired.jsonl');
  jsonl(gt, gtRecords);
  jsonl(csi, csiRecords);
  const result = spawnSync(process.execPath, [
    ALIGN, '--gt', gt, '--csi', csi, '--output', output,
    '--window-frames', '2', '--window-ms', '200',
    '--min-camera-frames', '2',
  ], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return fs.readFileSync(output, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
}

function detectedCameraFrames(baseMs) {
  const keypoints = Array.from({ length: 17 }, (_, i) => [i / 20, i / 20]);
  return [0, 50, 100].map(offset => ({
    ts_ns: (baseMs + offset) * 1e6,
    keypoints,
    confidence: 0.9,
    n_visible: 17,
    n_persons: 1,
  }));
}

test('windows each node independently and emits channel-major CSI', () => {
  const baseMs = 1_700_000_000_000;
  const csi = [0, 100].map((offset, tick) => ({
    type: 'sensing_update',
    timestamp: (baseMs + offset) / 1000,
    tick,
    nodes: [
      { node_id: 11, amplitude: tick === 0 ? [1, 2] : [3, 4], rssi_dbm: -40 },
      { node_id: 12, amplitude: tick === 0 ? [5, 6] : [7, 8], rssi_dbm: -42 },
    ],
  }));

  const paired = runAlignment(detectedCameraFrames(baseMs), csi);

  assert.equal(paired.length, 2);
  assert.deepEqual(paired.map(sample => sample.node_id), [11, 12]);
  assert.deepEqual(paired[0].csi_shape, [2, 2]);
  assert.deepEqual(paired[0].csi, [1, 3, 2, 4]);
  assert.deepEqual(paired[1].csi, [5, 7, 6, 8]);
});

test('retains explicit camera-negative windows as absence labels', () => {
  const baseMs = 1_700_000_000_000;
  const gt = [0, 50, 100].map(offset => ({
    ts_ns: (baseMs + offset) * 1e6,
    keypoints: [],
    confidence: 0,
    n_visible: 0,
    n_persons: 0,
  }));
  const csi = [0, 100].map((offset, tick) => ({
    type: 'sensing_update',
    timestamp: (baseMs + offset) / 1000,
    tick,
    nodes: [{ node_id: 11, amplitude: [1 + tick, 2 + tick], rssi_dbm: -40 }],
  }));

  const [sample] = runAlignment(gt, csi);

  assert.equal(sample.presence, false);
  assert.equal(sample.n_persons_mode, 0);
  assert.equal(sample.conf, 0);
  assert.deepEqual(sample.kp, Array.from({ length: 17 }, () => [0, 0]));
});

test('rejects enter-leave transition windows instead of contradicting labels', () => {
  const baseMs = 1_700_000_000_000;
  const keypoints = Array.from({ length: 17 }, () => [0.1, 0.2]);
  const gt = [
    { ts_ns: baseMs * 1e6, keypoints: [], confidence: 0, n_persons: 0 },
    { ts_ns: (baseMs + 30) * 1e6, keypoints: [], confidence: 0, n_persons: 0 },
    { ts_ns: (baseMs + 60) * 1e6, keypoints, confidence: 0.9, n_persons: 1 },
    { ts_ns: (baseMs + 90) * 1e6, keypoints, confidence: 0.9, n_persons: 1 },
  ];
  const csi = [0, 100].map((offset, tick) => ({
    type: 'sensing_update',
    timestamp: (baseMs + offset) / 1000,
    tick,
    nodes: [{ node_id: 11, amplitude: [1 + tick, 2 + tick], rssi_dbm: -40 }],
  }));

  assert.deepEqual(runAlignment(gt, csi), []);
});

test('rejects camera frames whose count contradicts their keypoints', () => {
  const baseMs = 1_700_000_000_000;
  const keypoints = Array.from({ length: 17 }, () => [0.1, 0.2]);
  const gt = [0, 50, 100].map(offset => ({
    ts_ns: (baseMs + offset) * 1e6,
    keypoints,
    confidence: 0.9,
    n_persons: 0,
  }));
  const csi = [0, 100].map((offset, tick) => ({
    type: 'sensing_update',
    timestamp: (baseMs + offset) / 1000,
    tick,
    nodes: [{ node_id: 11, amplitude: [1 + tick, 2 + tick], rssi_dbm: -40 }],
  }));

  assert.deepEqual(runAlignment(gt, csi), []);
});
