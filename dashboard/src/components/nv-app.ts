/* Top-level shell: 4-zone grid with rail / topbar / sidebar / scene / inspector / console.
 * View routing is per-rail-button: the central area swaps between
 * `<nv-scene>`, `<nv-app-store>`, etc. */

import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import './nv-rail';
import './nv-topbar';
import './nv-sidebar';
import './nv-scene';
import './nv-inspector';
import './nv-console';
import './nv-app-store';
import './nv-toast';
import './nv-modal';
import './nv-palette';
import './nv-debug-hud';
import './nv-settings-drawer';
import './nv-onboarding';
import './nv-ghost-murmur';
import './nv-help';

export type View = 'scene' | 'apps' | 'inspector' | 'witness' | 'ghost-murmur';

@customElement('nv-app')
export class NvApp extends LitElement {
  @state() private view: View = 'scene';

  static styles = css`
    :host {
      display: block;
      height: 100vh;
      width: 100vw;
      background: var(--bg-0);
    }
    .skip-link {
      position: absolute;
      top: -40px;
      left: 8px;
      padding: 6px 12px;
      background: var(--accent);
      color: #1a0f00;
      border-radius: 6px;
      font-size: 12.5px;
      font-weight: 600;
      text-decoration: none;
      z-index: 1000;
      transition: top 0.15s;
    }
    .skip-link:focus { top: 8px; }
    .app {
      display: grid;
      grid-template-columns: 56px 280px 1fr 340px;
      grid-template-rows: 48px 1fr 220px;
      grid-template-areas:
        'rail topbar topbar topbar'
        'rail sidebar main inspector'
        'rail sidebar console inspector';
      height: 100vh;
      width: 100vw;
    }
    nv-rail { grid-area: rail; }
    nv-topbar { grid-area: topbar; }
    nv-sidebar { grid-area: sidebar; }
    .main { grid-area: main; min-width: 0; min-height: 0; position: relative; overflow: hidden; }
    nv-inspector { grid-area: inspector; }
    nv-console { grid-area: console; min-height: 0; }
    @media (max-width: 1180px) {
      .app {
        grid-template-columns: 56px 1fr 320px;
        grid-template-areas:
          'rail topbar topbar'
          'rail main inspector'
          'rail console console';
      }
      nv-sidebar { display: none; }
    }
    @media (max-width: 860px) {
      .app {
        grid-template-columns: 1fr;
        grid-template-rows: 52px 1fr 200px;
        grid-template-areas:
          'topbar'
          'main'
          'console';
      }
      nv-rail, nv-sidebar, nv-inspector { display: none; }
    }
  `;

  override render() {
    return html`
      <a class="skip-link" href="#main-content"
        @click=${(e: Event) => { e.preventDefault(); const sr = this.shadowRoot; sr?.querySelector<HTMLElement>('.main')?.focus(); }}>
        Skip to main content
      </a>
      <div class="app">
        <nv-rail .view=${this.view} @navigate=${(e: CustomEvent<View>) => (this.view = e.detail)}></nv-rail>
        <nv-topbar></nv-topbar>
        <nv-sidebar></nv-sidebar>
        <main class="main" id="main-content" tabindex="-1" role="main" aria-label="Main view">
          ${this.view === 'apps'
            ? html`<nv-app-store></nv-app-store>`
            : this.view === 'ghost-murmur'
              ? html`<nv-ghost-murmur></nv-ghost-murmur>`
              : this.view === 'inspector'
                ? html`<nv-inspector expanded .pinTab=${'signal'}></nv-inspector>`
                : this.view === 'witness'
                  ? html`<nv-inspector expanded .pinTab=${'witness'}></nv-inspector>`
                  : html`<nv-scene></nv-scene>`}
        </main>
        <nv-inspector
          .pinTab=${this.view === 'inspector' ? 'signal'
            : this.view === 'witness' ? 'witness' : null}>
        </nv-inspector>
        <nv-console></nv-console>
      </div>
      <nv-toast></nv-toast>
      <nv-modal></nv-modal>
      <nv-palette></nv-palette>
      <nv-debug-hud></nv-debug-hud>
      <nv-settings-drawer></nv-settings-drawer>
      <nv-onboarding></nv-onboarding>
      <nv-help></nv-help>
    `;
  }
}
