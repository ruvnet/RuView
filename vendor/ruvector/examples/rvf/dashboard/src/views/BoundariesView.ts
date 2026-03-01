import { fetchBoundaryTimeline, fetchBoundaryAlerts, BoundaryPoint, BoundaryAlert } from '../api';
import { onEvent, LiveEvent } from '../ws';

export class BoundariesView {
  private container: HTMLElement | null = null;
  private chartCanvas: HTMLCanvasElement | null = null;
  private alertsEl: HTMLElement | null = null;
  private unsubWs: (() => void) | null = null;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private points: BoundaryPoint[] = [];

  mount(container: HTMLElement): void {
    this.container = container;

    const grid = document.createElement('div');
    grid.className = 'grid-12';
    container.appendChild(grid);

    // View header with explanation
    const header = document.createElement('div');
    header.className = 'col-12';
    header.style.cssText = 'padding:4px 0 8px 0';
    header.innerHTML = `
      <div style="font-size:14px;font-weight:600;color:var(--text-primary);margin-bottom:2px">Boundary Tracking</div>
      <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">
        Monitors the <span style="color:var(--accent)">causal boundary</span> &mdash; the expanding frontier where new events enter the graph.
        <strong>Instability</strong> = average boundary pressure (lower is better).
        <strong>Crossings</strong> = epochs where coherence dropped below 0.80 threshold.
        Amber ticks on the timeline mark boundary crossing events. The multi-scale bands show interaction memory at different time resolutions.
      </div>
    `;
    grid.appendChild(header);

    // Top metrics
    const pressureCard = this.createMetricCard('Boundary Instability', '--', 'accent');
    pressureCard.className += ' col-4';
    grid.appendChild(pressureCard);

    const crossedCard = this.createMetricCard('Crossings Detected', '--', '');
    crossedCard.className += ' col-4';
    grid.appendChild(crossedCard);

    const alertCountCard = this.createMetricCard('Active Alerts', '--', '');
    alertCountCard.className += ' col-4';
    grid.appendChild(alertCountCard);

    // Timeline chart
    const chartPanel = document.createElement('div');
    chartPanel.className = 'panel col-12';
    const chartHeader = document.createElement('div');
    chartHeader.className = 'panel-header';
    chartHeader.textContent = 'Boundary Evolution Timeline';
    chartPanel.appendChild(chartHeader);
    const chartBody = document.createElement('div');
    chartBody.className = 'panel-body';
    chartBody.style.height = '280px';
    chartBody.style.padding = '12px';
    this.chartCanvas = document.createElement('canvas');
    this.chartCanvas.style.width = '100%';
    this.chartCanvas.style.height = '100%';
    this.chartCanvas.style.display = 'block';
    chartBody.appendChild(this.chartCanvas);
    chartPanel.appendChild(chartBody);
    grid.appendChild(chartPanel);

    // Multi-scale memory visualization
    const scalePanel = document.createElement('div');
    scalePanel.className = 'panel col-12';
    const scaleHeader = document.createElement('div');
    scaleHeader.className = 'panel-header';
    scaleHeader.textContent = 'Multi-Scale Interaction Memory';
    scalePanel.appendChild(scaleHeader);
    const scaleBody = document.createElement('div');
    scaleBody.className = 'panel-body';
    scaleBody.style.height = '64px';
    const scaleCanvas = document.createElement('canvas');
    scaleCanvas.style.width = '100%';
    scaleCanvas.style.height = '100%';
    scaleCanvas.style.display = 'block';
    scaleBody.appendChild(scaleCanvas);
    scalePanel.appendChild(scaleBody);
    grid.appendChild(scalePanel);
    this.renderScaleBands(scaleCanvas);

    // Alerts list
    const alertPanel = document.createElement('div');
    alertPanel.className = 'panel col-12';
    const alertHeader = document.createElement('div');
    alertHeader.className = 'panel-header';
    alertHeader.textContent = 'Boundary Alerts';
    alertPanel.appendChild(alertHeader);
    this.alertsEl = document.createElement('div');
    this.alertsEl.className = 'panel-body';
    this.alertsEl.style.maxHeight = '240px';
    this.alertsEl.style.overflowY = 'auto';
    this.alertsEl.style.padding = '0';
    alertPanel.appendChild(this.alertsEl);
    grid.appendChild(alertPanel);

    this.loadData(pressureCard, crossedCard, alertCountCard);
    this.pollTimer = setInterval(() => {
      this.loadData(pressureCard, crossedCard, alertCountCard);
    }, 8000);

    this.unsubWs = onEvent((ev: LiveEvent) => {
      if (ev.event_type === 'boundary_alert') {
        this.loadData(pressureCard, crossedCard, alertCountCard);
      }
    });
  }

  private createMetricCard(label: string, value: string, modifier: string): HTMLElement {
    const card = document.createElement('div');
    card.className = 'metric-card';
    card.innerHTML = `
      <span class="metric-label">${label}</span>
      <span class="metric-value ${modifier}" data-metric>${value}</span>
      <span class="metric-sub" data-sub></span>
    `;
    return card;
  }

