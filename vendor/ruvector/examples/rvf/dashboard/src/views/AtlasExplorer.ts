import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { AtlasGraph, GraphNode, GraphEdge } from '../three/AtlasGraph';
import { fetchAtlasQuery } from '../api';
import { onEvent, LiveEvent } from '../ws';

const SCALES = ['2h', '12h', '3d', '27d'] as const;

/* ── Seeded RNG ── */
function seededRng(seed: number): () => number {
  let s = seed | 0;
  return () => { s = (s * 1103515245 + 12345) & 0x7fffffff; return s / 0x7fffffff; };
}

/* ── Constellation name generator ── */
const CONSTELLATION_NAMES = [
  'Lyra', 'Cygnus', 'Aquila', 'Orion', 'Centaurus', 'Vela', 'Puppis',
  'Sagittarius', 'Scorpius', 'Cassiopeia', 'Perseus', 'Andromeda',
  'Draco', 'Ursa Major', 'Leo', 'Virgo', 'Libra', 'Gemini',
];

/* ── Galaxy data generator ── */
function generateGalaxyData(
  scale: string,
  nodeCount: number,
  armCount: number,
  armSpread: number,
  coreConcentration: number,
): { nodes: GraphNode[]; edges: GraphEdge[] } {
  const n = nodeCount;
  const domains = ['transit', 'flare', 'rotation', 'eclipse', 'variability'];
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];
  const rng = seededRng(scale.length * 31337);

  const maxRadius = 8;

  for (let i = 0; i < n; i++) {
    const arm = i % armCount;
    const armAngle = (arm / armCount) * Math.PI * 2;
    const t = rng();
    // Core concentration: power law pushes more nodes toward center
    const radius = 0.3 + Math.pow(t, coreConcentration) * maxRadius;

    const spiralAngle = armAngle + radius * 0.6 + (rng() - 0.5) * armSpread;
    const diskHeight = (rng() - 0.5) * 0.5 * Math.exp(-radius * 0.12);

    const scatter = radius * 0.08;
    const x = radius * Math.cos(spiralAngle) + (rng() - 0.5) * scatter;
    const z = radius * Math.sin(spiralAngle) + (rng() - 0.5) * scatter;
    const y = diskHeight;

    const weight = 0.15 + rng() * 0.85;
    nodes.push({ id: `s${i}`, domain: domains[i % domains.length], x, y, z, weight });
  }

  // Edges: connect nearby stars
  for (let i = 1; i < n; i++) {
    let bestDist = Infinity;
    let bestJ = 0;
    const searchRange = Math.min(i, 25);
    for (let j = Math.max(0, i - searchRange); j < i; j++) {
      const dx = nodes[i].x - nodes[j].x;
      const dy = nodes[i].y - nodes[j].y;
      const dz = nodes[i].z - nodes[j].z;
      const d = dx * dx + dy * dy + dz * dz;
      if (d < bestDist) { bestDist = d; bestJ = j; }
    }
    edges.push({ source: nodes[bestJ].id, target: nodes[i].id, weight: Math.max(0.1, 1 - bestDist / 16) });

    // Cross-arm connections
    if (rng() > 0.85 && i > 4) {
      const extra = Math.floor(rng() * i);
      const dx = nodes[i].x - nodes[extra].x;
      const dz = nodes[i].z - nodes[extra].z;
      if (Math.sqrt(dx * dx + dz * dz) < 4) {
        edges.push({ source: nodes[extra].id, target: nodes[i].id, weight: 0.05 + rng() * 0.15 });
      }
    }
  }

  return { nodes, edges };
}

export class AtlasExplorer {
  private container: HTMLElement | null = null;
  private renderer: THREE.WebGLRenderer | null = null;
  private scene: THREE.Scene | null = null;
  private camera: THREE.PerspectiveCamera | null = null;
  private controls: OrbitControls | null = null;
  private graph: AtlasGraph | null = null;
  private starfield: THREE.Points | null = null;
  private nebulaGroup: THREE.Group | null = null;
  private starMapLabels: THREE.Group | null = null;
  private gridHelper: THREE.Group | null = null;
  private animFrameId = 0;
  private unsubWs: (() => void) | null = null;
  private activeScale: string = '12h';
  private time = 0;

