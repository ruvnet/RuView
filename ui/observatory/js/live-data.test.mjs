import { test } from 'node:test';
import assert from 'node:assert/strict';
import { visiblePersons } from './live-data.js';
import { HudController } from './hud-controller.js';
import { sensingService } from '../../services/sensing.service.js';

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

test('rssi modality suppresses every skeleton; presence data still flows', () => {
  const measured = { id: 5, keypoints: [{ confidence: 0.8 }] };
  const data = { persons: [measured], classification: { presence: true } };
  assert.deepEqual(visiblePersons(data, true, 'rssi'), []);
  // CSI and unspecified modalities keep the confidence gate as second defense.
  assert.deepEqual(visiblePersons(data, true, 'csi'), [measured]);
  assert.deepEqual(visiblePersons(data, true, undefined), [measured]);
  assert.deepEqual(visiblePersons(data, true, 'unknown'), [measured]);
});

test('live HUD uses the received classification and clears demo module badges', () => {
  const elements = new Map();
  for (const id of ['scenario-description', 'edge-modules-bar', 'scenario-quick-select', 'autoplay-icon']) {
    elements.set(id, { textContent: '', innerHTML: '', style: {}, value: '' });
  }
  globalThis.document = { getElementById: id => elements.get(id) || null };
  // Server frames render only when the service is live and demo is not forced.
  sensingService._applyServerSource('esp32', 'live_verified');
  const obs = { settings: { dataSource: 'demo' } };
  const hud = new HudController(obs);
  const demo = { _autoMode: false, currentScenario: 'single_breathing' };
  hud.updateHUD({ classification: { motion_level: 'absent' } }, demo, false);
  const demoCaption = elements.get('scenario-description').textContent;
  assert.ok(elements.get('edge-modules-bar').innerHTML.includes('VITALS'));
  obs.settings.dataSource = 'auto';
  hud.updateHUD({ classification: { motion_level: 'present_moving' } }, demo, true);
  assert.equal(elements.get('scenario-description').textContent, 'Live measurements: present moving');
  assert.equal(elements.get('edge-modules-bar').style.display, 'none');
  assert.equal(elements.get('edge-modules-bar').innerHTML, '');
  assert.equal(elements.get('scenario-quick-select').disabled, true);
  hud.updateHUD({ classification: { motion_level: 'absent' } }, demo, true);
  assert.equal(elements.get('scenario-description').textContent, 'Live measurements: absent');
  obs.settings.dataSource = 'demo';
  hud.updateHUD({}, demo);
  assert.equal(elements.get('scenario-quick-select').disabled, false);
  assert.equal(elements.get('scenario-description').textContent, demoCaption);
});

test('observatory badge is three-state: LIVE / RSSI-ONLY / DEMO', () => {
  const dot = { className: '' };
  const label = { textContent: '' };
  globalThis.document = {
    getElementById: id => (id === 'data-source-label' ? label : null),
    querySelector: sel => (sel === '#data-source-badge .dot' ? dot : null),
  };
  const obs = { settings: { dataSource: 'auto' } };
  const hud = new HudController(obs);

  sensingService._applyServerSource('esp32', 'live_verified');
  hud.updateSourceBadge();
  assert.equal(label.textContent, 'LIVE');
  assert.ok(dot.className.includes('dot--live'));

  sensingService._applyServerSource('wifi:HomeNetwork', 'live_unverified');
  hud.updateSourceBadge();
  assert.equal(label.textContent, 'RSSI-ONLY');
  assert.ok(dot.className.includes('dot--rssi'));

  // Dropped connection or forced demo never shows LIVE/RSSI-ONLY.
  sensingService._applyServerSource('wifi:HomeNetwork', 'stale');
  hud.updateSourceBadge();
  assert.equal(label.textContent, 'DEMO');
  sensingService._applyServerSource('esp32', 'live_verified');
  obs.settings.dataSource = 'demo';
  hud.updateSourceBadge();
  assert.equal(label.textContent, 'DEMO');
});
