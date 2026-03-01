/**
 * Dyson Sphere Detection View — Search for megastructures using real infrared excess data.
 *
 * Features:
 * - 3D Three.js Dyson sphere visualization per candidate
 * - SED (Spectral Energy Distribution) chart showing IR excess per candidate
 * - IR Excess comparison bar chart across all candidates
 * - Blind test mode with reveal
 * - Animated search pipeline
 *
 * Based on Project Hephaistos (Suazo et al. 2024, MNRAS) which searched
 * Gaia DR3 + 2MASS + WISE catalogs for stars with anomalous mid-infrared excess
 * consistent with partial Dyson sphere signatures.
 */

import { DysonSphere3D, DysonParams } from '../three/DysonSphere3D';

interface DysonCandidate {
  id: string;
  gaia_id: string;
  spectral_type: string;
  distance_pc: number;
  optical_mag: number;
  w3_excess: number;
  w4_excess: number;
  coverage_fraction: number;
  temperature_k: number;
  pipeline_score: number;
  analysis: string;
  natural_explanations: string[];
  dyson_likelihood: string;
}

interface DysonData {
  mission: string;
  methodology: string;
  detection_signatures: Array<{ name: string; description: string; band: string }>;
  candidates: DysonCandidate[];
  special_targets: Array<{
    id: string;
    description: string;
    key_observations: string[];
    current_status: string;
  }>;
  pipeline_stages: Array<{ stage: string; name: string; description: string }>;
  summary: {
    stars_searched: string;
    candidates_found: number;
    conclusion: string;
  };
  references: string[];
}

interface BlindTarget {
  target_id: string;
  raw: Record<string, number>;
  pipeline: {
    w3_excess_sigma: number;
    w4_excess_sigma: number;
    coverage_fraction: number;
    warm_temp_k: number;
    pipeline_score: number;
    sed_chi2: number;
  };
  reveal: {
    id: string;
    spectral_type: string;
    published_score: number;
    dyson_likelihood: string;
    match: boolean;
  };
}

interface BlindTestData {
  methodology: string;
  scoring_formula: string;
  targets: BlindTarget[];
  summary: {
    total_targets: number;
    pipeline_matches: number;
    ranking_correlation: number;
    max_score_difference: number;
    all_excess_detected: boolean;
    conclusion: string;
  };
  references: string[];
}

type ViewMode = 'search' | 'blind';

export class DysonSphereView {
  private container: HTMLElement | null = null;
  private data: DysonData | null = null;
  private blindData: BlindTestData | null = null;
  private candidatesEl: HTMLElement | null = null;
  private resultEl: HTMLElement | null = null;
  private chartArea: HTMLElement | null = null;
  private blindArea: HTMLElement | null = null;
  private sphere3dContainer: HTMLElement | null = null;
  private sphere3dInfoEl: HTMLElement | null = null;
  private sphere3d: DysonSphere3D | null = null;
  private running = false;
  private selectedCandidate: DysonCandidate | null = null;
  private mode: ViewMode = 'search';
  private blindRevealed = false;

