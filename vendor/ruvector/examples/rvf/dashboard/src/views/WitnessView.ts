import { fetchWitnessLog, WitnessLogEntry as ApiWitnessEntry, WitnessLogResponse } from '../api';
import { onEvent, LiveEvent } from '../ws';

interface WitnessEntry {
  timestamp: string;
  type: string;
  witness: string;
  action: string;
  hash: string;
  prevHash: string;
  coherence: number;
  measurement: string | null;
  epoch: number;
}

const TYPE_COLORS: Record<string, string> = {
  seal: '#FF4D4D',
  commit: '#00E5FF',
  merge: '#FFB020',
  verify: '#2ECC71',
};

const TYPE_LABELS: Record<string, string> = {
  seal: 'Chain anchor — immutable genesis point',
  commit: 'New evidence committed to chain',
  merge: 'Branch merge — combining data sources',
  verify: 'Verification step — confirms integrity',
};

export class WitnessView {
  private container: HTMLElement | null = null;
  private logEl: HTMLElement | null = null;
  private chainCanvas: HTMLCanvasElement | null = null;
  private coherenceCanvas: HTMLCanvasElement | null = null;
  private detailEl: HTMLElement | null = null;
  private metricsEls: Record<string, HTMLElement> = {};
  private unsubWs: (() => void) | null = null;
  private entries: WitnessEntry[] = [];
  private selectedIdx = -1;
  private chainMeta: { integrity: string; hashAlgo: string; rootHash: string; meanCoherence: number; minCoherence: number } = {
    integrity: '--', hashAlgo: 'SHAKE-256', rootHash: '--', meanCoherence: 0, minCoherence: 0,
  };

