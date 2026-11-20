// SPDX-License-Identifier: MIT
// Model training runner and evidence gate (ADR-371).
//
// Two concerns, deliberately separate:
//   1. `trainingGate` — a pure, read-only honesty gate for pose/count results:
//      a number is publishable only as a delta over the mean-pose baseline on a
//      leakage-free held-out split (CLAUDE.md, ADR-181). It never trains.
//   2. `runTraining` — delegates to the repository's real trainers
//      (wifi-densepose train-room, wifi-densepose-train `train`), writes only
//      under the trusted checkout, and fails closed when a toolchain is absent.

import { existsSync, mkdirSync, realpathSync } from 'node:fs';
import { isAbsolute, join, relative, resolve } from 'node:path';

export const SPLITS = Object.freeze(['chronological', 'blocked-gap', 'grouped-subject', 'grouped-session', 'random-frame']);
const LEAKAGE_FREE = new Set(['chronological', 'blocked-gap', 'grouped-subject', 'grouped-session']);
export const TRAIN_MODES = Object.freeze(['pose-smoke', 'pose', 'room']);

const isNum = (v) => typeof v === 'number' && Number.isFinite(v);
const toPct = (v) => (v <= 1 ? v * 100 : v);
const timeMillis = (value) => isNum(value) ? value : typeof value === 'string' ? Date.parse(value) : NaN;

/**
 * Evaluate a training report. Fields:
 *   metric ('pck@20' etc.), model_score, baseline_score (mean-pose, same split),
 *   split, train_subjects[], test_subjects[], train_end, test_start (ISO or
 *   number), n_test, reproducer (command), data ('real'|'synthetic').
 */
export function trainingGate(report = {}) {
  const findings = [];
  const fail = (code, message) => findings.push({ severity: 'fail', code, message });
  const warn = (code, message) => findings.push({ severity: 'warn', code, message });

  if (!isNum(report.model_score)) fail('model_score_missing', 'model_score is required.');
  if (!isNum(report.baseline_score)) fail('baseline_missing', 'baseline_score (mean-pose baseline on the same split) is required before any score is quoted.');
  if (!SPLITS.includes(report.split)) fail('split_missing', `split must be one of ${SPLITS.join(', ')}.`);
  else if (!LEAKAGE_FREE.has(report.split)) fail('split_leaky', `${report.split} splits leak temporally adjacent frames; use chronological, blocked-gap, or grouped splits.`);

  const train = Array.isArray(report.train_subjects) ? report.train_subjects.map(String) : null;
  const test = Array.isArray(report.test_subjects) ? report.test_subjects.map(String) : null;
  if (train && test) {
    const overlap = test.filter((s) => train.includes(s));
    if (overlap.length && report.split === 'grouped-subject') fail('subject_leakage', `subjects in both train and test: ${overlap.slice(0, 10).join(', ')}`);
    else if (overlap.length) warn('subject_overlap', `test shares ${overlap.length} subject(s) with train; results do not show cross-person generalization.`);
  } else warn('subjects_unreported', 'train_subjects/test_subjects not reported; cross-person generalization is unknown.');

  if (report.split === 'chronological' || report.split === 'blocked-gap') {
    const end = timeMillis(report.train_end);
    const start = timeMillis(report.test_start);
    if (!Number.isFinite(end) || !Number.isFinite(start)) fail('time_bounds_missing', 'chronological splits need train_end and test_start.');
    else if (start <= end) fail('temporal_leakage', 'test_start must be after train_end.');
  }
  if (!isNum(report.n_test) || report.n_test < 100) warn('small_test_set', 'n_test is missing or below 100 samples; confidence intervals will be wide.');

  let deltaPp = null;
  if (isNum(report.model_score) && isNum(report.baseline_score)) {
    deltaPp = Number((toPct(report.model_score) - toPct(report.baseline_score)).toFixed(2));
    if (deltaPp <= 0) fail('no_signal', `model does not beat the mean-pose baseline (${deltaPp} pp).`);
    if (toPct(report.model_score) >= 99.5) warn('implausible_score', 'near-perfect scores historically indicated leakage (retracted 92.9%/100% results); audit the split.');
  }

  const failed = findings.some((f) => f.severity === 'fail');
  const synthetic = report.data === 'synthetic';
  const evidence = failed ? null : synthetic ? 'SYNTHETIC' : typeof report.reproducer === 'string' && report.reproducer.trim() ? 'MEASURED' : 'CLAIMED';
  const metric = String(report.metric || 'score');
  return {
    ok: !failed,
    verdict: failed ? 'FAIL' : 'PASS',
    evidence,
    deltaPp,
    findings,
    statement: failed ? null
      : `Held-out ${metric} ${toPct(report.model_score).toFixed(1)}% vs ${toPct(report.baseline_score).toFixed(1)}% mean-pose baseline = +${deltaPp} pp (${evidence}${evidence === 'MEASURED' ? `, reproducer: ${report.reproducer}` : ''}, ${report.split} split).`,
  };
}

function within(root, path) {
  const rel = relative(root, path);
  return rel === '' || (!rel.startsWith('..') && !isAbsolute(rel));
}

/** Resolve a caller path that must stay inside the trusted checkout. */
export function repoPath(repo, value, { mustExist = false } = {}) {
  if (value === undefined) return undefined;
  if (typeof value !== 'string' || !value.trim()) throw Object.assign(new Error('path must be a non-empty string'), { reason: 'invalid_path' });
  const abs = resolve(repo, value);
  if (!within(repo, abs)) throw Object.assign(new Error(`${value} is outside the RuView checkout`), { reason: 'path_outside_repo' });
  if (mustExist) {
    if (!existsSync(abs)) throw Object.assign(new Error(`${value} does not exist`), { reason: 'path_missing' });
    if (!within(realpathSync(repo), realpathSync(abs))) throw Object.assign(new Error(`${value} resolves outside the checkout`), { reason: 'path_outside_repo' });
  }
  return abs;
}

