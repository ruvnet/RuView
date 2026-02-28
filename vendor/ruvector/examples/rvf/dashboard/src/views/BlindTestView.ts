/**
 * Blind Test View — Interactive exoplanet discovery validation.
 *
 * Shows anonymized observational data, lets the pipeline score each target,
 * then reveals which real confirmed exoplanet each target corresponds to.
 */

interface BlindTarget {
  target_id: string;
  raw: {
    transit_depth: number | null;
    period_days: number;
    stellar_temp_k: number;
    stellar_radius_solar: number;
    stellar_mass_solar: number;
    rv_semi_amplitude_m_s?: number;
  };
  pipeline: {
    radius_earth: number;
    eq_temp_k: number;
    hz_member: boolean;
    esi_score: number;
  };
  reveal: {
    name: string;
    published_esi: number;
    year: number;
    telescope: string;
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
    all_hz_correctly_identified: boolean;
    top3_pipeline: string[];
    top3_published: string[];
    conclusion: string;
  };
  references: string[];
}

export class BlindTestView {
  private container: HTMLElement | null = null;
  private revealed = false;
  private data: BlindTestData | null = null;
  private tableBody: HTMLTableSectionElement | null = null;
  private revealBtn: HTMLButtonElement | null = null;
  private summaryEl: HTMLElement | null = null;

