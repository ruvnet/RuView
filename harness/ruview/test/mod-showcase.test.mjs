// The ruview-live showcase (ADR-378): cell graphics, animation frames and the
// waterfall/radar views, under plain Node. Engine behaviour (blit, keys) is
// covered by `claude plugin test mod`.
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  ANIMATED, Braille, colormap, openArgsOf, VIEW_ROWS, commandsOf, DEFAULT, drain, Grid, lineChart, modelOf, phaseOf, picturesOf, pingOf, pulse,
  radarFan, rgb, settingsOf, shimmer, shownFrames, spectrumOf, spectrumWith, toBase64, viewOf, waterfall,
} from '../mod/hooks/register.mjs';

const decode = ({ columns, rows, cells }) => {
  const b = Buffer.from(cells, 'base64');
  assert.equal(b.length, columns * rows * 12, 'three u32 per cell');
  return new Uint32Array(b.buffer.slice(b.byteOffset, b.byteOffset + b.length));
};
const el = (type) => (props) => ({ type, props });
const UI = { Box: el('Box'), Text: el('Text'), Button: el('Button'), Raster: el('Raster') };
const flat = (n, out = []) => { if (n && typeof n === 'object') { out.push(n); [].concat(n.props?.children ?? []).forEach((c) => flat(c, out)); } return out; };
const spectrumCapture = (synthetic) => ({
  ok: true, packets: 40, decodedPackets: 40, nodes: [{ source: 'esp32', nodeId: 6, csiRateHz: 13.7, csiLossFraction: 0, rssiMean: -50, csi: { shape: '1x256', synthetic } }],
  spectrum: [{ source: 'esp32', nodeId: 6, subcarriers: 256, bins: 4, rateHz: 13.7, synthetic, frames: Array.from({ length: 40 }, (_, k) => [k, 2 * k, 3 * k, 4 * k]) }],
});
const radar = { ok: true, device: { name: 'kit' }, presentNow: true, targetsMax: 1, distanceCmMean: 120, heartBpmMean: 72, breathingBpmMean: 14 };

test('base64 and Grid pack the Raster contract exactly', () => {
  for (const n of [0, 1, 2, 3, 4, 5, 100]) {
    const bytes = Uint8Array.from({ length: n }, (_, i) => (i * 37) & 255);
    assert.equal(toBase64(bytes), Buffer.from(bytes).toString('base64'));
  }
  const g = new Grid(3, 2);
  g.set(1, 1, 'A', rgb(1, 2, 3), rgb(255, 0, 0));
  g.set(9, 9, 'Z');
  const u = decode(g.toRaster('k'));
  assert.deepEqual([...u.slice(0, 3)], [0x20, DEFAULT, DEFAULT], 'blank cells use the terminal colours');
  assert.deepEqual([...u.slice(12, 15)], [65, 0x010203, 0xff0000]);
  assert.deepEqual([new Grid(9999, 9999).columns, new Grid(9999, 9999).rows], [512, 256], 'clamped to the Raster bounds');
});

test('colour map is monotone in brightness and clamps', () => {
  const lum = (c) => ((c >> 16) & 255) * 0.3 + ((c >> 8) & 255) * 0.59 + (c & 255) * 0.11;
  let last = -1;
  for (let k = 0; k <= 20; k++) { const l = lum(colormap(k / 20)); assert.ok(l >= last - 1); last = l; }
  assert.equal(colormap(-5), colormap(0));
  assert.equal(colormap(NaN), colormap(0));
});

test('waterfall: newest frame at the bottom, two frames per row, robust limits', () => {
  const frames = Array.from({ length: 10 }, (_, k) => [k, k, k, k]);
  const { grid, lo, hi, shown } = waterfall(frames, 4, 3);
  assert.equal(shown, 6, 'rows × 2 frames');
  assert.ok(lo >= 4 && hi <= 9);
  const u = decode(grid.toRaster('w'));
  const bottom = (2 * 4) * 3;
  assert.equal(u[bottom], 0x2580, 'upper half block');
  assert.equal(u[bottom + 2], colormap(1), 'lower half of the last row is the newest frame');
  assert.doesNotThrow(() => waterfall([], 8, 4));
});

test('braille canvas sets the right dots and keeps the higher-priority colour', () => {
  const b = new Braille(2, 1);
  b.plot(0, 0, 1, 0); b.plot(1, 3, 2, 5); b.plot(0, 1, 3, 1);
  const u = decode(b.toGrid().toRaster('b'));
  assert.equal(u[0], 0x2800 + 0x01 + 0x80 + 0x02);
  assert.equal(u[1], 2, 'priority 5 wins the cell colour');
  assert.equal(u[3], 0x20, 'an empty cell is a space');
});

