// SPDX-License-Identifier: MIT
// Cell graphics for the ruview-live showcase (ADR-378): every picture is a
// terminal `Raster`, a grid of width-1 characters with 24-bit colours, so it
// draws in any terminal (Windows Terminal included), not only pixel-capable
// ones. Pure functions: no engine calls, so Node tests can check them.

/** The terminal's own colour (bit 24 alone), per the Raster contract. */
export const DEFAULT = 0x01000000;

/** 0xRRGGBB from components. */
export const rgb = (r, g, b) => (((r & 255) << 16) | ((g & 255) << 8) | (b & 255)) >>> 0;

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/** Standard padded base64 of a byte array (the mod environment has no Node Buffer). */
export function toBase64(bytes) {
  let out = '';
  let i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    out += B64[(n >> 18) & 63] + B64[(n >> 12) & 63] + B64[(n >> 6) & 63] + B64[n & 63];
  }
  const rest = bytes.length - i;
  if (rest === 1) {
    const n = bytes[i] << 16;
    out += `${B64[(n >> 18) & 63]}${B64[(n >> 12) & 63]}==`;
  } else if (rest === 2) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8);
    out += `${B64[(n >> 18) & 63]}${B64[(n >> 12) & 63]}${B64[(n >> 6) & 63]}=`;
  }
  return out;
}

/** A grid of cells: code point, foreground, background. */
export class Grid {
  constructor(columns, rows) {
    this.columns = Math.max(1, Math.min(512, Math.floor(columns)));
    this.rows = Math.max(1, Math.min(256, Math.floor(rows)));
    this.cells = new Uint32Array(this.columns * this.rows * 3);
    for (let i = 0; i < this.columns * this.rows; i++) {
      this.cells[i * 3] = 0x20;
      this.cells[i * 3 + 1] = DEFAULT;
      this.cells[i * 3 + 2] = DEFAULT;
    }
  }
  set(x, y, ch, fg = DEFAULT, bg = DEFAULT) {
    if (x < 0 || y < 0 || x >= this.columns || y >= this.rows) return;
    const i = (Math.floor(y) * this.columns + Math.floor(x)) * 3;
    this.cells[i] = typeof ch === 'number' ? ch : ch.codePointAt(0);
    this.cells[i + 1] = fg >>> 0;
    this.cells[i + 2] = bg >>> 0;
  }
  get(x, y) {
    const i = (y * this.columns + x) * 3;
    return [this.cells[i], this.cells[i + 1], this.cells[i + 2]];
  }
  /** Write a string left to right (labels over a picture). */
  text(x, y, str, fg = DEFAULT, bg = DEFAULT) {
    [...str].forEach((ch, k) => this.set(x + k, y, ch, fg, bg));
  }
  /** Props for the surface's `Raster` element. */
  toRaster(key) {
    const bytes = new Uint8Array(this.cells.buffer, this.cells.byteOffset, this.cells.byteLength);
    return { key, columns: this.columns, rows: this.rows, cells: toBase64(bytes) };
  }
}

// Perceptual-ish ramp on near-black: indigo → cyan → emerald → amber → white.
const STOPS = [
  [0.0, [10, 12, 22]], [0.22, [36, 38, 120]], [0.45, [0, 150, 200]],
  [0.65, [30, 205, 140]], [0.85, [235, 205, 70]], [1.0, [255, 250, 235]],
];

/** Colour for t in [0, 1]. */
export function colormap(t) {
  const x = Math.max(0, Math.min(1, Number.isFinite(t) ? t : 0));
  for (let k = 1; k < STOPS.length; k++) {
    const [t1, c1] = STOPS[k];
    if (x <= t1) {
      const [t0, c0] = STOPS[k - 1];
      const f = (x - t0) / (t1 - t0 || 1);
      return rgb(c0[0] + (c1[0] - c0[0]) * f, c0[1] + (c1[1] - c0[1]) * f, c0[2] + (c1[2] - c0[2]) * f);
    }
  }
  return rgb(255, 250, 235);
}

/** The values at fractions p of a sample (robust colour limits). */
export function percentiles(values, ps) {
  const v = values.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  if (!v.length) return ps.map(() => 0);
  return ps.map((p) => v[Math.min(v.length - 1, Math.max(0, Math.round(p * (v.length - 1))))]);
}

/**
 * Waterfall of amplitude frames (newest last): subcarrier bins across, time
 * down, two frames per text row through the upper-half block (fg = upper
 * frame, bg = lower frame). Colour limits are the 5th–95th percentile of the
 * shown frames. Returns { grid, lo, hi, shown }.
 */
