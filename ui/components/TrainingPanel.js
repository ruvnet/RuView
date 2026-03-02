// TrainingPanel Component for WiFi-DensePose UI
// Dark-mode panel for training management, CSI recordings, and progress charts.

import { trainingService } from '../services/training.service.js';

const TP_STYLES = `
.tp-panel{background:rgba(17,24,39,.9);border:1px solid rgba(56,68,89,.6);border-radius:8px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;color:#e0e0e0;overflow:hidden}
.tp-header{display:flex;align-items:center;justify-content:space-between;padding:14px 16px;background:rgba(13,17,23,.95);border-bottom:1px solid rgba(56,68,89,.6)}
.tp-title{font-size:14px;font-weight:600;color:#e0e0e0}
.tp-badge{font-size:11px;font-weight:600;padding:2px 8px;border-radius:10px}
.tp-badge-idle{background:rgba(108,117,125,.2);color:#8899aa;border:1px solid rgba(108,117,125,.3)}
.tp-badge-active{background:rgba(40,167,69,.2);color:#51cf66;border:1px solid rgba(40,167,69,.3);animation:tp-pulse 1.5s ease-in-out infinite}
.tp-badge-done{background:rgba(102,126,234,.2);color:#8ea4f0;border:1px solid rgba(102,126,234,.3)}
@keyframes tp-pulse{0%,100%{opacity:1}50%{opacity:.6}}
.tp-error{background:rgba(220,53,69,.15);color:#f5a0a8;border:1px solid rgba(220,53,69,.3);border-radius:4px;padding:8px 12px;margin:10px 12px 0;font-size:12px}
.tp-section{padding:12px;border-bottom:1px solid rgba(56,68,89,.3)}
.tp-section:last-child{border-bottom:none}
.tp-section-title{font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.5px;color:#8899aa;margin-bottom:8px}
.tp-empty{color:#6b7a8d;font-size:12px;padding:12px 0;text-align:center}
.tp-rec-row{display:flex;align-items:center;justify-content:space-between;padding:6px 8px;margin-bottom:4px;background:rgba(13,17,23,.6);border:1px solid rgba(56,68,89,.3);border-radius:4px}
.tp-rec-info{display:flex;flex-direction:column;gap:2px}
.tp-rec-name{font-size:12px;color:#c8d0dc;font-weight:500}
.tp-rec-meta{font-size:10px;color:#6b7a8d}
.tp-rec-actions{margin-top:8px}
.tp-config-header{display:flex;align-items:center;justify-content:space-between;margin-bottom:6px}
.tp-config-form{display:flex;flex-direction:column;gap:6px}
.tp-label{font-size:12px;color:#8899aa;display:block;margin-bottom:2px}
.tp-input-row{display:flex;justify-content:space-between;align-items:center;gap:8px}
.tp-input-row .tp-label{flex:1;margin-bottom:0}
.tp-input{width:110px;padding:4px 8px;background:rgba(30,40,60,.8);border:1px solid rgba(56,68,89,.6);border-radius:4px;color:#c8d0dc;font-size:12px}
.tp-input:focus{outline:none;border-color:#667eea}
.tp-ds-container{display:flex;flex-direction:column;gap:4px;margin-bottom:4px;max-height:100px;overflow-y:auto}
.tp-ds-item{display:flex;align-items:center;gap:6px;font-size:12px;color:#c8d0dc;cursor:pointer}
.tp-ds-item input{width:14px;height:14px}
.tp-train-actions{display:flex;gap:6px;margin-top:10px}
.tp-progress-bar{height:6px;background:rgba(30,40,60,.8);border-radius:3px;overflow:hidden;margin-bottom:4px}
.tp-progress-fill{height:100%;background:linear-gradient(90deg,#667eea,#764ba2);border-radius:3px;transition:width .3s}
.tp-progress-label{font-size:11px;color:#8899aa;text-align:center;margin-bottom:10px}
.tp-chart-row{display:flex;gap:8px;margin-bottom:10px;flex-wrap:wrap}
.tp-chart-row canvas{border:1px solid rgba(56,68,89,.4);border-radius:4px;flex:1;min-width:120px}
.tp-metrics-grid{display:grid;grid-template-columns:1fr 1fr;gap:6px}
.tp-metric-cell{background:rgba(13,17,23,.6);border:1px solid rgba(56,68,89,.3);border-radius:4px;padding:6px 8px}
.tp-metric-label{font-size:10px;color:#6b7a8d;text-transform:uppercase;letter-spacing:.3px}
.tp-metric-value{font-size:13px;color:#c8d0dc;font-weight:500;margin-top:2px}
.tp-btn{padding:5px 12px;border-radius:4px;font-size:12px;font-weight:500;cursor:pointer;border:1px solid transparent;transition:all .15s}
.tp-btn:disabled{opacity:.5;cursor:not-allowed}
.tp-btn-success{background:rgba(40,167,69,.2);color:#51cf66;border-color:rgba(40,167,69,.3)}
.tp-btn-success:hover:not(:disabled){background:rgba(40,167,69,.35)}
.tp-btn-danger{background:rgba(220,53,69,.2);color:#ff6b6b;border-color:rgba(220,53,69,.3)}
.tp-btn-danger:hover:not(:disabled){background:rgba(220,53,69,.35)}
.tp-btn-secondary{background:rgba(30,40,60,.8);color:#b0b8c8;border-color:rgba(56,68,89,.6)}
.tp-btn-secondary:hover:not(:disabled){background:rgba(40,50,75,.9)}
.tp-btn-rec{background:rgba(220,53,69,.15);color:#ff6b6b;border-color:rgba(220,53,69,.3)}
.tp-btn-rec:hover:not(:disabled){background:rgba(220,53,69,.3)}
.tp-btn-muted{background:transparent;color:#6b7a8d;border-color:rgba(56,68,89,.4);font-size:11px;padding:3px 8px}
.tp-btn-muted:hover:not(:disabled){color:#b0b8c8;border-color:rgba(56,68,89,.8)}
`;

