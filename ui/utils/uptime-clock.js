// Uptime Clock - Shows system uptime and current time in header

export class UptimeClock {
  constructor() {
    this.widget = null;
    this.startTime = Date.now();
    this.intervalId = null;
  }

  init() {
    this.createWidget();
    this.update();
    this.intervalId = setInterval(() => this.update(), 1000);
  }

  createWidget() {
    this.widget = document.createElement('div');
    this.widget.className = 'uptime-clock';
    this.widget.setAttribute('aria-label', 'System uptime');
    this.widget.innerHTML = `
      <span class="uptime-time"></span>
      <span class="uptime-separator">|</span>
      <span class="uptime-duration" title="Session uptime"></span>
    `;

    const headerInfo = document.querySelector('.header-info');
    if (headerInfo) {
      headerInfo.appendChild(this.widget);
    }
  }

  update() {
    if (!this.widget) return;

    // Current time
    const now = new Date();
    const time = now.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit' });
    this.widget.querySelector('.uptime-time').textContent = time;

    // Uptime
    const elapsed = Math.floor((Date.now() - this.startTime) / 1000);
    this.widget.querySelector('.uptime-duration').textContent = this.formatDuration(elapsed);
  }

  formatDuration(seconds) {
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) {
      const m = Math.floor(seconds / 60);
      const s = seconds % 60;
      return `${m}m ${s}s`;
    }
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    return `${h}h ${m}m`;
  }

  dispose() {
    if (this.intervalId) clearInterval(this.intervalId);
    this.widget?.remove();
  }
}