  // Configurable parameters
  private nodeCount = 150;
  private spiralArms = 4;
  private armSpread = 0.4;
  private coreConcentration = 1.0;
  private rotationSpeed = 0.15;
  private showGrid = true;
  private showLabels = true;
  private showEdges = true;
  private pulseNodes = true;

  // DOM refs for live slider updates
  private sliderRefs: Map<string, { slider: HTMLInputElement; valEl: HTMLElement }> = new Map();
  private statsEl: HTMLElement | null = null;

  mount(container: HTMLElement): void {
    this.container = container;

    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'display:flex;width:100%;height:100%;overflow:hidden';
    container.appendChild(wrapper);

    // Left sidebar: config + star map info
    const sidebar = this.buildSidebar();
    wrapper.appendChild(sidebar);

    // Main 3D viewport
    const mainArea = document.createElement('div');
    mainArea.style.cssText = 'flex:1;position:relative;min-width:0';
    wrapper.appendChild(mainArea);

    const canvasDiv = document.createElement('div');
    canvasDiv.className = 'three-container';
    mainArea.appendChild(canvasDiv);

    // Scale selector (time window)
    const scaleBar = document.createElement('div');
    scaleBar.className = 'scale-selector';
    for (const s of SCALES) {
      const btn = document.createElement('button');
      btn.className = 'scale-btn';
      if (s === this.activeScale) btn.classList.add('active');
      btn.textContent = s;
      btn.title = this.scaleDescription(s);
      btn.addEventListener('click', () => this.setScale(s, scaleBar));
      scaleBar.appendChild(btn);
    }
    canvasDiv.appendChild(scaleBar);

    // Stats overlay (top-left)
    this.statsEl = document.createElement('div');
    this.statsEl.style.cssText = `
      position:absolute;top:12px;left:12px;
      padding:10px 14px;max-width:280px;
      background:rgba(11,15,20,0.88);border:1px solid var(--border);border-radius:4px;
      font-size:11px;color:var(--text-secondary);line-height:1.5;z-index:10;
    `;
    this.statsEl.innerHTML = `
      <div style="font-size:13px;font-weight:600;color:var(--text-primary);margin-bottom:4px">Causal Event Atlas</div>
      <div>Each point is a <span style="color:var(--accent)">causal event</span> detected in the observation pipeline.
      Lines show cause-effect relationships between events. The galaxy structure emerges from how events cluster by domain.</div>
      <div id="atlas-stats" style="margin-top:8px;font-family:var(--font-mono);font-size:10px;color:var(--text-muted)"></div>
    `;
    canvasDiv.appendChild(this.statsEl);

    // Domain legend (bottom-left)
    const legend = document.createElement('div');
    legend.style.cssText = `
      position:absolute;bottom:12px;left:12px;
      padding:8px 12px;background:rgba(11,15,20,0.88);
      border:1px solid var(--border);border-radius:4px;
      font-size:10px;color:var(--text-secondary);z-index:10;
    `;
    legend.innerHTML = `
      <div style="font-size:9px;text-transform:uppercase;letter-spacing:0.5px;margin-bottom:3px">Event Domains</div>
      <div style="display:flex;flex-wrap:wrap;gap:6px 12px">
        <div style="display:flex;align-items:center;gap:4px"><span style="width:8px;height:8px;border-radius:50%;background:#00E5FF;display:inline-block"></span> Transit</div>
        <div style="display:flex;align-items:center;gap:4px"><span style="width:8px;height:8px;border-radius:50%;background:#FF4D4D;display:inline-block"></span> Flare</div>
        <div style="display:flex;align-items:center;gap:4px"><span style="width:8px;height:8px;border-radius:50%;background:#2ECC71;display:inline-block"></span> Rotation</div>
        <div style="display:flex;align-items:center;gap:4px"><span style="width:8px;height:8px;border-radius:50%;background:#9944FF;display:inline-block"></span> Eclipse</div>
        <div style="display:flex;align-items:center;gap:4px"><span style="width:8px;height:8px;border-radius:50%;background:#FFB020;display:inline-block"></span> Variability</div>
      </div>
    `;
    canvasDiv.appendChild(legend);

    // Interaction hints (bottom-right)
    const hint = document.createElement('div');
    hint.style.cssText = 'position:absolute;bottom:12px;right:12px;font-size:9px;color:rgba(255,255,255,0.3);z-index:10;pointer-events:none';
    hint.textContent = 'Drag to rotate | Scroll to zoom | Right-drag to pan';
    canvasDiv.appendChild(hint);

    // Three.js setup
    this.initThreeJs(canvasDiv);

    this.resize();
    window.addEventListener('resize', this.resize);
    this.loadData();
    this.animate();

    this.unsubWs = onEvent((ev: LiveEvent) => {
      if (ev.event_type === 'atlas_update') this.loadData();
    });
  }

