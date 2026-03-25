// Performance Monitor Overlay
// Shows FPS, memory usage, and network latency in real-time

export class PerfMonitor {
  constructor() {
    this.visible = false;
    this.panel = null;
    this.frames = [];
    this.lastFrameTime = 0;
    this.rafId = null;
    this.latencyHistory = [];
    this.maxHistory = 60;
  }

  init() {
    this.createPanel();
    document.addEventListener('toggle-perf-monitor', () => this.toggle());
  }

  createPanel() {
    this.panel = document.createElement('div');
    this.panel.className = 'perf-monitor';
    this.panel.setAttribute('role', 'status');
    this.panel.setAttribute('aria-label', 'Performance monitor');
    this.panel.innerHTML = `
      <div class="perf-header">
        <span>PERF</span>
        <button class="perf-close" aria-label="Close performance monitor">&times;</button>
      </div>
      <div class="perf-metrics">
        <div class="perf-row">
          <span class="perf-label">FPS</span>
          <span class="perf-value" data-metric="fps">--</span>
          <canvas class="perf-spark" data-spark="fps" width="60" height="20"></canvas>
        </div>
        <div class="perf-row">
          <span class="perf-label">MEM</span>
          <span class="perf-value" data-metric="memory">--</span>
          <canvas class="perf-spark" data-spark="memory" width="60" height="20"></canvas>
        </div>
        <div class="perf-row">
          <span class="perf-label">LAT</span>
          <span class="perf-value" data-metric="latency">--</span>
          <canvas class="perf-spark" data-spark="latency" width="60" height="20"></canvas>
        </div>
        <div class="perf-row">
          <span class="perf-label">DOM</span>
          <span class="perf-value" data-metric="dom">--</span>
        </div>
      </div>
    `;

    this.panel.querySelector('.perf-close').addEventListener('click', () => this.hide());

    // Make it draggable
    this.makeDraggable();

    document.body.appendChild(this.panel);

    this.sparkData = {
      fps: [],
      memory: [],
      latency: []
    };
  }

  makeDraggable() {
    const header = this.panel.querySelector('.perf-header');
    let dragging = false;
    let offsetX = 0;
    let offsetY = 0;

    header.addEventListener('mousedown', (e) => {
      if (e.target.tagName === 'BUTTON') return;
      dragging = true;
      offsetX = e.clientX - this.panel.offsetLeft;
      offsetY = e.clientY - this.panel.offsetTop;
      header.style.cursor = 'grabbing';
    });

    document.addEventListener('mousemove', (e) => {
      if (!dragging) return;
      this.panel.style.left = `${e.clientX - offsetX}px`;
      this.panel.style.top = `${e.clientY - offsetY}px`;
      this.panel.style.right = 'auto';
      this.panel.style.bottom = 'auto';
    });

    document.addEventListener('mouseup', () => {
      dragging = false;
      header.style.cursor = 'grab';
    });
  }

  toggle() {
    this.visible ? this.hide() : this.show();
  }

  show() {
    this.panel.classList.add('visible');
    this.visible = true;
    this.lastFrameTime = performance.now();
    this.tick();
  }

  hide() {
    this.panel.classList.remove('visible');
    this.visible = false;
    if (this.rafId) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
  }

  tick() {
    if (!this.visible) return;

    const now = performance.now();
    this.frames.push(now);

    // Keep only last second of frames
    while (this.frames.length > 0 && this.frames[0] < now - 1000) {
      this.frames.shift();
    }

    const fps = this.frames.length;
    this.updateMetric('fps', fps, 'fps');
    this.pushSpark('fps', fps, 0, 120);

    // Memory (if available)
    if (performance.memory) {
      const mb = Math.round(performance.memory.usedJSHeapSize / (1024 * 1024));
      const total = Math.round(performance.memory.jsHeapSizeLimit / (1024 * 1024));
      this.updateMetric('memory', `${mb}MB`, mb > total * 0.8 ? 'warning' : 'ok');
      this.pushSpark('memory', mb, 0, total);
    } else {
      this.updateMetric('memory', 'N/A', 'na');
    }

    // DOM node count
    const domNodes = document.querySelectorAll('*').length;
    this.updateMetric('dom', domNodes, domNodes > 3000 ? 'warning' : 'ok');

    // Estimate latency from last navigation or resource timing
    this.measureLatency();

    this.rafId = requestAnimationFrame(() => this.tick());
  }

  measureLatency() {
    const entries = performance.getEntriesByType('resource');
    if (entries.length > 0) {
      const last = entries[entries.length - 1];
      const latency = Math.round(last.responseEnd - last.requestStart);
      if (latency > 0 && latency < 30000) {
        this.latencyHistory.push(latency);
        if (this.latencyHistory.length > this.maxHistory) {
          this.latencyHistory.shift();
        }
        const avg = Math.round(
          this.latencyHistory.reduce((a, b) => a + b, 0) / this.latencyHistory.length
        );
        this.updateMetric('latency', `${avg}ms`, avg > 500 ? 'warning' : 'ok');
        this.pushSpark('latency', avg, 0, 1000);
      }
    }
  }

  updateMetric(metric, value, status) {
    const el = this.panel.querySelector(`[data-metric="${metric}"]`);
    if (!el) return;
    el.textContent = value;
    el.className = `perf-value perf-${status}`;
  }

  pushSpark(name, value, min, max) {
    const data = this.sparkData[name];
    if (!data) return;
    data.push(value);
    if (data.length > 60) data.shift();
    this.drawSpark(name, data, min, max);
  }

  drawSpark(name, data, min, max) {
    const canvas = this.panel.querySelector(`[data-spark="${name}"]`);
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const w = canvas.width;
    const h = canvas.height;

    ctx.clearRect(0, 0, w, h);

    if (data.length < 2) return;

    const range = max - min || 1;
    ctx.beginPath();
    ctx.strokeStyle = 'rgba(50, 184, 198, 0.8)';
    ctx.lineWidth = 1.5;

    data.forEach((val, i) => {
      const x = (i / (data.length - 1)) * w;
      const y = h - ((val - min) / range) * h;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });

    ctx.stroke();
  }

  dispose() {
    this.hide();
    if (this.panel?.parentNode) {
      this.panel.parentNode.removeChild(this.panel);
    }
  }
}