export default class TrainingPanel {
  constructor(container) {
    this.container = typeof container === 'string'
      ? document.getElementById(container) : container;
    if (!this.container) throw new Error('TrainingPanel: container element not found');

    this.state = {
      recordings: [], trainingStatus: null, isRecording: false,
      configOpen: true, loading: false, error: null
    };
    this.config = {
      epochs: 100, batch_size: 32, learning_rate: 3e-4, patience: 15,
      selectedRecordings: [], base_model: '', lora_profile_name: ''
    };
    this.progressData = { losses: [], pcks: [] };
    this.unsubscribers = [];
    this._injectStyles();
    this.render();
    this.refresh();
    this._bindEvents();
  }

  _bindEvents() {
    this.unsubscribers.push(
      trainingService.on('progress', (d) => this._onProgress(d)),
      trainingService.on('training-started', () => this.refresh()),
      trainingService.on('training-stopped', () => {
        trainingService.disconnectProgressStream();
        this.refresh();
      })
    );
  }

  _onProgress(data) {
    if (data.train_loss != null) this.progressData.losses.push(data.train_loss);
    if (data.val_pck != null) this.progressData.pcks.push(data.val_pck);
    this._set({ trainingStatus: { ...this.state.trainingStatus, ...data } });
  }

  // --- Data ---

  async refresh() {
    this._set({ loading: true, error: null });
    try {
      const [recordings, status] = await Promise.all([
        trainingService.listRecordings().catch(() => []),
        trainingService.getTrainingStatus().catch(() => null)
      ]);
      if (status && !status.active) this.progressData = { losses: [], pcks: [] };
      this._set({ recordings, trainingStatus: status, loading: false });
    } catch (e) { this._set({ loading: false, error: e.message }); }
  }

  // --- Actions ---

  async _startRec() {
    this._set({ loading: true, error: null });
    try {
      await trainingService.startRecording({ session_name: `rec_${Date.now()}`, label: 'pose' });
      this._set({ isRecording: true, loading: false });
      await this.refresh();
    } catch (e) { this._set({ loading: false, error: `Recording failed: ${e.message}` }); }
  }

