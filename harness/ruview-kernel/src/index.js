// SPDX-License-Identifier: MIT
// @ruvnet/ruview-kernel — Node loader with explicit backend selection (ADR-368).
//
// Backends:
//   wasm (default) — zero-import WebAssembly: no host authority at all.
//   napi           — optional napi-rs native addon; full process authority,
//                    so it is only used when requested (`napi`) or allowed
//                    (`auto`, which prefers napi and falls back to wasm).
// The loader always reports which backend actually runs and why any
// fallback happened; it never silently substitutes a backend for `napi`.

import { createHash } from 'node:crypto';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ABI_VERSION, KernelError, createKernel } from './abi.js';
import { loadWasmKernelFromBytes } from './wasm.js';
import { platformTriple } from './platform.js';

export { ABI_VERSION, KernelError, OPERATIONS } from './abi.js';
export { loadWasmKernelFromBytes } from './wasm.js';

export const PACKAGE_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
export const DEFAULT_WASM_PATH = join(PACKAGE_ROOT, 'wasm', 'ruview_kernel.wasm');
export const BACKENDS = Object.freeze(['wasm', 'napi', 'auto']);
const MAX_ARTIFACT_BYTES = 64 * 1024 * 1024;
const require = createRequire(import.meta.url);

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** Parse a `sha256  name` checksum file next to an artifact. */
function expectedDigest(artifactPath) {
  const sums = join(dirname(artifactPath), 'SHA256SUMS');
  if (!existsSync(sums)) return null;
  for (const line of readFileSync(sums, 'utf8').split('\n')) {
    const match = /^([0-9a-f]{64})\s+\*?(.+)$/.exec(line.trim());
    if (match && match[2] === basename(artifactPath)) return match[1];
  }
  return null;
}

/**
 * Read an artifact and check it against its SHA256SUMS entry. A missing
 * checksum is reported (integrity `unverified`); a mismatch always fails.
 */
export function readVerifiedArtifact(path) {
  const stat = statSync(path);
  if (!stat.isFile()) throw new KernelError('artifact_invalid', `${path} is not a regular file`);
  if (stat.size > MAX_ARTIFACT_BYTES) throw new KernelError('artifact_invalid', `${path} exceeds ${MAX_ARTIFACT_BYTES} bytes`);
  const bytes = readFileSync(path);
  const digest = sha256(bytes);
  const expected = expectedDigest(path);
  if (expected && expected !== digest) {
    throw new KernelError('integrity_mismatch', `${basename(path)} sha256 ${digest} does not match SHA256SUMS ${expected}`);
  }
  return { bytes, sha256: digest, integrity: expected ? 'verified' : 'unverified' };
}

/** Candidate native addon locations, most specific first. */
export function nativeCandidates(triple = platformTriple()) {
  const candidates = [join(PACKAGE_ROOT, 'native', `ruview_kernel.${triple}.node`)];
  try {
    candidates.push(require.resolve(`@ruvnet/ruview-kernel-${triple}`));
  } catch { /* optional platform package not installed */ }
  return candidates;
}

function loadWasm(options) {
  const path = options.wasmPath || DEFAULT_WASM_PATH;
  if (!existsSync(path)) {
    throw new KernelError('backend_unavailable', `WASM kernel not found at ${path}; run \`npm run build:wasm\` in harness/ruview-kernel`);
  }
  const artifact = readVerifiedArtifact(path);
  return loadWasmKernelFromBytes(artifact.bytes, { artifact: path, sha256: artifact.sha256, integrity: artifact.integrity });
}

function loadNapi(options) {
  const triple = platformTriple();
  const candidates = options.nativePath ? [options.nativePath] : nativeCandidates(triple);
  const path = candidates.find((p) => existsSync(p));
  if (!path) {
    throw new KernelError('backend_unavailable', `no napi-rs addon for ${triple}; run \`npm run build:napi\` or use the wasm backend`);
  }
  const artifact = readVerifiedArtifact(path);
  const addon = require(path);
  if (typeof addon?.call !== 'function' || typeof addon?.abiVersion !== 'function') {
    throw new KernelError('untrusted_module', `${basename(path)} does not expose the ruview-kernel ABI`);
  }
  const abi = addon.abiVersion();
  if (abi !== ABI_VERSION) throw new KernelError('abi_mismatch', `napi ABI ${abi} != loader ABI ${ABI_VERSION}`);
  return createKernel((op, json) => addon.call(op, json), {
    backend: 'napi', abi, triple, artifact: path, sha256: artifact.sha256, integrity: artifact.integrity,
  }, typeof addon.callF64 === 'function' ? (op, json, data) => addon.callF64(op, json, data) : null);
}

/**
 * Load the kernel. `backend` defaults to RUVIEW_KERNEL_BACKEND, then `wasm`.
 * Options: { backend, wasmPath, nativePath, requireIntegrity }.
 */
