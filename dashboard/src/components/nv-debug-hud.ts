/* Debug HUD toggled with `. Shows render fps, sim t, frames, |B|, SNR. */
import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { effect } from '@preact/signals-core';
import { fps, framesEmitted, bMag, snr, t as simT } from '../store/appStore';

@customElement('nv-debug-hud')
export class NvDebugHud extends LitElement {
  @state() private open = false;
  @state() private renderFps = 0;
  private lastTs = performance.now();
  private frameCount = 0;
  private rafId = 0;

  static styles = css`
    :host {
      position: fixed; bottom: 8px; right: 8px;
      width: 220px;
      background: rgba(13,17,23,0.85);
      backdrop-filter: blur(8px);
      border: 1px solid var(--line-2);
      border-radius: 8px;
      padding: 8px 10px;
      font-family: var(--mono); font-size: 11px;
      color: var(--ink-2);
      z-index: 99;
      display: none;
      box-shadow: var(--shadow);
    }
    :host([open]) { display: block; }
    .h {
      display: flex; justify-content: space-between;
      font-weight: 600; color: var(--ink);
      margin-bottom: 6px; padding-bottom: 4px;
      border-bottom: 1px solid var(--line);
    }
    .x { cursor: pointer; color: var(--ink-3); }
    .row {
      display: flex; justify-content: space-between;
      padding: 1px 0;
    }
    .k { color: var(--ink-3); }
    .v { color: var(--ink); }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener('keydown', this.onKey);
    effect(() => { fps.value; framesEmitted.value; bMag.value; snr.value; simT.value; this.requestUpdate(); });
    this.tick();
  }
  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('keydown', this.onKey);
    cancelAnimationFrame(this.rafId);
  }

  private onKey = (e: KeyboardEvent): void => {
    if (e.key === '`' && !(e.target as HTMLElement).matches('input, textarea')) {
      this.open = !this.open;
      this.toggleAttribute('open', this.open);
    }
  };

  private tick = (): void => {
    this.rafId = requestAnimationFrame(this.tick);
    const now = performance.now();
    this.frameCount++;
    if (now - this.lastTs >= 500) {
      this.renderFps = (this.frameCount * 1000) / (now - this.lastTs);
      this.frameCount = 0;
      this.lastTs = now;
      this.requestUpdate();
    }
  };

  override render() {
    return html`
      <div class="h"><span>nvsim · debug</span><span class="x" @click=${() => { this.open = false; this.removeAttribute('open'); }}>✕</span></div>
      <div class="row"><span class="k">render fps</span><span class="v">${this.renderFps.toFixed(1)}</span></div>
      <div class="row"><span class="k">sim fps</span><span class="v">${fps.value > 0 ? Math.round(fps.value) : '—'}</span></div>
      <div class="row"><span class="k">frames</span><span class="v">${framesEmitted.value.toString()}</span></div>
      <div class="row"><span class="k">|B|</span><span class="v">${(bMag.value * 1e9).toFixed(3)} nT</span></div>
      <div class="row"><span class="k">SNR</span><span class="v">${snr.value > 0 ? snr.value.toFixed(1) : '—'}</span></div>
      <div class="row"><span class="k">DOM</span><span class="v">${document.querySelectorAll('*').length}</span></div>
    `;
  }
}
