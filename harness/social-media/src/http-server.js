// SPDX-License-Identifier: MIT

import { createServer } from 'node:http';
import { runDoctor } from './doctor.js';
import { listPlatformCapabilities } from './platforms.js';

const MAX_CONNECTIONS = 128;

function send(response, status, value) {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    'cache-control': 'no-store',
    'content-length': Buffer.byteLength(body),
    'content-type': 'application/json; charset=utf-8',
    'x-content-type-options': 'nosniff',
  });
  response.end(body);
}

export function controlPlaneResponse(method, requestUrl) {
  if (method !== 'GET') return { status: 405, body: { ok: false, error: 'method_not_allowed' } };
  const path = new URL(requestUrl || '/', 'http://localhost').pathname;
  if (path === '/healthz') {
    const doctor = runDoctor();
    return { status: doctor.ok ? 200 : 503, body: { ok: doctor.ok, schema: doctor.schema, checks: doctor.checks } };
  }
  if (path === '/v1/capabilities') {
    return {
      status: 200,
      body: {
        ok: true,
        authority: 'read_only_zero_credential',
        executionAuthorized: false,
        capabilities: listPlatformCapabilities(),
      },
    };
  }
  return { status: 404, body: { ok: false, error: 'not_found' } };
}

export function createControlPlaneServer() {
  const server = createServer((request, response) => {
    const output = controlPlaneResponse(request.method, request.url);
    send(response, output.status, output.body);
  });
  server.maxConnections = MAX_CONNECTIONS;
  server.requestTimeout = 5_000;
  server.headersTimeout = 5_000;
  server.keepAliveTimeout = 5_000;
  return server;
}

export function startHttpServer() {
  const port = Number.parseInt(process.env.PORT || '8080', 10);
  if (!Number.isSafeInteger(port) || port < 1 || port > 65535) throw new TypeError('PORT must be between 1 and 65535');
  const server = createControlPlaneServer();
  server.listen(port, '0.0.0.0', () => {
    process.stderr.write(`[social-metaharness-http] read-only control plane listening on ${port}\n`);
  });
  const close = () => server.close(() => process.exit(0));
  process.once('SIGTERM', close);
  process.once('SIGINT', close);
  return server;
}
