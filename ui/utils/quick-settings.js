// Quick Settings Panel - Centralized configuration for all UI features
// Accessible via gear icon in header

import { apiService, API_TOKEN_STORAGE_KEY } from '../services/api.service.js';
import { i18n } from './i18n.js';

export class QuickSettings {
  constructor(app) {
    this.app = app;
    this.button = null;
    this.panel = null;
    this.isOpen = false;
  }

  // A stored token is applied at api.service.js module load (before any
  // request fires) — this panel only saves/clears it.
  init() {
    this.createButton();
    this.createPanel();
  }

  createButton() {
    this.button = document.createElement('button');
    this.button.className = 'settings-gear';
    this.button.setAttribute('aria-label', 'Settings');
    this.button.setAttribute('title', 'Quick settings');
    this.button.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>`;

    this.button.addEventListener('click', () => this.toggle());

    const headerInfo = document.querySelector('.header-info');
    if (headerInfo) headerInfo.appendChild(this.button);
  }

  createPanel() {
    this.panel = document.createElement('div');
    this.panel.className = 'quick-settings-panel';
    this.panel.setAttribute('role', 'dialog');
    this.panel.setAttribute('aria-label', 'Quick settings');

    this.panel.innerHTML = `
      <div class="qs-header">
        <h3>Settings</h3>
        <button class="qs-close" aria-label="Close">&times;</button>
      </div>
      <div class="qs-body">
        <div class="qs-section">
          <div class="qs-section-title">Display</div>
          <label class="qs-toggle">
            <span>Reduced motion</span>
            <input type="checkbox" id="qs-reduced-motion" ${this.prefersReducedMotion() ? 'checked' : ''}>
            <span class="qs-switch"></span>
          </label>
          <label class="qs-toggle">
            <span>High contrast</span>
            <input type="checkbox" id="qs-high-contrast">
            <span class="qs-switch"></span>
          </label>
          <label class="qs-toggle">
            <span>Compact mode</span>
            <input type="checkbox" id="qs-compact" ${this.getSetting('compact') ? 'checked' : ''}>
            <span class="qs-switch"></span>
          </label>
        </div>
        <div class="qs-section">
          <div class="qs-section-title">Monitoring</div>
          <label class="qs-toggle">
            <span>Health polling</span>
            <input type="checkbox" id="qs-health-polling" checked>
            <span class="qs-switch"></span>
          </label>
          <label class="qs-toggle">
            <span>Auto-reconnect</span>
            <input type="checkbox" id="qs-auto-reconnect" checked>
            <span class="qs-switch"></span>
          </label>
        </div>
        <div class="qs-section">
          <div class="qs-section-title">Cognitum Account</div>
          <div class="qs-row" style="flex-direction: column; align-items: stretch; gap: 6px;">
            <span id="qs-signin-status" style="font-size: 0.9em; opacity: 0.85;">Checking...</span>
            <div style="display: flex; gap: 8px;">
              <button class="qs-btn" id="qs-signin" hidden>Sign in with Cognitum</button>
              <button class="qs-btn-danger" id="qs-signout" hidden>Sign out</button>
            </div>
          </div>
        </div>
        <div class="qs-section">
          <div class="qs-section-title">API Access</div>
          <div class="qs-row" style="flex-direction: column; align-items: stretch; gap: 6px;">
            <span>Bearer token (set only if the server enforces RUVIEW_API_TOKEN)</span>
            <input type="password" id="qs-api-token" class="qs-text-input" placeholder="Paste token..." autocomplete="off" style="width: 100%; box-sizing: border-box;">
            <div style="display: flex; gap: 8px;">
              <button class="qs-btn" id="qs-api-token-save">Save & Apply</button>
              <button class="qs-btn-danger" id="qs-api-token-clear">Clear</button>
            </div>
            <span id="qs-api-token-status" style="font-size: 0.85em; opacity: 0.75;"></span>
          </div>
        </div>
        <div class="qs-section">
          <div class="qs-section-title">Data</div>
          <div class="qs-row">
            <span>Clear local data</span>
            <button class="qs-btn-danger" id="qs-clear-data">Clear</button>
          </div>
          <div class="qs-row">
            <span>${i18n.t('quickSettings.resetOnboarding')}</span>
            <button class="qs-btn" id="qs-reset-tour">${i18n.t('quickSettings.reset')}</button>
          </div>
        </div>
      </div>
    `;

    // Bind events
    this.panel.querySelector('.qs-close').addEventListener('click', () => this.close());

    // Re-check sign-in state whenever the page could be showing a stale view:
    // `pageshow` fires on a back/forward-cache restore (where no script re-runs
    // and no fetch would otherwise happen), and `visibilitychange` covers
    // signing in or out in another tab. Opening the panel alone is not enough —
    // the panel may already be open, or the page may be restored wholesale.
    window.addEventListener('pageshow', () => { void refreshSignInPanel(this.panel); });
    document.addEventListener('visibilitychange', () => {
      if (!document.hidden) void refreshSignInPanel(this.panel);
    });

    // ADR-271 sign-in. Bound here as well as in refreshSignInPanel so a click
    // works even if the status fetch has not resolved yet.
    this.panel.querySelector('#qs-signin')
      .addEventListener('click', () => { window.location.href = '/oauth/start'; });
    this.panel.querySelector('#qs-signout')
      .addEventListener('click', () => { window.location.href = '/oauth/logout'; });

    this.panel.querySelector('#qs-reduced-motion').addEventListener('change', (e) => {
      document.body.classList.toggle('reduced-motion', e.target.checked);
      this.saveSetting('reduced-motion', e.target.checked);
    });

    this.panel.querySelector('#qs-high-contrast').addEventListener('change', (e) => {
      document.body.classList.toggle('high-contrast', e.target.checked);
      this.saveSetting('high-contrast', e.target.checked);
    });

    this.panel.querySelector('#qs-compact').addEventListener('change', (e) => {
      document.body.classList.toggle('compact-mode', e.target.checked);
      this.saveSetting('compact', e.target.checked);
    });

    this.panel.querySelector('#qs-health-polling').addEventListener('change', (e) => {
      const healthService = this.app?.components?.dashboard?.healthSubscription;
      if (e.target.checked) {
        // Resume would need import - just dispatch event
        document.dispatchEvent(new CustomEvent('health-polling-toggle', { detail: true }));
      } else {
        document.dispatchEvent(new CustomEvent('health-polling-toggle', { detail: false }));
      }
    });

    this.panel.querySelector('#qs-api-token-save').addEventListener('click', () => {
      const input = this.panel.querySelector('#qs-api-token');
      const status = this.panel.querySelector('#qs-api-token-status');
      const token = input.value.trim();
      if (!token) {
        status.textContent = 'Enter a token first, or use Clear to remove one.';
        return;
      }
      try { localStorage.setItem(API_TOKEN_STORAGE_KEY, token); } catch { /* noop */ }
      apiService.setAuthToken(token);
      status.textContent = 'Token saved and applied. Reloading...';
      setTimeout(() => window.location.reload(), 600);
    });

    this.panel.querySelector('#qs-api-token-clear').addEventListener('click', () => {
      const input = this.panel.querySelector('#qs-api-token');
      const status = this.panel.querySelector('#qs-api-token-status');
      try { localStorage.removeItem(API_TOKEN_STORAGE_KEY); } catch { /* noop */ }
      apiService.setAuthToken(null);
      input.value = '';
      status.textContent = 'Token cleared. Reloading...';
      setTimeout(() => window.location.reload(), 600);
    });

    this.panel.querySelector('#qs-clear-data').addEventListener('click', () => {
      try {
        localStorage.clear();
        sessionStorage.clear();
      } catch { /* noop */ }
      this.close();
      window.location.reload();
    });

    this.panel.querySelector('#qs-reset-tour').addEventListener('click', () => {
      try { localStorage.removeItem('ruview-onboarding-done'); } catch { /* noop */ }
      this.close();
      document.dispatchEvent(new CustomEvent('start-onboarding'));
    });

    document.body.appendChild(this.panel);

    // Close on outside click
    document.addEventListener('click', (e) => {
      if (this.isOpen && !this.panel.contains(e.target) && !this.button.contains(e.target)) {
        this.close();
      }
    });

    // Apply saved settings on init
    this.applySavedSettings();
  }

  applySavedSettings() {
    if (this.getSetting('reduced-motion') || this.prefersReducedMotion()) {
      document.body.classList.add('reduced-motion');
      const cb = this.panel.querySelector('#qs-reduced-motion');
      if (cb) cb.checked = true;
    }
    if (this.getSetting('high-contrast')) {
      document.body.classList.add('high-contrast');
      const cb = this.panel.querySelector('#qs-high-contrast');
      if (cb) cb.checked = true;
    }
    if (this.getSetting('compact')) {
      document.body.classList.add('compact-mode');
    }
    const status = this.panel.querySelector('#qs-api-token-status');
    let hasToken = false;
    try { hasToken = !!localStorage.getItem(API_TOKEN_STORAGE_KEY); } catch { /* noop */ }
    if (status) status.textContent = hasToken ? 'A token is currently set.' : 'No token set (auth is off or unnecessary).';
  }

  prefersReducedMotion() {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  toggle() {
    this.isOpen ? this.close() : this.open();
  }

  open() {
    this.isOpen = true;
    this.panel.classList.add('open');
    // Refresh on every open, not once at construction: the session may have
    // been established in another tab, or expired since the page loaded.
    // Fire-and-forget — a failure renders as a message in the panel, and must
    // not stop the panel opening.
    void refreshSignInPanel(this.panel);
  }

  close() {
    this.isOpen = false;
    this.panel.classList.remove('open');
  }

  getSetting(key) {
    try { return JSON.parse(localStorage.getItem(`ruview-setting-${key}`)); }
    catch { return null; }
  }

  saveSetting(key, value) {
    try { localStorage.setItem(`ruview-setting-${key}`, JSON.stringify(value)); }
    catch { /* noop */ }
  }

  dispose() {
    this.button?.remove();
    this.panel?.remove();
  }
}

// ---- Cognitum browser sign-in (ADR-271) -------------------------------------
//
// `/oauth/status` is intentionally UNGATED: a signed-out browser cannot ask a
// gated endpoint whether sign-in is available. It returns capability flags and,
// when a session exists, who it belongs to — never a credential.
//
// Sign-in is a full-page navigation, not fetch(): the server replies 302 to
// auth.cognitum.one, and the browser must follow it and carry the transaction
// cookie. An XHR would follow the redirect invisibly and land nowhere useful.
export async function refreshSignInPanel(root = document) {
  const status = root.querySelector('#qs-signin-status');
  const signIn = root.querySelector('#qs-signin');
  const signOut = root.querySelector('#qs-signout');
  if (!status || !signIn || !signOut) return null;

  let info;
  try {
    const resp = await fetch('/oauth/status', { credentials: 'same-origin' });
    // 404 = a server predating ADR-271. Say so plainly rather than offering a
    // button that will 404.
    if (resp.status === 404) {
      status.textContent = 'This server does not support Cognitum sign-in.';
      signIn.hidden = true;
      signOut.hidden = true;
      return null;
    }
    if (!resp.ok) throw new Error(`status ${resp.status}`);
    info = await resp.json();
  } catch (err) {
    status.textContent = `Could not reach the server (${err.message}).`;
    signIn.hidden = true;
    signOut.hidden = true;
    return null;
  }

  if (info.signed_in) {
    status.textContent = `Signed in${info.account ? ` as ${info.account}` : ''}${
      info.scope ? ` - ${info.scope}` : ''
    }`;
    signIn.hidden = true;
    signOut.hidden = false;
  } else if (info.browser_signin) {
    status.textContent = info.auth_required
      ? 'This server requires sign-in.'
      : 'Optional: sign in to use your Cognitum account.';
    signIn.hidden = false;
    signOut.hidden = true;
  } else if (info.auth_required) {
    // Auth is on but OAuth is not — the static-token panel below is the path.
    status.textContent = 'This server uses a shared API token (see API Access below).';
    signIn.hidden = true;
    signOut.hidden = true;
  } else {
    status.textContent = 'This server does not require sign-in.';
    signIn.hidden = true;
    signOut.hidden = true;
  }

  signIn.onclick = () => { window.location.href = '/oauth/start'; };
  signOut.onclick = () => { window.location.href = '/oauth/logout'; };
  return info;
}
