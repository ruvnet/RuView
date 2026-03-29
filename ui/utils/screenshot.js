// Screenshot Tool - Capture current tab view as PNG
// Uses html2canvas-like approach with native Canvas API

import { toastManager } from './toast.js';

export class ScreenshotTool {
  constructor() {
    this.capturing = false;
  }

  init() {
    document.addEventListener('take-screenshot', () => this.capture());
  }

  async capture() {
    if (this.capturing) return;
    this.capturing = true;

    const activeTab = document.querySelector('.tab-content.active');
    if (!activeTab) {
      toastManager.warning('No active tab to capture');
      this.capturing = false;
      return;
    }

    try {
      // Flash effect
      this.flashEffect();

      // Try native ClipboardItem API first (modern browsers)
      if (typeof ClipboardItem !== 'undefined') {
        await this.captureToClipboard(activeTab);
        toastManager.success('Screenshot copied to clipboard', { duration: 3000 });
      } else {
        // Fallback: download as file
        await this.captureToFile(activeTab);
        toastManager.success('Screenshot saved as file', { duration: 3000 });
      }
    } catch (err) {
      console.error('Screenshot failed:', err);
      // Fallback: capture visible canvases + basic layout
      try {
        await this.captureCanvasFallback(activeTab);
        toastManager.success('Screenshot saved (canvas only)', { duration: 3000 });
      } catch {
        toastManager.error('Screenshot failed. Try using browser\'s built-in screenshot tool.');
      }
    }

    this.capturing = false;
  }

  async captureToClipboard(element) {
    const canvas = await this.renderToCanvas(element);
    const blob = await new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
    await navigator.clipboard.write([
      new ClipboardItem({ 'image/png': blob })
    ]);
  }

  async captureToFile(element) {
    const canvas = await this.renderToCanvas(element);
    const dataUrl = canvas.toDataURL('image/png');
    const link = document.createElement('a');
    link.href = dataUrl;
    link.download = `ruview-screenshot-${this.timestamp()}.png`;
    link.click();
  }

  async captureCanvasFallback(element) {
    // Find any canvas elements and merge them
    const canvases = element.querySelectorAll('canvas');
    if (canvases.length === 0) throw new Error('No canvas elements found');

    const firstCanvas = canvases[0];
    const mergedCanvas = document.createElement('canvas');
    mergedCanvas.width = firstCanvas.width || 800;
    mergedCanvas.height = firstCanvas.height || 600;
    const ctx = mergedCanvas.getContext('2d');

    // Dark background
    ctx.fillStyle = '#1f2121';
    ctx.fillRect(0, 0, mergedCanvas.width, mergedCanvas.height);

    canvases.forEach(c => {
      try { ctx.drawImage(c, 0, 0); } catch { /* tainted canvas */ }
    });

    // Add timestamp watermark
    ctx.fillStyle = 'rgba(255,255,255,0.3)';
    ctx.font = '12px monospace';
    ctx.fillText(`RuView - ${new Date().toLocaleString()}`, 10, mergedCanvas.height - 10);

    const dataUrl = mergedCanvas.toDataURL('image/png');
    const link = document.createElement('a');
    link.href = dataUrl;
    link.download = `ruview-screenshot-${this.timestamp()}.png`;
    link.click();
  }

  async renderToCanvas(element) {
    // Simple DOM-to-canvas renderer for basic content
    const rect = element.getBoundingClientRect();
    const canvas = document.createElement('canvas');
    const scale = window.devicePixelRatio || 1;
    canvas.width = rect.width * scale;
    canvas.height = rect.height * scale;
    const ctx = canvas.getContext('2d');
    ctx.scale(scale, scale);

    // Render background
    const styles = getComputedStyle(element);
    ctx.fillStyle = styles.backgroundColor || '#1f2121';
    ctx.fillRect(0, 0, rect.width, rect.height);

    // Render existing canvases
    const canvases = element.querySelectorAll('canvas');
    canvases.forEach(c => {
      const cRect = c.getBoundingClientRect();
      const x = cRect.left - rect.left;
      const y = cRect.top - rect.top;
      try { ctx.drawImage(c, x, y, cRect.width, cRect.height); } catch { /* tainted */ }
    });

    // Render text content
    ctx.fillStyle = styles.color || '#e0e0e0';
    ctx.font = `14px ${styles.fontFamily || 'sans-serif'}`;
    let textY = 30;
    element.querySelectorAll('h2, h3, .stat-value, .metric-label').forEach(el => {
      const text = el.textContent.trim();
      if (text && textY < rect.height - 20) {
        const elStyles = getComputedStyle(el);
        ctx.font = `${elStyles.fontWeight} ${elStyles.fontSize} ${styles.fontFamily || 'sans-serif'}`;
        ctx.fillStyle = elStyles.color;
        ctx.fillText(text, 20, textY);
        textY += parseInt(elStyles.fontSize) + 8;
      }
    });

    // Watermark
    ctx.fillStyle = 'rgba(255,255,255,0.15)';
    ctx.font = '11px monospace';
    ctx.fillText(`RuView - ${new Date().toLocaleString()}`, 10, rect.height - 10);

    return canvas;
  }

  flashEffect() {
    const flash = document.createElement('div');
    flash.className = 'screenshot-flash';
    document.body.appendChild(flash);
    flash.addEventListener('animationend', () => flash.remove());
  }

  timestamp() {
    return new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
  }

  dispose() {}
}
