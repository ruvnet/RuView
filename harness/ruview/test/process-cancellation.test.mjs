import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runProcess } from '../src/process-runner.js';

const alive = pid => { try { process.kill(pid, 0); return true; } catch { return false; } };
test('timeout stops a TERM-resistant descendant even when its parent closes first', { skip: process.platform === 'win32' }, async () => {
  const root = mkdtempSync(join(tmpdir(), 'ruview-cancel-'));
  const marker = join(root, 'pid');
  const descendant = join(root, 'descendant.cjs');
  writeFileSync(descendant, `require('node:fs').writeFileSync(process.argv[2], String(process.pid)); process.on('SIGTERM', () => {}); setInterval(() => {}, 20);`);
  const source = `const { spawn } = require('node:child_process'); spawn(process.execPath, [process.argv[1], process.argv[2]], {stdio:'ignore'}); setInterval(() => {},20);`;
  const start = Date.now();
  let deadline;
  try {
    const running = runProcess(process.execPath, ['-e', source, descendant, marker], { timeoutMs: 300 });
    const result = await Promise.race([running.then(() => null, e => e), new Promise(resolve => { deadline = setTimeout(() => resolve(null), 2000); })]);
    assert.ok(existsSync(marker), 'real descendant started');
    const pid = Number(readFileSync(marker, 'utf8'));
    assert.ok(result?.timedOut, 'runner returns a timeout instead of hanging');
    assert.ok(Date.now() - start < 1800, 'termination is bounded');
    const stoppedBy = Date.now() + 250;
    while(alive(pid) && Date.now() < stoppedBy) await new Promise(resolve => setTimeout(resolve, 10));
    assert.equal(alive(pid), false, 'no TERM-resistant descendant remains');
  } finally {
    clearTimeout(deadline);
    if (existsSync(marker)) { const pid = Number(readFileSync(marker, 'utf8')); if(alive(pid)) process.kill(pid, 'SIGKILL'); }
    rmSync(root, {recursive:true, force:true});
  }
});
test('pre-cancelled work never launches a child', async () => {
  const root = mkdtempSync(join(tmpdir(), 'ruview-preabort-'));
  const marker = join(root, 'spawned');
  const controller = new AbortController(); controller.abort();
  try {
    await assert.rejects(runProcess(process.execPath, ['-e', `require('node:fs').writeFileSync(process.argv[1], 'started')`, marker], {signal:controller.signal}), e => e.aborted);
    assert.equal(existsSync(marker), false);
  } finally { rmSync(root, {recursive:true, force:true}); }
});
