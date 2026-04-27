/* Scene canvas — SVG with draggable sources, NV crystal sensor, field lines, mini ODMR. */
import { LitElement, html, css, svg } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import { lastB, bMag, fps, snr, motionReduced, running, getClient, speed, pushLog, lastFrame, scenePositions } from '../store/appStore';

interface SceneItem { id: string; x: number; y: number; color: string; name: string; }

@customElement('nv-scene')
export class NvScene extends LitElement {
  @state() private zoom = 1.0;
  @state() private layerVisible = { source: true, field: true, label: true };
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
    .scene-toolbar {
      position: absolute; top: 14px; left: 14px;
      display: flex; gap: 6px; z-index: 5;
      background: rgba(13,17,23,0.85);
      backdrop-filter: blur(8px);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 4px;
    }
    [data-theme="light"] .scene-toolbar { background: rgba(255,255,255,0.85); }
    .scene-toolbar button {
      width: 28px; height: 28px;
      background: transparent;
      border: 1px solid transparent;
      border-radius: 6px;
      color: var(--ink-2);
      cursor: pointer;
      display: grid; place-items: center;
      font-size: 13px;
    }
    .scene-toolbar button:hover { color: var(--ink); background: var(--bg-2); }
    .scene-toolbar button.on { background: var(--bg-3); color: var(--accent); border-color: var(--line-2); }

