// Command Palette - Ctrl+K / Cmd+K to search and execute commands
// Fuzzy search across tabs, actions, and settings

export class CommandPalette {
  constructor(app) {
    this.app = app;
    this.overlay = null;
    this.input = null;
    this.results = null;
    this.visible = false;
    this.commands = [];
    this.selectedIndex = 0;
    this.filteredCommands = [];
  }

  init() {
    this.registerCommands();
    this.createDOM();
    this.bindGlobalShortcut();
  }

  registerCommands() {
    // Navigation commands
    const tabs = [
      { id: 'dashboard', label: 'Dashboard', icon: 'grid' },
      { id: 'hardware', label: 'Hardware', icon: 'cpu' },
      { id: 'demo', label: 'Live Demo', icon: 'play' },
      { id: 'architecture', label: 'Architecture', icon: 'layers' },
      { id: 'performance', label: 'Performance', icon: 'zap' },
      { id: 'applications', label: 'Applications', icon: 'box' },
      { id: 'sensing', label: 'Sensing', icon: 'wifi' },
      { id: 'training', label: 'Training', icon: 'database' },
    ];

    tabs.forEach(tab => {
      this.commands.push({
        category: 'Navigation',
        label: `Go to ${tab.label}`,
        keywords: [tab.id, tab.label.toLowerCase()],
        icon: tab.icon,
        action: () => {
          const tm = this.app?.getComponent?.('tabManager');
          if (tm) tm.switchToTab(tab.id);
        }
      });
    });

    // External pages
    this.commands.push({
      category: 'Navigation',
      label: 'Open Pose Fusion',
      keywords: ['pose', 'fusion', 'camera'],
      icon: 'external',
      action: () => { window.location.href = 'pose-fusion.html'; }
    });
    this.commands.push({
      category: 'Navigation',
      label: 'Open Observatory',
      keywords: ['observatory', '3d', 'signal'],
      icon: 'external',
      action: () => { window.location.href = 'observatory.html'; }
    });

    // Actions
    this.commands.push({
      category: 'Actions',
      label: 'Toggle Dark/Light Theme',
      keywords: ['theme', 'dark', 'light', 'mode', 'color'],
      icon: 'moon',
      action: () => document.dispatchEvent(new CustomEvent('toggle-theme'))
    });
    this.commands.push({
      category: 'Actions',
      label: 'Toggle Performance Monitor',
      keywords: ['perf', 'fps', 'memory', 'performance', 'monitor'],
      icon: 'activity',
      action: () => document.dispatchEvent(new CustomEvent('toggle-perf-monitor'))
    });
    this.commands.push({
      category: 'Actions',
      label: 'Toggle Activity Log',
      keywords: ['log', 'events', 'activity', 'history'],
      icon: 'list',
      action: () => document.dispatchEvent(new CustomEvent('toggle-activity-log'))
    });
    this.commands.push({
      category: 'Actions',
      label: 'Export Sensor Data',
      keywords: ['export', 'download', 'csv', 'json', 'data', 'save'],
      icon: 'download',
      action: () => document.dispatchEvent(new CustomEvent('export-data'))
    });
    this.commands.push({
      category: 'Actions',
      label: 'Toggle Fullscreen',
      keywords: ['fullscreen', 'full', 'screen', 'maximize'],
      icon: 'maximize',
      action: () => document.dispatchEvent(new CustomEvent('toggle-fullscreen'))
    });
    this.commands.push({
      category: 'Actions',
      label: 'Show Keyboard Shortcuts',
      keywords: ['keyboard', 'shortcuts', 'keys', 'help'],
      icon: 'keyboard',
      action: () => document.dispatchEvent(new CustomEvent('show-shortcuts'))
    });
  }

  createDOM() {
    this.overlay = document.createElement('div');
    this.overlay.className = 'cmd-palette-overlay';
    this.overlay.setAttribute('role', 'dialog');
    this.overlay.setAttribute('aria-label', 'Command palette');
    this.overlay.setAttribute('aria-modal', 'true');

    this.overlay.innerHTML = `
      <div class="cmd-palette">
        <div class="cmd-palette-input-wrap">
          <svg class="cmd-palette-search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
          <input type="text" class="cmd-palette-input" placeholder="Type a command..." aria-label="Search commands" autocomplete="off" spellcheck="false">
          <kbd class="cmd-palette-hint">Esc</kbd>
        </div>
        <div class="cmd-palette-results" role="listbox" aria-label="Commands"></div>
        <div class="cmd-palette-footer">
          <span><kbd>Up</kbd><kbd>Down</kbd> navigate</span>
          <span><kbd>Enter</kbd> execute</span>
          <span><kbd>Esc</kbd> close</span>
        </div>
      </div>
    `;

    this.overlay.addEventListener('click', (e) => {
      if (e.target === this.overlay) this.hide();
    });

    this.input = this.overlay.querySelector('.cmd-palette-input');
    this.results = this.overlay.querySelector('.cmd-palette-results');

    this.input.addEventListener('input', () => this.onInput());
    this.input.addEventListener('keydown', (e) => this.onKeydown(e));

    document.body.appendChild(this.overlay);
  }