/** Build the concrete command for a training mode (no execution). */
export function trainingCommand(args, { repo, wifiDensepose }) {
  const mode = args.mode || 'pose-smoke';
  if (!TRAIN_MODES.includes(mode)) throw Object.assign(new Error(`mode must be one of ${TRAIN_MODES.join(', ')}`), { reason: 'invalid_mode' });
  if (mode === 'room') {
    const passthru = [];
    const enrollment = repoPath(repo, args.enrollment, { mustExist: true });
    const output = repoPath(repo, args.output);
    if (enrollment) passthru.push('--enrollment', enrollment);
    if (output) passthru.push('--output', output);
    return wifiDensepose
      ? { mode, cmd: wifiDensepose, args: ['train-room', ...passthru], cwd: repo, timeoutMs: 1_800_000, writes: [output ?? join(repo, 'room-bank.json')] }
      : { mode, cmd: 'cargo', args: ['run', '--release', '-q', '-p', 'wifi-densepose-cli', '--', 'train-room', ...passthru], cwd: repo, timeoutMs: 3_600_000, writes: [output ?? join(repo, 'room-bank.json')] };
  }
  const trainArgs = [];
  const config = repoPath(repo, args.config, { mustExist: true });
  const dataDir = repoPath(repo, args.data_dir, { mustExist: true });
  const checkpoints = repoPath(repo, args.checkpoint_dir) ?? join(repo, 'v2', 'target', 'ruview-train', mode);
  if (config) trainArgs.push('--config', config);
  if (mode === 'pose-smoke') trainArgs.push('--dry-run', '--dry-run-samples', String(args.samples ?? 64));
  else {
    if (!dataDir) throw Object.assign(new Error('pose mode requires data_dir (MM-Fi recordings inside the checkout)'), { reason: 'data_dir_required' });
    trainArgs.push('--data-dir', dataDir);
  }
  trainArgs.push('--checkpoint-dir', checkpoints);
  if (args.cuda === true) trainArgs.push('--cuda');
  return {
    mode,
    cmd: 'cargo',
    // --manifest-path + cwd=checkpoint dir: the trainer's relative `logs/`
    // output lands next to the checkpoints, never in the source tree.
    args: ['run', '--release', '--manifest-path', join(repo, 'v2', 'Cargo.toml'), '-p', 'wifi-densepose-train', '--features', args.cuda ? 'cuda' : 'tch-backend', '--bin', 'train', '--', ...trainArgs],
    cwd: checkpoints,
    timeoutMs: mode === 'pose-smoke' ? 1_800_000 : 43_200_000,
    writes: [checkpoints],
  };
}

/** Plan (default) or run training. deps = { exec, findRepoRoot, which }. */
export async function runTraining(args, deps) {
  const repo = args.repo ? resolve(args.repo) : deps.findRepoRoot();
  if (!repo || !existsSync(join(repo, 'v2', 'Cargo.toml'))) {
    return { ok: false, reason: 'not_in_ruview_repo', remedy: 'Training builds the Rust trainers; run inside a RuView checkout or pass repo.' };
  }
  let plan;
  try {
    plan = trainingCommand(args, { repo, wifiDensepose: deps.which('wifi-densepose') });
  } catch (error) {
    return { ok: false, reason: error.reason || 'invalid_arguments', detail: error.message };
  }
  const summary = { mode: plan.mode, command: [plan.cmd, ...plan.args], cwd: plan.cwd, writes: plan.writes };
  if (args.confirm !== true) {
    return { ok: true, dryRun: true, plan: summary, next: 'Re-run with confirm to train. Nothing was executed.', gate: 'Publish results only through ruview_train_gate.' };
  }
  if (plan.cmd === 'cargo' && !deps.which('cargo')) return { ok: false, reason: 'cargo_missing', plan: summary, remedy: 'Install Rust via https://rustup.rs' };
  for (const dir of plan.writes) if (isAbsolute(dir) && within(repo, dir)) mkdirSync(dir, { recursive: true });
  const r = await deps.exec(plan.cmd, plan.args, { cwd: plan.cwd, timeoutMs: plan.timeoutMs });
  const text = `${r.stdout}\n${r.stderr}`;
  const hint = r.ok ? null : /Cannot find a libtorch install/i.test(text)
    ? 'libtorch not found: set LIBTORCH, or LIBTORCH_USE_PYTORCH=1 with the PyTorch version tch expects installed (tch 0.24 expects torch 2.11; see the torch-sys build script).'
    : /torch_api(?:_generated)?\.cpp|version mismatch|LIBTORCH_BYPASS_VERSION_CHECK/i.test(text)
      ? 'libtorch version does not match the tch crate (tch 0.24 expects torch 2.11); install that version instead of bypassing the check.'
      : /libtorch|torch-sys|LIBTORCH/i.test(text)
        ? 'libtorch build/link failed: check LIBTORCH / LD_LIBRARY_PATH (DYLD_LIBRARY_PATH on macOS).'
        : /requires the `tch-backend`/i.test(text) ? 'Build with the tch-backend feature.' : null;
  return {
    ok: r.ok,
    dryRun: false,
    plan: summary,
    exit: r.code,
    tail: r.stdout.slice(-2000),
    stderr: r.stderr.slice(-1200),
    ...(hint ? { remedy: hint } : {}),
    evidence: plan.mode === 'pose-smoke' ? 'SYNTHETIC (dry-run dataset)' : 'Unverified until evaluated with ruview_train_gate against the mean-pose baseline.',
  };
}
