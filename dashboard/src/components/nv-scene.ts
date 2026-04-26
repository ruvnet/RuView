/* Scene canvas — SVG with draggable sources, NV crystal sensor, field lines, mini ODMR. */
import { LitElement, html, css, svg } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import { lastB, bMag, fps, snr, motionReduced } from '../store/appStore';

interface SceneItem { id: string; x: number; y: number; color: string; name: string; }

@customElement('nv-scene')
export class NvScene extends LitElement {
  @state() private items: SceneItem[] = [
    { id: 'rebar', x: 740, y: 240, color: 'oklch(0.72 0.18 330)', name: 'rebar.steel' },
    { id: 'heart', x: 220, y: 180, color: 'oklch(0.78 0.14 195)', name: 'heart_proxy' },
    { id: 'mains', x: 180, y: 380, color: 'oklch(0.72 0.18 330)', name: 'mains_60Hz' },
    { id: 'door', x: 800, y: 470, color: 'oklch(0.78 0.14 145)', name: 'door.steel' },
  ];
  @state() private dragging: string | null = null;
  @state() private selected: string | null = null;
  private dragOffset = { dx: 0, dy: 0 };

  static styles = css`
    :host {
      display: block; height: 100%; width: 100%;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
      position: relative; overflow: hidden;
      border-bottom: 1px solid var(--line);
    }
    .grid {
      position: absolute; inset: 0;
      background-image:
        linear-gradient(var(--grid) 1px, transparent 1px),
        linear-gradient(90deg, var(--grid) 1px, transparent 1px);
      background-size: 32px 32px;
      pointer-events: none;
      mask-image: radial-gradient(ellipse at center, black 40%, transparent 100%);
    }
    svg { position: absolute; inset: 0; width: 100%; height: 100%; }
    .stat-card {
      background: rgba(13,17,23,0.7);
      backdrop-filter: blur(8px);
      border: 1px solid var(--line);
      border-radius: var(--radius-sm);
      padding: 8px 12px;
      font-size: 11px;
      min-width: 96px;
    }
    [data-theme="light"] .stat-card { background: rgba(255,255,255,0.85); }
    .stat-card .lbl {
      color: var(--ink-3);
      text-transform: uppercase; font-weight: 600; letter-spacing: 0.06em; font-size: 9.5px;
    }
    .stat-card .val { font-family: var(--mono); font-size: 16px; font-weight: 600; margin-top: 2px; }
    .stat-card .val.amber { color: var(--accent); }
    .stat-card .val.cyan { color: var(--accent-2); }
    .stat-card .val.mint { color: var(--accent-4); }
    .scene-readout {
      position: absolute; top: 14px; right: 14px;
      display: flex; gap: 8px; z-index: 5;
    }
    .draggable { cursor: grab; transition: filter 0.15s; }
    .draggable:hover { filter: brightness(1.15) drop-shadow(0 0 6px currentColor); }
    .draggable.dragging { cursor: grabbing; filter: brightness(1.25) drop-shadow(0 0 10px currentColor); }
    .field-line { stroke-dasharray: 4 6; }
    @keyframes dash { to { stroke-dashoffset: -200; } }
    .field-line.anim { animation: dash 4s linear infinite; }
    @keyframes spin {
      0% { transform: rotateY(0) rotateX(8deg); }
      100% { transform: rotateY(360deg) rotateX(8deg); }
    }
    .crystal { transform-origin: center; transform-box: fill-box; }
    .crystal.anim { animation: spin 12s linear infinite; }
    .label {
      font-family: var(--mono); font-size: 11px; fill: var(--ink-2);
      pointer-events: none;
    }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => { lastB.value; bMag.value; fps.value; snr.value; motionReduced.value; this.requestUpdate(); });
    window.addEventListener('pointermove', this.onPointerMove);
    window.addEventListener('pointerup', this.onPointerUp);
  }

  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('pointermove', this.onPointerMove);
    window.removeEventListener('pointerup', this.onPointerUp);
  }

  private onDown = (id: string, e: PointerEvent): void => {
    e.preventDefault();
    this.dragging = id;
    this.selected = id;
    const item = this.items.find((i) => i.id === id);
    if (!item) return;
    const svgEl = this.renderRoot.querySelector('svg') as SVGSVGElement | null;
    if (!svgEl) return;
    const pt = this.toSvg(e, svgEl);
    this.dragOffset = { dx: pt.x - item.x, dy: pt.y - item.y };
  };

  private onPointerMove = (e: PointerEvent): void => {
    if (!this.dragging) return;
    const svgEl = this.renderRoot.querySelector('svg') as SVGSVGElement | null;
    if (!svgEl) return;
    const pt = this.toSvg(e, svgEl);
    this.items = this.items.map((it) =>
      it.id === this.dragging
        ? { ...it, x: pt.x - this.dragOffset.dx, y: pt.y - this.dragOffset.dy }
        : it,
    );
  };

  private onPointerUp = (): void => { this.dragging = null; };

  private toSvg(e: PointerEvent, svgEl: SVGSVGElement): { x: number; y: number } {
    const r = svgEl.getBoundingClientRect();
    const vbX = ((e.clientX - r.left) / r.width) * 1000;
    const vbY = ((e.clientY - r.top) / r.height) * 600;
    return { x: vbX, y: vbY };
  }

  override render() {
    const b = lastB.value;
    const bnT = [b[0] * 1e9, b[1] * 1e9, b[2] * 1e9];
    const bMagNT = bMag.value * 1e9;
    const animClass = motionReduced.value ? '' : 'anim';

    return html`
      <div class="grid"></div>
      <svg viewBox="0 0 1000 600" preserveAspectRatio="xMidYMid meet" id="scene-svg">
        <defs>
          <radialGradient id="g-sensor" cx="50%" cy="50%" r="50%">
            <stop offset="0" stop-color="oklch(0.78 0.14 70)" stop-opacity="0.4"/>
            <stop offset="1" stop-color="oklch(0.78 0.14 70)" stop-opacity="0"/>
          </radialGradient>
          <filter id="glow"><feGaussianBlur stdDeviation="3" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
        </defs>

