import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { execTool } from '../src/exec.js';
import { readSerial } from '../src/devices/serial-pump.js';
import { readMmwave, mr60Checksum } from '../src/devices/mmwave.js';
import { readRplidar } from '../src/devices/lidar.js';
const python = ['python3', 'python'].find(p => spawnSync(p, ['--version'], { stdio: 'ignore' }).status === 0);
function fixture(t, bytes, interrupted) {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-partial-serial-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  writeFileSync(join(dir, 'serial.py'), [
    'import time',
    'class Serial:',
    `    def __init__(self, *a, **kw): self.first=True; self.chunk=bytes.fromhex('${bytes.toString('hex')}')`,
    '    def read(self, n):',
    '        if self.first: self.first=False; return self.chunk',
    interrupted ? "        raise RuntimeError('fixture disconnected after partial capture')" : "        time.sleep(0.01); return b''",
    '    def write(self, data): return len(data)',
    '    def flush(self): pass',
    '    def close(self): pass',
  ].join('\n'));
  return { python: () => python, exec: (cmd, args, opts) => execTool(cmd, args, { ...opts, cwd: dir }) };
}
function mr(type, value) {
  const payload = Buffer.alloc(4); payload.writeFloatLE(value);
  const header = Buffer.from([1, 0, 0, 0, 4, type >> 8, type & 255]);
  return Buffer.concat([header, Buffer.from([mr60Checksum(header)]), payload, Buffer.from([mr60Checksum(payload)])]);
}
const radar = Buffer.concat([mr(0x0a14, 15), mr(0x0a15, 72)]);
const scan = Buffer.from([5, 1, 0, 160, 15, 6, 1, 45, 64, 31]);

test('public serial pump preserves bytes and flags an interrupted subprocess', { skip: !python }, async t => {
  const result = await readSerial({ port: 'COM3', baud: 115200, seconds: 0.5 }, fixture(t, radar, true));
  assert.equal(result.ok, true, 'partial bytes remain available');
  assert.deepEqual(result.bytes, radar);
  assert.equal(result.captureIncomplete, true);
  assert.match(result.warnings.join(' '), /partial|incomplete/i);
});

test('public radar summary retains interrupted capture state', { skip: !python }, async t => {
  const radarResult = await readMmwave({ port: 'COM3', model: 'mr60bha2', seconds: 1 }, fixture(t, radar, true));
  assert.equal(radarResult.ok, true);
  assert.equal(radarResult.heartBpmMean, 72);
  assert.equal(radarResult.captureIncomplete, true);
});

test('public LiDAR summary retains interrupted capture state', { skip: !python }, async t => {
  const scanResult = await readRplidar({ port: 'COM3', seconds: 1 }, fixture(t, scan, true));
  assert.equal(scanResult.ok, true);
  assert.equal(scanResult.validPoints, 2);
  assert.equal(scanResult.captureIncomplete, true);
  assert.match(scanResult.warnings.join(' '), /partial|incomplete/i);
});

test('normal pump completion retains the complete-capture control', { skip: !python }, async t => {
  const result = await readSerial({ port: 'COM3', baud: 115200, seconds: 0.5 }, fixture(t, radar, false));
  assert.equal(result.ok, true);
  assert.deepEqual(result.bytes, radar);
  assert.notEqual(result.captureIncomplete, true);
  assert.equal(result.warnings, undefined);
});

test('auto-detection retains probe values but flags failure of the remainder', { skip: !python }, async t => {
  const good = fixture(t, radar, false); const bad = fixture(t, Buffer.alloc(0), true);
  let calls = 0;
  const result = await readMmwave({ port: 'COM3', seconds: 2.5 }, { python: good.python, exec: (...args) => (++calls === 1 ? good.exec : bad.exec)(...args) });
  assert.equal(result.ok, true);
  assert.equal(result.frames, 2);
  assert.equal(result.captureIncomplete, true);
  assert.match(result.warnings.join(' '), /remaining.*failed/);
});
