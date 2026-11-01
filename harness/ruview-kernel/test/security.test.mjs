import { test } from 'node:test';
import assert from 'node:assert/strict';
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DEFAULT_WASM_PATH, loadKernel } from '../src/index.js';
import { compileKernelModule, loadWasmKernelFromBytes } from '../src/wasm.js';
import { HAS_WASM, MODULE_WITH_IMPORT, requireWasm } from './helpers.mjs';

test('a module that imports host functions is refused', () => {
  assert.throws(() => compileKernelModule(MODULE_WITH_IMPORT), (e) => e.code === 'untrusted_module');
  assert.throws(() => loadWasmKernelFromBytes(MODULE_WITH_IMPORT), (e) => e.code === 'untrusted_module');
});

test('a module without the kernel exports is refused', () => {
  const empty = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
  assert.throws(() => compileKernelModule(empty), (e) => e.code === 'untrusted_module' && /missing exports/.test(e.message));
});

test('invalid backend names are rejected', () => {
  assert.throws(() => loadKernel({ backend: 'python' }), (e) => e.code === 'invalid_backend');
});

test('explicit napi never silently falls back to wasm', () => {
  assert.throws(() => loadKernel({ backend: 'napi', nativePath: '/nonexistent/ruview.node' }), (e) => e.code === 'backend_unavailable');
});

test('checksum mismatch fails closed and is never an auto fallback', { skip: !HAS_WASM && 'wasm not built' }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-kernel-'));
  try {
    const wasmPath = join(dir, 'ruview_kernel.wasm');
    copyFileSync(DEFAULT_WASM_PATH, wasmPath);
    writeFileSync(join(dir, 'SHA256SUMS'), `${'0'.repeat(64)}  ruview_kernel.wasm\n`);
    assert.throws(() => loadKernel({ backend: 'wasm', wasmPath }), (e) => e.code === 'integrity_mismatch');

    const fakeNative = join(dir, 'fake.node');
    writeFileSync(fakeNative, 'not an addon');
    writeFileSync(join(dir, 'SHA256SUMS'), `${'1'.repeat(64)}  fake.node\n`);
    assert.throws(() => loadKernel({ backend: 'auto', nativePath: fakeNative }), (e) => e.code === 'integrity_mismatch');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('requireIntegrity refuses an unchecksummed artifact', { skip: !HAS_WASM && 'wasm not built' }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-kernel-'));
  try {
    const wasmPath = join(dir, 'ruview_kernel.wasm');
    copyFileSync(DEFAULT_WASM_PATH, wasmPath);
    assert.equal(loadKernel({ backend: 'wasm', wasmPath }).integrity, 'unverified');
    assert.throws(() => loadKernel({ backend: 'wasm', wasmPath, requireIntegrity: true }), (e) => e.code === 'integrity_unverified');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('kernel rejects hostile frames without crashing the instance', () => {
  requireWasm();
  const k = loadKernel({ backend: 'wasm' });
  const cases = [
    [() => k.analyze([{ amplitudes: [1, 2] }], { n_subcarriers: 3 }), 'invalid_request'],
    [() => k.analyze([{ amplitudes: [1e300] }], { n_subcarriers: 1 }), 'invalid_request'],
    [() => k.analyze([{ amplitudes: [1], evil: true }], { n_subcarriers: 1 }), 'invalid_request'],
    [() => k.analyze([], { n_subcarriers: 100000 }), 'invalid_request'],
    [() => k.analyze([], { sample_rate_hz: -1 }), 'invalid_request'],
    [() => k.synthesize({ seconds: 3600, sample_rate_hz: 1000 }), 'limit_exceeded'],
    [() => k.call('session_push', { session: 424242, frames: [] }), 'unknown_session'],
  ];
  for (const [fn, code] of cases) assert.throws(fn, (e) => e.code === code, code);
  assert.equal(k.info().abi, 1, 'instance still healthy after rejected input');
});

test('session registry is bounded per instance', () => {
  requireWasm();
  const k = loadKernel({ backend: 'wasm' });
  const sessions = [];
  let limited = false;
  for (let i = 0; i < 70; i++) {
    try { sessions.push(k.openSession({})); } catch (e) { assert.equal(e.code, 'limit_exceeded'); limited = true; break; }
  }
  assert.ok(limited, 'more than 64 concurrent sessions must be refused');
  for (const s of sessions) s.close();
  assert.ok(k.openSession({}).close());
});
