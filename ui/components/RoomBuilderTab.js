/**
 * RoomBuilderTab — define room geometry and sensor node positions
 *
 * A 2D top-down floor-plan editor: set room width/depth, add sensor nodes,
 * drag them into place (or type exact coordinates), and save. Saving both
 * applies the positions live (no restart needed) and persists them to
 * <data_dir>/room_config.json, which is loaded automatically on the next
 * server launch — replacing the --node-positions CLI-only workflow.
 */

import { apiService } from '../services/api.service.js';
import { toastManager } from '../utils/toast.js';

const ENDPOINT = '/api/v1/config/room';
const CANVAS_W = 640;
const CANVAS_H = 480;
const MARGIN = 32;
const NODE_RADIUS = 9;
const METERS_PER_FOOT = 0.3048;
const UNITS_STORAGE_KEY = 'roombuilder-units';

export class RoomBuilderTab {
  /** @param {HTMLElement} container - the #roombuilder section element */
  constructor(container) {
    this.container = container;
    this.config = { width_m: 5, depth_m: 4, nodes: [] };
    this._dragIndex = null;
    this._loaded = false;
    // Display-only - this.config always stays in meters (that's what the
    // server/API/saved file use); only input values and labels convert.
    this._units = 'metric';
    try {
      const saved = localStorage.getItem(UNITS_STORAGE_KEY);
      if (saved === 'metric' || saved === 'imperial') this._units = saved;
    } catch (e) { /* storage unavailable - default to metric */ }
  }

  /** Convert a length in meters to the currently-selected display unit. */
  _toDisplay(meters) {
    return this._units === 'imperial' ? meters / METERS_PER_FOOT : meters;
  }

  /** Convert a length from the currently-selected display unit back to meters. */
  _fromDisplay(value) {
    return this._units === 'imperial' ? value * METERS_PER_FOOT : value;
  }

  _unitLabel() {
    return this._units === 'imperial' ? 'ft' : 'm';
  }

  async init() {
    this._buildDOM();
    this._wireEvents();
    await this._load();
  }

  // ---- DOM ----------------------------------------------------------------

