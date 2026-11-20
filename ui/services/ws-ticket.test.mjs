// Executed tests for the WebSocket ticket helper (ADR-272).
//
// Run: node --test ui/sw.test.mjs ui/services/ws-ticket.test.mjs
// (a directory argument does not work — Node resolves it as a module)
//
// WHAT THIS IS AND IS NOT.
// This EXECUTES the module in Node with stubbed `fetch` and `localStorage`. It
// is strictly more than the `node --check` syntax pass this file replaces, and
// strictly less than a browser: it does not exercise a real WebSocket upgrade,
// real cookie handling, or the page wiring. The claim "the UI JavaScript has
// never been run" is no longer true of this module; "browser-tested" still is
// not. Both statements matter and neither should be rounded up.

import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

const STORAGE_KEY = 'ruview-api-token';

// --- stubs installed before the module under test is imported ---------------
let stored = {};
let fetchCalls = [];
let fetchImpl = async () => ({ ok: true, status: 200, json: async () => ({ ticket: 'T' }) });

globalThis.localStorage = {
  getItem: (k) => (k in stored ? stored[k] : null),
  setItem: (k, v) => { stored[k] = String(v); },
  removeItem: (k) => { delete stored[k]; },
};
globalThis.fetch = async (...args) => { fetchCalls.push(args); return fetchImpl(...args); };

const { withWsTicket } = await import('./ws-ticket.js');

beforeEach(() => {
  stored = {};
  fetchCalls = [];
  fetchImpl = async () => ({ ok: true, status: 200, json: async () => ({ ticket: 'T' }) });
});

test('with no stored token the URL is returned unchanged and nothing is fetched', async () => {
  // Auth is off, so no ticket is needed. Minting one would be a pointless
  // round-trip on every reconnect.
  const url = await withWsTicket('ws://host/ws/sensing');
  assert.equal(url, 'ws://host/ws/sensing');
  assert.equal(fetchCalls.length, 0);
});

test('a minted ticket is appended and the bearer is sent only in the header', async () => {
  stored[STORAGE_KEY] = 'secret-bearer';
  const url = await withWsTicket('ws://host/ws/sensing');

  assert.equal(url, 'ws://host/ws/sensing?ticket=T');
  // The long-lived bearer must never reach a URL — only the bounded ticket does.
  assert.ok(!url.includes('secret-bearer'), `bearer leaked into URL: ${url}`);

  const [path, init] = fetchCalls[0];
  assert.equal(path, '/api/v1/ws-ticket');
  assert.equal(init.method, 'POST');
  assert.equal(init.headers.Authorization, 'Bearer secret-bearer');
});

test('an existing query string gets & rather than a second ?', async () => {
  stored[STORAGE_KEY] = 'b';
  const url = await withWsTicket('ws://host/ws/sensing?foo=1');
  assert.equal(url, 'ws://host/ws/sensing?foo=1&ticket=T');
});

test('the ticket value is URL-encoded', async () => {
  stored[STORAGE_KEY] = 'b';
  fetchImpl = async () => ({ ok: true, status: 200, json: async () => ({ ticket: 'a+b/c=d' }) });
  const url = await withWsTicket('ws://host/ws/sensing');
  assert.ok(url.endsWith('ticket=a%2Bb%2Fc%3Dd'), url);
});

test('404 means a server predating ADR-272, so connect without a ticket', async () => {
  // That server still exempts WebSockets, so an unticketed connect is correct.
  // This is what lets one UI work against both old and new servers, which is
  // what makes the legacy escape hatch removable rather than permanent.
  stored[STORAGE_KEY] = 'b';
  fetchImpl = async () => ({ ok: false, status: 404, json: async () => ({}) });
  assert.equal(await withWsTicket('ws://host/ws/sensing'), 'ws://host/ws/sensing');
});

test('a 503 does not append a ticket and does not throw', async () => {
  // Ticket store exhausted. The socket attempt will fail on its own and the
  // caller's reconnect logic handles it; throwing here would duplicate that.
  stored[STORAGE_KEY] = 'b';
  fetchImpl = async () => ({ ok: false, status: 503, json: async () => ({}) });
  assert.equal(await withWsTicket('ws://host/ws/sensing'), 'ws://host/ws/sensing');
});

