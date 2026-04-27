/* Help center — single dialog covering Quickstart / Glossary / FAQ /
 * Shortcuts. Opened from the topbar `?` button or by pressing `?` on
 * the keyboard. Self-contained, no external content. */

import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';

type Section = 'quickstart' | 'glossary' | 'faq' | 'shortcuts' | 'about';

interface GlossaryItem {
  term: string;
  body: string;
  category: 'physics' | 'rust' | 'ui';
}

const GLOSSARY: GlossaryItem[] = [
  { term: 'NV-diamond', category: 'physics', body: 'Nitrogen-vacancy defect in synthetic diamond. The simulator models a 1 mm³ ensemble (~10¹² centers) addressed by 532 nm pump light + a 2.87 GHz microwave drive. Used as a room-temperature magnetometer with shot-noise floor ~1 pT/√Hz at the published lab record.' },
  { term: 'CW-ODMR', category: 'physics', body: 'Continuously-driven optically-detected magnetic resonance. Sweep the microwave frequency around the NV zero-field splitting (D = 2.87 GHz) and watch the photoluminescence dip when the microwave matches the spin transition. The dip splits with applied magnetic field along each of the four ⟨111⟩ NV axes.' },
  { term: 'MagFrame', category: 'rust', body: 'Fixed-layout 60-byte binary record nvsim emits per (sensor × sample). Magic 0xC51A_6E70, version 1, little-endian. Carries timestamp, recovered B vector (pT), per-axis sigma, noise floor, and flag bits for saturation / shot-noise-disabled / heavy-attenuation.' },
  { term: 'Witness', category: 'rust', body: 'SHA-256 hash over the concatenated MagFrame bytes for a canonical reference run (Proof::REFERENCE_SCENE_JSON @ seed=42, N=256). Same inputs → same hash, byte-for-byte, across runs and machines. The dashboard re-derives it in WASM and compares against Proof::EXPECTED_WITNESS_HEX pinned at build time.' },
  { term: 'Determinism gate', category: 'rust', body: 'A pass/fail check: did this build of nvsim produce the expected witness? If yes → every constant (γ_e, D_GS, μ₀, contrast, T₂*, the PRNG stream, the frame layout, the pipeline ordering) is byte-identical to the published reference. If no → something drifted; the dashboard names which.' },
  { term: 'Lock-in demod', category: 'physics', body: 'Multiply the photoluminescence signal by cos(2π·f_mod·t) and low-pass to recover the slowly-varying B-field component. The simulator emulates a lock-in with output gain 2 and a single-pole IIR LP filter; settable via the Tunables panel (f_mod default 1 kHz).' },
  { term: 'Shot-noise floor', category: 'physics', body: 'δB = 1 / (γ_e · C · √(N · t · T₂*)) — the irreducible quantum noise floor for an NV ensemble. With nvsim defaults (N=10¹², C=0.03, T₂*=200 ns): ≈1.18 pT/√Hz. Toggleable via the Tunables panel for "analytic" runs without noise.' },
  { term: 'Biot-Savart', category: 'physics', body: 'Closed-form magnetic field at a point from a current loop or a magnetic dipole. The Scene panel\'s sources (heart proxy, mains loop, ferrous body, eddy current) all reduce to Biot-Savart-style superpositions over the sensor position.' },
  { term: 'Multistatic fusion', category: 'physics', body: 'Combining evidence from multiple sensors at known geometric configurations. RuView\'s Cramer-Rao-weighted attention over WiFi CSI nodes + 60 GHz radar nodes + (hypothetically) NV nodes; documented in ADR-029 and the Ghost Murmur view.' },
  { term: 'Scene', category: 'ui', body: 'The simulated magnetic environment: a list of sources (dipole, current loop, ferrous body, eddy current) plus one or more sensor positions and an ambient field. The dashboard ships a "rebar-walkby-01" reference scene; click "New scene…" in the command palette (⌘K) to build your own.' },
  { term: 'Tunables', category: 'ui', body: 'Sliders that change the running pipeline\'s digitiser config. Each edit debounces 300 ms, then rebuilds the WASM pipeline with the new f_s / f_mod / dt / shot-noise setting. The frame stream picks up the change without a restart.' },
  { term: 'Transport', category: 'ui', body: 'How the dashboard talks to nvsim. Default is WASM — the simulator runs in a Web Worker right here in your browser, no server. The optional WS transport is REST + binary WebSocket against a host-supplied nvsim-server (see ADR-092 §6.2). Toggle in Settings.' },
  { term: 'App Store', category: 'ui', body: 'Catalog of all 65+ hot-loadable WASM edge modules from wifi-densepose-wasm-edge plus the simulators. Each card carries id / category / status / event IDs; the toggle marks an app active in this session and (in WS mode) pushes the activation to a connected ESP32 mesh.' },
  { term: 'Ghost Murmur', category: 'ui', body: 'Research view that audits the publicly-reported April 2026 CIA NV-diamond heartbeat detector against the open physics literature. Includes a live "Try it yourself" sandbox where you can place a heart dipole at any distance from the sensor and ask: which transport tier would actually detect it?' },
];