  mount(container: HTMLElement): void {
    this.container = container;

    const outer = document.createElement('div');
    outer.style.cssText = 'display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden';
    container.appendChild(outer);

    // Header
    const header = document.createElement('div');
    header.style.cssText = 'padding:12px 20px;border-bottom:1px solid var(--border);flex-shrink:0';
    header.innerHTML = `
      <div style="display:flex;align-items:center;gap:10px;margin-bottom:4px">
        <span style="font-size:14px;font-weight:600;color:var(--text-primary)">Witness Chain</span>
        <span style="font-size:10px;padding:2px 8px;border-radius:3px;background:rgba(46,204,113,0.1);color:#2ECC71;font-weight:600;text-transform:uppercase;letter-spacing:0.5px">SHAKE-256</span>
        <span style="font-size:10px;padding:2px 8px;border-radius:3px;background:rgba(0,229,255,0.1);color:#00E5FF;font-weight:600;text-transform:uppercase;letter-spacing:0.5px">Ed25519</span>
      </div>
      <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">
        Cryptographic audit trail proving the causal history of every RVF pipeline event.
        Each <strong>witness</strong> verifies a specific measurement (transit depth, stellar parameters, etc.).
        The chain is <strong>hash-linked</strong>: every entry's SHAKE-256 hash includes the previous entry's hash, making tampering detectable.
        <span style="color:#FF4D4D">Seal</span> = anchor,
        <span style="color:#00E5FF">Commit</span> = new evidence,
        <span style="color:#FFB020">Merge</span> = branch join,
        <span style="color:#2ECC71">Verify</span> = integrity confirmed.
      </div>
    `;
    outer.appendChild(header);

    // Metrics row
    const metricsRow = document.createElement('div');
    metricsRow.style.cssText = 'display:flex;gap:12px;padding:12px 20px;border-bottom:1px solid var(--border);flex-shrink:0;flex-wrap:wrap';
    const metricDefs = [
      { key: 'entries', label: 'Chain Length', color: 'var(--accent)' },
      { key: 'integrity', label: 'Integrity', color: '#2ECC71' },
      { key: 'coherence', label: 'Mean Coherence', color: '' },
      { key: 'minCoherence', label: 'Min Coherence', color: '' },
      { key: 'depth', label: 'Epochs', color: '' },
      { key: 'rootHash', label: 'Root Hash', color: 'var(--text-muted)' },
    ];
    for (const m of metricDefs) {
      const card = document.createElement('div');
      card.style.cssText = 'background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);padding:10px 14px;min-width:100px;flex:1';
      card.innerHTML = `
        <div style="font-size:10px;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:3px">${m.label}</div>
        <div data-metric="${m.key}" style="font-family:var(--font-mono);font-size:18px;font-weight:500;color:${m.color || 'var(--text-primary)'};line-height:1.2">--</div>
      `;
      metricsRow.appendChild(card);
      this.metricsEls[m.key] = card.querySelector(`[data-metric="${m.key}"]`) as HTMLElement;
    }
    outer.appendChild(metricsRow);

    // Main content area
    const content = document.createElement('div');
    content.style.cssText = 'flex:1;overflow:auto;padding:16px 20px;display:flex;flex-direction:column;gap:16px';
    outer.appendChild(content);

    // Info panel — 3 columns
    const infoPanel = document.createElement('div');
    infoPanel.style.cssText = 'display:grid;grid-template-columns:1fr 1fr 1fr;gap:12px;flex-shrink:0';
    infoPanel.innerHTML = `
      <div style="background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);padding:14px">
        <div style="font-size:11px;font-weight:600;color:var(--accent);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:8px">How It Works</div>
        <div style="font-size:11px;color:var(--text-secondary);line-height:1.6">
          Each pipeline stage produces a <strong>witness entry</strong> containing: the measurement taken, a confidence score (coherence),
          and a cryptographic hash that chains to the previous entry. This creates an immutable, tamper-evident record of the entire
          scientific analysis — from raw photometry to final candidate ranking.
        </div>
      </div>
      <div style="background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);padding:14px">
        <div style="font-size:11px;font-weight:600;color:#FFB020;text-transform:uppercase;letter-spacing:0.5px;margin-bottom:8px">Hash Linking</div>
        <div style="font-size:11px;color:var(--text-secondary);line-height:1.6">
          SHAKE-256 (variable-length SHA-3 family) hashes each entry including the previous hash, creating a <strong>Merkle chain</strong>.
          If any entry is modified, all subsequent hashes become invalid. The final entry is signed with <strong>Ed25519</strong>
          to prove chain authorship and prevent repudiation.
        </div>
      </div>
      <div style="background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);padding:14px">
        <div style="font-size:11px;font-weight:600;color:#2ECC71;text-transform:uppercase;letter-spacing:0.5px;margin-bottom:8px">Coherence Score</div>
        <div style="font-size:11px;color:var(--text-secondary);line-height:1.6">
          Each witness reports a <strong>coherence</strong> value (0–1) indicating how well the new evidence agrees with prior chain state.
          Values < 0.90 are flagged as <span style="color:#FFB020">amber</span> (potential anomaly).
          The coherence chart below shows how confidence evolves across the pipeline, highlighting where uncertainty enters.
        </div>
      </div>
    `;
    content.appendChild(infoPanel);

    // Chain visualization (canvas)
    const chainPanel = document.createElement('div');
    chainPanel.style.cssText = 'background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;flex-shrink:0';
    const chainHeader = document.createElement('div');
    chainHeader.style.cssText = 'padding:10px 14px;font-size:11px;font-weight:500;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.6px;border-bottom:1px solid var(--border);display:flex;justify-content:space-between;align-items:center';
    chainHeader.innerHTML = '<span>Chain Topology</span><span style="font-size:10px;color:var(--text-muted);font-family:var(--font-mono)">Click a node for details</span>';
    chainPanel.appendChild(chainHeader);
    this.chainCanvas = document.createElement('canvas');
    this.chainCanvas.style.cssText = 'width:100%;height:120px;display:block;cursor:pointer';
    this.chainCanvas.addEventListener('click', (e) => this.onChainClick(e));
    chainPanel.appendChild(this.chainCanvas);
    content.appendChild(chainPanel);

    // Detail panel (shows on node click)
    this.detailEl = document.createElement('div');
    this.detailEl.style.cssText = 'background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);padding:14px;flex-shrink:0;display:none';
    content.appendChild(this.detailEl);

    // Coherence chart
    const cohPanel = document.createElement('div');
    cohPanel.style.cssText = 'background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;flex-shrink:0';
    const cohHeader = document.createElement('div');
    cohHeader.style.cssText = 'padding:10px 14px;font-size:11px;font-weight:500;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.6px;border-bottom:1px solid var(--border);display:flex;justify-content:space-between;align-items:center';
    cohHeader.innerHTML = '<span>Coherence Evolution</span><span style="font-size:10px;color:var(--text-muted);font-family:var(--font-mono)">Dashed line = 0.90 threshold</span>';
    cohPanel.appendChild(cohHeader);
    this.coherenceCanvas = document.createElement('canvas');
    this.coherenceCanvas.style.cssText = 'width:100%;height:140px;display:block';
    cohPanel.appendChild(this.coherenceCanvas);
    content.appendChild(cohPanel);

    // Witness log
    const logPanel = document.createElement('div');
    logPanel.style.cssText = 'background:var(--bg-panel);border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;flex:1;min-height:200px;display:flex;flex-direction:column';
    const logHeader = document.createElement('div');
    logHeader.style.cssText = 'padding:10px 14px;font-size:11px;font-weight:500;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.6px;border-bottom:1px solid var(--border);display:flex;justify-content:space-between;align-items:center;flex-shrink:0';
    logHeader.innerHTML = '<span>Witness Log</span><span style="font-size:10px;color:var(--text-muted);font-family:var(--font-mono)">Hash-linked entries</span>';
    logPanel.appendChild(logHeader);
    // Column headers
    const colHeaders = document.createElement('div');
    colHeaders.style.cssText = 'display:flex;align-items:center;gap:10px;padding:6px 14px;border-bottom:1px solid var(--border);font-size:10px;font-weight:500;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;flex-shrink:0';
    colHeaders.innerHTML = `
      <span style="min-width:60px">Time</span>
      <span style="min-width:52px">Type</span>
      <span style="min-width:90px">Witness</span>
      <span style="flex:1">Action</span>
      <span style="min-width:50px;text-align:right">Coh.</span>
      <span style="min-width:100px;text-align:right">Hash</span>
    `;
    logPanel.appendChild(colHeaders);
    this.logEl = document.createElement('div');
    this.logEl.style.cssText = 'flex:1;overflow-y:auto;font-family:var(--font-mono);font-size:11px';
    logPanel.appendChild(this.logEl);
    content.appendChild(logPanel);

    this.loadData();

    this.unsubWs = onEvent((ev: LiveEvent) => {
      if (ev.event_type === 'witness') {
        this.addLiveEntry(ev);
      }
    });
  }

