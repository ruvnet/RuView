import { test } from 'node:test';
import assert from 'node:assert/strict';
import { visiblePersons, frameSource, reconnectDelayMs, sourceBanner, LIVE_STALE_MS, isSensingFrame } from './live-data.js';
import { HudController } from './hud-controller.js';

test('live figures require positive, finite keypoint confidence; demo figures remain illustrative', () => {
  const unmeasured = [
    null, { id: 1 }, { id: 2, keypoints: [] },
    { id: 3, keypoints: [{ confidence: 0 }, { confidence: 0 }] },
    { id: 4, keypoints: [{ confidence: NaN }, { confidence: -1 }] },
  ];
  const measured = { id: 5, keypoints: [{ confidence: 0.8 }] };
  const data = { persons: [...unmeasured, measured] };
  assert.deepEqual(visiblePersons(data, true), [measured]);
  assert.equal(visiblePersons(data, false), data.persons);
  assert.deepEqual(visiblePersons(null, true), []);
});

test('live HUD uses the received classification and clears demo module badges', () => {
  const elements = new Map();
  for (const id of ['scenario-description', 'edge-modules-bar', 'scenario-quick-select', 'autoplay-icon']) {
    elements.set(id, { textContent: '', innerHTML: '', style: {}, value: '' });
  }
  globalThis.document = { getElementById: id => elements.get(id) || null };
  const obs = { settings: { dataSource: 'demo' } };
  const hud = new HudController(obs);
  const demo = { _autoMode: false, currentScenario: 'single_breathing' };
  hud.updateHUD({ classification: { motion_level: 'absent' } }, demo);
  const demoCaption = elements.get('scenario-description').textContent;
  assert.ok(elements.get('edge-modules-bar').innerHTML.includes('VITALS'));
  obs.settings.dataSource = 'ws';
  hud.updateHUD({ classification: { motion_level: 'present_moving' } }, demo);
  assert.equal(elements.get('scenario-description').textContent, 'Live measurements: present moving');
  assert.equal(elements.get('edge-modules-bar').style.display, 'none');
  assert.equal(elements.get('edge-modules-bar').innerHTML, '');
  assert.equal(elements.get('scenario-quick-select').disabled, true);
  hud.updateHUD({ classification: { motion_level: 'absent' } }, demo);
  assert.equal(elements.get('scenario-description').textContent, 'Live measurements: absent');
  obs.settings.dataSource = 'demo';
  hud.updateHUD({}, demo);
  assert.equal(elements.get('scenario-quick-select').disabled, false);
  assert.equal(elements.get('scenario-description').textContent, demoCaption);
});

test('live mode never draws demo frames: before the first frame, after a drop, or when frames go stale', () => {
  const frame = { classification: { presence: true } };
  const now = 100000;
  assert.equal(frameSource({ dataSource: 'demo', liveFrame: frame, liveAtMs: now, nowMs: now }), 'demo');
  assert.equal(frameSource({ dataSource: 'ws', liveFrame: frame, liveAtMs: now - 100, nowMs: now }), 'live');
  assert.equal(frameSource({ dataSource: 'ws', liveFrame: null, liveAtMs: NaN, nowMs: now }), 'no-data');
  assert.equal(frameSource({ dataSource: 'ws', liveFrame: frame, liveAtMs: NaN, nowMs: now }), 'no-data');
  assert.equal(frameSource({ dataSource: 'ws', liveFrame: frame, liveAtMs: now - LIVE_STALE_MS - 1, nowMs: now }), 'no-data');
  assert.equal(frameSource({ dataSource: 'ws', liveFrame: frame, liveAtMs: now - LIVE_STALE_MS, nowMs: now }), 'live');
  for (const liveFrame of [null, frame]) {
    for (const liveAtMs of [NaN, 0, now - 1, now - 60000]) {
      assert.notEqual(frameSource({ dataSource: 'ws', liveFrame, liveAtMs, nowMs: now }), 'demo');
    }
  }
});

test('reconnect backs off to 16 s and never stops', () => {
  assert.deepEqual([0, 1, 2, 3, 4, 5, 50].map(reconnectDelayMs), [1000, 2000, 4000, 8000, 16000, 16000, 16000]);
  assert.equal(reconnectDelayMs(-3), 1000);
  assert.equal(reconnectDelayMs(undefined), 1000);
});

test('every non-live state carries a banner that says so; live carries none', () => {
  const demo = sourceBanner('demo', { reason: 'No sensing server answered at http://x:3000.' });
  assert.equal(demo.tone, 'demo');
  assert.match(demo.title, /SIMULATED/);
  assert.match(demo.detail, /not|Nothing/);
  assert.match(demo.detail, /No sensing server answered/);
  const none = sourceBanner('no-data', { wsUrl: 'ws://x:3000/ws/sensing', attempt: 3 });
  assert.equal(none.tone, 'no-data');
  assert.match(none.detail, /ws:\/\/x:3000\/ws\/sensing/);
  assert.match(none.detail, /attempt 3/);
  assert.equal(sourceBanner('live'), null);
});

test('HUD badge and banner follow what is drawn', () => {
  const el = () => ({ textContent: '', innerHTML: '', style: {}, value: '', className: '', hidden: false });
  const elements = new Map(['data-source-label', 'data-source-banner', 'data-source-banner-title',
    'data-source-banner-detail', 'scenario-description', 'edge-modules-bar', 'scenario-quick-select',
    'autoplay-icon'].map(id => [id, el()]));
  const dot = el();
  globalThis.document = { getElementById: id => elements.get(id) || null, querySelector: () => dot };
  const obs = { settings: { dataSource: 'ws' } };
  const hud = new HudController(obs);
  const banner = elements.get('data-source-banner');

  hud.updateSource('no-data', { wsUrl: 'ws://h/ws/sensing', attempt: 1 });
  assert.equal(elements.get('data-source-label').textContent, 'NO DATA');
  assert.equal(banner.hidden, false);
  assert.equal(banner.className, 'source-banner source-banner--no-data');
  assert.equal(elements.get('data-source-banner-title').textContent, 'NO LIVE DATA');
  hud.updateHUD({}, { _autoMode: false, currentScenario: 'single_breathing' });
  assert.match(elements.get('scenario-description').textContent, /^No live data/);

  hud.updateSource('live', {});
  assert.equal(elements.get('data-source-label').textContent, 'LIVE');
  assert.equal(dot.className, 'dot dot--live');
  assert.equal(banner.hidden, true);

  obs.settings.dataSource = 'demo';
  hud.updateSource('demo', { reason: 'Demo selected in settings.' });
  assert.equal(elements.get('data-source-label').textContent, 'DEMO');
  assert.equal(banner.hidden, false);
  assert.equal(elements.get('data-source-banner-title').textContent, 'DEMO: SIMULATED DATA');
  assert.match(elements.get('data-source-banner-detail').textContent, /Demo selected in settings/);
});

test('only sensing frames replace the live picture; multiplexed messages are ignored', () => {
  assert.equal(isSensingFrame({ type: 'sensing_update', classification: {} }), true);
  assert.equal(isSensingFrame({ classification: {} }), true);
  for (const m of [{ type: 'edge_vitals', node_id: 3 }, { type: 'pose_data' }, null, [], 'x', 7]) {
    assert.equal(isSensingFrame(m), false);
  }
});
