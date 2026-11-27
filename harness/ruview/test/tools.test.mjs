// SPDX-License-Identifier: MIT
// RuView harness tests — Node's built-in test runner (no devDeps to install).
// Run: `node --test test/*.test.mjs`  (or `npm test`).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join, dirname, delimiter } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { claimCheck, summarize } from '../src/guardrails.js';
import { TOOLS, TOOL_ALIASES, runTool, listTools, findRepoRoot, run, which } from '../src/tools.js';
import { run as cliRun } from '../bin/cli.js';

const PKG_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

test('guardrail flags the retracted 100% framing as high severity', () => {
  const r = claimCheck('Our model reaches 100% accuracy on every pose.');
  assert.equal(r.ok, false);
  assert.ok(r.findings.some((f) => f.severity === 'high'));
});

test('guardrail flags an untagged percentage accuracy claim', () => {
  // "hit", not "measured" — "measured" would (correctly) route to the no-reproducer branch.
  const r = claimCheck('We hit 92.9% PCK on the test set.');
  assert.equal(r.ok, false);
  assert.ok(r.findings.some((f) => /not tagged/i.test(f.reason)));
});

test('guardrail passes a MEASURED claim that cites a reproducer', () => {
  const r = claimCheck('Held-out PCK@20 59.5% vs 50% mean-pose baseline = +9.4pp (MEASURED, verify.py).');
  assert.equal(r.ok, true, JSON.stringify(r.findings));
});

test('guardrail flags MEASURED with no reproducer', () => {
  const r = claimCheck('Presence detection 97% (MEASURED).');
  assert.equal(r.ok, false);
  assert.ok(r.findings.some((f) => /no reproducer/i.test(f.reason)));
});

test('guardrail ignores non-metric prose', () => {
  assert.equal(claimCheck('The ESP32 streams CSI over UDP to the sensing-server.').ok, true);
  assert.equal(claimCheck('').ok, true);
});

// ADR-263 F11/O9: precision pins — short metric tokens must not fire on prose.
test('guardrail does not false-positive on "map"/"F1" prose (ADR-263 F11)', () => {
  assert.equal(claimCheck('F-numbers map to findings.').ok, true);
  assert.equal(claimCheck('### F1 (HIGH, broken export): `require` points at a missing file').ok, true);
  assert.equal(claimCheck('The 0.1.0 tarball ships 44 `.map` files = 62,698 B of dead weight.').ok, true);
  assert.equal(claimCheck('the source maps can never resolve').ok, true);
  assert.equal(claimCheck('- **O1 (F1):** fix `exports` (see F2 for the 33% map weight — MEASURED, tarball listing)').ok, true);
  assert.equal(claimCheck('ADR-264: exports fix, map-free tarball, session-per-transport').ok, true);
});

test('guardrail still catches real short-token metric claims', () => {
  assert.equal(claimCheck('We reach mAP 62.3 on COCO.').ok, false);
  assert.equal(claimCheck('F1 score of 0.91 on the held set.').ok, false, 'f1 with a real score must still fire');
  assert.equal(claimCheck('IoU 0.75 across rooms.').ok, false);
});

// Digits hidden in a code span still make a claim — scrubbing must not blind the
// number gate to `0.95` (regression: code-span number bypassed the gate).
test('guardrail flags an accuracy number stated inside a code span', () => {
  const r = claimCheck('Count accuracy reached `0.95` in our tests.');
  assert.equal(r.ok, false, JSON.stringify(r.findings));
  assert.ok(r.findings.some((f) => /not tagged/i.test(f.reason)));
});

// A MEASURED claim whose only number hides in a code span must still reach the
// missing-reproducer check (regression: the scrubbed gate short-circuited it).
// Bare metric prose with no number at all (e.g. the README rule text) stays a pass.
test('guardrail flags a MEASURED code-span number with no reproducer', () => {
  const r = claimCheck('Detection accuracy `0.97` on the set (MEASURED).');
  assert.equal(r.ok, false, JSON.stringify(r.findings));
  assert.ok(r.findings.some((f) => /no reproducer/i.test(f.reason)));
  assert.equal(claimCheck('Every accuracy number must be MEASURED against a baseline.').ok, true);
});

// F1-score phrasings ("F1: 0.91", "F1 reaches 0.91") were scrubbed as option
// labels and slipped through; option refs alone must still not false-positive.
test('guardrail catches F1-score claims but not bare option refs (ADR-263 F11)', () => {
  assert.equal(claimCheck('F1: 0.91 on the held-out set.').ok, false, 'F1: value is a metric claim');
  assert.equal(claimCheck('F1 reaches 0.91 on the held-out set.').ok, false, 'F1 with a nearby number is a claim');
  assert.equal(claimCheck('Options O1–O9 are tracked in ADR-263 O2.').ok, true, 'option labels are not metrics');
  assert.equal(claimCheck('ADR-263 O2 lands the exports fix.').ok, true);
});