  private async loadData(): Promise<void> {
    let log: WitnessLogResponse;
    try {
      log = await fetchWitnessLog();
    } catch {
      log = {
        entries: [], chain_length: 0, integrity: '--', hash_algorithm: 'SHAKE-256',
        root_hash: '--', genesis_hash: '--', mean_coherence: 0, min_coherence: 0, total_epochs: 0,
      };
    }

    this.chainMeta = {
      integrity: log.integrity,
      hashAlgo: log.hash_algorithm,
      rootHash: log.root_hash,
      meanCoherence: log.mean_coherence,
      minCoherence: log.min_coherence,
    };

    this.entries = log.entries.map((e: ApiWitnessEntry) => ({
      timestamp: e.timestamp.includes('T') ? e.timestamp.split('T')[1]?.substring(0, 8) ?? '' : e.timestamp,
      type: e.type,
      witness: e.witness,
      action: e.action,
      hash: e.hash,
      prevHash: e.prev_hash,
      coherence: e.coherence,
      measurement: e.measurement,
      epoch: e.epoch,
    }));

    if (this.entries.length === 0) {
      this.entries = this.generateDemoEntries();
    }

    this.updateMetrics(log);
    this.renderChain();
    this.renderCoherence();
    this.renderLog();
  }

  private updateMetrics(log: WitnessLogResponse): void {
    const set = (key: string, val: string) => {
      const el = this.metricsEls[key];
      if (el) el.textContent = val;
    };
    set('entries', String(this.entries.length));
    set('integrity', this.chainMeta.integrity);
    set('coherence', this.chainMeta.meanCoherence > 0 ? this.chainMeta.meanCoherence.toFixed(4) : '--');
    set('minCoherence', this.chainMeta.minCoherence > 0 ? this.chainMeta.minCoherence.toFixed(4) : '--');
    set('depth', String(log.total_epochs));
    set('rootHash', this.chainMeta.rootHash.substring(0, 12) + '...');

    // Color the min coherence if below threshold
    const minEl = this.metricsEls['minCoherence'];
    if (minEl && this.chainMeta.minCoherence > 0 && this.chainMeta.minCoherence < 0.9) {
      minEl.style.color = '#FFB020';
    }
    // Color integrity
    const intEl = this.metricsEls['integrity'];
    if (intEl) {
      intEl.style.color = this.chainMeta.integrity === 'VALID' ? '#2ECC71' : '#FF4D4D';
    }
  }

