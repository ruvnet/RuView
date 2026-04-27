/* Inspector — tabbed: Signal / Frame / Witness. */
import { LitElement, html, css, svg, type PropertyValues } from 'lit';
import { customElement, state, property } from 'lit/decorators.js';
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
  /** When set by the parent, force the tab and pulse-highlight it. */
  @property({ attribute: false }) pinTab: Tab | null = null;
  /** When `expanded`, the inspector renders as a full-screen view with bigger
   * charts and a wider Witness panel. Used when the rail Inspector/Witness
   * button is clicked — see ADR-093 P1.13. */
  @property({ type: Boolean, reflect: true }) expanded = false;

  static styles = css`
    :host {
      display: flex; flex-direction: column;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      overflow: hidden;
      height: 100%;
    }
    :host([expanded]) {
      border-left: 0;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
    }
    :host([expanded]) .tabs {
      padding: 0 24px;
      background: var(--bg-1);
    }
    :host([expanded]) .tab {
      padding: 16px 22px;
      font-size: 13.5px;
      flex: 0 0 auto;
    }
    :host([expanded]) .body {
      padding: 24px 28px;
      max-width: 1400px;
      width: 100%;
      margin: 0 auto;
    }
    :host([expanded]) .card { padding: 18px 20px; }
    :host([expanded]) .card-h .ttl { font-size: 14px; }
    :host([expanded]) svg { height: 220px; }
    :host([expanded]) .frame-strip { height: 48px; }
    :host([expanded]) table { font-size: 12.5px; }
    :host([expanded]) td { padding: 6px 0; }
    :host([expanded]) .hex { font-size: 12px; padding: 14px; line-height: 1.7; }
    :host([expanded]) .witness-box { font-size: 13px; padding: 14px 16px; line-height: 1.6; }
    :host([expanded]) .verify-btn { padding: 12px; font-size: 13px; }
    :host([expanded]) .grid-2 {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }
    :host([expanded]) .grid-2 > .card { margin-bottom: 0; }
    @media (max-width: 1024px) {
      :host([expanded]) .grid-2 { grid-template-columns: 1fr; }
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

  override willUpdate(changed: PropertyValues): void {
    // Apply parent-driven tab pin during willUpdate so the new tab value
    // participates in this same render pass — avoids the "update after
    // update completed" Lit warning that would fire if we did this in
    // updated().
    if (changed.has('pinTab') && this.pinTab && this.tab !== this.pinTab) {
      this.tab = this.pinTab;
    }
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

  private renderHeader() {
    if (!this.expanded) return '';
    const titles: Record<Tab, string> = {
      signal: 'Signal inspector — live B-vector trace + frame stream',
      frame: 'Frame inspector — MagFrame v1 fields + raw bytes',
      witness: 'Witness panel — SHA-256 determinism gate',
    };
    return html`
      <h1 style="margin: 8px 0 14px; font-size: 20px; letter-spacing: -0.01em;">
        ${titles[this.tab]}
      </h1>
      <p style="margin: 0 0 18px; font-size: 12.5px; color: var(--ink-3); line-height: 1.55; max-width: 780px;">
        ${this.tab === 'signal'
          ? 'Real-time recovered field-vector and frame-stream sparkline. Both update at the running pipeline\'s frame rate. Use the Tunables panel in the sidebar to change f_s, f_mod, dt, and shot-noise behaviour.'
          : this.tab === 'frame'
            ? 'Decoded view of the most recent MagFrame: typed fields plus the raw 60-byte little-endian binary record (magic 0xC51A_6E70).'
            : 'Re-derive the SHA-256 witness for the canonical reference scene (seed=42, N=256) right now in your browser and compare against Proof::EXPECTED_WITNESS_HEX. Same inputs → same hash, byte-for-byte, across every machine and transport.'}
      </p>
    `;
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

    const b = lastB.value;
    const bnT = [b[0] * 1e9, b[1] * 1e9, b[2] * 1e9];
    const hasData = traceX.value.length > 0;

    return html`
      ${!hasData ? html`
        <div class="card" style="text-align:center; padding:18px;">
          <div style="font-size:13px; color:var(--ink-2); line-height:1.55;">
            No frames yet. Press <b>▶ Run</b> in the topbar (or hit <code style="font-family:var(--mono);background:var(--bg-3);padding:1px 5px;border-radius:4px;color:var(--accent);">Space</code>)
            to start the live B-vector trace.
          </div>
        </div>
      ` : ''}
      <div class=${this.expanded ? 'grid-2' : ''}>
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
          ${this.expanded ? html`<div style="display:flex;gap:14px;font-size:12px;font-family:var(--mono);margin-top:8px;">
            <span style="color:oklch(0.78 0.14 70);">x: ${bnT[0].toFixed(3)} nT</span>
            <span style="color:oklch(0.78 0.12 195);">y: ${bnT[1].toFixed(3)} nT</span>
            <span style="color:oklch(0.72 0.18 330);">z: ${bnT[2].toFixed(3)} nT</span>
            <span style="color:var(--accent);margin-left:auto;">|B| ${(bMag.value * 1e9).toFixed(3)} nT</span>
          </div>` : ''}
        </div>

        <div class="card">
          <div class="card-h">
            <span class="ttl">Frame stream</span>
            <span class="badge" id="strip-rate">live</span>
          </div>
          <div class="frame-strip" id="frame-strip">
            ${stripBars.value.map((v) => html`<div class="bar" style=${`height:${Math.max(4, v * 100)}%`}></div>`)}
          </div>
          ${this.expanded ? html`
            <div style="display:flex;gap:24px;font-family:var(--mono);font-size:12px;color:var(--ink-3);margin-top:12px;">
              <span>frames in window: <span style="color:var(--ink);">${stripBars.value.length}</span></span>
              <span>noise floor: <span style="color:var(--ink);">${lastFrame.value ? lastFrame.value.noiseFloorPtSqrtHz.toFixed(2) + ' pT/√Hz' : '—'}</span></span>
            </div>` : ''}
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
      ${!f ? html`
        <div class="card" style="text-align:center; padding:18px;">
          <div style="font-size:13px; color:var(--ink-2); line-height:1.55;">
            No MagFrame to display yet. Start the pipeline (<b>▶ Run</b>) to populate.
          </div>
        </div>
      ` : ''}
      <div class=${this.expanded ? 'grid-2' : ''}>
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
        ${this.expanded ? html`
          <div style="font-size: 11.5px; color: var(--ink-3); margin-top: 10px; line-height: 1.6;">
            Layout (little-endian): <code>magic(u32) version(u16) flags(u16) sensor_id(u16) _reserved(u16) t_us(u64) b_pt[3](f32) sigma_pt[3](f32) noise_floor(f32) temp_K(f32)</code>.
          </div>` : ''}
      </div>
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
    const match = expectedWitness.value && witnessHex.value && expectedWitness.value === witnessHex.value;
    return html`
      ${this.expanded ? html`
        <div style="display:grid;grid-template-columns:repeat(auto-fit, minmax(180px, 1fr));gap:12px;margin-bottom:18px;">
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Reference scene</div>
            <div style="font-family:var(--mono);font-size:14px;color:var(--ink);margin-top:4px;">Proof::REFERENCE</div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">2 dipoles · 1 loop · 1 ferrous · 1 sensor</div>
          </div>
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Seed</div>
            <div style="font-family:var(--mono);font-size:14px;color:var(--accent);margin-top:4px;">0x0000002A</div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">canonical Proof::SEED</div>
          </div>
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Sample count</div>
            <div style="font-family:var(--mono);font-size:14px;color:var(--ink);margin-top:4px;">256</div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">Proof::N_SAMPLES</div>
          </div>
          <div class="card" style="margin:0;">
            <div style="font-size:10px;color:var(--ink-3);text-transform:uppercase;letter-spacing:0.06em;">Status</div>
            <div style="font-family:var(--mono);font-size:14px;margin-top:4px;color:${status === 'ok' ? 'var(--ok)' : status === 'fail' ? 'var(--bad)' : 'var(--ink-3)'};">
              ${status === 'ok' ? '✓ matches' : status === 'fail' ? '✗ drift' : status === 'pending' ? '… running' : '— idle'}
            </div>
            <div style="font-size:11.5px;color:var(--ink-3);margin-top:2px;">${match ? 'byte-equivalent' : 'not yet verified'}</div>
          </div>
        </div>
      ` : ''}
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
      ${this.expanded ? html`
        <div class="card">
          <div class="card-h">
            <span class="ttl">What this verifies</span>
            <span class="badge">ADR-089 §5</span>
          </div>
          <div style="font-size: 12.5px; color: var(--ink-2); line-height: 1.6;">
            <p style="margin: 0 0 10px;">Pressing <b>Verify</b> runs the canonical reference pipeline
              (<code>Proof::generate</code>) end-to-end inside this browser's WASM Worker:
              scene → Biot-Savart synthesis → material attenuation → NV ensemble → ADC + lock-in →
              concatenated <code>MagFrame</code> bytes → SHA-256.</p>
            <p style="margin: 0 0 10px;">If the resulting hash matches the constant pinned at build time
              (<code>cc8de9b01b0ff5bd…</code>), every constant — γ_e, D_GS, μ₀, T₂*, contrast, the PRNG
              stream, the frame layout, the pipeline ordering — is byte-identical to the published
              reference. If it doesn't match, <i>something</i> drifted; the dashboard names which.</p>
            <p style="margin: 0;">This is the same regression test that runs in
              <code>cargo test -p nvsim</code> — running in your browser, against your own WASM build.</p>
          </div>
        </div>
      ` : ''}
    `;
  }

  override render() {
    return html`
      <div class="tabs" role="tablist">
        <button class="tab ${this.tab === 'signal' ? 'active' : ''}" data-pane="signal"
          role="tab" aria-selected=${this.tab === 'signal'}
          @click=${() => this.tab = 'signal'}>Signal</button>
        <button class="tab ${this.tab === 'frame' ? 'active' : ''}" data-pane="frame"
          role="tab" aria-selected=${this.tab === 'frame'}
          @click=${() => this.tab = 'frame'}>Frame</button>
        <button class="tab ${this.tab === 'witness' ? 'active' : ''}" data-pane="witness"
          role="tab" aria-selected=${this.tab === 'witness'}
          @click=${() => this.tab = 'witness'}>Witness</button>
      </div>
      <div class="body" role="tabpanel">
        ${this.renderHeader()}
        ${this.tab === 'signal' ? this.renderSignalTab()
          : this.tab === 'frame' ? this.renderFrameTab()
          : this.renderWitnessTab()}
      </div>
    `;
  }
}
