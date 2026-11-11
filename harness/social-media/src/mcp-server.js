// SPDX-License-Identifier: MIT

import { readFileSync } from 'node:fs';
import { listTools, runTool } from './tools.js';

const PKG = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
const MCP_POLICY = JSON.parse(readFileSync(new URL('../.harness/mcp-policy.json', import.meta.url), 'utf8'));
const PROTOCOL_VERSION = '2024-11-05';
const TOOL_NAMES = new Set(listTools().map((tool) => tool.name));

function boundedInteger(value, fallback, minimum, maximum) {
  return Number.isSafeInteger(value) && value >= minimum && value <= maximum ? value : fallback;
}

const MAX_REQUEST_BYTES = boundedInteger(MCP_POLICY.maxRequestBytes, 256 * 1024, 1024, 1024 * 1024);
const MAX_QUEUED_CALLS = boundedInteger(MCP_POLICY.maxQueuedToolCalls, 16, 1, 64);
const MAX_SESSION_CALLS = boundedInteger(MCP_POLICY.maxToolCallsPerTurn, 100, 1, 256);
const TOOL_TIMEOUT_MS = boundedInteger(MCP_POLICY.toolTimeoutMs, 5000, 100, 120000);

function send(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function result(id, value) {
  send({ jsonrpc: '2.0', id, result: value });
}

function error(id, code, message) {
  send({ jsonrpc: '2.0', id, error: { code, message } });
}

function withTimeout(operation, signal) {
  if (signal?.aborted) return Promise.reject(Object.assign(new Error('Request cancelled'), { rpcCode: -32800 }));
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener('abort', abort);
      callback(value);
    };
    const abort = () => finish(reject, Object.assign(new Error('Request cancelled'), { rpcCode: -32800 }));
    const timer = setTimeout(() => finish(reject, Object.assign(new Error('Tool timeout'), { rpcCode: -32001 })), TOOL_TIMEOUT_MS);
    signal?.addEventListener('abort', abort, { once: true });
    Promise.resolve(operation).then(
      (value) => finish(resolve, value),
      (cause) => finish(reject, cause),
    );
  });
}

function validEnvelope(message) {
  if (!message || typeof message !== 'object' || Array.isArray(message)) return false;
  if (message.jsonrpc !== '2.0' || typeof message.method !== 'string' || !message.method) return false;
  if (Object.hasOwn(message, 'id') && message.id !== null && typeof message.id !== 'string' && !Number.isFinite(message.id)) return false;
  return message.params === undefined || (message.params !== null && typeof message.params === 'object' && !Array.isArray(message.params));
}

async function handle(message, context = {}) {
  const { id, method, params } = message;
  if (method === 'initialize') {
    result(id, {
      protocolVersion: PROTOCOL_VERSION,
      capabilities: { tools: { listChanged: false } },
      serverInfo: { name: '@ruvnet/social-metaharness', version: PKG.version },
      instructions: 'Read-only research, policy planning, draft linting, metric normalization, and proposal evaluation. This server has no credentials and cannot connect accounts, publish, message, moderate, spend, deploy, or promote learned policy.',
    });
    return;
  }
  if (method === 'notifications/initialized' || method === 'initialized') return;
  if (method === 'notifications/cancelled') {
    if (context.queuedIds?.has(params?.requestId)) {
      context.cancelled?.add(params.requestId);
      context.controllers?.get(params.requestId)?.abort();
    }
    return;
  }
  if (method === 'ping') return result(id, {});
  if (method === 'tools/list') return result(id, { tools: listTools() });
  if (method === 'resources/list') return result(id, { resources: [] });
  if (method === 'prompts/list') return result(id, { prompts: [] });
  if (method === 'tools/call') {
    if (typeof params?.name !== 'string') return error(id, -32602, 'Tool name is required');
    const auditName = TOOL_NAMES.has(params.name) ? params.name : 'unknown_tool';
    process.stderr.write(`[social-metaharness-mcp] audit tools/call ${auditName}\n`);
    const output = await withTimeout(runTool(params.name, params.arguments || {}), context.signal);
    result(id, {
      content: [{ type: 'text', text: JSON.stringify(output, null, 2) }],
      isError: output?.ok === false,
    });
    return;
  }
  if (id !== undefined) error(id, -32601, `Method not found: ${method}`);
}

