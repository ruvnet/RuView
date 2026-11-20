// Binary (Float64Array) fast path (ADR-368 amendment). Evidence: SYNTHETIC.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { flattenFrames, MAX_INPUT_BYTES } from '../src/abi.js';
import { deepClose, loadKernel } from '../src/index.js';
import { napiSkip, requireWasm } from './helpers.mjs';

test('flattenFrames packs uniform frames and defers irregular ones to JSON', () => {
  const f = (a, p) => (p ? { amplitudes: a, phases: p } : { amplitudes: a });
  const packed = flattenFrames([f([1, 2], [5, 6]), f([3, 4], [7, 8])], 2);
  assert.deepEqual([...packed.data], [1, 2, 3, 4, 5, 6, 7, 8]);
  assert.equal(packed.withPhases, true);
  assert.deepEqual([...flattenFrames([f([1, 2]), f([3, 4])], 2).data], [1, 2, 3, 4]);
  assert.equal(flattenFrames([f([1, 2])], 3), null, 'width mismatch');
  assert.equal(flattenFrames([f([1, 2], [1, 2]), f([3, 4])], 2), null, 'mixed phases');
  assert.equal(flattenFrames([{ amplitudes: [1, 2], extra: 1 }], 2), null, 'unknown key');
  assert.equal(flattenFrames([f([1, '2'])], 2), null, 'non-number');
  assert.equal(flattenFrames([null], 2), null);
  assert.equal(flattenFrames([], 2).data.length, 0);
});

function transportsAgree(k) {
  assert.equal(k.binary, true, `${k.backend} exposes the binary transport`);
  const { frames } = k.synthesize({ seconds: 30, n_subcarriers: 16 });
  const viaBinary = k.analyze(frames, { n_subcarriers: 16 }, { includeReadings: true });
  const viaJson = k.call('analyze', { config: { n_subcarriers: 16 }, frames, include_readings: true });
  // JSON number parsing perturbs inputs by ULPs; outputs agree to 1e-9.
  assert.deepEqual(deepClose(viaBinary, viaJson), []);
  const s = k.openSession({ n_subcarriers: 16 });
  for (let i = 0; i < frames.length; i += 50) s.push(frames.slice(i, i + 50));
  assert.deepEqual(s.summary(), viaBinary.summary, 'streamed binary pushes equal batch binary analysis');
  s.close();
}

test('wasm binary and JSON transports agree', () => {
  requireWasm();
  transportsAgree(loadKernel({ backend: 'wasm' }));
});

test('napi binary and JSON transports agree', { skip: napiSkip }, () => {
  transportsAgree(loadKernel({ backend: 'napi' }));
});

test('binary entry points validate input and keep precise JSON errors for bad frames', () => {
  requireWasm();
  const k = loadKernel({ backend: 'wasm' });
  assert.throws(() => k.analyzeFlat([1, 2, 3]), (e) => e.code === 'invalid_request');
  assert.throws(() => k.analyzeFlat(new Float64Array(MAX_INPUT_BYTES / 8 + 1)), (e) => e.code === 'limit_exceeded');
  assert.throws(() => k.analyzeFlat(new Float64Array(5), { n_subcarriers: 2 }), (e) => e.code === 'invalid_request');
  assert.throws(() => k.analyzeFlat(Float64Array.of(1, NaN), { n_subcarriers: 2 }), (e) => e.code === 'invalid_request');
  // A malformed frame falls back to JSON, whose error names the frame.
  assert.throws(() => k.analyze([{ amplitudes: [1, 2] }, { amplitudes: [1] }], { n_subcarriers: 2 }), (e) => /frames\[1\]/.test(e.message));
  assert.equal(k.info().abi, 1, 'instance healthy after rejected input');
});