test('radar fan scales to the reading, labels rings, and pings honestly', () => {
  const near = radarFan(60, 18, { distances: [80, 90, 100], present: true });
  const far = radarFan(60, 18, { distances: [400], present: false });
  assert.ok(near.maxCm < far.maxCm);
  assert.ok(near.maxCm >= 150 && far.maxCm <= 600);
  const text = (g) => String.fromCodePoint(...[...decode(g.toRaster('f'))].filter((_, i) => i % 3 === 0));
  assert.match(text(near.grid), /1m/);
  const a = text(radarFan(60, 18, { distances: [100], ping: 0.2 }).grid);
  const b = text(radarFan(60, 18, { distances: [100], ping: 0.7 }).grid);
  assert.notEqual(a, b, 'the ping moves');
  assert.doesNotThrow(() => radarFan(10, 6, { distances: [], ping: 0.5 }), 'no reading, no ping');
});

test('line chart fits the data so a rhythm stays visible, shades the band where it overlaps', () => {
  const { lo, hi } = lineChart([70, 72, 74, 72, 70], 20, 3, { band: [40, 180], minSpan: 10 });
  assert.ok(hi - lo < 20, `scale ${lo}–${hi} fits the data, not the whole 40–180 band`);
  const shaded = decode(lineChart([70, 72], 10, 3, { band: [40, 180] }).grid.toRaster('c'));
  assert.ok([...shaded].some((v, i) => i % 3 === 2 && v !== DEFAULT), 'band shaded');
  const outside = decode(lineChart([300, 310], 10, 3, { band: [40, 180] }).grid.toRaster('c'));
  assert.ok([...outside].every((v, i) => i % 3 !== 2 || v === DEFAULT), 'no band when it is off the scale');
});

test('animation frames: shimmer moves, pulse follows the reported rates, replay drains', () => {
  assert.notDeepEqual(shimmer(40, 0).toRaster('s').cells, shimmer(40, 600).toRaster('s').cells);
  assert.equal(shimmer(40, 0).toRaster('s').cells, shimmer(40, 2400).toRaster('s').cells, 'a 2.4 s loop');
  assert.equal(phaseOf(1000, 60), 0, 'one beat a second');
  assert.equal(phaseOf(500, 60), 0.5);
  assert.equal(phaseOf(500, null), 0);
  const beat = (t) => decode(pulse(20, t, { heartBpm: 60, breathingBpm: 15 }).toRaster('p'))[1];
  assert.notEqual(beat(0), beat(500), 'the heart dims between beats');
  const still = decode(pulse(20, 123, {}).toRaster('p'));
  assert.equal(still[3 * 3], 0x2500, 'unknown breathing draws an empty gauge');
  assert.equal(drain(10, 20, 250), 5);
  assert.equal(drain(1, 20, 1000), 0);
  assert.equal(drain(NaN, 20, 100), 0);
  assert.deepEqual(shownFrames([1, 2, 3, 4], 2.7), [1, 2]);
  assert.deepEqual(shownFrames([1, 2], 9), []);
  assert.equal(pingOf(800), 0.5);
});

test('every animated picture keeps its size across frames (a blit must match the mount)', () => {
  const model = { ...modelOf(spectrumCapture(false), radar, 0) };
  const history = { heart: [70, 72], breathing: [14, 15], distance: [110, 120], rates: {} };
  for (const mode of ['overview', 'waterfall', 'radar']) {
    for (const columns of [70, 120, 200]) {
      const opts = { mode, columns, rows: 34, history, lag: { 'esp32:6': 30 } };
      const a = picturesOf(model, { ...opts, t: 0 });
      const b = picturesOf(model, { ...opts, t: 1234, lag: { 'esp32:6': 3 } });
      for (const key of ANIMATED) {
        assert.equal(Boolean(a[key]), Boolean(b[key]), `${mode}/${key}`);
        if (a[key]) assert.deepEqual([a[key].grid.columns, a[key].grid.rows], [b[key].grid.columns, b[key].grid.rows], `${mode}/${key}@${columns}`);
      }
    }
  }
});

test('waterfall view labels MEASURED vs SYNTHETIC and needs a terminal', () => {
  const opts = { mode: 'waterfall', refreshMs: 15000, liveRefreshMs: 4000, columns: 120, rows: 34, onMode() {}, onRefresh() {}, onClose() {} };
  for (const synthetic of [false, true]) {
    const texts = flat(viewOf(UI, modelOf(spectrumCapture(synthetic), null, 0), opts)).filter((n) => n.type === 'Text').map((n) => String(n.props.children));
    assert.ok(texts.some((t) => (synthetic ? /^SYNTHETIC/ : /^MEASURED/).test(t)));
  }
  const tree = flat(viewOf(UI, modelOf(spectrumCapture(false), null, 0), opts));
  assert.ok(tree.some((n) => n.type === 'Raster' && n.props.key === 'waterfall'));
  assert.deepEqual(tree.filter((n) => n.type === 'Button').map((b) => b.props.hotkey), ['1', '2', '3', 'r', 'c'], 'no next-node key for one node');
  const desk = JSON.stringify(viewOf({ Box: UI.Box, Text: UI.Text, Button: UI.Button }, modelOf(spectrumCapture(false), null, 0), opts));
  assert.match(desk, /open it in the terminal/, 'surfaces without Raster get a note, not a refused tree');
  const empty = JSON.stringify(viewOf(UI, modelOf({ ok: false, reason: 'no_packets' }, null, 0), opts));
  assert.match(empty, /nothing to draw/);
});

