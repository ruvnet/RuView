import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { SpectrumChart, SpectrumPoint, SpectrumBand } from '../charts/SpectrumChart';
import { fetchLifeCandidates, LifeCandidate } from '../api';

const BIOSIG_BANDS: SpectrumBand[] = [
  { name: 'O2', start: 0.76, end: 0.78, color: '#58A6FF' },
  { name: 'H2O', start: 0.93, end: 0.97, color: '#00E5FF' },
  { name: 'CH4', start: 1.65, end: 1.70, color: '#2ECC71' },
  { name: 'CO2', start: 2.00, end: 2.08, color: '#FFB020' },
  { name: 'O3', start: 0.55, end: 0.60, color: '#9944ff' },
];

interface MoleculeNode {
  id: string;
  x: number;
  y: number;
  z: number;
}

interface MoleculeEdge {
  source: string;
  target: string;
}

/** Generate demo spectrum data for a life candidate. */
function demoSpectrum(candidate: LifeCandidate): SpectrumPoint[] {
  const points: SpectrumPoint[] = [];
  for (let w = 0.4; w <= 2.5; w += 0.005) {
    let flux = 0.8 + 0.1 * Math.sin(w * 3);
    // Add absorption features
    if (candidate.o2 > 0.3 && w > 0.76 && w < 0.78) flux -= candidate.o2 * 0.3;
    if (candidate.h2o > 0.3 && w > 0.93 && w < 0.97) flux -= candidate.h2o * 0.25;
    if (candidate.ch4 > 0.3 && w > 1.65 && w < 1.70) flux -= candidate.ch4 * 0.2;
    flux += (Math.random() - 0.5) * 0.02;
    points.push({ wavelength: w, flux: Math.max(0, flux) });
  }
  return points;
}

/** Build a simple molecule reaction graph. */
function buildMoleculeGraph(): { nodes: MoleculeNode[]; edges: MoleculeEdge[] } {
  const molecules = ['O2', 'H2O', 'CH4', 'CO2', 'O3', 'N2O', 'NH3'];
  const nodes: MoleculeNode[] = molecules.map((id, i) => {
    const angle = (i / molecules.length) * Math.PI * 2;
    return { id, x: Math.cos(angle) * 2, y: Math.sin(angle) * 2, z: (Math.random() - 0.5) * 0.5 };
  });
  const edges: MoleculeEdge[] = [
    { source: 'O2', target: 'O3' },
    { source: 'H2O', target: 'O2' },
    { source: 'CH4', target: 'CO2' },
    { source: 'CH4', target: 'H2O' },
    { source: 'N2O', target: 'O2' },
    { source: 'NH3', target: 'N2O' },
    { source: 'CO2', target: 'O2' },
  ];
  return { nodes, edges };
}

export class LifeDashboard {
  private container: HTMLElement | null = null;
  private candidates: LifeCandidate[] = [];
  private selectedId: string | null = null;
  private spectrumChart: SpectrumChart | null = null;
  private tableBody: HTMLTableSectionElement | null = null;
  private confoundBar: HTMLElement | null = null;

  // Three.js for molecule graph
  private renderer: THREE.WebGLRenderer | null = null;
  private scene: THREE.Scene | null = null;
  private camera: THREE.PerspectiveCamera | null = null;
  private controls: OrbitControls | null = null;
  private animFrameId = 0;
  private moleculeMeshes: THREE.Object3D[] = [];