  /* ── Sidebar ── */

  private buildSidebar(): HTMLElement {
    const sidebar = document.createElement('div');
    sidebar.style.cssText = 'width:260px;border-right:1px solid var(--border);background:var(--bg-panel);overflow-y:auto;overflow-x:hidden;flex-shrink:0;display:flex;flex-direction:column';

    // Header
    const hdr = document.createElement('div');
    hdr.style.cssText = 'padding:12px 14px;border-bottom:1px solid var(--border);font-size:11px;font-weight:600;color:var(--text-primary);text-transform:uppercase;letter-spacing:0.5px';
    hdr.textContent = 'Atlas Configuration';
    sidebar.appendChild(hdr);

    // Scrollable content
    const content = document.createElement('div');
    content.style.cssText = 'flex:1;overflow-y:auto;padding:10px 12px';
    sidebar.appendChild(content);

    // Galaxy Shape section
    this.buildSection(content, 'Galaxy Shape', 'How the causal event network is arranged in 3D space', [
      { label: 'Event count', desc: 'Total causal events to display', min: 30, max: 1200, step: 10, value: this.nodeCount,
        onChange: (v: number) => { this.nodeCount = v; this.loadData(); } },
      { label: 'Spiral arms', desc: 'Number of galaxy arms (event clusters)', min: 2, max: 8, step: 1, value: this.spiralArms,
        onChange: (v: number) => { this.spiralArms = v; this.loadData(); } },
      { label: 'Arm spread', desc: 'How scattered events are within each arm', min: 0.1, max: 1.5, step: 0.1, value: this.armSpread,
        onChange: (v: number) => { this.armSpread = v; this.loadData(); } },
      { label: 'Core density', desc: 'Higher = more events packed near the center', min: 0.3, max: 3.0, step: 0.1, value: this.coreConcentration,
        onChange: (v: number) => { this.coreConcentration = v; this.loadData(); } },
    ]);

    // Animation section
    this.buildSection(content, 'Animation', 'Control how the atlas moves and rotates', [
      { label: 'Rotation speed', desc: 'How fast the view auto-rotates', min: 0, max: 2.0, step: 0.05, value: this.rotationSpeed,
        onChange: (v: number) => {
          this.rotationSpeed = v;
          if (this.controls) this.controls.autoRotateSpeed = v;
        } },
    ]);

    // Display toggles
    const toggleSection = document.createElement('div');
    toggleSection.style.cssText = 'margin-top:12px';
    toggleSection.innerHTML = '<div style="font-size:9px;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:8px;font-weight:600">Display Options</div>';

    const toggles = [
      { label: 'Show coordinate grid', desc: 'Reference grid below the galaxy', checked: this.showGrid,
        onChange: (v: boolean) => { this.showGrid = v; if (this.gridHelper) this.gridHelper.visible = v; } },
      { label: 'Show sector labels', desc: 'Constellation-style sector names', checked: this.showLabels,
        onChange: (v: boolean) => { this.showLabels = v; if (this.starMapLabels) this.starMapLabels.visible = v; } },
      { label: 'Show connections', desc: 'Lines between causally linked events', checked: this.showEdges,
        onChange: (v: boolean) => { this.showEdges = v; this.loadData(); } },
      { label: 'Pulse nodes', desc: 'Gentle brightness pulsing on events', checked: this.pulseNodes,
        onChange: (v: boolean) => { this.pulseNodes = v; } },
    ];

    for (const t of toggles) {
      const row = document.createElement('label');
      row.style.cssText = 'display:flex;align-items:flex-start;gap:8px;margin-bottom:8px;cursor:pointer';

      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = t.checked;
      cb.style.cssText = 'accent-color:#00E5FF;margin-top:2px;flex-shrink:0';
      cb.addEventListener('change', () => t.onChange(cb.checked));
      row.appendChild(cb);

      const info = document.createElement('div');
      info.innerHTML = `<div style="font-size:10px;color:var(--text-primary)">${t.label}</div><div style="font-size:9px;color:var(--text-muted);line-height:1.3">${t.desc}</div>`;
      row.appendChild(info);

      toggleSection.appendChild(row);
    }
    content.appendChild(toggleSection);

    // Presets
    const presetSection = document.createElement('div');
    presetSection.style.cssText = 'margin-top:12px;padding-top:10px;border-top:1px solid var(--border)';
    presetSection.innerHTML = '<div style="font-size:9px;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px;font-weight:600">Quick Presets</div>';

    const presets = [
      { name: 'Compact Cluster', nc: 60, arms: 3, spread: 0.2, core: 2.0, desc: 'Tight event cluster' },
      { name: 'Classic Spiral', nc: 200, arms: 4, spread: 0.4, core: 1.0, desc: 'Default galaxy layout' },
      { name: 'Open Network', nc: 400, arms: 6, spread: 1.0, core: 0.5, desc: 'Wide, loose structure' },
      { name: 'Dense Core', nc: 800, arms: 4, spread: 0.3, core: 2.5, desc: 'Many events, tight core' },
    ];

    for (const p of presets) {
      const btn = document.createElement('button');
      btn.className = 'scale-btn';
      btn.style.cssText = 'width:100%;text-align:left;margin-bottom:4px;padding:6px 10px;font-size:10px';
      btn.innerHTML = `<span style="color:var(--text-primary)">${p.name}</span> <span style="color:var(--text-muted);font-size:9px">${p.desc}</span>`;
      btn.addEventListener('click', () => {
        this.nodeCount = p.nc;
        this.spiralArms = p.arms;
        this.armSpread = p.spread;
        this.coreConcentration = p.core;
        this.syncSlider('Event count', p.nc);
        this.syncSlider('Spiral arms', p.arms);
        this.syncSlider('Arm spread', p.spread);
        this.syncSlider('Core density', p.core);
        this.loadData();
      });
      presetSection.appendChild(btn);
    }
    content.appendChild(presetSection);

    // Star Map Info
    const starMapSection = document.createElement('div');
    starMapSection.style.cssText = 'margin-top:12px;padding-top:10px;border-top:1px solid var(--border)';
    starMapSection.innerHTML = `
      <div style="font-size:9px;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px;font-weight:600">Star Map Sectors</div>
      <div style="font-size:9px;color:var(--text-muted);line-height:1.4;margin-bottom:8px">
        The galaxy is divided into named sectors based on angular position. Each sector contains events from multiple domains.
      </div>
    `;

    const sectorList = document.createElement('div');
    sectorList.style.cssText = 'display:grid;grid-template-columns:1fr 1fr;gap:3px';
    for (let i = 0; i < 8; i++) {
      const name = CONSTELLATION_NAMES[i];
      const angle = (i / 8 * 360).toFixed(0);
      const el = document.createElement('div');
      el.style.cssText = 'font-size:9px;padding:3px 6px;background:var(--bg-surface);border:1px solid var(--border);border-radius:3px';
      el.innerHTML = `<span style="color:var(--accent)">${name}</span> <span style="color:var(--text-muted)">${angle}\u00B0</span>`;
      sectorList.appendChild(el);
    }
    starMapSection.appendChild(sectorList);
    content.appendChild(starMapSection);

    return sidebar;
  }

