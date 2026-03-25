// Keyboard Shortcuts System
// Press '?' to show help overlay, number keys to switch tabs, etc.

export class KeyboardShortcuts {
  constructor(app) {
    this.app = app;
    this.shortcuts = new Map();
    this.helpVisible = false;
    this.enabled = true;
    this.overlay = null;
    this.registerDefaults();
  }

  registerDefaults() {
    this.register('?', 'Show keyboard shortcuts', () => this.toggleHelp());
    this.register('Escape', 'Close overlay / dialog', () => this.closeAll());
    this.register('1', 'Switch to Dashboard tab', () => this.switchTab('dashboard'));
    this.register('2', 'Switch to Hardware tab', () => this.switchTab('hardware'));
    this.register('3', 'Switch to Live Demo tab', () => this.switchTab('demo'));
    this.register('4', 'Switch to Architecture tab', () => this.switchTab('architecture'));
    this.register('5', 'Switch to Performance tab', () => this.switchTab('performance'));
    this.register('6', 'Switch to Applications tab', () => this.switchTab('applications'));
    this.register('7', 'Switch to Sensing tab', () => this.switchTab('sensing'));
    this.register('8', 'Switch to Training tab', () => this.switchTab('training'));
    this.register('p', 'Toggle performance monitor', () => this.togglePerfMonitor());
    this.register('t', 'Toggle dark/light theme', () => this.toggleTheme());
  }

  register(key, description, handler) {
    this.shortcuts.set(key, { description, handler });
  }

  init() {
    document.addEventListener('keydown', (e) => this.handleKeydown(e));
    this.createOverlay();
  }

  handleKeydown(e) {
    if (!this.enabled) return;

    // Ignore when typing in inputs
    const tag = e.target.tagName.toLowerCase();
    if (tag === 'input' || tag === 'textarea' || tag === 'select' || e.target.isContentEditable) {
      if (e.key === 'Escape') {
        e.target.blur();
      }
      return;
    }

    // Ignore modified keys (except shift for '?')
    if (e.ctrlKey || e.altKey || e.metaKey) return;

    const shortcut = this.shortcuts.get(e.key);
    if (shortcut) {
      e.preventDefault();
      shortcut.handler();
    }
  }

  switchTab(tabId) {
    const tabManager = this.app?.getComponent?.('tabManager');
    if (tabManager) {
      tabManager.switchToTab(tabId);
    }
  }

  togglePerfMonitor() {
    const event = new CustomEvent('toggle-perf-monitor');
    document.dispatchEvent(event);
  }

  toggleTheme() {
    const event = new CustomEvent('toggle-theme');
    document.dispatchEvent(event);
  }

  closeAll() {
    if (this.helpVisible) {
      this.hideHelp();
    }
  }

  createOverlay() {
    this.overlay = document.createElement('div');
    this.overlay.className = 'shortcuts-overlay';
    this.overlay.setAttribute('role', 'dialog');
    this.overlay.setAttribute('aria-label', 'Keyboard shortcuts');
    this.overlay.setAttribute('aria-modal', 'true');
    this.overlay.innerHTML = this.buildHelpHTML();
    this.overlay.addEventListener('click', (e) => {
      if (e.target === this.overlay) this.hideHelp();
    });
    document.body.appendChild(this.overlay);
  }

  buildHelpHTML() {
    const groups = [
      {
        title: 'Navigation',
        items: Array.from(this.shortcuts.entries())
          .filter(([key]) => /^[1-8]$/.test(key))
      },
      {
        title: 'Actions',
        items: Array.from(this.shortcuts.entries())
          .filter(([key]) => /^[a-z]$/.test(key))
      },
      {
        title: 'General',
        items: Array.from(this.shortcuts.entries())
          .filter(([key]) => !/^[1-8a-z]$/.test(key))
      }
    ];

    return `
      <div class="shortcuts-panel">
        <div class="shortcuts-header">
          <h2>Keyboard Shortcuts</h2>
          <button class="shortcuts-close" aria-label="Close">&times;</button>
        </div>
        <div class="shortcuts-body">
          ${groups.map(group => `
            <div class="shortcuts-group">
              <h3>${group.title}</h3>
              ${group.items.map(([key, { description }]) => `
                <div class="shortcut-row">
                  <kbd>${this.formatKey(key)}</kbd>
                  <span>${description}</span>
                </div>
              `).join('')}
            </div>
          `).join('')}
        </div>
      </div>
    `;
  }

  formatKey(key) {
    const map = { Escape: 'Esc', '?': '?' };
    return map[key] || key.toUpperCase();
  }

  toggleHelp() {
    this.helpVisible ? this.hideHelp() : this.showHelp();
  }

  showHelp() {
    this.overlay.classList.add('visible');
    this.helpVisible = true;
    // Focus close button
    const closeBtn = this.overlay.querySelector('.shortcuts-close');
    if (closeBtn) {
      closeBtn.onclick = () => this.hideHelp();
      closeBtn.focus();
    }
  }

  hideHelp() {
    this.overlay.classList.remove('visible');
    this.helpVisible = false;
  }

  dispose() {
    if (this.overlay?.parentNode) {
      this.overlay.parentNode.removeChild(this.overlay);
    }
  }
}
