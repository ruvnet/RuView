import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const cwd = new URL('..', import.meta.url);

function cli(args, input) {
  return spawnSync(process.execPath, ['bin/cli.js', ...args], {
    cwd,
    encoding: 'utf8',
    input,
    timeout: 5000,
  });
}

test('strict doctor passes offline', () => {
  const child = cli(['doctor', '--strict']);
  assert.equal(child.status, 0, child.stderr);
  const output = JSON.parse(child.stdout);
  assert.equal(output.ok, true);
  assert.equal(output.networkAttempted, false);
  assert.equal(output.credentialStoresRead, false);
});

test('unknown CLI arguments fail closed', () => {
  const child = cli(['doctor', '--fix']);
  assert.equal(child.status, 1);
  assert.match(child.stderr, /Unknown doctor argument/u);
});

test('structured operations require bounded JSON on stdin', () => {
  const child = cli(['direction', 'check'], '');
  assert.equal(child.status, 1);
  assert.match(child.stderr, /required on stdin/u);
});

test('platform listing exposes no credential value', () => {
  const child = cli(['platforms']);
  assert.equal(child.status, 0, child.stderr);
  assert.doesNotMatch(child.stdout, /(?:gh[opsu]_|xox[baprs]-|sk-)[A-Za-z0-9_-]{16,}/u);
});

test('help exposes the bounded autopilot command without an execution command', () => {
  const child = cli(['help']);
  assert.equal(child.status, 0, child.stderr);
  const output = JSON.parse(child.stdout);
  assert.ok(output.commands.includes('autopilot run < input.json'));
  assert.equal(output.commands.some((command) => /(?:publish|send|promote|execute)/u.test(command)), false);
});
