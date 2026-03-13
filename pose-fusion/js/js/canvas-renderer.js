/**
 * CanvasRenderer — Renders skeleton overlay on video, CSI heatmap,
 * embedding space visualization, and fusion confidence bars.
 */

import { SKELETON_CONNECTIONS } from './pose-decoder.js';

export class CanvasRenderer {
  constructor() {
    this.colors = {
      joint:      '#00d878',
      jointGlow:  'rgba(0, 216, 120, 0.4)',
      limb:       '#3eff8a',
      limbGlow:   'rgba(62, 255, 138, 0.15)',
      csiJoint:   '#ffb020',
      csiLimb:    '#ffc850',
      fused:      '#00e5ff',
      confidence: 'rgba(255,255,255,0.3)',
      videoEmb:   '#00e5ff',
      csiEmb:     '#ffb020',
      fusedEmb:   '#00d878',
    };
  }

  /**
   * Draw skeleton overlay on the video canvas
   * @param {CanvasRenderingContext2D} ctx
   * @param {Array<{x,y,confidence}>} keypoints - Normalized [0,1] coordinates
   * @param {number} width - Canvas width
   * @param {number} height - Canvas height
   * @param {object} opts
   */
  drawSkeleton(ctx, keypoints, width, height, opts = {}) {
    const minConf = opts.minConfidence || 0.3;
    const color = opts.color || 'green';
    const jointColor = color === 'amber' ? this.colors.csiJoint : this.colors.joint;
    const limbColor = color === 'amber' ? this.colors.csiLimb : this.colors.limb;
    const glowColor = color === 'amber' ? 'rgba(255,176,32,0.4)' : this.colors.jointGlow;

    ctx.clearRect(0, 0, width, height);

    if (!keypoints || keypoints.length === 0) return;

    // Draw limbs first (behind joints)
    ctx.lineWidth = 3;
    ctx.lineCap = 'round';

    for (const [i, j] of SKELETON_CONNECTIONS) {
      const kpA = keypoints[i];
      const kpB = keypoints[j];
      if (!kpA || !kpB || kpA.confidence < minConf || kpB.confidence < minConf) continue;

      const ax = kpA.x * width, ay = kpA.y * height;
      const bx = kpB.x * width, by = kpB.y * height;
      const avgConf = (kpA.confidence + kpB.confidence) / 2;

      // Glow
      ctx.strokeStyle = this.colors.limbGlow;
      ctx.lineWidth = 8;
      ctx.globalAlpha = avgConf * 0.4;
      ctx.beginPath();
      ctx.moveTo(ax, ay);
      ctx.lineTo(bx, by);
      ctx.stroke();

      // Main line
      ctx.strokeStyle = limbColor;
      ctx.lineWidth = 2.5;
      ctx.globalAlpha = avgConf;
      ctx.beginPath();
      ctx.moveTo(ax, ay);
      ctx.lineTo(bx, by);
      ctx.stroke();
    }

    // Draw joints
    ctx.globalAlpha = 1;
    for (const kp of keypoints) {
      if (!kp || kp.confidence < minConf) continue;

      const x = kp.x * width;
      const y = kp.y * height;
      const r = 3 + kp.confidence * 3;

      // Glow
      ctx.beginPath();
      ctx.arc(x, y, r + 4, 0, Math.PI * 2);
      ctx.fillStyle = glowColor;
      ctx.globalAlpha = kp.confidence * 0.6;
      ctx.fill();

      // Joint dot
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fillStyle = jointColor;
      ctx.globalAlpha = kp.confidence;
      ctx.fill();

      // White center
      ctx.beginPath();
      ctx.arc(x, y, r * 0.4, 0, Math.PI * 2);
      ctx.fillStyle = '#fff';
      ctx.globalAlpha = kp.confidence * 0.8;
      ctx.fill();
    }

    ctx.globalAlpha = 1;

    // Confidence label
    if (opts.label) {
      ctx.font = '11px "JetBrains Mono", monospace';
      ctx.fillStyle = jointColor;
      ctx.globalAlpha = 0.8;
      ctx.fillText(opts.label, 8, height - 8);
      ctx.globalAlpha = 1;
    }
  }