test('a network failure is swallowed rather than breaking the connect path', async () => {
  stored[STORAGE_KEY] = 'b';
  fetchImpl = async () => { throw new Error('offline'); };
  assert.equal(await withWsTicket('ws://host/ws/sensing'), 'ws://host/ws/sensing');
});

test('a 200 with no ticket field yields no ticket', async () => {
  stored[STORAGE_KEY] = 'b';
  fetchImpl = async () => ({ ok: true, status: 200, json: async () => ({}) });
  assert.equal(await withWsTicket('ws://host/ws/sensing'), 'ws://host/ws/sensing');
});

test('a fresh ticket is minted per call and never reused', async () => {
  // Tickets are single-use and expire in ~30s, so caching one across reconnects
  // fails on the second attempt.
  stored[STORAGE_KEY] = 'b';
  let n = 0;
  fetchImpl = async () => ({ ok: true, status: 200, json: async () => ({ ticket: `T${++n}` }) });

  assert.equal(await withWsTicket('ws://h/ws/sensing'), 'ws://h/ws/sensing?ticket=T1');
  assert.equal(await withWsTicket('ws://h/ws/sensing'), 'ws://h/ws/sensing?ticket=T2');
  assert.equal(fetchCalls.length, 2, 'each connect attempt must mint its own');
});

// ---- #2099: auth-required detection ----------------------------------------

const { resolveWsUrl, probeWsAuthRequired } = await import('./ws-ticket.js');

test('a stored token that the server refuses is reported as rejected', async () => {
  // Retrying with a token the server has refused can never succeed, so the
  // caller needs to know rather than loop.
  stored[STORAGE_KEY] = 'wrong';
  for (const status of [401, 403]) {
    fetchImpl = async () => ({ ok: false, status, json: async () => ({}) });
    const r = await resolveWsUrl('ws://host/ws/sensing');
    assert.deepEqual(r, { url: 'ws://host/ws/sensing', authRequired: 'rejected' });
  }
});

test('withWsTicket keeps its string contract when the token is refused', async () => {
  stored[STORAGE_KEY] = 'wrong';
  fetchImpl = async () => ({ ok: false, status: 401, json: async () => ({}) });
  assert.equal(await withWsTicket('ws://host/ws/sensing'), 'ws://host/ws/sensing');
});

test('a 503 or a 404 is not an auth refusal', async () => {
  stored[STORAGE_KEY] = 'b';
  for (const status of [404, 503]) {
    fetchImpl = async () => ({ ok: false, status, json: async () => ({}) });
    assert.equal((await resolveWsUrl('ws://h/ws/sensing')).authRequired, null);
  }
});

test('the probe reports a missing token when the server answers 401 with no bearer', async () => {
  fetchImpl = async () => ({ ok: false, status: 401, json: async () => ({}) });
  assert.equal(await probeWsAuthRequired(), 'missing');
  const [path, init] = fetchCalls[0];
  assert.equal(path, '/api/v1/ws-ticket');
  assert.equal(init.method, 'POST');
  assert.equal(init.headers, undefined, 'the probe must not send a bearer');
  assert.equal(init.credentials, 'same-origin', 'a signed-in session cookie still counts');
});

test('the probe treats success, other errors and network failure as not-auth', async () => {
  fetchImpl = async () => ({ ok: true, status: 200, json: async () => ({ ticket: 'T' }) });
  assert.equal(await probeWsAuthRequired(), null, 'auth off or a valid session');
  fetchImpl = async () => ({ ok: false, status: 500, json: async () => ({}) });
  assert.equal(await probeWsAuthRequired(), null);
  fetchImpl = async () => { throw new Error('offline'); };
  assert.equal(await probeWsAuthRequired(), null, 'an outage is not an auth problem');
});

test('the probe does not second-guess a stored token', async () => {
  stored[STORAGE_KEY] = 'b';
  assert.equal(await probeWsAuthRequired(), null);
  assert.equal(fetchCalls.length, 0);
});
