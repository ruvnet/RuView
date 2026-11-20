// SPDX-License-Identifier: MIT
// ruview-live drawing (ADR-377, ADR-378): the overview cards and the two
// showcase views (CSI waterfall, radar fan + vitals charts), built from the
// surface's own elements. Pictures are `Raster`s (terminal); other surfaces
// get an honest note instead. `picturesOf` builds every animated picture, so
// the first paint and each `$.ui.blit` frame share one size and one source.

import { pingOf, pulse, shimmer, shownFrames } from './anim.mjs';
import { ageOf, plausibilityOf, RANGES, sparkline } from './model.mjs';
import { colourbar, lineChart, radarFan, rgb, waterfall } from './raster.mjs';

const TABS = [['overview', '1', 'Overview'], ['waterfall', '2', 'CSI waterfall'], ['radar', '3', 'Radar']];
/** Keys of the pictures the hooks module repaints with `$.ui.blit`. */
export const ANIMATED = Object.freeze(['shimmer', 'waterfall', 'fan', 'pulse']);

/** Picture sizes that fit the pane body. */
export function sizesOf(columns = 80, rows = 30) {
  const width = Math.max(24, Math.min(128, columns - 6));
  const height = Math.max(6, Math.min(26, rows - 15)); // the header, tabs, card chrome and footer take ~15 rows
  return { width, height };
}

const wrapIndex = (i, n) => ((i % n) + n) % n;
const radarLayout = (columns, rows) => {
  const { width, height } = sizesOf(columns, rows);
  const wide = (columns ?? 80) >= 110;
  const fanW = wide ? Math.floor(width * 0.55) : width;
  // A 120° wedge is about width / (4·sin 60°) rows tall.
  const fanH = Math.max(6, Math.min(height, Math.round(fanW / 3.4) + 1));
  return { width, wide, fanW, fanH, chartW: wide ? Math.max(20, width - fanW - 6) : width };
};

/**
 * Every animated picture for this mode at time `t` (ms): `{ shimmer, waterfall,
 * fan, pulse }`, each `{ grid, ... }` or null. opts: { mode, columns, rows, t,
 * nodeIndex, lag (per node key), history }.
 */
export function picturesOf(model, opts) {
  const mode = opts.mode || 'overview';
  const t = Number.isFinite(opts.t) ? opts.t : 0;
  const columns = opts.columns ?? 80;
  const out = { shimmer: { grid: shimmer(Math.max(8, Math.min(160, columns - 4)), t) }, waterfall: null, fan: null, pulse: null };
  if (!model) return out;
  if (mode === 'waterfall' && model.spectrum?.length) {
    const nodes = model.spectrum;
    const node = nodes[wrapIndex(opts.nodeIndex ?? 0, nodes.length)];
    const { width, height } = sizesOf(columns, opts.rows);
    out.waterfall = { ...waterfall(shownFrames(node.frames, opts.lag?.[node.key]), width, height), node, nodes };
  }
  if (mode === 'radar' && model.radar) {
    const history = opts.history || { distance: [] };
    const { fanW, fanH, chartW } = radarLayout(columns, opts.rows);
    out.fan = radarFan(fanW, fanH, { distances: history.distance, present: model.radar.present, ping: pingOf(t) });
    out.pulse = { grid: pulse(Math.min(chartW, 40), t, { heartBpm: model.radar.heartBpm, breathingBpm: model.radar.breathingBpm }) };
  }
  return out;
}

/**
 * Draw the pane. opts: { mode, nodeIndex, refreshMs, liveRefreshMs, busy, now, t, lag, columns, rows,
 * history, udpPort, radarConfigured, onRefresh, onClose, onMode(mode), onNextNode }.
 */