  async _stopRec() {
    this._set({ loading: true, error: null });
    try {
      await trainingService.stopRecording();
      this._set({ isRecording: false, loading: false });
      await this.refresh();
    } catch (e) { this._set({ loading: false, error: `Stop recording failed: ${e.message}` }); }
  }

  async _delRec(id) {
    this._set({ loading: true, error: null });
    try {
      await trainingService.deleteRecording(id);
      this.config.selectedRecordings = this.config.selectedRecordings.filter(r => r !== id);
      await this.refresh();
    } catch (e) { this._set({ loading: false, error: `Delete failed: ${e.message}` }); }
  }

  async _launchTraining(method, extraCfg = {}) {
    this._set({ loading: true, error: null });
    this.progressData = { losses: [], pcks: [] };
    try {
      trainingService.connectProgressStream();
      const payload = {
        dataset_ids: this.config.selectedRecordings,
        config: {
          epochs: this.config.epochs,
          batch_size: this.config.batch_size,
          learning_rate: this.config.learning_rate,
          ...extraCfg
        }
      };
      await trainingService[method](payload);
      await this.refresh();
    } catch (e) { this._set({ loading: false, error: `Training failed: ${e.message}` }); }
  }

  async _stopTraining() {
    this._set({ loading: true, error: null });
    try { await trainingService.stopTraining(); await this.refresh(); }
    catch (e) { this._set({ loading: false, error: `Stop failed: ${e.message}` }); }
  }

  _set(p) { Object.assign(this.state, p); this.render(); }

  // --- Render ---

  render() {
    const el = this.container;
    el.innerHTML = '';
    const panel = this._el('div', 'tp-panel');
    panel.appendChild(this._renderHeader());
    if (this.state.error) panel.appendChild(this._el('div', 'tp-error', this.state.error));
    panel.appendChild(this._renderRecordings());
    const ts = this.state.trainingStatus;
    const active = ts && ts.active;
    if (active) panel.appendChild(this._renderProgress());
    else if (ts && !ts.active && this.progressData.losses.length > 0) panel.appendChild(this._renderComplete());
    else panel.appendChild(this._renderConfig());
    el.appendChild(panel);
    if (active) requestAnimationFrame(() => this._drawCharts());
  }

  _renderHeader() {
    const h = this._el('div', 'tp-header');
    h.appendChild(this._el('span', 'tp-title', 'Training'));
    const ts = this.state.trainingStatus;
    let cls = 'tp-badge tp-badge-idle', txt = 'Idle';
    if (ts && ts.active) { cls = 'tp-badge tp-badge-active'; txt = 'Training'; }
    else if (ts && !ts.active && this.progressData.losses.length > 0) { cls = 'tp-badge tp-badge-done'; txt = 'Completed'; }
    h.appendChild(this._el('span', cls, txt));
    return h;
  }

  _renderRecordings() {
    const s = this._el('div', 'tp-section');
    s.appendChild(this._el('div', 'tp-section-title', 'CSI Recordings'));
    if (this.state.recordings.length === 0 && !this.state.loading) {
      s.appendChild(this._el('div', 'tp-empty', 'Start recording CSI data to train a model'));
    } else {
      this.state.recordings.forEach(rec => {
        const row = this._el('div', 'tp-rec-row');
        const info = this._el('div', 'tp-rec-info');
        info.appendChild(this._el('span', 'tp-rec-name', rec.name || rec.id));
        const parts = [];
        if (rec.frame_count != null) parts.push(rec.frame_count + ' frames');
        if (rec.file_size_bytes != null) parts.push(this._fmtB(rec.file_size_bytes));
        if (rec.started_at && rec.ended_at) parts.push(Math.round((new Date(rec.ended_at) - new Date(rec.started_at)) / 1000) + 's');
        info.appendChild(this._el('span', 'tp-rec-meta', parts.join(' / ')));
        row.appendChild(info);
        const del = this._btn('Delete', 'tp-btn tp-btn-muted', () => this._delRec(rec.id));
        del.disabled = this.state.loading;
        row.appendChild(del);
        s.appendChild(row);
      });
    }
    const acts = this._el('div', 'tp-rec-actions');
    if (this.state.isRecording) {
      const b = this._btn('Stop Recording', 'tp-btn tp-btn-danger', () => this._stopRec());
      b.disabled = this.state.loading; acts.appendChild(b);
    } else {
      const b = this._btn('Start Recording', 'tp-btn tp-btn-rec', () => this._startRec());
      b.disabled = this.state.loading; acts.appendChild(b);
    }
    s.appendChild(acts);
    return s;
  }

