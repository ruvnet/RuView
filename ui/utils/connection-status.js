// Connection Status Widget - Persistent indicator in header
// Shows WebSocket and API connection state with reconnect button

import { sensingService } from '../services/sensing.service.js';

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
    } else if (state === 'error') {
      this.setStatus('error', 'Error');
    } else {
      this.setStatus('disconnected', 'Offline');
    }
  }

  setStatus(status, label) {
    if (!this.widget) return;
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
