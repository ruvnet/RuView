// SPDX-License-Identifier: MIT
// MCP stdio server e2e — spawns `bin/cli.js mcp start` and speaks JSON-RPC.
// Pins ADR-263 O2 (ping answered while a long tools/call runs), O6 (version
// from package.json), and O8 (underscore names advertised, dotted accepted,
// resources/prompts stubs).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { which } from '../src/tools.js';

const PKG_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const CLI = join(PKG_ROOT, 'bin', 'cli.js');

/** Start the MCP server; returns {send, next, close} where next(id) resolves the response with that id. */
function startServer() {
  const child = spawn(process.execPath, [CLI, 'mcp', 'start'], { stdio: ['pipe', 'pipe', 'pipe'] });
  const waiters = new Map();
  let buf = '';
  child.stdout.on('data', (d) => {
    buf += d;
    let nl;
    while ((nl = buf.indexOf('\n')) !== -1) {
      const line = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (!line) continue;
      const msg = JSON.parse(line);
      const w = waiters.get(msg.id);
      if (w) { waiters.delete(msg.id); w(msg); }
    }
  });
  return {
    send(msg) { child.stdin.write(JSON.stringify(msg) + '\n'); },
    next(id) { return new Promise((res) => waiters.set(id, res)); },
    close() { child.stdin.end(); child.kill(); },
  };
}

test('MCP handshake: initialize reports the package.json version; list endpoints respond', async () => {
  const pkg = JSON.parse(readFileSync(join(PKG_ROOT, 'package.json'), 'utf8'));
  const s = startServer();
  try {
    s.send({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} });
    const init = await s.next(1);
    assert.equal(init.result.serverInfo.version, pkg.version, 'ADR-263 O6: version must match package.json');
    assert.match(init.result.instructions, /credential-use/);

    s.send({ jsonrpc: '2.0', id: 2, method: 'tools/list' });
    const tools = (await s.next(2)).result.tools;
    assert.equal(tools.length, 22);
    for (const t of tools) assert.match(t.name, /^[a-zA-Z0-9_-]{1,64}$/, `advertised name not host-safe: ${t.name}`);
    const guidance = tools.find((tool) => tool.name === 'ruview_guidance');
    assert.ok(guidance);
    assert.equal(guidance.annotations.readOnlyHint, true);
    const spaces = tools.find((tool) => tool.name === 'ruview_spaces_list');
    assert.ok(spaces);
    assert.equal(spaces.annotations.readOnlyHint, false, 'OAuth refresh can update the local credential file');
    assert.equal(spaces.annotations.idempotentHint, false);
    assert.equal(spaces.annotations.destructiveHint, false);
    assert.equal(spaces.annotations.openWorldHint, true);

    s.send({ jsonrpc: '2.0', id: 3, method: 'resources/list' });
    const resources = (await s.next(3)).result.resources;
    assert.deepEqual(resources.map((r) => [r.uri, r.mimeType]), [['ui://ruview/console-v1.html', 'text/html;profile=mcp-app']]);
    s.send({ jsonrpc: '2.0', id: 4, method: 'prompts/list' });
    assert.deepEqual((await s.next(4)).result, { prompts: [] });

    // Dotted legacy name still callable (alias).
    s.send({ jsonrpc: '2.0', id: 5, method: 'tools/call', params: { name: 'ruview.onboard', arguments: {} } });
    const call = await s.next(5);
    assert.equal(call.result.isError, false);

    s.send({ jsonrpc: '2.0', id: 6, method: 'tools/call', params: { name: 'ruview_guidance', arguments: { topic: 'homecore', query: 'restore state', limit: 2 } } });
    const guided = JSON.parse((await s.next(6)).result.content[0].text);
    assert.equal(guided.ok, true);
    assert.equal(guided.topic, 'homecore');
    assert.ok(guided.capabilities.some(({ id }) => id === 'homecore-runtime-restore'));

    s.send({ jsonrpc: '2.0', id: 7, method: 'tools/call', params: { name: 'ruview_spaces_list', arguments: {} } });
    const deniedSpaces = JSON.parse((await s.next(7)).result.content[0].text);
    assert.equal(deniedSpaces.reason, 'authority_denied');
    assert.equal(deniedSpaces.requiredGrant, 'credential-use');
  } finally {
    s.close();
  }
});

test('MCP server answers ping while a long tools/call is in flight (ADR-263 O2)', { skip: !which('python') && !which('python3') ? 'python not on PATH' : false }, async () => {
  // Fake RuView repo whose verify.py sleeps 3 s then passes.
  const repo = mkdtempSync(join(tmpdir(), 'ruview-mcp-e2e-'));
  const proofDir = join(repo, 'archive', 'v1', 'data', 'proof');
  mkdirSync(proofDir, { recursive: true });
  writeFileSync(join(proofDir, 'verify.py'), 'import time\ntime.sleep(3)\nprint("VERDICT: PASS")\n');

  const s = startServer();
  try {
    s.send({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} });
    await s.next(1);

    const verifyDone = s.next(10);
    s.send({ jsonrpc: '2.0', id: 10, method: 'tools/call', params: { name: 'ruview_verify', arguments: { repo } } });

    // Give the server a beat to start the child, then ping.
    await new Promise((r) => setTimeout(r, 300));
    const t0 = Date.now();
    const pinged = s.next(11);
    s.send({ jsonrpc: '2.0', id: 11, method: 'ping' });
    await pinged;
    const pingMs = Date.now() - t0;
    assert.ok(pingMs < 1000, `ping took ${pingMs} ms while verify was in flight — server is blocking`);

    const verify = await verifyDone;
    const payload = JSON.parse(verify.result.content[0].text);
    assert.equal(payload.verdict, 'PASS');
  } finally {
    s.close();
    rmSync(repo, { recursive: true, force: true });
  }
});

