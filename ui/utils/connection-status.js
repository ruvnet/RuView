// Connection Status Widget - Persistent indicator in header
// Shows WebSocket and API connection state with reconnect button

import { sensingService } from '../services/sensing.service.js';

/** Ask QuickSettings to open on its API Access section (#2099). */
export function openApiAccess(reason) {
  document.dispatchEvent(new CustomEvent('ruview-open-api-access', { detail: { reason } }));
}

export class ConnectionStatus {
  constructor() {
    this.widget = null;
    this._unsub = null;
  }

  init() {
    this.createWidget();
    this.subscribe();
  }

  createWidget() {
    this.widget = document.createElement('div');
    this.widget.className = 'conn-status';
    this.widget.setAttribute('role', 'status');
    this.widget.setAttribute('aria-live', 'polite');
    this.widget.innerHTML = `
      <span class="conn-status-dot"></span>
      <span class="conn-status-label">Connecting</span>
      <button class="conn-status-reconnect" aria-label="Reconnect" title="Reconnect" style="display:none">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
      </button>
    `;

    // #2099: while the server wants a token, the widget is the way to it.
    this.widget.addEventListener('click', (e) => {
      if (sensingService.state !== 'auth-required') return;
      if (e.target.closest('.conn-status-reconnect')) return;
      // QuickSettings closes on any outside click; this click must not count.
      e.stopPropagation();
      openApiAccess(sensingService.authRequired);
    });

    this.widget.querySelector('.conn-status-reconnect').addEventListener('click', () => {
      this.setStatus('reconnecting', 'Reconnecting...');
      sensingService.reconnect?.();
    });

    // Insert into header-info, after theme toggle if present
    const headerInfo = document.querySelector('.header-info');
    if (headerInfo) {
      headerInfo.prepend(this.widget);
    }
  }

  subscribe() {
    this._unsub = sensingService.onStateChange(() => {
      this.update();
    });
    // Initial
    this.update();
  }

  update() {
    const state = sensingService.state;
    const source = sensingService.dataSource;

    if (state === 'connected' || state === 'streaming') {
      const label = source === 'live' ? 'Live' :
                    source === 'server-simulated' ? 'Simulated' :
                    'Connected';
      this.setStatus('connected', label);
    } else if (state === 'connecting' || state === 'reconnecting') {
      this.setStatus('reconnecting', 'Connecting...');
    } else if (state === 'auth-required') {
      this.setStatus('error', 'Token required');
      this.widget.title = 'This server requires an API token. Click to open Settings \u2192 API Access.';
      // Open the panel once per page load; after that the widget stays
      // clickable but does not keep popping the panel up.
      if (!this._openedApiAccess) {
        this._openedApiAccess = true;
        // Deferred so QuickSettings, initialised after this widget in the same
        // synchronous pass, is listening by the time the event fires.
        setTimeout(() => openApiAccess(sensingService.authRequired), 0);
      }
    } else if (state === 'error') {
      this.setStatus('error', 'Error');
    } else {
      this.setStatus('disconnected', 'Offline');
    }
  }

  setStatus(status, label) {
    if (!this.widget) return;
    if (sensingService.state !== 'auth-required') this.widget.removeAttribute('title');
    this.widget.style.cursor = sensingService.state === 'auth-required' ? 'pointer' : '';
    this.widget.className = `conn-status conn-status-${status}`;
    this.widget.querySelector('.conn-status-label').textContent = label;

    const reconnectBtn = this.widget.querySelector('.conn-status-reconnect');
    reconnectBtn.style.display =
      (status === 'disconnected' || status === 'error') ? '' : 'none';
  }

  dispose() {
    if (this._unsub) this._unsub();
    if (this.widget?.parentNode) {
      this.widget.parentNode.removeChild(this.widget);
    }
  }
}
