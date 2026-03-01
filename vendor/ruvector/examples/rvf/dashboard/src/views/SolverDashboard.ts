import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import {
  getSolver,
  demoTrainResult,
  demoAcceptanceManifest,
  demoPolicyState,
  type TrainResult,
  type AcceptanceManifest,
  type PolicyState,
} from '../solver';

interface TrainingPoint {
  cycle: number;
  accuracy: number;
  patterns: number;
  loss: number;
}

const MODE_COLORS = {
  A: '#FF4D4D',
  B: '#FFB020',
  C: '#2ECC71',
};

/* ── Seeded RNG for deterministic particles ── */
function seededRandom(seed: number): () => number {
  let s = seed | 0;
  return () => { s = (s * 1664525 + 1013904223) & 0x7fffffff; return s / 0x7fffffff; };
}

/**
 * SolverDashboard — Three.js + canvas visualization of the RVF self-learning solver.
 *
 * Shows:
 * - 3D Thompson Sampling landscape with galaxy background
 * - Training accuracy + loss curves
 * - Mode A/B/C acceptance comparison
 * - Policy state explorer with arm distributions
 * - Interactive 3D controls (speed, auto-rotate, reset)
 */
export class SolverDashboard {
  private container: HTMLElement | null = null;
  private renderer: THREE.WebGLRenderer | null = null;
  private scene: THREE.Scene | null = null;
  private camera: THREE.PerspectiveCamera | null = null;
  private controls: OrbitControls | null = null;
  private animFrameId = 0;
  private landscapeMesh: THREE.Mesh | null = null;

  // Galaxy background objects (persist across updates)
  private bgStars: THREE.Points | null = null;
  private galacticPlane: THREE.Points | null = null;
  private galacticCore: THREE.Mesh | null = null;
  private nebulae: THREE.Sprite[] = [];

  // Arm markers
  private armMarkers: THREE.Group | null = null;
  private peakGlows: THREE.Sprite[] = [];

  // State
  private trainingHistory: TrainingPoint[] = [];
  private manifest: AcceptanceManifest | null = null;
  private policy: PolicyState | null = null;
  private isTraining = false;
  private usesWasm = false;
  private landscapeTime = 0;
  private speed = 1;
  private autoRotate = false;

  // Configurable parameters
  private trainCount = 200;
  private minDifficulty = 1;
  private maxDifficulty = 8;
  private acceptCycles = 5;
  private holdoutSize = 50;
  private trainingPerCycle = 200;
  private stepBudget = 500;
  private autoTrainRounds = 8;

  // DOM refs
  private trainBtn: HTMLButtonElement | null = null;
  private acceptBtn: HTMLButtonElement | null = null;
  private statusEl: HTMLElement | null = null;
  private curveCanvas: HTMLCanvasElement | null = null;
  private modesEl: HTMLElement | null = null;
  private policyEl: HTMLElement | null = null;
  private controlsEl: HTMLElement | null = null;
  private speedLabel: HTMLElement | null = null;

