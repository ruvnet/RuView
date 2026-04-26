/* Inspector — tabbed: Signal / Frame / Witness. */
import { LitElement, html, css, svg } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import {
  traceX, traceY, traceZ, stripBars, lastFrame,
  witnessHex, expectedWitness, witnessVerified, getClient,
  pushLog, lastB, bMag,
} from '../store/appStore';

type Tab = 'signal' | 'frame' | 'witness';

@customElement('nv-inspector')
export class NvInspector extends LitElement {
  @state() private tab: Tab = 'signal';

  static styles = css`
    :host {
      display: flex; flex-direction: column;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      overflow: hidden;
      height: 100%;
    }
    .tabs {
      display: flex; border-bottom: 1px solid var(--line);
    }
    .tab {
      flex: 1;
      padding: 11px 8px;
      background: transparent; border: none;
      font-size: 11.5px; font-weight: 500;
      color: var(--ink-3);
      border-bottom: 2px solid transparent;
      cursor: pointer; transition: color 0.15s, border-color 0.15s;
    }
    .tab.active { color: var(--ink); border-bottom-color: var(--accent); }
    .tab:hover { color: var(--ink-2); }
    .body { padding: 14px; flex: 1; overflow-y: auto; }

    .card {
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: var(--radius); padding: 12px;
      margin-bottom: 12px;
    }
    .card-h {
      display: flex; justify-content: space-between; align-items: center;
      margin-bottom: 8px;
    }
    .card-h .ttl { font-size: 12px; font-weight: 600; }
    .badge {
      font-family: var(--mono); font-size: 10px;
      padding: 2px 6px;
      background: oklch(0.78 0.14 195 / 0.12);
      color: var(--accent-2);
      border-radius: 4px;
      border: 1px solid oklch(0.78 0.14 195 / 0.3);
    }
    svg { width: 100%; height: 130px; }
    .frame-strip {
      height: 28px;
      display: flex; align-items: flex-end; gap: 1px;
      padding: 4px 0;
    }
    .bar {
      flex: 1;
      background: linear-gradient(to top, var(--accent-2), var(--accent));
      border-radius: 1px;
      min-height: 2px;
    }
    table { width: 100%; border-collapse: collapse; font-family: var(--mono); font-size: 10.5px; }
    td { padding: 4px 0; border-bottom: 1px solid var(--line); }
    td:first-child { color: var(--ink-3); }
    td:last-child { text-align: right; color: var(--ink); }
    .hex {
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: var(--radius-sm);
      padding: 10px;
      font-family: var(--mono);
      font-size: 10.5px;
      color: var(--ink-2);
      line-height: 1.6;
      overflow-x: auto;
      white-space: nowrap;
    }
    .hex .magic { color: var(--accent); font-weight: 600; }
    .witness-box {
      font-family: var(--mono);
      font-size: 11px;
      color: var(--ink-2);
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 8px 10px;
      word-break: break-all;
      line-height: 1.5;
    }
    .verify-btn {
      margin-top: 10px;
      width: 100%;
      padding: 8px;
      border: 1px solid var(--line);
      background: var(--bg-3);
      color: var(--ink);
      border-radius: 8px;
      cursor: pointer;
      font-family: var(--mono);
      font-size: 12px;
    }
    .verify-btn:hover { border-color: var(--accent); }
    .verify-btn.ok { border-color: var(--ok); color: var(--ok); }
    .verify-btn.fail { border-color: var(--bad); color: var(--bad); }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => {
      traceX.value; traceY.value; traceZ.value; stripBars.value;
      lastFrame.value; witnessHex.value; witnessVerified.value;
      lastB.value; bMag.value;
      this.requestUpdate();
    });
  }

  private async verify(): Promise<void> {
    const c = getClient(); if (!c) return;
    witnessVerified.value = 'pending';
    pushLog('info', 'verifying witness over 256 frames…');
    try {
      const exp = expectedWitness.value;
      const expBytes = new Uint8Array(32);
      for (let i = 0; i < 32; i++) expBytes[i] = parseInt(exp.slice(i * 2, i * 2 + 2), 16);
      const r = await c.verifyWitness(expBytes);
      if (r.ok) {
        witnessVerified.value = 'ok';
        witnessHex.value = exp;
        pushLog('ok', `witness ${exp.slice(0, 16)}… matches · determinism gate ✓`);
      } else {
        witnessVerified.value = 'fail';
        const actual = Array.from(r.actual).map((b) => b.toString(16).padStart(2, '0')).join('');
        witnessHex.value = actual;
        pushLog('err', `WITNESS MISMATCH actual=${actual.slice(0, 16)}…`);
      }
    } catch (e) {
      witnessVerified.value = 'fail';
      pushLog('err', `verify failed: ${(e as Error).message}`);
    }
  }

  private renderSignalTab() {
    const W = 320, H = 130, cy = 65, scale = 22;
    const cap = 200;
    const make = (arr: number[]) => {
      let p = '';
      arr.forEach((v, i) => {
        const x = (i / Math.max(1, cap - 1)) * W;
        const y = cy - v * scale;
        p += (i === 0 ? 'M' : 'L') + ` ${x.toFixed(1)} ${y.toFixed(1)} `;
      });
      return p;
    };

    return html`
      <div class="card">
        <div class="card-h">
          <span class="ttl">B-vector trace</span>
          <span class="badge">3-axis · nT</span>
        </div>
        <svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="none">
          <line x1="0" y1=${cy} x2=${W} y2=${cy} stroke="var(--line)" stroke-width="0.5"/>
          ${svg`<path id="trace-x" d=${make(traceX.value)} stroke="oklch(0.78 0.14 70)" stroke-width="1.2" fill="none"/>`}
          ${svg`<path id="trace-y" d=${make(traceY.value)} stroke="oklch(0.78 0.12 195)" stroke-width="1.2" fill="none" opacity="0.8"/>`}
          ${svg`<path id="trace-z" d=${make(traceZ.value)} stroke="oklch(0.72 0.18 330)" stroke-width="1.2" fill="none" opacity="0.7"/>`}
        </svg>
      </div>

