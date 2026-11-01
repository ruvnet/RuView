import test from 'node:test';
import assert from 'node:assert/strict';
import { mr60Checksum, readMmwave } from '../src/devices/mmwave.js';

const header = [1, 0, 1, 0, 1, 0x0f, 9];
const bytes = Buffer.from([...header, mr60Checksum(header), 1, mr60Checksum([1])]);
const deps = () => ({ python: () => 'fixture-python', exec: async () => ({ ok: true, stdout: `HEX ${bytes.toString('hex')}\nEND ${bytes.length}\n` }) });

test('known radar model returns its single valid received presence frame', async () => {
  const result = await readMmwave({ port: '/dev/ttyUSB0', model: 'mr60bha2', seconds: 1 }, deps());
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.frames, 1);
  assert.equal(result.presentFraction, 1);
  assert.equal(result.model, 'mr60bha2');
});

test('auto detection still requires multiple validating frames', async () => {
  const result = await readMmwave({ port: '/dev/ttyUSB0', model: 'auto', seconds: 1 }, deps());
  assert.equal(result.ok, false);
  assert.equal(result.reason, 'no_valid_frames');
});
