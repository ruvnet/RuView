import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRuView, RuViewError, FIRMWARE_VARIANTS } from '../src/sdk.js';
import { TOOL_POLICY } from '../src/policy.js';
import { TOOLS } from '../src/tools.js';

test('SDK exposes every registry tool and typed helpers', async () => {
  const ruview = createRuView();
  assert.equal(ruview.tools().length, Object.keys(TOOLS).length);
  assert.ok(Object.isFrozen(ruview) && Object.isFrozen(ruview.firmware) && Object.isFrozen(ruview.training));
  assert.deepEqual(Object.keys(FIRMWARE_VARIANTS), ['s3-8mb', 's3-4mb', 'c6', 'c5']);
  const claim = await ruview.claimCheck('We achieve 100% accuracy.');
  assert.equal(claim.ok, false);
  const gate = await ruview.training.gate({ model_score: 0.7, baseline_score: 0.5, split: 'blocked-gap', train_end: '2026-01-01', test_start: '2026-01-05', n_test: 200 });
  assert.equal(gate.verdict, 'PASS');
});

test('SDK flash without confirm returns a plan; strict mode throws on failure', async () => {
  const ruview = createRuView();
  const plan = await ruview.firmware.flash({ port: 'COM7', bundle: '/missing/bundle' });
  assert.equal(plan.reason, 'bundle_missing');
  const strict = createRuView({ strict: true });
  await assert.rejects(() => strict.firmware.plan({ port: 'COM7', bundle: '/missing/bundle' }), (e) => e instanceof RuViewError && e.reason === 'bundle_missing');
});

test('policy files list exactly the registry tools (no drift)', () => {
  const claims = JSON.parse(readFileSync(new URL('../.harness/claims.json', import.meta.url))).policy;
  const mcp = JSON.parse(readFileSync(new URL('../.harness/mcp-policy.json', import.meta.url)));
  assert.deepEqual(Object.keys(TOOL_POLICY).sort(), Object.keys(TOOLS).sort());
  const readOnly = Object.entries(TOOL_POLICY).filter(([, p]) => p.readOnly && !p.requiredGrant).map(([n]) => n).sort();
  assert.deepEqual([...claims.readOnlyTools].sort(), readOnly);
  assert.deepEqual([...mcp.readOnlyTools].sort(), readOnly);
  const guarded = Object.entries(TOOL_POLICY).filter(([, p]) => p.readOnly && p.requiredGrant);
  for (const [name, p] of guarded) {
    assert.ok(claims.grants[p.requiredGrant]?.tools.includes(name), `${name} missing from claims grant ${p.requiredGrant}`);
    assert.equal(mcp.guardedReadTools[name]?.grant, p.requiredGrant, `${name} missing from mcp-policy guardedReadTools`);
  }
  const confirmed = Object.entries(TOOL_POLICY).filter(([, p]) => p.confirmField).map(([n]) => n).sort();
  assert.deepEqual(Object.keys(mcp.dangerousTools).sort(), confirmed);
});


test('SDK forwards explicit search limits to the registry validator', async () => {
  const sdk = createRuView();
  for (const limit of [0, NaN, -1, 26]) {
    const result = await sdk.memorySearch('firmware', limit);
    assert.equal(result.ok, false, `invalid limit ${String(limit)} must not become the default`);
    assert.equal(result.reason, 'invalid_arguments');
  }
  const strict = createRuView({ strict: true });
  await assert.rejects(() => strict.memorySearch('firmware', 0),
    (error) => error instanceof RuViewError && error.reason === 'invalid_arguments');
  assert.deepEqual(await sdk.memorySearch('firmware'), await sdk.call('ruview_memory_search', { query: 'firmware' }));
  assert.deepEqual(await sdk.memorySearch('firmware', 1), await sdk.call('ruview_memory_search', { query: 'firmware', limit: 1 }));
});
