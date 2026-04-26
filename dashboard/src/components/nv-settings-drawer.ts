/* Settings drawer — theme / density / motion / auto-update. */
import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import { theme, density, motionReduced, autoUpdate, transport, wsUrl } from '../store/appStore';

@customElement('nv-settings-drawer')
export class NvSettingsDrawer extends LitElement {
  @state() private open = false;

  static styles = css`
    :host {
      position: fixed; top: 0; right: 0; bottom: 0;
      width: 420px; max-width: 100vw;
      background: var(--bg-1);
      border-left: 1px solid var(--line);
      z-index: 51;
      transform: translateX(100%);
      transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1);
      display: flex; flex-direction: column;
      box-shadow: -20px 0 60px -20px rgba(0,0,0,0.5);
    }
    :host([open]) { transform: translateX(0); }
    .scrim {
      position: fixed; inset: 0;
      background: rgba(0,0,0,0.5);
      z-index: 50;
      opacity: 0; pointer-events: none;
      transition: opacity 0.2s;
    }
    :host([open]) .scrim { opacity: 1; pointer-events: auto; }
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

  override render() {
    return html`
      <div class="scrim" @click=${() => this.close()}></div>
      <div class="h">
        <div class="ttl">Settings</div>
        <button class="close" @click=${() => this.close()}>×</button>
      </div>
      <div class="body">
        <div class="group">
          <h4>Appearance</h4>
          <div class="row">
            <div><div class="lbl">Theme</div></div>
            <div class="seg">
              <button class=${theme.value === 'dark' ? 'on' : ''} @click=${() => theme.value = 'dark'}>dark</button>
              <button class=${theme.value === 'light' ? 'on' : ''} @click=${() => theme.value = 'light'}>light</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Density</div>
              <div class="desc">Affects panel padding and font scale.</div>
            </div>
            <div class="seg">
              <button class=${density.value === 'comfy' ? 'on' : ''} @click=${() => density.value = 'comfy'}>comfy</button>
              <button class=${density.value === 'default' ? 'on' : ''} @click=${() => density.value = 'default'}>default</button>
              <button class=${density.value === 'compact' ? 'on' : ''} @click=${() => density.value = 'compact'}>compact</button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="lbl">Reduce motion</div>
              <div class="desc">Disable rotating crystal & field-line animation.</div>
            </div>
            <span class="toggle ${motionReduced.value ? 'on' : ''}"
              @click=${() => motionReduced.value = !motionReduced.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Pipeline</h4>
          <div class="row">
            <div><div class="lbl">Auto-rerun on edit</div>
            <div class="desc">Restart pipeline when scene/config changes.</div></div>
            <span class="toggle ${autoUpdate.value ? 'on' : ''}"
              @click=${() => autoUpdate.value = !autoUpdate.value}></span>
          </div>
        </div>

        <div class="group">
          <h4>Transport</h4>
          <div class="row">
            <div><div class="lbl">Mode</div></div>
            <div class="seg">
              <button class=${transport.value === 'wasm' ? 'on' : ''} @click=${() => transport.value = 'wasm'}>WASM</button>
              <button class=${transport.value === 'ws' ? 'on' : ''} @click=${() => transport.value = 'ws'}>WS</button>
            </div>
          </div>
          ${transport.value === 'ws' ? html`
            <div class="row">
              <div><div class="lbl">WS URL</div></div>
              <input type="text" placeholder="ws://localhost:7878" .value=${wsUrl.value}
                @input=${(e: Event) => wsUrl.value = (e.target as HTMLInputElement).value} />
            </div>` : ''}
        </div>
      </div>
    `;
  }
}
