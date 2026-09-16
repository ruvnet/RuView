// Regression tests for the WebSocket reconnect state machine.
//
// Requested by the reviewer of the live-view PR ("add deterministic regressions"),
// whose second changed-code defect was: "explicit disconnect can immediately
// schedule reconnection again".
//
// Run: node --test ui/services/ws-reconnect.test.mjs
// (also listed in .github/workflows/ci.yml so CI executes it)

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  reconnectState, scheduleReconnect, cancelReconnect, reconnectSucceeded,
  suppressReconnect, allowReconnect, isReconnectSuppressed,
  RECONNECT_MAX_FAILURES,
} from './ws-reconnect.js';

test('a dropped link schedules a retry', () => {
  const state = reconnectState();
  let attempts = 0;
  scheduleReconnect(state, () => { attempts += 1; }, () => {});
  assert.notEqual(state._reconnectTimer, null, 'a retry must be pending');
  cancelReconnect(state);
});

test('a manual disconnect does not schedule a retry, even though closing fires onclose', () => {
  // The rejected behaviour: disconnect() closed the socket, which invoked the
  // page's onclose handler, which called scheduleReconnect unconditionally — so a
  // deliberate disconnect reconnected itself a second later.
  const state = reconnectState();
  suppressReconnect(state);
  let attempts = 0;
  scheduleReconnect(state, () => { attempts += 1; }, () => {});
  assert.equal(state._reconnectTimer, null, 'nothing may be scheduled while suppressed');
  assert.equal(attempts, 0);
  assert.equal(isReconnectSuppressed(state), true);
});

test('an explicit connect clears the suppression', () => {
  const state = reconnectState();
  suppressReconnect(state);
  allowReconnect(state);
  assert.equal(isReconnectSuppressed(state), false);
  let attempts = 0;
  scheduleReconnect(state, () => { attempts += 1; }, () => {});
  assert.notEqual(state._reconnectTimer, null, 'retries work again after an explicit connect');
  cancelReconnect(state);
});

test('a verified frame resets the backoff and cancels the pending retry', () => {
  const state = reconnectState();
  state._reconnectFailures = 3;
  state._reconnectDelay = 8000;
  scheduleReconnect(state, () => {}, () => {});
  reconnectSucceeded(state);
  assert.equal(state._reconnectFailures, 0);
  assert.equal(state._reconnectDelay, 1000);
  assert.equal(state._reconnectTimer, null, 'the pending retry is cancelled by success');
});

test('retries give up after the failure budget', async () => {
  const state = reconnectState();
  state._reconnectFailures = RECONNECT_MAX_FAILURES; // one more attempt crosses the cap
  state._reconnectDelay = 1;                         // keep the test fast
  let attempts = 0;
  let gaveUp = 0;
  scheduleReconnect(state, () => { attempts += 1; }, () => { gaveUp += 1; });
  await new Promise((r) => setTimeout(r, 40));
  assert.equal(gaveUp, 1, 'the page must fall back to the demo view after the budget');
  assert.equal(attempts, 0, 'the crossing attempt is not run');
});

test('only one retry is ever pending at a time', () => {
  const state = reconnectState();
  let scheduled = 0;
  scheduleReconnect(state, () => { scheduled += 1; }, () => {});
  const first = state._reconnectTimer;
  scheduleReconnect(state, () => { scheduled += 1; }, () => {});
  assert.equal(state._reconnectTimer, first, 'a second close must not stack another timer');
  cancelReconnect(state);
});