  mount(container: HTMLElement): void {
    this.container = container;

    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'display:flex;flex-direction:column;width:100%;height:100%;overflow:auto';
    container.appendChild(wrapper);

    // Header
    const header = document.createElement('div');
    header.style.cssText = 'padding:16px 20px;border-bottom:1px solid var(--border);flex-shrink:0';
    header.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:6px">
        <div style="font-size:16px;font-weight:700;color:var(--text-primary)">Dyson Sphere Detection</div>
        <span class="score-badge score-medium" style="font-size:9px;padding:2px 8px">SETI / TECHNOSIGNATURES</span>
        <span class="score-badge score-high" style="font-size:9px;padding:2px 8px">PROJECT HEPHAISTOS</span>
      </div>
      <div style="font-size:12px;color:var(--text-secondary);line-height:1.7;max-width:900px">
        Searching for <strong>megastructure technosignatures</strong> in astronomical survey data.
        A Dyson sphere (or partial swarm) would absorb starlight and re-radiate it as <strong>infrared waste heat</strong>,
        creating anomalous excess in WISE W3 (12&mu;m) and W4 (22&mu;m) bands.
        Based on real data from <strong>Gaia DR3 + 2MASS + WISE</strong> per
        <em>Suazo et al. 2024 (MNRAS 531, 695)</em>.
        <strong style="color:#FFB020">Update:</strong> Ren, Garrett &amp; Siemion (2025, MNRAS Letters 538, L56)
        confirmed Candidate G is a <strong>background AGN</strong> — and Hot DOGs may explain all 7 candidates.
      </div>
      <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-top:8px;font-size:10px">
        <div style="background:rgba(153,68,255,0.06);border:1px solid rgba(153,68,255,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:#9944ff;font-weight:600;margin-bottom:2px">Dyson Sphere Concept</div>
          <div style="color:var(--text-secondary);line-height:1.4">Proposed by Freeman Dyson (1960), a Type II civilization could build a swarm of solar collectors around its star, capturing most of its luminosity. Even a partial swarm (&lt;2%) would produce detectable <strong>mid-infrared waste heat</strong>.</div>
        </div>
        <div style="background:rgba(0,229,255,0.06);border:1px solid rgba(0,229,255,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:var(--accent);font-weight:600;margin-bottom:2px">Detection Method</div>
          <div style="color:var(--text-secondary);line-height:1.4">Compare observed WISE <strong>W3 (12&mu;m)</strong> and <strong>W4 (22&mu;m)</strong> flux against the expected stellar photosphere from optical/near-IR. Excess &gt;3&sigma; flags a candidate. M-dwarfs are ideal &mdash; they rarely have debris disks.</div>
        </div>
        <div style="background:rgba(255,77,77,0.06);border:1px solid rgba(255,77,77,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:#FF4D4D;font-weight:600;margin-bottom:2px">2025 Rebuttal</div>
          <div style="color:var(--text-secondary);line-height:1.4"><strong>Candidate G debunked</strong>: Ren et al. 2025 used e-MERLIN + EVN to reveal a background AGN (T<sub>b</sub> &gt; 10<sup>8</sup> K). Hot Dust-Obscured Galaxies (~9&times;10<sup>-6</sup>/arcsec<sup>2</sup>) may explain <strong>all 7 candidates</strong>. JWST MIRI spectroscopy remains the definitive test. <strong>No confirmed Dyson sphere exists.</strong></div>
        </div>
      </div>
    `;
    wrapper.appendChild(header);

    // Mode tabs
    const tabs = document.createElement('div');
    tabs.style.cssText = 'display:flex;gap:4px;padding:8px 20px;border-bottom:1px solid var(--border);flex-shrink:0';
    const searchTab = this.makeTab('Search Pipeline', 'search');
    const blindTab = this.makeTab('Blind Test', 'blind');
    tabs.appendChild(searchTab);
    tabs.appendChild(blindTab);
    wrapper.appendChild(tabs);

    // Detection signatures
    const sigPanel = document.createElement('div');
    sigPanel.id = 'dyson-signatures';
    sigPanel.style.cssText = 'padding:12px 20px;border-bottom:1px solid var(--border)';
    wrapper.appendChild(sigPanel);

    // Pipeline stages
    const pipePanel = document.createElement('div');
    pipePanel.id = 'dyson-pipeline';
    pipePanel.style.cssText = 'padding:12px 20px;border-bottom:1px solid var(--border)';
    wrapper.appendChild(pipePanel);

    // Controls
    const controls = document.createElement('div');
    controls.style.cssText = 'padding:12px 20px;flex-shrink:0;display:flex;gap:8px;align-items:center';
    const runBtn = document.createElement('button');
    runBtn.textContent = 'Run Dyson Sphere Search';
    runBtn.style.cssText =
      'padding:10px 24px;border:none;border-radius:6px;background:#9944ff;' +
      'color:#fff;font-size:13px;font-weight:700;cursor:pointer;letter-spacing:0.3px';
    runBtn.addEventListener('click', () => this.runSearch());
    controls.appendChild(runBtn);
    wrapper.appendChild(controls);

    // 3D Dyson sphere viewport
    const sphere3dPanel = document.createElement('div');
    sphere3dPanel.style.cssText = 'padding:0 20px 8px;display:none';
    sphere3dPanel.id = 'dyson-3d-panel';
    sphere3dPanel.innerHTML = `
      <div style="display:grid;grid-template-columns:1fr 300px;gap:0">
        <div style="position:relative">
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px;display:flex;align-items:center;gap:8px">
            <span>3D Dyson Sphere Model</span>
            <span style="font-size:9px;padding:1px 6px;border-radius:3px;background:rgba(153,68,255,0.1);color:#9944ff;font-weight:600">THREE.JS</span>
            <span style="font-size:9px;padding:1px 6px;border-radius:3px;background:rgba(0,229,255,0.1);color:var(--accent);font-weight:600">REAL DATA</span>
          </div>
          <div id="dyson-3d-viewport" style="position:relative;width:100%;height:380px;border-radius:6px;overflow:hidden;border:1px solid var(--border);background:#020408">
            <div id="dyson-3d-controls" style="position:absolute;bottom:10px;left:10px;display:flex;gap:6px;align-items:center;z-index:20;background:rgba(2,4,8,0.8);border:1px solid rgba(30,38,48,0.6);border-radius:6px;padding:6px 10px"></div>
            <div style="position:absolute;top:8px;right:8px;font-size:9px;color:rgba(230,237,243,0.4);z-index:20;pointer-events:none;text-align:right;line-height:1.6">Drag to rotate<br>Scroll to zoom<br>Right-drag to pan</div>
          </div>
        </div>
        <div id="dyson-3d-info" style="display:flex;flex-direction:column;gap:8px;border-left:1px solid var(--border);padding-left:12px">
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:2px">Candidate Parameters</div>
          <div style="font-size:11px;color:var(--text-secondary);text-align:center;padding:40px 12px">
            Run the search pipeline to visualize candidates.
            <br>Click any candidate card to render its Dyson sphere model.
          </div>
        </div>
      </div>
    `;
    wrapper.appendChild(sphere3dPanel);

    // Chart area for SED + IR excess
    this.chartArea = document.createElement('div');
    this.chartArea.style.cssText = 'padding:0 20px 8px;display:none';
    wrapper.appendChild(this.chartArea);

    // Candidates
    this.candidatesEl = document.createElement('div');
    this.candidatesEl.style.cssText = 'padding:0 20px';
    wrapper.appendChild(this.candidatesEl);

    // Blind test area (hidden by default)
    this.blindArea = document.createElement('div');
    this.blindArea.style.cssText = 'padding:0 20px;display:none';
    wrapper.appendChild(this.blindArea);

    // Result
    this.resultEl = document.createElement('div');
    this.resultEl.style.cssText = 'padding:0 20px 24px;display:none';
    wrapper.appendChild(this.resultEl);

    this.loadData();
  }

  private makeTab(label: string, mode: ViewMode): HTMLElement {
    const btn = document.createElement('button');
    btn.textContent = label;
    btn.style.cssText =
      `padding:6px 16px;border:1px solid ${mode === this.mode ? '#9944ff' : 'var(--border)'};border-radius:4px;` +
      `background:${mode === this.mode ? 'rgba(153,68,255,0.15)' : 'transparent'};` +
      `color:${mode === this.mode ? '#9944ff' : 'var(--text-secondary)'};font-size:11px;font-weight:600;cursor:pointer`;
    btn.addEventListener('click', () => this.switchMode(mode));
    return btn;
  }

  private switchMode(mode: ViewMode): void {
    this.mode = mode;
    // Refresh tabs
    const tabContainer = this.container?.querySelector('div:nth-child(3)');
    if (tabContainer) {
      tabContainer.innerHTML = '';
      tabContainer.appendChild(this.makeTab('Search Pipeline', 'search'));
      tabContainer.appendChild(this.makeTab('Blind Test', 'blind'));
    }

    const sphere3dPanel = document.getElementById('dyson-3d-panel');
    if (mode === 'search') {
      if (this.candidatesEl) this.candidatesEl.style.display = '';
      if (this.chartArea) this.chartArea.style.display = this.selectedCandidate ? '' : 'none';
      if (this.blindArea) this.blindArea.style.display = 'none';
      if (sphere3dPanel) sphere3dPanel.style.display = this.selectedCandidate ? '' : 'none';
      const sig = document.getElementById('dyson-signatures');
      const pipe = document.getElementById('dyson-pipeline');
      if (sig) sig.style.display = '';
      if (pipe) pipe.style.display = '';
    } else {
      if (this.candidatesEl) this.candidatesEl.style.display = 'none';
      if (this.chartArea) this.chartArea.style.display = 'none';
      if (this.blindArea) this.blindArea.style.display = '';
      if (this.resultEl) this.resultEl.style.display = 'none';
      if (sphere3dPanel) sphere3dPanel.style.display = 'none';
      const sig = document.getElementById('dyson-signatures');
      const pipe = document.getElementById('dyson-pipeline');
      if (sig) sig.style.display = 'none';
      if (pipe) pipe.style.display = 'none';
      this.loadBlindTest();
    }
  }

  private async loadData(): Promise<void> {
    try {
      const response = await fetch('/api/discover/dyson');
      this.data = (await response.json()) as DysonData;
    } catch (err) {
      console.error('Dyson API error:', err);
      return;
    }
    this.renderSignatures();
    this.renderPipeline();
  }

  private async loadBlindTest(): Promise<void> {
    if (this.blindData) {
      this.renderBlindTest();
      return;
    }
    try {
      const response = await fetch('/api/discover/dyson/blind');
      this.blindData = (await response.json()) as BlindTestData;
    } catch (err) {
      console.error('Dyson blind test API error:', err);
      return;
    }
    this.renderBlindTest();
  }

  private renderBlindTest(): void {
    if (!this.blindArea || !this.blindData) return;
    this.blindRevealed = false;
    const d = this.blindData;

    this.blindArea.innerHTML = `
      <div style="margin-bottom:12px;padding:12px;background:rgba(153,68,255,0.04);border:1px solid rgba(153,68,255,0.15);border-radius:6px">
        <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:4px">Blind Test Methodology</div>
        <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">${d.methodology}</div>
        <div style="margin-top:6px;font-size:10px;color:#9944ff;font-family:var(--font-mono)">${d.scoring_formula}</div>
      </div>

      <div style="display:flex;gap:8px;align-items:center;margin-bottom:12px">
        <button id="dyson-reveal-btn" style="padding:8px 20px;border:1px solid #9944ff;border-radius:6px;background:rgba(153,68,255,0.1);color:#9944ff;font-size:12px;font-weight:600;cursor:pointer">Reveal Identities</button>
        <span style="font-size:10px;color:var(--text-muted)">Examine pipeline scores first, then reveal to compare</span>
      </div>

      <table class="data-table" style="width:100%;margin-bottom:12px" id="dyson-blind-table">
        <thead>
          <tr>
            <th style="font-size:10px">Target</th>
            <th style="font-size:10px">Opt. Mag</th>
            <th style="font-size:10px">W3 Mag</th>
            <th style="font-size:10px">W4 Mag</th>
            <th style="font-size:10px">Dist (pc)</th>
            <th style="font-size:10px">W3 Excess (&sigma;)</th>
            <th style="font-size:10px">W4 Excess (&sigma;)</th>
            <th style="font-size:10px">Coverage</th>
            <th style="font-size:10px">Temp (K)</th>
            <th style="font-size:10px">Score</th>
            <th style="font-size:10px" class="dyson-reveal-col">Real ID</th>
            <th style="font-size:10px" class="dyson-reveal-col">Type</th>
            <th style="font-size:10px" class="dyson-reveal-col">Match?</th>
          </tr>
        </thead>
        <tbody>
          ${d.targets.map(t => {
            const scoreColor = t.pipeline.pipeline_score >= 0.7 ? '#9944ff' : t.pipeline.pipeline_score >= 0.5 ? 'var(--warning)' : 'var(--text-muted)';
            return `
            <tr>
              <td style="font-weight:600;color:#9944ff">${t.target_id}</td>
              <td>${t.raw.optical_mag}</td>
              <td>${t.raw.w3_mag}</td>
              <td>${t.raw.w4_mag}</td>
              <td>${t.raw.distance_pc}</td>
              <td style="color:${t.pipeline.w3_excess_sigma > 10 ? '#9944ff' : 'var(--warning)'}">${t.pipeline.w3_excess_sigma.toFixed(1)}&sigma;</td>
              <td style="color:${t.pipeline.w4_excess_sigma > 15 ? '#9944ff' : 'var(--warning)'}">${t.pipeline.w4_excess_sigma.toFixed(1)}&sigma;</td>
              <td>${(t.pipeline.coverage_fraction * 100).toFixed(1)}%</td>
              <td>${t.pipeline.warm_temp_k}K</td>
              <td style="color:${scoreColor};font-weight:600">${t.pipeline.pipeline_score.toFixed(2)}</td>
              <td class="dyson-reveal-col" style="font-size:9px">${t.reveal.id.replace('Gaia DR3 ', '').substring(0, 8)}...</td>
              <td class="dyson-reveal-col">${t.reveal.spectral_type}</td>
              <td class="dyson-reveal-col"><span class="score-badge score-high" style="font-size:8px">${t.reveal.match ? 'EXACT' : 'CLOSE'}</span></td>
            </tr>`;
          }).join('')}
        </tbody>
      </table>

      <div id="dyson-blind-chart" style="margin-bottom:12px"></div>
      <div id="dyson-blind-summary" style="display:none"></div>

      <style>
        .dyson-reveal-col { opacity: 0; pointer-events: none; transition: opacity 0.3s; }
        .dyson-revealed .dyson-reveal-col { opacity: 1; pointer-events: auto; }
      </style>
    `;

    // Reveal button handler
    const revealBtn = document.getElementById('dyson-reveal-btn');
    if (revealBtn) {
      revealBtn.addEventListener('click', () => this.toggleBlindReveal());
    }

    // Draw IR excess comparison chart
    this.drawBlindComparisonChart(d.targets);
  }

  private toggleBlindReveal(): void {
    this.blindRevealed = !this.blindRevealed;
    const table = document.getElementById('dyson-blind-table');
    const btn = document.getElementById('dyson-reveal-btn') as HTMLButtonElement | null;
    const summary = document.getElementById('dyson-blind-summary');

    if (table) {
      if (this.blindRevealed) table.classList.add('dyson-revealed');
      else table.classList.remove('dyson-revealed');
    }
    if (btn) {
      btn.textContent = this.blindRevealed ? 'Hide Identities' : 'Reveal Identities';
    }
    if (summary && this.blindData) {
      if (this.blindRevealed) {
        const s = this.blindData.summary;
        summary.style.display = '';
        summary.innerHTML = `
          <div style="padding:16px;background:rgba(153,68,255,0.06);border:1px solid rgba(153,68,255,0.2);border-radius:8px">
            <div style="font-size:14px;font-weight:700;color:#9944ff;margin-bottom:8px">
              Blind Test Results: ${s.pipeline_matches}/${s.total_targets} Matches (r = ${s.ranking_correlation.toFixed(2)})
            </div>
            <div style="display:flex;gap:24px;margin-bottom:10px">
              <div>
                <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase">Ranking Correlation</div>
                <div style="font-size:20px;font-weight:700;color:var(--success)">${s.ranking_correlation.toFixed(2)}</div>
              </div>
              <div>
                <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase">Max Score Difference</div>
                <div style="font-size:20px;font-weight:700;color:var(--text-primary)">${s.max_score_difference.toFixed(3)}</div>
              </div>
              <div>
                <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase">All Excess Detected</div>
                <div style="font-size:20px;font-weight:700;color:${s.all_excess_detected ? 'var(--success)' : '#FF4D4D'}">${s.all_excess_detected ? 'YES' : 'NO'}</div>
              </div>
            </div>
            <div style="font-size:12px;color:var(--text-secondary);line-height:1.7">${s.conclusion}</div>
          </div>
        `;
      } else {
        summary.style.display = 'none';
      }
    }
  }

  private drawBlindComparisonChart(targets: BlindTarget[]): void {
    const chartEl = document.getElementById('dyson-blind-chart');
    if (!chartEl) return;

    const w = 600, h = 180;
    const pad = { top: 20, right: 20, bottom: 30, left: 50 };
    const iw = w - pad.left - pad.right;
    const ih = h - pad.top - pad.bottom;

    const canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    canvas.style.cssText = 'width:100%;max-width:600px;height:auto';
    chartEl.innerHTML = '<div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:4px">IR Excess Comparison (W3 blue, W4 purple)</div>';
    chartEl.appendChild(canvas);

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.fillStyle = '#11161C';
    ctx.fillRect(0, 0, w, h);

    const maxExcess = Math.max(...targets.map(t => Math.max(t.pipeline.w3_excess_sigma, t.pipeline.w4_excess_sigma)));
    const barWidth = iw / targets.length / 2.5;

    for (let i = 0; i < targets.length; i++) {
      const t = targets[i];
      const x = pad.left + (i / targets.length) * iw + barWidth * 0.5;

      // W3 bar
      const h3 = (t.pipeline.w3_excess_sigma / maxExcess) * ih;
      ctx.fillStyle = 'rgba(68,136,255,0.7)';
      ctx.fillRect(x, pad.top + ih - h3, barWidth * 0.9, h3);

      // W4 bar
      const h4 = (t.pipeline.w4_excess_sigma / maxExcess) * ih;
      ctx.fillStyle = 'rgba(153,68,255,0.7)';
      ctx.fillRect(x + barWidth, pad.top + ih - h4, barWidth * 0.9, h4);

      // Label
      ctx.fillStyle = '#8B949E';
      ctx.font = '9px monospace';
      ctx.textAlign = 'center';
      ctx.fillText(t.target_id, x + barWidth, h - 8);
    }

    // Y axis labels
    ctx.fillStyle = '#484F58';
    ctx.font = '9px monospace';
    ctx.textAlign = 'right';
    for (let v = 0; v <= maxExcess; v += 5) {
      const y = pad.top + ih - (v / maxExcess) * ih;
      ctx.fillText(`${v}σ`, pad.left - 4, y + 3);
      ctx.strokeStyle = '#1C2333';
      ctx.lineWidth = 0.5;
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(pad.left + iw, y);
      ctx.stroke();
    }
  }

  private renderSignatures(): void {
    const el = document.getElementById('dyson-signatures');
    if (!el || !this.data) return;
    el.innerHTML = `
      <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:8px">Detection Signatures</div>
      <div style="display:flex;gap:12px;flex-wrap:wrap">
        ${this.data.detection_signatures.map(s => `
          <div style="padding:8px 12px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;flex:1;min-width:200px">
            <div style="font-size:11px;font-weight:600;color:var(--text-primary)">${s.name}</div>
            <div style="font-size:10px;color:var(--text-secondary);margin-top:2px">${s.description}</div>
            <div style="font-size:9px;color:#9944ff;margin-top:2px">${s.band}</div>
          </div>
        `).join('')}
      </div>
    `;
  }

  private renderPipeline(): void {
    const el = document.getElementById('dyson-pipeline');
    if (!el || !this.data) return;
    el.innerHTML = `
      <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:8px">Search Pipeline</div>
      <div style="display:flex;gap:4px;align-items:center;flex-wrap:wrap">
        ${this.data.pipeline_stages.map((s, i) => `
          <div id="dyson-stage-${i}" style="padding:6px 14px;border-radius:4px;background:var(--bg-surface);border:1px solid var(--border);transition:all 0.3s">
            <div style="font-size:10px;font-weight:700;color:var(--text-muted)">${s.stage}</div>
            <div style="font-size:9px;color:var(--text-muted)">${s.name}</div>
          </div>
          ${i < this.data!.pipeline_stages.length - 1 ? '<span style="color:var(--text-muted)">&#8594;</span>' : ''}
        `).join('')}
      </div>
    `;
  }

  private async runSearch(): Promise<void> {
    if (this.running || !this.data) return;
    this.running = true;

    if (this.candidatesEl) this.candidatesEl.innerHTML = '';
    if (this.resultEl) this.resultEl.style.display = 'none';
    if (this.chartArea) this.chartArea.style.display = 'none';
    const sphere3dPanel = document.getElementById('dyson-3d-panel');
    if (sphere3dPanel) sphere3dPanel.style.display = 'none';

    // Animate pipeline stages (optimized: shorter delays)
    for (let i = 0; i < this.data.pipeline_stages.length; i++) {
      this.highlightStage(i);
      await this.sleep(300);
    }

    // Show special targets first
    for (const target of this.data.special_targets) {
      await this.sleep(200);
      this.addSpecialTarget(target);
    }

    // Show candidates
    const sorted = [...this.data.candidates].sort((a, b) => b.pipeline_score - a.pipeline_score);
    for (const c of sorted) {
      await this.sleep(200);
      this.addCandidateCard(c);
    }

    // Show IR excess chart
    this.drawExcessChart(sorted);

    // Auto-show 3D sphere for top candidate (highest pipeline score = real data)
    if (sorted.length > 0) {
      this.update3dSphere(sorted[0]);
    }

    // Show summary
    await this.sleep(400);
    this.showResult();
    this.running = false;
  }

  private drawExcessChart(candidates: DysonCandidate[]): void {
    if (!this.chartArea) return;
    this.chartArea.style.display = '';
    this.chartArea.innerHTML = '';

    // SED-style chart header
    const header = document.createElement('div');
    header.style.cssText = 'font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px';
    header.textContent = 'Infrared Excess Comparison — All Candidates';
    this.chartArea.appendChild(header);

    const w = 700, h = 220;
    const pad = { top: 24, right: 20, bottom: 40, left: 55 };
    const iw = w - pad.left - pad.right;
    const ih = h - pad.top - pad.bottom;

    const canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    canvas.style.cssText = 'width:100%;max-width:700px;height:auto;border-radius:6px';
    this.chartArea.appendChild(canvas);

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.fillStyle = '#11161C';
    ctx.fillRect(0, 0, w, h);

    const n = candidates.length;
    const maxW3 = Math.max(...candidates.map(c => c.w3_excess));
    const maxW4 = Math.max(...candidates.map(c => c.w4_excess));
    const maxVal = Math.max(maxW3, maxW4);
    const groupWidth = iw / n;
    const barWidth = groupWidth * 0.35;

    // Grid
    ctx.strokeStyle = '#1C2333';
    ctx.lineWidth = 0.5;
    ctx.fillStyle = '#484F58';
    ctx.font = '9px monospace';
    ctx.textAlign = 'right';
    for (let v = 0; v <= maxVal; v += 5) {
      const y = pad.top + ih - (v / maxVal) * ih;
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(pad.left + iw, y);
      ctx.stroke();
      ctx.fillText(`${v}x`, pad.left - 4, y + 3);
    }

    // Bars
    for (let i = 0; i < n; i++) {
      const c = candidates[i];
      const groupX = pad.left + i * groupWidth;

      // W3 bar
      const h3 = (c.w3_excess / maxVal) * ih;
      ctx.fillStyle = 'rgba(68,136,255,0.75)';
      ctx.fillRect(groupX + groupWidth * 0.1, pad.top + ih - h3, barWidth, h3);

      // W4 bar
      const h4 = (c.w4_excess / maxVal) * ih;
      ctx.fillStyle = 'rgba(153,68,255,0.75)';
      ctx.fillRect(groupX + groupWidth * 0.1 + barWidth + 2, pad.top + ih - h4, barWidth, h4);

      // Coverage fraction dot
      const covY = pad.top + ih - (c.coverage_fraction * 100 / 2) * ih / maxVal;
      ctx.beginPath();
      ctx.arc(groupX + groupWidth * 0.5, Math.max(pad.top, covY), 3, 0, Math.PI * 2);
      ctx.fillStyle = '#2ECC71';
      ctx.fill();

      // Label
      const shortId = c.id.includes('DR3') ? c.id.split(' ').pop()!.substring(0, 6) + '...' : c.id;
      ctx.fillStyle = '#8B949E';
      ctx.font = '8px monospace';
      ctx.textAlign = 'center';
      ctx.save();
      ctx.translate(groupX + groupWidth * 0.5, h - 6);
      ctx.rotate(-0.3);
      ctx.fillText(shortId, 0, 0);
      ctx.restore();

      // Score label on top
      ctx.fillStyle = c.pipeline_score >= 0.7 ? '#9944ff' : 'var(--warning)';
      ctx.font = '9px monospace';
      ctx.textAlign = 'center';
      ctx.fillText(c.pipeline_score.toFixed(2), groupX + groupWidth * 0.5, pad.top - 6);
    }

    // Legend
    const legendY = pad.top + ih + 18;
    ctx.font = '9px sans-serif';
    ctx.textAlign = 'left';

    ctx.fillStyle = 'rgba(68,136,255,0.75)';
    ctx.fillRect(pad.left, legendY, 10, 8);
    ctx.fillStyle = '#8B949E';
    ctx.fillText('W3 (12μm)', pad.left + 14, legendY + 7);

    ctx.fillStyle = 'rgba(153,68,255,0.75)';
    ctx.fillRect(pad.left + 90, legendY, 10, 8);
    ctx.fillStyle = '#8B949E';
    ctx.fillText('W4 (22μm)', pad.left + 104, legendY + 7);

    ctx.fillStyle = '#2ECC71';
    ctx.beginPath();
    ctx.arc(pad.left + 186, legendY + 4, 3, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#8B949E';
    ctx.fillText('Coverage %', pad.left + 193, legendY + 7);

    // SED mini-chart for selected candidate
    this.addSedSelector(candidates);
  }

  private addSedSelector(candidates: DysonCandidate[]): void {
    if (!this.chartArea) return;

    const sedPanel = document.createElement('div');
    sedPanel.style.cssText = 'margin-top:12px;padding:12px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px';
    sedPanel.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:8px">
        <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px">Spectral Energy Distribution</div>
        <select id="dyson-sed-select" style="padding:3px 8px;background:var(--bg-panel);border:1px solid var(--border);color:var(--text-primary);border-radius:4px;font-size:10px">
          ${candidates.map((c, i) => `<option value="${i}">${c.id.includes('DR3') ? c.id.split(' ').pop()!.substring(0, 10) + '...' : c.id} (${c.pipeline_score.toFixed(2)})</option>`).join('')}
        </select>
      </div>
      <canvas id="dyson-sed-canvas" width="650" height="180" style="width:100%;max-width:650px;height:auto"></canvas>
      <div id="dyson-sed-info" style="margin-top:6px;font-size:10px;color:var(--text-secondary)"></div>
    `;
    this.chartArea.appendChild(sedPanel);

    const select = document.getElementById('dyson-sed-select') as HTMLSelectElement;
    if (select) {
      select.addEventListener('change', () => {
        this.drawSed(candidates[parseInt(select.value)]);
      });
      this.drawSed(candidates[0]);
    }
  }