const FAQ = [
  {
    q: 'Is this a real simulator or a mockup?',
    a: 'Real. The Rust crate at v2/crates/nvsim is the same code that runs in the browser via WASM. Press <b>Verify witness</b> on the Witness panel — the SHA-256 you see is byte-equivalent to what `cargo test -p nvsim` produces.',
  },
  {
    q: 'Why does my "Recovered |B|" sit much higher than "Predicted |B|" in the Ghost Murmur demo?',
    a: 'The recovered value reads the simulator\'s ADC quantization floor, not the actual magnetic signal. With COTS-default sensor noise (~300 pT/√Hz) and 16-bit ADC at ±10 µT FS, anything below ~1 pT vanishes into ~2 nT of digitization residual. That\'s the lesson — the press claim sits far below this floor at any meaningful range.',
  },
  {
    q: 'Can I run my own scene?',
    a: 'Yes. Press ⌘K to open the command palette and pick "New scene…". You get five fields (name, dipole moment, distance, ferrous toggle, mains toggle); the dashboard builds the JSON and pushes it via <code>client.loadScene()</code>.',
  },
  {
    q: 'Does any of my data leave the browser?',
    a: 'No. WASM mode is local-only — the worker, the WASM binary, and the IndexedDB persistence all live in your browser. The optional WS transport (off by default) talks to a host of your choosing.',
  },
  {
    q: 'What does the witness mismatch (red ✗) mean?',
    a: 'The current build of nvsim produced a SHA-256 that doesn\'t match the constant pinned at compile time. Possible causes: a different Rust toolchain, a dependency version drift, a manual edit to a physics constant, or an honest bug. Audit the diff against ADR-089 §5.',
  },
  {
    q: 'Why are the Inspector / Witness rail buttons there if there\'s already a right-side inspector?',
    a: 'The right-side inspector is the compact live view; the rail buttons open a full-width version with bigger charts, an explainer header, reference-scene metadata cards, and (on Witness) a "what this verifies" panel. Both stay in sync — the right rail is for glancing, the main area is for diving in.',
  },
  {
    q: 'Why is there an "App Store" if this is a magnetometer simulator?',
    a: 'Because nvsim is one tile in a larger sensing platform. The catalog lists every hot-loadable WASM edge module RuView ships — medical, security, building, retail, industrial, signal, learning, autonomy. The simulators (nvsim today, more in future) are first-class entries in the same catalog.',
  },
];

const QUICKSTART = [
  { step: 1, title: 'Hit ▶ Run', body: 'The big amber button in the topbar starts the live frame stream. The pipeline runs ~1.8 kHz on x86_64 WASM, well above the 1 kHz Cortex-A53 acceptance gate.' },
  { step: 2, title: 'Watch the B-vector trace', body: 'The Inspector → Signal tab shows the recovered field per axis updating in real time. The frame strip below it is one bar per ~32-frame batch.' },
  { step: 3, title: 'Verify the witness', body: 'Click the rail Witness button (or REPL: <code>proof.verify</code>). The dashboard re-runs the canonical reference scene and asserts the SHA-256 byte-for-byte.' },
  { step: 4, title: 'Drag a source', body: 'Grab the rebar / heart proxy / mains loop / ferrous door in the scene canvas; positions persist via IndexedDB.' },
  { step: 5, title: 'Tweak the tunables', body: 'Sliders in the left sidebar update the running pipeline (f_s, f_mod, integration time, shot-noise). Changes debounce 300 ms then push to the worker.' },
  { step: 6, title: 'Open the Ghost Murmur view', body: 'The ghost icon in the rail. Move the distance + moment sliders, hit "Run nvsim at this distance" — the live demo runs the real Rust pipeline through WASM and shows which transport tier would actually detect.' },
  { step: 7, title: 'Browse the App Store', body: 'The grid icon. 65+ edge apps: medical, security, building, retail, industrial, signal, learning. Toggle to mark active in this session.' },
];