      <div class="card">
        <div class="card-h">
          <span class="ttl">Frame stream</span>
          <span class="badge" id="strip-rate">live</span>
        </div>
        <div class="frame-strip" id="frame-strip">
          ${stripBars.value.map((v) => html`<div class="bar" style=${`height:${Math.max(4, v * 100)}%`}></div>`)}
        </div>
      </div>
    `;
  }

  private renderFrameTab() {
    const f = lastFrame.value;
    const bytes = f?.raw;
    let hex = '';
    if (bytes) {
      const arr = Array.from(bytes).map((b) => b.toString(16).padStart(2, '0'));
      hex = arr.slice(0, 60).join(' ');
    }
    return html`
      <div class="card">
        <div class="card-h">
          <span class="ttl">MagFrame v1 fields</span>
          <span class="badge">60 B</span>
        </div>
        <table>
          <tr><td>magic</td><td id="frame-magic">${f ? '0x' + f.magic.toString(16).toUpperCase() : '—'}</td></tr>
          <tr><td>version</td><td>${f?.version ?? '—'}</td></tr>
          <tr><td>flags</td><td>0x${(f?.flags ?? 0).toString(16).padStart(4, '0')}</td></tr>
          <tr><td>sensor_id</td><td>${f?.sensorId ?? '—'}</td></tr>
          <tr><td>t_us</td><td>${f ? f.tUs.toString() : '—'}</td></tr>
          <tr><td>b_pT[0]</td><td id="frame-bx">${f ? f.bPt[0].toFixed(1) : '—'}</td></tr>
          <tr><td>b_pT[1]</td><td id="frame-by">${f ? f.bPt[1].toFixed(1) : '—'}</td></tr>
          <tr><td>b_pT[2]</td><td id="frame-bz">${f ? f.bPt[2].toFixed(1) : '—'}</td></tr>
          <tr><td>noise_floor</td><td>${f ? f.noiseFloorPtSqrtHz.toFixed(2) : '—'}</td></tr>
          <tr><td>temp_K</td><td>${f ? f.temperatureK.toFixed(1) : '—'}</td></tr>
        </table>
      </div>
      <div class="card">
        <div class="card-h">
          <span class="ttl">Hex dump</span>
          <span class="badge">LE</span>
        </div>
        <div class="hex" id="frame-hex">${hex || '—'}</div>
      </div>
    `;
  }

  private renderWitnessTab() {
    const status = witnessVerified.value;
    const cls = status === 'ok' ? 'ok' : status === 'fail' ? 'fail' : '';
    const label =
      status === 'pending' ? 'Verifying…' :
      status === 'ok' ? '✓ Witness verified · determinism gate' :
      status === 'fail' ? '✗ Witness mismatch · audit required' :
      'Verify witness';
    return html`
      <div class="card">
        <div class="card-h">
          <span class="ttl">Expected (Proof::EXPECTED_WITNESS_HEX)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="expected-witness">${expectedWitness.value || '(loading…)'}</div>
      </div>
      <div class="card">
        <div class="card-h">
          <span class="ttl">Actual (last verify)</span>
          <span class="badge">SHA-256</span>
        </div>
        <div class="witness-box" id="actual-witness">${witnessHex.value || '(not verified yet)'}</div>
        <button class="verify-btn ${cls}" id="verify-btn" @click=${this.verify}>${label}</button>
      </div>
    `;
  }

  override render() {
    return html`
      <div class="tabs">
        <button class="tab ${this.tab === 'signal' ? 'active' : ''}" data-pane="signal" @click=${() => this.tab = 'signal'}>Signal</button>
        <button class="tab ${this.tab === 'frame' ? 'active' : ''}" data-pane="frame" @click=${() => this.tab = 'frame'}>Frame</button>
        <button class="tab ${this.tab === 'witness' ? 'active' : ''}" data-pane="witness" @click=${() => this.tab = 'witness'}>Witness</button>
      </div>
      <div class="body">
        ${this.tab === 'signal' ? this.renderSignalTab()
          : this.tab === 'frame' ? this.renderFrameTab()
          : this.renderWitnessTab()}
      </div>
    `;
  }
}
