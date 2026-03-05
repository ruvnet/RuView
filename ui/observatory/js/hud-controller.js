/**
 * HudController — Extracted HUD update, settings dialog, and scenario UI
 *
 * Manages all DOM-based HUD elements:
 * - Vital sign display with smooth lerp transitions and color coding
 * - Signal metrics, sparkline, and presence indicator
 * - Scenario description and edge module badges
 * - Mini person-count dot visualization
 * - Settings dialog (tabs, ranges, presets, data source)
 * - Quick-select scenario dropdown
 */

// ---- Constants ----

export const SCENARIO_NAMES = [
  'EMPTY ROOM','VITAL SIGNS','MULTI-PERSON','FALL DETECT',
  'SLEEP MONITOR','INTRUSION','GESTURE CTRL','CROWD OCCUPANCY',
  'SEARCH RESCUE','ELDERLY CARE','FITNESS','SECURITY PATROL',
];

export const DEFAULTS = {
  bloom: 0.08, bloomRadius: 0.2, bloomThresh: 0.6,
  exposure: 1.3, vignette: 0.25, grain: 0.01, chromatic: 0.0005,
  boneThick: 0.018, jointSize: 0.035, glow: 0.3, trail: 0.35,
  wireColor: '#00d878', jointColor: '#ff4060', aura: 0.02,
  field: 0.45, waves: 0.4, ambient: 0.7, reflect: 0.2,
  fov: 50, orbitSpeed: 0.15, grid: true, room: true,
  scenario: 'auto', cycle: 30, dataSource: 'demo', wsUrl: '',
};

export const SETTINGS_VERSION = '6';

export const PRESETS = {
  foundation: {},
  cinematic: {
    bloom: 1.2, bloomRadius: 0.5, bloomThresh: 0.2,
    exposure: 0.8, vignette: 0.7, grain: 0.04, chromatic: 0.002,
    glow: 0.6, trail: 0.8, aura: 0.06, field: 0.4,
    waves: 0.7, ambient: 0.25, reflect: 0.5, fov: 40, orbitSpeed: 0.08,
  },
  minimal: {
    bloom: 0.3, bloomRadius: 0.2, bloomThresh: 0.5,
    exposure: 1.1, vignette: 0.2, grain: 0, chromatic: 0,
    glow: 0.3, trail: 0.2, aura: 0.02, field: 0.7,
    waves: 0.3, ambient: 0.6, reflect: 0.1, wireColor: '#40ff90', jointColor: '#4080ff',
  },
  neon: {
    bloom: 2.5, bloomRadius: 0.8, bloomThresh: 0.1,
    exposure: 0.6, vignette: 0.6, grain: 0.02, chromatic: 0.004,
    glow: 2.0, trail: 1.0, aura: 0.15, field: 0.6,
    waves: 1.0, ambient: 0.15, reflect: 0.7, wireColor: '#00ffaa', jointColor: '#ff00ff',
  },
  tactical: {
    bloom: 0.5, bloomRadius: 0.3, bloomThresh: 0.4,
    exposure: 0.85, vignette: 0.4, grain: 0.04, chromatic: 0.001,
    glow: 0.5, trail: 0.4, aura: 0.03, field: 0.8,
    waves: 0.4, ambient: 0.3, reflect: 0.15, wireColor: '#30ff60', jointColor: '#ff8800',
  },
  medical: {
    bloom: 0.6, bloomRadius: 0.4, bloomThresh: 0.35,
    exposure: 1.0, vignette: 0.3, grain: 0.01, chromatic: 0.0005,
    glow: 0.6, trail: 0.3, aura: 0.04, field: 0.5,
    waves: 0.3, ambient: 0.5, reflect: 0.2, wireColor: '#00ccff', jointColor: '#ff3355',
  },
};

