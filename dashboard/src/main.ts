/* nvsim dashboard entry — boots the WasmClient, mounts <nv-app>. */
import './app.css';
import './components/nv-app';
import { effect } from '@preact/signals-core';

import { WasmClient } from './transport/WasmClient';
import { WsClient } from './transport/WsClient';
import type { NvsimClient, MagFrameBatch } from './transport/NvsimClient';
import {
  setClient, transport, wsUrl, connected, transportError,
  theme, density, motionReduced,
  pushLog, expectedWitness, framesEmitted, fps, lastB, bMag,
  pushTrace, pushStripBar, lastFrame, sceneJson, witnessHex,
  replHistory, scenePositions, type SceneItemPos,
  activeAppIds, pushAppEvent,
} from './store/appStore';
import { APP_RUNTIMES, type AppRuntimeContext } from './store/appRuntimes';
import { kvGet, kvSet } from './store/persistence';

function applyTheme(t: string): void {
  document.documentElement.setAttribute('data-theme', t);
}
function applyDensity(d: string): void {
  document.body.classList.remove('density-comfy', 'density-default', 'density-compact');
  document.body.classList.add(`density-${d}`);
}
function applyMotion(reduced: boolean): void {
  document.body.classList.toggle('reduce-motion', reduced);
}

(async () => {
  // Restore persisted prefs
  const t = (await kvGet<'dark' | 'light'>('theme')) ?? 'dark';
  const d = (await kvGet<'comfy' | 'default' | 'compact'>('density')) ?? 'default';
  const sysMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
  const m = (await kvGet<boolean>('motionReduced')) ?? sysMotion;
  theme.value = t; applyTheme(t);
  density.value = d; applyDensity(d);
  motionReduced.value = m; applyMotion(m);

  // React to changes → persist
  effect(() => { applyTheme(theme.value); kvSet('theme', theme.value); });
  effect(() => { applyDensity(density.value); kvSet('density', density.value); });
  effect(() => { applyMotion(motionReduced.value); kvSet('motionReduced', motionReduced.value); });

  // REPL history + scene drag positions persistence (P0.10, P1.7)
  const histSaved = await kvGet<string[]>('repl-history');
  if (histSaved && Array.isArray(histSaved)) replHistory.value = histSaved;
  effect(() => { void kvSet('repl-history', replHistory.value); });
  const positionsSaved = await kvGet<SceneItemPos[]>('scene-positions');
  if (positionsSaved && Array.isArray(positionsSaved)) scenePositions.value = positionsSaved;
  effect(() => { void kvSet('scene-positions', scenePositions.value); });

  // Restore WS URL preference + transport mode
  const savedWsUrl = (await kvGet<string>('wsUrl')) ?? '';
  if (savedWsUrl) wsUrl.value = savedWsUrl;
  const savedTransport = (await kvGet<'wasm' | 'ws'>('transport')) ?? 'wasm';
  transport.value = savedTransport;
  effect(() => { void kvSet('wsUrl', wsUrl.value); });
  effect(() => { void kvSet('transport', transport.value); });

  // Per-app runtime scratch state + history buffer (defined first so the
  // onFrames callback can close over them).
  const appState: Record<string, Record<string, number>> = {};
  const bMagHistory: number[] = [];
  const runtimeStartTs = performance.now();

  const onFrames = (batch: MagFrameBatch): void => {
    if (batch.frames.length === 0) return;
    const last = batch.frames[batch.frames.length - 1];
    lastFrame.value = last;
    const bx = last.bPt[0] * 1e-12;
    const by = last.bPt[1] * 1e-12;
    const bz = last.bPt[2] * 1e-12;
    lastB.value = [bx, by, bz];
    const bmagT = Math.sqrt(bx * bx + by * by + bz * bz);
    bMag.value = bmagT;
    pushTrace([bx * 1e9, by * 1e9, bz * 1e9]);
    pushStripBar(Math.min(1, Math.abs(bz * 1e9) / 5 + 0.3));
    bMagHistory.push(bmagT);
    while (bMagHistory.length > 256) bMagHistory.shift();

    const activeIds = activeAppIds.value;
    if (activeIds.size === 0) return;
    const elapsedS = (performance.now() - runtimeStartTs) / 1000;
    for (const id of activeIds) {
      const fn = APP_RUNTIMES[id];
      if (!fn) continue;
      if (!appState[id]) appState[id] = {};
      const ctx: AppRuntimeContext = {
        frame: last,
        bMagT: bmagT,
        bRecoveredT: [bx, by, bz],
        bHistory: bMagHistory,
        elapsedS,
        state: appState[id],
      };
      try {
        const result = fn(ctx);
        if (!result) continue;
        const evs = Array.isArray(result) ? result : [result];
        for (const ev of evs) {
          pushAppEvent(ev);
          pushLog('info',
            `<span class="k">[${ev.appId}]</span> <span class="s">${ev.eventName}</span> <span class="n">(${ev.eventId})</span>${ev.detail ? ' · ' + ev.detail : ''}`);
        }
      } catch (e) {
        pushLog('warn', `[${id}] runtime error: ${(e as Error).message}`);
      }
    }
  };

  // Boot transport (WASM by default, WS if user previously selected it)
  let activeClient: NvsimClient | null = null;
  async function bootTransport(): Promise<void> {
    try {
      if (activeClient) await activeClient.close();
      const want = transport.value;
      if (want === 'ws' && wsUrl.value.trim()) {
        const c = new WsClient(wsUrl.value.trim());
        const info = await c.boot();
        activeClient = c;
        connected.value = true;
        transportError.value = null;
        expectedWitness.value = info.expectedWitnessHex;
        wireClient(c);
        pushLog('ok', `transport WS · ${wsUrl.value} · nvsim@${info.buildVersion}`);
      } else {
        if (want === 'ws') {
          pushLog('warn', 'WS transport selected but no URL set — falling back to WASM');
        }
        const c = new WasmClient();
        const info = await c.boot();
        activeClient = c;
        connected.value = true;
        transportError.value = null;
        expectedWitness.value = info.expectedWitnessHex;
        wireClient(c);
        pushLog('ok', `transport WASM · nvsim@${info.buildVersion} · magic=0x${info.frameMagic.toString(16).toUpperCase()}`);
      }
      setClient(activeClient);
    } catch (e) {
      const msg = (e as Error).message;
      transportError.value = msg;
      connected.value = false;
      pushLog('err', `transport boot failed: ${msg}`);
    }
  }
  function wireClient(c: NvsimClient): void {
    c.onEvent((ev) => {
      if (ev.type === 'log') pushLog(ev.level, ev.msg);
      if (ev.type === 'fps') fps.value = ev.value;
      if (ev.type === 'state') framesEmitted.value = BigInt(ev.framesEmitted);
    });
    c.onFrames(onFrames);
  }

  // React to transport-mode flips: tear down + re-boot.
  let bootInProgress = false;
  effect(() => {
    transport.value; wsUrl.value;
    if (bootInProgress) return;
    bootInProgress = true;
    void bootTransport().finally(() => { bootInProgress = false; });
  });

  pushLog('info', 'nvsim — booting transport');

  // Initial boot — handled by the effect() above.
  // Auto-verify witness whenever a fresh transport boot completes.
  let verifiedFor: string | null = null;
  effect(() => {
    const exp = expectedWitness.value;
    const isConn = connected.value;
    if (!exp || !isConn) return;
    if (verifiedFor === exp) return;
    verifiedFor = exp;
    void (async () => {
      const c = activeClient;
      if (!c) return;
      try {
        const expBytes = new Uint8Array(32);
        for (let i = 0; i < 32; i++) expBytes[i] = parseInt(exp.slice(i * 2, i * 2 + 2), 16);
        const r = await c.verifyWitness(expBytes);
        if (r.ok) {
          witnessHex.value = exp;
          pushLog('ok', `witness verified · determinism gate ✓ · transport=${transport.value}`);
        } else {
          const actual = Array.from(r.actual).map((b) => b.toString(16).padStart(2, '0')).join('');
          witnessHex.value = actual;
          pushLog('err', `WITNESS MISMATCH · expected ${exp.slice(0, 16)}… got ${actual.slice(0, 16)}…`);
        }
      } catch (e) {
        pushLog('warn', `witness verify skipped: ${(e as Error).message}`);
      }
    })();
  });

  sceneJson.value = '(reference scene)';
})();