  /** Update a slider's DOM value and position to match a programmatic change. */
  private syncSlider(label: string, value: number): void {
    const ref = this.sliderRefs.get(label);
    if (!ref) return;
    ref.slider.value = String(value);
    ref.valEl.textContent = String(Number(ref.slider.step) % 1 === 0 ? Math.round(value) : value.toFixed(1));
  }

  private buildSection(
    parent: HTMLElement,
    title: string,
    description: string,
    sliders: { label: string; desc: string; min: number; max: number; step: number; value: number; onChange: (v: number) => void }[],
  ): void {
    const section = document.createElement('div');
    section.style.cssText = 'margin-bottom:14px';

    section.innerHTML = `
      <div style="font-size:9px;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:2px;font-weight:600">${title}</div>
      <div style="font-size:9px;color:var(--text-muted);line-height:1.3;margin-bottom:8px">${description}</div>
    `;

    for (const s of sliders) {
      const row = document.createElement('div');
      row.style.cssText = 'margin-bottom:8px';

      const header = document.createElement('div');
      header.style.cssText = 'display:flex;justify-content:space-between;align-items:center;margin-bottom:2px';

      const labelEl = document.createElement('div');
      labelEl.innerHTML = `<span style="font-size:10px;color:var(--text-primary)">${s.label}</span>`;
      header.appendChild(labelEl);

      const valEl = document.createElement('span');
      valEl.style.cssText = 'font-size:10px;font-family:var(--font-mono);color:var(--accent)';
      valEl.textContent = String(s.value);
      header.appendChild(valEl);
      row.appendChild(header);

      const descEl = document.createElement('div');
      descEl.style.cssText = 'font-size:8px;color:var(--text-muted);margin-bottom:3px';
      descEl.textContent = s.desc;
      row.appendChild(descEl);

      const slider = document.createElement('input');
      slider.type = 'range';
      slider.min = String(s.min);
      slider.max = String(s.max);
      slider.step = String(s.step);
      slider.value = String(s.value);
      slider.style.cssText = 'width:100%;height:3px;accent-color:#00E5FF;cursor:pointer';
      slider.addEventListener('input', () => {
        const v = parseFloat(slider.value);
        valEl.textContent = String(Number.isInteger(s.step) ? Math.round(v) : v.toFixed(1));
        s.onChange(v);
      });
      row.appendChild(slider);

      // Register ref so presets can update this slider
      this.sliderRefs.set(s.label, { slider, valEl });

      section.appendChild(row);
    }

    parent.appendChild(section);
  }

