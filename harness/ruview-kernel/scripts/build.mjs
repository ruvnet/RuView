#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Build the ruview-kernel artifacts from the Rust source in ../../v2 (ADR-368).
//   --wasm  cargo build -p ruview-kernel --target wasm32-unknown-unknown --release
//   --napi  cargo build --manifest-path v2/crates/ruview-kernel-napi/Cargo.toml --release
// Artifacts are copied into wasm/ and native/ with SHA256SUMS. No shell is used.

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { platformTriple } from '../src/platform.js';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const V2 = resolve(ROOT, '..', '..', 'v2');
const args = new Set(process.argv.slice(2));
const wantWasm = args.has('--wasm');
const wantNapi = args.has('--napi');
if (!wantWasm && !wantNapi) {
  console.error('Usage: node scripts/build.mjs [--wasm] [--napi]');
  process.exit(2);
}
if (!existsSync(join(V2, 'Cargo.toml'))) {
  console.error(`RuView Rust workspace not found at ${V2}; build from a RuView checkout.`);
  process.exit(2);
}

function cargo(cargoArgs, cwd) {
  console.error(`$ cargo ${cargoArgs.join(' ')}`);
  const r = spawnSync('cargo', cargoArgs, { cwd, stdio: 'inherit' });
  if (r.status !== 0) {
    console.error(`cargo failed (${r.error?.message || `exit ${r.status}`})`);
    const missing = uninitializedSubmodules();
    if (missing.length) {
      // The v2 workspace resolves path dependencies inside submodules; a fresh
      // clone or `git worktree add` leaves them empty and cargo fails on a
      // missing Cargo.toml.
      console.error(`Uninitialized git submodules (likely cause): ${missing.join(', ')}`);
      console.error('Fix: git submodule update --init --recursive   (run at the repository root)');
    }
    process.exit(1);
  }
}

function uninitializedSubmodules() {
  const r = spawnSync('git', ['submodule', 'status'], { cwd: join(V2, '..'), encoding: 'utf8' });
  if (r.status !== 0) return [];
  return r.stdout.split('\n').filter((l) => l.startsWith('-')).map((l) => l.slice(1).trim().split(/\s+/)[1]).filter(Boolean);
}

const sha256 = (path) => createHash('sha256').update(readFileSync(path)).digest('hex');

function install(src, dir, name) {
  mkdirSync(join(ROOT, dir), { recursive: true });
  const dest = join(ROOT, dir, name);
  copyFileSync(src, dest);
  const digest = sha256(dest);
  // Merge: one line per artifact, so several platform addons can share native/.
  const sums = join(ROOT, dir, 'SHA256SUMS');
  const kept = existsSync(sums)
    ? readFileSync(sums, 'utf8').split('\n').filter((l) => l.trim() && !l.trim().endsWith(`  ${name}`))
    : [];
  writeFileSync(sums, [...kept, `${digest}  ${name}`].sort((a, b) => a.slice(66).localeCompare(b.slice(66))).join('\n') + '\n');
  return { artifact: `${dir}/${name}`, sha256: digest };
}

/** Fail fast when the toolchain v2/ pins lacks the wasm32 target. */
function requireWasmTarget() {
  const r = spawnSync('rustup', ['target', 'list', '--installed'], { cwd: V2, encoding: 'utf8' });
  if (r.status === 0 && !r.stdout.split(/\r?\n/).includes('wasm32-unknown-unknown')) {
    console.error('wasm32-unknown-unknown is not installed for the toolchain pinned by v2/rust-toolchain.');
    console.error('Fix: (cd v2 && rustup target add wasm32-unknown-unknown)');
    process.exit(1);
  }
}

const built = [];
if (wantWasm) {
  requireWasmTarget();
  cargo(['build', '-p', 'ruview-kernel', '--release', '--target', 'wasm32-unknown-unknown', '--locked'], V2);
  const src = join(V2, 'target', 'wasm32-unknown-unknown', 'release', 'ruview_kernel.wasm');
  built.push(install(src, 'wasm', 'ruview_kernel.wasm'));
}
if (wantNapi) {
  const crate = join(V2, 'crates', 'ruview-kernel-napi');
  cargo(['build', '--release', '--locked', '--manifest-path', join(crate, 'Cargo.toml')], V2);
  const lib = { win32: 'ruview_kernel_napi.dll', darwin: 'libruview_kernel_napi.dylib' }[process.platform] || 'libruview_kernel_napi.so';
  const src = join(crate, 'target', 'release', lib);
  built.push(install(src, 'native', `ruview_kernel.${platformTriple()}.node`));
}
console.log(JSON.stringify({ ok: true, built }, null, 2));
