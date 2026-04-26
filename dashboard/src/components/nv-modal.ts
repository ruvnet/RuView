/* Modal dialog — opened via window.dispatchEvent('nv-modal', { title, body, buttons }). */
import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';

interface ModalButton {
  label: string;
  variant?: 'ghost' | 'primary' | 'danger';
  onClick?: () => void;
}
interface ModalReq {
  title: string;
  body: string;
  buttons?: ModalButton[];
}

@customElement('nv-modal')
export class NvModal extends LitElement {
  @state() private open = false;
  @state() private mTitle = '';
  @state() private mBody = '';
  @state() private buttons: ModalButton[] = [];

  static styles = css`
    :host {
      position: fixed; inset: 0;
      background: rgba(0,0,0,0.55);
      backdrop-filter: blur(4px);
      z-index: 200;
      display: grid; place-items: center;
      opacity: 0; pointer-events: none;
      transition: opacity 0.18s;
    }
    :host([open]) { opacity: 1; pointer-events: auto; }
    .modal {
      background: var(--bg-1);
      border: 1px solid var(--line-2);
      border-radius: var(--radius);
      box-shadow: 0 30px 80px -20px rgba(0,0,0,0.7);
      width: min(520px, 92vw);
      max-height: 86vh;
      display: flex; flex-direction: column;
      transform: translateY(12px) scale(0.98);
      transition: transform 0.22s cubic-bezier(0.2,0.7,0.3,1);
    }
    :host([open]) .modal { transform: translateY(0) scale(1); }
    .h {
      padding: 14px 16px;
      border-bottom: 1px solid var(--line);
      display: flex; align-items: center; justify-content: space-between;
    }
    .h .ttl { font-size: 14px; font-weight: 600; }
    .body { padding: 16px; overflow-y: auto; font-size: 13px; color: var(--ink-2); line-height: 1.55; }
    .f {
      padding: 12px 16px;
      border-top: 1px solid var(--line);
      display: flex; gap: 8px; justify-content: flex-end;
    }
    button {
      padding: 6px 12px;
      border-radius: 8px;
      font-size: 12.5px;
      cursor: pointer;
      font-family: inherit;
      border: 1px solid var(--line);
      background: var(--bg-2); color: var(--ink);
    }
    button.ghost { background: transparent; }
    button.primary { background: var(--accent); border-color: var(--accent); color: #1a0f00; }
    button.danger { background: var(--bad); border-color: var(--bad); color: #fff; }
    .close {
      width: 28px; height: 28px;
      background: transparent; border: 1px solid var(--line);
      border-radius: 6px;
      color: var(--ink-2);
    }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener('nv-modal', this.onModal as EventListener);
    window.addEventListener('keydown', this.onKey);
  }
  override disconnectedCallback(): void {
    super.disconnectedCallback();
    window.removeEventListener('nv-modal', this.onModal as EventListener);
    window.removeEventListener('keydown', this.onKey);
  }

  private onModal = (e: Event): void => {
    const r = (e as CustomEvent).detail as ModalReq;
    this.mTitle = r.title; this.mBody = r.body;
    this.buttons = r.buttons ?? [{ label: 'Close', variant: 'primary' }];
    this.open = true; this.setAttribute('open', '');
  };

  private onKey = (e: KeyboardEvent): void => {
    if (e.key === 'Escape' && this.open) this.close();
  };

  private close(): void { this.open = false; this.removeAttribute('open'); }
  private clickBtn(b: ModalButton): void { b.onClick?.(); this.close(); }

  override render() {
    return html`
      <div class="modal" role="dialog" aria-modal="true">
        <div class="h">
          <div class="ttl">${this.mTitle}</div>
          <button class="close" @click=${() => this.close()}>×</button>
        </div>
        <div class="body" .innerHTML=${this.mBody}></div>
        <div class="f">
          ${this.buttons.map((b) => html`
            <button class=${b.variant ?? ''} @click=${() => this.clickBtn(b)}>${b.label}</button>
          `)}
        </div>
      </div>
    `;
  }
}

export function openModal(req: ModalReq): void {
  window.dispatchEvent(new CustomEvent('nv-modal', { detail: req }));
}