  private scaleDescription(scale: string): string {
    const desc: Record<string, string> = {
      '2h': 'Last 2 hours — recent events only',
      '12h': 'Last 12 hours — short-term patterns',
      '3d': 'Last 3 days — medium-term connections',
      '27d': 'Last 27 days — full rotation cycle',
    };
    return desc[scale] ?? scale;
  }

  /* ── Three.js setup ── */

  private initThreeJs(canvasDiv: HTMLElement): void {
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x050810);
    this.scene.fog = new THREE.FogExp2(0x050810, 0.008);

    this.camera = new THREE.PerspectiveCamera(55, 1, 0.1, 1000);
    this.camera.position.set(0, 10, 18);
    this.camera.lookAt(0, 0, 0);

    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.2;
    canvasDiv.appendChild(this.renderer.domElement);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.05;
    this.controls.autoRotate = true;
    this.controls.autoRotateSpeed = this.rotationSpeed;
    this.controls.minDistance = 3;
    this.controls.maxDistance = 80;

    this.scene.add(new THREE.AmbientLight(0xffffff, 0.4));
    const dl = new THREE.DirectionalLight(0xCCDDFF, 0.3);
    dl.position.set(5, 10, 5);
    this.scene.add(dl);

    this.buildStarfield();
    this.buildNebula();
    this.buildCoordinateGrid();
    this.buildStarMapLabels();

