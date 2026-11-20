import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, writeFileSync, readFileSync, unlinkSync, mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
const packageRoot = fileURLToPath(new URL('..', import.meta.url));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const offline = { findRepoRoot: () => null, which: () => null, python: () => null };
async function copyPackage(t, damage) {
  const root = mkdtempSync(join(tmpdir(), 'ruview-doctor-manifest-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const entry of ['src', 'brain', 'package.json']) cpSync(join(packageRoot, entry), join(root, entry), { recursive: true });
  cpSync(join(packageRoot, '.harness'), join(root, '.harness'), { recursive: true });
  writeFileSync(join(root, 'integrity-data.txt'), 'owned fixture');
  const raw = JSON.stringify({ files: { 'package.json': sha(readFileSync(join(root, 'package.json'))), 'integrity-data.txt': sha('owned fixture') } });
  writeFileSync(join(root, '.harness', 'manifest.json'), raw);
  writeFileSync(join(root, '.harness', 'manifest.sha256'), sha(raw));
  if (damage === 'json') writeFileSync(join(root, '.harness', 'manifest.json'), '{"schema":2,');
  if (damage === 'checksum') unlinkSync(join(root, '.harness', 'manifest.sha256'));
  if (damage === 'checksum-directory') { unlinkSync(join(root, '.harness', 'manifest.sha256')); mkdirSync(join(root, '.harness', 'manifest.sha256')); }
  if (damage === 'data-directory') { unlinkSync(join(root, 'integrity-data.txt')); mkdirSync(join(root, 'integrity-data.txt')); }
  const { createRuView } = await import(pathToFileURL(join(root, 'src', 'sdk.js')).href);
  return createRuView({ strict: true, deps: offline }).doctor({ groups: ['harness'] });
}

test('public SDK doctor reports malformed manifest as a structured integrity warning', async t => {
  const report = await copyPackage(t, 'json');
  const check = report.checks.find(c => c.id === 'package-integrity');
  assert.equal(check.status, 'warn');
  assert.match(check.detail, /manifest\.json/);
  assert.match(check.remedy, /Reinstall/);
  assert.equal(report.ok, true, 'existing integrity warning policy is preserved');
});

test('public SDK doctor reports a missing checksum file without throwing', async t => {
  const report = await copyPackage(t, 'checksum');
  const check = report.checks.find(c => c.id === 'package-integrity');
  assert.equal(check.status, 'warn');
  assert.match(check.detail, /manifest\.sha256.*missing/);
  assert.equal(report.ok, true);
});

test('valid copied package control keeps successful integrity diagnostics', async t => {
  const report = await copyPackage(t);
  assert.equal(report.checks.find(c => c.id === 'package-integrity').status, 'pass');
  assert.equal(report.ok, true);
});

for (const [damage, pattern] of [['checksum-directory', /manifest\.sha256:unreadable/], ['data-directory', /integrity-data\.txt:unreadable/]]) {
  test(`public SDK doctor reports unreadable ${damage} as a warning`, async t => {
    const report = await copyPackage(t, damage);
    const check = report.checks.find(c => c.id === 'package-integrity');
    assert.equal(check.status, 'warn');
    assert.match(check.detail, pattern);
    assert.equal(report.ok, true);
  });
}
