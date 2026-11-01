import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { execTool } from '../src/exec.js';
import { readMmwave } from '../src/devices/mmwave.js';
const python = ['python3', 'python'].find(p => spawnSync(p, ['--version'], { stdio: 'ignore' }).status === 0);
function target(state, moving = 0, stationary = 0) {
  const body = Buffer.alloc(13); body[0] = 2; body[1] = 0xaa; body[2] = state;
  body.writeUInt16LE(moving, 3); body[5] = 60; body.writeUInt16LE(stationary, 6); body[8] = 60;
  body.writeUInt16LE(state === 1 || state === 3 ? moving : stationary, 9); body[11] = 0x55;
  return Buffer.concat([Buffer.from([0xf4, 0xf3, 0xf2, 0xf1, 13, 0]), body, Buffer.from([0xf8, 0xf7, 0xf6, 0xf5])]);
}
async function read(t, frames) {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-ld2410-distance-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  writeFileSync(join(dir, 'serial.py'), [
    'import time', 'class Serial:',
    `    def __init__(self,*a,**k): self.chunk=bytes.fromhex('${Buffer.concat(frames).toString('hex')}')`,
    '    def read(self,n):', '        if self.chunk: b=self.chunk; self.chunk=b""; return b',
    '        time.sleep(0.01); return b""', '    def close(self): pass',
  ].join('\n'));
  return readMmwave({ port: 'COM3', model: 'ld2410', seconds: 1 }, {
    python: () => python, exec: (cmd, args, opts) => execTool(cmd, args, { ...opts, cwd: dir }),
  });
}

test('public LD2410 reader does not average absent targets as zero-distance observations', { skip: !python }, async t => {
  const result = await read(t, [target(1, 90), target(0)]);
  assert.equal(result.ok, true);
  assert.equal(result.frames, 2);
  assert.equal(result.presentFraction, 0.5);
  assert.equal(result.distanceCmMean, 90);
  assert.equal(result.last[1].distanceCm, 0, 'raw no-target sentinel remains available');
});

test('all-absent LD2410 capture has no measured mean distance', { skip: !python }, async t => {
  const result = await read(t, [target(0), target(0)]);
  assert.equal(result.ok, true);
  assert.equal(result.presentFraction, 0);
  assert.equal(result.distanceCmMean, null);
});

test('moving and stationary positive-distance controls remain averaged', { skip: !python }, async t => {
  const result = await read(t, [target(1, 90), target(2, 0, 210)]);
  assert.equal(result.ok, true);
  assert.equal(result.presentFraction, 1);
  assert.equal(result.distanceCmMean, 150);
});