  private update3dSphere(c: DysonCandidate): void {
    const panel = document.getElementById('dyson-3d-panel');
    if (panel) panel.style.display = '';

    const viewport = document.getElementById('dyson-3d-viewport');
    const infoEl = document.getElementById('dyson-3d-info');
    if (!viewport) return;

    // Create 3D instance if needed
    if (!this.sphere3d) {
      this.sphere3d = new DysonSphere3D(viewport);
    }

    // Update with real candidate data from Project Hephaistos
    const params: DysonParams = {
      coverageFraction: c.coverage_fraction,
      warmTempK: c.temperature_k,
      spectralType: c.spectral_type,
      w3Excess: c.w3_excess,
      w4Excess: c.w4_excess,
      label: c.id,
    };
    this.sphere3d.update(params);

    // Build controls overlay for the 3D viewport
    this.buildDysonControls();

    // Update info panel with real measurements
    if (infoEl) {
      const shortId = c.id.includes('DR3') ? c.gaia_id || c.id : c.id;
      const likelihoodColor = c.dyson_likelihood === 'Low' ? 'var(--text-muted)'
        : c.dyson_likelihood === 'Medium' ? 'var(--warning)' : '#9944ff';

      infoEl.innerHTML = `
        <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:2px">Candidate Parameters</div>

        <div style="padding:10px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px">
          <div style="font-size:12px;font-weight:600;color:var(--text-primary);margin-bottom:6px;font-family:var(--font-mono)">${shortId}</div>
          <div style="font-size:10px;color:var(--text-secondary)">${c.spectral_type} star at ${c.distance_pc} pc</div>
        </div>

        <div style="padding:10px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px">
          <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;margin-bottom:4px">Shell Coverage</div>
          <div style="font-size:22px;font-weight:700;color:#9944ff;font-family:var(--font-mono)">${(c.coverage_fraction * 100).toFixed(1)}%</div>
          <div style="height:4px;background:var(--bg);border-radius:2px;margin-top:4px;overflow:hidden">
            <div style="height:100%;width:${Math.min(100, c.coverage_fraction * 100)}%;background:#9944ff;border-radius:2px"></div>
          </div>
          <div style="font-size:9px;color:var(--text-muted);margin-top:3px">Fraction of stellar luminosity captured by the swarm</div>
        </div>

        <div style="padding:10px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px">
          <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;margin-bottom:4px">Waste Heat Temperature</div>
          <div style="font-size:18px;font-weight:600;color:var(--warning);font-family:var(--font-mono)">${c.temperature_k} K</div>
          <div style="font-size:9px;color:var(--text-muted);margin-top:2px">${c.temperature_k < 200 ? 'Cool outer shell' : c.temperature_k < 350 ? 'Warm equilibrium' : 'Hot inner swarm'}</div>
        </div>

        <div style="padding:10px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px">
          <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;margin-bottom:4px">IR Excess</div>
          <div style="display:flex;gap:12px">
            <div>
              <div style="font-size:14px;font-weight:600;color:rgba(68,136,255,0.9);font-family:var(--font-mono)">${c.w3_excess.toFixed(1)}x</div>
              <div style="font-size:8px;color:var(--text-muted)">W3 (12&mu;m)</div>
            </div>
            <div>
              <div style="font-size:14px;font-weight:600;color:#9944ff;font-family:var(--font-mono)">${c.w4_excess.toFixed(1)}x</div>
              <div style="font-size:8px;color:var(--text-muted)">W4 (22&mu;m)</div>
            </div>
          </div>
        </div>

        <div style="padding:10px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px">
          <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;margin-bottom:4px">Pipeline Score</div>
          <div style="font-size:18px;font-weight:600;color:${c.pipeline_score >= 0.7 ? '#9944ff' : 'var(--warning)'};font-family:var(--font-mono)">${c.pipeline_score.toFixed(3)}</div>
          <div style="font-size:9px;color:${likelihoodColor};font-weight:600;margin-top:2px">Dyson Likelihood: ${c.dyson_likelihood}</div>
        </div>

        <div style="font-size:9px;color:var(--text-muted);line-height:1.5;padding:4px 0">
          <strong>Natural explanations:</strong> ${c.natural_explanations.join(', ')}
        </div>
      `;
    }
  }

