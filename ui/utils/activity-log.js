// Activity Log - Scrollable panel showing system events in real-time
// Toggle with 'L' key or command palette

export class ActivityLog {
  constructor() {
    this.panel = null;
    this.visible = false;
    this.entries = [];
    this.maxEntries = 200;
    this.logBody = null;
    this.filters = { info: true, warning: true, error: true, connection: true };
  }

  init() {
    this.createPanel();
    this.interceptConsole();
    document.addEventListener('toggle-activity-log', () => this.toggle());
  }

  createPanel() {
    this.panel = document.createElement('div');
    this.panel.className = 'activity-log';
    this.panel.setAttribute('role', 'log');
    this.panel.setAttribute('aria-label', 'Activity log');
    this.panel.innerHTML = `
      <div class="activity-log-header">
        <span class="activity-log-title">Activity Log</span>
        <div class="activity-log-controls">
          <button class="activity-log-filter active" data-filter="info" aria-label="Toggle info messages" title="Info">I</button>
          <button class="activity-log-filter active" data-filter="warning" aria-label="Toggle warnings" title="Warnings">W</button>
          <button class="activity-log-filter active" data-filter="error" aria-label="Toggle errors" title="Errors">E</button>
          <button class="activity-log-filter active" data-filter="connection" aria-label="Toggle connection events" title="Connection">C</button>
          <button class="activity-log-clear" aria-label="Clear log" title="Clear">Clear</button>
          <button class="activity-log-close" aria-label="Close activity log">&times;</button>
        </div>
      </div>
      <div class="activity-log-body"></div>
    `;

    this.logBody = this.panel.querySelector('.activity-log-body');

    // Filter toggles
    this.panel.querySelectorAll('.activity-log-filter').forEach(btn => {
      btn.addEventListener('click', () => {
        const filter = btn.dataset.filter;
        this.filters[filter] = !this.filters[filter];
        btn.classList.toggle('active', this.filters[filter]);
        this.rerender();
      });
    });

    // Clear button
    this.panel.querySelector('.activity-log-clear').addEventListener('click', () => {
      this.entries = [];
      this.rerender();
    });

    // Close button
    this.panel.querySelector('.activity-log-close').addEventListener('click', () => this.hide());

    // Make resizable by dragging top edge
    this.makeResizable();

    document.body.appendChild(this.panel);
  }

  makeResizable() {
    let resizing = false;
    let startY = 0;
    let startHeight = 0;

    this.panel.addEventListener('mousedown', (e) => {
      // Only top 5px edge
      const rect = this.panel.getBoundingClientRect();
      if (e.clientY - rect.top > 5) return;
      resizing = true;
      startY = e.clientY;
      startHeight = rect.height;
      e.preventDefault();
    });

    document.addEventListener('mousemove', (e) => {
      if (!resizing) return;
      const delta = startY - e.clientY;
      const newHeight = Math.max(150, Math.min(window.innerHeight * 0.7, startHeight + delta));
      this.panel.style.height = `${newHeight}px`;
    });

    document.addEventListener('mouseup', () => { resizing = false; });
  }

  interceptConsole() {
    const origInfo = console.info;
    const origWarn = console.warn;
    const origError = console.error;

    console.info = (...args) => {
      origInfo.apply(console, args);
      this.addEntry('info', args.map(String).join(' '));
    };

    console.warn = (...args) => {
      origWarn.apply(console, args);
      const msg = args.map(String).join(' ');
      const type = msg.includes('[WS-') || msg.includes('connect') ? 'connection' : 'warning';
      this.addEntry(type, msg);
    };

    console.error = (...args) => {
      origError.apply(console, args);
      this.addEntry('error', args.map(String).join(' '));
    };
  }

  addEntry(type, message) {
    const entry = {
      time: new Date(),
      type,
      message: this.truncate(message, 300)
    };

    this.entries.push(entry);
    if (this.entries.length > this.maxEntries) {
      this.entries.shift();
    }

    if (this.visible && this.filters[type]) {
      this.appendEntry(entry);
      // Auto-scroll to bottom
      this.logBody.scrollTop = this.logBody.scrollHeight;
    }
  }

  appendEntry(entry) {
    const el = document.createElement('div');
    el.className = `activity-log-entry activity-log-${entry.type}`;
    const time = entry.time.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
    el.innerHTML = `<span class="activity-log-time">${time}</span><span class="activity-log-type">${entry.type.toUpperCase().charAt(0)}</span><span class="activity-log-msg">${this.escapeHtml(entry.message)}</span>`;
    this.logBody.appendChild(el);
  }

  rerender() {
    this.logBody.innerHTML = '';
    this.entries
      .filter(e => this.filters[e.type])
      .forEach(e => this.appendEntry(e));
    this.logBody.scrollTop = this.logBody.scrollHeight;
  }

  toggle() {
    this.visible ? this.hide() : this.show();
  }

  show() {
    this.visible = true;
    this.panel.classList.add('visible');
    this.rerender();
  }

  hide() {
    this.visible = false;
    this.panel.classList.remove('visible');
  }

  truncate(str, max) {
    return str.length > max ? str.slice(0, max) + '...' : str;
  }

  escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  dispose() {
    this.hide();
    if (this.panel?.parentNode) {
      this.panel.parentNode.removeChild(this.panel);
    }
  }
}
