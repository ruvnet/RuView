/**
 * Discovery View — New planet discovery pipeline with 3D solar system visualization.
 *
 * Processes real unconfirmed KOI/TOI candidates through the RVF pipeline
 * to identify the most Earth-like world awaiting confirmation.
 * Includes interactive Three.js 3D visualization of each candidate's
 * orbital system — host star, planet orbit, habitable zone.
 */

import { PlanetSystem3D, PlanetSystemParams } from '../three/PlanetSystem3D';

interface DiscoveryCandidate {
  id: string;
  catalog: string;
  status: string;
  raw_observations: Record<string, number>;
  pipeline_derived: {
    radius_earth: number;
    semi_major_axis_au: number;
    eq_temp_k: number;
    hz_member: boolean;
    esi_score: number;
    radius_similarity: number;
    temperature_similarity: number;
  };
  analysis: string;
  confirmation_needs: string[];
  significance: string;
  discovery_rank: number;
}

interface DiscoveryData {
  mission: string;
  pipeline_stages: Array<{ stage: string; name: string; description: string }>;
  candidates: DiscoveryCandidate[];
  discovery: {
    top_candidate: string;
    esi_score: number;
    comparison: Record<string, string>;
    why_not_confirmed: string;
    what_confirmation_requires: string[];
    pipeline_witness_chain: Array<{ witness: string; measurement: string; confidence: number }>;
  };
  data_source: string;
  references: string[];
}

export class DiscoveryView {
  private container: HTMLElement | null = null;
  private data: DiscoveryData | null = null;
  private pipelineEl: HTMLElement | null = null;
  private candidatesEl: HTMLElement | null = null;
  private discoveryEl: HTMLElement | null = null;
  private running = false;
  private currentStage = -1;

