import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ABI_VERSION, KernelError, MAX_INPUT_BYTES, createKernel, decodeResponse, encodeRequest } from '../src/abi.js';
import { deepClose } from '../src/index.js';

test('encodeRequest enforces the operation allow-list and size bound', () => {
  assert.equal(encodeRequest('info', {}), '{}');
  assert.throws(() => encodeRequest('rm_rf', {}), (e) => e instanceof KernelError && e.code === 'unknown_operation');
  assert.throws(() => encodeRequest('analyze', 'x'.repeat(MAX_INPUT_BYTES + 1)), (e) => e.code === 'limit_exceeded');
  // Multi-byte characters count as UTF-8 bytes, not UTF-16 units.
  assert.throws(() => encodeRequest('analyze', '€'.repeat(MAX_INPUT_BYTES / 3 + 1)), (e) => e.code === 'limit_exceeded');
});

test('decodeResponse unwraps ok envelopes and raises structured errors', () => {
  assert.deepEqual(decodeResponse('{"ok":true,"result":{"a":1}}'), { a: 1 });
  assert.throws(() => decodeResponse('{"ok":false,"error":{"code":"invalid_request","message":"bad"}}'),
    (e) => e.code === 'invalid_request' && e.message === 'bad');
  assert.throws(() => decodeResponse('not json'), (e) => e.code === 'internal');
  assert.throws(() => decodeResponse('{"result":1}'), (e) => e.code === 'internal');
});

test('createKernel routes every helper through the single transport', () => {
  const calls = [];
  const transport = (op, json) => {
    calls.push([op, JSON.parse(json)]);
    if (op === 'session_open') return '{"ok":true,"result":{"session":9}}';
    if (op === 'session_close') return '{"ok":true,"result":{"closed":true}}';
    return `{"ok":true,"result":{"op":"${op}","abi":${ABI_VERSION}}}`;
  };
  const k = createKernel(transport, { backend: 'fake' });
  assert.equal(k.info().op, 'info');
  k.analyze([{ amplitudes: [1] }], { n_subcarriers: 1 }, { includeReadings: true });
  assert.deepEqual(calls.at(-1), ['analyze', { config: { n_subcarriers: 1 }, frames: [{ amplitudes: [1] }], include_readings: true }]);
  const s = k.openSession({});
  assert.equal(s.close(), true);
  assert.equal(s.close(), false, 'double close is a no-op');
  assert.throws(() => s.push([]), (e) => e.code === 'unknown_session');
  assert.ok(Object.isFrozen(k));
});

test('deepClose tolerates ULP noise but reports real differences', () => {
  assert.deepEqual(deepClose({ a: [1, 2.0000000000001] }, { a: [1, 2] }), []);
  assert.equal(deepClose({ a: 1 }, { a: 1.001 }).length, 1);
  assert.equal(deepClose({ a: 'x' }, { a: 'y' }).length, 1);
  assert.equal(deepClose([1], [1, 2]).length, 1);
  assert.equal(deepClose({ a: 1 }, {}).length, 1);
});
