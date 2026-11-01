// Executed regression test for #2087: the dashboard Streaming card read
// "IDLE, 0 client(s)" while the page's own socket was streaming, because the
// card is fed by a 30 s /health poll whose first read lands before the socket
// connects.
//
// Run: node --test ui/components/DashboardTab.test.mjs
//
// This EXECUTES DashboardTab's refresh wiring in Node with fake timers and a
// stubbed health service. It does not render the card in a browser.

import { test, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';

globalThis.localStorage = { getItem: () => null, setItem() {}, removeItem() {} };
globalThis.fetch = async () => ({ ok: true, status: 200, json: async () => ({}) });

const { DashboardTab } = await import('./DashboardTab.js');
const { healthService } = await import('../services/health.service.js');

let healthCalls;
let tab;

beforeEach(() => {
  healthCalls = 0;
  healthService.getSystemHealth = async () => { healthCalls++; return {}; };
  mock.timers.enable({ apis: ['setTimeout'] });
  tab = new DashboardTab({ querySelector: () => null });
});

const advance = () => mock.timers.tick(DashboardTab.STREAM_HEALTH_REFRESH_DELAY_MS);

test('health is re-read once the sensing socket connects', () => {
  tab._refreshStreamHealthOn('connecting');
  advance();
  assert.equal(healthCalls, 0, 'nothing to report until the socket is up');

  tab._refreshStreamHealthOn('connected');
  assert.equal(healthCalls, 0, 'waits for the server to register the socket');
  advance();
  assert.equal(healthCalls, 1);
  mock.timers.reset();
});

test('health is re-read when a connected socket drops', () => {
  tab._refreshStreamHealthOn('connected');
  advance();
  tab._refreshStreamHealthOn('reconnecting');
  advance();
  assert.equal(healthCalls, 2);
  mock.timers.reset();
});

test('repeated notifications of the same state do not refetch', () => {
  // onStateChange also fires on dataSource changes with an unchanged state.
  tab._refreshStreamHealthOn('connected');
  tab._refreshStreamHealthOn('connected');
  tab._refreshStreamHealthOn('connected');
  advance();
  assert.equal(healthCalls, 1);
  mock.timers.reset();
});

test('retry churn while never connected does not poll health', () => {
  for (const s of ['connecting', 'reconnecting', 'connecting', 'reconnecting']) {
    tab._refreshStreamHealthOn(s);
    advance();
  }
  assert.equal(healthCalls, 0);
  mock.timers.reset();
});
