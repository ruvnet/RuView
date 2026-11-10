/**
 * Room RF Spatial Field & Node Localization Renderer.
 * Renders 20x20 floor-plane attenuation grid, transceiver links, and presence centroids.
 * Complies with ELMAS.md: <= 350 lines, guard clauses, pure 2D Canvas.
 */

import { TURBO_LUT, scalarToColor } from './rf-colormap.js';

export class RfSpatialField {
  /**
   * @param {HTMLCanvasElement} canvas 
   * @param {Object} options 
   */
  constructor(canvas, options = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.gridWidth = 20;
    this.gridHeight = 20;
    this.roomSizeMeters = options.roomSizeMeters || 6.0;

    // Heatmap buffer for interpolation
    this.gridCanvas = document.createElement('canvas');
    this.gridCanvas.width = this.gridWidth;
    this.gridCanvas.height = this.gridHeight;
    this.gridCtx = this.gridCanvas.getContext('2d');
    this.gridImageData = this.gridCtx.createImageData(this.gridWidth, this.gridHeight);
    this.gridBuffer32 = new Uint32Array(this.gridImageData.data.buffer);

    this.latestField = null;
    this.latestNodes = [];
    this.latestPersons = [];
  }

  /**
   * Update spatial state from SensingUpdate payload.
   * @param {number[]} fieldValues 20x20 values
   * @param {Array} nodes Array of NodeInfo
   * @param {Array} persons Array of PersonDetection
   */
  update(fieldValues, nodes = [], persons = []) {
    this.latestField = fieldValues;
    this.latestNodes = nodes || [];
    this.latestPersons = persons || [];
    this.render();
  }

  render() {
    const w = this.canvas.width;
    const h = this.canvas.height;
    if (w === 0 || h === 0) return;

    // Clear background to deep technical slate
    this.ctx.fillStyle = '#0a0e14';
    this.ctx.fillRect(0, 0, w, h);

    this.renderHeatmap(w, h);
    this.renderGridLines(w, h);
    this.renderFresnelLinks(w, h);
    this.renderTransceivers(w, h);
    this.renderPresenceCentroids(w, h);
  }

  renderHeatmap(w, h) {
    if (!this.latestField || this.latestField.length < this.gridWidth * this.gridHeight) {
      return;
    }

    for (let i = 0; i < this.gridWidth * this.gridHeight; i++) {
      const raw = this.latestField[i] || 0;
      this.gridBuffer32[i] = scalarToColor(Math.max(0, Math.min(1, raw)), TURBO_LUT);
    }

    this.gridCtx.putImageData(this.gridImageData, 0, 0);

    // Smoothly stretch onto main canvas
    this.ctx.save();
    this.ctx.globalAlpha = 0.55;
    this.ctx.imageSmoothingEnabled = true;
    this.ctx.drawImage(this.gridCanvas, 0, 0, w, h);
    this.ctx.restore();
  }

  renderGridLines(w, h) {
    this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
    this.ctx.lineWidth = 1;
    const step = w / 6;

    for (let x = step; x < w; x += step) {
      this.ctx.beginPath();
      this.ctx.moveTo(x, 0);
      this.ctx.lineTo(x, h);
      this.ctx.stroke();
    }
    for (let y = step; y < h; y += step) {
      this.ctx.beginPath();
      this.ctx.moveTo(0, y);
      this.ctx.lineTo(w, y);
      this.ctx.stroke();
    }

    // Outer boundary
    this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.12)';
    this.ctx.strokeRect(0, 0, w, h);
  }

  renderFresnelLinks(w, h) {
    if (this.latestNodes.length < 2) return;

    this.ctx.strokeStyle = 'rgba(16, 185, 129, 0.25)';
    this.ctx.lineWidth = 1.5;
    this.ctx.setLineDash([4, 4]);

    for (let i = 0; i < this.latestNodes.length; i++) {
      for (let j = i + 1; j < this.latestNodes.length; j++) {
        const p1 = this.worldToCanvas(this.latestNodes[i].position, w, h);
        const p2 = this.worldToCanvas(this.latestNodes[j].position, w, h);

        this.ctx.beginPath();
        this.ctx.moveTo(p1.x, p1.y);
        this.ctx.lineTo(p2.x, p2.y);
        this.ctx.stroke();
      }
    }
    this.ctx.setLineDash([]);
  }

  renderTransceivers(w, h) {
    this.ctx.font = '10px "JetBrains Mono", Consolas, monospace';

    for (const node of this.latestNodes) {
      const pos = this.worldToCanvas(node.position, w, h);

      // Outer ring
      this.ctx.fillStyle = 'rgba(16, 185, 129, 0.15)';
      this.ctx.beginPath();
      this.ctx.arc(pos.x, pos.y, 14, 0, Math.PI * 2);
      this.ctx.fill();

      // Core point
      this.ctx.fillStyle = '#10b981';
      this.ctx.beginPath();
      this.ctx.arc(pos.x, pos.y, 4, 0, Math.PI * 2);
      this.ctx.fill();

      // Node label with tabular numbers
      this.ctx.fillStyle = '#f3f4f6';
      const label = `Node ${node.node_id} (${Math.round(node.rssi_dbm)}dBm)`;
      this.ctx.fillText(label, pos.x + 8, pos.y - 6);
    }
  }

  renderPresenceCentroids(w, h) {
    for (const person of this.latestPersons) {
      const pos = this.worldToCanvas(person.position, w, h);
      const isLying = person.pose === 'lying';
      const radiusX = isLying ? 22 : 12;
      const radiusY = isLying ? 9 : 12;

      // Pulse aura
      this.ctx.fillStyle = 'rgba(245, 158, 11, 0.2)';
      this.ctx.beginPath();
      this.ctx.ellipse(pos.x, pos.y, radiusX + 6, radiusY + 6, 0, 0, Math.PI * 2);
      this.ctx.fill();

      // Solid target marker
      this.ctx.fillStyle = '#f59e0b';
      this.ctx.beginPath();
      this.ctx.ellipse(pos.x, pos.y, radiusX, radiusY, 0, 0, Math.PI * 2);
      this.ctx.fill();

      // Heading indicator vector
      if (person.motion_score > 0.1) {
        this.ctx.strokeStyle = '#fbbf24';
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        this.ctx.moveTo(pos.x, pos.y);
        this.ctx.lineTo(pos.x + 18, pos.y - 12);
        this.ctx.stroke();
      }

      // Metadata tag
      this.ctx.fillStyle = '#ffffff';
      this.ctx.font = 'bold 11px "JetBrains Mono", Consolas, monospace';
      const status = person.pose ? `${person.pose.toUpperCase()}` : 'OCCUPANT';
      this.ctx.fillText(status, pos.x - 18, pos.y + radiusY + 14);
    }
  }

  worldToCanvas(pos, w, h) {
    if (!pos || pos.length < 3) return { x: w / 2, y: h / 2 };
    // Map -roomSize/2 .. +roomSize/2 to canvas pixel coordinates
    const half = this.roomSizeMeters / 2;
    const nx = (pos[0] + half) / this.roomSizeMeters;
    const ny = (pos[2] + half) / this.roomSizeMeters;
    return {
      x: Math.max(16, Math.min(w - 16, nx * w)),
      y: Math.max(16, Math.min(h - 16, ny * h))
    };
  }
}

