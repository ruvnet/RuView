import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { listTools } from '../src/tools.js';

const cwd = new URL('..', import.meta.url);

function runMcp(lines) {
  return spawnSync(process.execPath, ['bin/cli.js', 'mcp', 'start'], {
    cwd,
    encoding: 'utf8',
    input: `${lines.map((line) => JSON.stringify(line)).join('\n')}\n`,
    timeout: 5000,
  });
}

test('MCP discovers only read only tools', () => {
  const child = runMcp([
    { jsonrpc: '2.0', id: 1, method: 'initialize', params: {} },
    { jsonrpc: '2.0', method: 'notifications/initialized', params: {} },
    { jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} },
  ]);
  assert.equal(child.status, 0, child.stderr);
  const messages = child.stdout.trim().split('\n').map((line) => JSON.parse(line));
  const initialized = messages.find((message) => message.id === 1);
  const listed = messages.find((message) => message.id === 2);
  assert.match(initialized.result.instructions, /cannot connect accounts/u);
  assert.equal(listed.result.tools.length, 10);
  assert.equal(listed.result.tools.some((tool) => tool.name === 'social_autopilot_run'), true);
  for (const tool of listed.result.tools) {
    assert.equal(tool.annotations.readOnlyHint, true);
    assert.equal(tool.annotations.destructiveHint, false);
    assert.doesNotMatch(tool.name, /(?:connect|send|publish|reply|react|moderate|delete|spend|deploy|approve|promote)/u);
  }
});

test('MCP policy allowlist exactly matches the implemented tool registry', () => {
  const policy = JSON.parse(readFileSync(new URL('../.harness/mcp-policy.json', import.meta.url), 'utf8'));
  assert.deepEqual(policy.readOnlyTools, listTools().map(({ name }) => name));
  assert.deepEqual(policy.cliOnlyTools, []);
});

test('MCP tool call rejects credential material', () => {
  const secretValue = `ghp_${'z'.repeat(32)}`;
  const sensitiveField = `sec${'ret'}`;
  const child = runMcp([
    { jsonrpc: '2.0', id: 1, method: 'tools/call', params: {
      name: 'social_action_plan',
      arguments: { platform: 'github', operation: 'publish', requestedRoute: 'api', account: secretValue, [sensitiveField]: 'not-real' },
    } },
  ]);
  assert.equal(child.status, 0, child.stderr);
  const message = JSON.parse(child.stdout.trim());
  assert.equal(message.result.isError, true);
  assert.match(message.result.content[0].text, /credential_material_forbidden/u);
  assert.doesNotMatch(child.stdout, new RegExp(secretValue, 'u'));
  assert.doesNotMatch(child.stderr, new RegExp(secretValue, 'u'));
});

test('fresh MCP process invokes the offline doctor', () => {
  const child = runMcp([
    { jsonrpc: '2.0', id: 7, method: 'tools/call', params: { name: 'social_doctor', arguments: {} } },
  ]);
  assert.equal(child.status, 0, child.stderr);
  const message = JSON.parse(child.stdout.trim());
  const output = JSON.parse(message.result.content[0].text);
  assert.equal(message.result.isError, false);
  assert.equal(output.ok, true);
  assert.equal(output.networkAttempted, false);
});

test('MCP rejects malformed input without echoing it', () => {
  const child = spawnSync(process.execPath, ['bin/cli.js', 'mcp', 'start'], {
    cwd,
    encoding: 'utf8',
    input: '{not-json}\n',
    timeout: 5000,
  });
  assert.equal(child.status, 0, child.stderr);
  assert.match(child.stdout, /Parse error/u);
  assert.doesNotMatch(child.stdout, /not-json/u);
});

test('oversized MCP line is discarded without parsing a suffix', () => {
  const oversized = 'x'.repeat(262145);
  const child = spawnSync(process.execPath, ['bin/cli.js', 'mcp', 'start'], {
    cwd,
    encoding: 'utf8',
    input: `${oversized}{"jsonrpc":"2.0","id":9,"method":"ping"}\n`,
    timeout: 5000,
  });
  assert.equal(child.status, 0, child.stderr);
  const messages = child.stdout.trim().split('\n').filter(Boolean).map((line) => JSON.parse(line));
  assert.equal(messages.length, 1);
  assert.equal(messages[0].error.code, -32600);
  assert.equal(messages.some((message) => message.id === 9), false);
});

test('tools/call notifications without ids are rejected', () => {
  const child = runMcp([{ jsonrpc: '2.0', method: 'tools/call', params: { name: 'social_doctor', arguments: {} } }]);
  const message = JSON.parse(child.stdout.trim());
  assert.equal(message.error.code, -32600);
});

test('duplicate in-flight request ids are rejected', () => {
  const child = runMcp([
    { jsonrpc: '2.0', id: 11, method: 'tools/call', params: { name: 'social_doctor', arguments: {} } },
    { jsonrpc: '2.0', id: 11, method: 'tools/call', params: { name: 'social_platforms', arguments: {} } },
  ]);
  const messages = child.stdout.trim().split('\n').map((line) => JSON.parse(line));
  assert.equal(messages.some((message) => message.error?.message === 'Duplicate in-flight request id'), true);
  assert.equal(messages.some((message) => message.result?.isError === false), true);
});

test('queued tool call can be cancelled before execution', () => {
  const child = runMcp([
    { jsonrpc: '2.0', id: 12, method: 'tools/call', params: { name: 'social_doctor', arguments: {} } },
    { jsonrpc: '2.0', method: 'notifications/cancelled', params: { requestId: 12 } },
  ]);
  const message = JSON.parse(child.stdout.trim());
  assert.equal(message.id, 12);
  assert.equal(message.error.code, -32800);
});

test('MCP queue bound fails closed', () => {
  const calls = Array.from({ length: 17 }, (_, index) => ({
    jsonrpc: '2.0',
    id: 100 + index,
    method: 'tools/call',
    params: { name: 'social_doctor', arguments: {} },
  }));
  const child = runMcp(calls);
  const messages = child.stdout.trim().split('\n').map((line) => JSON.parse(line));
  assert.equal(messages.some((message) => message.error?.message === 'Tool queue is full'), true);
});
