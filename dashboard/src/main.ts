/* nvsim dashboard entry — boots the WasmClient, mounts <nv-app>. */
import './app.css';
import './components/nv-app';
import { effect } from '@preact/signals-core';

import { WasmClient } from './transport/WasmClient';
import {
  setClient, transport, theme, density, motionReduced,
  pushLog, expectedWitness, framesEmitted, fps, lastB, bMag,
  pushTrace, pushStripBar, lastFrame, sceneJson, witnessHex,
} from './store/appStore';
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

  // Boot WASM client
  const client = new WasmClient();
  setClient(client);

  pushLog('info', 'nvsim — booting WASM runtime');
  client.onEvent((ev) => {
    if (ev.type === 'log') pushLog(ev.level, ev.msg);
    if (ev.type === 'fps') fps.value = ev.value;
    if (ev.type === 'state') {
      framesEmitted.value = BigInt(ev.framesEmitted);
    }
  });

  client.onFrames((batch) => {
    if (batch.frames.length === 0) return;
    const last = batch.frames[batch.frames.length - 1];
    lastFrame.value = last;
    const bx = last.bPt[0] * 1e-12; // pT → T
    const by = last.bPt[1] * 1e-12;
    const bz = last.bPt[2] * 1e-12;
    lastB.value = [bx, by, bz];
    bMag.value = Math.sqrt(bx * bx + by * by + bz * bz);
    // For trace display we use nT scale.
    pushTrace([bx * 1e9, by * 1e9, bz * 1e9]);
    const amp = Math.min(1, Math.abs(bz * 1e9) / 5 + 0.3);
    pushStripBar(amp);
  });

  try {
    const info = await client.boot();
    expectedWitness.value = info.expectedWitnessHex;
    pushLog('ok', `WASM module ready · nvsim@${info.buildVersion} · magic=0x${info.frameMagic.toString(16).toUpperCase()}`);
    pushLog('info', `expected witness · ${info.expectedWitnessHex.slice(0, 16)}…`);

    // Load reference scene by default.
    sceneJson.value = '(reference scene)';
    transport.value = 'wasm';
  } catch (e) {
    pushLog('err', `boot failed: ${(e as Error).message}`);
  }

  // Auto-verify witness once at boot — proves WASM determinism contract.
  try {
    const exp = expectedWitness.value;
    if (exp) {
      const expBytes = new Uint8Array(32);
      for (let i = 0; i < 32; i++) expBytes[i] = parseInt(exp.slice(i * 2, i * 2 + 2), 16);
      const r = await client.verifyWitness(expBytes);
      if (r.ok) {
        witnessHex.value = exp;
        pushLog('ok', `witness verified · determinism gate ✓`);
      } else {
        const actual = Array.from(r.actual)
          .map((b) => b.toString(16).padStart(2, '0'))
          .join('');
        witnessHex.value = actual;
        pushLog('err', `WITNESS MISMATCH · expected ${exp.slice(0, 16)}… got ${actual.slice(0, 16)}…`);
      }
    }
  } catch (e) {
    pushLog('warn', `witness verify skipped: ${(e as Error).message}`);
  }
})();
