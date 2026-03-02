// ModelPanel Component for WiFi-DensePose UI
// Dark-mode panel for model management: listing, loading, LoRA profiles.

import { modelService } from '../services/model.service.js';

const MP_STYLES = `
.mp-panel{background:rgba(17,24,39,.9);border:1px solid rgba(56,68,89,.6);border-radius:8px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;color:#e0e0e0;overflow:hidden}
.mp-header{display:flex;align-items:center;justify-content:space-between;padding:14px 16px;background:rgba(13,17,23,.95);border-bottom:1px solid rgba(56,68,89,.6)}
.mp-title{font-size:14px;font-weight:600;color:#e0e0e0}
.mp-badge{background:rgba(102,126,234,.2);color:#8ea4f0;font-size:11px;font-weight:600;padding:2px 8px;border-radius:10px;border:1px solid rgba(102,126,234,.3)}
.mp-error{background:rgba(220,53,69,.15);color:#f5a0a8;border:1px solid rgba(220,53,69,.3);border-radius:4px;padding:8px 12px;margin:10px 12px 0;font-size:12px}
.mp-active-card{margin:12px;padding:12px;background:rgba(13,17,23,.8);border:1px solid rgba(56,68,89,.6);border-left:3px solid #28a745;border-radius:6px}
.mp-active-name{font-size:14px;font-weight:600;color:#c8d0dc;margin-bottom:6px}
.mp-active-meta{display:flex;gap:6px;flex-wrap:wrap;margin-bottom:8px}
.mp-active-stats{font-size:12px;color:#8899aa;margin-bottom:10px}
.mp-stat-label{color:#8899aa}.mp-stat-value{color:#c8d0dc;font-weight:500}.mp-stat-sep{color:rgba(56,68,89,.8);margin:0 6px}
.mp-lora-row{display:flex;align-items:center;gap:8px;margin-bottom:10px}
.mp-lora-label{font-size:12px;color:#8899aa}
.mp-lora-select{flex:1;padding:4px 8px;background:rgba(30,40,60,.8);border:1px solid rgba(56,68,89,.6);border-radius:4px;color:#c8d0dc;font-size:12px}
.mp-list-section{padding:0 12px 12px}
.mp-section-title{font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.5px;color:#8899aa;padding:10px 0 8px}
.mp-model-card{padding:10px;margin-bottom:8px;background:rgba(13,17,23,.6);border:1px solid rgba(56,68,89,.4);border-radius:6px;transition:border-color .2s}
.mp-model-card:hover{border-color:rgba(102,126,234,.4)}
.mp-card-name{font-size:13px;font-weight:500;color:#c8d0dc;margin-bottom:4px}
.mp-card-meta{display:flex;gap:6px;flex-wrap:wrap;margin-bottom:8px}
.mp-meta-tag{background:rgba(30,40,60,.8);color:#8899aa;font-size:10px;padding:2px 6px;border-radius:3px;border:1px solid rgba(56,68,89,.4)}
.mp-card-actions{display:flex;gap:6px}
.mp-empty{color:#6b7a8d;font-size:12px;padding:16px 0;text-align:center;line-height:1.5}
.mp-footer{padding:10px 12px;border-top:1px solid rgba(56,68,89,.4);display:flex;justify-content:flex-end}
.mp-btn{padding:5px 12px;border-radius:4px;font-size:12px;font-weight:500;cursor:pointer;border:1px solid transparent;transition:all .15s}
.mp-btn:disabled{opacity:.5;cursor:not-allowed}
.mp-btn-success{background:rgba(40,167,69,.2);color:#51cf66;border-color:rgba(40,167,69,.3)}
.mp-btn-success:hover:not(:disabled){background:rgba(40,167,69,.35)}
.mp-btn-danger{background:rgba(220,53,69,.2);color:#ff6b6b;border-color:rgba(220,53,69,.3)}
.mp-btn-danger:hover:not(:disabled){background:rgba(220,53,69,.35)}
.mp-btn-secondary{background:rgba(30,40,60,.8);color:#b0b8c8;border-color:rgba(56,68,89,.6)}
.mp-btn-secondary:hover:not(:disabled){background:rgba(40,50,75,.9)}
.mp-btn-muted{background:transparent;color:#6b7a8d;border-color:rgba(56,68,89,.4);font-size:11px;padding:4px 8px}
.mp-btn-muted:hover:not(:disabled){color:#ff6b6b;border-color:rgba(220,53,69,.3)}
`;