  // 3D visualization
  private planet3d: PlanetSystem3D | null = null;
  private planet3dContainer: HTMLElement | null = null;
  private planet3dInfoEl: HTMLElement | null = null;
  private controlsEl: HTMLElement | null = null;
  private vizPanel: HTMLElement | null = null;
  private selectedCardEl: HTMLElement | null = null;

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
        <div style="font-size:16px;font-weight:700;color:var(--text-primary)">New Planet Discovery</div>
        <span class="score-badge score-high" style="font-size:9px;padding:2px 8px">LIVE PIPELINE</span>
      </div>
      <div style="font-size:12px;color:var(--text-secondary);line-height:1.7;max-width:900px">
        The RVF pipeline processes <strong>real unconfirmed candidates</strong> from the Kepler Objects of Interest (KOI) catalog.
        These are stars with detected transit signals that have not yet been confirmed as planets.
        The pipeline derives physical properties from raw photometry and ranks candidates by Earth Similarity Index.
        <strong>Click any candidate</strong> to view its 3D orbital system.
      </div>
    `;
    wrapper.appendChild(header);

    // Pipeline stages
    this.pipelineEl = document.createElement('div');
    this.pipelineEl.style.cssText = 'padding:16px 20px;border-bottom:1px solid var(--border)';
    wrapper.appendChild(this.pipelineEl);

    // Run button
    const controls = document.createElement('div');
    controls.style.cssText = 'padding:12px 20px;flex-shrink:0';
    const runBtn = document.createElement('button');
    runBtn.textContent = 'Run Discovery Pipeline';
    runBtn.style.cssText =
      'padding:10px 24px;border:none;border-radius:6px;background:var(--accent);' +
      'color:#0B0F14;font-size:13px;font-weight:700;cursor:pointer;letter-spacing:0.3px';
    runBtn.addEventListener('click', () => this.runPipeline());
    controls.appendChild(runBtn);
    wrapper.appendChild(controls);

    // 3D Visualization panel (hidden until candidate selected)
    this.vizPanel = document.createElement('div');
    this.vizPanel.style.cssText =
      'padding:0 20px 16px;display:none';
    wrapper.appendChild(this.vizPanel);

    const vizInner = document.createElement('div');
    vizInner.style.cssText =
      'display:grid;grid-template-columns:1fr 260px;gap:0;' +
      'background:var(--bg-surface);border:1px solid var(--border);border-radius:8px;overflow:hidden';
    this.vizPanel.appendChild(vizInner);

    // 3D viewport (larger)
    this.planet3dContainer = document.createElement('div');
    this.planet3dContainer.style.cssText =
      'position:relative;min-height:420px;background:#020408';
    vizInner.appendChild(this.planet3dContainer);

    // Controls overlay (bottom-left of viewport)
    this.controlsEl = document.createElement('div');
    this.controlsEl.style.cssText =
      'position:absolute;bottom:10px;left:10px;display:flex;gap:6px;align-items:center;z-index:20;' +
      'background:rgba(2,4,8,0.8);border:1px solid rgba(30,38,48,0.6);border-radius:6px;padding:6px 10px';
    this.planet3dContainer.appendChild(this.controlsEl);

    // Interaction hint (top-right)
    const hint = document.createElement('div');
    hint.style.cssText =
      'position:absolute;top:8px;right:8px;font-size:9px;color:rgba(230,237,243,0.4);' +
      'z-index:20;pointer-events:none;text-align:right;line-height:1.6';
    hint.innerHTML = 'Drag to rotate<br>Scroll to zoom<br>Right-drag to pan';
    this.planet3dContainer.appendChild(hint);

    // Info sidebar
    this.planet3dInfoEl = document.createElement('div');
    this.planet3dInfoEl.style.cssText =
      'padding:16px;overflow-y:auto;max-height:420px;font-size:11px;' +
      'color:var(--text-secondary);line-height:1.7;border-left:1px solid var(--border)';
    vizInner.appendChild(this.planet3dInfoEl);

    // Candidates area
    this.candidatesEl = document.createElement('div');
    this.candidatesEl.style.cssText = 'padding:0 20px 16px';
    wrapper.appendChild(this.candidatesEl);

    // Discovery result
    this.discoveryEl = document.createElement('div');
    this.discoveryEl.style.cssText = 'padding:0 20px 24px;display:none';
    wrapper.appendChild(this.discoveryEl);

    this.loadData();
  }

  private async loadData(): Promise<void> {
    try {
      const response = await fetch('/api/discover');
      this.data = (await response.json()) as DiscoveryData;
    } catch (err) {
      console.error('Discovery API error:', err);
      return;
    }
    this.renderPipelineStages();
  }

  private renderPipelineStages(): void {
    if (!this.pipelineEl || !this.data) return;
    const stages = this.data.pipeline_stages;
    this.pipelineEl.innerHTML = `
      <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:8px">Pipeline Stages</div>
      <div style="display:flex;gap:4px;align-items:center;flex-wrap:wrap">
        ${stages.map((s, i) => `
          <div id="stage-${i}" style="padding:6px 14px;border-radius:4px;background:var(--bg-surface);border:1px solid var(--border);transition:all 0.3s">
            <div style="font-size:10px;font-weight:700;color:var(--text-muted)">${s.stage}</div>
            <div style="font-size:9px;color:var(--text-muted)">${s.name}</div>
          </div>
          ${i < stages.length - 1 ? '<span style="color:var(--text-muted)">&#8594;</span>' : ''}
        `).join('')}
      </div>
    `;
  }

  private async runPipeline(): Promise<void> {
    if (this.running || !this.data) return;
    this.running = true;
    this.currentStage = -1;

    // Clear previous results
    if (this.candidatesEl) this.candidatesEl.innerHTML = '';
    if (this.discoveryEl) this.discoveryEl.style.display = 'none';
    if (this.vizPanel) this.vizPanel.style.display = 'none';
    this.selectedCardEl = null;

    // Animate through pipeline stages
    for (let i = 0; i < this.data.pipeline_stages.length; i++) {
      this.currentStage = i;
      this.highlightStage(i);
      await this.sleep(600);
    }

    // Show candidates one by one
    const sorted = [...this.data.candidates].sort((a, b) => a.discovery_rank - b.discovery_rank);
    for (const c of sorted) {
      await this.sleep(400);
      this.addCandidateCard(c);
    }

    // Auto-show 3D for the top-ranked candidate
    if (sorted.length > 0) {
      this.show3D(sorted[0]);
    }

    // Show discovery
    await this.sleep(800);
    this.showDiscovery();
    this.running = false;
  }

  private highlightStage(index: number): void {
    for (let i = 0; i < (this.data?.pipeline_stages.length ?? 0); i++) {
      const el = document.getElementById(`stage-${i}`);
      if (!el) continue;
      if (i < index) {
        el.style.background = 'rgba(46,204,113,0.15)';
        el.style.borderColor = 'rgba(46,204,113,0.4)';
        el.querySelector('div')!.style.color = 'var(--success)';
      } else if (i === index) {
        el.style.background = 'rgba(0,229,255,0.15)';
        el.style.borderColor = 'var(--accent)';
        el.querySelector('div')!.style.color = 'var(--accent)';
      } else {
        el.style.background = 'var(--bg-surface)';
        el.style.borderColor = 'var(--border)';
        el.querySelector('div')!.style.color = 'var(--text-muted)';
      }
    }
  }

  private addCandidateCard(c: DiscoveryCandidate): void {
    if (!this.candidatesEl) return;

    const esiClass = c.pipeline_derived.esi_score >= 0.9 ? 'score-high' : c.pipeline_derived.esi_score >= 0.8 ? 'score-medium' : 'score-low';
    const isTop = c.discovery_rank === 1;

    const card = document.createElement('div');
    card.style.cssText = `
      padding:12px 16px;margin-bottom:8px;border-radius:6px;cursor:pointer;
      background:${isTop ? 'rgba(0,229,255,0.08)' : 'var(--bg-surface)'};
      border:1px solid ${isTop ? 'var(--accent)' : 'var(--border)'};
      animation: fadeIn 0.3s ease-out;
      transition: border-color 0.2s, background 0.2s;
    `;
    card.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:6px">
        <span style="font-size:13px;font-weight:600;color:var(--text-primary)">${c.id}</span>
        <span class="score-badge ${esiClass}" style="font-size:10px">ESI ${c.pipeline_derived.esi_score.toFixed(2)}</span>
        <span style="font-size:9px;color:var(--text-muted)">${c.catalog}</span>
        ${isTop ? '<span class="score-badge score-high" style="font-size:8px;background:rgba(0,229,255,0.2);color:var(--accent);border-color:var(--accent)">TOP DISCOVERY</span>' : ''}
        <span style="font-size:9px;color:var(--text-muted);margin-left:4px" title="Click to view 3D system">&#127760; View 3D</span>
        <span style="margin-left:auto;font-size:10px;color:var(--text-muted)">
          R=${c.pipeline_derived.radius_earth.toFixed(2)} R&#8853; | T=${c.pipeline_derived.eq_temp_k}K |
          HZ: ${c.pipeline_derived.hz_member ? '<span style="color:var(--success)">YES</span>' : '<span style="color:var(--critical)">NO</span>'}
        </span>
      </div>
      <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">${c.analysis}</div>
    `;

