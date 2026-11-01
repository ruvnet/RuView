import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const cli = fileURLToPath(new URL('../bin/cli.js', import.meta.url));

function capture(args) {
  // Invalid UDP port guarantees schema refusal before any socket is opened.
  const child = spawnSync(process.execPath, [cli, 'esp32', '--udp-port', '1', ...args], { encoding: 'utf8' });
  assert.equal(child.status, 1, child.stderr);
  const result = JSON.parse(child.stdout);
  assert.equal(result.reason, 'invalid_arguments');
  return result.errors;
}

for (const args of [['--node-id'], ['--node-id=']]) {
  test(`public CLI rejects absent numeric value ${args}`, () => {
    assert.ok(capture(args).some((error) => error.includes('node_id')), 'missing node selector must not become 0 or 1');
  });
}

test('public CLI still forwards explicit numeric node zero', () => {
  assert.ok(!capture(['--node-id', '0']).some((error) => error.includes('node_id')));
});