  _buildDOM() {
    this.container.innerHTML = `
      <style>
        .rb-layout { display: flex; gap: 20px; flex-wrap: wrap; }
        .rb-canvas-wrap { background: #0d1117; border: 1px solid #2a2f3a; border-radius: 8px; padding: 12px; }
        .rb-canvas-wrap canvas { display: block; cursor: default; border-radius: 4px; }
        .rb-panel { flex: 1; min-width: 280px; display: flex; flex-direction: column; gap: 16px; }
        .rb-card { background: #12161f; border: 1px solid #2a2f3a; border-radius: 8px; padding: 14px; }
        .rb-card-title { font-size: 12px; text-transform: uppercase; letter-spacing: .05em; color: #8b93a7; margin-bottom: 10px; }
        .rb-room-dims { display: flex; gap: 12px; }
        .rb-room-dims label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: #8b93a7; }
        .rb-room-dims input, .rb-node-row input { background: #0d1117; border: 1px solid #2a2f3a; border-radius: 4px; color: #e6e9ef; padding: 5px 7px; font-size: 13px; box-sizing: border-box; }
        .rb-room-dims input { width: 72px; }
        .rb-room-dims select { background: #0d1117; border: 1px solid #2a2f3a; border-radius: 4px; color: #e6e9ef; padding: 5px 7px; font-size: 13px; }
        .rb-node-row { display: grid; grid-template-columns: 42px 1fr 60px 60px 60px 1fr 24px; gap: 6px; align-items: center; margin-bottom: 6px; }
        .rb-node-row input { width: 100%; }
        .rb-node-row .rb-id { color: #32b8c6; font-weight: 600; font-size: 13px; }
        /* Number inputs' native up/down spinner eats most of the width in
           narrow columns (esp. the 42px ID field) - hide it. Nobody needs
           spinner arrows to type a node ID or a coordinate. */
        .rb-node-row input[type="number"]::-webkit-outer-spin-button,
        .rb-node-row input[type="number"]::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }
        .rb-node-row input[type="number"] { -moz-appearance: textfield; appearance: textfield; }
        .rb-remove-btn { background: none; border: none; color: #e05561; cursor: pointer; font-size: 15px; line-height: 1; }
        .rb-btn { background: #32b8c6; color: #06222a; border: none; border-radius: 6px; padding: 8px 16px; font-weight: 600; cursor: pointer; font-size: 13px; }
        .rb-btn.secondary { background: transparent; color: #32b8c6; border: 1px solid #32b8c6; }
        .rb-actions { display: flex; gap: 10px; margin-top: 8px; }
        .rb-hint { color: #6b7280; font-size: 12px; margin-top: 8px; line-height: 1.5; }
        .rb-col-headers { display: grid; grid-template-columns: 42px 1fr 60px 60px 60px 1fr 24px; gap: 6px; font-size: 11px; color: #6b7280; margin-bottom: 6px; }
      </style>
      <h2>Room Builder</h2>
      <p class="rb-hint" style="margin-bottom:16px;">
        Define your room and place sensor nodes. Node <strong>ID</strong> must match
        each board's provisioned <code>--node-id</code>. Saving applies immediately
        (no restart) and is what future launches load automatically.
      </p>
      <div class="rb-layout">
        <div class="rb-canvas-wrap">
          <canvas id="rbCanvas" width="${CANVAS_W}" height="${CANVAS_H}"></canvas>
        </div>
        <div class="rb-panel">
          <div class="rb-card">
            <div class="rb-card-title">Room Dimensions</div>
            <div class="rb-room-dims">
              <label><span>Width (<span class="rb-unit-label">${this._unitLabel()}</span>)</span> <input type="number" id="rbWidth" min="0.1" step="0.1" value="${this._toDisplay(this.config.width_m).toFixed(2)}"></label>
              <label><span>Depth (<span class="rb-unit-label">${this._unitLabel()}</span>)</span> <input type="number" id="rbDepth" min="0.1" step="0.1" value="${this._toDisplay(this.config.depth_m).toFixed(2)}"></label>
              <label><span>Units</span>
                <select id="rbUnits">
                  <option value="metric" ${this._units === 'metric' ? 'selected' : ''}>Metric (m)</option>
                  <option value="imperial" ${this._units === 'imperial' ? 'selected' : ''}>Imperial (ft)</option>
                </select>
              </label>
            </div>
          </div>
          <div class="rb-card">
            <div class="rb-card-title">Sensor Nodes</div>
            <div class="rb-col-headers">
              <span>ID</span><span>Label</span><span>X (<span class="rb-unit-label">${this._unitLabel()}</span>)</span><span>Y (<span class="rb-unit-label">${this._unitLabel()}</span>)</span><span>Z (<span class="rb-unit-label">${this._unitLabel()}</span>)</span><span></span><span></span>
            </div>
            <div id="rbNodeList"></div>
            <div class="rb-actions">
              <button class="rb-btn secondary" id="rbAddNode">+ Add Node</button>
            </div>
          </div>
          <div class="rb-actions">
            <button class="rb-btn" id="rbSave">Save</button>
          </div>
          <p class="rb-hint">
            Drag a node on the canvas to reposition it (X/Y only — set height
            with the Z field). Coordinates are in the same room-space meters
            used by <code>--node-positions</code>.
          </p>
          <p class="rb-hint">
            <strong>(0, 0)</strong> is the room's <strong>Northwest</strong> corner
            (the <strong>N</strong> arrow on the canvas points that way) — X increases
            going <strong>East</strong>, Y increases going <strong>South</strong>. Face
            the room's actual north wall and match it up when you place nodes.
          </p>
        </div>
      </div>
    `;
  }

