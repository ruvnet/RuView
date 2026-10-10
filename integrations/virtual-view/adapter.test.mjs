import test from 'node:test';
import assert from 'node:assert/strict';
import { createVirtualViewAdapter } from './adapter.mjs';
test('disabled by default and never actuates', () => {
  const adapter = createVirtualViewAdapter({ broker: () => { throw Error('not called'); } });
  let reads = 0;
  const snapshot = { get schema() { reads++; throw Error('not read'); } };
  assert.deepEqual(adapter.observe(snapshot), {
    decision: 'BLOCK', reason: 'disabled', authority: 'none', svg: null,
  });
  assert.equal(reads, 0);
  assert.equal(adapter.hardwareActuation, false);
  assert.equal(Object.isFrozen(adapter), true);
});

test('raw RF, LiDAR and malformed scene markers never reach the broker', () => {
  let calls = 0;
  const adapter = createVirtualViewAdapter({ enabled: true, broker: () => { calls++; } });
  for (const snapshot of [
    null, undefined, 'virtual-view.v1', [], {},
    { schema: 'spatial.evidence.v1', frame: 'room_enu' },
    { schema: 'ruview.lidar.depth.v1', frame: 'room_enu' },
    { schema: 'virtual-view.v1' },
    { schema: 'virtual-view.v1', frame: 'camera' },
    Object.create({ schema: 'virtual-view.v1', frame: 'room_enu' }),
    Object.defineProperty({ frame: 'room_enu' }, 'schema', { value: 'virtual-view.v1' }),
  ]) {
    assert.throws(() => adapter.observe(snapshot), /calibrated metric scene snapshot required/);
  }
  assert.equal(calls, 0);
});

test('scene marker getters are rejected without executing input code', () => {
  let reads = 0, calls = 0;
  const adapter = createVirtualViewAdapter({ enabled: true, broker: () => { calls++; } });
  for (const key of ['schema', 'frame']) {
    const snapshot = { schema: 'virtual-view.v1', frame: 'room_enu' };
    Object.defineProperty(snapshot, key, {
      enumerable: true,
      get() { reads++; return key === 'schema' ? 'virtual-view.v1' : 'room_enu'; },
    });
    assert.throws(() => adapter.observe(snapshot), /calibrated metric scene snapshot required/);
  }
  assert.equal(reads, 0);
  assert.equal(calls, 0);
});

test('proxy snapshots are rejected without running inspection traps', () => {
  let traps = 0, calls = 0;
  const snapshot = new Proxy({ schema: 'virtual-view.v1', frame: 'room_enu' }, {
    get() { traps++; throw Error('get trap'); },
    getPrototypeOf() { traps++; throw Error('prototype trap'); },
    getOwnPropertyDescriptor() { traps++; throw Error('descriptor trap'); },
  });
  const adapter = createVirtualViewAdapter({ enabled: true, broker: () => { calls++; } });
  assert.throws(() => adapter.observe(snapshot), /calibrated metric scene snapshot required/);
  assert.equal(traps, 0);
  assert.equal(calls, 0);
});

test('nested object and array getters are rejected without reads or broker calls', () => {
  let reads = 0, calls = 0;
  const adapter = createVirtualViewAdapter({ enabled: true, broker: () => { calls++; } });
  const getter = { enumerable: true, get() { reads++; return 0; } };
  const object = Object.defineProperty({}, 'uncertaintyM', getter);
  const array = Object.defineProperty([0], '0', getter);
  for (const extra of [object, [object], { nested: array }]) {
    assert.throws(() => adapter.observe({ schema: 'virtual-view.v1', frame: 'room_enu', extra }),
      /calibrated metric scene snapshot required/);
  }
  assert.equal(reads, 0);
  assert.equal(calls, 0);
});

