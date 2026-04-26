/* Command palette ⌘K. */
import { LitElement, html, css } from 'lit';
import { customElement, state, query } from 'lit/decorators.js';
import { toast } from './nv-toast';
import { openModal } from './nv-modal';
import {
  getClient, theme, expectedWitness, witnessHex, witnessVerified, pushLog, running,
} from '../store/appStore';

interface Cmd { ico: string; label: string; kbd?: string; run: () => void; }

@customElement('nv-palette')
export class NvPalette extends LitElement {
  @state() private open = false;
  @state() private filter = '';
  @state() private idx = 0;
  @query('#palette-input') private inputEl!: HTMLInputElement;

  static styles = css`
    :host {
      position: fixed; inset: 0; z-index: 220;
      background: rgba(0,0,0,0.5);
      opacity: 0; pointer-events: none;
      transition: opacity 0.15s;
      display: flex; justify-content: center; padding-top: 12vh;
      backdrop-filter: blur(4px);
    }
    :host([open]) { opacity: 1; pointer-events: auto; }
    .palette {
      width: min(560px, 92vw);
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      overflow: hidden;
      display: flex; flex-direction: column;
      max-height: 60vh;
    }
    .input {
      padding: 14px 16px;
      border-bottom: 1px solid var(--line);
    }
    input {
      width: 100%;
      background: transparent; border: none; outline: none;
      color: var(--ink); font-size: 14px;
      font-family: inherit;
    }
    .list { flex: 1; overflow-y: auto; padding: 4px; }
    .item {
      display: flex; align-items: center; gap: 10px;
      padding: 8px 12px;
      border-radius: 6px;
      cursor: pointer;
      font-size: 12.5px;
    }
    .item.active { background: var(--bg-3); }
    .item .ico { width: 20px; text-align: center; color: var(--accent); }
    .item .lbl { flex: 1; }
    .item .kbd {
      font-family: var(--mono); font-size: 10.5px;
      color: var(--ink-3);
      padding: 1px 5px; background: var(--bg-3); border-radius: 4px;
    }
  `;

  private cmds: Cmd[] = [
    { ico: '▶', label: 'Run pipeline', kbd: 'Space', run: async () => { await getClient()?.run(); running.value = true; toast('Pipeline running', '▶'); } },
    { ico: '❚', label: 'Pause pipeline', run: async () => { await getClient()?.pause(); running.value = false; toast('Paused', '❚❚'); } },
    { ico: '⟳', label: 'Reset pipeline', kbd: '⌘R', run: () => openModal({
      title: 'Reset pipeline?',
      body: '<p>Clears the frame stream and rewinds <code>t</code> to 0.</p>',
      buttons: [
        { label: 'Cancel', variant: 'ghost' },
        { label: 'Reset', variant: 'danger', onClick: async () => { await getClient()?.reset(); pushLog('warn', 'pipeline reset · t=0'); toast('Pipeline reset', '⟳'); } },
      ],
    }) },
    { ico: '✓', label: 'Verify witness', run: async () => {
      const c = getClient(); if (!c) return;
      witnessVerified.value = 'pending';
      const exp = expectedWitness.value;
      const eb = new Uint8Array(32);
      for (let i = 0; i < 32; i++) eb[i] = parseInt(exp.slice(i * 2, i * 2 + 2), 16);
      const r = await c.verifyWitness(eb);
      if (r.ok) { witnessVerified.value = 'ok'; witnessHex.value = exp; toast('Witness verified', '✓'); }
      else { witnessVerified.value = 'fail'; toast('Witness mismatch!', '✗'); }
    } },
    { ico: '☼', label: 'Toggle theme', kbd: '⌘/', run: () => { theme.value = theme.value === 'dark' ? 'light' : 'dark'; } },
    { ico: '⚙', label: 'Open settings', kbd: '⌘,', run: () => window.dispatchEvent(new CustomEvent('open-settings')) },
    { ico: '?', label: 'Keyboard shortcuts…', run: () => openModal({
      title: 'Keyboard shortcuts',
      body: `<div style="display:grid;grid-template-columns:auto 1fr;gap:6px 16px;font-size:13px;">
        <div><code>⌘K / Ctrl K</code></div><div>Command palette</div>
        <div><code>Space</code></div><div>Play / pause</div>
        <div><code>⌘R</code></div><div>Reset</div>
        <div><code>⌘,</code></div><div>Settings</div>
        <div><code>⌘/</code></div><div>Toggle theme</div>
        <div><code>\`</code></div><div>Debug HUD</div>
        <div><code>1 · 2 · 3</code></div><div>Inspector tabs</div>
        <div><code>Esc</code></div><div>Close modal/palette</div>
        <div><code>/</code></div><div>Focus REPL</div>
      </div>`,
      buttons: [{ label: 'Close', variant: 'primary' }],
    }) },
    { ico: 'i', label: 'About nvsim…', run: () => openModal({
      title: 'About nvsim',
      body: `<p><b>nvsim</b> is a deterministic, byte-reproducible forward simulator for nitrogen-vacancy diamond magnetometry.</p>
        <p>This dashboard runs nvsim as WASM in a Web Worker. Same <code>(scene, config, seed)</code> → byte-identical SHA-256 witness across runs and machines.</p>
        <p>License: MIT OR Apache-2.0 · See ADR-089, ADR-092.</p>`,
      buttons: [{ label: 'Close', variant: 'primary' }],
    }) },
  ];

  override connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener('keydown', this.onKey);
    window.addEventListener('nv-palette', this.onOpen as EventListener);
  }
  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('keydown', this.onKey);
    window.removeEventListener('nv-palette', this.onOpen as EventListener);
  }

  private onKey = (e: KeyboardEvent): void => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      this.openPal();
    } else if (e.key === 'Escape' && this.open) {
      this.closePal();
    } else if (this.open) {
      if (e.key === 'ArrowDown') { this.idx = Math.min(this.cmds.length - 1, this.idx + 1); e.preventDefault(); }
      else if (e.key === 'ArrowUp') { this.idx = Math.max(0, this.idx - 1); e.preventDefault(); }
      else if (e.key === 'Enter') { this.runIdx(); e.preventDefault(); }
    }
  };

  private onOpen = (): void => this.openPal();

  private openPal(): void {
    this.open = true; this.setAttribute('open', '');
    this.filter = ''; this.idx = 0;
    setTimeout(() => this.inputEl?.focus(), 0);
  }
  private closePal(): void { this.open = false; this.removeAttribute('open'); }

  private filtered(): Cmd[] {
    if (!this.filter.trim()) return this.cmds;
    const q = this.filter.toLowerCase();
    return this.cmds.filter((c) => c.label.toLowerCase().includes(q));
  }

  private runIdx(): void {
    const f = this.filtered();
    const c = f[this.idx];
    if (c) { c.run(); this.closePal(); }
  }

  override render() {
    const items = this.filtered();
    return html`
      <div class="palette" data-id="palette">
        <div class="input">
          <input id="palette-input" type="text" placeholder="Type a command…"
            .value=${this.filter}
            @input=${(e: Event) => { this.filter = (e.target as HTMLInputElement).value; this.idx = 0; }} />
        </div>
        <div class="list">
          ${items.map((c, i) => html`
            <div class="item ${i === this.idx ? 'active' : ''}" @click=${() => { this.idx = i; this.runIdx(); }}>
              <span class="ico">${c.ico}</span>
              <span class="lbl">${c.label}</span>
              ${c.kbd ? html`<span class="kbd">${c.kbd}</span>` : ''}
            </div>
          `)}
        </div>
      </div>
    `;
  }
}
