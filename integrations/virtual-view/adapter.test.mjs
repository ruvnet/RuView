import test from 'node:test';
import assert from 'node:assert/strict';
import { createVirtualViewAdapter } from './adapter.mjs';
test('disabled by default and never actuates', () => {
  const adapter = createVirtualViewAdapter({ broker: () => { throw Error('not called'); } });
  assert.equal(adapter.observe({}).reason, 'disabled'); assert.equal(adapter.hardwareActuation, false);
});
test('raw RF is rejected; explicit scene is forwarded to validating broker', () => {
  let received;
  const adapter = createVirtualViewAdapter({ enabled: true, broker: s => { received = s; return { authority: 'none' }; } });
  assert.throws(() => adapter.observe({ schema: 'spatial.evidence.v1' }));
  const s = { schema: 'virtual-view.v1', frame: 'room_enu' };
  assert.equal(adapter.observe(s).authority, 'none'); assert.equal(received, s);
});