  _renderConfig() {
    const s = this._el('div', 'tp-section');
    const hdr = this._el('div', 'tp-config-header');
    hdr.appendChild(this._el('span', 'tp-section-title', 'Training Configuration'));
    hdr.appendChild(this._btn(this.state.configOpen ? 'Collapse' : 'Expand', 'tp-btn tp-btn-muted',
      () => { this.state.configOpen = !this.state.configOpen; this.render(); }));
    s.appendChild(hdr);
    if (!this.state.configOpen) return s;

    const form = this._el('div', 'tp-config-form');
    if (this.state.recordings.length > 0) {
      form.appendChild(this._el('label', 'tp-label', 'Datasets'));
      const dc = this._el('div', 'tp-ds-container');
      this.state.recordings.forEach(rec => {
        const lb = this._el('label', 'tp-ds-item');
        const cb = document.createElement('input');
        cb.type = 'checkbox';
        cb.checked = this.config.selectedRecordings.includes(rec.id);
        cb.addEventListener('change', () => {
          if (cb.checked) { if (!this.config.selectedRecordings.includes(rec.id)) this.config.selectedRecordings.push(rec.id); }
          else { this.config.selectedRecordings = this.config.selectedRecordings.filter(r => r !== rec.id); }
        });
        lb.appendChild(cb);
        lb.appendChild(this._el('span', null, rec.name || rec.id));
        dc.appendChild(lb);
      });
      form.appendChild(dc);
    }
    const ir = (l, t, v, fn) => {
      const r = this._el('div', 'tp-input-row');
      r.appendChild(this._el('label', 'tp-label', l));
      const inp = document.createElement('input');
      inp.type = t; inp.className = 'tp-input'; inp.value = v;
      inp.addEventListener('change', () => fn(inp.value));
      r.appendChild(inp); return r;
    };
    form.appendChild(ir('Epochs', 'number', this.config.epochs, v => { this.config.epochs = parseInt(v) || 100; }));
    form.appendChild(ir('Batch Size', 'number', this.config.batch_size, v => { this.config.batch_size = parseInt(v) || 32; }));
    form.appendChild(ir('Learning Rate', 'text', this.config.learning_rate, v => { this.config.learning_rate = parseFloat(v) || 3e-4; }));
    form.appendChild(ir('Early Stop Patience', 'number', this.config.patience, v => { this.config.patience = parseInt(v) || 15; }));
    form.appendChild(ir('Base Model (opt.)', 'text', this.config.base_model, v => { this.config.base_model = v; }));
    form.appendChild(ir('LoRA Profile (opt.)', 'text', this.config.lora_profile_name, v => { this.config.lora_profile_name = v; }));
    s.appendChild(form);

    const acts = this._el('div', 'tp-train-actions');
    const btns = [
      this._btn('Start Training', 'tp-btn tp-btn-success', () => this._launchTraining('startTraining', { patience: this.config.patience, base_model: this.config.base_model || undefined })),
      this._btn('Pretrain', 'tp-btn tp-btn-secondary', () => this._launchTraining('startPretraining')),
      this._btn('LoRA', 'tp-btn tp-btn-secondary', () => this._launchTraining('startLoraTraining', { base_model: this.config.base_model || undefined, profile_name: this.config.lora_profile_name || 'default' }))
    ];
    btns.forEach(b => { b.disabled = this.state.loading; acts.appendChild(b); });
    s.appendChild(acts);
    return s;
  }