// Scenario descriptions shown below the dropdown
const SCENARIO_DESCRIPTIONS = {
  auto:              'Auto-cycling through all sensing scenarios.',
  empty_room:        'Baseline calibration with no human presence in the monitored zone.',
  single_breathing:  'Detecting vital signs through WiFi signal micro-variations.',
  two_walking:       'Tracking multiple people simultaneously via CSI multiplex separation.',
  fall_event:        'Sudden posture-change detection using acceleration feature analysis.',
  sleep_monitoring:  'Monitoring breathing patterns and apnea events during sleep.',
  intrusion_detect:  'Passive perimeter monitoring -- no cameras, pure RF sensing.',
  gesture_control:   'DTW-based gesture recognition from hand/arm motion signatures.',
  crowd_occupancy:   'Estimating room occupancy count from aggregate CSI variance.',
  search_rescue:     'Through-wall survivor detection using WiFi-MAT multistatic mode.',
  elderly_care:      'Continuous gait analysis for early mobility-decline detection.',
  fitness_tracking:  'Rep counting and exercise classification from body kinematics.',
  security_patrol:   'Multi-zone presence patrol with camera-free motion heatmaps.',
};

// Edge modules active per scenario
const SCENARIO_EDGE_MODULES = {
  auto:              [],
  empty_room:        [],
  single_breathing:  ['VITALS'],
  two_walking:       ['GAIT', 'TRACKING'],
  fall_event:        ['FALL', 'VITALS'],
  sleep_monitoring:  ['VITALS', 'APNEA'],
  intrusion_detect:  ['PRESENCE', 'ALERT'],
  gesture_control:   ['GESTURE', 'DTW'],
  crowd_occupancy:   ['OCCUPANCY'],
  search_rescue:     ['MAT', 'VITALS', 'PRESENCE'],
  elderly_care:      ['GAIT', 'VITALS', 'FALL'],
  fitness_tracking:  ['GESTURE', 'GAIT'],
  security_patrol:   ['PRESENCE', 'ALERT', 'TRACKING'],
};

// Edge-module badge colors
const MODULE_COLORS = {
  VITALS:    'var(--red-heart)',
  GAIT:      'var(--green-glow)',
  FALL:      'var(--red-alert)',
  GESTURE:   'var(--amber)',
  PRESENCE:  'var(--blue-signal)',
  TRACKING:  'var(--green-bright)',
  OCCUPANCY: 'var(--amber)',
  ALERT:     'var(--red-alert)',
  DTW:       'var(--amber)',
  APNEA:     'var(--red-heart)',
  MAT:       'var(--blue-signal)',
};

