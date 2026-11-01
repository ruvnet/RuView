import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runTraining, trainingCommand, trainingGate } from '../src/training.js';
import { runTool } from '../src/tools.js';

const honest = {
  metric: 'PCK@20', model_score: 0.595, baseline_score: 0.501, split: 'chronological',
  train_end: '2026-03-01T00:00:00Z', test_start: '2026-03-02T00:00:00Z',
  train_subjects: ['s1', 's2'], test_subjects: ['s3'], n_test: 800, reproducer: 'python eval.py --split chrono',
};

test('gate passes an honest baseline-relative, leakage-free result as MEASURED', () => {
  const r = trainingGate(honest);
  assert.equal(r.verdict, 'PASS');
  assert.equal(r.evidence, 'MEASURED');
  assert.equal(r.deltaPp, 9.4);
  assert.match(r.statement, /\+9\.4 pp \(MEASURED/);
});

test('gate fails the retracted-number patterns', () => {
  const codes = (report) => trainingGate(report).findings.filter((f) => f.severity === 'fail').map((f) => f.code);
  assert.ok(codes({ model_score: 0.929 }).includes('baseline_missing'));
  assert.ok(codes({ ...honest, split: 'random-frame' }).includes('split_leaky'));
  assert.ok(codes({ ...honest, split: 'grouped-subject', test_subjects: ['s1'] }).includes('subject_leakage'));
  assert.ok(codes({ ...honest, test_start: '2026-02-01T00:00:00Z' }).includes('temporal_leakage'));
  assert.ok(codes({ ...honest, model_score: 0.49 }).includes('no_signal'));
  assert.ok(trainingGate({ ...honest, model_score: 1.0 }).findings.some((f) => f.code === 'implausible_score'));
});

test('gate evidence tag follows the data source and reproducer', () => {
  assert.equal(trainingGate({ ...honest, reproducer: undefined }).evidence, 'CLAIMED');
  assert.equal(trainingGate({ ...honest, data: 'synthetic' }).evidence, 'SYNTHETIC');
});

function fakeRepo() {
  const repo = mkdtempSync(join(tmpdir(), 'ruview-repo-'));
  mkdirSync(join(repo, 'v2'), { recursive: true });
  writeFileSync(join(repo, 'v2', 'Cargo.toml'), '[workspace]\n');
  mkdirSync(join(repo, 'data', 'mmfi'), { recursive: true });
  writeFileSync(join(repo, 'enrollment.json'), '{}');
  return repo;
}

test('training commands stay inside the checkout', () => {
  const repo = fakeRepo();
  try {
    const smoke = trainingCommand({}, { repo });
    assert.deepEqual(smoke.args.slice(0, 11), ['run', '--release', '--manifest-path', join(repo, 'v2', 'Cargo.toml'), '-p', 'wifi-densepose-train', '--features', 'tch-backend', '--bin', 'train', '--']);
    assert.equal(smoke.cwd, join(repo, 'v2', 'target', 'ruview-train', 'pose-smoke'), 'trainer logs stay out of the source tree');
    assert.ok(smoke.args.includes('--dry-run'));
    const pose = trainingCommand({ mode: 'pose', data_dir: 'data/mmfi', cuda: true }, { repo });
    assert.ok(pose.args.includes('cuda') && pose.args.includes('--cuda'));
    const room = trainingCommand({ mode: 'room', enrollment: 'enrollment.json' }, { repo, wifiDensepose: '/usr/bin/wifi-densepose' });
    assert.deepEqual(room.args.slice(0, 3), ['train-room', '--enrollment', join(repo, 'enrollment.json')]);
    assert.throws(() => trainingCommand({ mode: 'pose', data_dir: '../../etc' }, { repo }), (e) => e.reason === 'path_outside_repo');
    assert.throws(() => trainingCommand({ mode: 'pose' }, { repo }), (e) => e.reason === 'data_dir_required');
    assert.throws(() => trainingCommand({ mode: 'room', enrollment: 'missing.json' }, { repo }), (e) => e.reason === 'path_missing');
    assert.throws(() => trainingCommand({ checkpoint_dir: '/tmp/out' }, { repo }), (e) => e.reason === 'path_outside_repo');
  } finally { rmSync(repo, { recursive: true, force: true }); }
});

test('training runs only with confirm and reports libtorch problems with a remedy', async () => {
  const repo = fakeRepo();
  const calls = [];
  const deps = {
    findRepoRoot: () => repo,
    which: (b) => (b === 'cargo' ? '/usr/bin/cargo' : null),
    exec: async (cmd, args, opts) => { calls.push({ cmd, args, cwd: opts.cwd }); return { ok: false, code: 101, stdout: '', stderr: 'error: failed to run custom build command for `torch-sys`\nLIBTORCH not set', error: 'exit 101' }; },
  };
  try {
    const plan = await runTraining({}, deps);
    assert.equal(plan.dryRun, true);
    assert.equal(calls.length, 0);
    const run = await runTraining({ confirm: true }, deps);
    assert.equal(run.ok, false);
    assert.equal(calls[0].cwd, join(repo, 'v2', 'target', 'ruview-train', 'pose-smoke'));
    assert.match(run.remedy, /libtorch/i);
    const none = await runTraining({}, { ...deps, findRepoRoot: () => null });
    assert.equal(none.reason, 'not_in_ruview_repo');
  } finally { rmSync(repo, { recursive: true, force: true }); }
});

test('MCP training requires workspace-write + confirm; plan and gate are read-only', async () => {
  assert.equal((await runTool('ruview_train', { confirm: true }, { source: 'mcp', grants: [] })).reason, 'authority_denied');
  assert.equal((await runTool('ruview_train', {}, { source: 'mcp', grants: ['workspace-write'] })).reason, 'not_confirmed');
  const gate = await runTool('ruview_train_gate', honest, { source: 'mcp', grants: [] });
  assert.equal(gate.verdict, 'PASS');
  assert.equal((await runTool('ruview_train_plan', { confirm: true }, { source: 'mcp' })).reason, 'invalid_arguments');
});
