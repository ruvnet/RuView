import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { CoherenceSurface } from '../three/CoherenceSurface';
import { fetchCoherence, fetchBoundaryAlerts, BoundaryAlert } from '../api';

/** Generate demo coherence values for a given epoch. */
function generateDemoValues(gridSize: number, epoch: number): number[] {
  const values: number[] = [];
  const seed = epoch * 0.1;
  for (let y = 0; y < gridSize; y++) {
    for (let x = 0; x < gridSize; x++) {
      const nx = x / gridSize;
      const ny = y / gridSize;
      const v = 0.5 + 0.3 * Math.sin(nx * 6 + seed) * Math.cos(ny * 6 + seed)
        + 0.2 * Math.sin((nx + ny) * 4 + seed * 0.5);
      values.push(Math.max(0, Math.min(1, v)));
    }
  }
  return values;
}

function computeStats(values: number[]): { mean: number; min: number; max: number; violations: number } {
  if (values.length === 0) return { mean: 0, min: 0, max: 0, violations: 0 };
  let sum = 0, min = 1, max = 0, violations = 0;
  for (const v of values) {
    sum += v;
    if (v < min) min = v;
    if (v > max) max = v;
    if (v < 0.8) violations++;
  }
  return { mean: sum / values.length, min, max, violations };
}

export class CoherenceHeatmap {
  private container: HTMLElement | null = null;
  private renderer: THREE.WebGLRenderer | null = null;
  private scene: THREE.Scene | null = null;
  private camera: THREE.PerspectiveCamera | null = null;
  private controls: OrbitControls | null = null;
  private surface: CoherenceSurface | null = null;
  private animFrameId = 0;
  private currentEpoch = 0;
  private gridSize = 64;
  private currentValues: number[] = [];
  private hud: HTMLElement | null = null;
  private metricsEls: Record<string, HTMLElement> = {};
  private alertList: HTMLElement | null = null;
  private raycaster = new THREE.Raycaster();
  private mouse = new THREE.Vector2();

