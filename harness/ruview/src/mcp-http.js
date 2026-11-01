// SPDX-License-Identifier: MIT
// MCP over Streamable HTTP (ADR-375) — what ChatGPT and other remote MCP
// clients connect to. Same handler and FIFO tool chain as stdio, plus:
//   - authentication on every request: `Authorization: Bearer <token>` or the
//     secret-path form `/mcp/<token>` (ChatGPT connectors cannot send a static
//     bearer header), compared in constant time;
//   - loopback bind by default; a non-loopback bind is the operator's choice;
//   - Origin allowlist (DNS-rebinding defence): browser origins are refused
//     unless listed; server-to-server clients send no Origin;
//   - write grants (workspace-write, hardware-write) are never honoured over
//     HTTP, so flashing and calibration stay stdio/CLI-only;
//   - single JSON responses (no SSE stream), bounded bodies, no sessions.

import http from 'node:http';
import { randomBytes, timingSafeEqual, createHash } from 'node:crypto';
import { createDispatcher, handleRpc, MAX_REQUEST_BYTES, parseGrants, SERVER_INFO, SUPPORTED_PROTOCOLS } from './mcp-server.js';

export const DEFAULT_HTTP_PORT = 8790;
export const HTTP_WRITE_GRANTS = Object.freeze(['workspace-write', 'hardware-write']);
const LOOPBACK = new Set(['127.0.0.1', '::1', 'localhost']);

const digest = (s) => createHash('sha256').update(String(s)).digest();
function sameSecret(a, b) {
  return typeof a === 'string' && a.length > 0 && timingSafeEqual(digest(a), digest(b));
}

export function generateToken() {
  return randomBytes(24).toString('base64url');
}

/** Grants usable over HTTP: everything the operator granted except writes. */
export function httpGrants(grants) {
  return grants.filter((g) => !HTTP_WRITE_GRANTS.includes(g));
}

function send(res, status, body, headers = {}) {
  const payload = body === undefined ? '' : JSON.stringify(body);
  res.writeHead(status, {
    'content-type': 'application/json',
    'cache-control': 'no-store',
    'x-content-type-options': 'nosniff',
    ...(payload ? { 'content-length': Buffer.byteLength(payload) } : {}),
    ...headers,
  });
  res.end(payload);
}

/**
 * Start the HTTP transport. opts: { host='127.0.0.1', port=8790, token,
 * allowOrigins=[], grants=env RUVIEW_MCP_GRANTS, log, handler=handleRpc }. Resolves to
 * { server, url, secretUrl, token, grants, close() }.
 */
export function startMcpHttp(opts = {}) {
  const host = opts.host || '127.0.0.1';
  const port = opts.port ?? DEFAULT_HTTP_PORT;
  const token = opts.token || generateToken();
  if (token.length < 16) return Promise.reject(new Error('token must be at least 16 characters'));
  const allowOrigins = new Set(opts.allowOrigins || []);
  const requested = opts.grants || parseGrants();
  const grants = httpGrants(requested);
  const log = opts.log || ((...a) => process.stderr.write('[ruview-mcp-http] ' + a.join(' ') + '\n'));
  const dropped = requested.filter((g) => !grants.includes(g));
  if (dropped.length) log(`write grants are never honoured over HTTP; ignoring: ${dropped.join(', ')}`);
  const { dispatch } = createDispatcher({ source: 'mcp', transport: 'http', grants }, opts.handler || handleRpc);

  const server = http.createServer((req, res) => {
    let url;
    try { url = new URL(req.url || '/', 'http://localhost'); } catch {
      return send(res, 400, { error: 'invalid_request_target' });
    }
    if (req.method === 'GET' && url.pathname === '/healthz') return send(res, 200, { ok: true, name: SERVER_INFO.name, version: SERVER_INFO.version });

    const segments = url.pathname.split('/').filter(Boolean);
    if (segments[0] !== 'mcp' || segments.length > 2) return send(res, 404, { error: 'not_found' });
    const auth = /^Bearer\s+(.+)$/i.exec(req.headers.authorization || '');
    if (!sameSecret(segments[1] || (auth && auth[1].trim()), token)) {
      return send(res, 401, { error: 'unauthorized' }, { 'www-authenticate': 'Bearer realm="ruview"' });
    }
    const origin = req.headers.origin;
    if (origin && !allowOrigins.has(origin)) return send(res, 403, { error: 'origin_not_allowed' });
    if (req.method !== 'POST') return send(res, 405, { error: 'method_not_allowed' }, { allow: 'POST' });
    const version = req.headers['mcp-protocol-version'];
    if (version && !SUPPORTED_PROTOCOLS.includes(version)) return send(res, 400, { error: 'unsupported_protocol_version', supported: SUPPORTED_PROTOCOLS });

    let size = 0;
    const chunks = [];
    let aborted = false;
    req.on('data', (chunk) => {
      size += chunk.length;
      if (size > MAX_REQUEST_BYTES) { aborted = true; send(res, 413, { error: 'request_too_large' }); req.destroy(); return; }
      chunks.push(chunk);
    });
    req.on('end', async () => {
      if (aborted) return;
      let msg;
      try { msg = JSON.parse(Buffer.concat(chunks).toString('utf8')); } catch {
        return send(res, 400, { jsonrpc: '2.0', id: null, error: { code: -32700, message: 'Parse error' } });
      }
      if (Array.isArray(msg)) return send(res, 400, { jsonrpc: '2.0', id: null, error: { code: -32600, message: 'Batches are not supported' } });
      const reply = await dispatch(msg);
      if (!reply) { res.writeHead(202, { 'cache-control': 'no-store' }); return res.end(); }
      send(res, 200, reply);
    });
  });
  // Tool runs (captures, analysis) can take minutes; headers must still arrive fast.
  server.headersTimeout = 10_000;
  server.requestTimeout = 15 * 60_000;

  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, () => {
      server.off('error', reject);
      const address = server.address();
      const shownHost = address.family === 'IPv6' ? `[${address.address}]` : address.address;
      const base = `http://${shownHost}:${address.port}/mcp`;
      resolve({
        server, token, grants, loopback: LOOPBACK.has(host),
        url: base, secretUrl: `${base}/${token}`,
        close: () => new Promise((r) => { server.closeAllConnections?.(); server.close(() => r()); }),
      });
    });
  });
}