test('summarize gives PASS/finding text', () => {
  assert.match(summarize(claimCheck('nothing here')), /PASS/);
  assert.match(summarize(claimCheck('100% accuracy')), /finding/);
});

test('registry exposes the documented tools with schemas (underscore-canonical)', () => {
  const names = Object.keys(TOOLS);
  for (const n of ['ruview_onboard', 'ruview_claim_check', 'ruview_verify', 'ruview_node_monitor', 'ruview_calibrate', 'ruview_node_flash', 'ruview_guidance', 'ruview_spaces_list', 'ruview_memory_search', 'ruview_kernel_selftest', 'ruview_doctor', 'ruview_firmware_plan', 'ruview_firmware_ports', 'ruview_train', 'ruview_train_plan', 'ruview_train_gate', 'ruview_devices_scan', 'ruview_esp32_capture', 'ruview_mmwave_read', 'ruview_lidar_read', 'ruview_host_list', 'ruview_host_run']) {
    assert.ok(names.includes(n), `missing ${n}`);
    assert.equal(TOOLS[n].inputSchema.type, 'object');
    assert.match(n, /^[a-zA-Z0-9_-]{1,64}$/, 'canonical names must satisfy host tool-name regexes');
  }
  assert.equal(listTools().length, names.length);
});

test('dotted legacy names resolve via aliases (ADR-263 O8)', async () => {
  assert.equal(TOOL_ALIASES['ruview.claim_check'], 'ruview_claim_check');
  assert.equal(TOOL_ALIASES['ruview.node_monitor'], 'ruview_node_monitor');
  const r = await runTool('ruview.onboard', {});
  assert.equal(r.ok, true);
});

test('ruview_onboard returns paths and a recommendation', async () => {
  const r = await runTool('ruview_onboard', {});
  assert.equal(r.ok, true);
  assert.ok(r.paths['live-esp32']);
  assert.ok(['repo-build', 'docker-demo'].includes(r.recommend));
});

test('ruview_claim_check tool wraps the guardrail', async () => {
  const r = await runTool('ruview_claim_check', { text: '100% accuracy' });
  assert.equal(r.ok, false);
  assert.match(r.summary, /honesty|tag|MEASURED|finding/i);
});

// ADR-263 F1/O1: the honesty gate must fail closed on empty input.
test('ruview_claim_check fails closed on empty/missing text', async () => {
  const empty = await runTool('ruview_claim_check', { text: '' });
  assert.equal(empty.ok, false);
  assert.equal(empty.reason, 'empty_text');
  const missing = await runTool('ruview_claim_check', {});
  assert.equal(missing.ok, false);
  assert.equal(missing.reason, 'invalid_arguments');
});

test('unknown tool fails closed', async () => {
  const r = await runTool('ruview_does_not_exist', {});
  assert.equal(r.ok, false);
  assert.equal(r.reason, 'unknown_tool');
});

test('node_monitor fails closed without a port', async () => {
  const r = await runTool('ruview_node_monitor', {});
  assert.equal(r.ok, false);
  assert.equal(r.reason, 'no_port');
});

test('node_flash needs a bundle and never writes without confirm (mutating guard)', async () => {
  const r = await runTool('ruview_node_flash', { port: 'COM8', variant: 's3-8mb' });
  assert.equal(r.ok, false);
  assert.equal(r.reason, 'invalid_arguments');
  const plan = await runTool('ruview_node_flash', { port: 'COM8', bundle: '/definitely/missing' });
  assert.equal(plan.ok, false);
  assert.equal(plan.reason, 'bundle_missing');
});

test('verify fails closed when not in a RuView repo', async () => {
  // point at a tmp dir with no repo markers
  const r = await runTool('ruview_verify', { repo: process.platform === 'win32' ? 'C:/Windows/Temp' : '/tmp' });
  assert.equal(r.ok, false);
  assert.ok(['proof_missing', 'python_missing'].includes(r.reason), r.reason);
});

// ADR-263 F2/O2: registry-level concurrency — a slow child must not block
// other tool calls (run() is promise-based, never spawnSync).
test('run() is non-blocking: a fast tool completes while a slow child runs', async () => {
  const slow = run('node', ['-e', 'setTimeout(() => {}, 2000)'], { timeout: 5000 });
  const t0 = Date.now();
  const fast = await runTool('ruview_onboard', {});
  const elapsed = Date.now() - t0;
  assert.equal(fast.ok, true);
  assert.ok(elapsed < 1000, `onboard took ${elapsed} ms while a 2 s child was running`);
  const r = await slow;
  assert.equal(r.ok, true);
});