  private async loadData(
    pressureCard: HTMLElement,
    crossedCard: HTMLElement,
    alertCountCard: HTMLElement,
  ): Promise<void> {
    let points: BoundaryPoint[];
    let alerts: BoundaryAlert[];

    try {
      points = await fetchBoundaryTimeline('default');
    } catch {
      points = this.generateDemoTimeline();
    }

    try {
      alerts = await fetchBoundaryAlerts();
    } catch {
      alerts = this.generateDemoAlerts();
    }

    this.points = points;

    // Update metrics
    const avgPressure = points.length > 0
      ? points.reduce((s, p) => s + p.pressure, 0) / points.length
      : 0;
    const crossings = points.filter((p) => p.crossed).length;

    const pVal = pressureCard.querySelector('[data-metric]');
    if (pVal) pVal.textContent = avgPressure.toFixed(3);

    const cVal = crossedCard.querySelector('[data-metric]');
    if (cVal) cVal.textContent = String(crossings);

    const aVal = alertCountCard.querySelector('[data-metric]');
    if (aVal) {
      aVal.textContent = String(alerts.length);
      aVal.className = `metric-value ${alerts.length > 3 ? 'critical' : alerts.length > 0 ? 'warning' : 'success'}`;
    }

    this.renderChart();
    this.renderAlerts(alerts);
  }

  private renderChart(): void {
    const canvas = this.chartCanvas;
    if (!canvas) return;
    const rect = canvas.parentElement?.getBoundingClientRect();
    if (!rect) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.scale(dpr, dpr);

    const w = rect.width;
    const h = rect.height;
    const pad = { top: 8, right: 12, bottom: 24, left: 40 };
    const pw = w - pad.left - pad.right;
    const ph = h - pad.top - pad.bottom;

    ctx.clearRect(0, 0, w, h);

    if (this.points.length === 0) return;

    const maxP = Math.max(...this.points.map((p) => p.pressure), 1);

    // Grid lines
    ctx.strokeStyle = '#1E2630';
    ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
      const y = pad.top + (ph * i) / 4;
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(pad.left + pw, y);
      ctx.stroke();
    }

    // Y-axis labels
    ctx.fillStyle = '#484F58';
    ctx.font = '10px "JetBrains Mono", monospace';
    ctx.textAlign = 'right';
    for (let i = 0; i <= 4; i++) {
      const y = pad.top + (ph * i) / 4;
      const val = maxP * (1 - i / 4);
      ctx.fillText(val.toFixed(2), pad.left - 6, y + 3);
    }

    // Line
    ctx.strokeStyle = '#00E5FF';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    this.points.forEach((p, i) => {
      const x = pad.left + (i / (this.points.length - 1)) * pw;
      const y = pad.top + ph - (p.pressure / maxP) * ph;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();

    // Crossing ticks
    ctx.strokeStyle = '#FFB020';
    ctx.lineWidth = 1;
    this.points.forEach((p, i) => {
      if (p.crossed) {
        const x = pad.left + (i / (this.points.length - 1)) * pw;
        ctx.beginPath();
        ctx.moveTo(x, pad.top);
        ctx.lineTo(x, pad.top + ph);
        ctx.stroke();
      }
    });
  }

  private renderAlerts(alerts: BoundaryAlert[]): void {
    if (!this.alertsEl) return;
    this.alertsEl.innerHTML = '';

    if (alerts.length === 0) {
      this.alertsEl.innerHTML = '<div class="empty-state" style="height:60px">No active alerts</div>';
      return;
    }

    for (const a of alerts) {
      const item = document.createElement('div');
      item.className = 'alert-item';
      const severity = a.pressure < 0.5 ? 'critical' : a.pressure < 0.8 ? 'warning' : 'success';
      item.innerHTML = `
        <span class="alert-dot ${severity}"></span>
        <span class="alert-msg">${a.message}</span>
        <span class="alert-sector">${a.target_id}</span>
      `;
      this.alertsEl.appendChild(item);
    }
  }

  private renderScaleBands(canvas: HTMLCanvasElement): void {
    requestAnimationFrame(() => {
      const rect = canvas.parentElement?.getBoundingClientRect();
      if (!rect) return;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      ctx.scale(dpr, dpr);

      const w = rect.width;
      const h = rect.height;
      const bands = [
        { label: 'Seconds', color: '#00E5FF', height: h * 0.33 },
        { label: 'Hours', color: '#0099AA', height: h * 0.33 },
        { label: 'Days', color: '#006677', height: h * 0.34 },
      ];

      let y = 0;
      for (const band of bands) {
        ctx.fillStyle = band.color;
        ctx.globalAlpha = 0.2;
        ctx.fillRect(0, y, w, band.height);
        ctx.globalAlpha = 1;
        ctx.fillStyle = '#8B949E';
        ctx.font = '9px "JetBrains Mono", monospace';
        ctx.fillText(band.label, 4, y + band.height / 2 + 3);
        y += band.height;
      }

      // Boundary flip ticks
      const tickCount = 8;
      ctx.strokeStyle = '#FFB020';
      ctx.lineWidth = 1;
      ctx.globalAlpha = 0.7;
      for (let i = 0; i < tickCount; i++) {
        const x = (w * (i + 1)) / (tickCount + 1) + (Math.sin(i * 3.14) * 20);
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
    });
  }

  private generateDemoTimeline(): BoundaryPoint[] {
    const pts: BoundaryPoint[] = [];
    for (let i = 0; i < 50; i++) {
      const p = 0.7 + 0.25 * Math.sin(i * 0.3) + (Math.random() - 0.5) * 0.1;
      pts.push({ epoch: i, pressure: Math.max(0, p), crossed: p < 0.75 });
    }
    return pts;
  }

  private generateDemoAlerts(): BoundaryAlert[] {
    return [
      { target_id: 'sector-7G', epoch: 42, pressure: 0.62, message: 'Coherence below threshold in sector 7G' },
      { target_id: 'sector-3A', epoch: 38, pressure: 0.71, message: 'Boundary radius expanding in sector 3A' },
      { target_id: 'sector-12F', epoch: 45, pressure: 0.45, message: 'Critical instability detected in sector 12F' },
    ];
  }

  unmount(): void {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    this.unsubWs?.();
    this.chartCanvas = null;
    this.alertsEl = null;
    this.container = null;
  }
}
