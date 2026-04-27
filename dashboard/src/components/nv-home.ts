/* Home view — friendly landing surface for new users.
 *
 * The full-power scene + sidebar + inspector + console are intentionally
 * dense; that's the operator surface. Home is for first-time visitors:
 * a single hero CTA, four quick-jump action cards, and a 1-paragraph
 * explanation of what this dashboard is. No jargon above the fold.
 */

import { LitElement, html, css } from 'lit';
import { customElement } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import { running, getClient, witnessVerified, fps, pushLog } from '../store/appStore';

export type Action = 'scene' | 'apps' | 'witness' | 'ghost-murmur' | 'help' | 'tour';

@customElement('nv-home')
export class NvHome extends LitElement {
  static styles = css`
    :host {
      display: block;
      height: 100%;
      overflow-y: auto;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
      padding: 28px clamp(16px, 6vw, 56px) 60px;
    }
    .hero {
      max-width: 800px;
      margin: 16px auto 28px;
      text-align: center;
    }
    .hero .icon {
      width: 56px; height: 56px;
      margin: 0 auto 18px;
      border-radius: 14px;
      background: linear-gradient(135deg, oklch(0.78 0.14 70) 0%, oklch(0.55 0.16 30) 100%);
      display: grid; place-items: center;
      font-family: var(--mono);
      font-weight: 700;
      font-size: 18px;
      color: #1a0f00;
      box-shadow: 0 8px 24px -6px oklch(0.55 0.16 30 / 0.4);
    }
    .hero h1 {
      margin: 0 0 8px;
      font-size: clamp(24px, 4vw, 34px);
      letter-spacing: -0.02em;
      color: var(--ink);
      line-height: 1.15;
    }
    .hero .tag {
      font-size: clamp(13px, 1.6vw, 15px);
      color: var(--ink-2);
      margin: 0 0 22px;
      line-height: 1.55;
    }
    .hero .ctas {
      display: flex; flex-wrap: wrap; gap: 8px;
      justify-content: center;
    }
    .cta {
      padding: 11px 20px;
      border-radius: 10px;
      font-size: 14px;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
      border: 1px solid var(--line);
      background: var(--bg-2);
      color: var(--ink);
      transition: transform 0.12s, border-color 0.12s, filter 0.12s;
    }
    .cta:hover { transform: translateY(-1px); border-color: var(--line-2); }
    .cta.primary {
      background: var(--accent);
      border-color: var(--accent);
      color: #1a0f00;
    }
    .cta.primary:hover { filter: brightness(1.08); }
    .status {
      display: inline-flex; align-items: center; gap: 8px;
      padding: 6px 12px;
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: 999px;
      font-size: 12px;
      font-family: var(--mono);
      color: var(--ink-2);
      margin-top: 18px;
    }
    .status .dot {
      width: 8px; height: 8px; border-radius: 50%;
      background: var(--ink-3);
    }
    .status.live .dot {
      background: var(--ok);
      box-shadow: 0 0 8px var(--ok);
      animation: pulse 2s infinite;
    }
    @keyframes pulse { 50% { opacity: 0.5; } }

    .grid {
      max-width: 980px;
      margin: 36px auto 0;
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 14px;
    }
    .card {
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      padding: 18px 20px;
      cursor: pointer;
      transition: transform 0.12s, border-color 0.12s, background 0.12s;
      display: flex; flex-direction: column; gap: 6px;
      text-align: left;
      color: inherit;
    }
    .card:hover {
      transform: translateY(-2px);
      border-color: var(--accent);
      background: linear-gradient(180deg, var(--bg-2) 0%, oklch(0.78 0.14 70 / 0.04) 100%);
    }
    .card .ico {
      font-size: 22px;
      line-height: 1;
      margin-bottom: 4px;
    }
    .card h3 {
      margin: 0;
      font-size: 14.5px;
      font-weight: 600;
      color: var(--ink);
      letter-spacing: -0.01em;
    }
    .card p {
      margin: 0;
      font-size: 12.5px;
      color: var(--ink-2);
      line-height: 1.55;
    }
    .card .arrow {
      color: var(--accent);
      font-family: var(--mono);
      font-size: 11.5px;
      margin-top: 6px;
    }

    .footnote {
      max-width: 800px;
      margin: 36px auto 0;
      text-align: center;
      font-size: 12px;
      color: var(--ink-3);
      line-height: 1.55;
    }
    .footnote code {
      font-family: var(--mono);
      background: var(--bg-3);
      padding: 1px 5px;
      border-radius: 4px;
      color: var(--accent);
      font-size: 11px;
    }
    .footnote a {
      color: var(--accent-2);
      text-decoration: underline dotted;
      cursor: pointer;
    }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => { running.value; witnessVerified.value; fps.value; this.requestUpdate(); });
  }

  private go(action: Action): void {
    if (action === 'tour') { window.dispatchEvent(new CustomEvent('nv-show-tour')); return; }
    if (action === 'help') { window.dispatchEvent(new CustomEvent('nv-show-help')); return; }
    this.dispatchEvent(new CustomEvent('navigate', { detail: action, bubbles: true, composed: true }));
  }

  private async runDemo(): Promise<void> {
    const c = getClient(); if (!c) return;
    if (running.value) return;
    await c.run();
    running.value = true;
    pushLog('ok', 'demo started · streaming MagFrames');
  }

  override render() {
    const isRunning = running.value;
    const wasVerified = witnessVerified.value === 'ok';
    return html`
      <div class="hero">
        <div class="icon" aria-hidden="true">NV</div>
        <h1>An open-source quantum-magnetometer simulator, in your browser.</h1>
        <p class="tag">
          nvsim runs a real Rust simulator (the same code that
          <code style="font-family:var(--mono); background:var(--bg-3); padding:1px 5px; border-radius:4px; color:var(--accent); font-size:12px;">cargo&nbsp;test</code>
          uses) entirely in WebAssembly. No server, no upload, no telemetry.
          Press the button to start the live magnetic-field simulation, or
          take the 60-second tour first.
        </p>
        <div class="ctas">
          <button class="cta primary" id="home-run-btn" @click=${() => this.runDemo()}>
            ${isRunning ? '✓ Demo running' : '▶ Run the simulation'}
          </button>
          <button class="cta" id="home-tour-btn" @click=${() => this.go('tour')}>
            ★ Take the 60-second tour
          </button>
          <button class="cta" id="home-help-btn" @click=${() => this.go('help')}>
            ? Help center
          </button>
        </div>
        <div class="status ${isRunning ? 'live' : ''}">
          <span class="dot"></span>
          ${isRunning
            ? html`Live · ${fps.value > 0 ? (fps.value / 1000).toFixed(2) + ' kHz' : 'starting…'}${wasVerified ? ' · witness verified ✓' : ''}`
            : html`Idle${wasVerified ? ' · witness verified ✓' : ''}`}
        </div>
      </div>