export function waterfall(frames, columns, rows) {
  const g = new Grid(columns, rows);
  const shown = (frames || []).slice(-(g.rows * 2));
  const [lo, hi] = percentiles(shown.flat(), [0.05, 0.95]);
  const span = hi - lo || 1;
  const offset = g.rows * 2 - shown.length; // empty frame slots at the top
  const colourAt = (slot, x) => {
    const f = shown[slot - offset];
    if (!f || !f.length) return rgb(10, 12, 22);
    const bin = Math.min(f.length - 1, Math.floor((x * f.length) / g.columns));
    return colormap((f[bin] - lo) / span);
  };
  for (let y = 0; y < g.rows; y++) {
    for (let x = 0; x < g.columns; x++) g.set(x, y, 0x2580, colourAt(y * 2, x), colourAt(y * 2 + 1, x));
  }
  return { grid: g, lo, hi, shown: shown.length };
}

/** A one-row colour bar for a legend. */
export function colourbar(columns) {
  const g = new Grid(columns, 1);
  for (let x = 0; x < g.columns; x++) g.set(x, 0, 0x2588, colormap(x / Math.max(1, g.columns - 1)));
  return g;
}

const BRAILLE_BITS = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/** A braille canvas: 2×4 dots per cell, one colour per cell (highest priority wins). */
export class Braille {
  constructor(columns, rows) {
    this.columns = Math.max(1, Math.min(512, Math.floor(columns)));
    this.rows = Math.max(1, Math.min(256, Math.floor(rows)));
    this.width = this.columns * 2;
    this.height = this.rows * 4;
    this.bits = new Uint8Array(this.columns * this.rows);
    this.colour = new Uint32Array(this.columns * this.rows).fill(DEFAULT);
    this.prio = new Int8Array(this.columns * this.rows).fill(-1);
    this.bg = new Uint32Array(this.columns * this.rows).fill(DEFAULT);
  }
  plot(px, py, colour = DEFAULT, prio = 0) {
    const x = Math.round(px);
    const y = Math.round(py);
    if (x < 0 || y < 0 || x >= this.width || y >= this.height) return;
    const c = (y >> 2) * this.columns + (x >> 1);
    this.bits[c] |= BRAILLE_BITS[y & 3][x & 1];
    if (prio >= this.prio[c]) { this.prio[c] = prio; this.colour[c] = colour; }
  }
  /** A straight segment between two dot positions. */
  line(x0, y0, x1, y1, colour, prio = 0) {
    const steps = Math.max(1, Math.ceil(Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0))));
    for (let s = 0; s <= steps; s++) this.plot(x0 + ((x1 - x0) * s) / steps, y0 + ((y1 - y0) * s) / steps, colour, prio);
  }
  /** Shade a cell's background (bands, glows). */
  shade(cx, cy, colour) {
    if (cx < 0 || cy < 0 || cx >= this.columns || cy >= this.rows) return;
    this.bg[cy * this.columns + cx] = colour;
  }
  toGrid() {
    const g = new Grid(this.columns, this.rows);
    for (let y = 0; y < this.rows; y++) {
      for (let x = 0; x < this.columns; x++) {
        const c = y * this.columns + x;
        g.set(x, y, this.bits[c] ? 0x2800 + this.bits[c] : 0x20, this.colour[c], this.bg[c]);
      }
    }
    return g;
  }
}

const GREY = rgb(70, 76, 84);
const DIM = rgb(110, 118, 128);

/**
 * Top-down radar fan: the sensor at the bottom centre, range rings, the field
 * of view as a wedge, and the measured distance as an arc across it (this kit
 * reports range, not bearing), with older distances as fading arcs.
 * history: distances in cm, oldest first; present: boolean|null.
 */