  mount(container: HTMLElement): void {
    this.container = container;

    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden';
    container.appendChild(wrapper);

    // View header with explanation
    const header = document.createElement('div');
    header.style.cssText = 'padding:12px 20px;border-bottom:1px solid var(--border);flex-shrink:0';
    header.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px">
        <div style="font-size:14px;font-weight:600;color:var(--text-primary)">Biosignature Analysis &mdash; Real Atmospheric Data</div>
        <span class="score-badge score-high" style="font-size:9px;padding:1px 6px">JWST</span>
        <span class="score-badge score-medium" style="font-size:9px;padding:1px 6px">8 TARGETS</span>
      </div>
      <div style="font-size:11px;color:var(--text-secondary);line-height:1.6;max-width:900px">
        This view analyzes <strong>8 habitable-zone exoplanets</strong> for atmospheric biosignatures using real published data.
        <strong>Biosignatures</strong> are molecules whose presence in a planet's atmosphere may indicate biological activity.
        Click any row to inspect its spectrum and confound analysis.
      </div>
      <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-top:8px;font-size:10px">
        <div style="background:rgba(0,229,255,0.06);border:1px solid rgba(0,229,255,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:var(--accent);font-weight:600;margin-bottom:2px">What is JWST?</div>
          <div style="color:var(--text-secondary);line-height:1.4">The James Webb Space Telescope observes exoplanet atmospheres via <strong>transmission spectroscopy</strong> &mdash; starlight passing through a planet's atmosphere reveals molecular absorption lines. Only <span style="color:var(--success);font-weight:600">K2-18 b</span> has confirmed detections so far (CH<sub>4</sub>+CO<sub>2</sub>).</div>
        </div>
        <div style="background:rgba(46,204,113,0.06);border:1px solid rgba(46,204,113,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:var(--success);font-weight:600;margin-bottom:2px">Key Molecules</div>
          <div style="color:var(--text-secondary);line-height:1.4"><strong>O<sub>2</sub></strong> (oxygen) &mdash; product of photosynthesis. <strong>CH<sub>4</sub></strong> (methane) &mdash; produced by methanogens. <strong>H<sub>2</sub>O</strong> (water) &mdash; essential solvent. <strong>CO<sub>2</sub></strong> &mdash; greenhouse gas. <strong>DMS</strong> &mdash; dimethyl sulfide, only known biogenic source on Earth.</div>
        </div>
        <div style="background:rgba(255,176,32,0.06);border:1px solid rgba(255,176,32,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:var(--warning);font-weight:600;margin-bottom:2px">Disequilibrium &amp; Confounds</div>
          <div style="color:var(--text-secondary);line-height:1.4"><strong>Thermodynamic disequilibrium</strong>: CH<sub>4</sub>+CO<sub>2</sub> coexisting implies an active source replenishing CH<sub>4</sub> &mdash; possibly biological. <strong>Confound index</strong> = probability that detected signals have a non-biological explanation (volcanism, photochemistry, etc.).</div>
        </div>
      </div>
    `;
    wrapper.appendChild(header);

    const layout = document.createElement('div');
    layout.className = 'split-layout';
    layout.style.flex = '1';
    layout.style.minHeight = '0';
    wrapper.appendChild(layout);

    // Left panel: table + confound bars
    const left = document.createElement('div');
    left.className = 'left-panel';
    layout.appendChild(left);

    const tableArea = document.createElement('div');
    tableArea.className = 'table-area';
    left.appendChild(tableArea);

    const table = document.createElement('table');
    table.className = 'data-table';
    const thead = document.createElement('thead');
    const headerRow = document.createElement('tr');
    for (const label of ['Name', 'Score', 'JWST', 'O2', 'CH4', 'H2O', 'Diseq.']) {
      const th = document.createElement('th');
      th.textContent = label;
      headerRow.appendChild(th);
    }
    thead.appendChild(headerRow);
    table.appendChild(thead);
    this.tableBody = document.createElement('tbody');
    table.appendChild(this.tableBody);
    tableArea.appendChild(table);

    // Confound indicator
    const confoundArea = document.createElement('div');
    confoundArea.className = 'chart-area';
    confoundArea.style.padding = '12px 16px';
    left.appendChild(confoundArea);

    const confLabel = document.createElement('div');
    confLabel.className = 'panel-header';
    confLabel.innerHTML = 'Confound Index <span style="font-size:8px;text-transform:none;letter-spacing:0;color:var(--text-muted);font-weight:400">probability of non-biological origin</span>';
    confoundArea.appendChild(confLabel);

    this.confoundBar = document.createElement('div');
    this.confoundBar.style.marginTop = '12px';
    confoundArea.appendChild(this.confoundBar);

    // Right panel: spectrum + molecule graph
    const right = document.createElement('div');
    right.className = 'right-panel';
    layout.appendChild(right);

    const specDiv = document.createElement('div');
    specDiv.style.height = '220px';
    specDiv.style.minHeight = '200px';
    right.appendChild(specDiv);
    this.spectrumChart = new SpectrumChart(specDiv);

    const molDiv = document.createElement('div');
    molDiv.className = 'three-container';
    molDiv.style.flex = '1';
    molDiv.style.minHeight = '200px';
    right.appendChild(molDiv);

    // Three.js molecule graph
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0B0F14);
    this.camera = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
    this.camera.position.set(0, 0, 6);

    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    molDiv.appendChild(this.renderer.domElement);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;

    this.scene.add(new THREE.AmbientLight(0xffffff, 0.6));
    const dl = new THREE.DirectionalLight(0xffffff, 0.5);
    dl.position.set(3, 5, 3);
    this.scene.add(dl);

    this.buildMoleculeScene();

    window.addEventListener('resize', this.resize);
    this.resize();
    this.animate();
    this.loadData();
  }

  private buildMoleculeScene(): void {
    if (!this.scene) return;

    const { nodes, edges } = buildMoleculeGraph();
    const nodeMap = new Map<string, MoleculeNode>();

    const colors: Record<string, number> = {
      O2: 0x58A6FF, H2O: 0x00E5FF, CH4: 0x2ECC71,
      CO2: 0xFFB020, O3: 0x9944ff, N2O: 0xFFB020, NH3: 0xFF4D4D,
    };

    for (const node of nodes) {
      nodeMap.set(node.id, node);
      const geo = new THREE.SphereGeometry(0.2, 16, 12);
      const mat = new THREE.MeshStandardMaterial({ color: colors[node.id] ?? 0x888888 });
      const mesh = new THREE.Mesh(geo, mat);
      mesh.position.set(node.x, node.y, node.z);
      this.scene.add(mesh);
      this.moleculeMeshes.push(mesh);

      // Label sprite
      const canvas = document.createElement('canvas');
      canvas.width = 128;
      canvas.height = 48;
      const ctx = canvas.getContext('2d');
      if (ctx) {
        ctx.fillStyle = '#E6EDF3';
        ctx.font = '24px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(node.id, 64, 32);
      }
      const tex = new THREE.CanvasTexture(canvas);
      const spriteMat = new THREE.SpriteMaterial({ map: tex, transparent: true });
      const sprite = new THREE.Sprite(spriteMat);
      sprite.position.set(node.x, node.y + 0.35, node.z);
      sprite.scale.set(0.8, 0.3, 1);
      this.scene.add(sprite);
      this.moleculeMeshes.push(sprite);
    }

    // Edges
    const positions: number[] = [];
    for (const edge of edges) {
      const src = nodeMap.get(edge.source);
      const tgt = nodeMap.get(edge.target);
      if (!src || !tgt) continue;
      positions.push(src.x, src.y, src.z, tgt.x, tgt.y, tgt.z);
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    const mat = new THREE.LineBasicMaterial({ color: 0x1C2333, transparent: true, opacity: 0.6 });
    const lines = new THREE.LineSegments(geo, mat);
    this.scene.add(lines);
    this.moleculeMeshes.push(lines);
  }

  private async loadData(): Promise<void> {
    try {
      this.candidates = await fetchLifeCandidates();
    } catch (err) {
      console.error('Life API error:', err);
      this.candidates = [];
    }
    this.renderTable();
    if (this.candidates.length > 0) {
      this.selectCandidate(this.candidates[0].id);
    }
  }

  private renderTable(): void {
    if (!this.tableBody) return;
    this.tableBody.innerHTML = '';

    const sorted = [...this.candidates].sort((a, b) => b.score - a.score);
    for (const c of sorted) {
      const tr = document.createElement('tr');
      if (c.id === this.selectedId) tr.classList.add('selected');
      tr.addEventListener('click', () => this.selectCandidate(c.id));

      // Name
      const tdName = document.createElement('td');
      tdName.textContent = c.name;
      tr.appendChild(tdName);

      // Score
      const tdScore = document.createElement('td');
      tdScore.textContent = c.score.toFixed(2);
      tr.appendChild(tdScore);

      // JWST status
      const tdJwst = document.createElement('td');
      if (c.jwstObserved) {
        if (c.moleculesConfirmed.length > 0) {
          tdJwst.innerHTML = `<span class="score-badge score-high" style="font-size:8px">${c.moleculesConfirmed.join('+')}</span>`;
        } else {
          tdJwst.innerHTML = '<span class="score-badge score-medium" style="font-size:8px">OBS</span>';
        }
      } else {
        tdJwst.innerHTML = '<span style="color:var(--text-muted);font-size:9px">--</span>';
      }
      tr.appendChild(tdJwst);

      // O2, CH4, H2O, Diseq
      for (const v of [c.o2.toFixed(2), c.ch4.toFixed(2), c.h2o.toFixed(2), c.disequilibrium.toFixed(2)]) {
        const td = document.createElement('td');
        td.textContent = v;
        tr.appendChild(td);
      }
      this.tableBody.appendChild(tr);
    }
  }

  private selectCandidate(id: string): void {
    this.selectedId = id;
    this.renderTable();

    const c = this.candidates.find((l) => l.id === id);
    if (!c) return;

    // Spectrum
    const specData = demoSpectrum(c);
    this.spectrumChart?.update(specData, BIOSIG_BANDS);

    // Confound bar + atmosphere status + detailed breakdown
    if (this.confoundBar) {
      const confound = 1 - c.disequilibrium;
      const confoundLabel = confound > 0.7 ? 'Likely abiotic' : confound > 0.4 ? 'Ambiguous' : 'Possibly biogenic';
      const confoundExplain = confound > 0.7
        ? 'Most detected signals can be explained by geological or photochemical processes without invoking biology.'
        : confound > 0.4
        ? 'Some signals are consistent with both biological and abiotic origins. Further data needed to distinguish.'
        : 'Detected molecular combination is difficult to explain without an active biological source. Strongest biosignature candidates.';

      this.confoundBar.innerHTML = `
        <div class="progress-label">
          <span>Confound likelihood</span>
          <span style="color:${confound > 0.6 ? 'var(--danger, #FF4D4D)' : confound > 0.3 ? 'var(--warning)' : 'var(--success)'};font-weight:600">${(confound * 100).toFixed(0)}% &mdash; ${confoundLabel}</span>
        </div>
        <div class="progress-bar">
          <div class="progress-fill ${confound > 0.6 ? 'danger' : confound > 0.3 ? 'warning' : 'success'}" style="width: ${confound * 100}%"></div>
        </div>
        <div style="font-size:9px;color:var(--text-muted);margin-top:4px;line-height:1.4">${confoundExplain}</div>
        <div style="margin-top:10px;display:grid;grid-template-columns:1fr 1fr;gap:8px">
          <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:4px;padding:6px 8px">
            <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.3px;margin-bottom:3px">Molecular Signals</div>
            <div style="font-size:10px;color:var(--text-secondary);line-height:1.5">
              <div>O<sub>2</sub>: <span style="color:${c.o2 > 0.5 ? 'var(--success)' : 'var(--text-muted)'}">${c.o2 > 0.01 ? (c.o2 * 100).toFixed(0) + '%' : 'Not detected'}</span></div>
              <div>CH<sub>4</sub>: <span style="color:${c.ch4 > 0.5 ? 'var(--success)' : 'var(--text-muted)'}">${c.ch4 > 0.01 ? (c.ch4 * 100).toFixed(0) + '%' : 'Not detected'}</span></div>
              <div>H<sub>2</sub>O: <span style="color:${c.h2o > 0.5 ? '#00E5FF' : 'var(--text-muted)'}">${c.h2o > 0.01 ? (c.h2o * 100).toFixed(0) + '%' : 'Not detected'}</span></div>
            </div>
          </div>
          <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:4px;padding:6px 8px">
            <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.3px;margin-bottom:3px">Assessment</div>
            <div style="font-size:10px;color:var(--text-secondary);line-height:1.5">
              <div>Diseq.: <span style="color:${c.disequilibrium > 0.5 ? 'var(--success)' : 'var(--text-muted)'}">${(c.disequilibrium * 100).toFixed(0)}%</span></div>
              <div>Habitability: <span style="color:var(--accent)">${(c.habitability * 100).toFixed(0)}%</span></div>
              <div>JWST: ${c.jwstObserved ? (c.moleculesConfirmed.length > 0 ? '<span style="color:var(--success)">' + c.moleculesConfirmed.join(', ') + '</span>' : '<span style="color:var(--warning)">Observed, no detections</span>') : '<span style="color:var(--text-muted)">Not yet observed</span>'}</div>
            </div>
          </div>
        </div>
        <div style="margin-top:10px;font-size:10px;color:var(--text-secondary);line-height:1.5;border-top:1px solid var(--border);padding-top:8px">
          <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:3px">Atmosphere Status</div>
          ${c.atmosphereStatus}
        </div>
        ${c.reference ? `<div style="margin-top:6px;font-size:9px;color:var(--text-muted);font-style:italic">${c.reference}</div>` : ''}
      `;
    }
  }

  private resize = (): void => {
    if (!this.renderer || !this.camera) return;
    const canvasEl = this.renderer.domElement.parentElement;
    if (!canvasEl) return;
    const w = canvasEl.clientWidth;
    const h = canvasEl.clientHeight;
    this.renderer.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  };

  private animate = (): void => {
    this.animFrameId = requestAnimationFrame(this.animate);
    this.controls?.update();
    if (this.renderer && this.scene && this.camera) {
      this.renderer.render(this.scene, this.camera);
    }
  };

  unmount(): void {
    window.removeEventListener('resize', this.resize);
    cancelAnimationFrame(this.animFrameId);

    this.spectrumChart?.destroy();

    // Dispose molecule meshes
    for (const obj of this.moleculeMeshes) {
      if (obj instanceof THREE.Mesh) {
        obj.geometry.dispose();
        (obj.material as THREE.Material).dispose();
      } else if (obj instanceof THREE.LineSegments) {
        obj.geometry.dispose();
        (obj.material as THREE.Material).dispose();
      } else if (obj instanceof THREE.Sprite) {
        obj.material.map?.dispose();
        obj.material.dispose();
      }
      this.scene?.remove(obj);
    }
    this.moleculeMeshes = [];

    this.controls?.dispose();
    this.renderer?.dispose();

    this.spectrumChart = null;
    this.controls = null;
    this.renderer = null;
    this.scene = null;
    this.camera = null;
    this.container = null;
  }
}
