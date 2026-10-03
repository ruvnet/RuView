// Executed regression test for #2099: with RUVIEW_API_TOKEN set on the server
// and no token in the browser, the sensing service retried forever and the
// header said "Connecting..." with no hint that a token was needed.
//
// Run: node --test ui/services/sensing.service.test.mjs
//
// This EXECUTES SensingService in Node with a stub WebSocket that refuses the
// upgrade the way a browser reports a 401 (close code 1006, no open), and a
// stubbed fetch standing in for the server. It does not drive a browser or the
// page wiring (banner, header widget, QuickSettings).

import { test, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

const STORAGE_KEY = 'ruview-api-token';

let stored = {};
let fetchCalls = [];
// Server stand-in: auth on, no session. Status probe and ticket mint both 401.
let ticketStatus = 401;
let sockets = [];
let refuseUpgrade = true;

globalThis.localStorage = {
  getItem: (k) => (k in stored ? stored[k] : null),
  setItem: (k, v) => { stored[k] = String(v); },
  removeItem: (k) => { delete stored[k]; },
};
globalThis.fetch = async (path, init = {}) => {
  fetchCalls.push([path, init]);
  if (path === '/api/v1/ws-ticket') {
    const ok = ticketStatus === 200;
    return { ok, status: ticketStatus, json: async () => (ok ? { ticket: 'T' } : {}) };
  }
  return { ok: true, status: 200, json: async () => ({ source: 'simulated' }) };
};

class FakeWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  constructor(url) {
    this.url = url;
    this.readyState = 0;
    sockets.push(this);
    setTimeout(() => {
      if (refuseUpgrade) {
        this.readyState = 3;
        this.onerror?.({});
        this.onclose?.({ code: 1006 });
      } else {
        this.readyState = 1;
        this.onopen?.();
      }
    }, 0);
  }
  close() { this.readyState = 3; }
}
globalThis.WebSocket = FakeWebSocket;

const { SensingService } = await import('./sensing.service.js');

const settle = (ms = 30) => new Promise((r) => setTimeout(r, ms));
let svc;

beforeEach(() => {
  stored = {};
  fetchCalls = [];
  ticketStatus = 401;
  sockets = [];
  refuseUpgrade = true;
  svc = new SensingService();
});

afterEach(() => svc.stop());

test('no token + server requires one: stops retrying and says why', async () => {
  const states = [];
  svc.onStateChange((s) => states.push(s));
  svc.start();
  await settle();

  assert.equal(svc.state, 'auth-required');
  assert.equal(svc.dataSource, 'auth-required');
  assert.equal(svc.authRequired, 'missing');
  assert.equal(svc._reconnectTimer, null, 'no retry may be pending');

  // Before the fix this kept opening sockets on a 1-16 s back-off forever.
  await settle(1200);
  assert.equal(sockets.length, 1, 'exactly one refused attempt, then no retries');
  assert.ok(!states.includes('reconnecting'), `went through reconnecting: ${states}`);
});

test('a stored token the server rejects is reported without opening a socket', async () => {
  stored[STORAGE_KEY] = 'stale';
  svc.start();
  await settle();
  assert.equal(svc.state, 'auth-required');
  assert.equal(svc.authRequired, 'rejected');
  assert.equal(sockets.length, 0);
});

test('a plain outage still reconnects (auth off, server down)', async () => {
  // Server unreachable: the probe's fetch fails too.
  const realFetch = globalThis.fetch;
  globalThis.fetch = async () => { throw new Error('ECONNREFUSED'); };
  try {
    svc.start();
    await settle();
    assert.equal(svc.state, 'reconnecting');
    assert.equal(svc.authRequired, null);
    assert.ok(svc._reconnectTimer, 'a retry must be scheduled');
  } finally {
    globalThis.fetch = realFetch;
  }
});

test('auth off and server up: no probe on the happy path', async () => {
  ticketStatus = 200;
  refuseUpgrade = false;
  svc.start();
  await settle();
  assert.equal(svc.state, 'connected');
  assert.ok(!fetchCalls.some(([p]) => p === '/api/v1/ws-ticket'),
    'no ticket request when no token is stored and the socket opens');
});

test('reconnect() leaves auth-required and tries again', async () => {
  svc.start();
  await settle();
  assert.equal(svc.state, 'auth-required');

  refuseUpgrade = false;
  svc.reconnect();
  await settle();
  assert.equal(svc.state, 'connected');
  assert.equal(svc.authRequired, null);
});
