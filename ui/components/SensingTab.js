/**
 * SensingTab — Live WiFi Sensing Visualization
 *
 * Connects to the sensing WebSocket service and renders:
 *   1. A 3D Gaussian-splat signal field (via gaussian-splats.js)
 *   2. An overlay HUD with real-time metrics (RSSI, variance, bands, classification)
 */

import { sensingService } from '../services/sensing.service.js';
import { GaussianSplatRenderer } from './gaussian-splats.js';

export class SensingTab {
  /** @param {HTMLElement} container - the #sensing section element */
  constructor(container) {
    this.container = container;
    this.splatRenderer = null;
    this._unsubData = null;
    this._unsubState = null;
    this._resizeObserver = null;
    this._threeLoaded = false;
    this._vitalTimer = null;
  }

  async init() {
    this._buildDOM();
    await this._loadThree();
    this._initSplatRenderer();
    this._connectService();
    this._setupResize();
    this._startVitalsPolling();
  }

  // ---- Vitals polling (numeric breathing / heart rate) --------------------
  //
  // The WebSocket broadcast omits `vital_signs` entirely whenever the server's
  // publication gate abstains, and it never carries the abstention reason.
  // `/api/v1/vital-signs` reports both, so poll it directly instead of trying
  // to infer "why is this blank" from a missing WS field.

  _startVitalsPolling() {
    this._stopVitalsPolling();
    this._updateVitals();                 // paint immediately, no first-tick gap
    this._vitalTimer = setInterval(() => {
      // Skip the request while the tab is hidden or removed from the DOM.
      if (document.hidden || !this.container.isConnected) return;
      this._updateVitals();
    }, 3000);
  }

  _stopVitalsPolling() {
    if (this._vitalTimer) {
      clearInterval(this._vitalTimer);
      this._vitalTimer = null;
    }
  }

