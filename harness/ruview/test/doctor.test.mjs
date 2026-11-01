import test from 'node:test';
import assert from 'node:assert/strict';
import { formatDoctor, runDoctor, verifyPackageManifest } from '../src/doctor.js';
import { runTool } from '../src/tools.js';

const offline = (overrides = {}) => ({
  which: () => null,
  findRepoRoot: () => null,
  python: () => null,
  exec: async () => ({ ok: false, code: 1, stdout: '', stderr: '', error: 'not run' }),
  importer: async () => { throw new Error('absent'); },
  fetch: async () => { throw new Error('offline'); },
  ...overrides,
});

test('bare machine: optional tooling warns with remedies, nothing fails', async () => {
  const report = await runDoctor({}, offline());
  assert.equal(report.ok, true);
  assert.ok(report.summary.warn > 0);
  for (const c of report.checks.filter((x) => x.status === 'warn')) assert.ok(c.remedy, `${c.id} needs a remedy`);
  assert.ok(report.nextSteps.length > 0);
  assert.match(formatDoctor(report), /ruview doctor: no failures/);
});

test('groups filter checks and a bad port is a failure', async () => {
  const report = await runDoctor({ groups: ['serial'], port: 'COM7;x' }, offline({
    python: () => '/usr/bin/python3',
    exec: async () => ({ ok: true, code: 0, stdout: '/dev/ttyUSB0 CP2102 USB to UART\n', stderr: '', error: null }),
  }));
  assert.deepEqual([...new Set(report.checks.map((c) => c.group))], ['serial']);
  assert.equal(report.checks.find((c) => c.id === 'port').status, 'fail');
  assert.equal(report.ok, false);
});

test('probe reports the detected chip; unreachable sensing server fails', async () => {
  const deps = offline({
    python: () => '/usr/bin/python3',
    exec: async (cmd, args) => (args.includes('chip_id')
      ? { ok: true, code: 0, stdout: 'Chip is ESP32-S3', stderr: '', error: null }
      : { ok: true, code: 0, stdout: '/dev/ttyUSB0 CP2102\n', stderr: '', error: null }),
  });
  const r = await runDoctor({ groups: ['serial', 'sensing'], port: '/dev/ttyUSB0', probe_port: true, sensing_url: 'http://127.0.0.1:9' }, deps);
  assert.equal(r.checks.find((c) => c.id === 'chip').detail, 'detected ESP32-S3');
  assert.equal(r.checks.find((c) => c.id === 'server').status, 'fail');
  const bad = await runDoctor({ groups: ['sensing'], sensing_url: 'file:///etc/passwd' }, offline());
  assert.match(bad.checks[0].detail, /http\(s\)/);
});

test('package manifest verification is exposed and consistent', () => {
  const r = verifyPackageManifest();
  assert.equal(typeof r.ok, 'boolean');
  assert.ok(r.files > 0);
});

test('MCP doctor cannot probe hardware or reach the network', async () => {
  for (const args of [{ probe_port: true }, { sensing_url: 'http://example.com' }]) {
    assert.equal((await runTool('ruview_doctor', args, { source: 'mcp' })).reason, 'invalid_arguments');
  }
  const r = await runTool('ruview_doctor', { groups: ['runtime'] }, { source: 'mcp', grants: [] });
  assert.equal(r.checks[0].id, 'node-version');
});