export function startMcpServer() {
  let queued = 0;
  let accepted = 0;
  let chain = Promise.resolve();
  const cancelled = new Set();
  const queuedIds = new Set();
  const controllers = new Map();
  process.stderr.write(`[social-metaharness-mcp] starting v${PKG.version} with ${listTools().length} read-only tools\n`);

  return new Promise((resolve, reject) => {
    let chunks = [];
    let bufferedBytes = 0;
    let discardingOversizedLine = false;

    const resetLine = () => {
      chunks = [];
      bufferedBytes = 0;
      discardingOversizedLine = false;
    };

    const acceptLine = (line) => {
      if (line.length === 0) return;
      let message;
      try {
        message = JSON.parse(line.toString('utf8'));
      } catch {
        error(null, -32700, 'Parse error');
        return;
      }
      if (!validEnvelope(message)) {
        error(message?.id ?? null, -32600, 'Invalid Request');
        return;
      }
        if (message.method !== 'tools/call') {
          void handle(message, { cancelled, queuedIds, controllers }).catch((cause) => error(message.id ?? null, Number.isInteger(cause?.rpcCode) ? cause.rpcCode : -32603, 'Internal error'));
          return;
        }
        const validId = typeof message.id === 'string' || (typeof message.id === 'number' && Number.isFinite(message.id));
        if (!validId) {
          error(message.id ?? null, -32600, 'tools/call requires a finite string or number id');
          return;
        }
        if (queuedIds.has(message.id)) {
          error(message.id, -32600, 'Duplicate in-flight request id');
          return;
        }
        if (queued >= MAX_QUEUED_CALLS) {
          error(message.id ?? null, -32000, 'Tool queue is full');
          return;
        }
        if (accepted >= MAX_SESSION_CALLS) {
          error(message.id ?? null, -32000, 'Tool call budget exhausted');
          return;
        }
        queued += 1;
        accepted += 1;
        queuedIds.add(message.id);
        const controller = new AbortController();
        controllers.set(message.id, controller);
        chain = chain.then(async () => {
          if (cancelled.delete(message.id)) {
            error(message.id, -32800, 'Request cancelled');
            return;
          }
          await handle(message, { signal: controller.signal, cancelled, queuedIds, controllers });
        }).catch((cause) => {
          error(message.id, Number.isInteger(cause?.rpcCode) ? cause.rpcCode : -32603, 'Internal error');
        }).finally(() => {
          cancelled.delete(message.id);
          queuedIds.delete(message.id);
          controllers.delete(message.id);
          queued -= 1;
        });
    };

    process.stdin.on('data', (chunk) => {
      const data = Buffer.from(chunk);
      let offset = 0;
      while (offset < data.length) {
        const newline = data.indexOf(0x0a, offset);
        const end = newline === -1 ? data.length : newline;
        const segment = data.subarray(offset, end);
        if (!discardingOversizedLine) {
          if (bufferedBytes + segment.length > MAX_REQUEST_BYTES) {
            chunks = [];
            bufferedBytes = 0;
            discardingOversizedLine = true;
            error(null, -32600, 'Request exceeds maximum line size');
          } else if (segment.length) {
            chunks.push(segment);
            bufferedBytes += segment.length;
          }
        }
        if (newline !== -1) {
          if (!discardingOversizedLine) acceptLine(Buffer.concat(chunks, bufferedBytes));
          resetLine();
          offset = newline + 1;
        } else {
          offset = data.length;
        }
      }
    });
    process.stdin.on('end', () => {
      if (!discardingOversizedLine && bufferedBytes > 0) acceptLine(Buffer.concat(chunks, bufferedBytes));
      chain.then(resolve, reject);
    });
    process.stdin.on('error', reject);
  });
}