        <!-- Field lines from each source to sensor -->
        ${this.items.map((it) => svg`
          <line class="field-line ${animClass}" x1=${it.x} y1=${it.y}
            x2="500" y2="320"
            stroke=${it.color} stroke-width="1" stroke-opacity="0.5"/>
        `)}

        <!-- Source primitives -->
        ${this.items.map((it) => svg`
          <g class=${`draggable ${this.dragging === it.id ? 'dragging' : ''} ${this.selected === it.id ? 'selected' : ''}`}
             data-id=${it.id} data-source-id=${it.id}
             transform=${`translate(${it.x.toFixed(0)},${it.y.toFixed(0)})`}
             @pointerdown=${(e: PointerEvent) => this.onDown(it.id, e)}>
            <ellipse cx="0" cy="0" rx="32" ry="22" fill=${it.color} fill-opacity="0.18"
              stroke=${it.color} stroke-width="1.2"/>
            <circle cx="0" cy="0" r="4" fill=${it.color}/>
            <text class="label" x="0" y="40" text-anchor="middle">${it.name}</text>
          </g>
        `)}

        <!-- Sensor (NV diamond) at center -->
        <g id="sensor-g" class="draggable" data-id="sensor" transform="translate(500, 320)">
          <circle cx="0" cy="0" r="46" fill="url(#g-sensor)"/>
          <g class=${`crystal ${animClass}`} stroke="oklch(0.78 0.14 70)" stroke-width="2"
             fill="oklch(0.78 0.14 70 / 0.08)" filter="url(#glow)">
            <polygon points="0,-22 19,-7 12,18 -12,18 -19,-7"/>
          </g>
          <circle cx="0" cy="0" r="3" fill="var(--accent)"/>
          <text class="label" x="0" y="56" text-anchor="middle">
            sensor · 〈111〉 NV
          </text>
          <text class="label" x="0" y="72" text-anchor="middle">
            B_in: <tspan fill="var(--accent)" id="b-in-svg">[${bnT[0].toFixed(2)}, ${bnT[1].toFixed(2)}, ${bnT[2].toFixed(2)}] nT</tspan>
          </text>
        </g>
      </svg>

      <div class="scene-readout">
        <div class="stat-card">
          <div class="lbl">|B|</div>
          <div class="val amber" id="bmag-readout">${bMagNT.toFixed(3)} nT</div>
        </div>
        <div class="stat-card">
          <div class="lbl">FPS</div>
          <div class="val cyan" id="fps-readout">${fps.value > 0 ? Math.round(fps.value) : '—'}</div>
        </div>
        <div class="stat-card">
          <div class="lbl">SNR</div>
          <div class="val mint" id="snr-readout">${snr.value > 0 ? snr.value.toFixed(1) : '—'}</div>
        </div>
      </div>
    `;
  }
}
