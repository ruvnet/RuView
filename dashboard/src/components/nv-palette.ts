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
    { ico: '+', label: 'New scene…', kbd: '⌘N', run: () => openModal({
      title: 'New scene',
      body: `<p>Build a fresh magnetic scene. The dashboard generates the JSON
        and pushes it to the running pipeline (or you can copy the JSON
        for offline use).</p>
        <label>Name</label>
        <input type="text" id="ns-name" value="custom-scene-${Date.now().toString(36)}" />
        <label>Heart-proxy dipole moment (A·m²)</label>
        <input type="text" id="ns-moment" value="1.0e-6" />
        <label>Distance heart → sensor (m)</label>
        <input type="text" id="ns-distance" value="0.5" />
        <label>Add ferrous distractor at +x = 1 m?</label>
        <select id="ns-ferrous">
          <option value="0">No</option>
          <option value="1" selected>Yes (steel coil, χ=5000)</option>
        </select>
        <label>Add 60 Hz mains-current loop?</label>
        <select id="ns-mains">
          <option value="0">No</option>
          <option value="1" selected>Yes (2 A loop, 5 cm radius, +y = 1 m)</option>
        </select>`,
      buttons: [
        { label: 'Cancel', variant: 'ghost' },
        { label: 'Create', variant: 'primary', onClick: async () => {
          const root = document.querySelector('nv-app')?.shadowRoot?.querySelector('nv-modal')?.shadowRoot;
          if (!root) return;
          const name = (root.querySelector<HTMLInputElement>('#ns-name')?.value ?? 'custom').trim();
          const m = parseFloat(root.querySelector<HTMLInputElement>('#ns-moment')?.value ?? '1e-6');
          const d = parseFloat(root.querySelector<HTMLInputElement>('#ns-distance')?.value ?? '0.5');
          const ferr = root.querySelector<HTMLSelectElement>('#ns-ferrous')?.value === '1';
          const mains = root.querySelector<HTMLSelectElement>('#ns-mains')?.value === '1';
          const scene = {
            dipoles: [{ position: [0, 0, d] as [number, number, number], moment: [0, 0, m] as [number, number, number] }],
            loops: mains ? [{
              centre: [0, 1, 0] as [number, number, number],
              normal: [0, 1, 0] as [number, number, number],
              radius: 0.05, current: 2.0, n_segments: 64,
            }] : [],
            ferrous: ferr ? [{ position: [1, 0, 0] as [number, number, number], volume: 1e-4, susceptibility: 5000 }] : [],
            eddy: [],
            sensors: [[0, 0, 0] as [number, number, number]],
            ambient_field: [1e-6, 0, 0] as [number, number, number],
          };
          await getClient()?.loadScene(scene);
          pushLog('ok', `scene <span class="s">${name}</span> loaded · 1 dipole · ${mains ? '1 loop · ' : ''}${ferr ? '1 ferrous · ' : ''}1 sensor`);
          toast(`Scene "${name}" loaded`, '+');
        } },
      ],
    }) },
    { ico: '📦', label: 'Export proof bundle…', kbd: '⌘E', run: async () => {
      const c = getClient(); if (!c) return;
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
        toast(`Proof bundle saved (${blob.size} B)`, '📦');
      } catch (e) { pushLog('err', `export failed: ${(e as Error).message}`); }
    } },
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
