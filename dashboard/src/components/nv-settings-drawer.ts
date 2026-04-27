/* Settings drawer — theme / density / motion / auto-update. */
import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import { theme, density, motionReduced, autoUpdate, transport, wsUrl } from '../store/appStore';

@customElement('nv-settings-drawer')
export class NvSettingsDrawer extends LitElement {
  @state() private open = false;

  static styles = css`
    /* The host covers the viewport without transforming itself. Only the
     * inner .panel is transformed; otherwise the host's transform would
     * create a containing block for the fixed-position scrim, clipping
     * it to the panel's 420 px width and breaking outside-to-dismiss. */
    :host {
      position: fixed; inset: 0;
      z-index: 51;
      pointer-events: none;
      opacity: 0;
      transition: opacity 0.2s;
    }
    :host([open]) { pointer-events: auto; opacity: 1; }
    .scrim {
      position: absolute; inset: 0;
      background: rgba(0, 0, 0, 0.5);
      backdrop-filter: blur(2px);
    }
    .panel {
      position: absolute;
      top: 0; right: 0; bottom: 0;
      width: 420px; max-width: 100vw;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      transform: translateX(100%);
      transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1);
      display: flex; flex-direction: column;
      box-shadow: -20px 0 60px -20px rgba(0, 0, 0, 0.5);
    }
    :host([open]) .panel { transform: translateX(0); }
    .h {
      padding: 14px 16px;
      border-bottom: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
    }
    .h .ttl { font-size: 14px; font-weight: 600; }
    .body { flex: 1; overflow-y: auto; padding: 16px; }
    .group { margin-bottom: 22px; }
    .group h4 {
      margin: 0 0 10px;
      font-size: 11px; font-weight: 600;
      text-transform: uppercase; letter-spacing: 0.08em;
      color: var(--ink-3);
    }
    .row {
      display: flex; justify-content: space-between; align-items: center;
      padding: 10px 0;
      border-bottom: 1px solid var(--line);
    }
    .row:last-child { border-bottom: 0; }
    .row .lbl { font-size: 13px; }
    .row .desc { font-size: 11.5px; color: var(--ink-3); margin-top: 2px; }
    .row > div:first-child { flex: 1; padding-right: 12px; }
    .seg {
      display: inline-flex;
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: var(--radius-sm);
      padding: 2px;
    }
    .seg button {
      padding: 4px 10px;
      background: transparent; border: none;
      border-radius: 6px;
      font-size: 11.5px; color: var(--ink-3);
      font-family: var(--mono);
      cursor: pointer;
    }
    .seg button.on { background: var(--bg-1); color: var(--ink); }
    .toggle {
      position: relative;
      width: 36px; height: 20px;
      background: var(--bg-3);
      border: 1px solid var(--line-2);
      border-radius: 999px;
      cursor: pointer;
      flex-shrink: 0;
    }
    .toggle::after {
      content: ''; position: absolute;
      top: 2px; left: 2px;
      width: 14px; height: 14px;
      background: var(--ink-3);
      border-radius: 50%;
      transition: transform 0.15s, background 0.15s;
    }
    .toggle.on { background: var(--accent); border-color: var(--accent); }
    .toggle.on::after { background: #1a0f00; transform: translateX(16px); }
    .close {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-2);
    }
    input[type="text"] {
      background: var(--bg-3);
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 6px 10px;
      color: var(--ink); font-family: var(--mono); font-size: 12px;
      outline: none;
    }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => { theme.value; density.value; motionReduced.value; autoUpdate.value; transport.value; wsUrl.value; this.requestUpdate(); });
    window.addEventListener('open-settings', () => { this.open = true; this.setAttribute('open', ''); });
  }

  private close(): void { this.open = false; this.removeAttribute('open'); }

  private async resetPrefs(): Promise<void> {
    if (!confirm('Reset all preferences and IndexedDB state? Reloads the page.')) return;
    try {
      const dbs = await indexedDB.databases?.();
      if (dbs) for (const d of dbs) if (d.name) indexedDB.deleteDatabase(d.name);
    } catch { /* noop */ }
    location.reload();
  }

  override render() {
    return html`
      <div class="scrim" @click=${() => this.close()}></div>
      <div class="panel" role="dialog" aria-modal="true" aria-label="Settings">
      <div class="h">
        <div class="ttl">Settings</div>
        <button class="close" @click=${() => this.close()}>×</button>
      </div>
      <div class="body">
        <div class="group">
          <h4>Appearance</h4>
          <div class="row">
            <div>
              <div class="lbl">Theme</div>
              <div class="desc">Dark is the default; light has higher contrast for daylight work.</div>
            </div>
            <div class="seg">
              <button class=${theme.value === 'dark' ? 'on' : ''}
                @click=${() => theme.value = 'dark'}>dark</button>
              <button class=${theme.value === 'light' ? 'on' : ''}
                @click=${() => theme.value = 'light'}>light</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Density</div>
              <div class="desc">Affects panel padding and font scale (15 / 14 / 13 px). Choose what your eyes prefer.</div>
            </div>
            <div class="seg">
              <button class=${density.value === 'comfy' ? 'on' : ''}
                @click=${() => density.value = 'comfy'}>comfy</button>
              <button class=${density.value === 'default' ? 'on' : ''}
                @click=${() => density.value = 'default'}>default</button>
              <button class=${density.value === 'compact' ? 'on' : ''}
                @click=${() => density.value = 'compact'}>compact</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Reduce motion</div>
              <div class="desc">Stops the rotating diamond, animated field lines, and chart easing. Auto-on if your system has the prefers-reduced-motion preference set.</div>
            </div>
            <span class="toggle ${motionReduced.value ? 'on' : ''}"
              role="switch" aria-checked=${motionReduced.value}
              @click=${() => motionReduced.value = !motionReduced.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Pipeline</h4>
          <div class="row">
            <div>
              <div class="lbl">Auto-rerun on edit</div>
              <div class="desc">When you change a Tunables slider or load a new scene, push the change to the worker without a manual restart.</div>
            </div>
            <span class="toggle ${autoUpdate.value ? 'on' : ''}"
              role="switch" aria-checked=${autoUpdate.value}
              @click=${() => autoUpdate.value = !autoUpdate.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Transport</h4>
          <div class="row">
            <div>
              <div class="lbl">Mode</div>
              <div class="desc">WASM runs nvsim in your browser (default, no server). WS connects to a host-supplied nvsim-server (REST + binary WebSocket); see ADR-092 §6.2.</div>
            </div>
            <div class="seg">
              <button class=${transport.value === 'wasm' ? 'on' : ''}
                @click=${() => transport.value = 'wasm'}>WASM</button>
              <button class=${transport.value === 'ws' ? 'on' : ''}
                @click=${() => transport.value = 'ws'}>WS</button>
            </div>
          </div>
          ${transport.value === 'ws' ? html`
            <div class="row">
              <div>
                <div class="lbl">WS URL</div>
                <div class="desc">Where your nvsim-server is listening. The server defaults to 127.0.0.1:7878.</div>
              </div>
              <input type="text" placeholder="ws://localhost:7878" .value=${wsUrl.value}
                @input=${(e: Event) => wsUrl.value = (e.target as HTMLInputElement).value} />
            </div>` : ''}
        </div>

        <div class="group">
          <h4>Help</h4>
          <div class="row">
            <div>
              <div class="lbl">Open help center</div>
              <div class="desc">Quickstart, glossary, FAQ, and shortcuts. Press <kbd style="font-family:var(--mono);font-size:10.5px;padding:1px 4px;background:var(--bg-3);border:1px solid var(--line);border-radius:3px;">?</kbd> any time.</div>
            </div>
            <button class="seg"
              @click=${() => { this.close(); window.dispatchEvent(new CustomEvent('nv-show-help')); }}
              style="padding:6px 12px;cursor:pointer;background:var(--bg-3);border:1px solid var(--line);border-radius:6px;color:var(--ink);">
              Open
            </button>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Replay welcome tour</div>
              <div class="desc">Re-show the 6-step first-run walkthrough.</div>
            </div>
            <button class="seg"
              @click=${() => { this.close(); window.dispatchEvent(new CustomEvent('nv-show-tour')); }}
              style="padding:6px 12px;cursor:pointer;background:var(--bg-3);border:1px solid var(--line);border-radius:6px;color:var(--ink);">
              Replay
            </button>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Reset all preferences</div>
              <div class="desc">Wipe theme, density, motion, scene drag positions, REPL history, and the onboarding-seen flag.</div>
            </div>
            <button class="seg"
              @click=${() => this.resetPrefs()}
              style="padding:6px 12px;cursor:pointer;background:var(--bg-3);border:1px solid oklch(0.65 0.22 25 / 0.4);border-radius:6px;color:var(--bad);">
              Reset
            </button>
          </div>
        </div>

        <div class="group">
          <h4>About</h4>
          <div class="row" style="border-bottom:0;">
            <div>
              <div class="lbl">nvsim · v0.3.0</div>
              <div class="desc">Open-source NV-diamond simulator. Apache-2.0 OR MIT.<br>
                <a style="color:var(--accent-2); text-decoration:underline dotted; cursor:pointer;"
                  @click=${() => { this.close(); window.dispatchEvent(new CustomEvent('nv-show-help', { detail: { section: 'about' } })); }}>
                  More info →
                </a></div>
            </div>
          </div>
        </div>
      </div>
      </div>
    `;
  }
}