  private renderChain(): void {
    const canvas = this.chainCanvas;
    if (!canvas) return;

    const rect = canvas.parentElement?.getBoundingClientRect();
    const w = rect?.width ?? 800;
    const h = 120;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, w, h);

    const n = this.entries.length;
    if (n === 0) return;

    const padX = 40;
    const padY = 20;
    const innerW = w - padX * 2;
    const midY = h / 2;
    const nodeR = 8;

    // Draw connecting lines first
    for (let i = 0; i < n - 1; i++) {
      const x1 = padX + (i / (n - 1)) * innerW;
      const x2 = padX + ((i + 1) / (n - 1)) * innerW;
      ctx.beginPath();
      ctx.moveTo(x1 + nodeR, midY);
      ctx.lineTo(x2 - nodeR, midY);
      ctx.strokeStyle = '#1E2630';
      ctx.lineWidth = 2;
      ctx.stroke();

      // Arrow head
      const ax = x2 - nodeR - 6;
      ctx.beginPath();
      ctx.moveTo(ax, midY - 3);
      ctx.lineTo(ax + 6, midY);
      ctx.lineTo(ax, midY + 3);
      ctx.fillStyle = '#1E2630';
      ctx.fill();
    }

    // Draw nodes
    for (let i = 0; i < n; i++) {
      const entry = this.entries[i];
      const x = padX + (n > 1 ? (i / (n - 1)) * innerW : innerW / 2);
      const color = TYPE_COLORS[entry.type] ?? '#00E5FF';
      const isSelected = i === this.selectedIdx;

      // Glow for selected
      if (isSelected) {
        ctx.beginPath();
        ctx.arc(x, midY, nodeR + 4, 0, Math.PI * 2);
        ctx.fillStyle = color.replace(')', ', 0.15)').replace('rgb', 'rgba').replace('#', '');
        // Use a simpler glow approach
        ctx.shadowColor = color;
        ctx.shadowBlur = 12;
        ctx.fill();
        ctx.shadowBlur = 0;
      }

      // Outer ring
      ctx.beginPath();
      ctx.arc(x, midY, nodeR, 0, Math.PI * 2);
      ctx.fillStyle = isSelected ? color : 'transparent';
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.fill();
      ctx.stroke();

      // Inner dot
      if (!isSelected) {
        ctx.beginPath();
        ctx.arc(x, midY, 3, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.fill();
      }

      // Witness label (above)
      ctx.fillStyle = '#8B949E';
      ctx.font = '9px monospace';
      ctx.textAlign = 'center';
      const label = entry.witness.replace('W_', '');
      ctx.fillText(label, x, midY - nodeR - padY + 8);

      // Coherence label (below)
      ctx.fillStyle = entry.coherence < 0.9 ? '#FFB020' : '#484F58';
      ctx.font = '9px monospace';
      ctx.fillText(entry.coherence.toFixed(2), x, midY + nodeR + 14);

      // Hash snippet (below coherence)
      ctx.fillStyle = '#30363D';
      ctx.font = '8px monospace';
      ctx.fillText(entry.hash.substring(0, 6), x, midY + nodeR + 24);
    }
  }

  private onChainClick(e: MouseEvent): void {
    const canvas = this.chainCanvas;
    if (!canvas || this.entries.length === 0) return;

    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const n = this.entries.length;
    const padX = 40;
    const innerW = rect.width - padX * 2;

    let closest = -1;
    let minDist = Infinity;
    for (let i = 0; i < n; i++) {
      const nx = padX + (n > 1 ? (i / (n - 1)) * innerW : innerW / 2);
      const dist = Math.abs(x - nx);
      if (dist < minDist && dist < 20) {
        minDist = dist;
        closest = i;
      }
    }

    if (closest >= 0) {
      this.selectedIdx = closest === this.selectedIdx ? -1 : closest;
      this.renderChain();
      this.showDetail(this.selectedIdx >= 0 ? this.entries[this.selectedIdx] : null);
    }
  }

