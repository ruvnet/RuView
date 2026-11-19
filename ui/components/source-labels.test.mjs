import { test } from 'node:test';
import assert from 'node:assert/strict';

globalThis.localStorage = { getItem: () => null, setItem() {}, removeItem() {} };
const { sensingService } = await import('../services/sensing.service.js');
const { DashboardTab } = await import('./DashboardTab.js');
const { SensingTab } = await import('./SensingTab.js');
const { LiveDemoTab } = await import('./LiveDemoTab.js');

test('all live views display the real modality instead of assuming ESP32 (#2558)', () => {
  const text = { textContent: '' };
  const message = { textContent: '' };
  const card = { querySelector: selector => selector === '.status-text' ? text : message };
  const sensingBanner = {};
  const demoBanner = {};
  const connection = {};
  const elements = {
    '#dashboard-datasource': card,
    '#sensingSourceBanner': sensingBanner,
    '#demo-source-banner': demoBanner,
    '#connection-status': connection,
  };
  const container = { querySelector: selector => elements[selector] || null };
  const dashboard = new DashboardTab(container);
  const sensing = new SensingTab(container);
  const demo = new LiveDemoTab(container);
  demo.state.isActive = true;

  for (const [source, expected] of [
    ['wifi:unnamed', 'HOST WIFI (RSSI ONLY)'],
    ['esp32', 'ESP32 HARDWARE'],
    ['live', 'SOURCE UNSPECIFIED'],
  ]) {
    sensingService._applyServerSource(source, 'live_unverified');
    dashboard.updateDataSourceIndicator();
    sensing._onStateChange('connected');
    demo.updateSourceBanner();
    demo.updateMetricsDisplay();
    for (const label of [text.textContent, sensingBanner.textContent, demoBanner.textContent,
      connection.textContent, demo.getStatusText()]) {
      assert.ok(label?.includes(expected), `${source}: ${label}`);
      if (source !== 'esp32') assert.ok(!label.includes('ESP32'));
    }
  }
});
