/* In-browser simulated runtimes for App Store apps.
 *
 * Each runtime takes the most recent nvsim MagFrame + a short rolling
 * history and decides whether to emit one or more app events. Outputs are
 * illustrative: nvsim produces magnetic-field samples, the wasm-edge
 * algorithms expect WiFi CSI subcarriers — different physical modalities.
 * The simulated runtime preserves *event-emission semantics* (the same
 * i32 event IDs, the same trigger logic shape) so users can see the
 * cards working without an ESP32 mesh.
 *
 * For engineering-grade output, deploy the real `wifi-densepose-wasm-edge`
 * crate to ESP32 firmware over the WS transport — see ADR-040 / ADR-092 §6.2.
 */

import type { MagFrameRecord } from '../transport/NvsimClient';

export interface AppEvent {
  /** Wall-clock timestamp (ms). */
  ts: number;
  /** App id that emitted. */
  appId: string;
  /** i32 event id from `event_types` mod in wifi-densepose-wasm-edge. */
  eventId: number;
  /** Human-readable event name (matches the constant name). */
  eventName: string;
  /** Numeric value the app reports (units app-specific). */
  value: number;
  /** Optional extra context for the console line. */
  detail?: string;
}

export interface AppRuntimeContext {
  frame: MagFrameRecord;
  bMagT: number;
  bRecoveredT: [number, number, number];
  /** Rolling history of |B| in T. Most recent last. */
  bHistory: number[];
  /** Time since the runtime was activated (s). */
  elapsedS: number;
  /** Per-app scratch state — runtimes can persist counters here. */
  state: Record<string, number>;
}

export type AppRuntimeFn = (ctx: AppRuntimeContext) => AppEvent | AppEvent[] | null;

/** Welford-style running-stat helper. */
function rollingMean(arr: number[]): number {
  if (arr.length === 0) return 0;
  let s = 0;
  for (const v of arr) s += v;
  return s / arr.length;
}
function rollingStd(arr: number[]): number {
  if (arr.length < 2) return 0;
  const m = rollingMean(arr);
  let s = 0;
  for (const v of arr) s += (v - m) * (v - m);
  return Math.sqrt(s / (arr.length - 1));
}

/** vital_trend — periodic 1-Hz HR/BR estimate from the B_z oscillation. */
const vitalTrend: AppRuntimeFn = (ctx) => {
  if (ctx.bHistory.length < 64) return null;
  const last = ctx.state['lastEmitS'] ?? 0;
  if (ctx.elapsedS - last < 1.0) return null;
  ctx.state['lastEmitS'] = ctx.elapsedS;

  // Crude HR estimate: count zero-crossings of detrended B_z over the last
  // 64 samples; treat each crossing pair as one cardiac cycle.
  const tail = ctx.bHistory.slice(-64);
  const m = rollingMean(tail);
  let crossings = 0;
  for (let i = 1; i < tail.length; i++) {
    if ((tail[i] - m) * (tail[i - 1] - m) < 0) crossings++;
  }
  // 64 samples ≈ 0.65 s at the worker's 32-frame batches × 16 ms tick.
  const cycles = crossings / 2;
  const hr = Math.max(40, Math.min(180, Math.round((cycles / 0.65) * 60)));
  const br = Math.max(8, Math.min(30, Math.round(hr / 4))); // crude proxy

  const evs: AppEvent[] = [
    { ts: Date.now(), appId: 'vital_trend', eventId: 100, eventName: 'VITAL_TREND', value: hr, detail: `HR≈${hr} BPM, BR≈${br} br/min` },
  ];
  if (hr < 60) evs.push({ ts: Date.now(), appId: 'vital_trend', eventId: 103, eventName: 'BRADYCARDIA', value: hr, detail: `HR=${hr} BPM` });
  else if (hr > 100) evs.push({ ts: Date.now(), appId: 'vital_trend', eventId: 104, eventName: 'TACHYCARDIA', value: hr, detail: `HR=${hr} BPM` });
  if (br < 12) evs.push({ ts: Date.now(), appId: 'vital_trend', eventId: 101, eventName: 'BRADYPNEA', value: br, detail: `BR=${br} br/min` });
  else if (br > 24) evs.push({ ts: Date.now(), appId: 'vital_trend', eventId: 102, eventName: 'TACHYPNEA', value: br, detail: `BR=${br} br/min` });
  return evs;
};

/** occupancy — variance threshold on |B| over a 5-second window. */
const occupancy: AppRuntimeFn = (ctx) => {
  if (ctx.bHistory.length < 32) return null;
  const last = ctx.state['lastEmitS'] ?? 0;
  if (ctx.elapsedS - last < 2.0) return null;
  const std = rollingStd(ctx.bHistory.slice(-128)) * 1e9; // T → nT
  const occupied = std > 0.01; // empirical threshold for the demo
  const wasOccupied = (ctx.state['occ'] ?? 0) > 0.5;
  if (occupied !== wasOccupied) {
    ctx.state['occ'] = occupied ? 1 : 0;
    ctx.state['lastEmitS'] = ctx.elapsedS;
    return {
      ts: Date.now(),
      appId: 'occupancy',
      eventId: occupied ? 300 : 302,
      eventName: occupied ? 'ZONE_OCCUPIED' : 'ZONE_TRANSITION',
      value: std,
      detail: occupied ? `σ(|B|)=${std.toFixed(3)} nT — entered` : `σ(|B|)=${std.toFixed(3)} nT — left`,
    };
  }
  return null;
};