test('nested proxies and custom array prototypes never execute input code', () => {
  let traps = 0, calls = 0;
  const adapter = createVirtualViewAdapter({ enabled: true, broker: () => { calls++; } });
  const handler = {
    get() { traps++; throw Error('get trap'); },
    getPrototypeOf() { traps++; throw Error('prototype trap'); },
    ownKeys() { traps++; throw Error('keys trap'); },
    getOwnPropertyDescriptor() { traps++; throw Error('descriptor trap'); },
  };
  const custom = Object.setPrototypeOf([], {
    get [Symbol.iterator]() { traps++; throw Error('iterator getter'); },
  });
  class SceneArray extends Array {}
  for (const extra of [
    { nested: new Proxy({}, handler) },
    [new Proxy([], handler)],
    { nested: custom },
    new SceneArray(),
  ]) {
    assert.throws(() => adapter.observe({ schema: 'virtual-view.v1', frame: 'room_enu', extra }),
      /calibrated metric scene snapshot required/);
  }
  assert.equal(traps, 0);
  assert.equal(calls, 0);
});

test('cyclic, non-JSON and oversized structures are rejected before the broker', () => {
  let calls = 0;
  const adapter = createVirtualViewAdapter({ enabled: true, broker: () => { calls++; } });
  const cyclic = {}; cyclic.self = cyclic;
  let deep = 0;
  for (let index = 0; index < 13; index++) deep = { next: deep };
  const sparse = Array(2); sparse[1] = 0;
  const extraProperty = [0]; extraProperty.extra = 0;
  for (const extra of [
    cyclic, deep, sparse, extraProperty, NaN, Infinity, undefined, () => {}, new Date(),
    'x'.repeat(4097), Array(1025).fill(0),
    { ['x'.repeat(129)]: 0 },
    Object.fromEntries(Array.from({ length: 65 }, (_, index) => [`k${index}`, 0])),
    Array.from({ length: 20 }, () => Array(1024).fill(0)),
  ]) {
    assert.throws(() => adapter.observe({ schema: 'virtual-view.v1', frame: 'room_enu', extra }),
      /calibrated metric scene snapshot required/);
  }
  assert.equal(calls, 0);
});

test('a complete 256-object synthetic scene stays within the structural budget', () => {
  const scene = {
    schema: 'virtual-view.v1', sceneId: 'fixture', requestId: 'request:1', policyId: 'fixed',
    revision: 1, frame: 'room_enu', proof: 'SYNTHETIC', nowMs: 1000, observedMs: 990,
    contact: 'static', target: 'o0', reference: 'o1', physicalAxis: 'x',
    objects: Array.from({ length: 256 }, (_, index) => ({
      id: `o${index}`, observed: [index / 100, index % 3, 0.1],
      predicted: [index / 100, index % 3, 0.1], size: [0.01, 0.01, 0.01],
      uncertaintyM: 0.015, sourceReceipt: `fixture:${index}`,
    })),
  };
  let calls = 0;
  const result = { authority: 'none' };
  const adapter = createVirtualViewAdapter({ enabled: true, broker: received => {
    calls++;
    assert.equal(received, scene);
    return result;
  } });
  assert.equal(adapter.observe(scene), result);
  assert.equal(calls, 1);
});

test('explicit immutable scene and signer are forwarded unchanged to trusted broker', () => {
  const snapshot = Object.freeze({ schema: 'virtual-view.v1', frame: 'room_enu' });
  const privateKey = Object.freeze({ researchSigner: true });
  const result = Object.freeze({ decision: 'SKIP', authority: 'none', svg: null });
  let calls = 0;
  const adapter = createVirtualViewAdapter({ enabled: true, privateKey, broker: (received, options) => {
    calls++;
    assert.equal(received, snapshot);
    assert.equal(options.privateKey, privateKey);
    return result;
  } });
  assert.equal(adapter.observe(snapshot), result);
  assert.equal(calls, 1);
});

test('full scene validation remains delegated and broker rejection propagates', () => {
  const rejection = new TypeError('missing tracked scene geometry');
  const adapter = createVirtualViewAdapter({ enabled: true, broker: () => { throw rejection; } });
  assert.throws(() => adapter.observe({ schema: 'virtual-view.v1', frame: 'room_enu' }),
    error => error === rejection);
});

test('enablement must be explicit and configuration types are checked', () => {
  for (const configuration of [
    { broker: null }, { broker: () => {}, enabled: 'true' }, { broker: () => {}, enabled: 1 },
  ]) assert.throws(() => createVirtualViewAdapter(configuration), /adapter configuration/);
});
