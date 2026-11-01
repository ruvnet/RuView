// SPDX-License-Identifier: MIT
// RuView harness — dependency-free MCP server core (JSON-RPC 2.0).
//
// Dependency-free on purpose: a published `npx ruview` must `mcp start` without
// pulling the full MCP SDK. `handleRpc` is transport-neutral; stdio (this file)
// and Streamable HTTP (mcp-http.js, ADR-375) both drive it. Logs go to stderr
// ONLY — on stdio, stdout is the JSON-RPC channel and must stay clean.
//
// ADR-263 O2: `tools/call` is dispatched asynchronously — a long-running
// verify/calibrate no longer blocks ping/tools/list, so hosts that health-check
// mid-run see a live server. Responses may therefore arrive out of request
// order, which JSON-RPC permits (ids correlate them).
//
// ADR-375: results carry `structuredContent` (2025-06-18) next to a compact
// JSON text block, and the UI tools point at a self-contained `ui://` widget
// for MCP Apps hosts and ChatGPT.

import { createInterface } from 'node:readline';
import { readFileSync } from 'node:fs';
import { listTools, resolveToolName, runTool } from './tools.js';
import { CONSOLE_HTML, CONSOLE_RESOURCE, CONSOLE_RESOURCE_META } from './ui/console-widget.js';

/** Newest first; an unknown client version gets the newest we speak. */
export const SUPPORTED_PROTOCOLS = Object.freeze(['2025-06-18', '2025-03-26', '2024-11-05']);
export const MAX_REQUEST_BYTES = 256 * 1024;
const MAX_QUEUED_TOOL_CALLS = 20;
// Single-source the version from package.json (ADR-263 O6).
const PKG = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
export const SERVER_INFO = Object.freeze({ name: 'ruview', title: 'RuView', version: PKG.version });

const INSTRUCTIONS = 'RuView WiFi-sensing operator tools. All results are fail-closed; accuracy claims must pass ruview_claim_check. Credentialed external reads are denied without an operator grant; ruview_spaces_list requires credential-use. Node captures, device scans and doctor results render in the ruview console widget.';

export function negotiateProtocol(requested) {
  return SUPPORTED_PROTOCOLS.includes(requested) ? requested : SUPPORTED_PROTOCOLS[0];
}

const ok = (id, result) => ({ jsonrpc: '2.0', id, result });
const fail = (id, code, message) => ({ jsonrpc: '2.0', id, error: { code, message } });
function log(...a) { process.stderr.write('[ruview-mcp] ' + a.join(' ') + '\n'); }

/**
 * Shape a tool result as an MCP CallToolResult: structured content for hosts
 * and widgets, plus compact JSON text for clients that read only `content`
 * (compact, not pretty-printed: the same data in fewer model tokens).
 */
export function toCallToolResult(name, out) {
  const structured = out && typeof out === 'object' && !Array.isArray(out) ? out : { value: out };
  return {
    content: [{ type: 'text', text: JSON.stringify(structured) }],
    structuredContent: structured,
    isError: structured.ok === false,
    _meta: { 'ruview/tool': name },
  };
}

/**
 * Handle one JSON-RPC message. Returns the response object, or null for a
 * notification (no response is sent).
 */
export async function handleRpc(msg, context = {}) {
  if (!msg || typeof msg !== 'object' || Array.isArray(msg) || msg.jsonrpc !== '2.0' || typeof msg.method !== 'string') {
    return fail(msg && typeof msg === 'object' && 'id' in msg ? msg.id : null, -32600, 'Invalid Request');
  }
  const { id, method, params } = msg;
  const isNotification = id === undefined;
  switch (method) {
    case 'initialize':
      return ok(id, {
        protocolVersion: negotiateProtocol(params?.protocolVersion),
        capabilities: { tools: { listChanged: false }, resources: { listChanged: false } },
        serverInfo: SERVER_INFO,
        instructions: INSTRUCTIONS,
      });
    case 'notifications/initialized':
    case 'initialized':
      return null;
    case 'notifications/cancelled':
      if (context.queuedIds?.has(params?.requestId)) context.cancelled?.add(params.requestId);
      return null;
    case 'ping':
      return ok(id, {});
    case 'tools/list':
      return ok(id, { tools: listTools() });
    case 'resources/list':
      return ok(id, { resources: [{ ...CONSOLE_RESOURCE, _meta: CONSOLE_RESOURCE_META }] });
    case 'resources/templates/list':
      return ok(id, { resourceTemplates: [] });
    case 'resources/read': {
      if (params?.uri !== CONSOLE_RESOURCE.uri) return fail(id, -32002, `Resource not found: ${String(params?.uri).slice(0, 200)}`);
      return ok(id, { contents: [{ uri: CONSOLE_RESOURCE.uri, mimeType: CONSOLE_RESOURCE.mimeType, text: CONSOLE_HTML, _meta: CONSOLE_RESOURCE_META }] });
    }
    case 'prompts/list':
      return ok(id, { prompts: [] });
    case 'tools/call': {
      const name = params?.name;
      const args = params?.arguments || {};
      log('audit', JSON.stringify({ event: 'tools/call', id, name, transport: context.transport || 'stdio' }));
      const out = await runTool(name, args, context);
      return ok(id, toCallToolResult(resolveToolName(name) || name, out));
    }
    default:
      return isNotification ? null : fail(id, -32601, `Method not found: ${method}`);
  }
}

