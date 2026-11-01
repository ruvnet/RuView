// End-to-end: Rust core → WASM/napi transports → JS API → CLI (ADR-368).
// Evidence for every numeric assertion here: SYNTHETIC.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { kernelStatus, loadKernel, parity, selfTest } from '../src/index.js';
import { napiSkip, requireWasm } from './helpers.mjs';

const CLI = join(dirname(dirname(fileURLToPath(import.meta.url))), 'bin', 'cli.js');
const cli = (...args) => {
  const r = spawnSync(process.execPath, [CLI, ...args], { encoding: 'utf8', env: { ...process.env, RUVIEW_KERNEL_BACKEND: '' } });
  return { status: r.status, stdout: r.stdout, stderr: r.stderr, json: () => JSON.parse(r.stdout) };
};

test('wasm backend passes the SYNTHETIC self-test', () => {
  requireWasm();
  const k = loadKernel({ backend: 'wasm' });
  assert.equal(k.backend, 'wasm');
  assert.equal(k.integrity, 'verified');
  const result = selfTest(k);
  assert.ok(result.ok, JSON.stringify(result.checks.filter((c) => !c.pass)));
  assert.equal(result.evidence, 'SYNTHETIC');
  assert.equal(result.kernel.target, 'wasm32');
});

test('napi backend passes the SYNTHETIC self-test', { skip: napiSkip }, () => {
  const k = loadKernel({ backend: 'napi' });
  assert.equal(k.backend, 'napi');
  const result = selfTest(k);
  assert.ok(result.ok, JSON.stringify(result.checks.filter((c) => !c.pass)));
  assert.equal(result.kernel.target, 'native');
});

test('wasm and napi produce the same outputs for the same input', { skip: napiSkip }, () => {
  const result = parity(loadKernel({ backend: 'wasm' }), loadKernel({ backend: 'napi' }));
  assert.ok(result.ok, result.differences.join('\n'));
  assert.equal(result.readings, 45);
});

test('auto prefers napi and reports an honest fallback to wasm', () => {
  requireWasm();
  const k = loadKernel({ backend: 'auto', nativePath: '/nonexistent/ruview_kernel.node' });
  assert.equal(k.backend, 'wasm');
  assert.equal(k.requestedBackend, 'auto');
  assert.match(k.fallbackReason, /no napi-rs addon|not found|nonexistent/);
});

test('kernelStatus reports both backends without throwing', () => {
  const status = kernelStatus();
  assert.equal(status.abi, 1);
  assert.equal(typeof status.wasm.available, 'boolean');
  assert.equal(typeof status.napi.available, 'boolean');
});

test('CLI: doctor, selftest, synth → analyze round trip', () => {
  requireWasm();
  const doctor = cli('doctor');
  assert.equal(doctor.status, 0, doctor.stderr);
  assert.equal(doctor.json().wasm.available, true);

  const st = cli('selftest', '--seconds', '45');
  assert.equal(st.status, 0, st.stdout);
  assert.equal(st.json().backend, 'wasm');

  const dir = mkdtempSync(join(tmpdir(), 'ruview-kernel-e2e-'));
  try {
    const file = join(dir, 'frames.json');
    const synth = cli('synth', '--seconds', '40', '--breathing', '12', '--subcarriers', '24', '--out', file);
    assert.equal(synth.status, 0, synth.stderr);
    assert.equal(cli('synth', '--seconds', '1', '--out', file).status, 1, 'synth must refuse to overwrite');

    const analyzed = cli('analyze', '--file', file, '--readings');
    assert.equal(analyzed.status, 0, analyzed.stderr);
    const report = analyzed.json();
    assert.equal(report.summary.frames, 800);
    assert.equal(report.readings.length, 40);
    assert.ok(Math.abs(report.summary.last.respiratory.bpm - 12) <= 3, `SYNTHETIC RR ${report.summary.last.respiratory.bpm}`);

    // JSONL input takes the same path.
    const frames = JSON.parse(cli('synth', '--seconds', '2', '--subcarriers', '4').stdout).frames;
    const jsonl = join(dir, 'frames.jsonl');
    writeFileSync(jsonl, frames.map((f) => JSON.stringify(f)).join('\n'));
    const small = cli('analyze', '--file', jsonl, '--config', '{"n_subcarriers":4}');
    assert.equal(small.status, 0, small.stdout);
    assert.equal(small.json().summary.frames, 40);

    const bad = cli('analyze', '--file', jsonl);
    assert.equal(bad.status, 1, 'width mismatch against default 56 subcarriers must fail');
    assert.equal(bad.json().code, 'invalid_request');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('CLI rejects unknown backends and commands', () => {
  assert.equal(cli('info', '--backend', 'gpu').status, 2);
  assert.equal(cli('frobnicate').status, 2);
});