  /**
   * Draw CSI amplitude heatmap
   * @param {CanvasRenderingContext2D} ctx
   * @param {{ data: Float32Array, width: number, height: number }} heatmap
   * @param {number} canvasW
   * @param {number} canvasH
   */
  drawCsiHeatmap(ctx, heatmap, canvasW, canvasH) {
    ctx.clearRect(0, 0, canvasW, canvasH);

    if (!heatmap || !heatmap.data || heatmap.height < 2) {
      ctx.fillStyle = '#0a0e18';
      ctx.fillRect(0, 0, canvasW, canvasH);
      ctx.font = '11px "JetBrains Mono", monospace';
      ctx.fillStyle = 'rgba(255,255,255,0.3)';
      ctx.fillText('Waiting for CSI data...', 8, canvasH / 2);
      return;
    }

    const { data, width: dw, height: dh } = heatmap;
    const cellW = canvasW / dw;
    const cellH = canvasH / dh;

    for (let y = 0; y < dh; y++) {
      for (let x = 0; x < dw; x++) {
        const val = Math.min(1, Math.max(0, data[y * dw + x]));
        ctx.fillStyle = this._heatmapColor(val);
        ctx.fillRect(x * cellW, y * cellH, cellW + 0.5, cellH + 0.5);
      }
    }

    // Axis labels
    ctx.font = '9px "JetBrains Mono", monospace';
    ctx.fillStyle = 'rgba(255,255,255,0.4)';
    ctx.fillText('Subcarrier →', 4, canvasH - 4);
    ctx.save();
    ctx.translate(canvasW - 4, canvasH - 4);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText('Time ↑', 0, 0);
    ctx.restore();
  }

  /**
   * Draw embedding space 2D projection
   * @param {CanvasRenderingContext2D} ctx
   * @param {{ video: Array, csi: Array, fused: Array }} points
   * @param {number} w
   * @param {number} h
   */
  drawEmbeddingSpace(ctx, points, w, h) {
    ctx.fillStyle = '#050810';
    ctx.fillRect(0, 0, w, h);

    // Grid
    ctx.strokeStyle = 'rgba(255,255,255,0.05)';
    ctx.lineWidth = 0.5;
    for (let i = 0; i <= 4; i++) {
      const x = (i / 4) * w;
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, h); ctx.stroke();
      const y = (i / 4) * h;
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
    }

    // Axes
    ctx.strokeStyle = 'rgba(255,255,255,0.1)';
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(w / 2, 0); ctx.lineTo(w / 2, h); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(0, h / 2); ctx.lineTo(w, h / 2); ctx.stroke();

    const drawPoints = (pts, color, size) => {
      if (!pts || pts.length === 0) return;
      const len = pts.length;
      for (let i = 0; i < len; i++) {
        const p = pts[i];
        if (!p) continue;
        const age = 1 - (i / len) * 0.7; // Fade older points
        const px = w / 2 + p[0] * w * 0.35;
        const py = h / 2 + p[1] * h * 0.35;

        if (px < 0 || px > w || py < 0 || py > h) continue;

        ctx.beginPath();
        ctx.arc(px, py, size, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.globalAlpha = age * 0.7;
        ctx.fill();
      }
    };

    drawPoints(points.video, this.colors.videoEmb, 3);
    drawPoints(points.csi, this.colors.csiEmb, 3);
    drawPoints(points.fused, this.colors.fusedEmb, 4);
    ctx.globalAlpha = 1;

    // Legend
    ctx.font = '9px "JetBrains Mono", monospace';
    const legends = [
      { color: this.colors.videoEmb, label: 'Video' },
      { color: this.colors.csiEmb, label: 'CSI' },
      { color: this.colors.fusedEmb, label: 'Fused' },
    ];
    legends.forEach((l, i) => {
      const ly = 12 + i * 14;
      ctx.fillStyle = l.color;
      ctx.beginPath();
      ctx.arc(10, ly - 3, 3, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = 'rgba(255,255,255,0.5)';
      ctx.fillText(l.label, 18, ly);
    });
  }

  _heatmapColor(val) {
    // Dark blue → cyan → green → yellow → red
    if (val < 0.25) {
      const t = val / 0.25;
      return `rgb(${Math.floor(t * 20)}, ${Math.floor(20 + t * 60)}, ${Math.floor(60 + t * 100)})`;
    } else if (val < 0.5) {
      const t = (val - 0.25) / 0.25;
      return `rgb(${Math.floor(20 + t * 20)}, ${Math.floor(80 + t * 100)}, ${Math.floor(160 - t * 60)})`;
    } else if (val < 0.75) {
      const t = (val - 0.5) / 0.25;
      return `rgb(${Math.floor(40 + t * 180)}, ${Math.floor(180 + t * 75)}, ${Math.floor(100 - t * 80)})`;
    } else {
      const t = (val - 0.75) / 0.25;
      return `rgb(${Math.floor(220 + t * 35)}, ${Math.floor(255 - t * 120)}, ${Math.floor(20 - t * 20)})`;
    }
  }
}