  private showDetail(entry: WitnessEntry | null): void {
    if (!this.detailEl) return;
    if (!entry) {
      this.detailEl.style.display = 'none';
      return;
    }

    const color = TYPE_COLORS[entry.type] ?? '#00E5FF';
    const typeDesc = TYPE_LABELS[entry.type] ?? '';
    const cohColor = entry.coherence < 0.9 ? '#FFB020' : '#2ECC71';

    this.detailEl.style.display = 'block';
    this.detailEl.innerHTML = `
      <div style="display:flex;align-items:center;gap:12px;margin-bottom:12px">
        <div style="width:10px;height:10px;border-radius:50%;background:${color}"></div>
        <span style="font-size:13px;font-weight:600;color:var(--text-primary);font-family:var(--font-mono)">${entry.witness}</span>
        <span style="font-size:10px;padding:2px 8px;border-radius:3px;background:${color}22;color:${color};font-weight:600;text-transform:uppercase">${entry.type}</span>
        <span style="font-size:10px;color:var(--text-muted)">${typeDesc}</span>
        <span style="margin-left:auto;font-size:11px;color:var(--text-muted);font-family:var(--font-mono)">Epoch ${entry.epoch}</span>
      </div>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
        <div>
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;margin-bottom:4px">Action</div>
          <div style="font-size:12px;color:var(--text-primary);line-height:1.5">${entry.action}</div>
        </div>
        <div>
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;margin-bottom:4px">Measurement</div>
          <div style="font-size:12px;color:var(--accent);font-family:var(--font-mono)">${entry.measurement ?? 'N/A'}</div>
        </div>
      </div>
      <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:12px;margin-top:12px;padding-top:12px;border-top:1px solid var(--border)">
        <div>
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;margin-bottom:2px">Coherence</div>
          <div style="font-size:16px;font-weight:500;color:${cohColor};font-family:var(--font-mono)">${entry.coherence.toFixed(4)}</div>
        </div>
        <div>
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;margin-bottom:2px">Hash</div>
          <div style="font-size:11px;color:var(--text-primary);font-family:var(--font-mono)">${entry.hash}</div>
        </div>
        <div>
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;margin-bottom:2px">Previous Hash</div>
          <div style="font-size:11px;color:var(--text-muted);font-family:var(--font-mono)">${entry.prevHash}</div>
        </div>
      </div>
    `;
  }

  private renderCoherence(): void {
    const canvas = this.coherenceCanvas;
    if (!canvas) return;

    const rect = canvas.parentElement?.getBoundingClientRect();
    const w = rect?.width ?? 800;
    const h = 140;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, w, h);

    const n = this.entries.length;
    if (n === 0) return;

    const padL = 50, padR = 20, padT = 16, padB = 28;
    const iW = w - padL - padR;
    const iH = h - padT - padB;

    // Y axis: 0.80 to 1.00
    const yMin = 0.80, yMax = 1.01;
    const toX = (i: number) => padL + (n > 1 ? (i / (n - 1)) * iW : iW / 2);
    const toY = (v: number) => padT + (1 - (v - yMin) / (yMax - yMin)) * iH;

    // Grid lines
    ctx.strokeStyle = '#161C24';
    ctx.lineWidth = 1;
    for (let v = 0.80; v <= 1.001; v += 0.05) {
      const y = toY(v);
      ctx.beginPath();
      ctx.moveTo(padL, y);
      ctx.lineTo(w - padR, y);
      ctx.stroke();

      // Label
      ctx.fillStyle = '#484F58';
      ctx.font = '10px monospace';
      ctx.textAlign = 'right';
      ctx.fillText(v.toFixed(2), padL - 6, y + 4);
    }

    // Threshold line at 0.90
    ctx.setLineDash([4, 4]);
    ctx.strokeStyle = '#FFB02066';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(padL, toY(0.90));
    ctx.lineTo(w - padR, toY(0.90));
    ctx.stroke();
    ctx.setLineDash([]);

