/* First-run welcome tour. 5 steps walking the user through the
 * dashboard's main concepts. Persists `seen` flag in IndexedDB so it
 * only shows the first time. ADR-092 §10 Pass 6.
 */

import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { kvGet, kvSet } from '../store/persistence';

interface TourStep {
  title: string;
  body: string;
  cta?: string;
}

const STEPS: TourStep[] = [
  {
    title: 'Welcome to nvsim',
    body: `<p><b>nvsim</b> is an open-source, deterministic forward simulator for
      nitrogen-vacancy diamond magnetometry — a real Rust crate compiled to
      WASM and running in your browser, right now.</p>
      <p>This 30-second tour highlights the four panels you'll use most.</p>`,
    cta: 'Start tour',
  },
  {
    title: '1. Scene canvas',
    body: `<p>The middle panel shows your <b>magnetic scene</b> — sources you can
      drag (rebar, heart proxy, mains hum, ferrous door) and a single NV-diamond
      sensor in the centre. Field lines from each source connect to the sensor
      and animate while the pipeline runs.</p>
      <p>Click <code>2</code> on your keyboard any time to jump to the Frame inspector.</p>`,
  },
  {
    title: '2. Run the pipeline',
    body: `<p>Click the <b>▶ Run</b> button (top-right) to start streaming
      <code>MagFrame</code> records at the digitiser's sample rate. The B-vector
      trace and Frame stream sparkline update live, and the FPS pill in the
      topbar shows the simulator's throughput in kHz.</p>
      <p><kbd>Space</kbd> toggles run/pause from anywhere.</p>`,
  },
  {
    title: '3. Witness panel',
    body: `<p>The <b>Witness</b> tab is the heart of nvsim's determinism contract.
      Click <b>Verify</b> and the pipeline re-derives the SHA-256 over a 256-frame
      reference run and asserts it matches the constant pinned in the Rust crate.</p>
      <p>Same input → same hash → byte-for-byte across browsers, OSes, transports.
      If the hash drifts, your build is non-canonical.</p>`,
  },
  {
    title: '4. App Store',
    body: `<p>The grid icon on the left rail opens the <b>App Store</b> — every
      hot-loadable WASM edge module RuView ships, plus the simulators. 66 apps
      across 13 categories: medical, security, building, retail, industrial,
      signal, learning, autonomy, and more.</p>
      <p>Toggle any card to mark it active in this session; the WS transport
      will push the activation set to a connected ESP32 mesh.</p>`,
  },
  {
    title: 'You are ready',
    body: `<p>Press <kbd>⌘K</kbd> (or <kbd>Ctrl K</kbd>) any time for the command
      palette, <kbd>?</kbd> for the full shortcuts list, or just start clicking.</p>
      <p>Source on GitHub:
      <code>github.com/ruvnet/RuView</code> · ADR-089, ADR-092 · MIT/Apache-2.0.</p>`,
    cta: 'Get started',
  },
];

@customElement('nv-onboarding')
export class NvOnboarding extends LitElement {
  @state() private open = false;
  @state() private step = 0;

  static styles = css`
    :host {
      position: fixed; inset: 0;
      background: rgba(0, 0, 0, 0.55);
      backdrop-filter: blur(4px);
      z-index: 240;
      display: grid; place-items: center;
      opacity: 0; pointer-events: none;
      transition: opacity 0.18s;
    }
    :host([open]) { opacity: 1; pointer-events: auto; }
    .card {
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      width: min(560px, 92vw);
      max-height: 86vh;
      display: flex; flex-direction: column;
      transform: translateY(12px) scale(0.98);
      transition: transform 0.22s cubic-bezier(0.2,0.7,0.3,1);
    }
    :host([open]) .card { transform: translateY(0) scale(1); }
    .h {
      padding: 20px 22px 8px;
      display: flex; justify-content: space-between; align-items: flex-start;
    }
    .h h2 { margin: 0; font-size: 18px; letter-spacing: -0.01em; }
    .body {
      padding: 8px 22px 16px;
      font-size: 13px; color: var(--ink-2); line-height: 1.55;
      overflow-y: auto;
    }
    .body p { margin: 0 0 12px; }
    .body code, .body kbd {
      font-family: var(--mono); font-size: 11.5px;
      padding: 1px 5px; background: var(--bg-3);
      border: 1px solid var(--line); border-radius: 4px;
      color: var(--accent);
    }
    .footer {
      display: flex; align-items: center; gap: 12px;
      padding: 12px 22px;
      border-top: 1px solid var(--line);
    }
    .dots { display: flex; gap: 6px; flex: 1; }
    .dot {
      width: 6px; height: 6px; border-radius: 50%;
      background: var(--bg-3); border: 1px solid var(--line-2);
    }
    .dot.active { background: var(--accent); border-color: var(--accent); }
    button {
      padding: 8px 14px;
      border-radius: 8px;
      font-size: 12.5px; font-weight: 500;
      border: 1px solid var(--line);
      background: var(--bg-2); color: var(--ink);
      cursor: pointer;
      font-family: inherit;
    }
    button.primary {
      background: var(--accent); border-color: var(--accent);
      color: #1a0f00;
    }
    button.ghost { background: transparent; }
    .skip {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px; color: var(--ink-2);
    }
  `;

  override async connectedCallback(): Promise<void> {
    super.connectedCallback();
    window.addEventListener('nv-show-tour', this.show as EventListener);
    const seen = await kvGet<boolean>('onboarding-seen');
    if (!seen) {
      this.open = true;
      this.setAttribute('open', '');
    }
  }
  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('nv-show-tour', this.show as EventListener);
  }

  private show = (): void => {
    this.step = 0;
    this.open = true;
    this.setAttribute('open', '');
  };

  private async dismiss(): Promise<void> {
    this.open = false;
    this.removeAttribute('open');
    await kvSet('onboarding-seen', true);
  }

  private next(): void {
    if (this.step < STEPS.length - 1) this.step++;
    else void this.dismiss();
  }

  private prev(): void {
    if (this.step > 0) this.step--;
  }

  override render() {
    const s = STEPS[this.step];
    return html`
      <div class="card" role="dialog" aria-modal="true" aria-label="Welcome tour">
        <div class="h">
          <h2>${s.title}</h2>
          <button class="skip" @click=${() => this.dismiss()} aria-label="Skip tour">×</button>
        </div>
        <div class="body" .innerHTML=${s.body}></div>
        <div class="footer">
          <div class="dots">
            ${STEPS.map((_, i) => html`<div class="dot ${i === this.step ? 'active' : ''}"></div>`)}
          </div>
          ${this.step > 0
            ? html`<button class="ghost" @click=${() => this.prev()}>Back</button>`
            : ''}
          <button class="primary" @click=${() => this.next()}>
            ${this.step === STEPS.length - 1 ? (s.cta ?? 'Done') : (s.cta ?? 'Next')}
          </button>
        </div>
      </div>
    `;
  }
}
