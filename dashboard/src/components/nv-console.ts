/* Console — log stream + REPL. */
import { LitElement, html, css } from 'lit';
import { customElement, query } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import {
  consoleLines, consoleFilter, consolePaused, pushLog,
  getClient, seed, theme, expectedWitness, witnessHex, witnessVerified,
  running, replHistory, pushReplHistory,
} from '../store/appStore';

@customElement('nv-console')
export class NvConsole extends LitElement {
  @query('#console-input') private inputEl!: HTMLInputElement;
  private hIdx = -1;

  static styles = css`
    :host {
      display: flex; flex-direction: column;
      background: var(--bg-1);
      overflow: hidden;
    }
    .tabs {
      display: flex; align-items: center;
      border-bottom: 1px solid var(--line);
      padding: 0 10px;
      gap: 2px;
    }
    .tab {
      padding: 8px 12px;
      background: transparent; border: none;
      font-size: 11.5px; color: var(--ink-3);
      font-family: var(--mono);
      border-bottom: 2px solid transparent;
      cursor: pointer;
      margin-bottom: -1px;
    }
    .tab.active { color: var(--ink); border-bottom-color: var(--accent); }
    .tab .cnt {
      background: var(--bg-3); padding: 1px 5px; border-radius: 999px;
      font-size: 9.5px; color: var(--ink-2); margin-left: 4px;
    }
    .spacer { flex: 1; }
    .tools { display: flex; gap: 4px; padding: 4px 0; }
    .tools button {
      width: 24px; height: 24px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-3);
      font-size: 11px; cursor: pointer;
    }
    .tools button:hover { color: var(--ink); border-color: var(--line-2); }

    .body {
      flex: 1; overflow-y: auto;
      font-family: var(--mono);
      font-size: 11.5px;
      padding: 6px 0;
      background: var(--bg-0);
    }
    .line {
      display: grid;
      grid-template-columns: 70px 60px 1fr;
      gap: 12px;
      padding: 2px 12px;
      color: var(--ink-2);
      border-left: 2px solid transparent;
    }
    .line:hover { background: var(--bg-1); }
    .ts { color: var(--ink-4); font-size: 10.5px; padding-top: 1px; }
    .lvl {
      font-size: 10px; font-weight: 600;
      text-transform: uppercase; letter-spacing: 0.04em; padding-top: 1px;
    }
    .line.info .lvl { color: var(--accent-2); }
    .line.warn .lvl { color: var(--warn); }
    .line.warn { border-left-color: var(--warn); background: oklch(0.7 0.18 35 / 0.04); }
    .line.err .lvl { color: var(--bad); }
    .line.err { border-left-color: var(--bad); background: oklch(0.65 0.22 25 / 0.05); }
    .line.dbg .lvl { color: var(--ink-3); }
    .line.ok .lvl { color: var(--ok); }
    .msg { color: var(--ink); white-space: pre-wrap; word-break: break-word; }

    .input {
      display: flex; align-items: center;
      border-top: 1px solid var(--line);
      background: var(--bg-0);
      padding: 0 10px;
      height: 32px; gap: 8px;
    }
    .prompt { color: var(--accent); font-family: var(--mono); font-size: 12px; }
    input[type="text"] {
      flex: 1; background: transparent; border: none; outline: none;
      color: var(--ink); font-family: var(--mono); font-size: 12px;
      height: 100%;
    }
    input::placeholder { color: var(--ink-4); }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => {
      consoleLines.value; consoleFilter.value; consolePaused.value;
      this.requestUpdate();
    });
  }

  override updated(): void {
    const body = this.renderRoot.querySelector('.body') as HTMLElement | null;
    if (body) body.scrollTop = body.scrollHeight;
  }

  private counts(): Record<string, number> {
    const c: Record<string, number> = { info: 0, warn: 0, err: 0, dbg: 0, ok: 0 };
    for (const l of consoleLines.value) c[l.level] = (c[l.level] ?? 0) + 1;
    c.all = consoleLines.value.length;
    return c;
  }

  private async exec(line: string): Promise<void> {
    line = line.trim();
    if (!line) return;
    pushLog('info', `<span style="color:var(--accent);">nvsim&gt;</span> ${line}`);
    pushReplHistory(line);
    this.hIdx = replHistory.value.length;
    const [cmd, ...args] = line.split(/\s+/);
    const arg = args.join(' ');
    const c = getClient();
    switch (cmd) {
      case 'help':
        pushLog('info', 'commands: help · scene.list · sensor.config · run · pause · reset · seed · proof.verify · proof.export · clear · theme · status');
        break;
      case 'scene.list':
        pushLog('info', 'scene rebar-walkby-01:');
        pushLog('info', '  rebar.steel.coil   @ [+2.7, 0.0, +0.3] m χ=5000');
        pushLog('info', '  dipole.heart_proxy @ [-1.4, +0.2, +0.4] m m=1.0e-6 A·m²');
        pushLog('info', '  loop.mains_60Hz    @ [-1.6, -0.4, 0.0] m I=2 A');
        pushLog('info', '  eddy.door_steel    @ [+0.0, +1.8, +0.4] m σ=1e6 S/m');
        break;
      case 'sensor.config':
        pushLog('info', 'NvSensor::cots_defaults() {');
        pushLog('info', '  pos=[0,0,0], V=1mm³, N=1e12, C=0.03, T2*=200ns');
        pushLog('info', '  D=2.870 GHz, γe=28 GHz/T, Γ=1.0 MHz, axes=4×〈111〉');
        pushLog('info', '  δB ≈ 1.18 pT/√Hz (Barry 2020 §III.A) }');
        break;
      case 'run':
        if (c) { await c.run(); running.value = true; pushLog('ok', 'pipeline RUN'); }
        break;
      case 'pause':
        if (c) { await c.pause(); running.value = false; pushLog('warn', 'pipeline PAUSED'); }
        break;
      case 'reset':
        if (c) { await c.reset(); pushLog('info', 'pipeline reset · t=0'); }
        break;
      case 'seed': {
        if (!arg) { pushLog('info', `current seed = 0x${seed.value.toString(16).toUpperCase()}`); break; }
        const v = BigInt(arg.startsWith('0x') ? arg : '0x' + arg);
        seed.value = v;
        if (c) await c.setSeed(v);
        pushLog('ok', `seed → 0x${v.toString(16).toUpperCase()}`);
        break;
      }
      case 'proof.verify': {
        if (!c) break;
        pushLog('dbg', 'computing SHA-256 over 256 frames…');
        try {
          const exp = expectedWitness.value;
          const expBytes = new Uint8Array(32);
          for (let i = 0; i < 32; i++) expBytes[i] = parseInt(exp.slice(i * 2, i * 2 + 2), 16);
          const r = await c.verifyWitness(expBytes);
          if (r.ok) { witnessVerified.value = 'ok'; witnessHex.value = exp; pushLog('ok', `witness ${exp.slice(0, 16)}… matches · determinism gate ✓`); }
          else { witnessVerified.value = 'fail'; pushLog('err', 'WITNESS MISMATCH'); }
        } catch (e) { pushLog('err', `verify failed: ${(e as Error).message}`); }
        break;
      }
      case 'proof.export': {
        if (!c) break;
        pushLog('dbg', 'building proof bundle…');
        try {
          const blob = await c.exportProofBundle();
          const url = URL.createObjectURL(blob);
          const a = document.createElement('a');
          a.href = url;
          a.download = `nvsim-proof-${Date.now()}.json`;
          a.click();
          URL.revokeObjectURL(url);
          pushLog('ok', `proof bundle exported · ${blob.size} bytes`);
        } catch (e) { pushLog('err', `export failed: ${(e as Error).message}`); }
        break;
      }
      case 'clear':
        consoleLines.value = [];
        break;
      case 'theme': {
        const t = (arg || '').toLowerCase();
        if (t === 'light' || t === 'dark') { theme.value = t; pushLog('ok', `theme → ${t}`); }
        else pushLog('info', 'theme [light|dark]');
        break;
      }
      case 'status':
        pushLog('info', `running=${running.value} seed=0x${seed.value.toString(16).toUpperCase()} verified=${witnessVerified.value}`);
        break;
      default:
        pushLog('err', `unknown command: ${cmd} · try help`);
    }
  }

  private onKey = (e: KeyboardEvent): void => {
    if (e.key === 'Enter') { void this.exec(this.inputEl.value); this.inputEl.value = ''; }
    else if (e.key === 'ArrowUp') {
      const h = replHistory.value;
      if (h.length) {
        this.hIdx = Math.max(0, this.hIdx - 1);
        this.inputEl.value = h[this.hIdx] ?? '';
        e.preventDefault();
      }
    } else if (e.key === 'ArrowDown') {
      const h = replHistory.value;
      if (h.length) {
        this.hIdx = Math.min(h.length, this.hIdx + 1);
        this.inputEl.value = h[this.hIdx] ?? '';
        e.preventDefault();
      }
    }
  };

  override render() {
    const c = this.counts();
    const filter = consoleFilter.value;
    const visible = consoleLines.value.filter((l) => filter === 'all' || l.level === filter);
    return html`
      <div class="tabs">
        ${(['all', 'info', 'warn', 'err', 'dbg'] as const).map((k) => html`
          <button class="tab ${filter === k ? 'active' : ''}" data-tab=${k}
            @click=${() => consoleFilter.value = k}>
            ${k} <span class="cnt">${c[k] ?? 0}</span>
          </button>
        `)}
        <span class="spacer"></span>
        <div class="tools">
          <button id="clear-log" title="Clear" @click=${() => consoleLines.value = []}>×</button>
          <button id="pause-log" title="Pause" @click=${() => consolePaused.value = !consolePaused.value}>
            ${consolePaused.value ? '▶' : '❚❚'}
          </button>
        </div>
      </div>
      <div class="body" role="log" aria-live="polite" aria-label="Console output">
        ${visible.map((l) => {
          const ts = new Date(l.ts);
          const tsStr = `${String(ts.getSeconds()).padStart(2, '0')}.${String(ts.getMilliseconds()).padStart(3, '0')}`;
          // Use innerHTML pass-through via unsafe-html alt: inject raw html via property
          return html`<div class="line ${l.level}">
            <div class="ts">${tsStr}</div>
            <div class="lvl">${l.level}</div>
            <div class="msg" .innerHTML=${l.msg}></div>
          </div>`;
        })}
      </div>
      <div class="input">
        <span class="prompt">nvsim&gt;</span>
        <input id="console-input" type="text"
          placeholder="help · scene.list · sensor.config · run · proof.verify · clear"
          @keydown=${this.onKey}/>
      </div>
    `;
  }
}
