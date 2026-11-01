// SPDX-License-Identifier: MIT
// Unified MCP handler (ADR-376): the @ruvnet/ruview tool registry plus the
// Homecore tools in one server. Homecore tool names are already prefixed
// (`homecore_*`), so there is no collision. Everything that is not a Homecore
// tool call — handshake, resources (the ui:// console), ruview tools, policy —
// is answered by the harness handler unchanged.

import { handleRpc, toCallToolResult } from '@ruvnet/ruview/mcp';
import { VERSION, wireKernel } from './index.js';

await wireKernel();

let homecorePromise = null;
const loadHomecore = () => (homecorePromise ??= import('homecore').catch(() => null));

/** Merge a Homecore tool into the MCP tool shape (no UI metadata). */
function homecoreTool(t) {
  return { name: t.name, ...(t.title ? { title: t.title } : {}), description: t.description, inputSchema: t.inputSchema, annotations: t.annotations };
}

export async function unifiedHandler(msg, context = {}) {
  const homecore = await loadHomecore();
  if (msg && msg.jsonrpc === '2.0' && msg.method === 'tools/call' && homecore && typeof msg.params?.name === 'string' && msg.params.name.startsWith('homecore_')) {
    const out = await homecore.runTool(msg.params.name, msg.params.arguments || {}, { ...context, source: 'mcp' });
    return { jsonrpc: '2.0', id: msg.id, result: toCallToolResult(msg.params.name, out) };
  }
  const reply = await handleRpc(msg, context);
  if (!reply || !reply.result) return reply;
  if (msg.method === 'initialize') {
    reply.result.serverInfo = { ...reply.result.serverInfo, name: 'ruview', version: VERSION };
    reply.result.instructions += ' Homecore developer tools (homecore_*) are included: source-cited guidance, WASM kernel status and reviewed memory.';
  }
  if (msg.method === 'tools/list' && homecore) {
    reply.result.tools = [...reply.result.tools, ...homecore.listTools({ source: 'mcp' }).map(homecoreTool)];
  }
  return reply;
}