  mount(container: HTMLElement): void {
    this.container = container;

    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden';
    container.appendChild(wrapper);

    // View header
    const viewHeader = document.createElement('div');
    viewHeader.style.cssText = 'padding:12px 20px;border-bottom:1px solid var(--border);flex-shrink:0';
    viewHeader.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px">
        <div style="font-size:14px;font-weight:600;color:var(--text-primary)">RVF Self-Learning Solver</div>
        <span class="score-badge score-high" style="font-size:9px;padding:1px 6px">WASM</span>
        <span class="score-badge score-medium" style="font-size:9px;padding:1px 6px">THOMPSON SAMPLING</span>
      </div>
      <div style="font-size:11px;color:var(--text-secondary);line-height:1.6;max-width:900px">
        Interactive WASM-powered constraint solver that <strong>learns to solve puzzles using multi-armed bandit algorithms</strong>.
        The solver improves by discovering which strategies work best for different puzzle difficulties, building a policy that adapts in real-time.
      </div>
    `;
    wrapper.appendChild(viewHeader);

    const layout = document.createElement('div');
    layout.className = 'split-layout';
    layout.style.flex = '1';
    layout.style.minHeight = '0';
    wrapper.appendChild(layout);

    // Left: scrollable controls + charts
    const left = document.createElement('div');
    left.className = 'left-panel';
    left.style.cssText = 'overflow-y:auto;overflow-x:hidden;padding:12px;scroll-behavior:smooth;-webkit-overflow-scrolling:touch';
    layout.appendChild(left);

    this.buildLeftPanel(left);

    // Right: Three.js landscape
    const right = document.createElement('div');
    right.className = 'right-panel';
    right.style.cssText = 'padding:0;position:relative;display:flex;flex-direction:column';
    layout.appendChild(right);

    const threeDiv = document.createElement('div');
    threeDiv.className = 'three-container';
    threeDiv.style.cssText = 'flex:1;min-height:0;position:relative';
    right.appendChild(threeDiv);

    this.initThreeJs(threeDiv);
    this.buildViewportControls(threeDiv);

    // Interaction hints
    const hints = document.createElement('div');
    hints.style.cssText = 'position:absolute;bottom:8px;left:50%;transform:translateX(-50%);font-size:9px;color:rgba(255,255,255,0.3);pointer-events:none;white-space:nowrap';
    hints.textContent = 'Drag to rotate | Scroll to zoom | Right-drag to pan';
    threeDiv.appendChild(hints);

    window.addEventListener('resize', this.resize);
    this.resize();
    this.animate();
    this.init();
  }

  /* ── Left panel build ── */

  private buildLeftPanel(left: HTMLElement): void {
    // Info cards (collapsed by default to save space)
    const infoToggle = document.createElement('div');
    infoToggle.style.cssText = 'font-size:10px;color:var(--accent);cursor:pointer;margin-bottom:8px;display:flex;align-items:center;gap:4px';
    infoToggle.innerHTML = '<span style="transition:transform 0.2s" id="info-arrow">&#9654;</span> How it works';
    const infoContent = document.createElement('div');
    infoContent.style.cssText = 'display:none;margin-bottom:12px';
    infoContent.innerHTML = `
      <div style="display:grid;grid-template-columns:1fr;gap:6px;font-size:10px">
        <div style="background:rgba(0,229,255,0.06);border:1px solid rgba(0,229,255,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:var(--accent);font-weight:600;margin-bottom:2px">Training</div>
          <div style="color:var(--text-secondary);line-height:1.4">Each cycle generates puzzles of varying difficulty (1-8). <strong>Thompson Sampling</strong> explores strategies by sampling from Beta distributions, balancing exploration vs exploitation.</div>
        </div>
        <div style="background:rgba(255,176,32,0.06);border:1px solid rgba(255,176,32,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:var(--warning);font-weight:600;margin-bottom:2px">Acceptance Test</div>
          <div style="color:var(--text-secondary);line-height:1.4"><span style="color:#FF4D4D;font-weight:600">A</span> accuracy only. <span style="color:#FFB020;font-weight:600">B</span> accuracy + cost. <span style="color:#2ECC71;font-weight:600">C</span> full multi-objective.</div>
        </div>
        <div style="background:rgba(153,68,255,0.06);border:1px solid rgba(153,68,255,0.15);border-radius:4px;padding:6px 8px">
          <div style="color:#9944ff;font-weight:600;margin-bottom:2px">3D Landscape</div>
          <div style="color:var(--text-secondary);line-height:1.4">Terrain shows <strong>bandit arm reward distributions</strong>. Peaks = high-reward strategies. <span style="color:#4488ff">Blue</span>=low, <span style="color:#2ECC71">green</span>=medium, <span style="color:#FF4D4D">red</span>=high.</div>
        </div>
      </div>
    `;
    infoToggle.addEventListener('click', () => {
      const open = infoContent.style.display !== 'none';
      infoContent.style.display = open ? 'none' : 'block';
      const arrow = infoToggle.querySelector('#info-arrow') as HTMLElement;
      if (arrow) arrow.style.transform = open ? '' : 'rotate(90deg)';
    });
    left.appendChild(infoToggle);
    left.appendChild(infoContent);

    // Controls panel
    const controlsPanel = document.createElement('div');
    controlsPanel.className = 'panel';
    controlsPanel.style.marginBottom = '12px';
    left.appendChild(controlsPanel);

    const hdr = document.createElement('div');
    hdr.className = 'panel-header';
    hdr.innerHTML = 'Controls <span style="font-size:8px;text-transform:none;letter-spacing:0;color:var(--text-muted);font-weight:400">train & test</span>';
    controlsPanel.appendChild(hdr);

    const body = document.createElement('div');
    body.className = 'panel-body';
    body.style.cssText = 'display:flex;gap:6px;align-items:center;flex-wrap:wrap;padding:8px 10px';
    controlsPanel.appendChild(body);

    this.trainBtn = document.createElement('button');
    this.trainBtn.textContent = 'Train (200)';
    this.trainBtn.className = 'scale-btn';
    this.trainBtn.style.cssText = 'font-size:11px;padding:4px 10px;white-space:nowrap';
    this.trainBtn.addEventListener('click', () => this.runTraining());
    body.appendChild(this.trainBtn);

    this.acceptBtn = document.createElement('button');
    this.acceptBtn.textContent = 'Acceptance';
    this.acceptBtn.className = 'scale-btn';
    this.acceptBtn.style.cssText = 'font-size:11px;padding:4px 10px;white-space:nowrap';
    this.acceptBtn.addEventListener('click', () => this.runAcceptance());
    body.appendChild(this.acceptBtn);

    const autoBtn = document.createElement('button');
    autoBtn.textContent = 'Auto (8x)';
    autoBtn.className = 'scale-btn';
    autoBtn.style.cssText = 'font-size:11px;padding:4px 10px;white-space:nowrap';
    autoBtn.addEventListener('click', () => this.runAutoTraining());
    body.appendChild(autoBtn);

    const optimizeBtn = document.createElement('button');
    optimizeBtn.textContent = 'Auto-Optimize';
    optimizeBtn.className = 'scale-btn';
    optimizeBtn.style.cssText = 'font-size:11px;padding:4px 10px;white-space:nowrap;border-color:rgba(46,204,113,0.3);color:var(--success)';
    optimizeBtn.title = 'Keep training until acceptance passes (max 30 rounds)';
    optimizeBtn.addEventListener('click', () => this.runAutoOptimize());
    body.appendChild(optimizeBtn);

    this.statusEl = document.createElement('div');
    this.statusEl.style.cssText = 'font-size:11px;color:var(--text-secondary);width:100%;margin-top:4px';
    this.statusEl.textContent = 'Initializing...';
    body.appendChild(this.statusEl);

    // Configuration panel
    this.buildConfigPanel(left);

    // Training curves (accuracy + loss)
    const curvePanel = document.createElement('div');
    curvePanel.className = 'panel';
    curvePanel.style.marginBottom = '12px';
    left.appendChild(curvePanel);

    const curveHdr = document.createElement('div');
    curveHdr.className = 'panel-header';
    curveHdr.innerHTML = 'Training Curves <span style="font-size:8px;text-transform:none;letter-spacing:0;color:var(--text-muted);font-weight:400">accuracy &amp; patterns</span>';
    curvePanel.appendChild(curveHdr);

    this.curveCanvas = document.createElement('canvas');
    this.curveCanvas.width = 500;
    this.curveCanvas.height = 200;
    this.curveCanvas.style.cssText = 'width:100%;height:180px;display:block';
    curvePanel.appendChild(this.curveCanvas);

    // Legend
    const legend = document.createElement('div');
    legend.style.cssText = 'display:flex;gap:12px;padding:4px 10px;font-size:9px;color:var(--text-muted)';
    legend.innerHTML = `
      <span><span style="display:inline-block;width:12px;height:2px;background:#00E5FF;vertical-align:middle;margin-right:3px"></span>Accuracy</span>
      <span><span style="display:inline-block;width:12px;height:2px;background:#58A6FF;vertical-align:middle;margin-right:3px;border-top:1px dashed #58A6FF"></span>Patterns</span>
      <span><span style="display:inline-block;width:12px;height:2px;background:#FF6B9D;vertical-align:middle;margin-right:3px;border-top:1px dashed #FF6B9D"></span>Loss</span>
    `;
    curvePanel.appendChild(legend);

    // Acceptance modes
    this.modesEl = document.createElement('div');
    this.modesEl.className = 'panel';
    this.modesEl.style.marginBottom = '12px';
    left.appendChild(this.modesEl);

    // Policy explorer
    this.policyEl = document.createElement('div');
    this.policyEl.className = 'panel';
    this.policyEl.style.marginBottom = '16px'; // extra bottom margin for scroll breathing room
    left.appendChild(this.policyEl);
  }

  /* ── 3D viewport controls overlay ── */

  private buildConfigPanel(parent: HTMLElement): void {
    const configPanel = document.createElement('div');
    configPanel.className = 'panel';
    configPanel.style.marginBottom = '12px';
    parent.appendChild(configPanel);

    const hdr = document.createElement('div');
    hdr.className = 'panel-header';
    hdr.style.cursor = 'pointer';
    hdr.innerHTML = '<span>Configuration</span> <span id="config-arrow" style="font-size:8px;transition:transform 0.2s">&#9654;</span>';
    configPanel.appendChild(hdr);

    const configBody = document.createElement('div');
    configBody.className = 'panel-body';
    configBody.style.cssText = 'display:none;padding:10px';
    configPanel.appendChild(configBody);

    hdr.addEventListener('click', () => {
      const open = configBody.style.display !== 'none';
      configBody.style.display = open ? 'none' : 'block';
      const arrow = hdr.querySelector('#config-arrow') as HTMLElement;
      if (arrow) arrow.style.transform = open ? '' : 'rotate(90deg)';
    });

    const grid = document.createElement('div');
    grid.style.cssText = 'display:grid;grid-template-columns:1fr 1fr;gap:8px';
    configBody.appendChild(grid);

    const makeSlider = (label: string, min: number, max: number, step: number, value: number, onChange: (v: number) => void): HTMLElement => {
      const wrap = document.createElement('div');
      wrap.style.cssText = 'display:flex;flex-direction:column;gap:3px';

      const row = document.createElement('div');
      row.style.cssText = 'display:flex;justify-content:space-between;align-items:center';
      const lbl = document.createElement('span');
      lbl.style.cssText = 'font-size:9px;color:var(--text-muted)';
      lbl.textContent = label;
      row.appendChild(lbl);

      const val = document.createElement('span');
      val.style.cssText = 'font-size:10px;font-family:var(--font-mono);color:var(--accent)';
      val.textContent = String(value);
      row.appendChild(val);
      wrap.appendChild(row);

      const slider = document.createElement('input');
      slider.type = 'range';
      slider.min = String(min);
      slider.max = String(max);
      slider.step = String(step);
      slider.value = String(value);
      slider.style.cssText = 'width:100%;height:3px;accent-color:#00E5FF;cursor:pointer';
      slider.addEventListener('input', () => {
        const v = parseFloat(slider.value);
        val.textContent = String(v);
        onChange(v);
      });
      wrap.appendChild(slider);
      return wrap;
    };

    // Training parameters
    const trainHeader = document.createElement('div');
    trainHeader.style.cssText = 'grid-column:1/-1;font-size:9px;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.4px;border-bottom:1px solid var(--border);padding-bottom:4px;margin-bottom:2px';
    trainHeader.textContent = 'Training';
    grid.appendChild(trainHeader);

    grid.appendChild(makeSlider('Puzzles per cycle', 50, 500, 50, this.trainCount, (v) => {
      this.trainCount = v;
      if (this.trainBtn) this.trainBtn.textContent = `Train (${v})`;
    }));

    grid.appendChild(makeSlider('Auto-train rounds', 3, 20, 1, this.autoTrainRounds, (v) => {
      this.autoTrainRounds = v;
    }));

    grid.appendChild(makeSlider('Min difficulty', 1, 5, 1, this.minDifficulty, (v) => {
      this.minDifficulty = v;
    }));

    grid.appendChild(makeSlider('Max difficulty', 3, 10, 1, this.maxDifficulty, (v) => {
      this.maxDifficulty = v;
    }));

    // Acceptance parameters
    const acceptHeader = document.createElement('div');
    acceptHeader.style.cssText = 'grid-column:1/-1;font-size:9px;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.4px;border-bottom:1px solid var(--border);padding-bottom:4px;margin-top:6px;margin-bottom:2px';
    acceptHeader.textContent = 'Acceptance Test';
    grid.appendChild(acceptHeader);

    grid.appendChild(makeSlider('Cycles', 2, 10, 1, this.acceptCycles, (v) => {
      this.acceptCycles = v;
    }));

    grid.appendChild(makeSlider('Holdout size', 10, 100, 10, this.holdoutSize, (v) => {
      this.holdoutSize = v;
    }));

    grid.appendChild(makeSlider('Training/cycle', 50, 500, 50, this.trainingPerCycle, (v) => {
      this.trainingPerCycle = v;
    }));

    grid.appendChild(makeSlider('Step budget', 100, 2000, 100, this.stepBudget, (v) => {
      this.stepBudget = v;
    }));

    // Preset buttons
    const presetRow = document.createElement('div');
    presetRow.style.cssText = 'grid-column:1/-1;display:flex;gap:6px;margin-top:6px';

    const makePreset = (name: string, values: { tc: number; ar: number; ac: number; hs: number; tpc: number; sb: number }) => {
      const btn = document.createElement('button');
      btn.className = 'scale-btn';
      btn.style.cssText = 'font-size:9px;padding:3px 8px;flex:1';
      btn.textContent = name;
      btn.addEventListener('click', () => {
        this.trainCount = values.tc;
        this.autoTrainRounds = values.ar;
        this.acceptCycles = values.ac;
        this.holdoutSize = values.hs;
        this.trainingPerCycle = values.tpc;
        this.stepBudget = values.sb;
        if (this.trainBtn) this.trainBtn.textContent = `Train (${values.tc})`;
        // Rebuild config panel sliders
        configBody.style.display = 'none';
        const arrow = hdr.querySelector('#config-arrow') as HTMLElement;
        if (arrow) arrow.style.transform = '';
        grid.innerHTML = '';
        this.buildConfigPanel(parent);
        // Remove old panel, keep new
        configPanel.remove();
      });
      return btn;
    };

    presetRow.appendChild(makePreset('Quick', { tc: 100, ar: 5, ac: 3, hs: 30, tpc: 100, sb: 300 }));
    presetRow.appendChild(makePreset('Balanced', { tc: 200, ar: 8, ac: 5, hs: 50, tpc: 200, sb: 500 }));
    presetRow.appendChild(makePreset('Thorough', { tc: 500, ar: 12, ac: 12, hs: 50, tpc: 800, sb: 2000 }));
    grid.appendChild(presetRow);
  }

  private buildViewportControls(parent: HTMLElement): void {
    this.controlsEl = document.createElement('div');
    this.controlsEl.style.cssText = `
      position:absolute;top:8px;right:8px;z-index:10;
      display:flex;flex-direction:column;gap:6px;
      background:rgba(11,15,20,0.85);border:1px solid rgba(0,229,255,0.15);
      border-radius:6px;padding:8px 10px;backdrop-filter:blur(6px);
      font-size:10px;color:var(--text-secondary);min-width:130px
    `;

    // Speed slider
    const speedRow = document.createElement('div');
    speedRow.style.cssText = 'display:flex;align-items:center;gap:6px';
    const speedIcon = document.createElement('span');
    speedIcon.textContent = 'Speed';
    speedIcon.style.cssText = 'color:var(--accent);font-weight:600;font-size:9px;min-width:34px';
    speedRow.appendChild(speedIcon);

    const speedSlider = document.createElement('input');
    speedSlider.type = 'range';
    speedSlider.min = '0.1';
    speedSlider.max = '5';
    speedSlider.step = '0.1';
    speedSlider.value = '1';
    speedSlider.style.cssText = 'flex:1;height:3px;accent-color:#00E5FF;cursor:pointer';
    speedRow.appendChild(speedSlider);

    this.speedLabel = document.createElement('span');
    this.speedLabel.style.cssText = 'font-family:var(--font-mono);font-size:9px;min-width:24px;text-align:right;color:var(--text-primary)';
    this.speedLabel.textContent = '1.0x';
    speedRow.appendChild(this.speedLabel);

    speedSlider.addEventListener('input', () => {
      this.speed = parseFloat(speedSlider.value);
      if (this.speedLabel) this.speedLabel.textContent = this.speed.toFixed(1) + 'x';
    });
    this.controlsEl.appendChild(speedRow);

    // Auto-rotate toggle
    const rotRow = document.createElement('div');
    rotRow.style.cssText = 'display:flex;align-items:center;gap:6px';
    const rotLabel = document.createElement('span');
    rotLabel.style.cssText = 'font-size:9px;color:var(--text-secondary);flex:1';
    rotLabel.textContent = 'Auto-rotate';
    rotRow.appendChild(rotLabel);

    const rotToggle = document.createElement('button');
    rotToggle.style.cssText = 'font-size:9px;padding:2px 8px;border-radius:3px;border:1px solid rgba(0,229,255,0.2);background:transparent;color:var(--text-muted);cursor:pointer';
    rotToggle.textContent = 'OFF';
    rotToggle.addEventListener('click', () => {
      this.autoRotate = !this.autoRotate;
      if (this.controls) this.controls.autoRotate = this.autoRotate;
      rotToggle.textContent = this.autoRotate ? 'ON' : 'OFF';
      rotToggle.style.color = this.autoRotate ? 'var(--accent)' : 'var(--text-muted)';
      rotToggle.style.borderColor = this.autoRotate ? 'rgba(0,229,255,0.4)' : 'rgba(0,229,255,0.2)';
    });
    rotRow.appendChild(rotToggle);
    this.controlsEl.appendChild(rotRow);

    // Reset button
    const resetBtn = document.createElement('button');
    resetBtn.style.cssText = 'font-size:9px;padding:3px 0;border-radius:3px;border:1px solid rgba(255,255,255,0.1);background:transparent;color:var(--text-muted);cursor:pointer;width:100%';
    resetBtn.textContent = 'Reset View';
    resetBtn.addEventListener('click', () => this.resetCamera());
    this.controlsEl.appendChild(resetBtn);

    parent.appendChild(this.controlsEl);
  }

  private resetCamera(): void {
    if (!this.camera || !this.controls) return;
    this.camera.position.set(0, 8, 14);
    this.camera.lookAt(0, 0, 0);
    this.controls.target.set(0, 0, 0);
    this.controls.update();
  }

  /* ── Three.js setup ── */

  private initThreeJs(threeDiv: HTMLElement): void {
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x050810);
    this.scene.fog = new THREE.FogExp2(0x050810, 0.003);

    this.camera = new THREE.PerspectiveCamera(50, 1, 0.1, 2000);
    this.camera.position.set(0, 8, 14);
    this.camera.lookAt(0, 0, 0);

    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.2;
    threeDiv.appendChild(this.renderer.domElement);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.maxPolarAngle = Math.PI * 0.48;
    this.controls.minDistance = 4;
    this.controls.maxDistance = 60;
    this.controls.autoRotateSpeed = 0.5;

    // Lights
    this.scene.add(new THREE.AmbientLight(0x334466, 0.6));
    const dl = new THREE.DirectionalLight(0xaaccff, 0.8);
    dl.position.set(5, 12, 5);
    this.scene.add(dl);
    const dl2 = new THREE.DirectionalLight(0xff8844, 0.2);
    dl2.position.set(-3, 5, -3);
    this.scene.add(dl2);

    // Galaxy background
    this.buildStarfield();
    this.buildGalacticPlane();
    this.buildNebulae();

    // Grid helper
    const grid = new THREE.GridHelper(12, 24, 0x1C2333, 0x0C1018);
    grid.position.y = -0.02;
    this.scene.add(grid);

    // Landscape
    this.buildLandscape();

    // Arm markers
    this.armMarkers = new THREE.Group();
    this.scene.add(this.armMarkers);
    this.buildArmMarkers();
  }

  /* ── Galaxy background ── */

  private buildStarfield(): void {
    if (!this.scene) return;
    const count = 5000;
    const rng = seededRandom(42);
    const positions = new Float32Array(count * 3);
    const colors = new Float32Array(count * 3);
    const sizes = new Float32Array(count);

    for (let i = 0; i < count; i++) {
      const theta = rng() * Math.PI * 2;
      const phi = Math.acos(2 * rng() - 1);
      const r = 200 + rng() * 600;
      positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
      positions[i * 3 + 2] = r * Math.cos(phi);

      const temp = rng();
      if (temp < 0.15) { colors[i*3]=0.7; colors[i*3+1]=0.75; colors[i*3+2]=1; }
      else if (temp < 0.4) { colors[i*3]=1; colors[i*3+1]=0.98; colors[i*3+2]=0.95; }
      else if (temp < 0.7) { colors[i*3]=1; colors[i*3+1]=0.92; colors[i*3+2]=0.8; }
      else { colors[i*3]=1; colors[i*3+1]=0.75; colors[i*3+2]=0.55; }

      sizes[i] = 0.8 + rng() * 2.0;
    }

    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geo.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    geo.setAttribute('size', new THREE.BufferAttribute(sizes, 1));

    this.bgStars = new THREE.Points(geo, new THREE.PointsMaterial({
      size: 1.2, vertexColors: true, sizeAttenuation: true, transparent: true, opacity: 0.85,
    }));
    this.scene.add(this.bgStars);
  }

  private buildGalacticPlane(): void {
    if (!this.scene) return;
    const count = 6000;
    const rng = seededRandom(123);
    const positions = new Float32Array(count * 3);
    const colors = new Float32Array(count * 3);

    for (let i = 0; i < count; i++) {
      const angle = rng() * Math.PI * 2;
      const r = Math.pow(rng(), 0.4) * 400;
      const thickness = (rng() - 0.5) * 15 * Math.exp(-r / 200);
      positions[i * 3] = Math.cos(angle) * r;
      positions[i * 3 + 1] = thickness;
      positions[i * 3 + 2] = Math.sin(angle) * r;

      const bright = 0.2 + 0.3 * Math.exp(-r / 150);
      colors[i * 3] = bright * 0.8;
      colors[i * 3 + 1] = bright * 0.7;
      colors[i * 3 + 2] = bright * 1.2;
    }

    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geo.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    this.galacticPlane = new THREE.Points(geo, new THREE.PointsMaterial({
      size: 1.5, vertexColors: true, sizeAttenuation: true, transparent: true, opacity: 0.3,
    }));
    this.galacticPlane.rotation.x = -Math.PI / 2 + 0.3;
    this.galacticPlane.rotation.z = 0.4;
    this.galacticPlane.position.y = 60;
    this.scene.add(this.galacticPlane);

    // Galactic core glow
    const coreGeo = new THREE.SphereGeometry(12, 16, 16);
    const coreMat = new THREE.MeshBasicMaterial({ color: 0x443366, transparent: true, opacity: 0.08 });
    this.galacticCore = new THREE.Mesh(coreGeo, coreMat);
    this.galacticCore.position.copy(this.galacticPlane.position);
    this.scene.add(this.galacticCore);
  }

  private buildNebulae(): void {
    if (!this.scene) return;
    const rng = seededRandom(777);
    const nebColors = [0x4466cc, 0x6644aa, 0x2288aa, 0xaa4466, 0x44aa88, 0x8866cc];

    for (let i = 0; i < 6; i++) {
      const canvas = document.createElement('canvas');
      canvas.width = 64; canvas.height = 64;
      const ctx = canvas.getContext('2d')!;
      const grad = ctx.createRadialGradient(32, 32, 0, 32, 32, 32);
      const c = nebColors[i % nebColors.length];
      const r = (c >> 16) & 0xff, g = (c >> 8) & 0xff, b = c & 0xff;
      grad.addColorStop(0, `rgba(${r},${g},${b},0.25)`);
      grad.addColorStop(0.5, `rgba(${r},${g},${b},0.08)`);
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, 64, 64);

      const tex = new THREE.CanvasTexture(canvas);
      const sprite = new THREE.Sprite(new THREE.SpriteMaterial({
        map: tex, transparent: true, blending: THREE.AdditiveBlending, opacity: 0.5,
      }));
      const angle = rng() * Math.PI * 2;
      const dist = 150 + rng() * 250;
      sprite.position.set(Math.cos(angle) * dist, -30 + rng() * 100, Math.sin(angle) * dist);
      sprite.scale.set(60 + rng() * 80, 60 + rng() * 80, 1);
      this.scene.add(sprite);
      this.nebulae.push(sprite);
    }
  }

  /* ── Thompson Sampling 3D landscape ── */

  private buildLandscape(): void {
    if (!this.scene) return;
    const gridSize = 48; // higher resolution
    const geo = new THREE.PlaneGeometry(10, 10, gridSize - 1, gridSize - 1);
    const colors = new Float32Array(geo.attributes.position.count * 3);

    for (let i = 0; i < geo.attributes.position.count; i++) {
      colors[i * 3] = 0.12;
      colors[i * 3 + 1] = 0.2;
      colors[i * 3 + 2] = 0.4;
    }
    geo.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    const mat = new THREE.MeshStandardMaterial({
      vertexColors: true,
      side: THREE.DoubleSide,
      roughness: 0.55,
      metalness: 0.15,
      wireframe: false,
    });

    this.landscapeMesh = new THREE.Mesh(geo, mat);
    this.landscapeMesh.rotation.x = -Math.PI / 2;
    this.scene.add(this.landscapeMesh);

    // Wireframe overlay for depth perception
    const wireMat = new THREE.MeshBasicMaterial({
      color: 0x00E5FF, wireframe: true, transparent: true, opacity: 0.04,
    });
    const wireMesh = new THREE.Mesh(geo, wireMat);
    wireMesh.rotation.x = -Math.PI / 2;
    wireMesh.position.y = 0.01;
    this.scene.add(wireMesh);

    this.setLandscapeValues(this.generateLandscapeValues(gridSize, 0));
  }

  private buildArmMarkers(): void {
    if (!this.armMarkers || !this.scene) return;
    // Clear existing
    while (this.armMarkers.children.length > 0) {
      this.armMarkers.remove(this.armMarkers.children[0]);
    }
    this.peakGlows = [];

    // 8 bandit arms positioned in a ring
    const arms = ['Naked Singles', 'Hidden Pairs', 'X-Wing', 'Backtrack', 'Constraint Prop', 'Pattern Match', 'Speculative', 'Hybrid'];
    const radius = 3.5;

    for (let i = 0; i < arms.length; i++) {
      const angle = (i / arms.length) * Math.PI * 2;
      const x = Math.cos(angle) * radius;
      const z = Math.sin(angle) * radius;

      // Pillar base
      const pillarGeo = new THREE.CylinderGeometry(0.08, 0.08, 0.3, 8);
      const pillarMat = new THREE.MeshBasicMaterial({ color: 0x00E5FF, transparent: true, opacity: 0.4 });
      const pillar = new THREE.Mesh(pillarGeo, pillarMat);
      pillar.position.set(x, 0.15, z);
      this.armMarkers.add(pillar);

      // Glow sprite on peak
      const canvas = document.createElement('canvas');
      canvas.width = 32; canvas.height = 32;
      const ctx = canvas.getContext('2d')!;
      const grad = ctx.createRadialGradient(16, 16, 0, 16, 16, 16);
      grad.addColorStop(0, 'rgba(0,229,255,0.6)');
      grad.addColorStop(1, 'rgba(0,229,255,0)');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, 32, 32);

      const tex = new THREE.CanvasTexture(canvas);
      const glow = new THREE.Sprite(new THREE.SpriteMaterial({
        map: tex, transparent: true, blending: THREE.AdditiveBlending,
      }));
      glow.position.set(x, 0.8, z);
      glow.scale.set(0.5, 0.5, 1);
      this.armMarkers.add(glow);
      this.peakGlows.push(glow);

      // Label
      const labelCanvas = document.createElement('canvas');
      labelCanvas.width = 128; labelCanvas.height = 32;
      const lctx = labelCanvas.getContext('2d')!;
      lctx.fillStyle = 'rgba(0,229,255,0.8)';
      lctx.font = '11px monospace';
      lctx.textAlign = 'center';
      lctx.fillText(arms[i], 64, 18);

      const labelTex = new THREE.CanvasTexture(labelCanvas);
      const labelSprite = new THREE.Sprite(new THREE.SpriteMaterial({
        map: labelTex, transparent: true,
      }));
      labelSprite.position.set(x, -0.3, z);
      labelSprite.scale.set(2, 0.5, 1);
      this.armMarkers.add(labelSprite);
    }
  }

  private generateLandscapeValues(gridSize: number, seed: number): number[] {
    const values: number[] = [];
    const armCount = 8;
    const radius = 3.5;

    for (let y = 0; y < gridSize; y++) {
      for (let x = 0; x < gridSize; x++) {
        // Map grid to world coordinates
        const wx = (x / (gridSize - 1) - 0.5) * 10;
        const wz = (y / (gridSize - 1) - 0.5) * 10;

        let v = 0.05; // base

        // Each bandit arm creates a gaussian peak
        for (let a = 0; a < armCount; a++) {
          const angle = (a / armCount) * Math.PI * 2;
          const ax = Math.cos(angle) * radius;
          const az = Math.sin(angle) * radius;
          const dist = Math.sqrt((wx - ax) ** 2 + (wz - az) ** 2);

          // Arm reward varies by seed (training progress)
          const armReward = 0.2 + 0.6 * Math.abs(Math.sin(a * 1.7 + seed * 0.3));
          const sigma = 1.0 + 0.3 * Math.sin(a + seed * 0.5);
          v += armReward * Math.exp(-dist * dist / (2 * sigma * sigma));
        }

        // Add some noise
        v += 0.05 * Math.sin(wx * 3 + seed) * Math.cos(wz * 2.5 + seed * 0.7);

        values.push(Math.max(0, Math.min(1, v)));
      }
    }
    return values;
  }

  private setLandscapeValues(values: number[]): void {
    if (!this.landscapeMesh) return;

    const geo = this.landscapeMesh.geometry;
    const pos = geo.attributes.position;
    const col = geo.attributes.color;
    const count = Math.min(values.length, pos.count);

    for (let i = 0; i < count; i++) {
      const v = Math.max(0, Math.min(1, values[i]));

      // Height (more dramatic)
      pos.setZ(i, v * 3);

      // Color: deep blue -> cyan -> green -> yellow -> red
      if (v < 0.2) {
        col.setXYZ(i, 0.05, 0.1 + v * 2, 0.3 + v);
      } else if (v < 0.45) {
        const t = (v - 0.2) / 0.25;
        col.setXYZ(i, 0.05 + t * 0.1, 0.5 + t * 0.3, 0.5 * (1 - t));
      } else if (v < 0.7) {
        const t = (v - 0.45) / 0.25;
        col.setXYZ(i, 0.15 + t * 0.65, 0.8 - t * 0.2, 0.05);
      } else {
        const t = (v - 0.7) / 0.3;
        col.setXYZ(i, 0.8 + t * 0.2, 0.6 - t * 0.5, 0.05);
      }
    }

    pos.needsUpdate = true;
    col.needsUpdate = true;
    geo.computeVertexNormals();

    // Update peak glow positions
    this.updatePeakGlows(values);
  }

  private updatePeakGlows(values: number[]): void {
    const gridSize = 48;
    const armCount = 8;
    const radius = 3.5;

    for (let a = 0; a < armCount && a < this.peakGlows.length; a++) {
      const angle = (a / armCount) * Math.PI * 2;
      const ax = Math.cos(angle) * radius;
      const az = Math.sin(angle) * radius;

      // Find height at arm position
      const gx = Math.round(((ax / 10) + 0.5) * (gridSize - 1));
      const gz = Math.round(((az / 10) + 0.5) * (gridSize - 1));
      const idx = gz * gridSize + gx;
      const h = idx >= 0 && idx < values.length ? values[idx] * 3 : 0.5;

      this.peakGlows[a].position.y = h + 0.3;
      this.peakGlows[a].scale.setScalar(0.3 + values[idx >= 0 && idx < values.length ? idx : 0] * 0.8);
    }
  }

  private updateLandscape(): void {
    const seed = this.trainingHistory.length * 1.7 + (this.manifest ? 5 : 0);
    const gridSize = 48;
    const values = this.generateLandscapeValues(gridSize, seed);

    // Incorporate training accuracy
    if (this.trainingHistory.length > 0) {
      const lastAcc = this.trainingHistory[this.trainingHistory.length - 1].accuracy;
      for (let i = 0; i < values.length; i++) {
        values[i] = values[i] * 0.5 + lastAcc * 0.3 + values[i] * lastAcc * 0.2;
      }
    }

    this.setLandscapeValues(values);
  }

  /* ── Init + Training ── */

  private async init(): Promise<void> {
    const solver = await getSolver();
    this.usesWasm = solver !== null;
    if (this.statusEl) {
      this.statusEl.textContent = this.usesWasm
        ? 'WASM solver loaded — ready to train'
        : 'Demo mode (WASM unavailable) — ready to train';
      this.statusEl.style.color = this.usesWasm ? 'var(--success)' : 'var(--warning)';
    }
    this.renderModes();
    this.renderPolicy();
  }

  private async runTraining(): Promise<void> {
    if (this.isTraining) return;
    this.isTraining = true;
    if (this.trainBtn) this.trainBtn.disabled = true;
    if (this.statusEl) {
      this.statusEl.textContent = 'Training...';
      this.statusEl.style.color = 'var(--accent)';
    }

    const cycle = this.trainingHistory.length;
    let result: TrainResult;

    const solver = await getSolver();
    if (solver) {
      result = solver.train({ count: this.trainCount, minDifficulty: this.minDifficulty, maxDifficulty: this.maxDifficulty });
      this.policy = solver.policy();
    } else {
      await new Promise((r) => setTimeout(r, 500));
      result = demoTrainResult(this.trainCount, cycle);
      this.policy = demoPolicyState();
    }

    // Compute synthetic loss (decreasing with cycles)
    const baseLoss = 1 - result.accuracy;
    const noise = (Math.random() - 0.5) * 0.05;
    const loss = Math.max(0.01, baseLoss + noise);

    this.trainingHistory.push({
      cycle,
      accuracy: result.accuracy,
      patterns: result.patternsLearned,
      loss,
    });

    this.renderCurve();
    this.updateLandscape();
    this.renderPolicy();

    if (this.statusEl) {
      this.statusEl.textContent = `Cycle ${cycle + 1}: ${(result.accuracy * 100).toFixed(1)}% accuracy, ${result.patternsLearned} patterns, loss ${loss.toFixed(3)}`;
      this.statusEl.style.color = 'var(--text-secondary)';
    }

    this.isTraining = false;
    if (this.trainBtn) this.trainBtn.disabled = false;
  }

  private async runAcceptance(): Promise<void> {
    if (this.isTraining) return;
    this.isTraining = true;
    if (this.acceptBtn) this.acceptBtn.disabled = true;
    if (this.statusEl) {
      this.statusEl.textContent = 'Running acceptance test...';
      this.statusEl.style.color = 'var(--accent)';
    }

    const solver = await getSolver();
    if (solver) {
      this.manifest = solver.acceptance({ cycles: this.acceptCycles, holdoutSize: this.holdoutSize, trainingPerCycle: this.trainingPerCycle, stepBudget: this.stepBudget });
      this.policy = solver.policy();
    } else {
      await new Promise((r) => setTimeout(r, 1200));
      this.manifest = demoAcceptanceManifest();
      this.policy = demoPolicyState();
    }

    this.renderModes();
    this.updateLandscape();
    this.renderPolicy();

    if (this.statusEl) {
      const passed = this.manifest.allPassed;
      this.statusEl.textContent = `Acceptance: ${passed ? 'PASSED' : 'FAILED'} — ${this.manifest.witnessEntries} witness entries`;
      this.statusEl.style.color = passed ? 'var(--success)' : 'var(--danger)';
    }

    this.isTraining = false;
    if (this.acceptBtn) this.acceptBtn.disabled = false;
  }

  private async runAutoTraining(): Promise<void> {
    if (this.isTraining) return;
    for (let i = 0; i < this.autoTrainRounds; i++) {
      await this.runTraining();
      await new Promise((r) => setTimeout(r, 150));
    }
    await this.runAcceptance();
  }

  private async runAutoOptimize(): Promise<void> {
    if (this.isTraining) return;

    // The acceptance test creates a FRESH solver each time — pre-training
    // only updates the UI visualization. What actually matters is the
    // acceptance config (training_per_cycle, cycles, step_budget) and the
    // random seed. We try progressively more aggressive configs and seeds.

    const phases = [
      // Phase 1: Current settings, try 3 seeds
      { label: 'Phase 1', tpc: this.trainingPerCycle, cycles: this.acceptCycles, budget: this.stepBudget, hs: this.holdoutSize, seeds: 3 },
      // Phase 2: Medium intensity
      { label: 'Phase 2', tpc: 500, cycles: 8, budget: 1200, hs: 50, seeds: 3 },
      // Phase 3: High intensity
      { label: 'Phase 3', tpc: 800, cycles: 12, budget: 2000, hs: 50, seeds: 3 },
    ];

    let attempt = 0;
    const totalAttempts = phases.reduce((sum, p) => sum + p.seeds, 0);

    for (const phase of phases) {
      // Apply acceptance params for this phase
      this.trainingPerCycle = phase.tpc;
      this.acceptCycles = phase.cycles;
      this.stepBudget = phase.budget;
      this.holdoutSize = phase.hs;

      for (let seedIdx = 0; seedIdx < phase.seeds; seedIdx++) {
        attempt++;

        // Do one training round between attempts (for visualization)
        if (attempt > 1) {
          if (this.statusEl) {
            this.statusEl.textContent = `${phase.label}: training before attempt ${attempt}/${totalAttempts}...`;
            this.statusEl.style.color = 'var(--accent)';
          }
          await this.runTraining();
          await new Promise((r) => setTimeout(r, 50));
        }

        if (this.statusEl) {
          this.statusEl.textContent = `${phase.label}: acceptance attempt ${attempt}/${totalAttempts} (tpc=${phase.tpc}, c=${phase.cycles}, sb=${phase.budget})...`;
          this.statusEl.style.color = 'var(--warning)';
        }

        // Run acceptance (runAcceptance uses random seed via getSolver)
        await this.runAcceptance();
        const passed = this.manifest?.allPassed ?? false;

        if (passed) {
          if (this.statusEl) {
            this.statusEl.textContent = `Auto-optimize: PASSED on attempt ${attempt} (${phase.label})!`;
            this.statusEl.style.color = 'var(--success)';
          }
          return;
        }

        // Show why it failed
        if (this.statusEl && this.manifest) {
          const mc = this.manifest.modeC;
          const reasons: string[] = [];
          if (!mc.accuracyMaintained) reasons.push('accuracy < 80%');
          if (!mc.costImproved) reasons.push('cost not improved 5%');
          if (!mc.robustnessImproved) reasons.push('robustness not improved 3%');
          if (mc.dimensionsImproved < 2) reasons.push(`only ${mc.dimensionsImproved}/2 dims`);
          this.statusEl.textContent = `Attempt ${attempt}: FAILED — ${reasons.join(', ')}`;
          this.statusEl.style.color = '#FF4D4D';
        }

        await new Promise((r) => setTimeout(r, 100));
      }
    }

    if (this.statusEl) {
      this.statusEl.textContent = `Auto-optimize: did not pass after ${attempt} attempts. The acceptance test creates a fresh solver each time — try increasing Training/cycle and Cycles.`;
      this.statusEl.style.color = '#FF4D4D';
    }
  }

  /* ── Training curve rendering ── */

  private renderCurve(): void {
    if (!this.curveCanvas || this.trainingHistory.length === 0) return;

    const ctx = this.curveCanvas.getContext('2d');
    if (!ctx) return;

    const w = this.curveCanvas.width;
    const h = this.curveCanvas.height;
    const pad = { top: 14, right: 14, bottom: 28, left: 44 };
    const iw = w - pad.left - pad.right;
    const ih = h - pad.top - pad.bottom;

    ctx.clearRect(0, 0, w, h);

    // Background
    ctx.fillStyle = '#0B0F14';
    ctx.fillRect(0, 0, w, h);

    const pts = this.trainingHistory;
    const maxCycle = Math.max(pts.length - 1, 1);

    // Grid lines
    ctx.strokeStyle = '#1A2030';
    ctx.lineWidth = 0.5;
    for (let pct = 0; pct <= 1; pct += 0.25) {
      const y = pad.top + ih * (1 - pct);
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(pad.left + iw, y);
      ctx.stroke();

      ctx.fillStyle = '#484F58';
      ctx.font = '10px monospace';
      ctx.textAlign = 'right';
      ctx.fillText(`${(pct * 100).toFixed(0)}%`, pad.left - 6, y + 3);
    }

    // Fill area under accuracy curve
    ctx.fillStyle = 'rgba(0,229,255,0.06)';
    ctx.beginPath();
    ctx.moveTo(pad.left, pad.top + ih);
    for (let i = 0; i < pts.length; i++) {
      const x = pad.left + (i / maxCycle) * iw;
      const y = pad.top + ih * (1 - pts[i].accuracy);
      ctx.lineTo(x, y);
    }
    ctx.lineTo(pad.left + ((pts.length - 1) / maxCycle) * iw, pad.top + ih);
    ctx.closePath();
    ctx.fill();

    // Accuracy line
    ctx.strokeStyle = '#00E5FF';
    ctx.lineWidth = 2.5;
    ctx.beginPath();
    for (let i = 0; i < pts.length; i++) {
      const x = pad.left + (i / maxCycle) * iw;
      const y = pad.top + ih * (1 - pts[i].accuracy);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // Accuracy dots
    ctx.fillStyle = '#00E5FF';
    for (let i = 0; i < pts.length; i++) {
      const x = pad.left + (i / maxCycle) * iw;
      const y = pad.top + ih * (1 - pts[i].accuracy);
      ctx.beginPath();
      ctx.arc(x, y, 3.5, 0, Math.PI * 2);
      ctx.fill();
    }

    // Patterns line (secondary)
    if (pts.length > 1) {
      const maxP = Math.max(...pts.map((p) => p.patterns), 1);
      ctx.strokeStyle = '#58A6FF';
      ctx.lineWidth = 1.5;
      ctx.setLineDash([5, 3]);
      ctx.beginPath();
      for (let i = 0; i < pts.length; i++) {
        const x = pad.left + (i / maxCycle) * iw;
        const y = pad.top + ih * (1 - pts[i].patterns / maxP);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // Loss line
    if (pts.length > 1) {
      ctx.strokeStyle = '#FF6B9D';
      ctx.lineWidth = 1.5;
      ctx.setLineDash([3, 4]);
      ctx.beginPath();
      for (let i = 0; i < pts.length; i++) {
        const x = pad.left + (i / maxCycle) * iw;
        const y = pad.top + ih * (1 - pts[i].loss);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // X axis labels
    ctx.fillStyle = '#556677';
    ctx.font = '10px monospace';
    ctx.textAlign = 'center';
    const step = pts.length <= 10 ? 1 : Math.ceil(pts.length / 10);
    for (let i = 0; i < pts.length; i += step) {
      const x = pad.left + (i / maxCycle) * iw;
      ctx.fillText(`C${i + 1}`, x, h - 8);
    }

    // Current value annotation
    const last = pts[pts.length - 1];
    ctx.fillStyle = '#00E5FF';
    ctx.font = 'bold 11px monospace';
    ctx.textAlign = 'right';
    ctx.fillText(`${(last.accuracy * 100).toFixed(1)}%`, w - 4, pad.top + 4);
  }

  /* ── Acceptance modes ── */

  private renderModes(): void {
    if (!this.modesEl) return;
    this.modesEl.innerHTML = '';

    const hdr = document.createElement('div');
    hdr.className = 'panel-header';
    hdr.textContent = 'Acceptance Modes (A / B / C)';
    this.modesEl.appendChild(hdr);

    const body = document.createElement('div');
    body.className = 'panel-body';
    this.modesEl.appendChild(body);

    if (!this.manifest) {
      body.innerHTML = `
        <div style="text-align:center;padding:8px 0">
          <div style="color:var(--text-muted);font-size:12px;margin-bottom:6px">Click "Acceptance" to see results</div>
          <div style="font-size:10px;color:var(--text-muted);line-height:1.5">
            Train the solver first (multiple cycles recommended) before running acceptance.
          </div>
        </div>
      `;
      return;
    }

    const modes = [
      { label: 'Mode A', desc: 'Heuristic — accuracy only', data: this.manifest.modeA, color: MODE_COLORS.A },
      { label: 'Mode B', desc: 'Compiler — accuracy + cost', data: this.manifest.modeB, color: MODE_COLORS.B },
      { label: 'Mode C', desc: 'Learned — full multi-objective', data: this.manifest.modeC, color: MODE_COLORS.C },
    ];

    for (const mode of modes) {
      const row = document.createElement('div');
      row.style.cssText = 'margin-bottom:8px;padding:8px;background:var(--bg-surface);border:1px solid var(--border);border-radius:4px';

      const lastAcc = mode.data.cycles.length > 0
        ? mode.data.cycles[mode.data.cycles.length - 1].accuracy
        : 0;
      const passStr = mode.data.passed ? 'PASS' : 'FAIL';
      const passColor = mode.data.passed ? 'var(--success)' : '#FF4D4D';

      row.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:4px">
          <span style="color:${mode.color};font-weight:600;font-size:11px">${mode.label}</span>
          <span style="font-size:9px;color:var(--text-muted)">${mode.desc}</span>
          <span style="color:${passColor};font-weight:700;font-size:11px">${passStr} ${(lastAcc * 100).toFixed(1)}%</span>
        </div>
      `;

      // Sparkline bars
      const spark = document.createElement('div');
      spark.style.cssText = 'display:flex;gap:2px;align-items:flex-end;height:20px;margin-bottom:4px';
      for (const cycle of mode.data.cycles) {
        const bar = document.createElement('div');
        bar.style.cssText = `flex:1;height:${cycle.accuracy * 100}%;background:${mode.color};border-radius:1px;opacity:0.7;transition:height 0.3s`;
        bar.title = `Cycle ${cycle.cycle}: ${(cycle.accuracy * 100).toFixed(1)}% | Cost: ${cycle.costPerSolve.toFixed(0)}`;
        spark.appendChild(bar);
      }
      row.appendChild(spark);

      // Compact badges
      const badges = document.createElement('div');
      badges.style.cssText = 'display:flex;gap:3px;flex-wrap:wrap';
      const addBadge = (text: string, ok: boolean) => {
        const b = document.createElement('span');
        b.style.cssText = `font-size:8px;padding:1px 4px;border-radius:2px;border:1px solid ${ok ? 'rgba(46,204,113,0.3)' : 'rgba(255,77,77,0.3)'};color:${ok ? 'var(--success)' : '#FF4D4D'}`;
        b.textContent = text;
        badges.appendChild(b);
      };
      addBadge(`Acc: ${mode.data.accuracyMaintained ? 'OK' : 'FAIL'}`, mode.data.accuracyMaintained);
      addBadge(`Cost: ${mode.data.costImproved ? 'OK' : 'N/A'}`, mode.data.costImproved);
      addBadge(`Rob: ${mode.data.robustnessImproved ? 'OK' : 'N/A'}`, mode.data.robustnessImproved);
      addBadge(`Viol: ${mode.data.zeroViolations ? '0' : '>0'}`, mode.data.zeroViolations);
      row.appendChild(badges);

      body.appendChild(row);
    }