export function viewOf(ui, model, opts) {
  const { Box, Text, Button } = ui;
  const mode = opts.mode || 'overview';
  const live = mode !== 'overview';
  const interval = live ? (opts.liveRefreshMs ?? opts.refreshMs) : opts.refreshMs;
  const t = (children, props = {}) => Text({ wrap: 'truncate-end', ...props, children });
  const row = (children, props = {}) => Box({ flexDirection: 'row', gap: 1, ...props, children: children.filter(Boolean) });
  const pics = ui.Raster ? picturesOf(model, opts) : {};

  // Header: freshness badge and age, then the signal rule.
  const finiteAt = model && Number.isFinite(model.at);
  const age = finiteAt && Number.isFinite(opts.now) ? opts.now - model.at : NaN;
  const fresh = Number.isFinite(age) && age <= interval * 2 + 5000;
  const when = finiteAt ? new Date(model.at).toLocaleTimeString() : '—';
  const badge = !model ? t('○ STARTING', { dimColor: true, bold: true })
    : fresh ? t('● LIVE', { color: 'green', bold: true }) : t('○ STALE', { color: 'yellow', bold: true });
  const header = row([
    t('RuView', { color: 'cyan', bold: true }),
    badge,
    t(model ? `updated ${when}${ageOf(age) ? ` (${ageOf(age)})` : ''} · every ${Math.round(interval / 1000)}s` : `waiting for the first capture… · every ${Math.round(interval / 1000)}s`, { dimColor: true }),
    opts.busy ? t('⟳ refreshing', { color: 'cyan' }) : null,
  ]);
  const rule = pics.shimmer ? ui.Raster(pics.shimmer.grid.toRaster('shimmer')) : null;

  // Hotkeys reach a Pane only while it holds the keyboard; say how to give it.
  const tabs = opts.onMode ? row([
    ...TABS.map(([m, key, label]) => Button({
      key: `tab-${m}`, hotkey: key, label: `${m === mode ? '▸ ' : ''}${label} (${key})`, onPress: () => opts.onMode(m),
    })),
    opts.focused === false ? t('click the pane or press ctrl+x tab to use the keys', { dimColor: true, italic: true }) : null,
  ], { gap: 2 }) : null;

  const alerts = (model?.alerts || []).map((a) => Text({ color: a.level === 'bad' ? 'red' : 'yellow', wrap: 'wrap', children: `! ${a.text}` }));

  let body;
  if (mode === 'waterfall') body = waterfallView(ui, model, opts, pics, t, row);
  else if (mode === 'radar') body = radarView(ui, model, opts, pics, t, row);
  else body = overview(ui, model, opts, t, row);

  const footer = row([
    Button({ key: 'refresh', label: opts.busy ? 'Refreshing…' : 'Refresh (r)', hotkey: 'r', onPress: opts.onRefresh }),
    mode === 'waterfall' && opts.onNextNode && (model?.spectrum?.length ?? 0) > 1 ? Button({ key: 'next-node', label: 'Next node (n)', hotkey: 'n', onPress: opts.onNextNode }) : null,
    Button({ key: 'close', label: 'Close (c)', hotkey: 'c', onPress: opts.onClose }),
  ], { gap: 2 });

  return Box({ flexDirection: 'column', gap: 1, paddingX: 1, children: [Box({ flexDirection: 'column', children: [header, rule].filter(Boolean) }), tabs, ...alerts, body, footer].filter(Boolean) });
}

function card(ui, title, subtitle, borderColor, children, t, row) {
  return ui.Box({
    flexDirection: 'column', borderStyle: 'round', borderColor, paddingX: 1, flexGrow: 1,
    children: [row([t(title, { bold: true }), subtitle ? t(subtitle, { dimColor: true }) : null]), ...children.filter(Boolean)],
  });
}

function needsTerminal(ui, t) {
  return ui.Raster ? null : t('This view draws with terminal cells; open it in the terminal. The overview works everywhere.', { dimColor: true });
}

