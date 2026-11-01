// SPDX-License-Identifier: MIT
// ruview-live data: settings, CLI argv, result parsing, the pane model, trend
// history and the status line. Pure functions, tested under plain Node.

const HOST_RE = /^[A-Za-z0-9][A-Za-z0-9.-]{0,252}$/;
export const HISTORY = 48;
export const MODES = Object.freeze(['overview', 'waterfall', 'radar']);

/**
 * The harness CLI beside this mod: the plugin root is <pkg>/mod, so the CLI is
 * <pkg>/bin/cli.js. Mods import nothing but their own files, so the engine's
 * `$.plugin.root` locates it.
 */
export function cliPathOf(pluginRoot) {
  return `${String(pluginRoot).replace(/[\\/]+$/, '')}/../bin/cli.js`;
}

/** Normalise plugin options (userConfig) into bounded settings. */
export function settingsOf(options = {}) {
  const num = (v, d, lo, hi) => {
    const n = Number(v);
    return Number.isFinite(n) ? Math.min(Math.max(Math.round(n), lo), hi) : d;
  };
  const host = typeof options.radarHost === 'string' ? options.radarHost.trim() : '';
  return {
    udpPort: num(options.udpPort, 5005, 1024, 65535),
    radarHost: HOST_RE.test(host) ? host : '',
    refreshMs: num(options.refreshSeconds, 15, 5, 3600) * 1000,
    captureSeconds: num(options.captureSeconds, 3, 1, 10),
    liveRefreshMs: num(options.liveRefreshSeconds, 4, 2, 60) * 1000,
  };
}

/**
 * argv for one capture and (optionally) one radar read. In a live view the
 * capture is shorter, and the waterfall asks for binned amplitude frames.
 */
export function commandsOf(settings, { live = false, spectrum = null } = {}) {
  const s = String(live ? Math.min(settings.captureSeconds, 2) : settings.captureSeconds);
  const capture = ['esp32', '--seconds', s, '--udp-port', String(settings.udpPort), '--json'];
  if (spectrum) capture.push('--spectrum', '--spectrum-bins', String(spectrum.bins), '--spectrum-frames', String(spectrum.frames));
  return {
    capture,
    radar: settings.radarHost ? ['mmwave', '--source', 'esphome', '--host', settings.radarHost, '--seconds', s, '--json'] : null,
  };
}

/** Parse a CLI run into its JSON result, or an honest failure. */
export function resultOf(run) {
  if (!run) return null;
  try {
    const parsed = JSON.parse(run.stdout);
    if (parsed && typeof parsed === 'object') return parsed;
  } catch { /* fall through */ }
  return { ok: false, reason: 'cli_error', detail: String(run.stderr || run.stdout || `exit ${run.exitCode}`).trim().slice(0, 300) };
}

const fixed = (v, d = 1) => (typeof v === 'number' && Number.isFinite(v) ? v.toFixed(d) : '—');
const pct = (v) => (typeof v === 'number' ? `${(v * 100).toFixed(1)}%` : '—');
const num = (v) => (typeof v === 'number' && Number.isFinite(v) ? v : null);

/** Resting ranges outside which a device-reported vital is flagged, not trusted. */
export const RANGES = Object.freeze({ heart: [40, 180], breathing: [4, 40] });

/** Why a device-reported vital looks implausible, or null. */
export function plausibilityOf(kind, value) {
  if (value == null || !RANGES[kind]) return null;
  const [lo, hi] = RANGES[kind];
  return value < lo || value > hi ? `outside ${lo}–${hi} bpm` : null;
}

const SPARK = '▁▂▃▄▅▆▇█';
/** Unicode sparkline of the last `width` finite values ('' below two points). */
export function sparkline(values, width = 16) {
  const v = (values || []).filter((x) => typeof x === 'number' && Number.isFinite(x)).slice(-width);
  if (v.length < 2) return '';
  const lo = Math.min(...v);
  const hi = Math.max(...v);
  return v.map((x) => SPARK[hi === lo ? 3 : Math.round(((x - lo) / (hi - lo)) * 7)]).join('');
}

