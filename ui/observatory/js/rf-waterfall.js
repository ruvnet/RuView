/**
 * CSI Waterfall Spectrogram Renderer.
 * High-performance 2D Canvas matrix showing 64 subcarriers across time.
 * Complies with ELMAS.md: <= 350 lines, guard clauses, pure raster ops.
 */

import { VIRIDIS_LUT } from './rf-colormap.js';

export class RfWaterfall {
  /**
   * @param {HTMLCanvasElement} canvas
   * @param {Object} options
   */
  constructor(canvas, options = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d', { alpha: false });
    this.subcarrierCount = options.subcarrierCount || 64;
    this.historyDepth = options.historyDepth || 240;
    this.lut = options.lut || VIRIDIS_LUT;

    // Offscreen buffer for continuous waterfall scrolling
    this.offscreen = document.createElement('canvas');
    this.offscreen.width = this.subcarrierCount;
    this.offscreen.height = this.historyDepth;
    this.offCtx = this.offscreen.getContext('2d', { alpha: false });

    // Single-line image data for zero-allocation push
    this.lineImageData = this.offCtx.createImageData(this.subcarrierCount, 1);
    this.lineBuffer32 = new Uint32Array(this.lineImageData.data.buffer);

    // Dynamic amplitude normalization bounds (dBm or relative magnitude)
    this.minAmp = options.minAmp || 0.0;
    this.maxAmp = options.maxAmp || 100.0;
    this.selectedNodeId = null;

    this.initCanvas();
  }

  initCanvas() {
    this.offCtx.fillStyle = '#0a0e14';
    this.offCtx.fillRect(0, 0, this.offscreen.width, this.offscreen.height);
    this.render();
  }

  /**
   * Push a new CSI subcarrier amplitude vector (typically 52 or 64 elements).
   * @param {number[]} amplitudes
   * @param {number|null} nodeId
   */
  pushFrame(amplitudes, nodeId = null) {
    if (!amplitudes || amplitudes.length === 0) return;
    if (this.selectedNodeId !== null && nodeId !== this.selectedNodeId) return;

    // Shift offscreen canvas down by 1 pixel
    this.offCtx.drawImage(
      this.offscreen,
      0, 0, this.subcarrierCount, this.historyDepth - 1,
      0, 1, this.subcarrierCount, this.historyDepth - 1
    );

    // Map amplitudes to row 0 pixels using LUT
    const span = Math.max(1e-4, this.maxAmp - this.minAmp);
    const count = Math.min(this.subcarrierCount, amplitudes.length);

    for (let i = 0; i < this.subcarrierCount; i++) {
      const val = i < count ? amplitudes[i] : 0;
      const normalized = Math.max(0, Math.min(1, (val - this.minAmp) / span));
      const lutIdx = Math.round(normalized * 255);
      this.lineBuffer32[i] = this.lut[lutIdx];
    }

    this.offCtx.putImageData(this.lineImageData, 0, 0);
    this.render();
  }

  /**
   * Render offscreen waterfall onto the display canvas with axis markings.
   */
  render() {
    const w = this.canvas.width;
    const h = this.canvas.height;
    if (w === 0 || h === 0) return;

    this.ctx.imageSmoothingEnabled = false;
    this.ctx.drawImage(this.offscreen, 0, 0, w, h);

    // Overlay technical subcarrier grid markings
    this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.08)';
    this.ctx.lineWidth = 1;
    this.ctx.font = '10px "JetBrains Mono", Consolas, monospace';
    this.ctx.fillStyle = 'rgba(156, 163, 175, 0.7)';

    const step = w / 4;
    for (let i = 1; i < 4; i++) {
      const x = Math.round(i * step);
      this.ctx.beginPath();
      this.ctx.moveTo(x, 0);
      this.ctx.lineTo(x, h);
      this.ctx.stroke();

      const subcarrierIdx = Math.round((i / 4) * this.subcarrierCount - this.subcarrierCount / 2);
      this.ctx.fillText(`${subcarrierIdx > 0 ? '+' : ''}${subcarrierIdx}`, x + 4, h - 8);
    }
  }

  /**
   * Set dynamic normalization range.
   * @param {number} min 
   * @param {number} max 
   */
  setRange(min, max) {
    if (max <= min) return;
    this.minAmp = min;
    this.maxAmp = max;
  }

  /**
   * Filter waterfall to display a specific ESP32 node.
   * @param {number|null} nodeId 
   */
  setNodeFilter(nodeId) {
    this.selectedNodeId = nodeId;
  }
}

