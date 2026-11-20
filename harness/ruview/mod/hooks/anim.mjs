// SPDX-License-Identifier: MIT
// ruview-live animation frames (ADR-378): pure functions of time that the
// hooks module repaints in place with `$.ui.blit` (no redraw). Decoration is
// labelled as decoration; anything driven by a reading says which reading.

import { Grid, rgb } from './raster.mjs';

const mix = (a, b, f) => rgb(
  ((a >> 16) & 255) + ((((b >> 16) & 255) - ((a >> 16) & 255)) * f),
  ((a >> 8) & 255) + ((((b >> 8) & 255) - ((a >> 8) & 255)) * f),
  (a & 255) + (((b & 255) - (a & 255)) * f),
);
const CYAN = rgb(0, 190, 220);
const EMERALD = rgb(40, 210, 140);
const DARK = rgb(16, 22, 30);

/** A one-row cyan→emerald rule with a highlight sweeping across it (pure decoration). */
export function shimmer(columns, tMs) {
  const g = new Grid(columns, 1);
  const head = ((tMs / 2400) % 1) * (g.columns + 24) - 12;
  for (let x = 0; x < g.columns; x++) {
    const base = mix(CYAN, EMERALD, x / Math.max(1, g.columns - 1));
    const glow = Math.max(0, 1 - Math.abs(x - head) / 10);
    g.set(x, 0, 0x2501, mix(mix(DARK, base, 0.55), rgb(235, 255, 250), glow * glow));
  }
  return g;
}

/** Phase in [0, 1) of a rhythm at `perMinute` (0 when unknown). */
export function phaseOf(tMs, perMinute) {
  if (!Number.isFinite(perMinute) || perMinute <= 0) return 0;
  return ((tMs / 60000) * perMinute) % 1;
}

/**
 * A heart icon that beats, and a breathing gauge that fills and empties, at
 * the device-reported rates: a metronome of the reading, not a waveform.
 * Unknown rates draw a still, dim strip.
 */
export function pulse(columns, tMs, { heartBpm = null, breathingBpm = null } = {}) {
  const g = new Grid(columns, 1);
  const beat = Number.isFinite(heartBpm) ? Math.exp(-phaseOf(tMs, heartBpm) * 7) : 0;
  g.set(0, 0, 0x2665, Number.isFinite(heartBpm) ? mix(rgb(90, 20, 35), rgb(255, 90, 120), beat) : rgb(70, 76, 84));
  const width = g.columns - 3;
  const breath = Number.isFinite(breathingBpm) ? 0.5 - 0.5 * Math.cos(2 * Math.PI * phaseOf(tMs, breathingBpm)) : 0;
  const fill = breath * width;
  for (let x = 0; x < width; x++) {
    const f = Math.max(0, Math.min(1, fill - x));
    const on = mix(rgb(40, 120, 200), rgb(140, 220, 255), x / Math.max(1, width));
    g.set(x + 3, 0, f > 0 ? 0x2588 : 0x2500, f > 0 ? mix(DARK, on, f) : rgb(40, 46, 54));
  }
  return g;
}

/**
 * Waterfall replay: frames arrive a capture at a time, so the view shows them
 * at their arrival rate instead of jumping. `lag` is how many received frames
 * are not shown yet; it drains at `rateHz` and never exceeds what is held.
 */
export function drain(lag, rateHz, dtMs) {
  if (!Number.isFinite(lag) || lag <= 0) return 0;
  const rate = Number.isFinite(rateHz) && rateHz > 0 ? rateHz : 10;
  return Math.max(0, lag - (rate * dtMs) / 1000);
}

/** Frames to show now: all but the `lag` newest. */
export function shownFrames(frames, lag) {
  const hold = Math.min(frames.length, Math.max(0, Math.floor(lag || 0)));
  return hold ? frames.slice(0, frames.length - hold) : frames;
}

/** Sonar ping phase for the radar fan: a ring leaving the sensor every 1.6 s. */
export const pingOf = (tMs) => (tMs % 1600) / 1600;