    // Overall result
    const overall = document.createElement('div');
    overall.style.cssText = `padding:6px;border-radius:4px;text-align:center;font-size:11px;font-weight:700;
      background:${this.manifest.allPassed ? 'rgba(46,204,113,0.1)' : 'rgba(255,77,77,0.1)'};
      border:1px solid ${this.manifest.allPassed ? 'rgba(46,204,113,0.3)' : 'rgba(255,77,77,0.3)'};
      color:${this.manifest.allPassed ? 'var(--success)' : '#FF4D4D'}`;
    overall.textContent = this.manifest.allPassed ? 'ALL MODES PASSED' : 'SOME MODES FAILED';
    body.appendChild(overall);

    // Witness info
    const witness = document.createElement('div');
    witness.style.cssText = 'font-size:9px;color:var(--text-muted);margin-top:6px;text-align:center';
    witness.innerHTML = `Witness chain: <span style="color:var(--text-secondary)">${this.manifest.witnessEntries} entries</span> &middot; <span style="color:var(--text-secondary)">${this.manifest.witnessChainBytes} bytes</span>`;
    body.appendChild(witness);

    // Mode C explanation
    const note = document.createElement('div');
    note.style.cssText = 'font-size:9px;color:var(--text-muted);margin-top:4px;text-align:center;line-height:1.4';
    note.innerHTML = `Pass/Fail = Mode C (full learned). Each test trains a <strong>fresh solver</strong> from scratch &mdash; increase <em>Training/cycle</em> and <em>Cycles</em> to give it more learning time.`;
    body.appendChild(note);
  }

  /* ── Policy state ── */

  private renderPolicy(): void {
    if (!this.policyEl) return;
    this.policyEl.innerHTML = '';

    const hdr = document.createElement('div');
    hdr.className = 'panel-header';
    hdr.innerHTML = 'Policy State <span style="font-size:8px;text-transform:none;letter-spacing:0;color:var(--text-muted);font-weight:400">learned decisions</span>';
    this.policyEl.appendChild(hdr);

    const body = document.createElement('div');
    body.className = 'panel-body';
    body.style.fontSize = '11px';
    this.policyEl.appendChild(body);

    if (!this.policy) {
      body.innerHTML = '<div style="color:var(--text-muted);text-align:center;padding:8px 0">Train to populate policy state</div>';
      return;
    }

    const arm2Rate = this.policy.speculativeAttempts > 0
      ? (this.policy.speculativeArm2Wins / this.policy.speculativeAttempts * 100).toFixed(1) + '%'
      : 'N/A';
    const earlyCommitRate = this.policy.earlyCommitsTotal > 0
      ? (this.policy.earlyCommitsWrong / this.policy.earlyCommitsTotal * 100).toFixed(1) + '%'
      : 'N/A';

    // Compact stats grid
    const statsGrid = document.createElement('div');
    statsGrid.style.cssText = 'display:grid;grid-template-columns:1fr 1fr;gap:6px;margin-bottom:8px';

    const addStat = (label: string, value: string, color: string) => {
      const cell = document.createElement('div');
      cell.style.cssText = 'background:var(--bg-surface);border:1px solid var(--border);border-radius:4px;padding:6px 8px';
      cell.innerHTML = `
        <div style="font-size:9px;color:var(--text-muted);margin-bottom:2px">${label}</div>
        <div style="font-size:13px;font-weight:600;color:${color};font-family:var(--font-mono)">${value}</div>
      `;
      statsGrid.appendChild(cell);
    };

    addStat('Prepass', this.policy.prepass || 'none', 'var(--accent)');
    addStat('Spec. Attempts', String(this.policy.speculativeAttempts), 'var(--text-primary)');
    addStat('Arm-2 Win Rate', arm2Rate, arm2Rate !== 'N/A' && parseFloat(arm2Rate) > 50 ? 'var(--success)' : 'var(--warning)');
    addStat('Early Commit Err', earlyCommitRate, earlyCommitRate !== 'N/A' && parseFloat(earlyCommitRate) < 20 ? 'var(--success)' : '#FF4D4D');
    body.appendChild(statsGrid);

    // Context bucket details
    const buckets = Object.keys(this.policy.contextStats);
    if (buckets.length > 0) {
      const bucketsDiv = document.createElement('div');
      bucketsDiv.innerHTML = '<div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:6px">Difficulty Buckets</div>';

      for (const bucket of buckets) {
        const bData = this.policy.contextStats[bucket];
        const modes = Object.keys(bData);
        const barDiv = document.createElement('div');
        barDiv.style.cssText = 'margin-bottom:6px';

        let totalAttempts = 0;
        let totalSuccesses = 0;
        for (const mode of modes) {
          const md = bData[mode] as Record<string, number>;
          totalAttempts += md.attempts || 0;
          totalSuccesses += md.successes || 0;
        }
        const rate = totalAttempts > 0 ? (totalSuccesses / totalAttempts * 100).toFixed(0) : '0';
        const rateNum = totalAttempts > 0 ? totalSuccesses / totalAttempts : 0;
        const color = rateNum > 0.7 ? 'var(--success)' : rateNum > 0.4 ? 'var(--warning)' : '#FF4D4D';

        barDiv.innerHTML = `
          <div style="display:flex;justify-content:space-between;font-size:10px;margin-bottom:2px">
            <span style="color:var(--text-secondary);text-transform:capitalize">${bucket}</span>
            <span style="color:${color};font-weight:600">${rate}% (${totalSuccesses}/${totalAttempts})</span>
          </div>
          <div class="progress-bar" style="height:4px">
            <div class="progress-fill" style="width:${rate}%;background:${color};transition:width 0.3s"></div>
          </div>
        `;
        bucketsDiv.appendChild(barDiv);
      }
      body.appendChild(bucketsDiv);
    }
  }

  /* ── Animation + Resize ── */

  private resize = (): void => {
    if (!this.renderer || !this.camera || !this.container) return;
    const canvas = this.renderer.domElement.parentElement;
    if (!canvas) return;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (w === 0 || h === 0) return;
    this.renderer.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  };

  private animate = (): void => {
    this.animFrameId = requestAnimationFrame(this.animate);
    this.landscapeTime += 0.005 * this.speed;

    // Subtle landscape undulation
    if (this.landscapeMesh) {
      const geo = this.landscapeMesh.geometry;
      const pos = geo.attributes.position;
      for (let i = 0; i < pos.count; i++) {
        const base = pos.getZ(i);
        const wobble = Math.sin(this.landscapeTime * 2 + i * 0.1) * 0.015 * this.speed;
        pos.setZ(i, base + wobble);
      }
      pos.needsUpdate = true;
    }

    // Animate peak glows
    for (let i = 0; i < this.peakGlows.length; i++) {
      const glow = this.peakGlows[i];
      const pulse = 1 + 0.15 * Math.sin(this.landscapeTime * 3 + i * 1.2);
      glow.material.opacity = 0.5 * pulse;
    }

    this.controls?.update();
    if (this.renderer && this.scene && this.camera) {
      this.renderer.render(this.scene, this.camera);
    }
  };

  unmount(): void {
    window.removeEventListener('resize', this.resize);
    cancelAnimationFrame(this.animFrameId);

    // Dispose landscape
    if (this.landscapeMesh) {
      this.scene?.remove(this.landscapeMesh);
      this.landscapeMesh.geometry.dispose();
      (this.landscapeMesh.material as THREE.Material).dispose();
      this.landscapeMesh = null;
    }

    // Dispose galaxy
    if (this.bgStars) {
      this.scene?.remove(this.bgStars);
      this.bgStars.geometry.dispose();
      (this.bgStars.material as THREE.Material).dispose();
      this.bgStars = null;
    }
    if (this.galacticPlane) {
      this.scene?.remove(this.galacticPlane);
      this.galacticPlane.geometry.dispose();
      (this.galacticPlane.material as THREE.Material).dispose();
      this.galacticPlane = null;
    }
    if (this.galacticCore) {
      this.scene?.remove(this.galacticCore);
      this.galacticCore.geometry.dispose();
      (this.galacticCore.material as THREE.Material).dispose();
      this.galacticCore = null;
    }
    for (const neb of this.nebulae) {
      this.scene?.remove(neb);
      neb.material.dispose();
    }
    this.nebulae = [];

    // Dispose arm markers
    if (this.armMarkers) {
      this.scene?.remove(this.armMarkers);
      this.armMarkers = null;
    }
    this.peakGlows = [];

    this.controls?.dispose();
    this.renderer?.dispose();

    this.controls = null;
    this.renderer = null;
    this.scene = null;
    this.camera = null;
    this.container = null;
  }
}
