/* Topbar — breadcrumbs, transport pill, FPS pill, seed pill, controls. */
import { LitElement, html, css } from 'lit';
import { customElement } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import {
  fps, transportLabel, seed, theme, sceneName,
  running, getClient,
} from '../store/appStore';

@customElement('nv-topbar')
export class NvTopbar extends LitElement {
  static styles = css`
    :host {
      display: flex; align-items: center;
      padding: 0 16px; gap: 12px;
      background: var(--bg-1);
      border-bottom: 1px solid var(--line);
      z-index: 10;
    }
    .crumbs { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--ink-3); }
    .crumbs .sep { color: var(--ink-4); }
    .crumbs .cur { color: var(--ink); font-weight: 500; }
    .spacer { flex: 1; }
    .pill {
      display: inline-flex; align-items: center; gap: 6px;
      padding: 5px 10px;
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: 999px;
      font-size: 12px; color: var(--ink-2);
      font-family: var(--mono); font-weight: 500;
    }
    .pill .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 6px var(--ok); animation: pulse 2s infinite; }
    .pill.wasm .dot { background: var(--accent-2); box-shadow: 0 0 6px var(--accent-2); }
    .pill.seed { color: var(--ink-3); }
    .pill.seed b { color: var(--accent); font-weight: 600; }
    button {
      display: inline-flex; align-items: center; gap: 6px;
      padding: 6px 12px;
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: 8px;
      font-size: 12.5px; font-weight: 500; color: var(--ink);
      cursor: pointer;
      transition: all 0.15s;
    }
    button:hover { border-color: var(--line-2); background: var(--bg-3); }
    button.primary { background: var(--accent); border-color: var(--accent); color: #1a0f00; }
    button.primary:hover { filter: brightness(1.08); }
    button.ghost { background: transparent; }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => { fps.value; transportLabel.value; seed.value; theme.value; sceneName.value; running.value; this.requestUpdate(); });
  }

  private async toggleRun(): Promise<void> {
    const c = getClient(); if (!c) return;
    if (running.value) { await c.pause(); running.value = false; }
    else { await c.run(); running.value = true; }
  }
  private async reset(): Promise<void> {
    const c = getClient(); if (!c) return;
    await c.reset();
  }
  private toggleTheme(): void {
    theme.value = theme.value === 'dark' ? 'light' : 'dark';
  }

  override render() {
    const seedHex = seed.value.toString(16).toUpperCase().padStart(8, '0');
    return html`
      <div class="crumbs">
        <span class="home">RuView</span><span class="sep">/</span>
        <span>nvsim</span><span class="sep">/</span>
        <span class="cur" id="scene-name">${sceneName.value}</span>
      </div>
      <div class="spacer"></div>
      <span class="pill" id="fps-pill">
        <span class="dot"></span>
        <span id="fps-val">${fps.value > 0 ? (fps.value / 1000).toFixed(2) + ' kHz' : 'idle'}</span>
      </span>
      <span class="pill wasm" id="transport-pill"><span class="dot"></span>${transportLabel.value}</span>
      <span class="pill seed" id="seed-pill">seed: <b>0x${seedHex}</b></span>
      <button class="ghost" id="theme-btn" title="Toggle theme" @click=${this.toggleTheme}>
        ${theme.value === 'dark' ? '☼' : '☾'}
      </button>
      <button id="reset-btn" @click=${this.reset}>↺ Reset</button>
      <button class="primary" id="run-btn" @click=${this.toggleRun}>
        ${running.value ? '❚❚ Pause' : '▶ Run'}
      </button>
    `;
  }
}