  mount(container: HTMLElement): void {
    this.container = container;

    // Main layout: metrics top, 3D center, scrubber bottom
    const layout = document.createElement('div');
    layout.style.cssText = 'display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden';
    container.appendChild(layout);

    // ── View header with explanation ──
    const header = document.createElement('div');
    header.style.cssText = 'padding:12px 20px;border-bottom:1px solid var(--border);display:flex;align-items:center;gap:16px;flex-shrink:0';
    header.innerHTML = `
      <div style="flex:1">
        <div style="font-size:14px;font-weight:600;color:var(--text-primary);margin-bottom:2px">Coherence Field</div>
        <div style="font-size:11px;color:var(--text-secondary);line-height:1.4">
          Measures causal consistency across the event graph. High coherence (blue, flat) = events agree.
          Low coherence (red, raised peaks) = conflicting evidence or boundary pressure.
          Warning threshold at 0.80, critical at 0.70.
        </div>
      </div>
    `;
    layout.appendChild(header);

    // ── Metric cards row ──
    const metricsRow = document.createElement('div');
    metricsRow.style.cssText = 'display:flex;gap:12px;padding:12px 20px;flex-shrink:0;flex-wrap:wrap';
    const metricDefs = [
      { key: 'mean', label: 'MEAN COHERENCE', icon: '~' },
      { key: 'min', label: 'MINIMUM', icon: 'v' },
      { key: 'max', label: 'MAXIMUM', icon: '^' },
      { key: 'violations', label: 'BELOW THRESHOLD', icon: '!' },
    ];
    for (const m of metricDefs) {
      const card = document.createElement('div');
      card.className = 'metric-card';
      card.style.cssText = 'flex:1;min-width:140px';
      const valEl = document.createElement('div');
      valEl.className = 'metric-value';
      valEl.textContent = '--';
      const labelEl = document.createElement('div');
      labelEl.className = 'metric-label';
      labelEl.textContent = m.label;
      card.appendChild(labelEl);
      card.appendChild(valEl);
      metricsRow.appendChild(card);
      this.metricsEls[m.key] = valEl;
    }
    layout.appendChild(metricsRow);

    // ── Main area: 3D + alerts sidebar ──
    const mainArea = document.createElement('div');
    mainArea.style.cssText = 'flex:1;display:flex;overflow:hidden;min-height:0';
    layout.appendChild(mainArea);

    // Three.js canvas
    const canvasDiv = document.createElement('div');
    canvasDiv.className = 'three-container';
    canvasDiv.style.flex = '1';
    mainArea.appendChild(canvasDiv);

    // HUD overlay for hover info
    this.hud = document.createElement('div');
    this.hud.style.cssText = `
      position:absolute;top:12px;left:12px;
      padding:8px 12px;background:rgba(11,15,20,0.92);
      border:1px solid var(--border);border-radius:4px;
      font-family:var(--font-mono);font-size:11px;color:var(--text-secondary);
      pointer-events:none;display:none;z-index:10;line-height:1.6;
    `;
    canvasDiv.appendChild(this.hud);

    // Color legend overlay
    const legend = document.createElement('div');
    legend.style.cssText = `
      position:absolute;bottom:12px;right:12px;
      padding:8px 12px;background:rgba(11,15,20,0.9);
      border:1px solid var(--border);border-radius:4px;
      font-family:var(--font-mono);font-size:10px;color:var(--text-secondary);
      z-index:10;display:flex;flex-direction:column;gap:4px;
    `;
    legend.innerHTML = `
      <div style="font-size:9px;text-transform:uppercase;letter-spacing:0.5px;margin-bottom:2px">Coherence Scale</div>
      <div style="display:flex;align-items:center;gap:6px">
        <div style="width:60px;height:6px;border-radius:3px;background:linear-gradient(to right,#FF4D4D,#FFB020,#00E5FF,#0044AA)"></div>
      </div>
      <div style="display:flex;justify-content:space-between;width:60px">
        <span>0.6</span><span>0.8</span><span>1.0</span>
      </div>
      <div style="margin-top:4px;display:flex;flex-direction:column;gap:2px">
        <div style="display:flex;align-items:center;gap:4px"><span style="width:6px;height:6px;border-radius:50%;background:#FFB020;display:inline-block"></span> Warning &lt;0.80</div>
        <div style="display:flex;align-items:center;gap:4px"><span style="width:6px;height:6px;border-radius:50%;background:#FF4D4D;display:inline-block"></span> Critical &lt;0.70</div>
      </div>
    `;
    canvasDiv.appendChild(legend);

    // Interaction hint
    const hint = document.createElement('div');
    hint.style.cssText = `
      position:absolute;bottom:12px;left:12px;
      font-size:10px;color:var(--text-muted);font-family:var(--font-mono);
      z-index:10;pointer-events:none;
    `;
    hint.textContent = 'Drag to rotate, scroll to zoom, hover for values';
    canvasDiv.appendChild(hint);

    // ── Alerts sidebar ──
    const alertsSidebar = document.createElement('div');
    alertsSidebar.style.cssText = 'width:240px;background:var(--bg-panel);border-left:1px solid var(--border);display:flex;flex-direction:column;overflow:hidden;flex-shrink:0';
    const alertsHeader = document.createElement('div');
    alertsHeader.className = 'panel-header';
    alertsHeader.textContent = 'Active Alerts';
    alertsSidebar.appendChild(alertsHeader);
    this.alertList = document.createElement('div');
    this.alertList.style.cssText = 'flex:1;overflow-y:auto;padding:4px 0';
    alertsSidebar.appendChild(this.alertList);
    mainArea.appendChild(alertsSidebar);

    // ── Epoch scrubber ──
    const scrubberDiv = document.createElement('div');
    scrubberDiv.className = 'time-scrubber';
    scrubberDiv.style.flexShrink = '0';
    const scrubLabel = document.createElement('span');
    scrubLabel.className = 'time-scrubber-title';
    scrubLabel.textContent = 'Epoch';
    scrubberDiv.appendChild(scrubLabel);
    const slider = document.createElement('input');
    slider.type = 'range';
    slider.className = 'time-scrubber-range';
    slider.min = '0';
    slider.max = '100';
    slider.value = '0';
    scrubberDiv.appendChild(slider);
    const scrubVal = document.createElement('span');
    scrubVal.className = 'time-scrubber-label';
    scrubVal.textContent = 'E0';
    scrubberDiv.appendChild(scrubVal);
    slider.addEventListener('input', () => {
      const epoch = Number(slider.value);
      scrubVal.textContent = `E${epoch}`;
      this.currentEpoch = epoch;
      this.loadData(epoch);
    });
    layout.appendChild(scrubberDiv);

    // ── Three.js setup ──
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0B0F14);

    this.camera = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
    this.camera.position.set(4, 7, 10);
    this.camera.lookAt(0, 0, 0);

    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    canvasDiv.appendChild(this.renderer.domElement);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.1;
    this.controls.maxPolarAngle = Math.PI * 0.45;
    this.controls.minDistance = 4;
    this.controls.maxDistance = 30;

