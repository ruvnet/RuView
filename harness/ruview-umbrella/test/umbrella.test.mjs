// ADR-376: the `ruview` umbrella — CLI dispatch to every component and one
// MCP server (harness + Homecore tools), over stdio and HTTP.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { unifiedHandler } from '../src/mcp.js';
import { capabilities, componentVersions, KERNEL_COMMANDS, VERSION } from '../src/index.js';

const BIN = fileURLToPath(new URL('../bin/ruview.js', import.meta.url));
const PKG = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
const cli = (args, env = {}) => spawnSync(process.execPath, [BIN, ...args], { encoding: 'utf8', env: { ...process.env, ...env }, timeout: 120000 });
const rpc = (method, params, id = 1) => unifiedHandler({ jsonrpc: '2.0', id, method, params });

test('version and capabilities cover every component', async () => {
  assert.equal(VERSION, PKG.version);
  const versions = await componentVersions();
  for (const name of Object.keys(PKG.dependencies)) assert.ok(versions[name], `${name} resolves`);
  const caps = await capabilities();
  assert.ok(caps.tools.harness.includes('ruview_esp32_capture'));
  assert.ok(caps.tools.harness.includes('ruview_mmwave_read'));
  assert.ok(caps.tools.homecore.includes('homecore_guidance'));
  assert.deepEqual(caps.tools.kernelCli, [...KERNEL_COMMANDS]);
  const v = cli(['--version']);
  assert.equal(v.status, 0);
  assert.match(v.stdout, new RegExp(`^ruview ${VERSION.replace(/\./g, '\\.')}`));
  assert.match(v.stdout, /homecore \d+\.\d+\.\d+/);
});

test('CLI dispatches to the harness, Homecore and the kernel', () => {
  const help = cli(['--help']);
  assert.equal(help.status, 0);
  assert.match(help.stdout, /RuView in one install/);
  assert.match(help.stdout, /esp32 \[--udp-port 5005\]/, 'harness help follows');

  const claim = cli(['claim-check', '--text', 'presence accuracy 0.9', '--json']);
  assert.equal(JSON.parse(claim.stdout).name ?? 'ruview_claim_check', 'ruview_claim_check');

  const hc = cli(['homecore', 'guidance', '--topic', 'plugins', '--query', 'Wasmtime', '--limit', '1']);
  assert.equal(hc.status, 0, hc.stderr);
  assert.match(hc.stdout, /wasm-plugins/);

  const kernel = cli(['kernel', 'info']);
  assert.equal(kernel.status, 0, kernel.stderr);
  assert.match(kernel.stdout, /"abi"|"version"/);
  const selftest = cli(['kernel', '--backend', 'wasm', '--json']);
  assert.ok([0, 1].includes(selftest.status), 'plain `kernel` stays the harness self-test');
  assert.match(selftest.stdout, /SYNTHETIC/);
});

test('one MCP server lists harness + Homecore tools and routes calls', async () => {
  const init = (await rpc('initialize', { protocolVersion: '2025-06-18' })).result;
  assert.deepEqual([init.serverInfo.name, init.serverInfo.version], ['ruview', VERSION]);
  assert.match(init.instructions, /homecore_/);
  const tools = (await rpc('tools/list')).result.tools;
  const names = tools.map((t) => t.name);
  assert.ok(names.includes('ruview_esp32_capture') && names.includes('homecore_guidance'));
  assert.equal(new Set(names).size, names.length, 'no duplicate tool names');
  for (const t of tools) assert.match(t.name, /^[a-zA-Z0-9_-]{1,64}$/);
  assert.ok(tools.find((t) => t.name === 'ruview_esp32_capture')._meta.ui.resourceUri.startsWith('ui://'));

  const hc = (await rpc('tools/call', { name: 'homecore_guidance', arguments: { topic: 'plugins', query: 'Wasmtime', limit: 1 } }, 2)).result;
  assert.equal(hc._meta['ruview/tool'], 'homecore_guidance');
  assert.equal(typeof hc.structuredContent, 'object');
  const rv = (await rpc('tools/call', { name: 'ruview_claim_check', arguments: { text: 'x MEASURED' } }, 3)).result;
  assert.equal(rv._meta['ruview/tool'], 'ruview_claim_check');
  const denied = (await unifiedHandler({ jsonrpc: '2.0', id: 4, method: 'tools/call', params: { name: 'ruview_devices_scan', arguments: {} } }, { source: 'mcp', grants: [] })).result;
  assert.equal(denied.structuredContent.reason, 'authority_denied', 'harness authority model is preserved');
  const res = (await rpc('resources/read', { uri: 'ui://ruview/console-v1.html' }, 5)).result;
  assert.match(res.contents[0].text, /RuView console/);
});

test('stdio and HTTP transports serve the unified tool set', async (t) => {
  const child = spawn(process.execPath, [BIN, 'mcp', 'start'], { stdio: ['pipe', 'pipe', 'pipe'] });
  t.after(() => child.kill());
  const lines = [];
  child.stdout.on('data', (d) => lines.push(...String(d).trim().split('\n')));
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/list' })}\n`);
  for (let i = 0; i < 100 && !lines.length; i++) await new Promise((r) => setTimeout(r, 50));
  const stdioTools = JSON.parse(lines[0]).result.tools.map((x) => x.name);
  assert.ok(stdioTools.includes('homecore_guidance') && stdioTools.includes('ruview_mmwave_read'));

  const token = 'umbrella-test-token-0123456789';
  const port = 20000 + Math.floor(Math.random() * 20000);
  const http = spawn(process.execPath, [BIN, 'mcp', 'start', '--http', '--port', String(port)], { env: { ...process.env, RUVIEW_MCP_TOKEN: token }, stdio: ['ignore', 'pipe', 'pipe'] });
  t.after(() => http.kill());
  let ok = false;
  for (let i = 0; i < 100 && !ok; i++) {
    try { ok = (await fetch(`http://127.0.0.1:${port}/healthz`)).ok; } catch { await new Promise((r) => setTimeout(r, 100)); }
  }
  assert.ok(ok, 'HTTP transport started');
  const r = await fetch(`http://127.0.0.1:${port}/mcp/${token}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/list' }) });
  const httpTools = (await r.json()).result.tools.map((x) => x.name);
  assert.deepEqual(httpTools.sort(), [...stdioTools].sort());
  assert.equal((await fetch(`http://127.0.0.1:${port}/mcp`, { method: 'POST', body: '{}' })).status, 401);
});