  _wireEvents() {
    const widthEl = this.container.querySelector('#rbWidth');
    const depthEl = this.container.querySelector('#rbDepth');
    widthEl.addEventListener('input', () => {
      this.config.width_m = Math.max(0.1, this._fromDisplay(parseFloat(widthEl.value) || 0));
      this._render();
    });
    depthEl.addEventListener('input', () => {
      this.config.depth_m = Math.max(0.1, this._fromDisplay(parseFloat(depthEl.value) || 0));
      this._render();
    });

    this.container.querySelector('#rbUnits').addEventListener('change', (e) => {
      this._units = e.target.value;
      try { localStorage.setItem(UNITS_STORAGE_KEY, this._units); } catch (err) { /* ignore */ }
      this._refreshUnitDisplay();
    });

    this.container.querySelector('#rbAddNode').addEventListener('click', () => {
      this._addNode();
    });
    this.container.querySelector('#rbSave').addEventListener('click', () => {
      this._save();
    });

    const canvas = this.container.querySelector('#rbCanvas');
    canvas.addEventListener('mousedown', (e) => this._onCanvasDown(e));
    canvas.addEventListener('mousemove', (e) => this._onCanvasMove(e));
    window.addEventListener('mouseup', () => { this._dragIndex = null; });
  }

  // ---- Data -----------------------------------------------------------------

  async _load() {
    try {
      const data = await apiService.get(ENDPOINT);
      if (data && typeof data === 'object') {
        this.config = {
          width_m: data.width_m > 0 ? data.width_m : 5,
          depth_m: data.depth_m > 0 ? data.depth_m : 4,
          nodes: Array.isArray(data.nodes) ? data.nodes : [],
        };
      }
    } catch (e) {
      console.warn('[RoomBuilder] Failed to load room config, using defaults:', e.message);
    }
    this._loaded = true;
    this._refreshUnitDisplay();
  }

  /** Re-render everything that shows a unit-dependent value, using the
   * current `this._units` - called on load and whenever the unit toggle
   * changes. `this.config` itself never changes here; only what's displayed. */
  _refreshUnitDisplay() {
    const label = this._unitLabel();
    this.container.querySelectorAll('.rb-unit-label').forEach((el) => {
      el.textContent = label;
    });
    this.container.querySelector('#rbWidth').value = this._toDisplay(this.config.width_m).toFixed(2);
    this.container.querySelector('#rbDepth').value = this._toDisplay(this.config.depth_m).toFixed(2);
    this._renderNodeList();
    this._render();
  }

  async _save() {
    // Pull the latest values out of the node-row inputs before sending -
    // numeric fields don't write back into this.config until blur/save.
    this._syncNodesFromInputs();
    const ids = this.config.nodes.map((n) => n.id);
    if (new Set(ids).size !== ids.length) {
      toastManager.error('Cannot save: two or more nodes have the same ID. Give each a unique ID first.');
      return;
    }
    try {
      // The server validates width/depth/duplicate-ids and responds 200 OK
      // with an {"error": ...} body rather than a non-2xx status, so a
      // rejected save must be checked for explicitly - it won't throw.
      const result = await apiService.post(ENDPOINT, this.config);
      if (result && result.error) {
        toastManager.error(`Save rejected: ${result.error}`);
        return;
      }
      toastManager.success('Room config saved — applied immediately, and will load automatically on next launch.');
    } catch (e) {
      toastManager.error(`Failed to save room config: ${e.message}`);
    }
  }

  _nextFreeId() {
    const used = new Set(this.config.nodes.map((n) => n.id));
    let id = 0;
    while (used.has(id)) id++;
    return id;
  }

  _addNode() {
    this.config.nodes.push({
      id: this._nextFreeId(),
      x: this.config.width_m / 2,
      y: this.config.depth_m / 2,
      z: 0.4,
      label: '',
    });
    this._renderNodeList();
    this._render();
  }

  _removeNode(index) {
    this.config.nodes.splice(index, 1);
    this._renderNodeList();
    this._render();
  }

