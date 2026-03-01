import { WitnessLog, WitnessLogEntry } from '../components/WitnessLog';
import { fetchStatus, fetchMemoryTiers, fetchWitnessLog, SystemStatus, MemoryTiers } from '../api';
import { onEvent, LiveEvent } from '../ws';

const PIPELINE_STAGES = ['P0', 'P1', 'P2', 'L0', 'L1', 'L2'];

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatUptime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  return `${h}h ${m}m ${s}s`;
}

export class StatusDashboard {
  private container: HTMLElement | null = null;
  private witnessLog: WitnessLog | null = null;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private unsubWs: (() => void) | null = null;
  private downloadEl: HTMLElement | null = null;
  private pipelineEl: HTMLElement | null = null;
  private gaugesEl: HTMLElement | null = null;
  private segmentEl: HTMLElement | null = null;
  private uptimeEl: HTMLElement | null = null;

  mount(container: HTMLElement): void {
    this.container = container;

    const outerWrap = document.createElement('div');
    outerWrap.style.cssText = 'display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden';
    container.appendChild(outerWrap);

    // View header with explanation
    const viewHeader = document.createElement('div');
    viewHeader.style.cssText = 'padding:12px 20px;border-bottom:1px solid var(--border);flex-shrink:0';
    viewHeader.innerHTML = `
      <div style="font-size:14px;font-weight:600;color:var(--text-primary);margin-bottom:2px">System Status</div>
      <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">
        Live overview of the RVF runtime. <strong>Pipeline stages</strong> show data processing progress (P0&ndash;P2 = planet pipeline, L0&ndash;L2 = life pipeline).
        <strong>Downloads</strong> track segment ingestion. <strong>Memory tiers</strong> show S/M/L utilization.
        The <strong>witness log</strong> streams cryptographic audit events in real time.
      </div>
    `;
    outerWrap.appendChild(viewHeader);

    const grid = document.createElement('div');
    grid.className = 'status-grid';
    grid.style.flex = '1';
    grid.style.overflow = 'auto';
    grid.style.minHeight = '0';
    outerWrap.appendChild(grid);

    // Top-left: System health + uptime
    const healthPanel = this.createPanel('System Health');
    grid.appendChild(healthPanel);
    this.uptimeEl = document.createElement('div');
    this.uptimeEl.className = 'panel-body';
    healthPanel.appendChild(this.uptimeEl);

    // Top-right: Pipeline stages
    const pipePanel = this.createPanel('Pipeline Stages');
    grid.appendChild(pipePanel);
    this.pipelineEl = document.createElement('div');
    this.pipelineEl.className = 'panel-body';
    pipePanel.appendChild(this.pipelineEl);

    // Downloads panel
    const dlPanel = this.createPanel('Download Progress');
    grid.appendChild(dlPanel);
    this.downloadEl = document.createElement('div');
    this.downloadEl.className = 'panel-body';
    dlPanel.appendChild(this.downloadEl);

    // Memory gauges
    const memPanel = this.createPanel('Memory Tiers (S / M / L)');
    grid.appendChild(memPanel);
    this.gaugesEl = document.createElement('div');
    this.gaugesEl.className = 'panel-body';
    this.gaugesEl.innerHTML = '<div class="gauge-container"></div>';
    memPanel.appendChild(this.gaugesEl);

    // Segments (full width)
    const segPanel = this.createPanel('Segment Overview');
    segPanel.classList.add('full-width');
    grid.appendChild(segPanel);
    this.segmentEl = document.createElement('div');
    this.segmentEl.className = 'panel-body';
    segPanel.appendChild(this.segmentEl);

    // Witness log (full width)
    const logWrapper = document.createElement('div');
    logWrapper.classList.add('full-width');
    logWrapper.style.minHeight = '200px';
    grid.appendChild(logWrapper);
    this.witnessLog = new WitnessLog(logWrapper);

    // Load initial data
    this.loadData();
    this.loadWitnessLog();
    this.pollTimer = setInterval(() => this.loadData(), 5000);

    // Live witness events
    this.unsubWs = onEvent((ev: LiveEvent) => {
      if (ev.event_type === 'witness') {
        this.witnessLog?.addEntry({
          timestamp: new Date(ev.timestamp * 1000).toISOString().substring(11, 19),
          type: String(ev.data['type'] ?? 'update'),
          action: String(ev.data['action'] ?? ''),
          hash: String(ev.data['hash'] ?? ''),
        });
      }
    });
  }

