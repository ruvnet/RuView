// SPDX-License-Identifier: MIT
// LiDAR access (ADR-373):
//   * rplidar  — Slamtec RPLIDAR A1/A2/A3/C1/S-series legacy SCAN protocol over
//                USB serial. Starting a scan spins the motor/laser; stopping
//                is sent on exit. No persistent configuration is written.
//   * iphone   — the RuView iPhone LiDAR relay (integrations/iphone-lidar),
//                read as a WebSocket client. Only depth statistics are
//                returned; raw depth never leaves this process.

import { readSerial } from './serial-pump.js';

const SCAN = 'a520';
const STOP = 'a525';
const DESCRIPTOR = [0xa5, 0x5a, 0x05, 0x00, 0x00, 0x40, 0x81];

/** Parse an RPLIDAR SCAN byte stream into measurement nodes. */
export function parseRplidar(bytes) {
  let i = 0;
  // Skip to the scan response descriptor if present; otherwise assume raw nodes.
  for (let j = 0; j + DESCRIPTOR.length <= bytes.length; j++) {
    if (DESCRIPTOR.every((v, k) => bytes[j + k] === v)) { i = j + DESCRIPTOR.length; break; }
  }
  const points = [];
  let invalid = 0;
  let revolutions = 0;
  while (i + 5 <= bytes.length) {
    const b0 = bytes[i];
    const b1 = bytes[i + 1];
    const start = b0 & 1;
    const inv = (b0 >> 1) & 1;
    if (start === inv || (b1 & 1) !== 1) { invalid += 1; i += 1; continue; } // resync
    const angle = ((b1 >> 1) | (bytes[i + 2] << 7)) / 64;
    const distanceMm = (bytes[i + 3] | (bytes[i + 4] << 8)) / 4;
    if (start) revolutions += 1;
    points.push({ angle, distanceMm, quality: b0 >> 2, start: Boolean(start) });
    i += 5;
  }
  return { points, invalid, revolutions };
}

const median = (xs) => {
  if (!xs.length) return null;
  const s = [...xs].sort((a, b) => a - b);
  const middle = Math.floor(s.length / 2);
  return s.length % 2 ? s[middle] : (s[middle - 1] + s[middle]) / 2;
};

export function summarizeScan({ points, invalid, revolutions }, seconds) {
  const valid = points.filter((p) => p.distanceMm > 0);
  const d = valid.map((p) => p.distanceMm);
  const sectors = new Set(valid.map((p) => Math.floor(p.angle / 10) % 36));
  let min = Infinity;
  let max = -Infinity;
  for (const value of d) { min = Math.min(min, value); max = Math.max(max, value); }
  return {
    points: points.length, validPoints: valid.length, resyncBytes: invalid, revolutions,
    scanRateHz: Number((revolutions / seconds).toFixed(2)),
    rangeMm: d.length ? { min, median: median(d), max } : null,
    angularCoverage: Number((sectors.size / 36).toFixed(2)),
  };
}

export async function readRplidar(args, deps) {
  const seconds = args.seconds ?? 5;
  let cap;
  try {
    cap = await readSerial({ port: args.port, baud: args.baud ?? 115200, seconds, startHex: SCAN, stopHex: STOP, dtr: 'low', maxBytes: 4 * 1024 * 1024 }, deps);
  } catch (error) {
    return { ok: false, reason: error.reason || 'invalid_arguments', detail: error.message };
  }
  if (!cap.ok) return { ok: false, reason: cap.reason, detail: cap.detail, remedy: cap.remedy };
  const summary = summarizeScan(parseRplidar(cap.bytes), cap.seconds);
  if (!summary.validPoints) {
    return { ok: false, reason: 'no_scan_points', bytes: cap.bytes.length, ...summary, remedy: 'A2/A3/S-series need 115200/256000/1000000 baud per model; check the USB adapter powers the motor (A1 motor follows DTR).' };
  }
  return { ok: true, source: 'rplidar', bytes: cap.bytes.length, ...summary, ...(cap.captureIncomplete ? { captureIncomplete: true } : {}), ...(cap.warnings ? { warnings: cap.warnings } : {}), evidence: 'MEASURED: live scan read on this host' };
}

