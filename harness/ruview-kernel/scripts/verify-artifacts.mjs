#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Verify packaged kernel artifacts before pack/publish (ADR-368):
// checksums match, the WASM module imports nothing, and ABI versions agree.

import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { ABI_VERSION } from '../src/abi.js';
import { DEFAULT_WASM_PATH, PACKAGE_ROOT, loadKernel, readVerifiedArtifact } from '../src/index.js';
import { compileKernelModule } from '../src/wasm.js';

const requireWasm = process.argv.includes('--require-wasm');
const quiet = process.argv.includes('--quiet');
const report = { ok: true, wasm: null, native: [] };
const fail = (message) => { report.ok = false; (report.errors ||= []).push(message); };

if (existsSync(DEFAULT_WASM_PATH)) {
  try {
    const artifact = readVerifiedArtifact(DEFAULT_WASM_PATH);
    if (artifact.integrity !== 'verified') fail('wasm/ruview_kernel.wasm has no SHA256SUMS entry');
    compileKernelModule(artifact.bytes);
    const kernel = loadKernel({ backend: 'wasm' });
    if (kernel.info().abi !== ABI_VERSION) fail('WASM ABI mismatch');
    report.wasm = { sha256: artifact.sha256, bytes: artifact.bytes.length, imports: 0 };
  } catch (error) {
    fail(`wasm: ${error.message}`);
  }
} else if (requireWasm) {
  fail('wasm/ruview_kernel.wasm is missing; run `npm run build:wasm`');
}

const nativeDir = join(PACKAGE_ROOT, 'native');
for (const name of existsSync(nativeDir) ? readdirSync(nativeDir).filter((n) => n.endsWith('.node')).sort() : []) {
  try {
    const artifact = readVerifiedArtifact(join(nativeDir, name));
    if (artifact.integrity !== 'verified') fail(`native/${name} has no SHA256SUMS entry`);
    report.native.push({ name, sha256: artifact.sha256 });
  } catch (error) {
    fail(`native/${name}: ${error.message}`);
  }
}

if (!quiet || !report.ok) console.log(JSON.stringify(report, null, 2));
process.exit(report.ok ? 0 : 1);