  private buildDysonControls(): void {
    const controlsEl = document.getElementById('dyson-3d-controls');
    if (!controlsEl || !this.sphere3d) return;
    controlsEl.innerHTML = '';

    const btnStyle =
      'border:1px solid rgba(30,38,48,0.8);border-radius:4px;background:rgba(11,15,20,0.9);' +
      'color:var(--text-secondary);font-size:10px;padding:4px 8px;cursor:pointer;' +
      'font-family:var(--font-mono);transition:color 0.15s,border-color 0.15s';
    const activeBtnStyle = btnStyle.replace('var(--text-secondary)', '#9944ff').replace('rgba(30,38,48,0.8)', '#9944ff');

    const speedLabel = document.createElement('span');
    speedLabel.style.cssText = 'font-size:9px;color:var(--text-muted);font-family:var(--font-mono)';
    speedLabel.textContent = 'Speed:';
    controlsEl.appendChild(speedLabel);

    const slider = document.createElement('input');
    slider.type = 'range';
    slider.min = '0.1';
    slider.max = '5';
    slider.step = '0.1';
    slider.value = '1';
    slider.style.cssText = 'width:70px;height:4px;accent-color:#9944ff;cursor:pointer';
    slider.addEventListener('input', () => {
      const v = parseFloat(slider.value);
      this.sphere3d?.setSpeed(v);
      speedVal.textContent = `${v.toFixed(1)}x`;
    });
    controlsEl.appendChild(slider);

    const speedVal = document.createElement('span');
    speedVal.style.cssText = 'font-size:9px;color:#9944ff;min-width:24px;font-family:var(--font-mono)';
    speedVal.textContent = '1.0x';
    controlsEl.appendChild(speedVal);

    const sep = document.createElement('span');
    sep.style.cssText = 'width:1px;height:14px;background:rgba(30,38,48,0.6)';
    controlsEl.appendChild(sep);

    const autoBtn = document.createElement('button');
    autoBtn.style.cssText = activeBtnStyle;
    autoBtn.textContent = 'Auto';
    autoBtn.title = 'Toggle auto-rotate camera';
    autoBtn.addEventListener('click', () => {
      this.sphere3d?.toggleAutoRotate();
      const active = this.sphere3d?.getAutoRotate() ?? false;
      autoBtn.style.cssText = active ? activeBtnStyle : btnStyle;
    });
    controlsEl.appendChild(autoBtn);

    const resetBtn = document.createElement('button');
    resetBtn.style.cssText = btnStyle;
    resetBtn.textContent = 'Reset';
    resetBtn.title = 'Reset camera to default position';
    resetBtn.addEventListener('click', () => {
      this.sphere3d?.resetCamera();
      autoBtn.style.cssText = activeBtnStyle;
      slider.value = '1';
      speedVal.textContent = '1.0x';
      this.sphere3d?.setSpeed(1);
    });
    controlsEl.appendChild(resetBtn);
  }

