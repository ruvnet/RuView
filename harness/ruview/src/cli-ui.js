// SPDX-License-Identifier: MIT
// Terminal rendering for the CLI (ADR-375). Used only when stdout is a TTY and
// `--json` is absent; pipes and scripts keep receiving exact JSON. Respects
// NO_COLOR and TERM=dumb. Pure functions of (tool, result) so tests can pin
// the output byte-for-byte.

const ESC = '\x1b[';
const ANSI_RE = /\x1b\[[0-9;]*m/g;

export function colorEnabled(stream = process.stdout, env = process.env) {
  return Boolean(stream && stream.isTTY) && !('NO_COLOR' in env) && env.TERM !== 'dumb';
}

export function createStyle(on) {
  const wrap = (code) => (s) => (on ? `${ESC}${code}m${s}${ESC}0m` : String(s));
  return { acid: wrap('38;5;155'), muted: wrap('38;5;245'), warn: wrap('38;5;221'), bad: wrap('38;5;203'), bold: wrap('1') };
}

const visible = (s) => String(s).replace(ANSI_RE, '').length;
const pad = (s, n) => String(s) + ' '.repeat(Math.max(0, n - visible(s)));
const num = (v, d = 0) => (typeof v === 'number' && Number.isFinite(v) ? v.toFixed(d) : '—');
const pct = (v) => (typeof v === 'number' ? `${(v * 100).toFixed(v > 0 && v < 0.01 ? 2 : 1)}%` : '—');
const BARS = '▁▂▃▄▅▆▇█';

/** Unicode bar of `width` cells for a 0..1 fraction. */
export function bar(frac, width = 10) {
  const f = Math.max(0, Math.min(1, Number(frac) || 0));
  const full = Math.floor(f * width);
  return '█'.repeat(full) + (full < width ? (f * width - full >= 0.5 ? '▌' : ' ') : '') + ' '.repeat(Math.max(0, width - full - 1));
}

/** Sparkline of capture windows; missing samples retain their time position. */
export function sparkline(values) {
  const v = values.filter((x) => typeof x === 'number' && Number.isFinite(x));
  if (!v.length) return '·'.repeat(values.length);
  const lo = Math.min(...v); const hi = Math.max(...v);
  return values.map((x) => typeof x === 'number' && Number.isFinite(x)
    ? BARS[hi === lo ? 3 : Math.round(((x - lo) / (hi - lo)) * 7)] : '·').join('');
}

/** Align rows (arrays of cells) into columns. */
export function table(rows, style, gap = 2) {
  const widths = [];
  for (const row of rows) row.forEach((c, i) => { widths[i] = Math.max(widths[i] || 0, visible(c)); });
  return rows.map((row, r) => {
    const line = row.map((c, i) => (i === row.length - 1 ? String(c) : pad(c, widths[i] + gap))).join('').trimEnd();
    return r === 0 ? style.muted(line) : line;
  }).join('\n');
}

function head(view, r, s) {
  const measured = r.evidence && /^MEASURED/.test(r.evidence);
  const status = r.ok === false
    ? s.bad(`✖ ${String(r.reason || 'failed').replace(/_/g, ' ')}`)
    : measured ? s.acid('● MEASURED') : s.acid('✔ ok');
  return `${s.muted(`RUVIEW / ${view}`)}   ${status}`;
}

function failureLines(r, s) {
  if (r.ok !== false) return [];
  const out = [];
  for (const k of ['detail', 'remedy', 'hint']) if (r[k]) out.push(`${s.muted(k === 'remedy' ? 'fix' : k)}  ${r[k]}`);
  return out;
}

function renderCapture(r, s, history) {
  const lines = [head('NODE STREAM', r, s), ...failureLines(r, s)];
  lines.push(s.muted(`packets ${r.packets ?? '—'} · decoded ${r.decodedPackets ?? '—'} · csi nodes ${r.csiNodes ?? '—'} · ${r.seconds ?? '—'} s · ${r.listen ?? ''}`));
  for (const hb of r.heartbeatOnlySenders || []) lines.push(s.warn(`! heartbeat-only: ${hb.address} (${hb.heartbeats} heartbeats, no CSI)`));
  const nodes = r.nodes || [];
  if (nodes.length) {
    const maxHz = Math.max(...nodes.map((n) => n.csiRateHz || 0), 1);
    const rows = [['SOURCE', 'NODE', 'CSI RATE', '', 'LOSS', 'RSSI', 'SHAPE', 'SEQ', history ? 'TREND' : '']];
    for (const n of nodes) {
      const key = `${n.source}:${n.nodeId}`;
      const csi = n.csi || {};
      const seq = [n.seqReordered && `${n.seqReordered}r`, n.seqStrays && `${n.seqStrays}s`, n.seqResyncs && `${n.seqResyncs}R`].filter(Boolean).join(' ') || '—';
      const loss = n.csiLossFraction;
      rows.push([
        n.source === 'realtek' ? 'realtek' : 'esp32', String(n.nodeId),
        `${num(n.csiRateHz, 1)} Hz`, s.acid(bar((n.csiRateHz || 0) / maxHz, 8)),
        loss == null ? '—' : loss > 0.05 ? s.warn(pct(loss)) : pct(loss),
        `${num(n.rssiMean, 1)} dBm`,
        `${csi.shape || '—'}${n.csiShapes ? ` +${n.csiShapes.length - 1}` : ''}${csi.synthetic ? s.warn(' SYNTHETIC') : ''}`,
        seq, history ? s.acid(sparkline(history.get(key) || [])) : '',
      ]);
    }
    lines.push('', table(rows, s));
  }
  const a = r.analysis;
  if (a) {
    lines.push('');
    if (a.ok === false) lines.push(s.warn(`kernel analysis: ${a.reason}${a.detail ? ` — ${a.detail}` : ''}`));
    else {
      const last = a.summary?.last || {}; const hr = last.heart || {}; const rr = last.respiratory || {};
      lines.push(`${s.bold('kernel analysis')} ${s.muted(`${a.backend} · ${a.integrity} · ${a.input?.frames} frames @ ${num(a.input?.sampleRateHz, 1)} Hz`)}`);
      lines.push(`  heart ${num(hr.bpm, 1)} bpm (${hr.status || '—'})   breathing ${rr.status === 'unavailable' ? 'unavailable' : `${num(rr.bpm, 1)} bpm (${rr.status || '—'})`}`);
      lines.push(s.muted(`  ${a.note || 'estimates without a reference measurement'}`));
    }
  }
  if (r.unknownPackets) lines.push(s.muted(`unknown packets ${r.unknownPackets}: ${Object.keys(r.unknownMagics || {}).join(', ')}`));
  return lines.join('\n');
}

function renderDevices(r, s) {
  const lines = [head('DEVICES', r, s), ...failureLines(r, s)];
  const rows = [['PORT', 'BRIDGE', 'LIKELY', 'CONFIRM WITH']];
  for (const d of (r.devices || []).filter((x) => !x.builtin)) {
    rows.push([d.port, d.bridge || d.usb || 'unknown', (d.likelyRoles || []).join('/') || '—', (d.confirmWith || [])[0] || '—']);
  }
  if (rows.length > 1) lines.push('', table(rows, s));
  if (r.note) lines.push('', s.muted(r.note));
  return lines.join('\n');
}

function renderMonitor(r, s) {
  const lines = [head('SERIAL MONITOR', r, s), ...failureLines(r, s)];
  if (r.ok !== false || r.lines !== undefined) lines.push(`csi log lines ${r.csi_callbacks ?? 0} · console lines ${r.lines ?? '—'} · ${r.baud ?? '—'} baud · reset on open: ${r.reset_on_open === false ? 'no' : 'unknown'}`);
  return lines.join('\n');
}

function renderFlash(r, s) {
  const lines = [head(r.dryRun ? 'FLASH PLAN' : 'FLASH', r, s), ...failureLines(r, s)];
  const p = r.plan;
  if (p) {
    lines.push(s.muted(`${p.chip} · ${p.flashSize} · ${p.port} · checksums ${p.checksums || 'none'}`));
    lines.push('', table([['OFFSET', 'ROLE', 'BYTES', 'INTEGRITY'], ...(p.images || []).map((i) => [i.offset, i.role, String(i.bytes), i.integrity === 'verified' ? s.acid(i.integrity) : s.warn(i.integrity)])], s));
  }
  if (r.bootLog) lines.push('', `boot log: ${r.bootLog.lines ?? 0} lines · CSI callbacks ${r.bootLog.csiCallbacks ?? 0} · panic ${r.bootLog.panic ? s.bad('yes') : 'no'}`);
  if (r.evidence) lines.push(s.acid(r.evidence));
  return lines.join('\n');
}

function renderMmwave(r, s) {
  const lines = [head('RADAR', r, s), ...failureLines(r, s)];
  if (r.ok === false) {
    for (const a of r.attempts || []) lines.push(s.muted(`tried ${a.model}: ${a.bytes} bytes, ${a.frames} frames`));
    return lines.join('\n');
  }
  const d = r.device || {};
  lines.push(s.muted(r.source === 'esphome'
    ? `ESPHome ${d.esphomeVersion || '?'} · ${d.name || r.host} · ${d.project || ''}${d.projectVersion ? ` ${d.projectVersion}` : ''} · ${r.stateUpdates} updates in ${r.seconds} s`
    : `${r.model} (${r.band}) · ${r.frames} frames · ${r.checksumErrors} checksum errors · ${r.frameRateHz} Hz`));
  const present = r.presentNow ?? (r.presentFraction == null ? null : r.presentFraction >= 0.5);
  lines.push('', `${s.bold('presence')}  ${present == null ? '—' : present ? s.acid('● detected') : s.muted('○ none')}${r.presentFraction == null ? '' : s.muted(`  (${(r.presentFraction * 100).toFixed(0)}% of reports)`)}${r.targetsMax != null ? s.muted(`  targets ${r.targetsMax}`) : ''}`);
  lines.push(`${s.bold('distance')}  ${r.distanceCmMean == null ? '—' : `${num(r.distanceCmMean, 1)} cm`}`);
  lines.push(`${s.bold('heart')}     ${r.heartBpmMean == null ? '—' : `${num(r.heartBpmMean, 1)} bpm`}   ${s.bold('breathing')}  ${r.breathingBpmMean == null ? '—' : `${num(r.breathingBpmMean, 1)} bpm`}`);
  if (r.entities) {
    lines.push('', table([['ENTITY', 'LAST', 'UPDATES'], ...r.entities.map((e) => [e.name, e.last == null ? '—' : `${typeof e.last === 'number' ? num(e.last, 2) : e.last}${e.unit ? ` ${e.unit}` : ''}`, String(e.updates)])], s));
  }
  lines.push('', s.muted('device-reported values (computed by the radar firmware), not validated against a reference'));
  return lines.join('\n');
}

function renderGeneric(tool, r, s) {
  const view = String(tool || 'result').replace(/^ruview_/, '').replace(/_/g, ' ').toUpperCase();
  const lines = [head(view, r, s), ...failureLines(r, s)];
  const scalars = Object.entries(r).filter(([k, v]) => !['ok', 'reason', 'detail', 'remedy', 'hint'].includes(k) && (v === null || ['string', 'number', 'boolean'].includes(typeof v)));
  for (const [k, v] of scalars.slice(0, 16)) lines.push(`${s.muted(pad(k, 18))}${String(v).length > 140 ? `${String(v).slice(0, 137)}…` : v}`);
  const nested = Object.keys(r).filter((k) => r[k] && typeof r[k] === 'object');
  if (nested.length) lines.push(s.muted(`… ${nested.join(', ')} (use --json for the full result)`));
  return lines.join('\n');
}

/**
 * Render a tool result for a terminal. `history` (Map of node key → rates)
 * adds trend sparklines in watch mode.
 */
export function renderResult(tool, r, { color = false, history } = {}) {
  const s = createStyle(color);
  if (!r || typeof r !== 'object') return String(r);
  if (tool === 'ruview_esp32_capture' || r.nodes) return renderCapture(r, s, history);
  if (tool === 'ruview_devices_scan') return renderDevices(r, s);
  if (tool === 'ruview_node_monitor') return renderMonitor(r, s);
  if (tool === 'ruview_mmwave_read') return renderMmwave(r, s);
  if (tool === 'ruview_node_flash' || tool === 'ruview_firmware_plan') return renderFlash(r, s);
  return renderGeneric(tool, r, s);
}

/**
 * Show a countdown on stderr while `fn` runs (TTY only). Returns fn's result.
 */
export async function withProgress(label, seconds, fn, stream = process.stderr) {
  if (!stream.isTTY || !seconds) return fn();
  const frames = '⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏';
  const start = Date.now();
  let i = 0;
  const draw = () => {
    const left = Math.max(0, seconds - (Date.now() - start) / 1000);
    stream.write(`\r${frames[i++ % frames.length]} ${label} · ${left.toFixed(0)}s left  `);
  };
  draw();
  const timer = setInterval(draw, 120);
  try { return await fn(); } finally { clearInterval(timer); stream.write('\r\x1b[2K'); }
}