test('tools/call executions are serialized — two slow calls run sequentially', { skip: !which('python') && !which('python3') ? 'python not on PATH' : false }, async () => {
  // Two verify.py that each sleep 0.8 s. Serialized ⇒ ~1.6 s+; concurrent ⇒ ~0.8 s.
  const repo = mkdtempSync(join(tmpdir(), 'ruview-mcp-serial-'));
  const proofDir = join(repo, 'archive', 'v1', 'data', 'proof');
  mkdirSync(proofDir, { recursive: true });
  writeFileSync(join(proofDir, 'verify.py'), 'import time\ntime.sleep(0.8)\nprint("VERDICT: PASS")\n');

  const s = startServer();
  try {
    s.send({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} });
    await s.next(1);

    const t0 = Date.now();
    const a = s.next(20);
    const b = s.next(21);
    s.send({ jsonrpc: '2.0', id: 20, method: 'tools/call', params: { name: 'ruview_verify', arguments: { repo } } });
    s.send({ jsonrpc: '2.0', id: 21, method: 'tools/call', params: { name: 'ruview_verify', arguments: { repo } } });
    const [ra, rb] = await Promise.all([a, b]);
    const elapsed = Date.now() - t0;

    assert.equal(JSON.parse(ra.result.content[0].text).verdict, 'PASS');
    assert.equal(JSON.parse(rb.result.content[0].text).verdict, 'PASS');
    assert.ok(elapsed > 1400, `two 0.8 s tool calls finished in ${elapsed} ms — they overlapped instead of serializing`);
  } finally {
    s.close();
    rmSync(repo, { recursive: true, force: true });
  }
});

test('MCP bounds oversized input and its tool queue, and cancels queued calls', { skip: !which('python') && !which('python3') ? 'python not on PATH' : false }, async () => {
  const repo = mkdtempSync(join(tmpdir(), 'ruview-mcp-bounds-'));
  const proofDir = join(repo, 'archive', 'v1', 'data', 'proof');
  mkdirSync(proofDir, { recursive: true });
  writeFileSync(join(proofDir, 'verify.py'), 'import time\ntime.sleep(2)\nprint("VERDICT: PASS")\n');

  const s = startServer();
  try {
    // A request above the 256 KiB bound is discarded without taking down the server.
    s.send({ jsonrpc: '2.0', id: 90, method: 'initialize', params: { padding: 'x'.repeat(300_000) } });
    const pinged = s.next(91);
    s.send({ jsonrpc: '2.0', id: 91, method: 'ping' });
    assert.deepEqual((await pinged).result, {});

    // The first call remains in flight while the second waits, so cancellation
    // must prevent the queued request from ever reaching the tool implementation.
    s.send({ jsonrpc: '2.0', id: 100, method: 'tools/call', params: { name: 'ruview_verify', arguments: { repo } } });
    const cancelled = s.next(101);
    s.send({ jsonrpc: '2.0', id: 101, method: 'tools/call', params: { name: 'ruview_verify', arguments: { repo } } });
    s.send({ jsonrpc: '2.0', method: 'notifications/cancelled', params: { requestId: 101 } });

    // The 20-call bound includes the in-flight request. Fill the remaining
    // slots and assert the next request fails immediately rather than growing
    // memory without limit.
    for (let id = 102; id < 120; id += 1) {
      s.send({ jsonrpc: '2.0', id, method: 'tools/call', params: { name: 'ruview_verify', arguments: { repo } } });
    }
    const full = s.next(120);
    s.send({ jsonrpc: '2.0', id: 120, method: 'tools/call', params: { name: 'ruview_verify', arguments: { repo } } });
    assert.equal((await full).error.code, -32000);
    assert.equal((await cancelled).error.code, -32800);
  } finally {
    s.close();
    rmSync(repo, { recursive: true, force: true });
  }
});

test('stdin close flushes an in-flight tools/call response before exit', async () => {
  const child = spawn(process.execPath, [CLI, 'mcp', 'start'], { stdio: ['pipe', 'pipe', 'pipe'] });
  let out = '';
  child.stdout.on('data', (d) => { out += d; });
  const exited = new Promise((res) => child.on('exit', res));

  // Write a tools/call then immediately close stdin. The old fire-and-forget
  // dispatch raced rl 'close' → process.exit and could drop this response.
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: 42, method: 'tools/call', params: { name: 'ruview_onboard', arguments: {} } }) + '\n');
  child.stdin.end();

  await exited;
  const msgs = out.trim().split('\n').filter(Boolean).map((l) => JSON.parse(l));
  const resp = msgs.find((m) => m.id === 42);
  assert.ok(resp, 'the in-flight tools/call response must be flushed to stdout before exit');
  assert.equal(resp.result.isError, false);
});