    // Label threshold
    ctx.fillStyle = '#FFB020';
    ctx.font = '9px monospace';
    ctx.textAlign = 'left';
    ctx.fillText('threshold', w - padR - 55, toY(0.90) - 4);

    // Fill area under coherence line
    ctx.beginPath();
    ctx.moveTo(toX(0), toY(yMin));
    for (let i = 0; i < n; i++) {
      ctx.lineTo(toX(i), toY(Math.max(yMin, this.entries[i].coherence)));
    }
    ctx.lineTo(toX(n - 1), toY(yMin));
    ctx.closePath();
    const grad = ctx.createLinearGradient(0, padT, 0, padT + iH);
    grad.addColorStop(0, 'rgba(0, 229, 255, 0.08)');
    grad.addColorStop(1, 'rgba(0, 229, 255, 0.01)');
    ctx.fillStyle = grad;
    ctx.fill();

    // Coherence line
    ctx.beginPath();
    for (let i = 0; i < n; i++) {
      const x = toX(i);
      const y = toY(Math.max(yMin, this.entries[i].coherence));
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    }
    ctx.strokeStyle = '#00E5FF';
    ctx.lineWidth = 2;
    ctx.stroke();

    // Data points
    for (let i = 0; i < n; i++) {
      const x = toX(i);
      const coh = Math.max(yMin, this.entries[i].coherence);
      const y = toY(coh);
      const color = this.entries[i].coherence < 0.9
        ? '#FFB020'
        : TYPE_COLORS[this.entries[i].type] ?? '#00E5FF';

      ctx.beginPath();
      ctx.arc(x, y, 4, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
      ctx.strokeStyle = '#0B0F14';
      ctx.lineWidth = 1.5;
      ctx.stroke();

      // X-axis labels
      ctx.fillStyle = '#484F58';
      ctx.font = '8px monospace';
      ctx.textAlign = 'center';
      const label = this.entries[i].witness.replace('W_', '');
      if (n <= 20 || i % 2 === 0) {
        ctx.fillText(label, x, h - padB + 14);
      }
    }
  }

  private renderLog(): void {
    if (!this.logEl) return;
    this.logEl.innerHTML = '';

    for (let i = 0; i < this.entries.length; i++) {
      this.appendLogEntry(this.entries[i], i);
    }
  }

  private appendLogEntry(entry: WitnessEntry, idx: number): void {
    if (!this.logEl) return;
    const row = document.createElement('div');
    row.style.cssText = 'display:flex;align-items:center;gap:10px;padding:6px 14px;border-bottom:1px solid var(--border-subtle);cursor:pointer;transition:background 0.1s';
    row.addEventListener('mouseenter', () => { row.style.background = 'rgba(255,255,255,0.015)'; });
    row.addEventListener('mouseleave', () => { row.style.background = idx === this.selectedIdx ? 'rgba(0,229,255,0.04)' : ''; });
    row.addEventListener('click', () => {
      this.selectedIdx = idx === this.selectedIdx ? -1 : idx;
      this.renderChain();
      this.showDetail(this.selectedIdx >= 0 ? this.entries[this.selectedIdx] : null);
      // Highlight the row
      const rows = this.logEl?.children;
      if (rows) {
        for (let r = 0; r < rows.length; r++) {
          (rows[r] as HTMLElement).style.background = r === this.selectedIdx ? 'rgba(0,229,255,0.04)' : '';
        }
      }
    });

    const color = TYPE_COLORS[entry.type] ?? '#00E5FF';
    const cohColor = entry.coherence < 0.9 ? '#FFB020' : '#484F58';

    row.innerHTML = `
      <span style="color:var(--text-muted);min-width:60px;white-space:nowrap;font-size:10px">${entry.timestamp}</span>
      <span style="padding:2px 8px;border-radius:3px;font-size:9px;font-weight:600;text-transform:uppercase;letter-spacing:0.3px;min-width:52px;text-align:center;background:${color}18;color:${color}">${entry.type}</span>
      <span style="color:var(--accent);min-width:90px;font-size:11px">${entry.witness}</span>
      <span style="color:var(--text-primary);flex:1;font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${entry.action}">${entry.action}</span>
      <span style="color:${cohColor};min-width:50px;text-align:right;font-size:11px">${entry.coherence.toFixed(2)}</span>
      <span style="color:var(--text-muted);font-size:10px;min-width:100px;text-align:right" title="Hash: ${entry.hash} | Prev: ${entry.prevHash}">${entry.hash.substring(0, 8)}..${entry.prevHash.substring(0, 4)}</span>
    `;
    this.logEl.appendChild(row);
  }

