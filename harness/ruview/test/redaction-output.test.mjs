import test from 'node:test';
import assert from 'node:assert/strict';
import { runProcess } from '../src/process-runner.js';
import { redact, REDACTED } from '../src/redact.js';

test('child output redacts quoted configuration, cookie headers and multiline key blocks', async () => {
  const secrets = ['fixture phrase with spaces', 'fixture-cookie-value', 'fixture-pairing-value', 'YWxwaGEgYnJhdm8='];
  const output = ['{"password":"fixture phrase with spaces"}', 'Cookie: session=fixture-cookie-value; preference=dark', 'setup_code=fixture-pairing-value', '-----BEGIN PRIVATE KEY-----\nYWxwaGEgYnJhdm8=\n-----END PRIVATE KEY-----'].join('\n');
  const result = await runProcess(process.execPath, ['-e', 'process.stdout.write(process.argv[1]); process.stderr.write(process.argv[1])', output]);
  for (const stream of [result.stdout, result.stderr]) {
    for (const secret of secrets) assert.ok(!stream.includes(secret), 'fixture credential must not appear');
    assert.ok(stream.includes(REDACTED));
  }
  assert.equal(redact('test pairing::tests::valid_pairing_flow ... ok'), 'test pairing::tests::valid_pairing_flow ... ok');
});