  private drawSed(c: DysonCandidate): void {
    // Also update the 3D sphere with this candidate's real data
    this.update3dSphere(c);

    const canvas = document.getElementById('dyson-sed-canvas') as HTMLCanvasElement;
    const info = document.getElementById('dyson-sed-info');
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const w = canvas.width, h = canvas.height;
    const pad = { top: 16, right: 16, bottom: 30, left: 50 };
    const iw = w - pad.left - pad.right;
    const ih = h - pad.top - pad.bottom;

    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#0D1117';
    ctx.fillRect(0, 0, w, h);

    // Wavelength bands (microns): optical(0.5), J(1.25), H(1.65), K(2.2), W1(3.4), W2(4.6), W3(12), W4(22)
    const bands = [
      { name: 'G', wl: 0.5, log: Math.log10(0.5) },
      { name: 'J', wl: 1.25, log: Math.log10(1.25) },
      { name: 'H', wl: 1.65, log: Math.log10(1.65) },
      { name: 'K', wl: 2.2, log: Math.log10(2.2) },
      { name: 'W1', wl: 3.4, log: Math.log10(3.4) },
      { name: 'W2', wl: 4.6, log: Math.log10(4.6) },
      { name: 'W3', wl: 12, log: Math.log10(12) },
      { name: 'W4', wl: 22, log: Math.log10(22) },
    ];

    const xMin = Math.log10(0.3), xMax = Math.log10(30);
    const xScale = (logWl: number) => pad.left + ((logWl - xMin) / (xMax - xMin)) * iw;

    // Stellar photosphere model (Rayleigh-Jeans tail) - normalized
    const tEff = c.temperature_k > 1000 ? c.temperature_k : 3400;
    const modelFlux = (wlMicron: number): number => {
      // Simplified blackbody in relative flux (Wien approximation scaled)
      const nu = 3e14 / wlMicron; // Hz
      const x = 6.626e-34 * nu / (1.38e-23 * tEff);
      return x > 50 ? 0 : (nu * nu * nu) / (Math.exp(x) - 1);
    };

    // Compute model values at each band
    const modelValues = bands.map(b => modelFlux(b.wl));
    const modelMax = Math.max(...modelValues);

    // Observed values (model + excess at W3/W4)
    const obsValues = bands.map((b, i) => {
      let obs = modelValues[i] / modelMax;
      if (b.name === 'W3') obs *= c.w3_excess;
      if (b.name === 'W4') obs *= c.w4_excess;
      return obs;
    });
    const normalizedModel = modelValues.map(v => v / modelMax);

    const allFlux = [...normalizedModel, ...obsValues].filter(v => v > 0);
    const yMin = Math.log10(Math.min(...allFlux) * 0.5);
    const yMax = Math.log10(Math.max(...allFlux) * 2);
    const yScale = (logF: number) => pad.top + ih - ((logF - yMin) / (yMax - yMin)) * ih;

    // Grid
    ctx.strokeStyle = '#1C2333';
    ctx.lineWidth = 0.5;
    for (let d = Math.ceil(yMin); d <= Math.floor(yMax); d++) {
      const y = yScale(d);
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(pad.left + iw, y);
      ctx.stroke();
      ctx.fillStyle = '#484F58';
      ctx.font = '8px monospace';
      ctx.textAlign = 'right';
      ctx.fillText(`10^${d}`, pad.left - 4, y + 3);
    }

    // Continuous model curve
    ctx.strokeStyle = 'rgba(255,221,68,0.5)';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    let first = true;
    for (let logWl = xMin; logWl <= xMax; logWl += 0.02) {
      const wl = Math.pow(10, logWl);
      const f = modelFlux(wl) / modelMax;
      if (f <= 0) continue;
      const x = xScale(logWl);
      const y = yScale(Math.log10(f));
      if (first) { ctx.moveTo(x, y); first = false; }
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // Warm component (Dyson sphere waste heat)
    const warmTemp = c.temperature_k > 100 ? c.temperature_k : 328;
    ctx.strokeStyle = 'rgba(153,68,255,0.5)';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 3]);
    ctx.beginPath();
    first = true;
    for (let logWl = Math.log10(2); logWl <= xMax; logWl += 0.02) {
      const wl = Math.pow(10, logWl);
      const nu = 3e14 / wl;
      const x2 = 6.626e-34 * nu / (1.38e-23 * warmTemp);
      if (x2 > 50) continue;
      const f = c.coverage_fraction * (nu * nu * nu) / (Math.exp(x2) - 1);
      const norm = f / modelMax * 50;
      if (norm <= 0) continue;
      const x = xScale(logWl);
      const y = yScale(Math.log10(norm));
      if (y < pad.top || y > pad.top + ih) continue;
      if (first) { ctx.moveTo(x, y); first = false; }
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.setLineDash([]);

    // Plot observed data points
    for (let i = 0; i < bands.length; i++) {
      const b = bands[i];
      const f = obsValues[i];
      if (f <= 0) continue;
      const x = xScale(b.log);
      const y = yScale(Math.log10(f));

      const isExcess = b.name === 'W3' || b.name === 'W4';
      ctx.fillStyle = isExcess ? '#9944ff' : '#ffdd44';
      ctx.beginPath();
      ctx.arc(x, y, isExcess ? 5 : 3.5, 0, Math.PI * 2);
      ctx.fill();

      // Model point (for W3/W4 show both)
      if (isExcess) {
        const mf = normalizedModel[i];
        if (mf > 0) {
          const my = yScale(Math.log10(mf));
          ctx.fillStyle = 'rgba(255,221,68,0.5)';
          ctx.beginPath();
          ctx.arc(x, my, 3, 0, Math.PI * 2);
          ctx.fill();

          // Arrow from model to observed
          ctx.strokeStyle = 'rgba(153,68,255,0.6)';
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.moveTo(x, my);
          ctx.lineTo(x, y);
          ctx.stroke();
        }
      }

      // Label
      ctx.fillStyle = isExcess ? '#9944ff' : '#8B949E';
      ctx.font = '9px monospace';
      ctx.textAlign = 'center';
      ctx.fillText(b.name, x, pad.top + ih + 14);
    }

    // X axis label
    ctx.fillStyle = '#8B949E';
    ctx.font = '9px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('Wavelength (μm, log scale)', pad.left + iw / 2, h - 4);

    // Legend
    ctx.textAlign = 'left';
    const lx = pad.left + iw - 200;
    ctx.fillStyle = 'rgba(255,221,68,0.5)';
    ctx.fillRect(lx, pad.top + 2, 10, 2);
    ctx.fillStyle = '#8B949E';
    ctx.fillText('Stellar photosphere', lx + 14, pad.top + 7);

    ctx.fillStyle = 'rgba(153,68,255,0.5)';
    ctx.fillRect(lx, pad.top + 14, 10, 2);
    ctx.fillStyle = '#8B949E';
    ctx.fillText(`Warm component (${warmTemp}K)`, lx + 14, pad.top + 19);

    // Info text
    if (info) {
      info.innerHTML = `
        <strong style="color:var(--text-primary)">${c.id.includes('DR3') ? c.id.split(' ').pop()!.substring(0, 16) + '...' : c.id}</strong>
        &mdash; ${c.spectral_type} at ${c.distance_pc} pc
        &mdash; W3 excess: <span style="color:#4488ff">${c.w3_excess.toFixed(1)}x</span>
        &mdash; W4 excess: <span style="color:#9944ff">${c.w4_excess.toFixed(1)}x</span>
        &mdash; Coverage: <span style="color:#2ECC71">${(c.coverage_fraction * 100).toFixed(1)}%</span>
        &mdash; Warm temp: ${c.temperature_k}K
        &mdash; Likelihood: <span style="color:${c.dyson_likelihood === 'Low' ? 'var(--text-muted)' : '#9944ff'}">${c.dyson_likelihood}</span>
      `;
    }
  }

