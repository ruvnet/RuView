import { fetchMemoryTiers, MemoryTiers } from '../api';

export class MemoryView {
  private container: HTMLElement | null = null;
  private gaugesEl: HTMLElement | null = null;
  private detailEl: HTMLElement | null = null;
  private pollTimer: ReturnType<typeof setInterval> | null = null;

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
      <div style="font-size:14px;font-weight:600;color:var(--text-primary);margin-bottom:2px">Memory Tiers</div>
      <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">
        RVF uses a <span style="color:var(--accent)">3-tier memory hierarchy</span> for vector storage and retrieval.
        <strong>S (Hot/L1)</strong> = fastest access (&lt;1&mu;s), recent data in CPU cache.
        <strong>M (Warm/HNSW)</strong> = indexed vectors (~12&mu;s), approximate nearest-neighbor graph.
        <strong>L (Cold/Disk)</strong> = archived segments (~450&mu;s), full scan on demand.
        Utilization above 90% triggers tier promotion/eviction policies.
      </div>
    `;
    grid.appendChild(header);

    // Top metrics
    const totalCard = this.createMetricCard('Total Entries', '--', '');
    totalCard.className += ' col-4';
    grid.appendChild(totalCard);

    const usedCard = this.createMetricCard('Used Capacity', '--', 'accent');
    usedCard.className += ' col-4';
    grid.appendChild(usedCard);

    const utilCard = this.createMetricCard('Avg Utilization', '--', '');
    utilCard.className += ' col-4';
    grid.appendChild(utilCard);

    // Gauges panel
    const gaugePanel = document.createElement('div');
    gaugePanel.className = 'panel col-12';
    const gaugeHeader = document.createElement('div');
    gaugeHeader.className = 'panel-header';
    gaugeHeader.textContent = 'Memory Tier Utilization';
    gaugePanel.appendChild(gaugeHeader);
    this.gaugesEl = document.createElement('div');
    this.gaugesEl.className = 'panel-body';
    this.gaugesEl.style.display = 'flex';
    this.gaugesEl.style.justifyContent = 'center';
    this.gaugesEl.style.gap = '48px';
    this.gaugesEl.style.padding = '24px';
    gaugePanel.appendChild(this.gaugesEl);
    grid.appendChild(gaugePanel);

    // Detail table
    const detailPanel = document.createElement('div');
    detailPanel.className = 'panel col-12';
    const detailHeader = document.createElement('div');
    detailHeader.className = 'panel-header';
    detailHeader.textContent = 'Tier Details';
    detailPanel.appendChild(detailHeader);
    this.detailEl = document.createElement('div');
    this.detailEl.style.padding = '0';
    detailPanel.appendChild(this.detailEl);
    grid.appendChild(detailPanel);

    this.loadData(totalCard, usedCard, utilCard);
    this.pollTimer = setInterval(() => {
      this.loadData(totalCard, usedCard, utilCard);
    }, 5000);
  }

  private createMetricCard(label: string, value: string, modifier: string): HTMLElement {
    const card = document.createElement('div');
    card.className = 'metric-card';
    card.innerHTML = `
      <span class="metric-label">${label}</span>
      <span class="metric-value ${modifier}" data-metric>${value}</span>
    `;
    return card;
  }

  private async loadData(
    totalCard: HTMLElement,
    usedCard: HTMLElement,
    utilCard: HTMLElement,
  ): Promise<void> {
    let tiers: MemoryTiers;

    try {
      tiers = await fetchMemoryTiers();
    } catch {
      tiers = {
        small: { used: 42, total: 64 },
        medium: { used: 288, total: 512 },
        large: { used: 1843, total: 8192 },
      };
    }

    const totalUsed = tiers.small.used + tiers.medium.used + tiers.large.used;
    const totalCap = tiers.small.total + tiers.medium.total + tiers.large.total;
    const avgUtil = totalCap > 0 ? totalUsed / totalCap : 0;

    const tVal = totalCard.querySelector('[data-metric]');
    if (tVal) tVal.textContent = `${totalCap} MB`;

    const uVal = usedCard.querySelector('[data-metric]');
    if (uVal) uVal.textContent = `${totalUsed} MB`;

    const aVal = utilCard.querySelector('[data-metric]');
    if (aVal) {
      aVal.textContent = `${(avgUtil * 100).toFixed(1)}%`;
      aVal.className = `metric-value ${avgUtil > 0.9 ? 'critical' : avgUtil > 0.7 ? 'warning' : 'success'}`;
    }

    this.renderGauges(tiers);
    this.renderDetail(tiers);
  }

  private renderGauges(tiers: MemoryTiers): void {
    if (!this.gaugesEl) return;
    this.gaugesEl.innerHTML = '';

    const tierData = [
      { label: 'S - Hot / L1', sublabel: 'Cache', ...tiers.small, color: '#00E5FF' },
      { label: 'M - Warm / HNSW', sublabel: 'Index', ...tiers.medium, color: '#2ECC71' },
      { label: 'L - Cold / Disk', sublabel: 'Segments', ...tiers.large, color: '#FFB020' },
    ];

    for (const t of tierData) {
      const pct = t.total > 0 ? t.used / t.total : 0;
      const gauge = document.createElement('div');
      gauge.className = 'gauge';
      gauge.style.width = '140px';

      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('viewBox', '0 0 80 80');
      svg.classList.add('gauge-ring');

      const bg = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
      bg.setAttribute('cx', '40');
      bg.setAttribute('cy', '40');
      bg.setAttribute('r', '34');
      bg.setAttribute('fill', 'none');
      bg.setAttribute('stroke', '#1E2630');
      bg.setAttribute('stroke-width', '4');
      svg.appendChild(bg);

      const circ = 2 * Math.PI * 34;
      const fg = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
      fg.setAttribute('cx', '40');
      fg.setAttribute('cy', '40');
      fg.setAttribute('r', '34');
      fg.setAttribute('fill', 'none');
      fg.setAttribute('stroke', pct > 0.9 ? '#FF4D4D' : pct > 0.7 ? '#FFB020' : t.color);
      fg.setAttribute('stroke-width', '4');
      fg.setAttribute('stroke-dasharray', `${circ * pct} ${circ * (1 - pct)}`);
      fg.setAttribute('stroke-dashoffset', `${circ * 0.25}`);
      fg.setAttribute('stroke-linecap', 'round');
      svg.appendChild(fg);

      const txt = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      txt.setAttribute('x', '40');
      txt.setAttribute('y', '42');
      txt.setAttribute('text-anchor', 'middle');
      txt.setAttribute('fill', '#E6EDF3');
      txt.setAttribute('font-size', '13');
      txt.setAttribute('font-weight', '500');
      txt.setAttribute('font-family', '"JetBrains Mono", monospace');
      txt.textContent = `${(pct * 100).toFixed(0)}%`;
      svg.appendChild(txt);

      gauge.appendChild(svg);

      const label = document.createElement('div');
      label.className = 'gauge-label';
      label.style.textAlign = 'center';
      label.style.lineHeight = '1.4';
      label.innerHTML = `${t.label}<br><span style="color:#484F58;font-size:9px">${t.used} / ${t.total} MB</span>`;
      gauge.appendChild(label);

      this.gaugesEl.appendChild(gauge);
    }
  }

  private renderDetail(tiers: MemoryTiers): void {
    if (!this.detailEl) return;

    const rows = [
      { tier: 'S', name: 'Hot / L1 Cache', ...tiers.small, latency: '0.8 us' },
      { tier: 'M', name: 'Warm / HNSW Index', ...tiers.medium, latency: '12.4 us' },
      { tier: 'L', name: 'Cold / Disk Segments', ...tiers.large, latency: '450 us' },
    ];

    this.detailEl.innerHTML = `
      <table class="data-table">
        <thead>
          <tr>
            <th>Tier</th>
            <th>Name</th>
            <th>Used</th>
            <th>Capacity</th>
            <th>Utilization</th>
            <th>Avg Latency</th>
          </tr>
        </thead>
        <tbody>
          ${rows.map((r) => {
            const pct = r.total > 0 ? r.used / r.total : 0;
            const cls = pct > 0.9 ? 'critical' : pct > 0.7 ? 'warning' : 'success';
            return `<tr>
              <td>${r.tier}</td>
              <td style="font-family:var(--font-sans)">${r.name}</td>
              <td>${r.used} MB</td>
              <td>${r.total} MB</td>
              <td><span class="score-badge score-${pct > 0.7 ? (pct > 0.9 ? 'low' : 'medium') : 'high'}">${(pct * 100).toFixed(1)}%</span></td>
              <td>${r.latency}</td>
            </tr>`;
          }).join('')}
        </tbody>
      </table>
    `;
  }

  unmount(): void {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    this.gaugesEl = null;
    this.detailEl = null;
    this.container = null;
  }
}