  _renderProgress() {
    const ts = this.state.trainingStatus || {};
    const s = this._el('div', 'tp-section');
    s.appendChild(this._el('div', 'tp-section-title', 'Training Progress'));

    const pct = ts.total_epochs ? Math.round((ts.epoch / ts.total_epochs) * 100) : 0;
    const bar = this._el('div', 'tp-progress-bar');
    const fill = this._el('div', 'tp-progress-fill');
    fill.style.width = pct + '%';
    bar.appendChild(fill); s.appendChild(bar);
    s.appendChild(this._el('div', 'tp-progress-label', `Epoch ${ts.epoch ?? 0} / ${ts.total_epochs ?? '?'}  (${pct}%)`));

    const cr = this._el('div', 'tp-chart-row');
    const lc = document.createElement('canvas'); lc.id = 'tp-loss-chart'; lc.width = 260; lc.height = 140;
    const pc = document.createElement('canvas'); pc.id = 'tp-pck-chart'; pc.width = 260; pc.height = 140;
    cr.appendChild(lc); cr.appendChild(pc); s.appendChild(cr);

    const g = this._el('div', 'tp-metrics-grid');
    const mc = (l, v) => { const c = this._el('div', 'tp-metric-cell'); c.appendChild(this._el('div', 'tp-metric-label', l)); c.appendChild(this._el('div', 'tp-metric-value', v)); return c; };
    g.appendChild(mc('Loss', ts.train_loss != null ? ts.train_loss.toFixed(4) : '--'));
    g.appendChild(mc('PCK', ts.val_pck != null ? (ts.val_pck * 100).toFixed(1) + '%' : '--'));
    g.appendChild(mc('OKS', ts.val_oks != null ? ts.val_oks.toFixed(3) : '--'));
    g.appendChild(mc('LR', ts.lr != null ? ts.lr.toExponential(1) : '--'));
    g.appendChild(mc('Best PCK', ts.best_pck != null ? (ts.best_pck * 100).toFixed(1) + '% (e' + (ts.best_epoch ?? '?') + ')' : '--'));
    g.appendChild(mc('Patience', ts.patience_remaining != null ? String(ts.patience_remaining) : '--'));
    g.appendChild(mc('ETA', ts.eta_secs != null ? this._fmtEta(ts.eta_secs) : '--'));
    g.appendChild(mc('Phase', ts.phase || '--'));
    s.appendChild(g);

    const stop = this._btn('Stop Training', 'tp-btn tp-btn-danger', () => this._stopTraining());
    stop.disabled = this.state.loading; stop.style.marginTop = '10px'; s.appendChild(stop);
    return s;
  }

  _renderComplete() {
    const ts = this.state.trainingStatus || {};
    const s = this._el('div', 'tp-section');
    s.appendChild(this._el('div', 'tp-section-title', 'Training Complete'));
    const g = this._el('div', 'tp-metrics-grid');
    const mc = (l, v) => { const c = this._el('div', 'tp-metric-cell'); c.appendChild(this._el('div', 'tp-metric-label', l)); c.appendChild(this._el('div', 'tp-metric-value', v)); return c; };
    const losses = this.progressData.losses;
    g.appendChild(mc('Final Loss', losses.length > 0 ? losses[losses.length - 1].toFixed(4) : '--'));
    g.appendChild(mc('Best PCK', ts.best_pck != null ? (ts.best_pck * 100).toFixed(1) + '%' : '--'));
    g.appendChild(mc('Best Epoch', ts.best_epoch != null ? String(ts.best_epoch) : '--'));
    g.appendChild(mc('Total Epochs', String(losses.length)));
    s.appendChild(g);
    const acts = this._el('div', 'tp-train-actions');
    acts.appendChild(this._btn('New Training', 'tp-btn tp-btn-secondary', () => {
      this.progressData = { losses: [], pcks: [] }; this._set({ trainingStatus: null });
    }));
    s.appendChild(acts);
    return s;
  }