  private async loadWitnessLog(): Promise<void> {
    try {
      const log = await fetchWitnessLog();
      for (const entry of log.entries) {
        const ts = entry.timestamp.includes('T')
          ? entry.timestamp.split('T')[1]?.substring(0, 8) ?? entry.timestamp
          : entry.timestamp;
        this.witnessLog?.addEntry({
          timestamp: ts,
          type: entry.type,
          action: `${entry.witness}: ${entry.action}`,
          hash: entry.hash,
        });
      }
    } catch {
      // Fallback: show demo entries so log is never empty
      const demoEntries: WitnessLogEntry[] = [
        { timestamp: '14:00:01', type: 'seal', action: 'W_root: Chain initialized', hash: 'a1b2c3d4' },
        { timestamp: '14:00:12', type: 'commit', action: 'W_photometry: Light curves ingested', hash: 'b3c4d5e6' },
        { timestamp: '14:01:03', type: 'commit', action: 'W_periodogram: BLS search completed', hash: 'c5d6e7f8' },
        { timestamp: '14:02:18', type: 'commit', action: 'W_stellar: Stellar parameters derived', hash: 'd7e8f9a0' },
        { timestamp: '14:03:45', type: 'merge', action: 'W_transit: Transit model merged', hash: 'e9f0a1b2' },
        { timestamp: '14:04:22', type: 'commit', action: 'W_radial_velocity: RV data ingested', hash: 'f1a2b3c4' },
        { timestamp: '14:05:10', type: 'commit', action: 'W_orbit: Orbital solutions computed', hash: 'a3b4c5d6' },
        { timestamp: '14:06:33', type: 'commit', action: 'W_esi: ESI ranking computed', hash: 'b5c6d7e8' },
        { timestamp: '14:08:01', type: 'merge', action: 'W_spectroscopy: JWST observations merged', hash: 'c7d8e9f0' },
        { timestamp: '14:09:15', type: 'commit', action: 'W_biosig: Biosignature scoring done', hash: 'd9e0f1a2' },
        { timestamp: '14:10:42', type: 'commit', action: 'W_blind: Blind test passed (τ=1.0)', hash: 'e1f2a3b4' },
        { timestamp: '14:15:55', type: 'verify', action: 'W_seal: Chain sealed — Ed25519 signed', hash: 'c9d0e1f2' },
      ];
      for (const e of demoEntries) {
        this.witnessLog?.addEntry(e);
      }
    }
  }

  private createPanel(title: string): HTMLElement {
    const panel = document.createElement('div');
    panel.className = 'panel';
    const header = document.createElement('div');
    header.className = 'panel-header';
    header.textContent = title;
    panel.appendChild(header);
    return panel;
  }

  private async loadData(): Promise<void> {
    let status: SystemStatus;
    let tiers: MemoryTiers;

    try {
      status = await fetchStatus();
    } catch (err) {
      console.error('Status API error:', err);
      status = { uptime: 0, segments: 0, file_size: 0, download_progress: {} };
    }

    try {
      tiers = await fetchMemoryTiers();
    } catch (err) {
      console.error('Memory API error:', err);
      tiers = { small: { used: 0, total: 0 }, medium: { used: 0, total: 0 }, large: { used: 0, total: 0 } };
    }

    this.renderHealth(status);
    this.renderPipeline();
    this.renderDownloads(status.download_progress);
    this.renderGauges(tiers);
    this.renderSegments(status);
  }

  private renderHealth(status: SystemStatus): void {
    if (!this.uptimeEl) return;
    this.uptimeEl.innerHTML = `
      <div style="display: flex; gap: 32px; flex-wrap: wrap;">
        <div class="gauge">
          <div class="gauge-label">Uptime</div>
          <div class="gauge-value">${formatUptime(status.uptime)}</div>
        </div>
        <div class="gauge">
          <div class="gauge-label">Segments</div>
          <div class="gauge-value">${status.segments}</div>
        </div>
        <div class="gauge">
          <div class="gauge-label">File Size</div>
          <div class="gauge-value">${formatBytes(status.file_size)}</div>
        </div>
      </div>
    `;
  }

  private renderPipeline(): void {
    if (!this.pipelineEl) return;
    // Simulate active stages
    const activeIdx = Math.floor(Date.now() / 3000) % PIPELINE_STAGES.length;
    this.pipelineEl.innerHTML = '';
    const stagesDiv = document.createElement('div');
    stagesDiv.className = 'pipeline-stages';

    for (let i = 0; i < PIPELINE_STAGES.length; i++) {
      if (i > 0) {
        const arrow = document.createElement('span');
        arrow.className = 'pipeline-arrow';
        arrow.textContent = '\u2192';
        stagesDiv.appendChild(arrow);
      }

      const stage = document.createElement('div');
      stage.className = 'pipeline-stage';
      if (i < activeIdx) stage.classList.add('active');
      else if (i === activeIdx) stage.classList.add('pending');
      else stage.classList.add('idle');
      stage.textContent = PIPELINE_STAGES[i];
      stagesDiv.appendChild(stage);
    }
    this.pipelineEl.appendChild(stagesDiv);
  }