    // Lighting for phong material
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.4));
    const dirLight = new THREE.DirectionalLight(0xCCDDFF, 0.6);
    dirLight.position.set(5, 10, 5);
    this.scene.add(dirLight);
    const fillLight = new THREE.DirectionalLight(0x4488AA, 0.3);
    fillLight.position.set(-5, 3, -5);
    this.scene.add(fillLight);

    this.surface = new CoherenceSurface(this.scene, this.gridSize, this.gridSize);

    // Mouse hover for value readout
    canvasDiv.addEventListener('mousemove', this.onMouseMove);

    this.resize();
    window.addEventListener('resize', this.resize);
    this.loadData(0);
    this.loadAlerts();
    this.animate();
  }

  private onMouseMove = (event: MouseEvent): void => {
    if (!this.renderer || !this.camera || !this.hud) return;
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    this.mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;

    this.raycaster.setFromCamera(this.mouse, this.camera);
    const meshes = this.scene?.children.filter((c) => c instanceof THREE.Mesh) ?? [];
    const intersects = this.raycaster.intersectObjects(meshes);

    if (intersects.length > 0 && this.currentValues.length > 0) {
      const hit = intersects[0];
      const point = hit.point;
      // Map world coords back to grid cell
      const gx = Math.round(((point.x + 5) / 10) * (this.gridSize - 1));
      const gz = Math.round(((point.z + 5) / 10) * (this.gridSize - 1));
      if (gx >= 0 && gx < this.gridSize && gz >= 0 && gz < this.gridSize) {
        const idx = gz * this.gridSize + gx;
        const v = this.currentValues[idx];
        if (v !== undefined) {
          const status = v >= 0.85 ? 'STABLE' : v >= 0.80 ? 'NOMINAL' : v >= 0.70 ? 'WARNING' : 'CRITICAL';
          const color = v >= 0.85 ? 'var(--accent)' : v >= 0.80 ? 'var(--text-primary)' : v >= 0.70 ? 'var(--warning)' : 'var(--critical)';
          this.hud.style.display = 'block';
          this.hud.innerHTML = `
            <div style="color:var(--text-muted)">Sector (${gx}, ${gz})</div>
            <div style="font-size:16px;font-weight:600;color:${color}">${v.toFixed(3)}</div>
            <div style="color:${color};font-size:10px">${status}</div>
          `;
          return;
        }
      }
    }
    if (this.hud) this.hud.style.display = 'none';
  };

  private updateMetrics(values: number[]): void {
    const stats = computeStats(values);
    const setMetric = (key: string, val: string, cls?: string) => {
      const el = this.metricsEls[key];
      if (el) {
        el.textContent = val;
        el.className = 'metric-value' + (cls ? ` ${cls}` : '');
      }
    };
    setMetric('mean', stats.mean.toFixed(3), stats.mean >= 0.85 ? 'accent' : stats.mean >= 0.80 ? '' : 'warning');
    setMetric('min', stats.min.toFixed(3), stats.min >= 0.80 ? 'success' : stats.min >= 0.70 ? 'warning' : 'critical');
    setMetric('max', stats.max.toFixed(3), 'accent');
    setMetric('violations', `${stats.violations}`, stats.violations === 0 ? 'success' : stats.violations < 100 ? 'warning' : 'critical');
  }

  private async loadAlerts(): Promise<void> {
    if (!this.alertList) return;
    try {
      const alerts = await fetchBoundaryAlerts();
      this.renderAlerts(alerts);
    } catch {
      this.renderAlerts([
        { target_id: '7G', epoch: 7, pressure: 0.74, message: 'Coherence drop in sector 7G (0.74)' },
        { target_id: '3A', epoch: 5, pressure: 0.62, message: 'Witness chain gap in sector 3A' },
        { target_id: 'global', epoch: 7, pressure: 0.79, message: 'Boundary expansion +14.5%' },
      ]);
    }
  }

  private renderAlerts(alerts: BoundaryAlert[]): void {
    if (!this.alertList) return;
    this.alertList.innerHTML = '';
    if (alerts.length === 0) {
      this.alertList.innerHTML = '<div style="padding:16px;color:var(--text-muted);font-size:11px;text-align:center">No active alerts</div>';
      return;
    }
    for (const a of alerts) {
      const severity = a.pressure < 0.70 ? 'critical' : a.pressure < 0.80 ? 'warning' : 'success';
      const item = document.createElement('div');
      item.className = 'alert-item';
      item.innerHTML = `
        <div class="alert-dot ${severity}"></div>
        <div style="flex:1">
          <div style="font-size:11px;color:var(--text-primary);margin-bottom:2px">${a.message}</div>
          <div style="font-size:10px;color:var(--text-muted);font-family:var(--font-mono)">Sector ${a.target_id} | Coherence: ${a.pressure.toFixed(2)}</div>
        </div>
      `;
      this.alertList.appendChild(item);
    }
  }

  private async loadData(epoch: number): Promise<void> {
    if (!this.surface) return;
    try {
      const data = await fetchCoherence('default', epoch);
      this.currentValues = data.map((d) => d.value);
      this.surface.setValues(this.currentValues);
      this.updateMetrics(this.currentValues);
    } catch {
      this.currentValues = generateDemoValues(this.gridSize, epoch);
      this.surface.setValues(this.currentValues);
      this.updateMetrics(this.currentValues);
    }
  }

  private resize = (): void => {
    if (!this.renderer || !this.camera || !this.container) return;
    const canvas = this.renderer.domElement.parentElement;
    if (!canvas) return;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
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
    this.surface?.dispose();
    this.controls?.dispose();
    this.renderer?.dispose();
    this.surface = null;
    this.controls = null;
    this.renderer = null;
    this.scene = null;
    this.camera = null;
    this.container = null;
    this.hud = null;
    this.alertList = null;
    this.metricsEls = {};
  }
}
