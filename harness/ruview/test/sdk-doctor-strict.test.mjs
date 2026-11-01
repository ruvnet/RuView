import test from 'node:test';
import assert from 'node:assert/strict';
import { createRuView, RuViewError } from '../src/sdk.js';
const deps = { python: () => null, findRepoRoot: () => null, fetch: async () => { throw new Error('offline fixture'); } };
const options = { groups: ['sensing'], sensing_url: 'file:///invalid-protocol' };

test('strict SDK doctor rejects a failing diagnostic with its report attached', async () => {
  const ruview = createRuView({ strict: true, deps });
  await assert.rejects(() => ruview.doctor(options), (error) => error instanceof RuViewError && error.tool === 'ruview_doctor' && error.result.ok === false && error.result.summary.fail === 1);
});

test('ordinary SDK doctor returns failing diagnostics; strict successful doctor remains a report', async () => {
  const ordinary = await createRuView({ deps }).doctor(options);
  assert.equal(ordinary.ok, false);
  const successful = await createRuView({ strict: true, deps }).doctor({ groups: ['runtime'] });
  assert.equal(successful.ok, true);
});