  // --- Chart drawing ---

  _drawCharts() {
    this._drawChart('tp-loss-chart', this.progressData.losses, { color: '#ff6b6b', label: 'Loss', yMin: 0, yMax: null });
    this._drawChart('tp-pck-chart', this.progressData.pcks, { color: '#51cf66', label: 'PCK', yMin: 0, yMax: 1 });
  }

  _drawChart(id, data, opts) {
    const cv = document.getElementById(id);
    if (!cv) return;
    const ctx = cv.getContext('2d'), w = cv.width, h = cv.height;
    const p = { t: 20, r: 10, b: 24, l: 44 };
    ctx.fillStyle = '#0d1117'; ctx.fillRect(0, 0, w, h);
    ctx.fillStyle = '#8899aa'; ctx.font = '11px -apple-system,sans-serif'; ctx.fillText(opts.label, p.l, 14);
    if (!data.length) { ctx.fillStyle = '#6b7a8d'; ctx.fillText('No data', w / 2 - 20, h / 2); return; }
    const pw = w - p.l - p.r, ph = h - p.t - p.b;
    let yMin = opts.yMin ?? Math.min(...data), yMax = opts.yMax ?? Math.max(...data);
    if (yMax === yMin) yMax = yMin + 1;
    ctx.strokeStyle = 'rgba(255,255,255,.08)'; ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
      const y = p.t + (ph / 4) * i;
      ctx.beginPath(); ctx.moveTo(p.l, y); ctx.lineTo(w - p.r, y); ctx.stroke();
      const v = yMax - ((yMax - yMin) / 4) * i;
      ctx.fillStyle = '#6b7a8d'; ctx.font = '9px sans-serif'; ctx.fillText(v.toFixed(v >= 1 ? 2 : 3), 2, y + 3);
    }
    const xl = Math.min(data.length, 5);
    for (let i = 0; i < xl; i++) {
      const idx = Math.round((data.length - 1) * (i / (xl - 1 || 1)));
      ctx.fillStyle = '#6b7a8d'; ctx.fillText(String(idx + 1), p.l + (pw * idx) / (data.length - 1 || 1) - 4, h - 4);
    }
    ctx.strokeStyle = opts.color; ctx.lineWidth = 1.5; ctx.beginPath();
    data.forEach((v, i) => {
      const x = p.l + (pw * i) / (data.length - 1 || 1);
      const y = p.t + ph - ((v - yMin) / (yMax - yMin)) * ph;
      i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    });
    ctx.stroke();
    if (data.length > 0) {
      const ly = p.t + ph - ((data[data.length - 1] - yMin) / (yMax - yMin)) * ph;
      ctx.fillStyle = opts.color; ctx.beginPath(); ctx.arc(p.l + pw, ly, 3, 0, Math.PI * 2); ctx.fill();
    }
  }

  // --- Helpers ---

  _el(tag, cls, txt) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (txt != null) e.textContent = txt;
    return e;
  }

  _btn(txt, cls, fn) {
    const b = document.createElement('button');
    b.className = cls; b.textContent = txt;
    b.addEventListener('click', fn); return b;
  }

  _fmtB(b) { return b < 1024 ? b + ' B' : b < 1048576 ? (b / 1024).toFixed(1) + ' KB' : (b / 1048576).toFixed(1) + ' MB'; }
  _fmtEta(s) { return s < 60 ? Math.round(s) + 's' : s < 3600 ? Math.round(s / 60) + 'm' : (s / 3600).toFixed(1) + 'h'; }

  _injectStyles() {
    if (document.getElementById('training-panel-styles')) return;
    const s = document.createElement('style');
    s.id = 'training-panel-styles';
    s.textContent = TP_STYLES;
    document.head.appendChild(s);
  }

  destroy() {
    this.unsubscribers.forEach(fn => fn());
    this.unsubscribers = [];
    trainingService.disconnectProgressStream();
    if (this.container) this.container.innerHTML = '';
  }

  dispose() {
    this.destroy();
  }
}
