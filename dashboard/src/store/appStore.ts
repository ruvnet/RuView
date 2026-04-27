/* Application-wide reactive state.
 *
 * One signal per logical observable; components subscribe to only the
 * signals they read. Keeps re-renders surgical even at 1 kHz frame rates.
 * Persistence lives in `persistence.ts`; this module is pure state.
 */
import { signal, computed } from '@preact/signals-core';
import type { NvsimClient, MagFrameRecord, NvsimEvent } from '../transport/NvsimClient';

export type Theme = 'dark' | 'light';
export type Density = 'comfy' | 'default' | 'compact';
export type TransportMode = 'wasm' | 'ws';

export const transport = signal<TransportMode>('wasm');
export const wsUrl = signal<string>('');
export const connected = signal<boolean>(false);
export const transportError = signal<string | null>(null);

export const running = signal<boolean>(false);
export const paused = signal<boolean>(true);
export const speed = signal<number>(1.0);
export const t = signal<number>(0); // sim time (s)
export const framesEmitted = signal<bigint>(0n);

export const seed = signal<bigint>(0xCAFEBABEn);

export const fs = signal<number>(10000); // sample rate Hz
export const fmod = signal<number>(1000); // lockin Hz
export const dtMs = signal<number>(1.0);
export const noiseEnabled = signal<boolean>(true);

export const theme = signal<Theme>('dark');
export const density = signal<Density>('default');
export const motionReduced = signal<boolean>(false);
export const autoUpdate = signal<boolean>(true);

export const lastB = signal<[number, number, number]>([0, 0, 0]); // T
export const bMag = signal<number>(0);
export const snr = signal<number>(0);
export const fps = signal<number>(0);

export const witnessHex = signal<string>('');
export const witnessVerified = signal<'pending' | 'ok' | 'fail' | 'idle'>('idle');
export const expectedWitness = signal<string>('');

export const lastFrame = signal<MagFrameRecord | null>(null);
export const traceX = signal<number[]>([]);
export const traceY = signal<number[]>([]);
export const traceZ = signal<number[]>([]);
export const stripBars = signal<number[]>([]);

export const sceneName = signal<string>('rebar-walkby-01');
export const sceneJson = signal<string>('');

export const consolePaused = signal<boolean>(false);
export const consoleFilter = signal<'all' | 'info' | 'warn' | 'err' | 'dbg' | 'ok'>('all');

/** REPL command history, persisted via persistence.ts (kvSet 'repl-history'). */
export const replHistory = signal<string[]>([]);
export function pushReplHistory(cmd: string): void {
  const next = replHistory.value.slice();
  next.push(cmd);
  while (next.length > 200) next.shift();
  replHistory.value = next;
}

/** Scene drag positions, persisted via persistence.ts (kvSet 'scene-positions'). */
export interface SceneItemPos { id: string; x: number; y: number }
export const scenePositions = signal<SceneItemPos[]>([]);

/** App-runtime emitted events. See appRuntimes.ts. */
import type { AppEvent } from './appRuntimes';
export const appEvents = signal<AppEvent[]>([]);
export const appEventCounts = signal<Record<string, number>>({});

export function pushAppEvent(ev: AppEvent): void {
  const next = appEvents.value.slice();
  next.push(ev);
  while (next.length > 200) next.shift();
  appEvents.value = next;

  const c = { ...appEventCounts.value };
  c[ev.appId] = (c[ev.appId] ?? 0) + 1;
  appEventCounts.value = c;
}

/** Active app activations — driven by the App Store toggles. Mirrored
 * from `apps.ts` but exposed as a signal here so `main.ts` can dispatch
 * frames to active runtimes without importing the App Store component. */
export const activeAppIds = signal<Set<string>>(new Set());

export const transportLabel = computed<string>(() =>
  transport.value === 'wasm' ? 'wasm' : 'ws',
);

let _client: NvsimClient | null = null;
export function setClient(c: NvsimClient): void { _client = c; }
export function getClient(): NvsimClient | null { return _client; }

export interface ConsoleLine {
  ts: number;
  level: 'info' | 'warn' | 'err' | 'dbg' | 'ok';
  msg: string;
}
export const consoleLines = signal<ConsoleLine[]>([]);
const MAX_LINES = 200;

export function pushLog(level: ConsoleLine['level'], msg: string): void {
  if (consolePaused.value) return;
  const next = consoleLines.value.slice();
  next.push({ ts: Date.now(), level, msg });
  while (next.length > MAX_LINES) next.shift();
  consoleLines.value = next;
}

export function pushTrace(b: [number, number, number]): void {
  const cap = 200;
  const x = traceX.value.slice(); x.push(b[0]); if (x.length > cap) x.shift();
  const y = traceY.value.slice(); y.push(b[1]); if (y.length > cap) y.shift();
  const z = traceZ.value.slice(); z.push(b[2]); if (z.length > cap) z.shift();
  traceX.value = x;
  traceY.value = y;
  traceZ.value = z;
}

export function pushStripBar(amp: number): void {
  const cap = 48;
  const next = stripBars.value.slice();
  next.push(Math.max(0, Math.min(1, amp)));
  while (next.length > cap) next.shift();
  stripBars.value = next;
}

export function recordEvent(_ev: NvsimEvent): void {
  // future: route NvsimEvent into store updates per type. For V1 the
  // worker pushes B-vector / frame data directly via the data plane.
}