/**
 * Serialize tools/call through one FIFO chain: hardware/mutating tools
 * (calibrate, serial monitor, flash) must never overlap. Everything else
 * answers immediately (ADR-263 O2). Shared by the stdio and HTTP transports.
 */
export function createDispatcher(baseContext, handler = handleRpc) {
  let chain = Promise.resolve();
  let queued = 0;
  const cancelled = new Set();
  const queuedIds = new Set();
  const context = { ...baseContext, cancelled, queuedIds };
  const run = (msg) => handler(msg, context).catch((err) => {
    log('handler error:', String(err));
    return msg && msg.id !== undefined ? fail(msg.id, -32603, String(err && err.message || err)) : null;
  });

  function dispatch(msg) {
    if (!msg || msg.method !== 'tools/call') return run(msg);
    const validId = typeof msg.id === 'string' || (typeof msg.id === 'number' && Number.isFinite(msg.id));
    if (!validId) return Promise.resolve(fail(msg?.id ?? null, -32600, 'tools/call requires a finite string or number id'));
    if (queuedIds.has(msg.id)) return Promise.resolve(fail(msg.id, -32600, 'Duplicate in-flight request id'));
    if (queued >= MAX_QUEUED_TOOL_CALLS) { log('tool queue full:', String(msg.id)); return Promise.resolve(fail(msg.id, -32000, 'Tool queue is full')); }
    queued += 1;
    queuedIds.add(msg.id);
    const result = chain.then(async () => {
      try {
        if (cancelled.delete(msg.id)) return fail(msg.id, -32800, 'Request cancelled');
        return await run(msg);
      } finally {
        cancelled.delete(msg.id);
        queuedIds.delete(msg.id);
        queued -= 1;
      }
    });
    chain = result.then(() => {}, () => {});
    return result;
  }
  return { dispatch, idle: () => chain };
}

export function parseGrants(env = process.env) {
  return String(env.RUVIEW_MCP_GRANTS || '').split(',').map((v) => v.trim()).filter(Boolean);
}

/** stdio transport. `handler` lets an embedding package (the `ruview` umbrella) serve a merged tool set. */
export function startMcpServer({ handler = handleRpc, label = `${listTools().length} tools` } = {}) {
  log(`starting v${SERVER_INFO.version} (protocol ${SUPPORTED_PROTOCOLS[0]}, ${label}, stdio)`);
  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
  const { dispatch, idle } = createDispatcher({ source: 'mcp', transport: 'stdio', grants: parseGrants() }, handler);
  const send = (res) => { if (res) process.stdout.write(JSON.stringify(res) + '\n'); };

  rl.on('line', (line) => {
    if (Buffer.byteLength(line, 'utf8') > MAX_REQUEST_BYTES) { log('oversized JSON-RPC line dropped'); return; }
    const s = line.trim();
    if (!s) return;
    let msg;
    try { msg = JSON.parse(s); } catch { log('bad JSON line dropped'); return; }
    dispatch(msg).then(send);
  });

  rl.on('close', () => {
    // Wait for any queued/in-flight tool call to settle (its response written)
    // before exiting — fire-and-forget used to race this and drop the response.
    idle().then(() => setImmediate(() => {
      log('stdin closed — exiting');
      const done = () => process.exit(0);
      // Pipe writes are async; flush buffered stdout before exit.
      if (process.stdout.writableLength) process.stdout.once('drain', done);
      else done();
    }));
  });
}
