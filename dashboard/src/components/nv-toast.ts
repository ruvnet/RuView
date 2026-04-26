/* Toast notification — shown briefly via window.dispatchEvent('nv-toast', detail). */
import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';

@customElement('nv-toast')
export class NvToast extends LitElement {
  @state() private visible = false;
  @state() private msg = '';
  @state() private icon = '✓';
  private timer: number | null = null;

  static styles = css`
    :host {
      position: fixed; bottom: 24px; left: 50%;
      transform: translateX(-50%) translateY(80px);
      background: var(--bg-2);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      padding: 10px 14px;
      font-size: 12.5px;
      box-shadow: var(--shadow);
      z-index: 100;
      opacity: 0; pointer-events: none;
      transition: opacity 0.2s, transform 0.2s;
      display: flex; align-items: center; gap: 8px;
    }
    :host([visible]) {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
      pointer-events: auto;
    }
    .icon { color: var(--accent); }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener('nv-toast', this.onToast as EventListener);
  }
  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('nv-toast', this.onToast as EventListener);
  }

  private onToast = (e: Event): void => {
    const detail = (e as CustomEvent).detail as { msg?: string; icon?: string };
    this.msg = detail.msg ?? 'Done';
    this.icon = detail.icon ?? '✓';
    this.visible = true;
    this.setAttribute('visible', '');
    if (this.timer !== null) window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => {
      this.visible = false;
      this.removeAttribute('visible');
    }, 1800);
  };

  override render() {
    return html`<span class="icon">${this.icon}</span><span>${this.msg}</span>`;
  }
}

export function toast(msg: string, icon = '✓'): void {
  window.dispatchEvent(new CustomEvent('nv-toast', { detail: { msg, icon } }));
}