// Vital-sign color-coding thresholds
function vitalColor(type, value) {
  if (value <= 0) return 'var(--text-secondary)';
  if (type === 'hr') {
    if (value < 50 || value > 130) return 'var(--red-alert)';
    if (value < 60 || value > 100) return 'var(--amber)';
    return 'var(--green-glow)';
  }
  if (type === 'br') {
    if (value < 8 || value > 28) return 'var(--red-alert)';
    if (value < 12 || value > 20) return 'var(--amber)';
    return 'var(--green-glow)';
  }
  if (type === 'conf') {
    if (value < 40) return 'var(--red-alert)';
    if (value < 70) return 'var(--amber)';
    return 'var(--green-glow)';
  }
  return 'var(--text-primary)';
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

// ---- HudController class ----

export class HudController {
  constructor(observatory) {
    this._obs = observatory;
    this._settingsOpen = false;
    this._rssiHistory = [];
    this._sparklineCtx = document.getElementById('rssi-sparkline')?.getContext('2d');

    // Lerp state for smooth vital-sign transitions
    this._lerpHr = 0;
    this._lerpBr = 0;
    this._lerpConf = 0;

    // Track current scenario for description/edge updates
    this._currentScenarioKey = null;
  }

  // ============================================================
  // Settings dialog
  // ============================================================

  initSettings() {
    const overlay = document.getElementById('settings-overlay');
    const btn = document.getElementById('settings-btn');
    const closeBtn = document.getElementById('settings-close');
    btn.addEventListener('click', () => this.toggleSettings());
    closeBtn.addEventListener('click', () => this.toggleSettings());
    overlay.addEventListener('click', (e) => { if (e.target === overlay) this.toggleSettings(); });

    // Tab switching
    document.querySelectorAll('.stab').forEach(tab => {
      tab.addEventListener('click', () => {
        document.querySelectorAll('.stab').forEach(t => t.classList.remove('active'));
        document.querySelectorAll('.stab-content').forEach(c => c.classList.remove('active'));
        tab.classList.add('active');
        document.getElementById(`stab-${tab.dataset.stab}`).classList.add('active');
      });
    });

    const obs = this._obs;
    const s = obs.settings;

    // Bind ranges
    this._bindRange('opt-bloom', 'bloom', v => { obs._postProcessing._bloomPass.strength = v; });
    this._bindRange('opt-bloom-radius', 'bloomRadius', v => { obs._postProcessing._bloomPass.radius = v; });
    this._bindRange('opt-bloom-thresh', 'bloomThresh', v => { obs._postProcessing._bloomPass.threshold = v; });
    this._bindRange('opt-exposure', 'exposure', v => { obs._renderer.toneMappingExposure = v; });
    this._bindRange('opt-vignette', 'vignette', v => { obs._postProcessing._vignettePass.uniforms.uVignetteStrength.value = v; });
    this._bindRange('opt-grain', 'grain', v => { obs._postProcessing._vignettePass.uniforms.uGrainStrength.value = v; });
    this._bindRange('opt-chromatic', 'chromatic', v => { obs._postProcessing._vignettePass.uniforms.uChromaticStrength.value = v; });
    this._bindRange('opt-bone-thick', 'boneThick');
    this._bindRange('opt-joint-size', 'jointSize');
    this._bindRange('opt-glow', 'glow');
    this._bindRange('opt-trail', 'trail');
    this._bindRange('opt-aura', 'aura');
    this._bindRange('opt-field', 'field', v => { obs._fieldMat.opacity = v; });
    this._bindRange('opt-waves', 'waves');
    this._bindRange('opt-ambient', 'ambient', v => { obs._ambient.intensity = v * 5.0; });
    this._bindRange('opt-reflect', 'reflect', v => {
      obs._floorMat.roughness = 1.0 - v * 0.7;
      obs._floorMat.metalness = v * 0.5;
    });
    this._bindRange('opt-fov', 'fov', v => {
      obs._camera.fov = v;
      obs._camera.updateProjectionMatrix();
    });
    this._bindRange('opt-orbit-speed', 'orbitSpeed');
    this._bindRange('opt-cycle', 'cycle', v => { obs._demoData.setCycleDuration(v); });

    // Color pickers
    document.getElementById('opt-wire-color').value = s.wireColor;
    document.getElementById('opt-wire-color').addEventListener('input', (e) => {
      s.wireColor = e.target.value; obs._applyColors(); this.saveSettings();
    });
    document.getElementById('opt-joint-color').value = s.jointColor;
    document.getElementById('opt-joint-color').addEventListener('input', (e) => {
      s.jointColor = e.target.value; obs._applyColors(); this.saveSettings();
    });

    // Checkboxes
    document.getElementById('opt-grid').checked = s.grid;
    document.getElementById('opt-grid').addEventListener('change', (e) => {
      s.grid = e.target.checked; obs._grid.visible = e.target.checked; this.saveSettings();
    });
    document.getElementById('opt-room').checked = s.room;
    document.getElementById('opt-room').addEventListener('change', (e) => {
      s.room = e.target.checked; obs._roomWire.visible = e.target.checked; this.saveSettings();
    });

    // Scenario select
    const scenarioSel = document.getElementById('opt-scenario');
    scenarioSel.value = s.scenario;
    scenarioSel.addEventListener('change', (e) => {
      s.scenario = e.target.value;
      obs._demoData.setScenario(e.target.value);
      this.saveSettings();
    });

    // Data source
    const dsSel = document.getElementById('opt-data-source');
    dsSel.value = s.dataSource;
    dsSel.addEventListener('change', (e) => {
      s.dataSource = e.target.value;
      document.getElementById('ws-url-row').style.display = e.target.value === 'ws' ? 'flex' : 'none';
      if (e.target.value === 'ws' && s.wsUrl) obs._connectWS(s.wsUrl);
      else obs._disconnectWS();
      this.updateSourceBadge(s.dataSource, obs._ws);
      this.saveSettings();
    });
    document.getElementById('ws-url-row').style.display = s.dataSource === 'ws' ? 'flex' : 'none';

    const wsInput = document.getElementById('opt-ws-url');
    wsInput.value = s.wsUrl;
    wsInput.addEventListener('change', (e) => {
      s.wsUrl = e.target.value;
      if (s.dataSource === 'ws') obs._connectWS(e.target.value);
      this.saveSettings();
    });

    // Buttons
    document.getElementById('btn-reset-camera').addEventListener('click', () => {
      obs._camera.position.set(6, 5, 8);
      obs._controls.target.set(0, 1.2, 0);
      obs._controls.update();
    });
    document.getElementById('btn-export-settings').addEventListener('click', () => {
      const blob = new Blob([JSON.stringify(s, null, 2)], { type: 'application/json' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = 'ruview-observatory-settings.json';
      a.click();
    });
    document.getElementById('btn-reset-settings').addEventListener('click', () => {
      this.applyPreset(DEFAULTS);
    });

    const presetSel = document.getElementById('opt-preset');
    presetSel.addEventListener('change', (e) => {
      const p = PRESETS[e.target.value];
      if (p) this.applyPreset({ ...DEFAULTS, ...p });
    });

    obs._grid.visible = s.grid;
    obs._roomWire.visible = s.room;
  }

  // ============================================================
  // Quick-select (top bar scenario dropdown)
  // ============================================================

  initQuickSelect() {
    const sel = document.getElementById('scenario-quick-select');
    if (!sel) return;
    sel.addEventListener('change', (e) => {
      this._obs._demoData.setScenario(e.target.value);
      const settingsSel = document.getElementById('opt-scenario');
      if (settingsSel) settingsSel.value = e.target.value;
      this._obs.settings.scenario = e.target.value;
      this.saveSettings();
    });
  }

  // ============================================================
  // Toggle / save / preset
  // ============================================================

  toggleSettings() {
    this._settingsOpen = !this._settingsOpen;
    document.getElementById('settings-overlay').style.display = this._settingsOpen ? 'flex' : 'none';
  }

  get settingsOpen() {
    return this._settingsOpen;
  }

  saveSettings() {
    try {
      localStorage.setItem('ruview-observatory-settings', JSON.stringify(this._obs.settings));
    } catch {}
  }

  applyPreset(preset) {
    const obs = this._obs;
    Object.assign(obs.settings, preset);
    this.saveSettings();
    const rangeMap = {
      'opt-bloom': 'bloom', 'opt-bloom-radius': 'bloomRadius', 'opt-bloom-thresh': 'bloomThresh',
      'opt-exposure': 'exposure', 'opt-vignette': 'vignette', 'opt-grain': 'grain', 'opt-chromatic': 'chromatic',
      'opt-bone-thick': 'boneThick', 'opt-joint-size': 'jointSize', 'opt-glow': 'glow', 'opt-trail': 'trail', 'opt-aura': 'aura',
      'opt-field': 'field', 'opt-waves': 'waves', 'opt-ambient': 'ambient', 'opt-reflect': 'reflect',
      'opt-fov': 'fov', 'opt-orbit-speed': 'orbitSpeed', 'opt-cycle': 'cycle',
    };
    for (const [id, key] of Object.entries(rangeMap)) {
      const el = document.getElementById(id);
      const valEl = document.getElementById(`${id}-val`);
      if (el) el.value = obs.settings[key];
      if (valEl) valEl.textContent = obs.settings[key];
    }
    const gridEl = document.getElementById('opt-grid');
    if (gridEl) { gridEl.checked = obs.settings.grid; obs._grid.visible = obs.settings.grid; }
    const roomEl = document.getElementById('opt-room');
    if (roomEl) { roomEl.checked = obs.settings.room; obs._roomWire.visible = obs.settings.room; }
    document.getElementById('opt-wire-color').value = obs.settings.wireColor;
    document.getElementById('opt-joint-color').value = obs.settings.jointColor;
    obs._applyPostSettings();
    obs._renderer.toneMappingExposure = obs.settings.exposure;
    obs._fieldMat.opacity = obs.settings.field;
    obs._ambient.intensity = obs.settings.ambient * 5.0;
    obs._floorMat.roughness = 1.0 - obs.settings.reflect * 0.7;
    obs._floorMat.metalness = obs.settings.reflect * 0.5;
    obs._camera.fov = obs.settings.fov;
    obs._camera.updateProjectionMatrix();
    obs._demoData.setCycleDuration(obs.settings.cycle);
    obs._applyColors();
  }

  // ============================================================
  // Source badge
  // ============================================================

  updateSourceBadge(dataSource, ws) {
    const dot = document.querySelector('#data-source-badge .dot');
    const label = document.getElementById('data-source-label');
    if (dataSource === 'ws' && ws?.readyState === WebSocket.OPEN) {
      dot.className = 'dot dot--live'; label.textContent = 'LIVE';
    } else {
      dot.className = 'dot dot--demo'; label.textContent = 'DEMO';
    }
  }

  // ============================================================
  // HUD update (called every frame)
  // ============================================================

  updateHUD(data, demoData) {
    if (!data) return;
    const vs = data.vital_signs || {};
    const feat = data.features || {};
    const cls = data.classification || {};

    // Sync scenario dropdown
    const quickSel = document.getElementById('scenario-quick-select');
    const cur = demoData._autoMode ? 'auto' : demoData.currentScenario;
    if (quickSel && quickSel.value !== cur) quickSel.value = cur;
    const autoIcon = document.getElementById('autoplay-icon');
    if (autoIcon) autoIcon.className = demoData._autoMode ? '' : 'hidden';

    const targetHr = vs.heart_rate_bpm || 0;
    const targetBr = vs.breathing_rate_bpm || 0;
    const targetConf = Math.round((cls.confidence || 0) * 100);

    // Smooth lerp transitions (blend 4% per frame toward target — very stable)
    const lerpFactor = 0.04;
    this._lerpHr = targetHr > 0 ? lerp(this._lerpHr, targetHr, lerpFactor) : 0;
    this._lerpBr = targetBr > 0 ? lerp(this._lerpBr, targetBr, lerpFactor) : 0;
    this._lerpConf = targetConf > 0 ? lerp(this._lerpConf, targetConf, lerpFactor) : 0;

    const dispHr = this._lerpHr > 1 ? Math.round(this._lerpHr) : '--';
    const dispBr = this._lerpBr > 1 ? Math.round(this._lerpBr) : '--';
    const dispConf = this._lerpConf > 1 ? Math.round(this._lerpConf) : '--';

    this._setText('hr-value', dispHr);
    this._setText('br-value', dispBr);
    this._setText('conf-value', dispConf);
    this._setWidth('hr-bar', Math.min(100, this._lerpHr / 120 * 100));
    this._setWidth('br-bar', Math.min(100, this._lerpBr / 30 * 100));
    this._setWidth('conf-bar', this._lerpConf);

    // Color-code vital values
    this._setColor('hr-value', vitalColor('hr', this._lerpHr));
    this._setColor('br-value', vitalColor('br', this._lerpBr));
    this._setColor('conf-value', vitalColor('conf', this._lerpConf));

    // Color-code bar fills to match
    this._setBarColor('hr-bar', vitalColor('hr', this._lerpHr));
    this._setBarColor('br-bar', vitalColor('br', this._lerpBr));
    this._setBarColor('conf-bar', vitalColor('conf', this._lerpConf));

    this._setText('rssi-value', `${Math.round(feat.mean_rssi || 0)} dBm`);
    this._setText('var-value', (feat.variance || 0).toFixed(2));
    this._setText('motion-value', (feat.motion_band_power || 0).toFixed(3));

    // Mini person-count dots
    const personCount = data.estimated_persons || 0;
    this._updatePersonDots(personCount);

    const presEl = document.getElementById('presence-indicator');
    const presLabel = document.getElementById('presence-label');
    if (presEl) {
      const ml = cls.motion_level || 'absent';
      presEl.className = 'presence-state';
      if (ml === 'active') { presEl.classList.add('presence--active'); presLabel.textContent = 'ACTIVE'; }
      else if (cls.presence) { presEl.classList.add('presence--present'); presLabel.textContent = 'PRESENT'; }
      else { presEl.classList.add('presence--absent'); presLabel.textContent = 'ABSENT'; }
    }

    const fallEl = document.getElementById('fall-alert');
    if (fallEl) fallEl.style.display = cls.fall_detected ? 'block' : 'none';

    // Scenario description and edge modules
    const scenarioKey = demoData._autoMode ? (demoData.currentScenario || 'auto') : (demoData.currentScenario || 'auto');
    if (scenarioKey !== this._currentScenarioKey) {
      this._currentScenarioKey = scenarioKey;
      this._updateScenarioDescription(scenarioKey);
      this._updateEdgeModules(scenarioKey);
    }
  }

  // ============================================================
  // Sparkline
  // ============================================================

  updateSparkline(data) {
    const rssi = data?.features?.mean_rssi;
    if (rssi == null || !this._sparklineCtx) return;
    this._rssiHistory.push(rssi);
    if (this._rssiHistory.length > 60) this._rssiHistory.shift();

    const ctx = this._sparklineCtx;
    const w = ctx.canvas.width, h = ctx.canvas.height;
    ctx.clearRect(0, 0, w, h);
    if (this._rssiHistory.length < 2) return;

    ctx.beginPath();
    ctx.strokeStyle = '#2090ff';
    ctx.lineWidth = 1.5;
    ctx.shadowColor = '#2090ff';
    ctx.shadowBlur = 4;
    for (let i = 0; i < this._rssiHistory.length; i++) {
      const x = (i / (this._rssiHistory.length - 1)) * w;
      const norm = Math.max(0, Math.min(1, (this._rssiHistory[i] + 80) / 60));
      const y = h - norm * h;
      i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.shadowBlur = 0;
    ctx.lineTo(w, h);
    ctx.lineTo(0, h);
    ctx.closePath();
    const grad = ctx.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0, 'rgba(32,144,255,0.15)');
    grad.addColorStop(1, 'rgba(32,144,255,0)');
    ctx.fillStyle = grad;
    ctx.fill();
  }

  // ============================================================
  // Private helpers
  // ============================================================

  _setText(id, val) {
    const e = document.getElementById(id);
    if (e) e.textContent = val;
  }

  _setWidth(id, pct) {
    const e = document.getElementById(id);
    if (e) e.style.width = `${pct}%`;
  }

  _setColor(id, color) {
    const e = document.getElementById(id);
    if (e) e.style.color = color;
  }

  _setBarColor(id, color) {
    const e = document.getElementById(id);
    if (e) e.style.background = color;
  }

  _bindRange(id, key, applyFn) {
    const el = document.getElementById(id);
    const valEl = document.getElementById(`${id}-val`);
    if (!el) return;
    el.value = this._obs.settings[key];
    if (valEl) valEl.textContent = this._obs.settings[key];
    el.addEventListener('input', (e) => {
      const v = parseFloat(e.target.value);
      this._obs.settings[key] = v;
      if (valEl) valEl.textContent = v;
      if (applyFn) applyFn(v);
      this.saveSettings();
    });
  }

  _updatePersonDots(count) {
    const container = document.getElementById('persons-dots');
    if (!container) {
      // Fall back to text-only display
      this._setText('persons-value', count);
      return;
    }
    // Build dot icons: filled for detected persons, dim for empty slots (max 8)
    const maxDots = 8;
    const clamped = Math.min(count, maxDots);
    let html = '';
    for (let i = 0; i < maxDots; i++) {
      const active = i < clamped;
      html += `<span class="person-dot${active ? ' person-dot--active' : ''}"></span>`;
    }
    container.innerHTML = html;
    this._setText('persons-value', count);
  }

  _updateScenarioDescription(scenarioKey) {
    const el = document.getElementById('scenario-description');
    if (!el) return;
    el.textContent = SCENARIO_DESCRIPTIONS[scenarioKey] || '';
  }

  _updateEdgeModules(scenarioKey) {
    const bar = document.getElementById('edge-modules-bar');
    if (!bar) return;
    const modules = SCENARIO_EDGE_MODULES[scenarioKey] || [];
    if (modules.length === 0) {
      bar.innerHTML = '';
      bar.style.display = 'none';
      return;
    }
    bar.style.display = 'flex';
    bar.innerHTML = modules.map(m => {
      const color = MODULE_COLORS[m] || 'var(--text-secondary)';
      return `<span class="edge-badge" style="--badge-color:${color}">${m}</span>`;
    }).join('');
  }
}
