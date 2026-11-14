import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, realpathSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { runClaudeCode, buildClaudeCodeArgs } from '../src/hosts/claude-code.js';
import { runCodex, buildCodexArgs } from '../src/hosts/codex.js';
import { runProcess, scrubEnvironment } from '../src/process-runner.js';
import { redact } from '../src/redact.js';
import { assertTrustedRuViewRepo } from '../src/repo-trust.js';
function fixture() {
  // assertTrustedRuViewRepo and process.cwd() both return resolved paths, and
  // os.tmpdir() is a symlink on macOS (/var -> /private/var), so the fixture
  // root has to be resolved too or the equality assertions cannot hold.
  const dir = realpathSync(mkdtempSync(join(tmpdir(), 'ruview-hosts-')));
  mkdirSync(join(dir, '.git')); mkdirSync(join(dir, 'v2')); mkdirSync(join(dir, 'firmware'));
  writeFileSync(join(dir, 'README.md'), '# RuView\nWiFi DensePose repository\n');
  const cli = join(dir, 'fake-cli.mjs');
  writeFileSync(cli, `let input='';process.stdin.setEncoding('utf8');for await(const chunk of process.stdin)input+=chunk;process.stdout.write(JSON.stringify({argv:process.argv.slice(2),input,cwd:process.cwd(),secret:process.env.TEST_SECRET}));`);
  return { dir, cli };
}
test('redacts common and environment-provided secrets', () => {
  const clean = redact('Authorization: Bearer abc.def password=hunter2 key=sk-super-secret-value', { env: { OPENAI_API_KEY: 'sk-super-secret-value' } });
  assert.doesNotMatch(clean, /abc\.def|hunter2|super-secret/);
});
test('environment is an explicit allowlist', () => {
  assert.deepEqual(scrubEnvironment({ PATH: 'ok', TEST_SECRET: 'no', HOME: 'yes' }), { PATH: 'ok', HOME: 'yes' });
});
test('trust preflight rejects a different anchor', () => {
  const { dir } = fixture(); const other = mkdtempSync(join(tmpdir(), 'ruview-anchor-'));
  try { assert.equal(assertTrustedRuViewRepo(dir), dir); assert.throws(() => assertTrustedRuViewRepo(dir, { trustedRoot: other }), /trusted root/); }
  finally { rmSync(dir, { recursive: true, force: true }); rmSync(other, { recursive: true, force: true }); }
});
test('Claude adapter sends prompt over stdin in plan mode', async () => {
  const { dir, cli } = fixture();
  try {
    const seen = JSON.parse((await runClaudeCode({ prompt: 'inspect only', repoRoot: dir, command: process.execPath, commandArgs: [cli], env: { ...process.env, TEST_SECRET: 'must-not-leak' } })).stdout);
    assert.equal(seen.input, 'inspect only'); assert.equal(seen.secret, undefined); assert.deepEqual(seen.argv, buildClaudeCodeArgs());
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
test('Codex adapter uses exec stdin and read-only ephemeral JSON mode', async () => {
  const { dir, cli } = fixture();
  try {
    const seen = JSON.parse((await runCodex({ prompt: 'map the repository', repoRoot: dir, command: process.execPath, commandArgs: [cli] })).stdout);
    assert.equal(seen.input, 'map the repository'); assert.deepEqual(seen.argv, buildCodexArgs(dir)); assert.equal(seen.cwd, dir);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
test('write mode maps to explicit host write policies', () => {
  assert.ok(buildClaudeCodeArgs({ write: false }).includes('plan')); assert.ok(buildClaudeCodeArgs({ write: true }).includes('acceptEdits'));
  assert.ok(buildCodexArgs('X', { write: false }).includes('read-only')); assert.ok(buildCodexArgs('X', { write: true }).includes('workspace-write'));
});
test('runner bounds output and times out', async () => {
  await assert.rejects(runProcess(process.execPath, ['-e', 'process.stdout.write("x".repeat(200))'], { maxOutputBytes: 32 }), (error) => error.truncated);
  await assert.rejects(runProcess(process.execPath, ['-e', 'setTimeout(() => {}, 10000)'], { timeoutMs: 20 }), (error) => error.timedOut);
});