function overview(ui, model, opts, t, row) {
  if (!model) return null;
  const history = opts.history || { heart: [], breathing: [], distance: [], rates: {} };
  const vital = (label, value, raw, kind, series) => {
    const warn = kind ? plausibilityOf(kind, raw) : null;
    return row([
      t(label.padEnd(10), { dimColor: true }),
      t(value.padStart(9), { bold: raw != null && !warn, color: warn ? 'yellow' : undefined }),
      t(sparkline(series), { color: 'cyan' }),
      warn ? t(`⚠ ${warn}`, { color: 'yellow' }) : null,
    ]);
  };
  let radarCard = null;
  if (model.radar) {
    const r = model.radar;
    const presence = r.present == null ? t('? presence unknown', { dimColor: true })
      : r.present ? t(`● PRESENCE DETECTED${r.targets ? ` · ${r.targets} target${r.targets === 1 ? '' : 's'}` : ''}`, { color: 'green', bold: true })
        : t('○ no presence', { dimColor: true });
    radarCard = card(ui, '60 GHz RADAR', r.name, r.present ? 'green' : 'gray', [
      presence,
      vital('distance', r.distance, r.distanceCm, null, history.distance),
      vital('heart', r.heart, r.heartBpm, 'heart', history.heart),
      vital('breathing', r.breathing, r.breathingBpm, 'breathing', history.breathing),
      t('device-reported values, not validated against a reference', { dimColor: true, italic: true }),
    ], t, row);
  } else if (!opts.radarConfigured) {
    radarCard = card(ui, '60 GHz RADAR', 'not configured', 'gray', [
      t('Set radarHost to an ESPHome radar kit:', { dimColor: true }),
      t('claude plugin configure ruview-live', { color: 'cyan' }),
    ], t, row);
  }
  const lines = model.nodes.length
    ? model.nodes.map((n) => row([
      t(n.label.padEnd(10), { bold: true }),
      t(n.rate.padStart(9)),
      t(sparkline(history.rates[n.key]), { color: 'cyan' }),
      t(`loss ${n.loss}`, n.lossy ? { color: 'yellow' } : { dimColor: true }),
      t(n.rssi, { dimColor: true }),
      t(n.shape, { dimColor: true }),
      n.synthetic ? t('SYNTHETIC', { color: 'yellow', bold: true }) : null,
    ]))
    : [
      t('none streaming to this machine', { dimColor: true }),
      model.noPackets ? t(`check node target IP/port · firewall UDP ${opts.udpPort ?? 5005}`, { dimColor: true }) : null,
    ];
  const nodesCard = card(ui, 'CSI NODES', `UDP ${opts.udpPort ?? 5005} · ${model.decoded}/${model.packets} decoded`, model.nodes.length ? 'cyan' : 'gray', [
    ...lines,
    model.nodes.length ? t('press 2 for the live CSI waterfall', { dimColor: true, italic: true }) : null,
  ], t, row);
  const cards = [radarCard, nodesCard].filter(Boolean);
  return ui.Box({ flexDirection: (opts.columns ?? 80) >= 100 ? 'row' : 'column', gap: 1, children: cards });
}

function waterfallView(ui, model, opts, pics, t, row) {
  const note = needsTerminal(ui, t);
  if (note) return note;
  if (!model) return t('waiting for the first capture…', { dimColor: true });
  if (!pics.waterfall) {
    return card(ui, 'CSI WATERFALL', 'no CSI frames', 'gray', [
      t('No CSI node is streaming to this machine, so there is nothing to draw.', { dimColor: true }),
      t(`Point a node at this host's UDP ${opts.udpPort ?? 5005} (provision.py --target-ip), then this view fills by itself.`, { dimColor: true }),
    ], t, row);
  }
  const { grid, lo, hi, node: n, nodes } = pics.waterfall;
  const legend = colourbar(Math.min(32, grid.columns - 20));
  return card(ui, 'CSI WATERFALL', `${n.label} · ${n.subcarriers} subcarriers → ${n.bins} bins · ${n.rateHz ?? '—'} Hz${nodes.length > 1 ? ` · node ${nodes.indexOf(n) + 1}/${nodes.length}` : ''}`, n.synthetic ? 'yellow' : 'cyan', [
    n.synthetic ? t('SYNTHETIC — simulator frames, not a live measurement', { color: 'yellow', bold: true }) : t('MEASURED — live CSI amplitude received on this host', { color: 'green' }),
    ui.Raster(grid.toRaster('waterfall')),
    row([t('subcarrier →', { dimColor: true }), t('newest at the bottom · replayed at the arrival rate', { dimColor: true })], { justifyContent: 'space-between' }),
    row([t(lo.toFixed(0), { dimColor: true }), ui.Raster(legend.toRaster('legend')), t(`${hi.toFixed(0)} amplitude (5th–95th pct)`, { dimColor: true })]),
  ], t, row);
}