      <div class="grid">
        <div class="card" tabindex="0" role="button"
          @click=${() => this.go('scene')}
          @keydown=${(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); this.go('scene'); } }}>
          <div class="ico">🌐</div>
          <h3>Live scene</h3>
          <p>Drag magnetic sources, watch the recovered field update in real time, and tweak sample rate / noise / integration.</p>
          <div class="arrow">Open scene →</div>
        </div>

        <div class="card" tabindex="0" role="button"
          @click=${() => this.go('apps')}
          @keydown=${(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); this.go('apps'); } }}>
          <div class="ico">🛍</div>
          <h3>App Store · 66 edge apps</h3>
          <p>Browse 65 hot-loadable WASM sensing modules across medical, security, building, retail, industrial, learning. Six run live in the browser.</p>
          <div class="arrow">Browse the catalogue →</div>
        </div>

        <div class="card" tabindex="0" role="button"
          @click=${() => this.go('witness')}
          @keydown=${(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); this.go('witness'); } }}>
          <div class="ico">✓</div>
          <h3>Determinism gate</h3>
          <p>Re-derive the SHA-256 witness for the canonical reference scene right here in your browser. Same inputs → same hash, every time.</p>
          <div class="arrow">Verify the witness →</div>
        </div>

        <div class="card" tabindex="0" role="button"
          @click=${() => this.go('ghost-murmur')}
          @keydown=${(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); this.go('ghost-murmur'); } }}>
          <div class="ico">👻</div>
          <h3>Ghost Murmur reality check</h3>
          <p>Audit the publicly-reported April 2026 CIA NV-diamond program against published physics. Live distance/moment sliders.</p>
          <div class="arrow">Read the spec →</div>
        </div>
      </div>

      <p class="footnote">
        New here? <a @click=${() => this.go('tour')}>Take the 60-second guided tour</a>
        — every panel is explained. Or press <code>?</code> for the help center
        (quickstart, glossary, FAQ, shortcuts) any time.<br>
        Open source · Apache-2.0 OR MIT · <code>github.com/ruvnet/RuView</code>
      </p>
    `;
  }
}
