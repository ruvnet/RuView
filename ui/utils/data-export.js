// Data Export Utility - Export sensor/pose data as JSON or CSV

import { sensingService } from '../services/sensing.service.js';
import { toastManager } from './toast.js';

export class DataExport {
  constructor() {
    this.buffer = [];
    this.maxBuffer = 1000;
    this.recording = false;
    this._unsub = null;
  }

  init() {
    document.addEventListener('export-data', () => this.showExportDialog());

    // Continuously buffer sensing data when available
    this._unsub = sensingService.onData((data) => {
      if (this.buffer.length >= this.maxBuffer) {
        this.buffer.shift();
      }
      this.buffer.push({
        timestamp: new Date().toISOString(),
        ...this.extractFields(data)
      });
    });
  }

  extractFields(data) {
    // Extract relevant fields from sensing data
    return {
      rssi: data.rssi ?? null,
      variance: data.variance ?? null,
      motion_band: data.motion_band ?? null,
      breathing_band: data.breathing_band ?? null,
      classification: data.classification ?? null,
      person_count: data.person_count ?? data.persons ?? null,
      subcarriers: data.subcarrier_count ?? null,
      source: data.source ?? null
    };
  }

  showExportDialog() {
    if (this.buffer.length === 0) {
      toastManager.warning('No sensor data to export. Connect to a data source first.');
      return;
    }

    // Create dialog
    const overlay = document.createElement('div');
    overlay.className = 'export-dialog-overlay';
    overlay.innerHTML = `
      <div class="export-dialog" role="dialog" aria-label="Export data" aria-modal="true">
        <h3>Export Sensor Data</h3>
        <p class="export-dialog-info">${this.buffer.length} data points available</p>
        <div class="export-dialog-options">
          <label class="export-option">
            <input type="radio" name="export-format" value="json" checked>
            <span>JSON</span>
            <small>Full data with nested fields</small>
          </label>
          <label class="export-option">
            <input type="radio" name="export-format" value="csv">
            <span>CSV</span>
            <small>Flat table, spreadsheet-ready</small>
          </label>
        </div>
        <div class="export-dialog-range">
          <label>
            Last <input type="number" id="export-count" value="${Math.min(this.buffer.length, 500)}" min="1" max="${this.buffer.length}"> data points
          </label>
        </div>
        <div class="export-dialog-actions">
          <button class="btn btn--secondary export-cancel">Cancel</button>
          <button class="btn btn--primary export-confirm">Export</button>
        </div>
      </div>
    `;

    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) overlay.remove();
    });
    overlay.querySelector('.export-cancel').addEventListener('click', () => overlay.remove());
    overlay.querySelector('.export-confirm').addEventListener('click', () => {
      const format = overlay.querySelector('input[name="export-format"]:checked').value;
      const count = parseInt(overlay.querySelector('#export-count').value, 10) || this.buffer.length;
      this.exportData(format, count);
      overlay.remove();
    });

    document.body.appendChild(overlay);
    overlay.querySelector('.export-confirm').focus();
  }

  exportData(format, count) {
    const data = this.buffer.slice(-count);

    let content, filename, mimeType;

    if (format === 'json') {
      content = JSON.stringify(data, null, 2);
      filename = `ruview-data-${this.timestamp()}.json`;
      mimeType = 'application/json';
    } else {
      content = this.toCSV(data);
      filename = `ruview-data-${this.timestamp()}.csv`;
      mimeType = 'text/csv';
    }

    this.downloadFile(content, filename, mimeType);
    toastManager.success(`Exported ${data.length} data points as ${format.toUpperCase()}`);
  }

  toCSV(data) {
    if (data.length === 0) return '';
    const headers = Object.keys(data[0]);
    const rows = data.map(row => headers.map(h => {
      const val = row[h];
      if (val === null || val === undefined) return '';
      if (typeof val === 'string' && (val.includes(',') || val.includes('"'))) {
        return `"${val.replace(/"/g, '""')}"`;
      }
      return String(val);
    }).join(','));
    return [headers.join(','), ...rows].join('\n');
  }

  downloadFile(content, filename, mimeType) {
    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.style.display = 'none';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  timestamp() {
    return new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
  }

  dispose() {
    if (this._unsub) this._unsub();
  }
}
