/* Sidebar — Scene panel, NV sensor panel, Tunables, Pipeline diagram. */
import { LitElement, html, css } from 'lit';
import { customElement } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import { fs, fmod, dtMs, noiseEnabled, running } from '../store/appStore';

@customElement('nv-sidebar')
export class NvSidebar extends LitElement {
  static styles = css`
    :host {
      display: flex; flex-direction: column; gap: 14px;
      padding: 14px; overflow-y: auto;
      background: var(--bg-1); border-right: 1px solid var(--line);
    }
    .panel {
      background: var(--bg-2); border: 1px solid var(--line);
      border-radius: var(--radius); padding: 12px;
    }
    .panel-h {
      display: flex; align-items: center; justify-content: space-between;
      font-size: 11px; font-weight: 600; color: var(--ink-3);
      text-transform: uppercase; letter-spacing: 0.08em;
      margin-bottom: 10px;
    }
    .count {
      background: var(--bg-3); color: var(--ink-2);
      padding: 1px 6px; border-radius: 999px;
      font-family: var(--mono); font-size: 10px;
      text-transform: none; letter-spacing: 0;
    }
    .scene-item {
      display: flex; align-items: center; gap: 10px;
      padding: 8px 10px;
      border-radius: var(--radius-sm);
      cursor: pointer;
      transition: background 0.15s;
      border: 1px solid transparent;
    }
    .scene-item:hover { background: var(--bg-3); }
    .scene-item .swatch { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
    .scene-item .name { font-size: 13px; flex: 1; }
    .scene-item .meta { font-family: var(--mono); font-size: 10.5px; color: var(--ink-3); }
    .field-row {
      display: flex; align-items: center; justify-content: space-between;
      padding: 6px 0; font-size: 12.5px;
      border-bottom: 1px solid var(--line);
    }
    .field-row:last-child { border-bottom: 0; }
    .field-row .lbl { color: var(--ink-3); }
    .field-row .val { font-family: var(--mono); color: var(--ink); font-size: 12px; }
    .slider-row { padding: 8px 0; border-bottom: 1px solid var(--line); }
    .slider-row:last-child { border-bottom: 0; padding-bottom: 0; }
    .slider-row .top { display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 12px; }
    .slider-row .top .lbl { color: var(--ink-3); }
    .slider-row .top .val { font-family: var(--mono); color: var(--ink); }
    input[type="range"] {
      -webkit-appearance: none; appearance: none;
      width: 100%; height: 4px;
      background: var(--bg-3); border-radius: 2px; outline: none;
    }
    input[type="range"]::-webkit-slider-thumb {
      -webkit-appearance: none; appearance: none;
      width: 14px; height: 14px; border-radius: 50%;
      background: var(--accent); cursor: pointer;
      border: 2px solid var(--bg-2);
      box-shadow: 0 0 0 1px var(--line-2);
    }
    .pipeline { display: flex; gap: 4px; align-items: center; flex-wrap: wrap; margin-top: 6px; }
    .stage {
      flex: 1; min-width: 50px;
      padding: 4px 6px;
      background: var(--bg-3); border: 1px solid var(--line);
      border-radius: 6px; font-size: 9.5px; text-align: center;
      color: var(--ink-2); font-family: var(--mono);
    }
    .stage.live { border-color: var(--accent-2); color: var(--accent-2); }
    .stage-arrow { color: var(--ink-4); font-size: 10px; }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => { fs.value; fmod.value; dtMs.value; noiseEnabled.value; running.value; this.requestUpdate(); });
  }

  override render() {
    return html`
      <div class="panel">
        <div class="panel-h">Scene <span class="count">4 sources</span></div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.72 0.18 330)"></span>
          <span class="name">rebar.steel.coil</span>
          <span class="meta">χ=5000</span>
        </div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.78 0.14 195)"></span>
          <span class="name">heart_proxy</span>
          <span class="meta">1e-6 A·m²</span>
        </div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.72 0.18 330)"></span>
          <span class="name">mains_60Hz</span>
          <span class="meta">2 A · 60 Hz</span>
        </div>
        <div class="scene-item">
          <span class="swatch" style="background:oklch(0.78 0.14 145)"></span>
          <span class="name">door.steel</span>
          <span class="meta">eddy</span>
        </div>
      </div>

      <div class="panel">
        <div class="panel-h">NV sensor <span class="count">COTS</span></div>
        <div class="field-row"><span class="lbl">V</span><span class="val">1 mm³</span></div>
        <div class="field-row"><span class="lbl">N</span><span class="val">1e12 NV</span></div>
        <div class="field-row"><span class="lbl">C</span><span class="val">0.030</span></div>
        <div class="field-row"><span class="lbl">T₂*</span><span class="val">200 ns</span></div>
        <div class="field-row"><span class="lbl">δB</span><span class="val">1.18 pT/√Hz</span></div>
      </div>

      <div class="panel">
        <div class="panel-h">Tunables</div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Sample rate</span><span class="val">${(fs.value / 1000).toFixed(1)} kHz</span></div>
          <input type="range" min="1000" max="100000" .value=${String(fs.value)}
            @input=${(e: Event) => fs.value = +(e.target as HTMLInputElement).value} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Lockin f_mod</span><span class="val">${(fmod.value / 1000).toFixed(3)} kHz</span></div>
          <input type="range" min="100" max="5000" .value=${String(fmod.value)}
            @input=${(e: Event) => fmod.value = +(e.target as HTMLInputElement).value} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Integration t</span><span class="val">${dtMs.value.toFixed(1)} ms</span></div>
          <input type="range" min="0.1" max="10" step="0.1" .value=${String(dtMs.value)}
            @input=${(e: Event) => dtMs.value = +(e.target as HTMLInputElement).value} />
        </div>
        <div class="slider-row">
          <div class="top"><span class="lbl">Shot noise</span><span class="val">${noiseEnabled.value ? 'ON' : 'OFF'}</span></div>
          <input type="range" min="0" max="1" .value=${noiseEnabled.value ? '1' : '0'}
            @input=${(e: Event) => noiseEnabled.value = (e.target as HTMLInputElement).value === '1'} />
        </div>
      </div>

      <div class="panel">
        <div class="panel-h">Pipeline</div>
        <div class="pipeline">
          <span class="stage ${running.value ? 'live' : ''}">scene</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${running.value ? 'live' : ''}">B-S</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${running.value ? 'live' : ''}">prop</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${running.value ? 'live' : ''}">NV</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${running.value ? 'live' : ''}">ADC</span>
          <span class="stage-arrow">→</span>
          <span class="stage ${running.value ? 'live' : ''}">frame</span>
        </div>
      </div>
    `;
  }
}
