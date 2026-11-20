import { python } from './helpers/python.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const script = fileURLToPath(new URL('../../../scripts/overnight-empty-capture.py', import.meta.url));
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
function packet(antennas = 1) {
  const b = Buffer.alloc(20 + antennas * 4);
  b.writeUInt32LE(0xC5110001); b[4] = 7; b[5] = antennas; b.writeUInt16LE(2, 6);
  b.writeUInt32LE(2437, 8); b.writeUInt32LE(0x01020304, 12); b.writeInt8(-50, 16); b.writeInt8(-95, 17); b[18] = 1; b[19] = 3;
  for (let i = 20; i < b.length; i += 4) Buffer.from([3, 4, 250, 8]).copy(b, i);
  return b;
}
async function capture(packets) {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-overnight-'));
  const probe = dgram.createSocket('udp4'); probe.bind(0, '127.0.0.1'); await once(probe, 'listening');
  const port = probe.address().port; await new Promise(resolve => probe.close(resolve));
  const tx = dgram.createSocket('udp4');
  const child = spawn(python, ['-u', script, '--port', String(port), '--segments', '1', '--segment-seconds', '1', '--output', dir]);
  const exited = once(child, 'exit');
  try {
    for (let i = 0; !readdirSync(dir).length && i < 40; i++) await delay(25);
    assert.ok(readdirSync(dir).length, 'capture must bind and open its segment');
    for (const p of packets) await new Promise((resolve, reject) => tx.send(p, port, '127.0.0.1', e => e ? reject(e) : resolve()));
    const [code] = await exited; assert.equal(code, 0);
    const text = readFileSync(join(dir, readdirSync(dir)[0]), 'utf8').trim();
    return text ? text.split('\n').map(JSON.parse) : [];
  } finally { tx.close(); if (child.exitCode === null) { child.kill('SIGINT'); await exited; } rmSync(dir, { recursive: true, force: true }); }
}
test('public overnight capture records ADR018 metadata and only declared IQ payload', async () => {
  const b = packet(2), [f] = await capture([Buffer.concat([b, Buffer.from([255, 255])])]);
  assert.equal(f.rssi, -50); assert.equal(f.channel, 6); assert.equal(f.seq, 0x01020304);
  assert.equal(f.n_antennas, 2); assert.equal(f.n_subcarriers, 2); assert.equal(f.subcarriers, 4);
  assert.deepEqual(f.amplitudes, [5, 10, 5, 10]); assert.equal(f.iq_hex, b.subarray(20).toString('hex'));
});
test('public overnight capture rejects wrong magic and truncated payloads while keeping valid frames', async () => {
  const valid = packet(), wrong = Buffer.from(valid); wrong.writeUInt32LE(0xC5110003);
  const frames = await capture([wrong, valid.subarray(0, valid.length - 1), valid]);
  assert.equal(frames.length, 1); assert.deepEqual(frames[0].amplitudes, [5, 10]);
});