  bindGlobalShortcut() {
    document.addEventListener('keydown', (e) => {
      // Ctrl+K or Cmd+K
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        this.toggle();
      }
    });
  }

  toggle() {
    this.visible ? this.hide() : this.show();
  }

  show() {
    this.visible = true;
    this.overlay.classList.add('visible');
    this.input.value = '';
    this.selectedIndex = 0;
    this.filteredCommands = [...this.commands];
    this.renderResults();
    this.input.focus();
  }

  hide() {
    this.visible = false;
    this.overlay.classList.remove('visible');
  }

  onInput() {
    const query = this.input.value.toLowerCase().trim();
    if (!query) {
      this.filteredCommands = [...this.commands];
    } else {
      this.filteredCommands = this.commands
        .map(cmd => {
          const score = this.fuzzyScore(query, cmd);
          return { ...cmd, score };
        })
        .filter(cmd => cmd.score > 0)
        .sort((a, b) => b.score - a.score);
    }
    this.selectedIndex = 0;
    this.renderResults();
  }

  fuzzyScore(query, cmd) {
    const targets = [cmd.label.toLowerCase(), ...cmd.keywords, cmd.category.toLowerCase()];
    let best = 0;
    for (const target of targets) {
      if (target === query) return 100;
      if (target.startsWith(query)) best = Math.max(best, 80);
      if (target.includes(query)) best = Math.max(best, 60);
      // Check each word
      const words = query.split(/\s+/);
      const allMatch = words.every(w => targets.some(t => t.includes(w)));
      if (allMatch) best = Math.max(best, 40);
    }
    return best;
  }

  renderResults() {
    if (this.filteredCommands.length === 0) {
      this.results.innerHTML = '<div class="cmd-palette-empty">No matching commands</div>';
      return;
    }

    let lastCategory = '';
    let html = '';

    this.filteredCommands.forEach((cmd, i) => {
      if (cmd.category !== lastCategory) {
        lastCategory = cmd.category;
        html += `<div class="cmd-palette-category">${cmd.category}</div>`;
      }
      const selected = i === this.selectedIndex ? ' cmd-palette-item-selected' : '';
      html += `
        <div class="cmd-palette-item${selected}" data-index="${i}" role="option" aria-selected="${i === this.selectedIndex}">
          <span class="cmd-palette-item-icon">${this.getIcon(cmd.icon)}</span>
          <span class="cmd-palette-item-label">${cmd.label}</span>
        </div>`;
    });

    this.results.innerHTML = html;

    // Click handlers
    this.results.querySelectorAll('.cmd-palette-item').forEach(el => {
      el.addEventListener('click', () => {
        const idx = parseInt(el.dataset.index, 10);
        this.executeCommand(idx);
      });
      el.addEventListener('mouseenter', () => {
        this.selectedIndex = parseInt(el.dataset.index, 10);
        this.updateSelection();
      });
    });

    // Scroll selected into view
    const selectedEl = this.results.querySelector('.cmd-palette-item-selected');
    if (selectedEl) selectedEl.scrollIntoView({ block: 'nearest' });
  }

  updateSelection() {
    this.results.querySelectorAll('.cmd-palette-item').forEach((el, i) => {
      const isSelected = parseInt(el.dataset.index, 10) === this.selectedIndex;
      el.classList.toggle('cmd-palette-item-selected', isSelected);
      el.setAttribute('aria-selected', String(isSelected));
    });
  }

  onKeydown(e) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      this.selectedIndex = Math.min(this.selectedIndex + 1, this.filteredCommands.length - 1);
      this.updateSelection();
      const sel = this.results.querySelector('.cmd-palette-item-selected');
      if (sel) sel.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      this.selectedIndex = Math.max(this.selectedIndex - 1, 0);
      this.updateSelection();
      const sel = this.results.querySelector('.cmd-palette-item-selected');
      if (sel) sel.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter') {
      e.preventDefault();
      this.executeCommand(this.selectedIndex);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      this.hide();
    }
  }

  executeCommand(index) {
    const cmd = this.filteredCommands[index];
    if (cmd) {
      this.hide();
      cmd.action();
    }
  }

  getIcon(name) {
    const icons = {
      grid: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>',
      cpu: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><line x1="9" y1="1" x2="9" y2="4"/><line x1="15" y1="1" x2="15" y2="4"/><line x1="9" y1="20" x2="9" y2="23"/><line x1="15" y1="20" x2="15" y2="23"/></svg>',
      play: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>',
      layers: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 17 12 22 22 17"/><polyline points="2 12 12 17 22 12"/></svg>',
      zap: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>',
      box: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>',
      wifi: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12.55a11 11 0 0 1 14.08 0"/><path d="M1.42 9a16 16 0 0 1 21.16 0"/><path d="M8.53 16.11a6 6 0 0 1 6.95 0"/><line x1="12" y1="20" x2="12.01" y2="20"/></svg>',
      database: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>',
      external: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>',
      moon: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>',
      activity: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>',
      list: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>',
      download: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>',
      maximize: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" y1="3" x2="14" y2="10"/><line x1="3" y1="21" x2="10" y2="14"/></svg>',
      keyboard: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="4" width="20" height="16" rx="2"/><line x1="6" y1="8" x2="6.01" y2="8"/><line x1="10" y1="8" x2="10.01" y2="8"/><line x1="14" y1="8" x2="14.01" y2="8"/><line x1="18" y1="8" x2="18.01" y2="8"/><line x1="8" y1="12" x2="8.01" y2="12"/><line x1="12" y1="12" x2="12.01" y2="12"/><line x1="16" y1="12" x2="16.01" y2="12"/><line x1="7" y1="16" x2="17" y2="16"/></svg>'
    };
    return icons[name] || '';
  }

  dispose() {
    if (this.overlay?.parentNode) {
      this.overlay.parentNode.removeChild(this.overlay);
    }
  }
}