test('run() reports a timeout as a failure, not a hang', async () => {
  const r = await run('node', ['-e', 'setTimeout(() => {}, 10000)'], { timeout: 300 });
  assert.equal(r.ok, false);
  assert.match(String(r.error), /timed out/);
});

test('run() bounds captured output instead of dying on big streams (ADR-263 O4)', async () => {
  // 4 MiB of stdout would have hit spawnSync's 1 MiB default maxBuffer (ENOBUFS).
  const r = await run('node', ['-e', "process.stdout.write('x'.repeat(4 * 1024 * 1024)); console.log('TAIL_MARKER')"], { timeout: 30000 });
  assert.equal(r.ok, true);
  assert.ok(r.stdout.length <= 65536, `tail not bounded: ${r.stdout.length}`);
  assert.ok(r.stdout.includes('TAIL_MARKER'), 'tail must keep the end of the stream');
});

test('which() finds node and re-probes misses (hits are cached)', () => {
  assert.ok(which('node'), 'node must be on PATH in the test env');
  assert.equal(which('definitely-not-a-binary-xyz'), null);
  assert.equal(which('definitely-not-a-binary-xyz'), null); // re-probed, still absent
});

// ADR-263 O8: a miss must not be cached — an operator who installs a tool
// mid-session (e.g. python after a python_missing failure) must be found next call.
test('which() re-probes after a miss so a newly-installed tool is found', () => {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-which-'));
  const name = 'ruview-probe-xyz';
  const isWin = process.platform === 'win32';
  const bin = join(dir, isWin ? `${name}.cmd` : name);
  const prevPath = process.env.PATH;
  try {
    assert.equal(which(name), null, 'not on PATH yet → miss');
    writeFileSync(bin, isWin ? '@echo off\n' : '#!/bin/sh\n', { mode: 0o755 });
    process.env.PATH = dir + delimiter + prevPath;
    assert.ok(which(name), 'installed mid-session → the miss must not have been cached');
  } finally {
    process.env.PATH = prevPath;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('CLI run(): claim-check exits non-zero on a bad claim', async () => {
  const code = await cliRun(['claim-check', '--text', '100% accuracy']);
  assert.notEqual(code, 0);
});

// ADR-263 F1/O1: the CLI must not PASS silently with no input.
test('CLI run(): claim-check with no input exits 2 (fail-closed)', async () => {
  assert.equal(await cliRun(['claim-check']), 2);
  assert.equal(await cliRun(['claim-check', '--text', '   ']), 2);
});

test('CLI run(): doctor exits 0 (tools-only path)', async () => {
  const code = await cliRun(['doctor']);
  assert.equal(code, 0);
});

test('CLI run(): unknown command exits non-zero', async () => {
  assert.notEqual(await cliRun(['definitely-not-a-command']), 0);
});

test('findRepoRoot locates this monorepo from cwd and memoizes the answer', () => {
  // when run from within wifi-densepose, it should find a root; elsewhere null is fine
  const root = findRepoRoot();
  assert.ok(root === null || typeof root === 'string');
  assert.equal(findRepoRoot(), root);
  assert.equal(findRepoRoot(process.cwd()), root);
});

// ADR-263 F7/O7: skills ship from one source; the projected copies must match.
test('.claude/skills/*/SKILL.md are byte-identical to skills/*.md', () => {
  const srcDir = join(PKG_ROOT, 'skills');
  for (const f of readdirSync(srcDir).filter((f) => f.endsWith('.md'))) {
    const name = f.replace(/\.md$/, '');
    const src = readFileSync(join(srcDir, f), 'utf8');
    const projected = readFileSync(join(PKG_ROOT, '.claude', 'skills', name, 'SKILL.md'), 'utf8');
    assert.equal(projected, src, `skill drift: ${name} — run \`npm run sync-skills\``);
  }
});

// ADR-263 F6/O6 + F3/O3: package hygiene pins.
test('package.json has no optionalDependencies and no hardcoded server version drift', () => {
  const pkg = JSON.parse(readFileSync(join(PKG_ROOT, 'package.json'), 'utf8'));
  assert.equal(pkg.optionalDependencies, undefined, 'ADR-263 O3: optional deps tripled the cold npx install');
  assert.equal(pkg.dependencies, undefined, 'the harness is dependency-free by design');
  const mcpSrc = readFileSync(join(PKG_ROOT, 'src', 'mcp-server.js'), 'utf8');
  assert.ok(!/version:\s*'\d+\.\d+\.\d+'/.test(mcpSrc), 'ADR-263 O6: server version must come from package.json');
});