  private addLiveEntry(ev: LiveEvent): void {
    const entry: WitnessEntry = {
      timestamp: new Date(ev.timestamp * 1000).toISOString().substring(11, 19),
      type: String(ev.data['type'] ?? 'commit'),
      witness: String(ev.data['witness'] ?? 'W_live'),
      action: String(ev.data['action'] ?? 'live_event'),
      hash: String(ev.data['hash'] ?? this.fakeHash('live')),
      prevHash: this.entries.length > 0 ? this.entries[this.entries.length - 1].hash : '0000000000000000',
      coherence: Number(ev.data['coherence'] ?? 1.0),
      measurement: ev.data['measurement'] ? String(ev.data['measurement']) : null,
      epoch: this.entries.length,
    };
    this.entries.push(entry);
    this.appendLogEntry(entry, this.entries.length - 1);
    this.renderChain();
    this.renderCoherence();

    // Update entry count metric
    const el = this.metricsEls['entries'];
    if (el) el.textContent = String(this.entries.length);

    // Auto-scroll log
    if (this.logEl) {
      this.logEl.scrollTop = this.logEl.scrollHeight;
    }
  }

  private fakeHash(seed: string): string {
    let h = 0;
    for (let i = 0; i < seed.length; i++) {
      h = ((h << 5) - h + seed.charCodeAt(i)) | 0;
    }
    return Math.abs(h).toString(16).padStart(16, '0').substring(0, 16);
  }

  private generateDemoEntries(): WitnessEntry[] {
    const witnesses = [
      { w: 'W_root', t: 'seal', a: 'Chain initialized — genesis anchor', m: null },
      { w: 'W_photometry', t: 'commit', a: 'Kepler light curves ingested (196K targets)', m: 'transit_depth_rms=4.2e-5' },
      { w: 'W_periodogram', t: 'commit', a: 'BLS search completed — 2,842 signals', m: 'bls_power_max=42.7' },
      { w: 'W_stellar', t: 'commit', a: 'Stellar parameters derived (Gaia DR3)', m: 'T_eff_sigma=47K' },
      { w: 'W_transit', t: 'merge', a: 'Transit model merged with stellar params', m: 'R_p_range=0.92-2.61' },
      { w: 'W_radial_velocity', t: 'commit', a: 'HARPS RV data — mass constraints', m: 'K_rv_range=0.089-3.2' },
      { w: 'W_orbit', t: 'commit', a: 'Orbital solutions — HZ classification', m: 'hz_candidates=10' },
      { w: 'W_esi', t: 'commit', a: 'ESI ranking computed', m: 'esi_top=0.93' },
      { w: 'W_spectroscopy', t: 'merge', a: 'JWST atmospheric observations merged', m: 'CH4+CO2_detected' },
      { w: 'W_biosig', t: 'commit', a: 'Biosignature scoring pipeline', m: 'diseq_max=0.82' },
      { w: 'W_blind', t: 'commit', a: 'Blind test passed (τ=1.0)', m: 'kendall_tau=1.000' },
      { w: 'W_seal', t: 'verify', a: 'Chain sealed — Ed25519 signed', m: 'chain_length=12' },
    ];
    let prevHash = '0000000000000000';
    return witnesses.map((w, i) => {
      const hash = this.fakeHash(w.w + i);
      const entry: WitnessEntry = {
        timestamp: new Date(Date.now() - (witnesses.length - i) * 120000).toISOString().substring(11, 19),
        type: w.t,
        witness: w.w,
        action: w.a,
        hash,
        prevHash,
        coherence: 1.0 - i * 0.01,
        measurement: w.m,
        epoch: i,
      };
      prevHash = hash;
      return entry;
    });
  }

  unmount(): void {
    this.unsubWs?.();
    this.logEl = null;
    this.chainCanvas = null;
    this.coherenceCanvas = null;
    this.detailEl = null;
    this.metricsEls = {};
    this.container = null;
    this.entries = [];
    this.selectedIdx = -1;
  }
}