/** "12s ago" / "3m ago" from a millisecond age, or '' when unknown. */
export function ageOf(ms) {
  if (!Number.isFinite(ms) || ms < 0) return '';
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s}s ago` : `${Math.round(s / 60)}m ago`;
}

/** The pane's model: plain data, drawn by the views and summarised by statusOf. */
export function modelOf(capture, radar, at) {
  const alerts = [];
  // "No packets" is shown inside the nodes card; only real faults become alerts.
  if (capture && capture.ok === false && capture.reason !== 'no_packets') {
    alerts.push({ level: 'bad', text: `nodes: ${String(capture.reason).replace(/_/g, ' ')}${capture.remedy ? ` — ${capture.remedy}` : ''}` });
  }
  for (const hb of capture?.heartbeatOnlySenders || []) alerts.push({ level: 'warn', text: `${hb.address} sends heartbeats but no CSI (run \`ruview monitor --baud 1500000\` on it: "lack of csi buf" = buffer starvation)` });
  if (radar && radar.ok === false) alerts.push({ level: 'bad', text: `radar: ${String(radar.reason).replace(/_/g, ' ')}${radar.detail ? ` — ${radar.detail}` : ''}` });
  const nodes = (capture?.nodes || []).map((n) => ({
    key: `${n.source}:${n.nodeId}`,
    label: `${n.source === 'realtek' ? 'realtek' : 'esp32'} ${n.nodeId}`,
    rateHz: num(n.csiRateHz),
    rate: `${fixed(n.csiRateHz)} Hz`,
    loss: pct(n.csiLossFraction),
    rssi: `${fixed(n.rssiMean)} dBm`,
    shape: n.csi?.shape || '—',
    lossy: typeof n.csiLossFraction === 'number' && n.csiLossFraction > 0.05,
    synthetic: Boolean(n.csi?.synthetic),
  }));
  const spectrum = (capture?.spectrum || []).map((s) => ({
    key: `${s.source}:${s.nodeId}`,
    label: `${s.source === 'realtek' ? 'realtek' : 'esp32'} ${s.nodeId}`,
    shape: s.shape ?? null, subcarriers: s.subcarriers, bins: s.bins, frames: Array.isArray(s.frames) ? s.frames : [],
    rateHz: num(s.rateHz), synthetic: Boolean(s.synthetic),
  }));
  const radarRow = radar && radar.ok !== false ? {
    name: radar.device?.name || radar.host || 'radar',
    present: radar.presentNow ?? (radar.presentFraction == null ? null : radar.presentFraction >= 0.5),
    targets: num(radar.targetsMax),
    distanceCm: num(radar.distanceCmMean),
    heartBpm: num(radar.heartBpmMean),
    breathingBpm: num(radar.breathingBpmMean),
    distance: radar.distanceCmMean == null ? '—' : `${fixed(radar.distanceCmMean)} cm`,
    heart: radar.heartBpmMean == null ? '—' : `${fixed(radar.heartBpmMean)} bpm`,
    breathing: radar.breathingBpmMean == null ? '—' : `${fixed(radar.breathingBpmMean)} bpm`,
  } : null;
  return {
    at, packets: capture?.packets ?? 0, decoded: capture?.decodedPackets ?? 0,
    noPackets: capture?.reason === 'no_packets', nodes, spectrum, radar: radarRow, alerts,
  };
}

/** Append one model to a bounded history of trend values. */
export function historyWith(history, model, max = HISTORY) {
  const h = history || { heart: [], breathing: [], distance: [], rates: {} };
  const cap = (arr, v) => [...arr, v].slice(-max);
  const rates = { ...h.rates };
  for (const n of model?.nodes || []) rates[n.key] = cap(rates[n.key] || [], n.rateHz);
  return {
    heart: cap(h.heart, model?.radar?.heartBpm ?? null),
    breathing: cap(h.breathing, model?.radar?.breathingBpm ?? null),
    distance: cap(h.distance, model?.radar?.distanceCm ?? null),
    rates,
  };
}

/**
 * Carry each node's waterfall frames across captures, so the picture scrolls
 * instead of restarting every refresh. Only nodes in this capture remain: a
 * node that stopped streaming (or a failed capture) leaves the waterfall, so
 * old frames are never shown as live. A node whose CSI shape or bin count
 * changed starts over rather than joining two subcarrier layouts.
 */
export function spectrumWith(previous, spectrum, max = 128) {
  const byKey = new Map((previous || []).map((s) => [s.key, s]));
  return (spectrum || []).map((s) => {
    const old = byKey.get(s.key);
    const same = old && old.bins === s.bins && old.shape === s.shape;
    return { ...s, frames: (same ? [...old.frames, ...s.frames] : [...s.frames]).slice(-max) };
  });
}

/** One line for the status bar. */
export function statusOf(model) {
  if (!model) return 'RuView · starting';
  const parts = [`RuView · ${model.nodes.length} node${model.nodes.length === 1 ? '' : 's'}`];
  if (model.radar) parts.push(`radar ${model.radar.present == null ? '?' : model.radar.present ? 'present' : 'clear'}`);
  if (model.alerts.length) parts.push(`${model.alerts.length} alert${model.alerts.length === 1 ? '' : 's'}`);
  return parts.join(' · ');
}