  async _updateVitals() {
    let d;
    try {
      const res = await fetch('/api/v1/vital-signs', { cache: 'no-store' });
      if (!res.ok) return;
      d = await res.json();
    } catch {
      return; // transient network error — keep the previous reading on screen
    }

    const certVitals = d.vital_signs || {};
    const raw = d.raw_vital_signs || {};
    const g = d.gates || {};
    // Ordered trace of the certification path (which precondition closed).
    const certTrace = d.certification || {};
    const b = d.buffer_status || {};
    const set = (id, text) => {
      const el = this.container.querySelector('#' + id);
      if (el) el.textContent = text;
    };
    const num = (x) => (x == null ? '--' : Number(x).toFixed(2));
    const bpm = (x) => (x == null ? '-- bpm' : Number(x).toFixed(1) + ' bpm');
    const mark = (ok) => (ok ? '✓' : '✗');

    // The per-node vitals buffers are cleared whenever the measured CSI frame
    // rate shifts enough for `reconfigure_sample_rate` to fire. That blanks
    // the estimator for a few seconds, which used to make the card blink to
    // "-- bpm". Hold the last good raw reading across those gaps, but mark it
    // (dimmed + `⏱Ns`) so a held value is never mistaken for a live one.
    const HOLD_MS = 60000;
    this._vitalHold = this._vitalHold || {};
    const now = Date.now();
    const hold = (key, value) => {
      if (value !== null && value !== undefined) {
        this._vitalHold[key] = { value, at: now };
        return { value, held: false };
      }
      const kept = this._vitalHold[key];
      if (kept && now - kept.at < HOLD_MS) {
        return { value: kept.value, held: true, ageS: Math.round((now - kept.at) / 1000) };
      }
      return { value: null, held: false };
    };
    const heldBpm = (h) => (h.value == null
      ? '-- bpm'
      : Number(h.value).toFixed(1) + ' bpm' + (h.held ? ` ⏱${h.ageS}s` : ''));
    const dim = (id, held) => {
      const el = this.container.querySelector('#' + id);
      if (el) el.style.opacity = held ? '0.55' : '1';
    };

    // Certified block — only populated when every gate is open at once.
    set('valBreathBpm', bpm(certVitals.breathing_rate_bpm));
    set('valHeartBpm', bpm(certVitals.heart_rate_bpm));

    // Ungated block — the estimator's live output. Present as soon as the
    // buffers fill, so the card keeps showing numbers between certified
    // windows. Never claim this is certified: `gates` shows the distance.
    const hBr = hold('br', raw.breathing_rate_bpm);
    const hHr = hold('hr', raw.heart_rate_bpm);
    set('valRawBreathBpm', heldBpm(hBr));
    set('valRawHeartBpm', heldBpm(hHr));
    dim('valRawBreathBpm', hBr.held);
    dim('valRawHeartBpm', hHr.held);
    set('valBreathConf', num(raw.breathing_confidence));
    set('valHeartConf', num(raw.heartbeat_confidence));
    set('valVitalQuality', num(raw.signal_quality));
    // Buffer fill drives `signal_quality` (it is scaled by the fraction of the
    // breathing window that is populated), so show the real per-node numbers.
    set('valVitalBuffer',
        `${b.breathing_samples ?? '--'}/${b.breathing_capacity ?? '--'} · ` +
        `${b.heartbeat_samples ?? '--'}/${b.heartbeat_capacity ?? '--'}`);

    const certified = d.authority === 'explicit_calibration';

    const badge = this.container.querySelector('#vitalBadge');
    if (badge) {
      badge.textContent = certified ? '已认证' : '未认证';
      badge.style.color = certified ? '#4ade80' : '#8899aa';
    }

    const authEl = this.container.querySelector('#vitalAuthority');
    if (authEl) {
      authEl.textContent = certified
        ? 'authority: explicit_calibration'
        : 'abstained: ' + (d.abstention_reason || 'no reason given');
    }

    const gateEl = this.container.querySelector('#vitalGates');
    if (gateEl) {
      gateEl.textContent =
        `人数 ${g.person_count ?? '--'} ${mark(g.person_count_ok)} (需=1) · ` +
        `质量 ${num(g.signal_quality)} ${mark(g.signal_quality_ok)} ` +
        `(需≥${num(g.min_signal_quality)}) · ` +
        `呼吸置信 ${num(raw.breathing_confidence)} ${mark(g.breathing_confidence_ok)} · ` +
        `心跳置信 ${num(raw.heartbeat_confidence)} ${mark(g.heartbeat_confidence_ok)} ` +
        `(置信需≥${num(g.min_confidence)})`;
    }

    const certEl = this.container.querySelector('#vitalCert');
    if (certEl) {
      certEl.textContent = certified ? '' : (
        `认证缺: ${certTrace.reason || '--'}` +
        ` · 标定新鲜=${certTrace.explicit_calibration_fresh ? '是' : '否'}` +
        ` · 绑定=${certTrace.grid_binding_present ? '有' : '无'}` +
        ` · holdout=${certTrace.holdout_ready ? '就绪' : '未就绪'}` +
        (certTrace.runtime_window_size != null
          ? `(${certTrace.holdout_frames ?? '--'}/${certTrace.runtime_window_size})`
          : '') +
        ` · 模型人数=${certTrace.calibrated_person_count ?? '--'}`
      );
    }
  }

  // ---- DOM construction --------------------------------------------------

