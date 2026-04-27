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

export type View = 'scene' | 'apps' | 'settings';

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
      <div class="app">
        <nv-rail .view=${this.view} @navigate=${(e: CustomEvent<View>) => (this.view = e.detail)}></nv-rail>
        <nv-topbar></nv-topbar>
        <nv-sidebar></nv-sidebar>
        <div class="main">
          ${this.view === 'apps' ? html`<nv-app-store></nv-app-store>` : html`<nv-scene></nv-scene>`}
        </div>
        <nv-inspector></nv-inspector>
        <nv-console></nv-console>
      </div>
      <nv-toast></nv-toast>
      <nv-modal></nv-modal>
      <nv-palette></nv-palette>
      <nv-debug-hud></nv-debug-hud>
      <nv-settings-drawer></nv-settings-drawer>
      <nv-onboarding></nv-onboarding>
    `;
  }
}
