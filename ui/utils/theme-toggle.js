// Theme Toggle - Manual dark/light mode switch with persistence

export class ThemeToggle {
  constructor() {
    this.button = null;
    this.currentTheme = this.getSavedTheme() || this.getSystemTheme();
  }

  init() {
    this.createButton();
    this.applyTheme(this.currentTheme);
    document.addEventListener('toggle-theme', () => this.toggle());

    // Listen for system theme changes
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
      if (!this.getSavedTheme()) {
        this.applyTheme(e.matches ? 'dark' : 'light');
      }
    });
  }

  createButton() {
    this.button = document.createElement('button');
    this.button.className = 'theme-toggle';
    this.button.setAttribute('aria-label', 'Toggle dark/light theme');
    this.button.setAttribute('title', 'Toggle theme (T)');
    this.updateIcon();
    this.button.addEventListener('click', () => this.toggle());

    // Insert into header
    const headerInfo = document.querySelector('.header-info');
    if (headerInfo) {
      headerInfo.prepend(this.button);
    } else {
      const header = document.querySelector('.header');
      if (header) header.appendChild(this.button);
    }
  }

  toggle() {
    this.currentTheme = this.currentTheme === 'dark' ? 'light' : 'dark';
    this.applyTheme(this.currentTheme);
    this.saveTheme(this.currentTheme);
  }

  applyTheme(theme) {
    this.currentTheme = theme;
    document.documentElement.setAttribute('data-color-scheme', theme);
    this.updateIcon();
  }

  updateIcon() {
    if (!this.button) return;
    const isDark = this.currentTheme === 'dark';
    this.button.innerHTML = isDark
      ? '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>'
      : '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>';
    this.button.setAttribute('aria-label', isDark ? 'Switch to light theme' : 'Switch to dark theme');
  }

  getSystemTheme() {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  getSavedTheme() {
    try {
      return localStorage.getItem('ruview-theme');
    } catch {
      return null;
    }
  }

  saveTheme(theme) {
    try {
      localStorage.setItem('ruview-theme', theme);
    } catch {
      // localStorage not available
    }
  }

  dispose() {
    if (this.button?.parentNode) {
      this.button.parentNode.removeChild(this.button);
    }
  }
}