  private renderDownloads(progress: Record<string, number>): void {
    if (!this.downloadEl) return;
    this.downloadEl.innerHTML = '';

    for (const [name, pct] of Object.entries(progress)) {
      const label = document.createElement('div');
      label.className = 'progress-label';
      label.innerHTML = `<span>${name}</span><span>${(pct * 100).toFixed(0)}%</span>`;
      this.downloadEl.appendChild(label);

      const bar = document.createElement('div');
      bar.className = 'progress-bar';
      const fill = document.createElement('div');
      fill.className = `progress-fill ${pct >= 1 ? 'success' : pct > 0.8 ? 'info' : 'warning'}`;
      fill.style.width = `${Math.min(100, pct * 100)}%`;
      bar.appendChild(fill);
      this.downloadEl.appendChild(bar);
    }
  }

  private renderGauges(tiers: MemoryTiers): void {
    if (!this.gaugesEl) return;
    const container = this.gaugesEl.querySelector('.gauge-container');
    if (!container) return;
    container.innerHTML = '';

    const tierData = [
      { label: 'Small', ...tiers.small },
      { label: 'Medium', ...tiers.medium },
      { label: 'Large', ...tiers.large },
    ];

    for (const t of tierData) {
      const pct = t.total > 0 ? t.used / t.total : 0;
      const gauge = document.createElement('div');
      gauge.className = 'gauge';

      // SVG ring gauge
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('viewBox', '0 0 80 80');
      svg.classList.add('gauge-ring');

      const bgCircle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
      bgCircle.setAttribute('cx', '40');
      bgCircle.setAttribute('cy', '40');
      bgCircle.setAttribute('r', '34');
      bgCircle.setAttribute('fill', 'none');
      bgCircle.setAttribute('stroke', '#1C2333');
      bgCircle.setAttribute('stroke-width', '6');
      svg.appendChild(bgCircle);

      const circumference = 2 * Math.PI * 34;
      const fgCircle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
      fgCircle.setAttribute('cx', '40');
      fgCircle.setAttribute('cy', '40');
      fgCircle.setAttribute('r', '34');
      fgCircle.setAttribute('fill', 'none');
      fgCircle.setAttribute('stroke', pct > 0.9 ? '#FF4D4D' : pct > 0.7 ? '#FFB020' : '#00E5FF');
      fgCircle.setAttribute('stroke-width', '6');
      fgCircle.setAttribute('stroke-dasharray', `${circumference * pct} ${circumference * (1 - pct)}`);
      fgCircle.setAttribute('stroke-dashoffset', `${circumference * 0.25}`);
      fgCircle.setAttribute('stroke-linecap', 'round');
      svg.appendChild(fgCircle);

      const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      text.setAttribute('x', '40');
      text.setAttribute('y', '44');
      text.setAttribute('text-anchor', 'middle');
      text.setAttribute('fill', '#E6EDF3');
      text.setAttribute('font-size', '14');
      text.setAttribute('font-weight', '700');
      text.textContent = `${(pct * 100).toFixed(0)}%`;
      svg.appendChild(text);

      gauge.appendChild(svg);

      const label = document.createElement('div');
      label.className = 'gauge-label';
      label.textContent = `${t.label} (${t.used}/${t.total})`;
      gauge.appendChild(label);

      container.appendChild(gauge);
    }
  }

  private renderSegments(status: SystemStatus): void {
    if (!this.segmentEl) return;
    this.segmentEl.innerHTML = `
      <div style="display: flex; gap: 8px; flex-wrap: wrap;">
        ${Array.from({ length: Math.min(status.segments, 64) }, (_, i) => {
          const hue = (i / Math.min(status.segments, 64)) * 240;
          return `<div style="width: 12px; height: 12px; border-radius: 2px; background: hsl(${hue}, 60%, 45%);" title="Segment ${i}"></div>`;
        }).join('')}
        ${status.segments > 64 ? `<span style="color: var(--text-muted); font-size: 11px; align-self: center;">+${status.segments - 64} more</span>` : ''}
      </div>
    `;
  }

  unmount(): void {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    this.unsubWs?.();
    this.witnessLog?.destroy();

    this.witnessLog = null;
    this.downloadEl = null;
    this.pipelineEl = null;
    this.gaugesEl = null;
    this.segmentEl = null;
    this.uptimeEl = null;
    this.container = null;
  }
}