    .sim-controls {
      position: absolute; bottom: 14px; right: 14px;
      display: flex; gap: 6px; align-items: center;
      background: rgba(13,17,23,0.85);
      backdrop-filter: blur(12px);
      border: 1px solid var(--line-2);
      border-radius: 999px;
      padding: 6px 10px;
      z-index: 5;
    }
    [data-theme="light"] .sim-controls { background: rgba(255,255,255,0.92); }
    .sim-controls .play {
      width: 32px; height: 32px;
      background: var(--accent);
      border: none;
      border-radius: 50%;
      color: #1a0f00;
      cursor: pointer;
      display: grid; place-items: center;
      font-size: 13px;
    }
    .sim-controls .play:hover { filter: brightness(1.08); }
    .sim-controls .step {
      width: 26px; height: 26px;
      border-radius: 6px;
      background: transparent;
      color: var(--ink-2);
      border: 1px solid var(--line);
      cursor: pointer;
      font-size: 11px;
    }
    .sim-controls .step:hover { color: var(--ink); border-color: var(--line-2); }
    .sim-controls .speed {
      font-family: var(--mono); font-size: 11px;
      color: var(--ink-2);
      padding: 0 6px;
      min-width: 36px;
      text-align: center;
      cursor: pointer;
    }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    // Restore drag positions if any are persisted.
    if (scenePositions.value.length > 0) {
      this.items = this.items.map((it) => {
        const saved = scenePositions.value.find((p) => p.id === it.id);
        return saved ? { ...it, x: saved.x, y: saved.y } : it;
      });
    }
    effect(() => {
      lastB.value; bMag.value; fps.value; snr.value; motionReduced.value;
      running.value; speed.value; lastFrame.value;
      this.requestUpdate();
    });
    // Compute SNR from the last frame: |B_pT| / max(σ_pT[k]) per ADR-093 P1.4.
    effect(() => {
      const f = lastFrame.value;
      if (!f) return;
      const bmag = Math.sqrt(f.bPt[0] ** 2 + f.bPt[1] ** 2 + f.bPt[2] ** 2);
      const sigmaMax = Math.max(Math.abs(f.sigmaPt[0]), Math.abs(f.sigmaPt[1]), Math.abs(f.sigmaPt[2]), 0.001);
      const snrVal = bmag / sigmaMax;
      if (Number.isFinite(snrVal)) snr.value = snrVal;
    });
    window.addEventListener('pointermove', this.onPointerMove);
    window.addEventListener('pointerup', this.onPointerUp);
    window.addEventListener('keydown', this.onKey);
  }

  /** Tab cycles selection; arrow keys nudge by 8 px (32 px with Shift);
   * Esc deselects. ADR-093 P2.6. */
  private onKey = (e: KeyboardEvent): void => {
    const target = e.target as HTMLElement | null;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;
    if (!this.selected) {
      if (e.key === 'Tab' && document.activeElement === document.body) {
        e.preventDefault();
        this.selected = this.items[0]?.id ?? null;
      }
      return;
    }
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      const step = e.shiftKey ? 32 : 8;
      const dx = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0;
      const dy = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0;
      this.items = this.items.map((it) =>
        it.id === this.selected
          ? { ...it, x: Math.max(20, Math.min(980, it.x + dx)), y: Math.max(20, Math.min(580, it.y + dy)) }
          : it,
      );
      scenePositions.value = this.items.map(({ id, x, y }) => ({ id, x, y }));
    } else if (e.key === 'Tab') {
      e.preventDefault();
      const idx = this.items.findIndex((it) => it.id === this.selected);
      const next = (idx + (e.shiftKey ? -1 : 1) + this.items.length) % this.items.length;
      this.selected = this.items[next].id;
    } else if (e.key === 'Escape') {
      this.selected = null;
    }
  };

  private async toggleRun(): Promise<void> {
    const c = getClient(); if (!c) return;
    if (running.value) { await c.pause(); running.value = false; }
    else { await c.run(); running.value = true; }
  }
  private async stepFwd(): Promise<void> {
    const c = getClient(); if (!c) return;
    await c.step('fwd', 10);
    pushLog('dbg', 'sim step → +1 frame');
  }
  private async stepBack(): Promise<void> {
    const c = getClient(); if (!c) return;
    await c.step('back', 10);
    pushLog('dbg', 'sim step ← -1 frame');
  }
  private cycleSpeed(): void {
    const speeds = [0.25, 0.5, 1.0, 2.0, 4.0];
    const idx = speeds.indexOf(speed.value);
    speed.value = speeds[(idx + 1) % speeds.length];
  }
  private zoomIn(): void { this.zoom = Math.min(2.5, this.zoom * 1.2); }
  private zoomOut(): void { this.zoom = Math.max(0.5, this.zoom / 1.2); }
  private fitView(): void { this.zoom = 1.0; }
  private toggleLayer(k: 'source' | 'field' | 'label'): void {
    this.layerVisible = { ...this.layerVisible, [k]: !this.layerVisible[k] };
  }

  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('pointermove', this.onPointerMove);
    window.removeEventListener('pointerup', this.onPointerUp);
    window.removeEventListener('keydown', this.onKey);
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

  private onPointerUp = (): void => {
    if (this.dragging) {
      // Persist all positions on drop.
      scenePositions.value = this.items.map(({ id, x, y }) => ({ id, x, y }));
    }
    this.dragging = null;
  };

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

    const vbW = 1000 / this.zoom;
    const vbH = 600 / this.zoom;
    const vbX = (1000 - vbW) / 2;
    const vbY = (600 - vbH) / 2;

    return html`
      <div class="grid"></div>
      <svg viewBox="${vbX.toFixed(1)} ${vbY.toFixed(1)} ${vbW.toFixed(1)} ${vbH.toFixed(1)}"
        preserveAspectRatio="xMidYMid meet" id="scene-svg">
        <defs>
          <radialGradient id="g-sensor" cx="50%" cy="50%" r="50%">
            <stop offset="0" stop-color="oklch(0.78 0.14 70)" stop-opacity="0.4"/>
            <stop offset="1" stop-color="oklch(0.78 0.14 70)" stop-opacity="0"/>
          </radialGradient>
          <filter id="glow"><feGaussianBlur stdDeviation="3" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
        </defs>

        <!-- Field lines from each source to sensor -->
        ${this.layerVisible.field ? this.items.map((it) => svg`
          <line class="field-line ${animClass}" x1=${it.x} y1=${it.y}
            x2="500" y2="320"
            stroke=${it.color} stroke-width="1" stroke-opacity="0.5"/>
        `) : ''}

        <!-- Source primitives -->
        ${this.layerVisible.source ? this.items.map((it) => svg`
          <g class=${`draggable ${this.dragging === it.id ? 'dragging' : ''} ${this.selected === it.id ? 'selected' : ''}`}
             data-id=${it.id} data-source-id=${it.id}
             transform=${`translate(${it.x.toFixed(0)},${it.y.toFixed(0)})`}
             @pointerdown=${(e: PointerEvent) => this.onDown(it.id, e)}>
            <ellipse cx="0" cy="0" rx="32" ry="22" fill=${it.color} fill-opacity="0.18"
              stroke=${it.color} stroke-width="1.2"/>
            <circle cx="0" cy="0" r="4" fill=${it.color}/>
            ${this.layerVisible.label ? svg`<text class="label" x="0" y="40" text-anchor="middle">${it.name}</text>` : ''}
          </g>
        `) : ''}

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

      <div class="scene-toolbar" id="scene-toolbar">
        <button id="zoom-in-btn" title="Zoom in" @click=${this.zoomIn}>+</button>
        <button id="zoom-out-btn" title="Zoom out" @click=${this.zoomOut}>−</button>
        <button id="fit-btn" title="Fit to view" @click=${this.fitView}>⊡</button>
        <button id="layer-source-btn" class=${this.layerVisible.source ? 'on' : ''}
          title="Sources" @click=${() => this.toggleLayer('source')}>●</button>
        <button id="layer-field-btn" class=${this.layerVisible.field ? 'on' : ''}
          title="Field lines" @click=${() => this.toggleLayer('field')}>≈</button>
        <button id="layer-label-btn" class=${this.layerVisible.label ? 'on' : ''}
          title="Labels" @click=${() => this.toggleLayer('label')}>T</button>
      </div>

      <div class="sim-controls" id="sim-controls">
        <button class="step" id="step-back-btn" title="Step back" @click=${this.stepBack}>⏮</button>
        <button class="play" id="play-btn" title="Play / pause" @click=${this.toggleRun}>
          ${running.value ? '❚❚' : '▶'}
        </button>
        <button class="step" id="step-fwd-btn" title="Step forward" @click=${this.stepFwd}>⏭</button>
        <span class="speed" id="speed-val" title="Cycle speed" @click=${this.cycleSpeed}>${speed.value}×</span>
      </div>

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