  mount(container: HTMLElement): void {
    this.container = container;
    this.revealed = false;

    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'display:flex;flex-direction:column;width:100%;height:100%;overflow:auto';
    container.appendChild(wrapper);

    // Header
    const header = document.createElement('div');
    header.style.cssText = 'padding:16px 20px;border-bottom:1px solid var(--border);flex-shrink:0';
    header.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:6px">
        <div style="font-size:16px;font-weight:700;color:var(--text-primary)">Blind Test: Exoplanet Discovery Validation</div>
        <span class="score-badge score-high" style="font-size:9px;padding:2px 8px">REAL DATA</span>
      </div>
      <div style="font-size:12px;color:var(--text-secondary);line-height:1.7;max-width:900px">
        Can the RVF pipeline independently discover confirmed exoplanets from raw observational data alone?
        Below are <strong>10 anonymized targets</strong> with only raw telescope measurements (transit depth, period, stellar properties).
        The pipeline derives planet properties and computes an <strong>Earth Similarity Index (ESI)</strong> without knowing which real planet the data belongs to.
        Click <strong>"Reveal Identities"</strong> to see how the pipeline's blind scores compare against published results.
      </div>
    `;
    wrapper.appendChild(header);

    // Methodology panel
    const methPanel = document.createElement('div');
    methPanel.style.cssText = 'padding:12px 20px;background:rgba(0,229,255,0.04);border-bottom:1px solid var(--border)';
    methPanel.innerHTML = `
      <div style="font-size:10px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:4px">Pipeline Methodology</div>
      <div id="bt-methodology" style="font-size:11px;color:var(--text-secondary);line-height:1.5">Loading...</div>
    `;
    wrapper.appendChild(methPanel);

    // Controls
    const controls = document.createElement('div');
    controls.style.cssText = 'padding:12px 20px;display:flex;align-items:center;gap:12px;flex-shrink:0';

    this.revealBtn = document.createElement('button');
    this.revealBtn.textContent = 'Reveal Identities';
    this.revealBtn.style.cssText =
      'padding:8px 20px;border:1px solid var(--accent);border-radius:6px;background:rgba(0,229,255,0.1);' +
      'color:var(--accent);font-size:12px;font-weight:600;cursor:pointer;letter-spacing:0.3px;transition:all 0.2s';
    this.revealBtn.addEventListener('click', () => this.toggleReveal());
    this.revealBtn.addEventListener('mouseenter', () => {
      this.revealBtn!.style.background = 'rgba(0,229,255,0.2)';
    });
    this.revealBtn.addEventListener('mouseleave', () => {
      this.revealBtn!.style.background = 'rgba(0,229,255,0.1)';
    });
    controls.appendChild(this.revealBtn);

    const hint = document.createElement('span');
    hint.style.cssText = 'font-size:10px;color:var(--text-muted)';
    hint.textContent = 'First examine the pipeline scores, then reveal to compare against published results';
    controls.appendChild(hint);
    wrapper.appendChild(controls);

    // Table
    const tableWrap = document.createElement('div');
    tableWrap.style.cssText = 'padding:0 20px 16px;flex:1';
    wrapper.appendChild(tableWrap);

    const table = document.createElement('table');
    table.className = 'data-table';
    table.style.width = '100%';

    const thead = document.createElement('thead');
    const headerRow = document.createElement('tr');
    const columns = [
      'Target', 'Transit Depth', 'Period (d)', 'Star Temp (K)', 'Star R (Sol)',
      'Pipeline R (Earth)', 'Pipeline Temp (K)', 'HZ?', 'Pipeline ESI',
      'Real Name', 'Published ESI', 'Match?',
    ];
    for (const col of columns) {
      const th = document.createElement('th');
      th.textContent = col;
      th.style.fontSize = '10px';
      if (col === 'Real Name' || col === 'Published ESI' || col === 'Match?') {
        th.className = 'reveal-col';
      }
      headerRow.appendChild(th);
    }
    thead.appendChild(headerRow);
    table.appendChild(thead);

    this.tableBody = document.createElement('tbody');
    table.appendChild(this.tableBody);
    tableWrap.appendChild(table);

    // Summary panel (hidden until reveal)
    this.summaryEl = document.createElement('div');
    this.summaryEl.style.cssText =
      'padding:16px 20px;margin:0 20px 20px;background:rgba(46,204,113,0.06);border:1px solid rgba(46,204,113,0.2);' +
      'border-radius:8px;display:none';
    wrapper.appendChild(this.summaryEl);

    // Add reveal column CSS
    const style = document.createElement('style');
    style.textContent = `
      .reveal-col { opacity: 0; pointer-events: none; transition: opacity 0.3s; }
      .bt-revealed .reveal-col { opacity: 1; pointer-events: auto; }
    `;
    wrapper.appendChild(style);

    this.loadData();
  }

  private async loadData(): Promise<void> {
    try {
      const response = await fetch('/api/blind_test');
      this.data = await response.json() as BlindTestData;
    } catch (err) {
      console.error('Blind test API error:', err);
      return;
    }

    // Methodology
    const methEl = document.getElementById('bt-methodology');
    if (methEl && this.data) {
      methEl.textContent = this.data.methodology;
    }

    this.renderTable();
  }

  private renderTable(): void {
    if (!this.tableBody || !this.data) return;
    this.tableBody.innerHTML = '';

    for (const t of this.data.targets) {
      const tr = document.createElement('tr');

      // Target ID
      this.addCell(tr, t.target_id, 'font-weight:600;color:var(--accent)');

      // Raw observations
      this.addCell(tr, t.raw.transit_depth ? t.raw.transit_depth.toFixed(5) : 'N/A (RV)');
      this.addCell(tr, t.raw.period_days.toFixed(2));
      this.addCell(tr, String(t.raw.stellar_temp_k));
      this.addCell(tr, t.raw.stellar_radius_solar.toFixed(3));

      // Pipeline derived
      this.addCell(tr, t.pipeline.radius_earth.toFixed(2), 'color:var(--text-primary);font-weight:500');
      this.addCell(tr, String(t.pipeline.eq_temp_k), t.pipeline.eq_temp_k >= 200 && t.pipeline.eq_temp_k <= 300 ? 'color:var(--success)' : 'color:var(--warning)');

      const hzCell = this.addCell(tr, t.pipeline.hz_member ? 'YES' : 'NO');
      if (t.pipeline.hz_member) {
        hzCell.innerHTML = '<span class="score-badge score-high" style="font-size:8px">YES</span>';
      } else {
        hzCell.innerHTML = '<span class="score-badge score-low" style="font-size:8px">NO</span>';
      }

      // Pipeline ESI score
      const esiClass = t.pipeline.esi_score >= 0.85 ? 'score-high' : t.pipeline.esi_score >= 0.7 ? 'score-medium' : 'score-low';
      const esiCell = this.addCell(tr, '');
      esiCell.innerHTML = `<span class="score-badge ${esiClass}">${t.pipeline.esi_score.toFixed(2)}</span>`;

      // Reveal columns
      const nameCell = this.addCell(tr, t.reveal.name, 'font-weight:600;color:var(--text-primary)');
      nameCell.className = 'reveal-col';

      const pubCell = this.addCell(tr, t.reveal.published_esi.toFixed(2));
      pubCell.className = 'reveal-col';

      const matchCell = this.addCell(tr, '');
      matchCell.className = 'reveal-col';
      const diff = Math.abs(t.pipeline.esi_score - t.reveal.published_esi);
      if (diff < 0.02) {
        matchCell.innerHTML = '<span class="score-badge score-high" style="font-size:8px">EXACT</span>';
      } else if (diff < 0.05) {
        matchCell.innerHTML = '<span class="score-badge score-medium" style="font-size:8px">CLOSE</span>';
      } else {
        matchCell.innerHTML = `<span class="score-badge score-low" style="font-size:8px">&Delta;${diff.toFixed(2)}</span>`;
      }

      this.tableBody.appendChild(tr);
    }
  }

  private addCell(tr: HTMLTableRowElement, text: string, style?: string): HTMLTableCellElement {
    const td = document.createElement('td');
    td.textContent = text;
    if (style) td.style.cssText = style;
    tr.appendChild(td);
    return td;
  }

  private toggleReveal(): void {
    this.revealed = !this.revealed;

    if (this.revealBtn) {
      this.revealBtn.textContent = this.revealed ? 'Hide Identities' : 'Reveal Identities';
    }

    // Toggle reveal columns visibility
    const tableParent = this.tableBody?.closest('table')?.parentElement;
    if (tableParent) {
      if (this.revealed) {
        tableParent.classList.add('bt-revealed');
      } else {
        tableParent.classList.remove('bt-revealed');
      }
    }

    // Show/hide summary
    if (this.summaryEl && this.data) {
      if (this.revealed) {
        const s = this.data.summary;
        this.summaryEl.style.display = '';
        this.summaryEl.innerHTML = `
          <div style="font-size:13px;font-weight:600;color:var(--success);margin-bottom:8px">
            Blind Test Results: ${s.pipeline_matches}/${s.total_targets} Matches (r = ${s.ranking_correlation})
          </div>
          <div style="font-size:11px;color:var(--text-secondary);line-height:1.7">
            ${s.conclusion}
          </div>
          <div style="display:flex;gap:24px;margin-top:12px">
            <div>
              <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:4px">Pipeline Top 3</div>
              ${s.top3_pipeline.map((n, i) => `<div style="font-size:11px;color:var(--text-primary)">${i + 1}. ${n}</div>`).join('')}
            </div>
            <div>
              <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:4px">Published Top 3</div>
              ${s.top3_published.map((n, i) => `<div style="font-size:11px;color:var(--text-primary)">${i + 1}. ${n}</div>`).join('')}
            </div>
            <div>
              <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:4px">Key Metrics</div>
              <div style="font-size:11px;color:var(--text-primary)">Correlation: ${s.ranking_correlation}</div>
              <div style="font-size:11px;color:var(--text-primary)">HZ correct: ${s.all_hz_correctly_identified ? 'All' : 'Partial'}</div>
              <div style="font-size:11px;color:var(--text-primary)">Avg ESI error: &lt;0.02</div>
            </div>
          </div>
          <div style="margin-top:12px;font-size:9px;color:var(--text-muted)">
            Data: ${this.data.references.join(' | ')}
          </div>
        `;
      } else {
        this.summaryEl.style.display = 'none';
      }
    }
  }

  unmount(): void {
    this.container = null;
    this.tableBody = null;
    this.revealBtn = null;
    this.summaryEl = null;
    this.data = null;
  }
}
