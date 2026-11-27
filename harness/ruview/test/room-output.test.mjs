import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, statSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runTraining } from '../src/training.js';

// Exercise real output I/O; this writer is not an ML trainer.
function fixture() {
  const repo = mkdtempSync(join(tmpdir(), 'ruview-room-output-'));
  mkdirSync(join(repo, 'v2'));
  writeFileSync(join(repo, 'v2', 'Cargo.toml'), '[workspace]\n');
  const calls = [];
  const deps = {
    findRepoRoot: () => repo,
    which: name => name === 'wifi-densepose' ? '/fixture/trainer' : null,
    exec: async (cmd, args, options) => {
      calls.push({ cmd, args, options });
      const index = args.indexOf('--output');
      const output = index < 0 ? join(options.cwd, 'room-bank.json') : args[index + 1];
      try {
        writeFileSync(output, '{"fixture":true}\n');
        return { ok: true, code: 0, stdout: 'fixture output saved', stderr: '' };
      } catch (error) {
        return { ok: false, code: 1, stdout: '', stderr: error.code };
      }
    },
  };
  return { repo, calls, deps, cleanup: () => rmSync(repo, { recursive: true, force: true }) };
}

for (const mode of ['default', 'nested', 'cargo']) {
  test('room training prepares an output file: ' + mode, async () => {
    const f = fixture();
    try {
      if (mode === 'cargo') f.deps.which = name => name === 'cargo' ? '/fixture/cargo' : null;
      const output = mode === 'nested' ? 'banks/room.json' : 'room-bank.json';
      const result = await runTraining({ mode: 'room', confirm: true, ...(mode === 'nested' ? { output } : {}) }, f.deps);
      assert.equal(result.ok, true, result.stderr);
      assert.equal(f.calls.length, 1);
      assert.ok(statSync(join(f.repo, output)).isFile());
      assert.deepEqual(JSON.parse(readFileSync(join(f.repo, output), 'utf8')), { fixture: true });
      assert.deepEqual(result.plan.writes, [join(f.repo, output)]);
    } finally { f.cleanup(); }
  });
}

test('room training can replace a prior regular output file', async () => {
  const f = fixture();
  try {
    writeFileSync(join(f.repo, 'room-bank.json'), '{"previous":true}');
    const result = await runTraining({ mode: 'room', confirm: true }, f.deps);
    assert.equal(result.ok, true);
    assert.deepEqual(JSON.parse(readFileSync(join(f.repo, 'room-bank.json'), 'utf8')), { fixture: true });
  } finally { f.cleanup(); }
});

test('planning room training creates no output and executes nothing', async () => {
  const f = fixture();
  try {
    const result = await runTraining({ mode: 'room', output: 'banks/room.json' }, f.deps);
    assert.equal(result.dryRun, true);
    assert.equal(f.calls.length, 0);
    assert.equal(existsSync(join(f.repo, 'banks')), false);
  } finally { f.cleanup(); }
});

test('pose checkpoints remain directories', async () => {
  const f = fixture();
  try {
    f.deps.which = name => name === 'cargo' ? '/fixture/cargo' : null;
    f.deps.exec = async (_cmd, _args, options) => {
      assert.ok(statSync(options.cwd).isDirectory());
      return { ok: true, code: 0, stdout: '', stderr: '' };
    };
    assert.equal((await runTraining({ mode: 'pose-smoke', confirm: true }, f.deps)).ok, true);
  } finally { f.cleanup(); }
});

test('an unusable output parent refuses without replacing that file', async () => {
  const f = fixture();
  try {
    writeFileSync(join(f.repo, 'blocked'), 'preserve me');
    const result = await runTraining({ mode: 'room', output: 'blocked/room.json', confirm: true }, f.deps);
    assert.equal(result.ok, false);
    assert.equal(result.reason, 'output_preparation_failed');
    assert.equal(f.calls.length, 0);
    assert.equal(readFileSync(join(f.repo, 'blocked'), 'utf8'), 'preserve me');
  } finally { f.cleanup(); }
});
