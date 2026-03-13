/**
 * VideoCapture — getUserMedia webcam capture with frame extraction.
 * Provides quality metrics (brightness, motion) for fusion confidence gating.
 */

export class VideoCapture {
  constructor(videoElement) {
    this.video = videoElement;
    this.stream = null;
    this.offscreen = document.createElement('canvas');
    this.offCtx = this.offscreen.getContext('2d', { willReadFrequently: true });
    this.prevFrame = null;
    this.motionScore = 0;
    this.brightnessScore = 0;
  }

  async start(constraints = {}) {
    const defaultConstraints = {
      video: {
        width: { ideal: 640 },
        height: { ideal: 480 },
        facingMode: 'user',
        frameRate: { ideal: 30 }
      },
      audio: false
    };

    try {
      this.stream = await navigator.mediaDevices.getUserMedia(
        Object.keys(constraints).length ? constraints : defaultConstraints
      );
      this.video.srcObject = this.stream;
      await this.video.play();

      this.offscreen.width = this.video.videoWidth;
      this.offscreen.height = this.video.videoHeight;

      return true;
    } catch (err) {
      console.error('[Video] Camera access failed:', err.message);
      return false;
    }
  }

  stop() {
    if (this.stream) {
      this.stream.getTracks().forEach(t => t.stop());
      this.stream = null;
    }
    this.video.srcObject = null;
  }

  get isActive() {
    return this.stream !== null && this.video.readyState >= 2;
  }

  get width() { return this.video.videoWidth || 640; }
  get height() { return this.video.videoHeight || 480; }

  /**
   * Capture current frame as RGB Uint8Array + compute quality metrics.
   * @param {number} targetW - Target width for CNN input
   * @param {number} targetH - Target height for CNN input
   * @returns {{ rgb: Uint8Array, width: number, height: number, motion: number, brightness: number }}
   */
  captureFrame(targetW = 56, targetH = 56) {
    if (!this.isActive) return null;

    // Draw to offscreen at target resolution
    this.offscreen.width = targetW;
    this.offscreen.height = targetH;
    this.offCtx.drawImage(this.video, 0, 0, targetW, targetH);
    const imageData = this.offCtx.getImageData(0, 0, targetW, targetH);
    const rgba = imageData.data;

    // Convert RGBA → RGB
    const pixels = targetW * targetH;
    const rgb = new Uint8Array(pixels * 3);
    let brightnessSum = 0;
    let motionSum = 0;

    for (let i = 0; i < pixels; i++) {
      const r = rgba[i * 4];
      const g = rgba[i * 4 + 1];
      const b = rgba[i * 4 + 2];
      rgb[i * 3] = r;
      rgb[i * 3 + 1] = g;
      rgb[i * 3 + 2] = b;

      // Luminance for brightness
      const lum = 0.299 * r + 0.587 * g + 0.114 * b;
      brightnessSum += lum;

      // Motion: diff from previous frame
      if (this.prevFrame) {
        const pr = this.prevFrame[i * 3];
        const pg = this.prevFrame[i * 3 + 1];
        const pb = this.prevFrame[i * 3 + 2];
        motionSum += Math.abs(r - pr) + Math.abs(g - pg) + Math.abs(b - pb);
      }
    }

    this.brightnessScore = brightnessSum / (pixels * 255);
    this.motionScore = this.prevFrame ? Math.min(1, motionSum / (pixels * 100)) : 0;
    this.prevFrame = new Uint8Array(rgb);

    return {
      rgb,
      width: targetW,
      height: targetH,
      motion: this.motionScore,
      brightness: this.brightnessScore
    };
  }

  /**
   * Capture full-resolution RGBA for overlay rendering
   * @returns {ImageData|null}
   */
  captureFullFrame() {
    if (!this.isActive) return null;
    this.offscreen.width = this.width;
    this.offscreen.height = this.height;
    this.offCtx.drawImage(this.video, 0, 0);
    return this.offCtx.getImageData(0, 0, this.width, this.height);
  }