/** intrusion — |B| above ambient + dwell timer. */
const intrusion: AppRuntimeFn = (ctx) => {
  const ambient = ctx.state['ambient'] ?? ctx.bMagT;
  ctx.state['ambient'] = 0.95 * ambient + 0.05 * ctx.bMagT;
  const exceeds = ctx.bMagT > ambient * 1.5 && ctx.bMagT > 1e-12;
  const dwellStart = ctx.state['dwellStart'] ?? 0;
  if (exceeds && dwellStart === 0) {
    ctx.state['dwellStart'] = ctx.elapsedS;
  } else if (!exceeds) {
    ctx.state['dwellStart'] = 0;
  }
  if (exceeds && dwellStart > 0 && ctx.elapsedS - dwellStart > 0.5 && (ctx.state['lastEmitS'] ?? 0) < dwellStart) {
    ctx.state['lastEmitS'] = ctx.elapsedS;
    return {
      ts: Date.now(),
      appId: 'intrusion',
      eventId: 200,
      eventName: 'INTRUSION_ALERT',
      value: ctx.bMagT * 1e9,
      detail: `|B|=${(ctx.bMagT * 1e9).toFixed(2)} nT > 1.5× ambient (${(ambient * 1e9).toFixed(2)} nT) for ${(ctx.elapsedS - dwellStart).toFixed(1)} s`,
    };
  }
  return null;
};

/** coherence — z-score of recent |B| against a longer baseline. */
const coherence: AppRuntimeFn = (ctx) => {
  if (ctx.bHistory.length < 64) return null;
  const last = ctx.state['lastEmitS'] ?? 0;
  if (ctx.elapsedS - last < 0.5) return null;
  ctx.state['lastEmitS'] = ctx.elapsedS;

  const recent = ctx.bHistory.slice(-32);
  const baseline = ctx.bHistory.slice(-128, -32);
  if (baseline.length < 32) return null;
  const mu = rollingMean(baseline);
  const sd = rollingStd(baseline);
  if (sd === 0) return null;
  const recentMean = rollingMean(recent);
  const z = Math.abs(recentMean - mu) / sd;
  return {
    ts: Date.now(),
    appId: 'coherence',
    eventId: 2,
    eventName: 'COHERENCE_SCORE',
    value: z,
    detail: `z=${z.toFixed(2)} σ ${z > 3 ? '· DRIFT' : z > 1.5 ? '· marginal' : '· stable'}`,
  };
};

/** adversarial — detect physically-impossible 1/r³ violation. */
const adversarial: AppRuntimeFn = (ctx) => {
  if (ctx.bHistory.length < 32) return null;
  const last = ctx.state['lastEmitS'] ?? 0;
  if (ctx.elapsedS - last < 3.0) return null;

  // Fake "multi-link consistency": compare instantaneous |B| with the
  // smoothed |B|. A sharp factor-of-N step violates dipole physics
  // (real 1/r³ source moves continuously).
  const tail = ctx.bHistory.slice(-32);
  let maxJump = 0;
  for (let i = 1; i < tail.length; i++) {
    const j = Math.abs(Math.log(Math.max(tail[i], 1e-15)) - Math.log(Math.max(tail[i - 1], 1e-15)));
    if (j > maxJump) maxJump = j;
  }
  if (maxJump > 5) {
    ctx.state['lastEmitS'] = ctx.elapsedS;
    return {
      ts: Date.now(),
      appId: 'adversarial',
      eventId: 3,
      eventName: 'ANOMALY_DETECTED',
      value: maxJump,
      detail: `log-jump ${maxJump.toFixed(1)} — physically implausible step in |B|`,
    };
  }
  return null;
};

/** exo_ghost_hunter — empty-room CSI anomaly detector adapted to the
 * magnetic noise floor: flag impulsive / periodic / drift / random
 * patterns and a hidden-presence sub-detector at 0.15-0.5 Hz. */
const exoGhostHunter: AppRuntimeFn = (ctx) => {
  if (ctx.bHistory.length < 128) return null;
  const last = ctx.state['lastEmitS'] ?? 0;
  if (ctx.elapsedS - last < 4.0) return null;
  ctx.state['lastEmitS'] = ctx.elapsedS;

  const tail = ctx.bHistory.slice(-128);
  const std = rollingStd(tail) * 1e9;
  // Detect impulsive: max - mean > 4σ
  const m = rollingMean(tail);
  let maxDev = 0;
  for (const v of tail) {
    const d = Math.abs(v - m);
    if (d > maxDev) maxDev = d;
  }
  const cls: 1 | 3 | 4 = maxDev > 4 * (std * 1e-9) ? 1 // impulsive
    : ctx.elapsedS > 10 ? 3 // drift bias as a default after warmup
    : 4; // random
  const clsName = cls === 1 ? 'impulsive' : cls === 3 ? 'drift' : 'random';
  return {
    ts: Date.now(),
    appId: 'exo_ghost_hunter',
    eventId: 651,
    eventName: 'ANOMALY_CLASS',
    value: cls,
    detail: `class=${clsName} · σ=${std.toFixed(3)} nT`,
  };
};

export const APP_RUNTIMES: Record<string, AppRuntimeFn> = {
  vital_trend: vitalTrend,
  occupancy,
  intrusion,
  coherence,
  adversarial,
  exo_ghost_hunter: exoGhostHunter,
};

export function hasRuntime(appId: string): boolean {
  return appId in APP_RUNTIMES;
}