export default class ModelPanel {
  constructor(container) {
    this.container = typeof container === 'string'
      ? document.getElementById(container) : container;
    if (!this.container) throw new Error('ModelPanel: container element not found');

    this.state = { models: [], activeModel: null, loraProfiles: [], loading: false, error: null };
    this.unsubs = [];
    this._injectStyles();
    this.render();
    this.refresh();
    this.unsubs.push(
      modelService.on('model-loaded', () => this.refresh()),
      modelService.on('model-unloaded', () => this.refresh()),
      modelService.on('lora-activated', () => this.refresh())
    );
  }

  // --- Data ---

  async refresh() {
    this._set({ loading: true, error: null });
    try {
      const [listRes, active] = await Promise.all([
        modelService.listModels().catch(() => ({ models: [] })),
        modelService.getActiveModel().catch(() => null)
      ]);
      let lora = [];
      if (active) lora = await modelService.getLoraProfiles().catch(() => []);
      this._set({ models: listRes?.models ?? [], activeModel: active, loraProfiles: lora, loading: false });
    } catch (e) { this._set({ loading: false, error: e.message }); }
  }

  // --- Actions ---

  async _load(id) {
    this._set({ loading: true, error: null });
    try { await modelService.loadModel(id); await this.refresh(); }
    catch (e) { this._set({ loading: false, error: `Load failed: ${e.message}` }); }
  }

  async _unload() {
    this._set({ loading: true, error: null });
    try { await modelService.unloadModel(); await this.refresh(); }
    catch (e) { this._set({ loading: false, error: `Unload failed: ${e.message}` }); }
  }

  async _delete(id) {
    this._set({ loading: true, error: null });
    try { await modelService.deleteModel(id); await this.refresh(); }
    catch (e) { this._set({ loading: false, error: `Delete failed: ${e.message}` }); }
  }

  async _loraChange(modelId, profile) {
    if (!profile) return;
    this._set({ loading: true, error: null });
    try { await modelService.activateLoraProfile(modelId, profile); await this.refresh(); }
    catch (e) { this._set({ loading: false, error: `LoRA failed: ${e.message}` }); }
  }

  _set(p) { Object.assign(this.state, p); this.render(); }

  // --- Render ---

  render() {
    const el = this.container;
    el.innerHTML = '';
    const panel = this._el('div', 'mp-panel');

    // Header
    const hdr = this._el('div', 'mp-header');
    hdr.appendChild(this._el('span', 'mp-title', 'Model Library'));
    hdr.appendChild(this._el('span', 'mp-badge', String(this.state.models.length)));
    panel.appendChild(hdr);

    if (this.state.error) panel.appendChild(this._el('div', 'mp-error', this.state.error));

    // Active model
    if (this.state.activeModel) panel.appendChild(this._renderActive());

    // List
    const ls = this._el('div', 'mp-list-section');
    ls.appendChild(this._el('div', 'mp-section-title', 'Available Models'));
    const models = this.state.models.filter(
      m => !(this.state.activeModel && this.state.activeModel.model_id === m.id)
    );
    if (models.length === 0 && !this.state.loading) {
      ls.appendChild(this._el('div', 'mp-empty', 'No .rvf models found. Train a model or place .rvf files in data/models/'));
    } else {
      models.forEach(m => ls.appendChild(this._renderCard(m)));
    }
    panel.appendChild(ls);

    // Footer
    const ft = this._el('div', 'mp-footer');
    const rb = this._btn('Refresh', 'mp-btn mp-btn-secondary', () => this.refresh());
    rb.disabled = this.state.loading;
    ft.appendChild(rb);
    panel.appendChild(ft);

    el.appendChild(panel);
  }