  _buildDOM() {
    this.container.innerHTML = `
      <h2>Live WiFi Sensing</h2>

      <!-- Data-source status banner — updated by _onStateChange -->
      <div id="sensingSourceBanner" class="sensing-source-banner sensing-source-reconnecting"
           role="status" aria-live="polite">
        RECONNECTING...
      </div>

      <div class="sensing-layout">
        <!-- 3D viewport -->
        <div class="sensing-viewport" id="sensingViewport">
          <div class="sensing-loading">Loading 3D engine...</div>
        </div>

        <!-- Side panel -->
        <div class="sensing-panel">
          <!-- Connection -->
          <div class="sensing-card">
            <div class="sensing-card-title">Connection</div>
            <div class="sensing-connection">
              <span class="sensing-dot" id="sensingDot"></span>
              <span id="sensingState">Connecting...</span>
              <span class="sensing-source" id="sensingSource"></span>
            </div>
          </div>

          <!-- RSSI -->
          <div class="sensing-card">
            <div class="sensing-card-title">RSSI</div>
            <div class="sensing-big-value" id="sensingRssi">-- dBm</div>
            <canvas id="sensingSparkline" width="200" height="40"></canvas>
          </div>

          <!-- Signal Features -->
          <div class="sensing-card">
            <div class="sensing-card-title">Signal Features</div>
            <div class="sensing-meters">
              <div class="sensing-meter">
                <label>Variance</label>
                <div class="sensing-bar"><div class="sensing-bar-fill" id="barVariance"></div></div>
                <span class="sensing-meter-val" id="valVariance">0</span>
              </div>
              <div class="sensing-meter">
                <label>Motion Band</label>
                <div class="sensing-bar"><div class="sensing-bar-fill motion" id="barMotion"></div></div>
                <span class="sensing-meter-val" id="valMotion">0</span>
              </div>
              <div class="sensing-meter">
                <label>Breathing Band</label>
                <div class="sensing-bar"><div class="sensing-bar-fill breath" id="barBreath"></div></div>
                <span class="sensing-meter-val" id="valBreath">0</span>
              </div>
              <div class="sensing-meter">
                <label>Spectral Power</label>
                <div class="sensing-bar"><div class="sensing-bar-fill spectral" id="barSpectral"></div></div>
                <span class="sensing-meter-val" id="valSpectral">0</span>
              </div>
            </div>
          </div>

          <!-- Classification -->
          <div class="sensing-card">
            <div class="sensing-card-title">Classification</div>
            <div class="sensing-classification" id="sensingClassification">
              <div class="sensing-class-label" id="classLabel">ABSENT</div>
              <div class="sensing-confidence">
                <label>Confidence</label>
                <div class="sensing-bar"><div class="sensing-bar-fill confidence" id="barConfidence"></div></div>
                <span class="sensing-meter-val" id="valConfidence">0%</span>
              </div>
            </div>
          </div>

          <!-- Vitals: certified (gated) + raw (ungated) breathing/heart rate -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Vitals <span id="vitalBadge" style="float:right;font-size:11px;
                    font-weight:600;color:#8899aa;">--</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row">
                <span title="需最新显式标定 + 恰好一人 + 质量/置信度全部门限">认证 Breathing</span>
                <span id="valBreathBpm">-- bpm</span>
              </div>
              <div class="sensing-detail-row">
                <span title="依赖相位心搏证据，单链路通常过不了门">认证 Heart Rate</span>
                <span id="valHeartBpm">-- bpm</span>
              </div>
            </div>
            <div style="margin-top:8px;font-size:11px;color:var(--text-secondary,#8899aa);">
              原始估算（未认证 · 持续更新）
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row">
                <span>Breathing</span><span id="valRawBreathBpm">-- bpm</span>
              </div>
              <div class="sensing-detail-row">
                <span>Heart Rate</span><span id="valRawHeartBpm">-- bpm</span>
              </div>
              <div class="sensing-detail-row">
                <span> Breath Conf</span><span id="valBreathConf">--</span>
              </div>
              <div class="sensing-detail-row">
                <span> Heart Conf</span><span id="valHeartConf">--</span>
              </div>
              <div class="sensing-detail-row">
                <span>Signal Quality</span><span id="valVitalQuality">--</span>
              </div>
              <div class="sensing-detail-row">
                <span>Buffer (breath/heart)</span><span id="valVitalBuffer">--</span>
              </div>
            </div>
            <div id="vitalAuthority" style="margin-top:8px;font-size:11px;line-height:1.5;
                 color:var(--text-secondary,#8899aa);word-break:break-word;">
              authority: --
            </div>
            <div id="vitalGates" style="margin-top:4px;font-size:11px;line-height:1.4;
                 color:var(--text-secondary,#8899aa);word-break:break-word;"></div>
            <div id="vitalCert" style="margin-top:4px;font-size:11px;line-height:1.4;
                 color:var(--text-secondary,#8899aa);word-break:break-word;"></div>
            <div style="margin-top:4px;font-size:10px;line-height:1.4;
                 color:var(--text-secondary,#8899aa);">
              ⏱n s = 缓冲区重置，显示 n 秒前的保持值
            </div>
          </div>

          <!-- Setup info -->
          <div class="sensing-card">
            <div class="sensing-card-title">About This Data</div>
            <p class="sensing-about-text">
              Metrics are computed from WiFi Channel State Information (CSI).
              With <strong><span id="sensingNodeCount">0</span> ESP32 node(s)</strong> you get presence detection, breathing
              estimation, and gross motion. Add <strong>3-4+ ESP32 nodes</strong>
              around the room for spatial resolution and limb-level tracking.
            </p>
          </div>

          <!-- Node Status -->
          <div class="sensing-card" id="sensingNodeCards">
            <div class="sensing-card-title">NODE STATUS</div>
            <div id="nodeStatusContainer"></div>
          </div>

          <!-- Extra info -->
          <div class="sensing-card">
            <div class="sensing-card-title">Details</div>
            <div class="sensing-details">
              <div class="sensing-detail-row">
                <span>Dominant Freq</span><span id="valDomFreq">0 Hz</span>
              </div>
              <div class="sensing-detail-row">
                <span>Change Points</span><span id="valChangePoints">0</span>
              </div>
              <div class="sensing-detail-row">
                <span>Sample Rate</span><span id="valSampleRate">--</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  // ---- Three.js loading --------------------------------------------------

  async _loadThree() {
    if (window.THREE) {
      this._threeLoaded = true;
      return;
    }

    return new Promise((resolve, reject) => {
      const script = document.createElement('script');
      script.src = 'https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js';
      script.onload = () => {
        this._threeLoaded = true;
        resolve();
      };
      script.onerror = () => reject(new Error('Failed to load Three.js'));
      document.head.appendChild(script);
    });
  }

  // ---- Splat renderer ----------------------------------------------------

  _initSplatRenderer() {
    const viewport = this.container.querySelector('#sensingViewport');
    if (!viewport) return;

    // Remove loading message
    viewport.innerHTML = '';

    try {
      this.splatRenderer = new GaussianSplatRenderer(viewport, {
        width: viewport.clientWidth,
        height: viewport.clientHeight || 500,
      });
    } catch (e) {
      console.error('[SensingTab] Failed to init splat renderer:', e);
      viewport.innerHTML = '<div class="sensing-loading">3D rendering unavailable</div>';
    }
  }

  // ---- Service connection ------------------------------------------------

  _connectService() {
    sensingService.start();

    this._unsubData = sensingService.onData((data) => this._onSensingData(data));
    this._unsubState = sensingService.onStateChange((state) => this._onStateChange(state));
  }

  _onSensingData(data) {
    // Update 3D view
    if (this.splatRenderer) {
      this.splatRenderer.update(data);
    }

    // Update HUD
    this._updateHUD(data);

    // Update per-node panels
    this._updateNodePanels(data);
  }

  _onStateChange(state) {
    const dot    = this.container.querySelector('#sensingDot');
    const text   = this.container.querySelector('#sensingState');
    const banner = this.container.querySelector('#sensingSourceBanner');

    if (dot && text) {
      const stateLabels = {
        disconnected: 'Disconnected',
        connecting:   'Connecting...',
        connected:    'Connected',
        reconnecting: 'Reconnecting...',
        simulated:    'Simulated',
        'auth-required': 'API token required',
      };
      dot.className = 'sensing-dot ' + state;
      text.textContent = stateLabels[state] || state;
    }

    if (banner) {
      // Map the service's dataSource to banner text and CSS modifier class.
      const dataSource = sensingService.dataSource;
      const bannerConfig = {
        'live':              { text: 'LIVE \u2014 ESP32 HARDWARE',           cls: 'sensing-source-live' },
        'server-simulated':  { text: 'SIMULATED \u2014 NO HARDWARE',        cls: 'sensing-source-server-sim' },
        'reconnecting':      { text: 'RECONNECTING...',                    cls: 'sensing-source-reconnecting' },
        'unreachable':       { text: 'NO DATA \u2014 SERVER UNREACHABLE',   cls: 'sensing-source-simulated' },
        'simulated':         { text: 'INVENTED DATA \u2014 NOT MEASURED',   cls: 'sensing-source-simulated' },
        'auth-required':     { text: 'API TOKEN REQUIRED \u2014 SETTINGS \u2192 API ACCESS', cls: 'sensing-source-simulated' },
      };
      const cfg = bannerConfig[dataSource] || bannerConfig.reconnecting;
      banner.textContent = cfg.text;
      banner.className = 'sensing-source-banner ' + cfg.cls;
    }
  }

  // ---- HUD update --------------------------------------------------------

  _updateHUD(data) {
    const f = data.features || {};
    const c = data.classification || {};

    // Node count
    const nodeCount = (data.nodes || []).length;
    const countEl = this.container.querySelector('#sensingNodeCount');
    if (countEl) countEl.textContent = String(nodeCount);

    // RSSI
    this._setText('sensingRssi', `${(f.mean_rssi || -80).toFixed(1)} dBm`);
    this._setText('sensingSource', data.source || '');

    // Bars (scale to 0-100%)
    this._setBar('barVariance', f.variance, 10, 'valVariance', f.variance);
    this._setBar('barMotion', f.motion_band_power, 0.5, 'valMotion', f.motion_band_power);
    this._setBar('barBreath', f.breathing_band_power, 0.3, 'valBreath', f.breathing_band_power);
    this._setBar('barSpectral', f.spectral_power, 2.0, 'valSpectral', f.spectral_power);

    // Classification
    const label = this.container.querySelector('#classLabel');
    if (label) {
      const level = (c.motion_level || 'absent').toUpperCase();
      label.textContent = level;
      label.className = 'sensing-class-label ' + (c.motion_level || 'absent');
    }

    const confPct = ((c.confidence || 0) * 100).toFixed(0);
    this._setBar('barConfidence', c.confidence, 1.0, 'valConfidence', confPct + '%');

    // Details
    this._setText('valDomFreq', (f.dominant_freq_hz || 0).toFixed(3) + ' Hz');
    this._setText('valChangePoints', String(f.change_points || 0));
    const srcLabel = (data.source === 'simulated' || data.source === 'simulate') ? 'sim' : data.source || 'live';
    this._setText('valSampleRate', srcLabel);

    // Sparkline
    this._drawSparkline();
  }

  _setText(id, text) {
    const el = this.container.querySelector('#' + id);
    if (el) el.textContent = text;
  }

  _setBar(barId, value, maxVal, valId, displayVal) {
    const bar = this.container.querySelector('#' + barId);
    if (bar) {
      const pct = Math.min(100, Math.max(0, ((value || 0) / maxVal) * 100));
      bar.style.width = pct + '%';
    }
    if (valId && displayVal != null) {
      const el = this.container.querySelector('#' + valId);
      if (el) el.textContent = typeof displayVal === 'number' ? displayVal.toFixed(3) : displayVal;
    }
  }

  _drawSparkline() {
    const canvas = this.container.querySelector('#sensingSparkline');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const history = sensingService.getRssiHistory();
    if (history.length < 2) return;

    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    const min = Math.min(...history) - 2;
    const max = Math.max(...history) + 2;
    const range = max - min || 1;

    ctx.beginPath();
    ctx.strokeStyle = '#32b8c6';
    ctx.lineWidth = 1.5;

    for (let i = 0; i < history.length; i++) {
      const x = (i / (history.length - 1)) * w;
      const y = h - ((history[i] - min) / range) * h;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
  }

  // ---- Per-node panels ---------------------------------------------------

  _updateNodePanels(data) {
    const container = this.container.querySelector('#nodeStatusContainer');
    if (!container) return;
    const nodeFeatures = data.node_features || [];
    if (nodeFeatures.length === 0) {
      container.textContent = '';
      const msg = document.createElement('div');
      msg.style.cssText = 'color:#888;font-size:12px;padding:8px;';
      msg.textContent = 'No nodes detected';
      container.appendChild(msg);
      return;
    }
    const NODE_COLORS = ['#00ccff', '#ff6600', '#00ff88', '#ff00cc', '#ffcc00', '#8800ff', '#00ffcc', '#ff0044'];
    container.textContent = '';
    for (const nf of nodeFeatures) {
      const color = NODE_COLORS[nf.node_id % NODE_COLORS.length];
      const statusColor = nf.stale ? '#888' : '#0f0';

      const row = document.createElement('div');
      row.style.cssText = `display:flex;align-items:center;gap:8px;padding:6px 8px;margin-bottom:4px;background:rgba(255,255,255,0.03);border-radius:6px;border-left:3px solid ${color};`;

      const idCol = document.createElement('div');
      idCol.style.minWidth = '50px';
      const nameEl = document.createElement('div');
      nameEl.style.cssText = `font-size:11px;font-weight:600;color:${color};`;
      nameEl.textContent = 'Node ' + nf.node_id;
      const statusEl = document.createElement('div');
      statusEl.style.cssText = `font-size:9px;color:${statusColor};`;
      statusEl.textContent = nf.stale ? 'STALE' : 'ACTIVE';
      idCol.appendChild(nameEl);
      idCol.appendChild(statusEl);

      const metricsCol = document.createElement('div');
      metricsCol.style.cssText = 'flex:1;font-size:10px;color:#aaa;';
      metricsCol.textContent = (nf.rssi_dbm || -80).toFixed(0) + ' dBm · var ' + (nf.features?.variance || 0).toFixed(1);

      const classCol = document.createElement('div');
      classCol.style.cssText = 'font-size:10px;font-weight:600;color:#ccc;';
      const motion = (nf.classification?.motion_level || 'absent').toUpperCase();
      const conf = ((nf.classification?.confidence || 0) * 100).toFixed(0);
      classCol.textContent = motion + ' ' + conf + '%';

      row.appendChild(idCol);
      row.appendChild(metricsCol);
      row.appendChild(classCol);
      container.appendChild(row);
    }
  }

  // ---- Resize ------------------------------------------------------------

  _setupResize() {
    const viewport = this.container.querySelector('#sensingViewport');
    if (!viewport || !window.ResizeObserver) return;

    this._resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (this.splatRenderer) {
          this.splatRenderer.resize(entry.contentRect.width, entry.contentRect.height);
        }
      }
    });
    this._resizeObserver.observe(viewport);
  }

  // ---- Cleanup -----------------------------------------------------------

  dispose() {
    this._stopVitalsPolling();
    if (this._unsubData) this._unsubData();
    if (this._unsubState) this._unsubState();
    if (this._resizeObserver) this._resizeObserver.disconnect();
    if (this.splatRenderer) this.splatRenderer.dispose();
    sensingService.stop();
  }
}