export function loadKernel(options = {}) {
  const requested = options.backend || process.env.RUVIEW_KERNEL_BACKEND || 'wasm';
  if (!BACKENDS.includes(requested)) {
    throw new KernelError('invalid_backend', `backend must be one of ${BACKENDS.join(', ')}`);
  }
  let kernel;
  let fallbackReason = null;
  if (requested === 'wasm') kernel = loadWasm(options);
  else if (requested === 'napi') kernel = loadNapi(options);
  else {
    try {
      kernel = loadNapi(options);
    } catch (error) {
      if (error.code === 'integrity_mismatch') throw error; // tampering is never a fallback case
      fallbackReason = error.message;
      kernel = loadWasm(options);
    }
  }
  if (options.requireIntegrity && kernel.integrity !== 'verified') {
    throw new KernelError('integrity_unverified', `${kernel.backend} artifact has no SHA256SUMS entry`);
  }
  return Object.freeze({ ...kernel, requestedBackend: requested, fallbackReason });
}

/** Non-throwing availability report for both backends (used by `doctor`). */
export function kernelStatus(options = {}) {
  const probe = (backend) => {
    try {
      const k = loadKernel({ ...options, backend });
      const info = k.info();
      return { available: true, version: info.version, abi: k.abi, target: info.target, artifact: k.artifact, sha256: k.sha256, integrity: k.integrity };
    } catch (error) {
      return { available: false, code: error.code || 'error', reason: error.message };
    }
  };
  return { node: process.version, triple: platformTriple(), abi: ABI_VERSION, wasm: probe('wasm'), napi: probe('napi') };
}

const within = (value, target, tolerance) => typeof value === 'number' && Math.abs(value - target) <= tolerance;

/**
 * Deterministic end-to-end self-test on SYNTHETIC CSI with known tones.
 * Exercises synthesize → analyze → streaming session → validation errors.
 */
export function selfTest(kernel, { seconds = 60, breathingBpm = 15, heartBpm = 72 } = {}) {
  const started = performance.now();
  const checks = [];
  const check = (name, pass, detail = undefined) => checks.push({ name, pass: Boolean(pass), ...(detail === undefined ? {} : { detail }) });

  const info = kernel.info();
  check('abi matches loader', info.abi === ABI_VERSION, { abi: info.abi });

  const synth = kernel.synthesize({ seconds, breathing_bpm: breathingBpm, heart_bpm: heartBpm, seed: 7 });
  check('synthetic data is labelled SYNTHETIC', synth.evidence === 'SYNTHETIC');
  const report = kernel.analyze(synth.frames, {}, { includeReadings: true });
  const last = report.summary.last;
  check('one reading per second', report.readings.length === Math.round(seconds), { readings: report.readings.length });
  check('respiratory rate recovered within 3 bpm', within(last?.respiratory.bpm, breathingBpm, 3), { bpm: last?.respiratory.bpm });
  check('heart rate recovered within 8 bpm', within(last?.heart.bpm, heartBpm, 8), { bpm: last?.heart.bpm });

  const session = kernel.openSession({});
  for (let i = 0; i < synth.frames.length; i += 97) session.push(synth.frames.slice(i, i + 97));
  const streamed = session.summary();
  session.close();
  check('streaming session equals batch analysis', JSON.stringify(streamed) === JSON.stringify(report.summary));

  let rejected = null;
  try { kernel.analyze([{ amplitudes: [1] }], { n_subcarriers: 2 }); } catch (error) { rejected = error.code; }
  check('malformed frame rejected fail-closed', rejected === 'invalid_request', { code: rejected });

  return {
    ok: checks.every((c) => c.pass),
    evidence: 'SYNTHETIC',
    backend: kernel.backend,
    kernel: { version: info.version, abi: info.abi, target: info.target },
    summary: report.summary,
    checks,
    elapsedMs: Math.round(performance.now() - started),
  };
}

/** Numeric deep comparison with a relative tolerance (libm may differ by ULPs across targets). */
export function deepClose(a, b, tolerance = 1e-9, path = '$') {
  if (typeof a === 'number' && typeof b === 'number') {
    const scale = Math.max(1, Math.abs(a), Math.abs(b));
    return Math.abs(a - b) <= tolerance * scale ? [] : [`${path}: ${a} != ${b}`];
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return [`${path}: length ${a.length} != ${b.length}`];
    return a.flatMap((v, i) => deepClose(v, b[i], tolerance, `${path}[${i}]`));
  }
  if (a && b && typeof a === 'object' && typeof b === 'object') {
    const keys = [...new Set([...Object.keys(a), ...Object.keys(b)])].sort();
    return keys.flatMap((k) => deepClose(a[k], b[k], tolerance, `${path}.${k}`));
  }
  return Object.is(a, b) ? [] : [`${path}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`];
}

/** Run the same SYNTHETIC workload on two kernels and diff every output. */
export function parity(left, right, { seconds = 45 } = {}) {
  const request = { seconds, seed: 11, n_subcarriers: 32 };
  const a = left.synthesize(request);
  const b = right.synthesize(request);
  const synthDiff = deepClose(a.frames, b.frames);
  const ra = left.analyze(a.frames, { n_subcarriers: 32 }, { includeReadings: true });
  const rb = right.analyze(a.frames, { n_subcarriers: 32 }, { includeReadings: true });
  const analyzeDiff = deepClose(ra, rb);
  const differences = [...synthDiff, ...analyzeDiff];
  return {
    ok: differences.length === 0,
    evidence: 'SYNTHETIC',
    backends: [left.backend, right.backend],
    frames: a.frames.length,
    readings: ra.readings.length,
    exactJsonMatch: JSON.stringify(ra) === JSON.stringify(rb),
    differences: differences.slice(0, 20),
  };
}