  /** Pull edited values out of the node-row inputs back into this.config.nodes.
   *
   * Rows are matched to nodes by array index (`row.dataset.index`), NOT by
   * `id` - id is one of the fields being edited, so using it as the lookup
   * key meant renaming a node to an ID that collided with another node's ID
   * broke the row<->node link: both rows' edits would then read/write
   * whichever node .find() happened to return first, which looked like one
   * node "jumping" onto or stacking with another. Array index never changes
   * just because a field's value changes, so it can't have that problem. */
  _syncNodesFromInputs() {
    const rows = this.container.querySelectorAll('.rb-node-row');
    rows.forEach((row) => {
      const idx = parseInt(row.dataset.index, 10);
      const node = this.config.nodes[idx];
      if (!node) return;
      const idInput = row.querySelector('.rb-id-input');
      const newId = parseInt(idInput.value, 10);
      if (!Number.isNaN(newId)) node.id = newId;
      node.label = row.querySelector('.rb-label').value;
      node.x = this._fromDisplay(parseFloat(row.querySelector('.rb-x').value) || 0);
      node.y = this._fromDisplay(parseFloat(row.querySelector('.rb-y').value) || 0);
      node.z = this._fromDisplay(parseFloat(row.querySelector('.rb-z').value) || 0);
    });
    this._warnOnDuplicateIds();
  }

  _warnOnDuplicateIds() {
    const seen = new Set();
    for (const node of this.config.nodes) {
      if (seen.has(node.id)) {
        toastManager.warning(`Duplicate node ID ${node.id} — IDs must be unique before saving.`);
        return;
      }
      seen.add(node.id);
    }
  }

  // ---- Node list (form rows) -------------------------------------------------

  _renderNodeList() {
    const list = this.container.querySelector('#rbNodeList');
    list.innerHTML = '';
    // A bad value here (non-finite x/y/z, or an id survey away turning up
    // NaN) used to throw out of .toFixed() partway through the loop,
    // silently aborting the render for every node after it - one bad node
    // could make a later, perfectly fine node vanish from both the list and
    // the canvas with no visible error. Coerce defensively instead.
    const safe = (v, fallback = 0) => (Number.isFinite(v) ? v : fallback);
    this.config.nodes.forEach((node, idx) => {
      if (!Number.isFinite(node.x) || !Number.isFinite(node.y) || !Number.isFinite(node.z)) {
        console.warn('[RoomBuilder] Node has non-finite coordinate(s), coercing to 0:', node);
      }
      const row = document.createElement('div');
      row.className = 'rb-node-row';
      row.dataset.index = idx;
      row.innerHTML = `
        <input class="rb-id-input" type="number" min="0" value="${safe(node.id)}" title="Node ID (must match --node-id)">
        <input class="rb-label" type="text" value="${node.label || ''}" placeholder="e.g. front-right">
        <input class="rb-x" type="number" step="0.05" value="${this._toDisplay(safe(node.x)).toFixed(2)}">
        <input class="rb-y" type="number" step="0.05" value="${this._toDisplay(safe(node.y)).toFixed(2)}">
        <input class="rb-z" type="number" step="0.05" value="${this._toDisplay(safe(node.z)).toFixed(2)}">
        <span></span>
        <button class="rb-remove-btn" title="Remove node">&times;</button>
      `;
      row.querySelectorAll('input').forEach((inp) => {
        inp.addEventListener('input', () => {
          this._syncNodesFromInputs();
          this._render();
        });
      });
      row.querySelector('.rb-remove-btn').addEventListener('click', () => this._removeNode(idx));
      list.appendChild(row);
    });
  }

  // ---- Canvas -----------------------------------------------------------------

  /** Room-space (x,y) meters -> canvas pixel coordinates. */
  _toPixel(x, y) {
    const scale = Math.min(
      (CANVAS_W - 2 * MARGIN) / this.config.width_m,
      (CANVAS_H - 2 * MARGIN) / this.config.depth_m
    );
    return { px: MARGIN + x * scale, py: MARGIN + y * scale, scale };
  }

  /** Canvas pixel coordinates -> room-space (x,y) meters, clamped to the room. */
  _toRoom(px, py) {
    const scale = Math.min(
      (CANVAS_W - 2 * MARGIN) / this.config.width_m,
      (CANVAS_H - 2 * MARGIN) / this.config.depth_m
    );
    const x = Math.min(this.config.width_m, Math.max(0, (px - MARGIN) / scale));
    const y = Math.min(this.config.depth_m, Math.max(0, (py - MARGIN) / scale));
    return { x, y };
  }

  _canvasPos(e) {
    const canvas = this.container.querySelector('#rbCanvas');
    const rect = canvas.getBoundingClientRect();
    return {
      x: ((e.clientX - rect.left) / rect.width) * CANVAS_W,
      y: ((e.clientY - rect.top) / rect.height) * CANVAS_H,
    };
  }