  /**
   * Detect motion region + detailed motion grid for body-part tracking.
   * Returns bounding box + a grid showing WHERE motion is concentrated.
   * @returns {{ x, y, w, h, detected: boolean, motionGrid: number[][], gridCols: number, gridRows: number, exitDirection: string|null }}
   */
  detectMotionRegion(targetW = 56, targetH = 56) {
    if (!this.isActive || !this.prevFrame) return { detected: false, motionGrid: null };

    this.offscreen.width = targetW;
    this.offscreen.height = targetH;
    this.offCtx.drawImage(this.video, 0, 0, targetW, targetH);
    const rgba = this.offCtx.getImageData(0, 0, targetW, targetH).data;

    let minX = targetW, minY = targetH, maxX = 0, maxY = 0;
    let motionPixels = 0;
    const threshold = 25;

    // Motion grid: divide frame into cells and track motion intensity per cell
    const gridCols = 10;
    const gridRows = 8;
    const cellW = targetW / gridCols;
    const cellH = targetH / gridRows;
    const motionGrid = Array.from({ length: gridRows }, () => new Float32Array(gridCols));
    const cellPixels = cellW * cellH;

    // Also track motion centroid weighted by intensity
    let motionCxSum = 0, motionCySum = 0, motionWeightSum = 0;

    for (let y = 0; y < targetH; y++) {
      for (let x = 0; x < targetW; x++) {
        const i = y * targetW + x;
        const r = rgba[i * 4], g = rgba[i * 4 + 1], b = rgba[i * 4 + 2];
        const pr = this.prevFrame[i * 3], pg = this.prevFrame[i * 3 + 1], pb = this.prevFrame[i * 3 + 2];
        const diff = Math.abs(r - pr) + Math.abs(g - pg) + Math.abs(b - pb);

        if (diff > threshold * 3) {
          motionPixels++;
          if (x < minX) minX = x;
          if (y < minY) minY = y;
          if (x > maxX) maxX = x;
          if (y > maxY) maxY = y;
        }

        // Accumulate per-cell motion intensity
        const gc = Math.min(Math.floor(x / cellW), gridCols - 1);
        const gr = Math.min(Math.floor(y / cellH), gridRows - 1);
        const intensity = diff / (3 * 255); // Normalize 0-1
        motionGrid[gr][gc] += intensity / cellPixels;

        // Weighted centroid
        if (diff > threshold) {
          motionCxSum += x * diff;
          motionCySum += y * diff;
          motionWeightSum += diff;
        }
      }
    }

    const detected = motionPixels > (targetW * targetH * 0.02);

    // Motion centroid (normalized 0-1)
    const motionCx = motionWeightSum > 0 ? motionCxSum / (motionWeightSum * targetW) : 0.5;
    const motionCy = motionWeightSum > 0 ? motionCySum / (motionWeightSum * targetH) : 0.5;

    // Detect exit direction: if centroid is near edges
    let exitDirection = null;
    if (detected && motionCx < 0.1) exitDirection = 'left';
    else if (detected && motionCx > 0.9) exitDirection = 'right';
    else if (detected && motionCy < 0.1) exitDirection = 'up';
    else if (detected && motionCy > 0.9) exitDirection = 'down';

    // Track last known position for through-wall persistence
    if (detected) {
      this._lastDetected = {
        x: minX / targetW,
        y: minY / targetH,
        w: (maxX - minX) / targetW,
        h: (maxY - minY) / targetH,
        cx: motionCx,
        cy: motionCy,
        exitDirection,
        time: performance.now()
      };
    }

    return {
      detected,
      x: minX / targetW,
      y: minY / targetH,
      w: (maxX - minX) / targetW,
      h: (maxY - minY) / targetH,
      coverage: motionPixels / (targetW * targetH),
      motionGrid,
      gridCols,
      gridRows,
      motionCx,
      motionCy,
      exitDirection
    };
  }

  /**
   * Get the last known detection info (for through-wall persistence)
   */
  get lastDetection() {
    return this._lastDetected || null;
  }
}