function radarView(ui, model, opts, pics, t, row) {
  const note = needsTerminal(ui, t);
  if (note) return note;
  if (!model) return t('waiting for the first capture…', { dimColor: true });
  if (!opts.radarConfigured) {
    return card(ui, 'RADAR', 'not configured', 'gray', [t('Set radarHost: claude plugin configure ruview-live', { color: 'cyan' })], t, row);
  }
  const r = model.radar;
  if (!r || !pics.fan) return card(ui, '60 GHz RADAR', 'unreachable', 'red', [t('No reading this refresh; see the alert above.', { dimColor: true })], t, row);
  const history = opts.history || { heart: [], breathing: [], distance: [] };
  const { wide, chartW } = radarLayout(opts.columns, opts.rows);
  const presence = r.present ? t(`● PRESENCE DETECTED · ${r.distance}${r.targets ? ` · ${r.targets} target${r.targets === 1 ? '' : 's'}` : ''}`, { color: 'green', bold: true })
    : t('○ no presence', { dimColor: true });
  const fan = card(ui, '60 GHz RADAR', `${r.name} · range ${pics.fan.maxCm / 100} m`, r.present ? 'green' : 'gray', [
    presence,
    ui.Raster(pics.fan.grid.toRaster('fan')),
    t('range only: this kit reports distance, not bearing (arc = every bearing at that range)', { dimColor: true, italic: true }),
  ], t, row);
  const chart = (label, series, kind, unit, colour, minSpan) => {
    const { grid: g, lo, hi } = lineChart(series, chartW, 3, { band: kind ? RANGES[kind] : null, colour, minSpan });
    const last = [...series].reverse().find((v) => Number.isFinite(v));
    const warn = kind ? plausibilityOf(kind, last ?? null) : null;
    return [
      row([t(label, { bold: true }), t(last == null ? '—' : `${last.toFixed(1)} ${unit}`, { color: warn ? 'yellow' : undefined }), warn ? t(`⚠ ${warn}`, { color: 'yellow' }) : null, t(`${lo.toFixed(0)}–${hi.toFixed(0)}`, { dimColor: true })]),
      ui.Raster(g.toRaster(`chart-${label}`)),
    ];
  };
  const charts = card(ui, 'VITALS', 'device-reported, not validated', 'gray', [
    pics.pulse ? ui.Raster(pics.pulse.grid.toRaster('pulse')) : null,
    pics.pulse ? t('♥ beats and the gauge breathes at the reported rates (a metronome, not a waveform)', { dimColor: true, italic: true }) : null,
    ...chart('heart', history.heart, 'heart', 'bpm', rgb(255, 110, 130), 10),
    ...chart('breathing', history.breathing, 'breathing', 'bpm', rgb(110, 200, 255), 4),
    ...chart('distance', history.distance, null, 'cm', rgb(120, 255, 170), 20),
    t('shaded = inside the plausible resting range', { dimColor: true, italic: true }),
  ], t, row);
  return ui.Box({ flexDirection: wide ? 'row' : 'column', gap: 1, children: [fan, charts] });
}