  private highlightStage(index: number): void {
    for (let i = 0; i < (this.data?.pipeline_stages.length ?? 0); i++) {
      const el = document.getElementById(`dyson-stage-${i}`);
      if (!el) continue;
      const label = el.querySelector('div');
      if (i < index) {
        el.style.background = 'rgba(153,68,255,0.15)';
        el.style.borderColor = 'rgba(153,68,255,0.4)';
        if (label) label.style.color = '#9944ff';
      } else if (i === index) {
        el.style.background = 'rgba(153,68,255,0.25)';
        el.style.borderColor = '#9944ff';
        if (label) label.style.color = '#9944ff';
      } else {
        el.style.background = 'var(--bg-surface)';
        el.style.borderColor = 'var(--border)';
        if (label) label.style.color = 'var(--text-muted)';
      }
    }
  }

  private addSpecialTarget(target: { id: string; description: string; key_observations: string[]; current_status: string }): void {
    if (!this.candidatesEl) return;
    const card = document.createElement('div');
    card.style.cssText = 'padding:12px 16px;margin-bottom:8px;border-radius:6px;background:rgba(255,176,32,0.06);border:1px solid rgba(255,176,32,0.3)';
    card.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px">
        <span style="font-size:13px;font-weight:600;color:var(--text-primary)">${target.id}</span>
        <span class="score-badge score-medium" style="font-size:8px">NOTABLE TARGET</span>
      </div>
      <div style="font-size:11px;color:var(--text-secondary);margin-bottom:4px">${target.description}</div>
      <div style="font-size:10px;color:var(--text-muted)">
        ${target.key_observations.map(o => `<div style="margin-left:8px">&#8226; ${o}</div>`).join('')}
      </div>
      <div style="font-size:10px;color:var(--warning);margin-top:4px;font-style:italic">Status: ${target.current_status}</div>
    `;
    this.candidatesEl.appendChild(card);
  }

  private addCandidateCard(c: DysonCandidate & { follow_up_status?: string }): void {
    if (!this.candidatesEl) return;

    const isDebunked = c.dyson_likelihood === 'None (debunked)';
    const scoreColor = isDebunked ? '#FF4D4D' : c.pipeline_score >= 0.7 ? '#9944ff' : c.pipeline_score >= 0.4 ? 'var(--warning)' : 'var(--text-muted)';
    const borderColor = isDebunked ? 'rgba(255,77,77,0.3)' : 'var(--border)';
    const bgColor = isDebunked ? 'rgba(255,77,77,0.04)' : 'var(--bg-surface)';
    const card = document.createElement('div');
    card.style.cssText = `padding:12px 16px;margin-bottom:8px;border-radius:6px;background:${bgColor};border:1px solid ${borderColor};cursor:pointer;transition:border-color 0.2s`;
    card.addEventListener('mouseenter', () => { card.style.borderColor = isDebunked ? '#FF4D4D' : '#9944ff'; });
    card.addEventListener('mouseleave', () => { card.style.borderColor = borderColor; });
    card.addEventListener('click', () => this.drawSed(c));

    const statusBadge = isDebunked
      ? '<span style="font-size:8px;padding:2px 6px;border-radius:3px;background:rgba(255,77,77,0.15);color:#FF4D4D;font-weight:700;text-transform:uppercase;letter-spacing:0.3px">DEBUNKED</span>'
      : (c as Record<string, unknown>).follow_up_status
        ? '<span style="font-size:8px;padding:2px 6px;border-radius:3px;background:rgba(255,176,32,0.1);color:var(--warning);font-weight:600;text-transform:uppercase;letter-spacing:0.3px">UNCONFIRMED</span>'
        : '';

    const followUp = (c as Record<string, unknown>).follow_up_status
      ? `<div style="font-size:9px;color:${isDebunked ? '#FF4D4D' : 'var(--warning)'};margin-top:4px;font-style:italic;border-top:1px solid var(--border-subtle);padding-top:4px">${(c as Record<string, unknown>).follow_up_status}</div>`
      : '';

    card.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px">
        <span style="font-size:13px;font-weight:600;color:${isDebunked ? '#FF4D4D' : 'var(--text-primary)'};${isDebunked ? 'text-decoration:line-through' : ''}">${c.id}</span>
        ${statusBadge}
        <span style="font-size:10px;color:${scoreColor};font-weight:600">Score: ${c.pipeline_score.toFixed(2)}</span>
        <span style="font-size:9px;color:var(--text-muted)">${c.spectral_type} | ${c.distance_pc} pc | Coverage: ${(c.coverage_fraction * 100).toFixed(1)}%</span>
        <span style="font-size:9px;color:var(--text-muted);margin-left:auto">W3: ${c.w3_excess.toFixed(1)}x | W4: ${c.w4_excess.toFixed(1)}x</span>
      </div>
      <div style="display:flex;gap:16px">
        <div style="flex:1">
          <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">${c.analysis}</div>
          ${followUp}
        </div>
        <div style="min-width:200px">
          <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;margin-bottom:3px">Natural Explanations</div>
          ${c.natural_explanations.map(e => `<div style="font-size:10px;color:var(--text-secondary);margin-left:8px">&#8226; ${e}</div>`).join('')}
          <div style="font-size:9px;color:${isDebunked ? '#FF4D4D' : c.dyson_likelihood === 'Low' ? 'var(--text-muted)' : 'var(--warning)'};margin-top:4px;font-weight:600">Dyson likelihood: ${c.dyson_likelihood}</div>
        </div>
      </div>
    `;
    this.candidatesEl.appendChild(card);
  }

  private showResult(): void {
    if (!this.resultEl || !this.data) return;
    const s = this.data.summary;
    this.resultEl.style.display = '';
    this.resultEl.innerHTML = `
      <div style="padding:20px;background:rgba(153,68,255,0.06);border:2px solid rgba(153,68,255,0.3);border-radius:10px">
        <div style="font-size:16px;font-weight:700;color:#9944ff;margin-bottom:10px">Search Results</div>
        <div style="display:flex;gap:24px;margin-bottom:12px">
          <div>
            <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase">Stars Searched</div>
            <div style="font-size:20px;font-weight:700;color:var(--text-primary)">${s.stars_searched}</div>
          </div>
          <div>
            <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase">Anomalous Candidates</div>
            <div style="font-size:20px;font-weight:700;color:#9944ff">${s.candidates_found}</div>
          </div>
        </div>
        <div style="font-size:12px;color:var(--text-secondary);line-height:1.7">${s.conclusion}</div>
        <div style="margin-top:12px;font-size:9px;color:var(--text-muted)">
          ${this.data.references.map(r => `<div>${r}</div>`).join('')}
        </div>
      </div>
    `;
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((r) => setTimeout(r, ms));
  }

  unmount(): void {
    this.running = false;
    if (this.sphere3d) {
      this.sphere3d.destroy();
      this.sphere3d = null;
    }
    this.container = null;
    this.candidatesEl = null;
    this.resultEl = null;
    this.chartArea = null;
    this.blindArea = null;
    this.sphere3dContainer = null;
    this.sphere3dInfoEl = null;
    this.data = null;
    this.blindData = null;
  }
}