const SHORTCUTS = [
  { keys: '⌘K  /  Ctrl K', label: 'Command palette' },
  { keys: 'Space', label: 'Play / pause pipeline' },
  { keys: '⌘R  /  Ctrl R', label: 'Reset pipeline (with confirm)' },
  { keys: '⌘,  /  Ctrl ,', label: 'Settings drawer' },
  { keys: '⌘N  /  Ctrl N', label: 'New scene' },
  { keys: '⌘E  /  Ctrl E', label: 'Export proof bundle' },
  { keys: '⌘/  /  Ctrl /', label: 'Toggle theme (dark / light)' },
  { keys: '`', label: 'Toggle debug HUD' },
  { keys: '?', label: 'Open this help center' },
  { keys: '1 · 2 · 3', label: 'Switch inspector tab (Signal / Frame / Witness)' },
  { keys: 'Esc', label: 'Close any modal / palette / drawer' },
  { keys: '/', label: 'Focus the REPL prompt' },
];

@customElement('nv-help')
export class NvHelp extends LitElement {
  @state() private open = false;
  @state() private section: Section = 'quickstart';
  @state() private query = '';

  static styles = css`
    :host {
      position: fixed; inset: 0;
      background: rgba(0, 0, 0, 0.55);
      backdrop-filter: blur(4px);
      z-index: 230;
      display: grid; place-items: center;
      opacity: 0; pointer-events: none;
      transition: opacity 0.18s;
    }
    :host([open]) { opacity: 1; pointer-events: auto; }
    .modal {
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      width: min(880px, 94vw);
      max-height: 86vh;
      display: grid;
      grid-template-columns: 200px 1fr;
      grid-template-rows: auto 1fr auto;
      overflow: hidden;
      transform: translateY(12px) scale(0.98);
      transition: transform 0.22s cubic-bezier(0.2,0.7,0.3,1);
    }
    :host([open]) .modal { transform: translateY(0) scale(1); }
    @media (max-width: 700px) {
      .modal { grid-template-columns: 1fr; grid-template-rows: auto auto 1fr auto; max-height: 92vh; }
      .nav { border-right: 0; border-bottom: 1px solid var(--line); flex-direction: row; overflow-x: auto; }
      .nav button { white-space: nowrap; }
    }
    .h {
      grid-column: 1 / -1;
      padding: 14px 18px;
      border-bottom: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
    }
    .h .ttl { font-size: 15px; font-weight: 600; }
    .nav {
      border-right: 1px solid var(--line);
      padding: 12px 8px;
      display: flex; flex-direction: column; gap: 2px;
      background: var(--bg-1);
    }
    .nav button {
      text-align: left;
      padding: 8px 12px;
      background: transparent;
      border: 1px solid transparent;
      border-radius: 6px;
      color: var(--ink-3);
      font-size: 12.5px;
      cursor: pointer;
      transition: color 0.15s, background 0.15s;
    }
    .nav button:hover { color: var(--ink); background: var(--bg-2); }
    .nav button.on {
      color: var(--ink); background: var(--bg-3);
      border-color: var(--line-2);
    }
    .body {
      padding: 18px 22px;
      overflow-y: auto;
      font-size: 13px;
      color: var(--ink-2);
      line-height: 1.6;
    }
    .body h2 {
      margin: 0 0 8px;
      font-size: 18px;
      color: var(--ink);
      letter-spacing: -0.01em;
    }
    .body .lead {
      color: var(--ink-3);
      font-size: 12.5px;
      margin: 0 0 14px;
    }
    .body p { margin: 0 0 12px; }
    .body code {
      font-family: var(--mono);
      background: var(--bg-3);
      padding: 1px 5px;
      border-radius: 4px;
      font-size: 11.5px;
      color: var(--accent);
    }
    .body kbd {
      font-family: var(--mono);
      padding: 2px 6px;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 4px;
      font-size: 11.5px;
      color: var(--ink);
    }
    .step {
      display: grid;
      grid-template-columns: 32px 1fr;
      gap: 12px;
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .step:last-child { border-bottom: 0; }
    .step .num {
      width: 26px; height: 26px;
      border-radius: 50%;
      background: var(--accent);
      color: #1a0f00;
      font-family: var(--mono);
      font-size: 12.5px;
      font-weight: 700;
      display: grid; place-items: center;
    }
    .step .ttl { color: var(--ink); font-weight: 600; font-size: 13.5px; margin-bottom: 2px; }
    .step .body-text { font-size: 12.5px; color: var(--ink-2); line-height: 1.55; }
    .glossary-search {
      width: 100%;
      padding: 8px 12px;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 6px;
      font-family: var(--mono);
      font-size: 12.5px;
      color: var(--ink);
      outline: none;
      margin-bottom: 14px;
    }
    .glossary-search:focus { border-color: var(--accent); }
    .term {
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .term:last-child { border-bottom: 0; }
    .term .head {
      display: flex; align-items: center; gap: 8px; margin-bottom: 4px;
    }
    .term .name {
      font-family: var(--mono);
      font-size: 13.5px;
      color: var(--accent);
      font-weight: 600;
    }
    .term .badge {
      font-family: var(--mono);
      font-size: 9.5px;
      padding: 1px 6px;
      border-radius: 4px;
      border: 1px solid var(--line);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }
    .term .badge.physics { color: var(--accent-2); border-color: oklch(0.78 0.12 195 / 0.4); }
    .term .badge.rust { color: var(--accent); border-color: oklch(0.78 0.14 70 / 0.4); }
    .term .badge.ui { color: var(--accent-4); border-color: oklch(0.78 0.14 145 / 0.4); }
    .term .body-text {
      font-size: 12.5px;
      color: var(--ink-2);
      line-height: 1.55;
    }
    .faq-item {
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .faq-item:last-child { border-bottom: 0; }
    .faq-item .q {
      color: var(--ink);
      font-weight: 600;
      font-size: 13.5px;
      margin-bottom: 4px;
    }
    .faq-item .a { font-size: 12.5px; color: var(--ink-2); line-height: 1.55; }
    .shortcuts {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: 8px 16px;
      align-items: baseline;
    }
    .f {
      grid-column: 1 / -1;
      padding: 10px 18px;
      border-top: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
      font-size: 11.5px; color: var(--ink-3);
    }
    .close {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-2);
      cursor: pointer;
    }
    .close:hover { color: var(--ink); border-color: var(--line-2); }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener('nv-show-help', this.show as EventListener);
    window.addEventListener('nv-show-help-close', this.closeListener);
    window.addEventListener('keydown', this.onKey);
  }
  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('nv-show-help', this.show as EventListener);
    window.removeEventListener('nv-show-help-close', this.closeListener);
    window.removeEventListener('keydown', this.onKey);
  }
  private closeListener = (): void => this.close();

  private show = (e: Event): void => {
    const detail = (e as CustomEvent).detail as { section?: Section } | undefined;
    if (detail?.section) this.section = detail.section;
    this.open = true;
    this.setAttribute('open', '');
  };
  private close(): void {
    this.open = false;
    this.removeAttribute('open');
  }
  private onKey = (e: KeyboardEvent): void => {
    const target = e.target as HTMLElement | null;
    const isInput = target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA';
    if (e.key === '?' && !isInput && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      this.show(new CustomEvent('nv-show-help'));
    } else if (e.key === 'Escape' && this.open) {
      this.close();
    }
  };

  private filteredGlossary(): GlossaryItem[] {
    if (!this.query.trim()) return GLOSSARY;
    const q = this.query.toLowerCase();
    return GLOSSARY.filter((g) =>
      g.term.toLowerCase().includes(q) || g.body.toLowerCase().includes(q),
    );
  }

  private renderQuickstart() {
    return html`
      <h2>Quickstart</h2>
      <p class="lead">Seven taps to get from "I just opened the dashboard" to "I'm running my own scene with verified determinism."</p>
      <button
        style="display:inline-flex; align-items:center; gap:8px; padding:10px 16px; margin-bottom:14px; background:var(--accent); color:#1a0f00; border:none; border-radius:8px; font-size:13px; font-weight:600; cursor:pointer; font-family:inherit;"
        @click=${() => { window.dispatchEvent(new CustomEvent('nv-show-help-close')); window.dispatchEvent(new CustomEvent('nv-show-tour')); }}>
        ★ Take the interactive 10-step tour
      </button>
      ${QUICKSTART.map((s) => html`
        <div class="step">
          <div class="num">${s.step}</div>
          <div>
            <div class="ttl">${s.title}</div>
            <div class="body-text" .innerHTML=${s.body}></div>
          </div>
        </div>
      `)}
    `;
  }

  private renderGlossary() {
    const items = this.filteredGlossary();
    return html`
      <h2>Glossary</h2>
      <p class="lead">Every piece of jargon in the dashboard, defined in one paragraph each.</p>
      <input class="glossary-search" type="text" placeholder="Search 14 terms…"
        .value=${this.query}
        @input=${(e: Event) => this.query = (e.target as HTMLInputElement).value} />
      ${items.length === 0
        ? html`<p style="color: var(--ink-3);">No terms match.</p>`
        : items.map((g) => html`
            <div class="term">
              <div class="head">
                <span class="name">${g.term}</span>
                <span class="badge ${g.category}">${g.category}</span>
              </div>
              <div class="body-text">${g.body}</div>
            </div>
          `)}
    `;
  }

  private renderFaq() {
    return html`
      <h2>FAQ</h2>
      <p class="lead">The questions I was asked twice in the first week of demos.</p>
      ${FAQ.map((item) => html`
        <div class="faq-item">
          <div class="q">${item.q}</div>
          <div class="a" .innerHTML=${item.a}></div>
        </div>
      `)}
    `;
  }

  private renderShortcuts() {
    return html`
      <h2>Keyboard shortcuts</h2>
      <p class="lead">Everything is reachable without a mouse.</p>
      <div class="shortcuts">
        ${SHORTCUTS.map((s) => html`
          <kbd>${s.keys}</kbd><span>${s.label}</span>
        `)}
      </div>
    `;
  }

  private renderAbout() {
    return html`
      <h2>About this dashboard</h2>
      <p class="lead">What you're looking at, in one screen.</p>
      <p><b>nvsim</b> is a deterministic forward simulator for nitrogen-vacancy diamond magnetometry.
        The Rust crate at <code>v2/crates/nvsim</code> is the source of truth; this dashboard is a
        Vite + Lit single-page app that ships the crate compiled to WebAssembly inside a Web Worker.</p>
      <p>The defining commitment is <b>determinism</b>: same <code>(scene, config, seed)</code> →
        byte-identical SHA-256 witness across browsers, OSes, and transports. Press the
        <kbd>Verify witness</kbd> button on the Witness tab to assert this live.</p>
      <p>The codebase is open source (Apache-2.0 OR MIT). Find it on GitHub:
        <code>github.com/ruvnet/RuView</code>. Decisions are documented in ADRs 089 (nvsim),
        090 (Lindblad extension, conditional), 091 (sub-THz radar research),
        092 (this dashboard), 093 (UX gap analysis).</p>
      <p>This dashboard is one of several RuView demos. Sibling demos at
        <code>github.io/RuView/</code> include the Observatory and Pose Fusion views.</p>
    `;
  }

  override render() {
    return html`
      <div class="modal" role="dialog" aria-modal="true" aria-label="Help center">
        <div class="h">
          <div class="ttl">Help</div>
          <button class="close" aria-label="Close help" @click=${() => this.close()}>×</button>
        </div>
        <nav class="nav" role="tablist" aria-label="Help sections">
          ${(['quickstart', 'glossary', 'faq', 'shortcuts', 'about'] as Section[]).map((s) => html`
            <button class=${this.section === s ? 'on' : ''} role="tab"
              aria-selected=${this.section === s}
              @click=${() => this.section = s}>
              ${s === 'quickstart' ? '🚀 Quickstart'
                : s === 'glossary' ? '📖 Glossary'
                : s === 'faq' ? '? FAQ'
                : s === 'shortcuts' ? '⌨ Shortcuts'
                : 'ℹ About'}
            </button>
          `)}
        </nav>
        <div class="body" role="tabpanel">
          ${this.section === 'quickstart' ? this.renderQuickstart()
            : this.section === 'glossary' ? this.renderGlossary()
            : this.section === 'faq' ? this.renderFaq()
            : this.section === 'shortcuts' ? this.renderShortcuts()
            : this.renderAbout()}
        </div>
        <div class="f">
          <span>Press <kbd style="font-family:var(--mono);font-size:10.5px;padding:1px 4px;background:var(--bg-3);border:1px solid var(--line);border-radius:3px;">?</kbd> any time to reopen</span>
          <span>nvsim · Apache-2.0 OR MIT</span>
        </div>
      </div>
    `;
  }
}

export function showHelp(section?: Section): void {
  window.dispatchEvent(new CustomEvent('nv-show-help', { detail: { section } }));
}