  _onCanvasDown(e) {
    const pos = this._canvasPos(e);
    this.config.nodes.forEach((node, idx) => {
      const { px, py } = this._toPixel(node.x, node.y);
      if (this._dragIndex == null && Math.hypot(px - pos.x, py - pos.y) <= NODE_RADIUS + 4) {
        this._dragIndex = idx;
      }
    });
  }

  _onCanvasMove(e) {
    if (this._dragIndex == null) return;
    const node = this.config.nodes[this._dragIndex];
    if (!node) return;
    const pos = this._canvasPos(e);
    const room = this._toRoom(pos.x, pos.y);
    node.x = Math.round(room.x * 100) / 100;
    node.y = Math.round(room.y * 100) / 100;
    this._renderNodeList();
    this._render();
  }

  _render() {
    const canvas = this.container.querySelector('#rbCanvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, CANVAS_W, CANVAS_H);

    // Room rectangle.
    const topLeft = this._toPixel(0, 0);
    const bottomRight = this._toPixel(this.config.width_m, this.config.depth_m);
    ctx.strokeStyle = '#32b8c6';
    ctx.lineWidth = 2;
    ctx.strokeRect(topLeft.px, topLeft.py, bottomRight.px - topLeft.px, bottomRight.py - topLeft.py);

    // Faint grid every meter, for scale reference.
    ctx.strokeStyle = 'rgba(50,184,198,0.15)';
    ctx.lineWidth = 1;
    for (let gx = 1; gx < this.config.width_m; gx++) {
      const { px } = this._toPixel(gx, 0);
      ctx.beginPath();
      ctx.moveTo(px, topLeft.py);
      ctx.lineTo(px, bottomRight.py);
      ctx.stroke();
    }
    for (let gy = 1; gy < this.config.depth_m; gy++) {
      const { py } = this._toPixel(0, gy);
      ctx.beginPath();
      ctx.moveTo(topLeft.px, py);
      ctx.lineTo(bottomRight.px, py);
      ctx.stroke();
    }

    // Nodes.
    this.config.nodes.forEach((node, idx) => {
      const { px, py } = this._toPixel(node.x, node.y);
      if (!Number.isFinite(px) || !Number.isFinite(py)) {
        console.warn('[RoomBuilder] Skipping node with non-finite position:', node);
        return;
      }
      ctx.beginPath();
      ctx.arc(px, py, NODE_RADIUS, 0, Math.PI * 2);
      ctx.fillStyle = idx === this._dragIndex ? '#ffd166' : '#e05561';
      ctx.fill();
      ctx.strokeStyle = '#0d1117';
      ctx.lineWidth = 2;
      ctx.stroke();

      ctx.fillStyle = '#e6e9ef';
      ctx.font = '12px monospace';
      ctx.textAlign = 'center';
      const label = node.label ? `${node.id}:${node.label}` : `${node.id}`;
      ctx.fillText(label, px, py - NODE_RADIUS - 6);
    });

    this._drawCompass(ctx);
  }

  /** (0,0,0) is the room's Northwest corner by convention - X increases
   * going East, Y increases going South. That's already how _toPixel maps
   * room space onto the canvas (x -> right, y -> down); this just draws a
   * fixed compass badge in the corner so it reads as "North" instead of
   * an arbitrary direction, so a person can orient the room to reality. */
  _drawCompass(ctx) {
    const cx = 20;
    const cy = 20;
    const len = 10;
    ctx.save();
    ctx.strokeStyle = '#e6e9ef';
    ctx.fillStyle = '#e6e9ef';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(cx, cy + len);
    ctx.lineTo(cx, cy - len);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(cx, cy - len);
    ctx.lineTo(cx - 4, cy - len + 6);
    ctx.lineTo(cx + 4, cy - len + 6);
    ctx.closePath();
    ctx.fill();
    ctx.font = 'bold 11px monospace';
    ctx.textAlign = 'center';
    ctx.fillText('N', cx, cy + len + 13);
    ctx.restore();
  }
}