  _renderActive() {
    const am = this.state.activeModel;
    const card = this._el('div', 'mp-active-card');
    card.appendChild(this._el('div', 'mp-active-name', am.model_id || 'Active Model'));

    const full = this.state.models.find(m => m.id === am.model_id);
    if (full) {
      const meta = this._el('div', 'mp-active-meta');
      if (full.version) meta.appendChild(this._tag('v' + full.version));
      if (full.pck_score != null) meta.appendChild(this._tag('PCK ' + (full.pck_score * 100).toFixed(1) + '%'));
      card.appendChild(meta);
    }

    if (am.avg_inference_ms != null) {
      const st = this._el('div', 'mp-active-stats');
      st.innerHTML = `<span class="mp-stat-label">Inference:</span> <span class="mp-stat-value">${am.avg_inference_ms.toFixed(1)} ms</span><span class="mp-stat-sep">|</span><span class="mp-stat-label">Frames:</span> <span class="mp-stat-value">${am.frames_processed ?? 0}</span>`;
      card.appendChild(st);
    }

    if (this.state.loraProfiles.length > 0) {
      const row = this._el('div', 'mp-lora-row');
      row.appendChild(this._el('span', 'mp-lora-label', 'LoRA Profile:'));
      const sel = document.createElement('select');
      sel.className = 'mp-lora-select';
      const def = document.createElement('option');
      def.value = ''; def.textContent = '-- none --'; sel.appendChild(def);
      this.state.loraProfiles.forEach(p => {
        const o = document.createElement('option');
        o.value = p; o.textContent = p; sel.appendChild(o);
      });
      sel.addEventListener('change', () => this._loraChange(am.model_id, sel.value));
      row.appendChild(sel);
      card.appendChild(row);
    }

    const ub = this._btn('Unload', 'mp-btn mp-btn-danger', () => this._unload());
    ub.disabled = this.state.loading;
    card.appendChild(ub);
    return card;
  }

  _renderCard(model) {
    const card = this._el('div', 'mp-model-card');
    card.appendChild(this._el('div', 'mp-card-name', model.filename || model.id));
    const meta = this._el('div', 'mp-card-meta');
    if (model.version) meta.appendChild(this._tag('v' + model.version));
    if (model.size_bytes != null) meta.appendChild(this._tag(this._fmtB(model.size_bytes)));
    if (model.pck_score != null) meta.appendChild(this._tag('PCK ' + (model.pck_score * 100).toFixed(1) + '%'));
    if (model.lora_profiles && model.lora_profiles.length > 0) meta.appendChild(this._tag(model.lora_profiles.length + ' LoRA'));
    card.appendChild(meta);

    const acts = this._el('div', 'mp-card-actions');
    const lb = this._btn('Load', 'mp-btn mp-btn-success', () => this._load(model.id));
    lb.disabled = this.state.loading;
    const db = this._btn('Delete', 'mp-btn mp-btn-muted', () => this._delete(model.id));
    db.disabled = this.state.loading;
    acts.appendChild(lb); acts.appendChild(db);
    card.appendChild(acts);
    return card;
  }

  // --- Helpers ---

  _el(tag, cls, txt) { const e = document.createElement(tag); if (cls) e.className = cls; if (txt != null) e.textContent = txt; return e; }
  _btn(txt, cls, fn) { const b = document.createElement('button'); b.className = cls; b.textContent = txt; b.addEventListener('click', fn); return b; }
  _tag(txt) { return this._el('span', 'mp-meta-tag', txt); }
  _fmtB(b) { return b < 1024 ? b + ' B' : b < 1048576 ? (b / 1024).toFixed(1) + ' KB' : (b / 1048576).toFixed(1) + ' MB'; }

  _injectStyles() {
    if (document.getElementById('model-panel-styles')) return;
    const s = document.createElement('style');
    s.id = 'model-panel-styles';
    s.textContent = MP_STYLES;
    document.head.appendChild(s);
  }

  destroy() {
    this.unsubs.forEach(fn => fn());
    this.unsubs = [];
    if (this.container) this.container.innerHTML = '';
  }

  dispose() {
    this.destroy();
  }
}