test('radar view: fan, pulse and charts, with the honesty labels', () => {
  const opts = { mode: 'radar', refreshMs: 15000, liveRefreshMs: 4000, columns: 130, rows: 34, radarConfigured: true, history: { heart: [70, 72], breathing: [14, 15], distance: [110, 120], rates: {} }, onMode() {}, onRefresh() {}, onClose() {} };
  const tree = flat(viewOf(UI, modelOf(null, radar, 0), opts));
  const keys = tree.filter((n) => n.type === 'Raster').map((n) => n.props.key);
  assert.deepEqual(keys, ['shimmer', 'fan', 'pulse', 'chart-heart', 'chart-breathing', 'chart-distance']);
  const s = JSON.stringify(tree.map((n) => (n.type === 'Text' ? n.props.children : '')));
  assert.match(s, /range only: this kit reports distance, not bearing/);
  assert.match(s, /a metronome, not a waveform/);
  assert.match(s, /device-reported, not validated/);
  const down = JSON.stringify(viewOf(UI, modelOf(null, { ok: false, reason: 'timeout' }, 0), opts));
  assert.match(down, /unreachable/);
});

test('spectrum requests, merging across captures, and live commands', () => {
  assert.deepEqual(spectrumOf(120, 34), { bins: 114, frames: 38 });
  assert.deepEqual(spectrumOf(10, 5), { bins: 24, frames: 12 });
  const c = commandsOf(settingsOf({ radarHost: 'kit.local' }), { live: true, spectrum: { bins: 64, frames: 48 } });
  assert.deepEqual(c.capture, ['esp32', '--seconds', '2', '--udp-port', '5005', '--json', '--spectrum', '--spectrum-bins', '64', '--spectrum-frames', '48']);
  assert.equal(c.radar[c.radar.indexOf('--seconds') + 1], '2');
  const a = { key: 'esp32:6', bins: 4, frames: [[1], [2]] };
  const merged = spectrumWith(spectrumWith([], [a]), [{ ...a, frames: [[3]] }], 2);
  assert.deepEqual(merged[0].frames, [[2], [3]], 'frames carry across captures, bounded');
  assert.deepEqual(spectrumWith(merged, [{ ...a, bins: 8, frames: [[9]] }])[0].frames, [[9]], 'a new bin count starts over');
});

test('an unfocused pane says how to give it the keys; a focused one does not', () => {
  const base = { mode: 'overview', refreshMs: 15000, columns: 120, rows: 34, onMode() {}, onRefresh() {}, onClose() {} };
  const model = modelOf(spectrumCapture(false), null, 0);
  assert.match(JSON.stringify(viewOf(UI, model, { ...base, focused: false })), /ctrl\+x tab to use the keys/);
  assert.doesNotMatch(JSON.stringify(viewOf(UI, model, { ...base, focused: true })), /ctrl\+x tab/);
});

test('the pane opens as a dialog that takes the keys, sized to the view', () => {
  for (const mode of ['overview', 'waterfall', 'radar']) {
    const a = openArgsOf(mode);
    assert.deepEqual([a.id, a.focus, a.closeOnEscape, a.holdToasts], ['ruview-live', true, true, true], 'focus + closeOnEscape + holdToasts = a dialog that takes the keys');
    assert.equal(a.rows, VIEW_ROWS[mode]);
  }
  assert.ok(VIEW_ROWS.waterfall > VIEW_ROWS.overview, 'the live views ask for more room');
});

test('review fixes: a silent node leaves the waterfall; a new CSI shape starts over', () => {
  const a = { key: 'esp32:6', shape: '1x64', bins: 4, frames: [[1], [2]] };
  const held = spectrumWith([], [a]);
  assert.deepEqual(spectrumWith(held, []), [], 'a failed capture or silent node never keeps old frames on screen');
  const other = { key: 'esp32:7', shape: '1x64', bins: 4, frames: [[5]] };
  assert.deepEqual(spectrumWith(held, [other]).map((s) => s.key), ['esp32:7']);
  const reshaped = spectrumWith(held, [{ ...a, shape: '1x128', frames: [[9]] }]);
  assert.deepEqual(reshaped[0].frames, [[9]], 'same bin count, different layout: not joined');
  const opts = { mode: 'waterfall', refreshMs: 15000, liveRefreshMs: 4000, columns: 120, rows: 34, onMode() {}, onRefresh() {}, onClose() {} };
  const gone = modelOf({ ok: false, reason: 'no_packets' }, null, 0);
  assert.match(JSON.stringify(viewOf(UI, { ...gone, spectrum: spectrumWith(held, gone.spectrum) }, opts)), /nothing to draw/, 'the empty card returns once frames stop');
});