/** Validate and reduce one ruview.lidar.depth.v1 packet to statistics. */
export function depthStats(packet) {
  if (!packet || packet.type !== 'ruview.lidar.depth.v1') throw new Error('unsupported packet type');
  const depth = packet.depth || {};
  if (depth.encoding !== 'u16le-mm+u8-confidence') throw new Error('unsupported depth encoding');
  const { width, height } = depth;
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width * height > 1_000_000) throw new Error('invalid depth dimensions');
  const mm = Buffer.from(String(depth.millimetersBase64 || ''), 'base64');
  const conf = Buffer.from(String(depth.confidenceBase64 || ''), 'base64');
  if (mm.length !== width * height * 2 || conf.length !== width * height) throw new Error('depth payload length mismatch');
  const values = [];
  let confident = 0;
  for (let i = 0; i < width * height; i++) {
    const v = mm.readUInt16LE(i * 2);
    if (v > 0 && conf[i] >= 1) { values.push(v); confident += 1; }
  }
  return { width, height, confidentFraction: Number((confident / (width * height)).toFixed(3)), medianDepthM: values.length ? median(values) / 1000 : null };
}

/**
 * Connect to the iPhone LiDAR relay as a WebSocket client.
 * args: { url: ws(s)://host:port/ws/lidar, seconds, max_frames }. Returns early
 * once max_frames arrive or the connection is refused. The access token comes
 * only from RUVIEW_LIDAR_TOKEN (never from tool arguments or logs).
 */
export function readIphoneLidar(args, deps = {}) {
  const seconds = Math.min(Math.max(args.seconds ?? 5, 1), 60);
  const maxFrames = Math.min(Math.max(args.max_frames ?? 10_000, 1), 10_000);
  let url;
  try {
    url = new URL(args.url);
    if (!['ws:', 'wss:'].includes(url.protocol)) throw new Error('url must be ws:// or wss://');
    if (url.searchParams.has('token')) throw new Error('put the relay token in RUVIEW_LIDAR_TOKEN, not in the URL');
  } catch (error) {
    return Promise.resolve({ ok: false, reason: 'invalid_url', detail: error.message });
  }
  const token = (deps.env || process.env).RUVIEW_LIDAR_TOKEN;
  if (token) url.searchParams.set('token', token);
  const WS = deps.WebSocket || globalThis.WebSocket;
  if (typeof WS !== 'function') return Promise.resolve({ ok: false, reason: 'websocket_unavailable', remedy: 'Use Node 22+ (built-in WebSocket client).' });
  const shown = `${url.protocol}//${url.host}${url.pathname}`;
  return new Promise((resolve) => {
    const frames = [];
    let rejected = 0;
    let lastError = null;
    let ws;
    let done = false;
    let opened = false;
    const finish = (extra = {}) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      try { ws?.close(); } catch { /* ignore */ }
      const n = frames.length;
      resolve({
        ok: n > 0 && !extra.reason,
        source: 'iphone', relay: shown, seconds, frames: n, rejectedFrames: rejected,
        fps: Number((n / Math.max((Date.now() - started) / 1000, 0.001)).toFixed(2)),
        ...(n ? { last: frames.at(-1), confidentFractionMean: Number((frames.reduce((a, f) => a + f.confidentFraction, 0) / n).toFixed(3)) } : {}),
        ...(n === 0 && !extra.reason ? (opened
          ? { reason: 'no_frames', detail: lastError, remedy: 'Connected, but the iPhone is not streaming: start capture in the RuView LiDAR app pointed at this relay.' }
          : { reason: 'connect_failed', detail: lastError, remedy: 'Check the relay is running at this URL and RUVIEW_LIDAR_TOKEN matches the token it printed.' }) : {}),
        evidence: n ? 'MEASURED: live depth frames received on this host' : null,
        ...extra,
      });
    };
    const started = Date.now();
    const timer = setTimeout(() => finish(), seconds * 1000);
    try {
      ws = new WS(url.href);
    } catch (error) {
      finish({ ok: false, reason: 'connect_failed', detail: error.message });
      return;
    }
    ws.onopen = () => { opened = true; };
    ws.onmessage = (event) => {
      try {
        const text = typeof event.data === 'string' ? event.data : Buffer.from(event.data).toString('utf8');
        if (text.length > 4_000_000) throw new Error('frame too large');
        frames.push(depthStats(JSON.parse(text)));
        if (frames.length >= maxFrames) finish();
      } catch (error) { rejected += 1; lastError = error.message; }
    };
    ws.onerror = () => {
      lastError = 'websocket error';
      if (!opened) finish(); // refused before opening: nothing will arrive
    };
    ws.onclose = (event) => {
      if (!opened) finish();
      else if (!frames.length && event?.code && event.code !== 1000) finish({ ok: false, reason: 'relay_closed', detail: `close code ${event.code}${event.code === 1008 ? ' (token rejected?)' : ''}` });
      else finish(); // The closed stream cannot contribute another frame.
    };
  });
}