    this.graph = new AtlasGraph(this.scene);
  }

  /* ── Background starfield ── */

  private buildStarfield(): void {
    if (!this.scene) return;
    const count = 6000;
    const rng = seededRng(42);
    const positions = new Float32Array(count * 3);
    const colors = new Float32Array(count * 3);

    for (let i = 0; i < count; i++) {
      const theta = rng() * Math.PI * 2;
      const phi = Math.acos(2 * rng() - 1);
      const r = 60 + rng() * 300;
      positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
      positions[i * 3 + 2] = r * Math.cos(phi);

      const temp = rng();
      if (temp < 0.15) { colors[i*3]=0.7; colors[i*3+1]=0.75; colors[i*3+2]=1; }
      else if (temp < 0.5) { colors[i*3]=0.95; colors[i*3+1]=0.95; colors[i*3+2]=1; }
      else if (temp < 0.8) { colors[i*3]=1; colors[i*3+1]=0.92; colors[i*3+2]=0.8; }
      else { colors[i*3]=1; colors[i*3+1]=0.75; colors[i*3+2]=0.55; }
    }

    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geo.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    this.starfield = new THREE.Points(geo, new THREE.PointsMaterial({
      size: 0.6, vertexColors: true, transparent: true, opacity: 0.8,
      sizeAttenuation: true, depthWrite: false,
    }));
    this.scene.add(this.starfield);
  }

  private buildNebula(): void {
    if (!this.scene) return;
    this.nebulaGroup = new THREE.Group();
    const rng = seededRng(555);
    const nebColors = [0x00E5FF, 0x4400FF, 0xFF4D4D, 0x00FF88, 0x9944FF, 0xFFB020];

    for (let i = 0; i < 8; i++) {
      const canvas = document.createElement('canvas');
      canvas.width = 64; canvas.height = 64;
      const ctx = canvas.getContext('2d')!;
      const grad = ctx.createRadialGradient(32, 32, 0, 32, 32, 32);
      const c = nebColors[i % nebColors.length];
      const r = (c >> 16) & 0xff, g = (c >> 8) & 0xff, b = c & 0xff;
      grad.addColorStop(0, `rgba(${r},${g},${b},0.2)`);
      grad.addColorStop(0.5, `rgba(${r},${g},${b},0.06)`);
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, 64, 64);

      const tex = new THREE.CanvasTexture(canvas);
      const sprite = new THREE.Sprite(new THREE.SpriteMaterial({
        map: tex, transparent: true, blending: THREE.AdditiveBlending, opacity: 0.4,
      }));
      const angle = rng() * Math.PI * 2;
      const dist = 20 + rng() * 40;
      sprite.position.set(Math.cos(angle) * dist, -5 + rng() * 10, Math.sin(angle) * dist);
      sprite.scale.set(15 + rng() * 25, 15 + rng() * 25, 1);
      this.nebulaGroup.add(sprite);
    }
    this.scene.add(this.nebulaGroup);
  }

  /* ── Star map features ── */

  private buildCoordinateGrid(): void {
    if (!this.scene) return;
    this.gridHelper = new THREE.Group();

    // Concentric ring grid (like a radar/star chart)
    const ringMat = new THREE.LineBasicMaterial({ color: 0x1A2530, transparent: true, opacity: 0.4 });
    for (let r = 2; r <= 10; r += 2) {
      const curve = new THREE.EllipseCurve(0, 0, r, r, 0, Math.PI * 2, false, 0);
      const points = curve.getPoints(64);
      const geo = new THREE.BufferGeometry().setFromPoints(points.map(p => new THREE.Vector3(p.x, 0, p.y)));
      const ring = new THREE.Line(geo, ringMat);
      ring.position.y = -0.05;
      this.gridHelper.add(ring);
    }

    // Radial lines (8 sectors)
    const lineMat = new THREE.LineBasicMaterial({ color: 0x1A2530, transparent: true, opacity: 0.3 });
    for (let i = 0; i < 8; i++) {
      const angle = (i / 8) * Math.PI * 2;
      const points = [new THREE.Vector3(0, -0.05, 0), new THREE.Vector3(Math.cos(angle) * 10, -0.05, Math.sin(angle) * 10)];
      const geo = new THREE.BufferGeometry().setFromPoints(points);
      this.gridHelper.add(new THREE.Line(geo, lineMat));
    }

    this.gridHelper.visible = this.showGrid;
    this.scene.add(this.gridHelper);
  }

  private buildStarMapLabels(): void {
    if (!this.scene) return;
    this.starMapLabels = new THREE.Group();

    for (let i = 0; i < 8; i++) {
      const angle = (i / 8) * Math.PI * 2;
      const name = CONSTELLATION_NAMES[i];
      const r = 9.5;

      const canvas = document.createElement('canvas');
      canvas.width = 128; canvas.height = 32;
      const ctx = canvas.getContext('2d')!;
      ctx.fillStyle = 'rgba(0,229,255,0.5)';
      ctx.font = '11px monospace';
      ctx.textAlign = 'center';
      ctx.fillText(name, 64, 20);

      const tex = new THREE.CanvasTexture(canvas);
      const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, transparent: true }));
      sprite.position.set(Math.cos(angle) * r, 0.5, Math.sin(angle) * r);
      sprite.scale.set(3, 0.75, 1);
      this.starMapLabels.add(sprite);
    }

    this.starMapLabels.visible = this.showLabels;
    this.scene.add(this.starMapLabels);
  }

  /* ── Data loading ── */

  private setScale(scale: string, bar: HTMLElement): void {
    this.activeScale = scale;
    bar.querySelectorAll('.scale-btn').forEach((b) => {
      (b as HTMLElement).classList.toggle('active', b.textContent === scale);
    });
    this.loadData();
  }

  private async loadData(): Promise<void> {
    if (!this.graph || !this.scene) return;

    try {
      const result = await fetchAtlasQuery(this.activeScale);
      if (!this.graph) return;
      const nodes: GraphNode[] = [
        { id: result.event_id, domain: 'transit', x: 0, y: 0, z: 0, weight: result.weight },
      ];
      for (const pid of result.parents) {
        nodes.push({
          id: pid, domain: 'rotation', weight: 0.5,
          x: (Math.random() - 0.5) * 6, y: (Math.random() - 0.5) * 1, z: (Math.random() - 0.5) * 6,
        });
      }
      for (const cid of result.children) {
        nodes.push({
          id: cid, domain: 'flare', weight: 0.5,
          x: (Math.random() - 0.5) * 6, y: (Math.random() - 0.5) * 1, z: (Math.random() - 0.5) * 6,
        });
      }
      const edges: GraphEdge[] = [
        ...result.parents.map((p: string) => ({ source: p, target: result.event_id, weight: result.weight })),
        ...result.children.map((c: string) => ({ source: result.event_id, target: c, weight: result.weight })),
      ];
      this.graph.setNodes(nodes);
      if (this.showEdges) this.graph.setEdges(edges, nodes);
      this.updateStats(nodes.length, edges.length);
    } catch {
      if (!this.graph) return;
      const demo = generateGalaxyData(this.activeScale, this.nodeCount, this.spiralArms, this.armSpread, this.coreConcentration);
      this.graph.setNodes(demo.nodes);
      if (this.showEdges) this.graph.setEdges(demo.edges, demo.nodes);
      else this.graph.setEdges([], demo.nodes);
      this.updateStats(demo.nodes.length, this.showEdges ? demo.edges.length : 0);
    }
  }

  private updateStats(nodeCount: number, edgeCount: number): void {
    const statsInner = this.statsEl?.querySelector('#atlas-stats');
    if (statsInner) {
      statsInner.innerHTML = `Events: <span style="color:var(--accent)">${nodeCount}</span> | Connections: <span style="color:var(--accent)">${edgeCount}</span> | Scale: <span style="color:var(--accent)">${this.activeScale}</span> | Arms: <span style="color:var(--accent)">${this.spiralArms}</span>`;
    }
  }

  /* ── Animation ── */

  private resize = (): void => {
    if (!this.renderer || !this.camera || !this.container) return;
    const canvasParent = this.renderer.domElement.parentElement;
    if (!canvasParent) return;
    const w = canvasParent.clientWidth;
    const h = canvasParent.clientHeight;
    if (w === 0 || h === 0) return;
    this.renderer.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  };

  private animate = (): void => {
    this.animFrameId = requestAnimationFrame(this.animate);
    this.time += 0.016;
    this.controls?.update();

    // Slow starfield rotation
    if (this.starfield) this.starfield.rotation.y += 0.00003;

    // Pulse graph nodes
    if (this.pulseNodes && this.graph) {
      const pulse = 0.85 + 0.15 * Math.sin(this.time * 1.5);
      this.graph.setPulse(pulse);
    }

    if (this.renderer && this.scene && this.camera) {
      this.renderer.render(this.scene, this.camera);
    }
  };

  unmount(): void {
    window.removeEventListener('resize', this.resize);
    cancelAnimationFrame(this.animFrameId);
    this.unsubWs?.();

    if (this.starfield) {
      this.scene?.remove(this.starfield);
      this.starfield.geometry.dispose();
      (this.starfield.material as THREE.Material).dispose();
      this.starfield = null;
    }

    if (this.nebulaGroup) {
      for (const child of this.nebulaGroup.children) {
        if (child instanceof THREE.Mesh || child instanceof THREE.Sprite) {
          if ('geometry' in child) (child as THREE.Mesh).geometry.dispose();
          (child.material as THREE.Material).dispose();
        }
      }
      this.scene?.remove(this.nebulaGroup);
      this.nebulaGroup = null;
    }

    if (this.gridHelper) { this.scene?.remove(this.gridHelper); this.gridHelper = null; }
    if (this.starMapLabels) { this.scene?.remove(this.starMapLabels); this.starMapLabels = null; }

    this.graph?.dispose();
    this.controls?.dispose();
    this.renderer?.dispose();

    this.graph = null;
    this.controls = null;
    this.renderer = null;
    this.scene = null;
    this.camera = null;
    this.container = null;
  }
}
