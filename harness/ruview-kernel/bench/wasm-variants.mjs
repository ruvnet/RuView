#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Reproducer for the ADR-368 amendment's rejected WASM build options.
// Builds the kernel three ways into separate target dirs and reports size,
// median analyzeFlat time over 15 runs (2 400 SYNTHETIC frames), and whether
// outputs are byte-identical to the baseline build.
//   node bench/wasm-variants.mjs     (from harness/ruview-kernel; needs cargo + wasm32 target)
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { flattenFrames } from '../src/abi.js';
import { loadWasmKernelFromBytes } from '../src/wasm.js';

const V2 = resolve(import.meta.dirname, '..', '..', '..', 'v2');
const variants = {
  baseline: {},
  simd128: { CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS: '-C target-feature=+simd128' },
  'opt-level-s': { CARGO_PROFILE_RELEASE_OPT_LEVEL: 's' },
};
const built = {};
for (const [name, env] of Object.entries(variants)) {
  const targetDir = join(tmpdir(), `ruview-kernel-variant-${name}`);
  const r = spawnSync('cargo', ['build', '-q', '-p', 'ruview-kernel', '--release', '--target', 'wasm32-unknown-unknown', '--target-dir', targetDir], { cwd: V2, env: { ...process.env, ...env }, stdio: 'inherit' });
  if (r.status !== 0) process.exit(1);
  built[name] = readFileSync(join(targetDir, 'wasm32-unknown-unknown', 'release', 'ruview_kernel.wasm'));
}
const base = loadWasmKernelFromBytes(built.baseline);
const { frames } = base.synthesize({ seconds: 120 });
const { data } = flattenFrames(frames, 56);
const reference = JSON.stringify(base.analyzeFlat(data, {}, { phases: true, includeReadings: true }));
const rows = [];
for (const [name, bytes] of Object.entries(built)) {
  const k = loadWasmKernelFromBytes(bytes);
  const identical = JSON.stringify(k.analyzeFlat(data, {}, { phases: true, includeReadings: true })) === reference;
  for (let i = 0; i < 3; i++) k.analyzeFlat(data, {}, { phases: true });
  const runs = [];
  for (let i = 0; i < 15; i++) { const t = performance.now(); k.analyzeFlat(data, {}, { phases: true }); runs.push(performance.now() - t); }
  runs.sort((a, b) => a - b);
  rows.push({ variant: name, bytes: bytes.length, medianMs: Number(runs[7].toFixed(1)), identicalToBaseline: identical });
}
console.log(JSON.stringify({ evidence: 'MEASURED (this host only)', frames: frames.length, rows }, null, 2));