    card.addEventListener('click', () => {
      this.show3D(c);
      // Highlight selected card
      if (this.selectedCardEl) {
        this.selectedCardEl.style.borderColor = this.selectedCardEl.dataset.isTop === '1' ? 'var(--accent)' : 'var(--border)';
        this.selectedCardEl.style.boxShadow = 'none';
      }
      card.style.borderColor = 'var(--accent)';
      card.style.boxShadow = '0 0 8px rgba(0,229,255,0.2)';
      this.selectedCardEl = card;
    });
    card.dataset.isTop = isTop ? '1' : '0';

    this.candidatesEl.appendChild(card);
  }

  private show3D(c: DiscoveryCandidate): void {
    if (!this.vizPanel || !this.planet3dContainer || !this.planet3dInfoEl) return;

    this.vizPanel.style.display = '';

    // Destroy previous 3D if exists
    if (this.planet3d) {
      this.planet3d.destroy();
      this.planet3d = null;
    }

    // Create new 3D system
    this.planet3d = new PlanetSystem3D(this.planet3dContainer);
    const params: PlanetSystemParams = {
      label: c.id,
      radiusEarth: c.pipeline_derived.radius_earth,
      semiMajorAxisAU: c.pipeline_derived.semi_major_axis_au,
      eqTempK: c.pipeline_derived.eq_temp_k,
      stellarTempK: c.raw_observations['stellar_temp_k'] ?? 5500,
      stellarRadiusSolar: c.raw_observations['stellar_radius_solar'] ?? 1.0,
      periodDays: c.raw_observations['period_days'] ?? 365,
      hzMember: c.pipeline_derived.hz_member,
      esiScore: c.pipeline_derived.esi_score,
      transitDepth: c.raw_observations['transit_depth'] ?? 0.001,
    };
    this.planet3d.update(params);

    // Build controls overlay
    this.buildControls();

    // Update info sidebar
    const starType = this.getSpectralType(params.stellarTempK);
    const tempLabel = this.getTempLabel(params.eqTempK);

    this.planet3dInfoEl.innerHTML = `
      <div style="font-size:14px;font-weight:700;color:var(--text-primary);margin-bottom:10px">${c.id}</div>
      <div style="margin-bottom:12px">
        <span class="score-badge ${c.pipeline_derived.esi_score >= 0.9 ? 'score-high' : c.pipeline_derived.esi_score >= 0.8 ? 'score-medium' : 'score-low'}"
              style="font-size:11px;padding:3px 8px">ESI ${c.pipeline_derived.esi_score.toFixed(2)}</span>
        <span style="margin-left:6px;font-size:10px;color:${c.pipeline_derived.hz_member ? 'var(--success)' : 'var(--critical)'}">
          ${c.pipeline_derived.hz_member ? 'Habitable Zone' : 'Outside HZ'}
        </span>
      </div>

      <div style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:6px">Planet</div>
      <div style="padding-left:8px;border-left:2px solid var(--accent);margin-bottom:12px">
        <div>Radius: <span style="color:var(--accent)">${params.radiusEarth.toFixed(2)} R&#8853;</span></div>
        <div>Temperature: <span style="color:var(--accent)">${params.eqTempK} K</span> <span style="font-size:9px;color:var(--text-muted)">(${tempLabel})</span></div>
        <div>Orbit: <span style="color:var(--accent)">${params.semiMajorAxisAU.toFixed(3)} AU</span></div>
        <div>Period: <span style="color:var(--accent)">${params.periodDays.toFixed(1)} days</span></div>
        <div>Transit depth: <span style="color:var(--accent)">${(params.transitDepth * 1e6).toFixed(0)} ppm</span></div>
      </div>

      <div style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:6px">Host Star</div>
      <div style="padding-left:8px;border-left:2px solid #ffd2a1;margin-bottom:12px">
        <div>Type: <span style="color:#ffd2a1">${starType}</span></div>
        <div>T<sub>eff</sub>: <span style="color:#ffd2a1">${params.stellarTempK} K</span></div>
        <div>Radius: <span style="color:#ffd2a1">${params.stellarRadiusSolar.toFixed(3)} R&#9737;</span></div>
      </div>

      <div style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:6px">ESI Breakdown</div>
      <div style="padding-left:8px;border-left:2px solid var(--success);margin-bottom:12px">
        <div>Radius similarity: <span style="color:var(--success)">${(c.pipeline_derived.radius_similarity * 100).toFixed(0)}%</span></div>
        <div>Temp similarity: <span style="color:var(--success)">${(c.pipeline_derived.temperature_similarity * 100).toFixed(0)}%</span></div>
      </div>

      <div style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:6px">Comparison to Earth</div>
      <div style="font-size:10px;line-height:1.8">
        ${this.earthComparison(params)}
      </div>
    `;

    // Scroll viz into view
    this.vizPanel.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }

  private buildControls(): void {
    if (!this.controlsEl || !this.planet3d) return;
    this.controlsEl.innerHTML = '';

    const btnStyle =
      'border:1px solid rgba(30,38,48,0.8);border-radius:4px;background:rgba(11,15,20,0.9);' +
      'color:var(--text-secondary);font-size:10px;padding:4px 8px;cursor:pointer;' +
      'font-family:var(--font-mono);transition:color 0.15s,border-color 0.15s';
    const activeBtnStyle = btnStyle.replace('var(--text-secondary)', 'var(--accent)').replace('rgba(30,38,48,0.8)', 'var(--accent)');

    // Speed label
    const speedLabel = document.createElement('span');
    speedLabel.style.cssText = 'font-size:9px;color:var(--text-muted);font-family:var(--font-mono)';
    speedLabel.textContent = 'Speed:';
    this.controlsEl.appendChild(speedLabel);

    // Speed slider
    const slider = document.createElement('input');
    slider.type = 'range';
    slider.min = '0.1';
    slider.max = '5';
    slider.step = '0.1';
    slider.value = '1';
    slider.style.cssText = 'width:70px;height:4px;accent-color:var(--accent);cursor:pointer';
    slider.addEventListener('input', () => {
      const v = parseFloat(slider.value);
      this.planet3d?.setSpeed(v);
      speedVal.textContent = `${v.toFixed(1)}x`;
    });
    this.controlsEl.appendChild(slider);

    const speedVal = document.createElement('span');
    speedVal.style.cssText = 'font-size:9px;color:var(--accent);min-width:24px;font-family:var(--font-mono)';
    speedVal.textContent = '1.0x';
    this.controlsEl.appendChild(speedVal);

    // Separator
    const sep = document.createElement('span');
    sep.style.cssText = 'width:1px;height:14px;background:rgba(30,38,48,0.6)';
    this.controlsEl.appendChild(sep);

    // Auto-rotate toggle
    const autoBtn = document.createElement('button');
    autoBtn.style.cssText = activeBtnStyle;
    autoBtn.textContent = 'Auto';
    autoBtn.title = 'Toggle auto-rotate camera';
    autoBtn.addEventListener('click', () => {
      this.planet3d?.toggleAutoRotate();
      const active = this.planet3d?.getAutoRotate() ?? false;
      autoBtn.style.cssText = active ? activeBtnStyle : btnStyle;
    });
    this.controlsEl.appendChild(autoBtn);

    // Reset view button
    const resetBtn = document.createElement('button');
    resetBtn.style.cssText = btnStyle;
    resetBtn.textContent = 'Reset';
    resetBtn.title = 'Reset camera to default position';
    resetBtn.addEventListener('click', () => {
      this.planet3d?.resetCamera();
      autoBtn.style.cssText = activeBtnStyle;
      slider.value = '1';
      speedVal.textContent = '1.0x';
      this.planet3d?.setSpeed(1);
    });
    this.controlsEl.appendChild(resetBtn);
  }

  private getSpectralType(teff: number): string {
    if (teff > 7500) return `A-type (${teff} K)`;
    if (teff > 6000) return `F-type (${teff} K)`;
    if (teff > 5200) return `G-type (${teff} K) — Sun-like`;
    if (teff > 3700) return `K-type (${teff} K)`;
    return `M-type (${teff} K)`;
  }

  private getTempLabel(eqTempK: number): string {
    if (eqTempK < 180) return 'frozen';
    if (eqTempK < 240) return 'cold';
    if (eqTempK < 280) return 'temperate';
    if (eqTempK < 330) return 'warm';
    return 'hot';
  }

  private earthComparison(p: PlanetSystemParams): string {
    const rRatio = p.radiusEarth;
    const tRatio = p.eqTempK / 255; // Earth's effective temp ~255K
    const aRatio = p.semiMajorAxisAU; // Earth = 1 AU
    const pRatio = p.periodDays / 365.25;

    const fmt = (v: number, unit: string) => {
      if (v > 0.95 && v < 1.05) return `<span style="color:var(--success)">~Earth (${v.toFixed(2)}${unit})</span>`;
      if (v > 1) return `<span style="color:var(--accent)">${v.toFixed(2)}x Earth</span>`;
      return `<span style="color:var(--accent)">${v.toFixed(2)}x Earth</span>`;
    };

    return `
      <div>Radius: ${fmt(rRatio, 'x')}</div>
      <div>Temperature: ${fmt(tRatio, 'x')}</div>
      <div>Orbit: ${fmt(aRatio, ' AU')}</div>
      <div>Year: ${fmt(pRatio, 'x')}</div>
    `;
  }

  private showDiscovery(): void {
    if (!this.discoveryEl || !this.data) return;
    const d = this.data.discovery;
    this.discoveryEl.style.display = '';

    this.discoveryEl.innerHTML = `
      <div style="padding:20px;background:rgba(0,229,255,0.06);border:2px solid var(--accent);border-radius:10px">
        <div style="display:flex;align-items:center;gap:10px;margin-bottom:12px">
          <div style="font-size:18px;font-weight:800;color:var(--accent)">DISCOVERY: ${d.top_candidate}</div>
          <span class="score-badge score-high" style="font-size:12px;padding:3px 10px">ESI ${d.esi_score}</span>
          <span style="font-size:10px;color:var(--success);font-weight:600">MOST EARTH-LIKE CANDIDATE IN KEPLER CATALOG</span>
        </div>

        <div style="display:grid;grid-template-columns:1fr 1fr;gap:16px;margin-bottom:16px">
          <div>
            <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px">Comparison to Known Worlds</div>
            ${Object.entries(d.comparison).map(([k, v]) => `
              <div style="font-size:11px;color:var(--text-secondary);margin-bottom:4px;padding-left:8px;border-left:2px solid var(--accent)">
                <strong>${k.replace('vs_', 'vs ')}</strong>: ${v}
              </div>
            `).join('')}
          </div>
          <div>
            <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px">Why Not Yet Confirmed</div>
            <div style="font-size:11px;color:var(--text-secondary);line-height:1.6">${d.why_not_confirmed}</div>
          </div>
        </div>

        <div style="margin-bottom:16px">
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px">Pipeline Witness Chain</div>
          <div style="display:flex;gap:6px;flex-wrap:wrap">
            ${d.pipeline_witness_chain.map(w => `
              <div style="padding:6px 10px;background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;font-size:10px">
                <div style="color:var(--accent);font-weight:600">${w.witness}</div>
                <div style="color:var(--text-secondary)">${w.measurement}</div>
                <div style="color:${w.confidence > 0.9 ? 'var(--success)' : 'var(--warning)'}">${(w.confidence * 100).toFixed(0)}% conf.</div>
              </div>
            `).join('')}
          </div>
        </div>

        <div>
          <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px">Steps to Confirmation</div>
          ${d.what_confirmation_requires.map(s => `
            <div style="font-size:11px;color:var(--text-secondary);margin-bottom:3px;padding-left:8px">${s}</div>
          `).join('')}
        </div>
      </div>
    `;
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((r) => setTimeout(r, ms));
  }

  unmount(): void {
    this.running = false;
    if (this.planet3d) {
      this.planet3d.destroy();
      this.planet3d = null;
    }
    this.container = null;
    this.pipelineEl = null;
    this.candidatesEl = null;
    this.discoveryEl = null;
    this.vizPanel = null;
    this.planet3dContainer = null;
    this.planet3dInfoEl = null;
    this.controlsEl = null;
    this.selectedCardEl = null;
    this.data = null;
  }
}
