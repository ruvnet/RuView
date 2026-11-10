/**
 * Main Application Orchestrator for the RuView Scientific RF Observatory.
 * Coordinates telemetry stream, waterfall spectrogram, spatial field, and vital sign monitors.
 * Complies with ELMAS.md: <= 350 lines, modular lifecycle.
 */

import { RfWaterfall } from './rf-waterfall.js';
import { RfSpatialField } from './rf-spatial.js';
import { RfVitalsMonitor } from './rf-vitals.js';
import { RfTelemetryStream } from './rf-telemetry.js';

export class RfObservatoryApp {
  constructor() {
    this.elements = this.queryDomElements();
    this.waterfall = null;
    this.spatialField = null;
    this.vitalsMonitor = null;
    this.telemetry = null;

    this.init();
  }

  queryDomElements() {
    return {
      waterfallCanvas: document.getElementById('waterfall-canvas'),
      spatialCanvas: document.getElementById('spatial-canvas'),
      vitalsCanvas: document.getElementById('vitals-canvas'),
      sourceBadge: document.getElementById('source-badge'),
      fpsValue: document.getElementById('fps-value'),
      jitterValue: document.getElementById('jitter-value'),
      presenceBadge: document.getElementById('presence-badge'),
      confidenceValue: document.getElementById('confidence-value'),
      motionValue: document.getElementById('motion-value'),
      nodesCount: document.getElementById('nodes-count'),
      bpmValue: document.getElementById('bpm-value'),
      fallStatus: document.getElementById('fall-status'),
      alertLog: document.getElementById('alert-log'),
      pauseBtn: document.getElementById('pause-btn')
    };
  }

  init() {
    if (!this.elements.waterfallCanvas || !this.elements.spatialCanvas) return;

    this.setupCanvases();
    this.waterfall = new RfWaterfall(this.elements.waterfallCanvas);
    this.spatialField = new RfSpatialField(this.elements.spatialCanvas);
    this.vitalsMonitor = new RfVitalsMonitor(this.elements.vitalsCanvas, {
      bpmElement: this.elements.bpmValue,
      fallStatusElement: this.elements.fallStatus,
      alertLogElement: this.elements.alertLog
    });

    this.telemetry = new RfTelemetryStream({
      onUpdate: (payload, meta) => this.handleSensingUpdate(payload, meta),
      onStatusChange: (status) => this.handleStatusChange(status)
    });

    this.bindEvents();
    this.telemetry.connect();
  }

  setupCanvases() {
    const resize = () => {
      for (const canvas of [this.elements.waterfallCanvas, this.elements.spatialCanvas, this.elements.vitalsCanvas]) {
        if (!canvas) continue;
        const rect = canvas.getBoundingClientRect();
        if (canvas.width !== Math.floor(rect.width) || canvas.height !== Math.floor(rect.height)) {
          canvas.width = Math.max(10, Math.floor(rect.width));
          canvas.height = Math.max(10, Math.floor(rect.height));
        }
      }
      this.waterfall?.render();
      this.spatialField?.render();
      this.vitalsMonitor?.render();
    };

    window.addEventListener('resize', resize);
    // Initial size pass
    setTimeout(resize, 50);
  }

  handleSensingUpdate(payload, meta) {
    this.updateProvenanceBadge(meta.provenance);
    this.updateLinkMetrics(meta.fps, meta.jitterMs);

    // 1. CSI Waterfall: feed first active node's amplitude or synthesize from variance
    const primaryNode = payload.nodes?.[0];
    const amplitudes = primaryNode?.amplitude?.length > 0
      ? primaryNode.amplitude
      : this.synthesizeSpectralAmplitudes(payload.features);

    this.waterfall.pushFrame(amplitudes, primaryNode?.node_id);

    // 2. Spatial Field & Node cartography
    this.spatialField.update(
      payload.signal_field?.values,
      payload.nodes,
      payload.persons
    );

    // 3. Vitals & Fall monitoring
    this.vitalsMonitor.update(
      payload.vital_signs,
      payload.features,
      payload.persons
    );

    // 4. Update HUD badges
    this.updateHudMetrics(payload);
  }

  updateProvenanceBadge(source) {
    if (!this.elements.sourceBadge) return;
    const isLive = source.startsWith('esp32') && !source.includes('offline');

    if (isLive) {
      this.elements.sourceBadge.textContent = 'PROVENANCE: LIVE ESP32';
      this.elements.sourceBadge.className = 'status-badge badge--live';
    } else {
      this.elements.sourceBadge.textContent = 'PROVENANCE: SYNTHETIC (ADR-295)';
      this.elements.sourceBadge.className = 'status-badge badge--synthetic';
    }
  }

  updateLinkMetrics(fps, jitterMs) {
    if (this.elements.fpsValue) this.elements.fpsValue.textContent = fps.toFixed(1);
    if (this.elements.jitterValue) this.elements.jitterValue.textContent = jitterMs.toFixed(1);
  }

  updateHudMetrics(payload) {
    const classification = payload.classification || {};
    if (this.elements.presenceBadge) {
      const isPresent = classification.presence;
      this.elements.presenceBadge.textContent = isPresent
        ? (classification.motion_level?.toUpperCase() || 'PRESENT')
        : 'EMPTY ROOM';
      this.elements.presenceBadge.className = isPresent
        ? 'status-badge badge--occupied'
        : 'status-badge badge--empty';
    }

    if (this.elements.confidenceValue) {
      const conf = Math.round((classification.confidence || 0) * 100);
      this.elements.confidenceValue.textContent = `${conf}%`;
    }

    if (this.elements.motionValue) {
      const motion = payload.features?.motion_band_power || 0;
      this.elements.motionValue.textContent = motion.toFixed(2);
    }

    if (this.elements.nodesCount) {
      this.elements.nodesCount.textContent = (payload.nodes?.length || 0).toString();
    }
  }

  handleStatusChange(status) {
    if (this.elements.sourceBadge && !status.connected) {
      this.elements.sourceBadge.textContent = 'RECONNECTING WS...';
      this.elements.sourceBadge.className = 'status-badge badge--offline';
    }
  }

  synthesizeSpectralAmplitudes(features) {
    const count = 64;
    const amp = new Float64Array(count);
    const variance = features?.variance || 10.0;
    const peakIdx = Math.floor(count / 2);

    for (let i = 0; i < count; i++) {
      const dist = Math.abs(i - peakIdx);
      const baseline = Math.max(10, 80 - dist * 2);
      amp[i] = baseline + (Math.random() - 0.5) * variance;
    }
    return Array.from(amp);
  }

  bindEvents() {
    this.elements.pauseBtn?.addEventListener('click', () => {
      const paused = this.telemetry.togglePause();
      this.elements.pauseBtn.textContent = paused ? 'RESUME STREAM' : 'PAUSE STREAM';
      this.elements.pauseBtn.setAttribute('aria-pressed', paused ? 'true' : 'false');
    });

    window.addEventListener('keydown', (e) => {
      if (e.code === 'Space' && e.target === document.body) {
        e.preventDefault();
        this.elements.pauseBtn?.click();
      }
    });
  }
}

// Bootstrap app on DOM load
window.addEventListener('DOMContentLoaded', () => {
  window.rfApp = new RfObservatoryApp();
});