export function radarFan(columns, rows, { distances = [], present = null, fovDeg = 120, ping = null } = {}) {
  const c = new Braille(columns, rows);
  const ox = c.width / 2;
  const oy = c.height - 1;
  const known = distances.filter((d) => Number.isFinite(d) && d > 0);
  const latest = known.length ? known[known.length - 1] : null;
  const maxCm = Math.max(150, Math.min(600, Math.ceil(((latest ?? 100) * 1.4) / 50) * 50));
  const half = (fovDeg * Math.PI) / 360;
  // Dots per cm (dots are about square): the wedge's half-width is r·sin(half).
  const scale = Math.min(oy, ox / Math.sin(Math.min(half, Math.PI / 2))) / maxCm;
  const arc = (rCm, colour, prio, density = 1) => {
    const r = rCm * scale;
    const steps = Math.max(8, Math.ceil(r * 2 * half * density));
    for (let s = 0; s <= steps; s++) {
      const a = -half + (2 * half * s) / steps;
      c.plot(ox + r * Math.sin(a), oy - r * Math.cos(a), colour, prio);
    }
  };
  // Wedge edges and rings every 50 cm.
  for (const a of [-half, half]) c.line(ox, oy, ox + maxCm * scale * Math.sin(a), oy - maxCm * scale * Math.cos(a), GREY, 0);
  for (let r = 50; r <= maxCm; r += 50) arc(r, GREY, 0, 0.35);
  // Trail: older distances fade from teal to near-background.
  const trail = known.slice(-8, -1);
  trail.forEach((d, k) => {
    const f = (k + 1) / (trail.length + 1);
    arc(d, rgb(20 + 20 * f, 70 + 90 * f, 80 + 80 * f), 1);
  });
  // Ping (animation): a ring leaving the sensor and reaching the measured range.
  if (ping != null && latest != null) {
    const f = Math.max(0, Math.min(1, ping));
    const fade = 0.35 + 0.65 * f;
    arc(latest * f, rgb(40 * fade, 160 * fade, 200 * fade), 2, 1);
  }
  const hit = ping != null && ping > 0.92;
  if (latest != null) arc(latest, hit ? rgb(220, 255, 235) : present ? rgb(80, 255, 150) : rgb(230, 235, 240), 3, 1.6);
  // Sensor marker.
  c.plot(ox, oy, rgb(255, 220, 90), 4);
  c.plot(ox - 1, oy, rgb(255, 220, 90), 4);
  const g = c.toGrid();
  // Ring labels along the right edge of the wedge.
  for (let r = 100; r <= maxCm; r += 100) {
    const px = ox + r * scale * Math.sin(half);
    const py = oy - r * scale * Math.cos(half);
    g.text(Math.min(g.columns - 3, Math.round(px / 2) + 1), Math.max(0, Math.round(py / 4)), `${r / 100}m`, DIM);
  }
  return { grid: g, maxCm };
}

/**
 * Braille line chart of a series (oldest first, nulls skipped). The scale fits
 * the data (at least `minSpan` wide), so a rhythm stays visible; a plausible
 * band is shaded where it overlaps the scale. Returns { grid, lo, hi }.
 */
export function lineChart(series, columns, rows, { band = null, colour = rgb(0, 200, 220), minSpan = 2 } = {}) {
  const c = new Braille(columns, rows);
  const pts = (series || []).map((v, i) => [i, v]).filter(([, v]) => Number.isFinite(v)).slice(-c.width);
  const vals = pts.map(([, v]) => v);
  let lo = vals.length ? Math.min(...vals) : (band ? band[0] : 0);
  let hi = vals.length ? Math.max(...vals) : (band ? band[1] : 1);
  const pad = Math.max(0, minSpan - (hi - lo)) / 2 + (hi - lo) * 0.1;
  lo -= pad;
  hi += pad;
  if (hi - lo < 1e-9) { lo -= 1; hi += 1; }
  const yOf = (v) => (c.height - 1) - ((v - lo) / (hi - lo)) * (c.height - 1);
  if (band && band[1] >= lo && band[0] <= hi) {
    const top = Math.max(0, Math.floor(yOf(Math.min(band[1], hi)) / 4));
    const bottom = Math.min(c.rows - 1, Math.floor(yOf(Math.max(band[0], lo)) / 4));
    for (let y = top; y <= bottom; y++) for (let x = 0; x < c.columns; x++) c.shade(x, y, rgb(14, 34, 30));
  }
  const n = pts.length;
  const xOf = (k) => (n <= 1 ? c.width - 1 : (k * (c.width - 1)) / (n - 1));
  for (let k = 0; k < n; k++) {
    const y = yOf(pts[k][1]);
    if (k > 0) c.line(xOf(k - 1), yOf(pts[k - 1][1]), xOf(k), y, colour, 1);
    else c.plot(xOf(k), y, colour, 1);
  }
  if (n) c.plot(xOf(n - 1), yOf(pts[n - 1][1]), rgb(255, 255, 255), 2);
  return { grid: c.toGrid(), lo, hi };
}
