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
import { sensingService } from '../services/sensing.service.js';

const ENDPOINT = '/api/v1/config/room';
const CANVAS_W = 640;
const CANVAS_H = 480;
const MARGIN = 32;
const NODE_RADIUS = 9;
const AP_RADIUS = 8;
const LIVE_DOT_RADIUS = 8;
const METERS_PER_FOOT = 0.3048;
const METERS_PER_INCH = 0.0254;
// Typical residential defaults, used when a storey is first added.
const DEFAULT_CEILING_IN = 96;    // 8 ft
const DEFAULT_SUBFLOOR_IN = 16;   // joists + subfloor between storeys
const UNITS_STORAGE_KEY = 'roombuilder-units';
// Derived from the inch figures above so there is one source of truth for a
// default storey; a second one in metres drifted from it the moment ceiling
// height became an input rather than a constant.
const DEFAULT_CEILING_M = DEFAULT_CEILING_IN * METERS_PER_INCH;
const DEFAULT_SUBFLOOR_M = DEFAULT_SUBFLOOR_IN * METERS_PER_INCH;
// Click within this many pixels of a wall endpoint to grab it.
const WALL_HIT_PX = 7;

export class RoomBuilderTab {
  /** @param {HTMLElement} container - the #roombuilder section element */
  constructor(container) {
    this.container = container;
    this.config = { width_m: 5, depth_m: 4, nodes: [], ap_position: null };
    this._dragIndex = null;
    this._draggingAp = false;
    // Which storey the canvas is editing. Nodes and walls on other storeys
    // are drawn faintly rather than hidden, so you can line up a second-floor
    // node with the wall below it -- which is the whole point of stacking
    // storeys on a shared origin.
    this._activeFloor = 1;
    // 'select' drags nodes and the AP; 'wall' draws segments; 'footprint'
    // clicks out the building's outline vertex by vertex.
    this._mode = 'select';
    this._wallStart = null;
    this._wallPreview = null;
    // The outline being traced right now, as {x, y} in room coordinates. It
    // is not part of `config` until closed: a half-traced ring is not a shape,
    // and the server rejects one with fewer than three vertices.
    this._ringDraft = null;
    // Where the pointer is, so the segment about to be committed is visible
    // before the click that commits it.
    this._ringHover = null;
    this._loaded = false;
    // Live tracked-position overlay, fed by sensingService (/ws/sensing).
    // `_liveDot` is only ever set from a "bistatic_velocity", "doppler_centroid",
    // or "motion_centroid" position_source (real room-space meters, same
    // convention as this.config.nodes) — a "field_peak" fix lives in the Observatory's own
    // grid-centered coordinate frame (unrelated to this room's actual
    // width_m/depth_m), so plotting it here would be a fabricated-looking
    // position. `_liveStatus` explains why no dot is showing when that's the
    // case. `_liveDotSource` records which tier produced the current dot, so
    // the canvas label can say which one is actually driving it.
    this._liveDot = null;
    this._liveDotSource = null;
    this._liveStatus = 'Waiting for live sensing data…';
    this._unsubSensingData = null;
    // Display-only - this.config always stays in meters (that's what the
    // server/API/saved file use); only input values and labels convert.
    this._units = 'imperial';
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

  /** Heights are entered in INCHES when imperial, not feet.
   *
   * A person siting a node measures "the outlet is 20 inches" and "the
   * ceiling is 8 foot". Forcing one unit on both means every height becomes a
   * conversion done by hand, and a conversion done nine times is one done
   * wrong at least once. Plan distances stay in feet, where feet are natural.
   */
  _toHeight(meters) {
    return this._units === 'imperial' ? meters / METERS_PER_INCH : meters;
  }

  _fromHeight(value) {
    return this._units === 'imperial' ? value * METERS_PER_INCH : value;
  }

  _heightLabel() {
    return this._units === 'imperial' ? 'in' : 'm';
  }

  /** Derived elevation of a storey's floor surface above the origin.
   *
   * Not measured, and not editable: nobody can put a tape on the height of a
   * second floor above a first. It is the running sum of the ceiling height
   * and subfloor thickness of every storey below, both of which a person can
   * actually measure or look up.
   */
  _derivedElevation(level) {
    const floors = this._floors();
    let z = 0;
    for (const f of floors) {
      if (f.level >= level) break;
      z += (f.ceiling_m || 0) + (f.subfloor_m || 0);
    }
    return Math.round(z * 10000) / 10000;
  }

  /** Recompute every storey's elevation from ceiling + subfloor, and carry
   * anything standing on those storeys along with them.
   *
   * A node's stored z is absolute (height above the ground floor), but the
   * height a person entered was relative to its own storey. If floor 1's
   * ceiling changes, floor 2 moves, and a node 20 inches above floor 2 must
   * stay 20 inches above floor 2 rather than staying at an absolute height
   * that is now inside the ceiling below.
   */
  _reflowElevations() {
    // Read every measured height BEFORE the elevations move, then write them
    // back after. Preserving the measurement is the intent; shifting z by a
    // delta was the mechanism, and it double-applied whenever another writer
    // had already accounted for the same elevation.
    const heights = this.config.nodes.map((n) => this._nodeHeight(n));
    const apLevel = Number.isFinite(this.config.ap_floor) ? this.config.ap_floor : 1;
    const apHeight = Array.isArray(this.config.ap_position)
      ? this.config.ap_position[2] - this._elevationOf(apLevel)
      : null;

    const floors = this._floors();
    floors.forEach((f) => { f.elevation_m = this._derivedElevation(f.level); });
    this.config.floors = floors;

    this.config.nodes.forEach((n, i) => this._setNodeHeight(n, heights[i]));
    if (apHeight !== null) {
      this.config.ap_position[2] =
        Math.round((apHeight + this._elevationOf(apLevel)) * 10000) / 10000;
    }
  }

  async init() {
    this._buildDOM();
    this._wireEvents();
    await this._load();
    // sensingService is a singleton started once globally (app.js); we just
    // subscribe here, same pattern as DashboardTab's live-data subscription.
    this._unsubSensingData = sensingService.onData((data) => this._onSensingData(data));
  }

  // ---- Live position overlay ---------------------------------------------

  _onSensingData(data) {
    const persons = Array.isArray(data.persons) ? data.persons : [];
    if (persons.length === 0) {
      this._liveDot = null;
      this._liveDotSource = null;
      this._liveStatus = 'No person currently detected.';
      const idleStatusEl = this.container.querySelector('#rbLiveStatus');
      if (idleStatusEl) idleStatusEl.textContent = this._liveStatus;
      this._render();
      return;
    }

    // With today's centroid methods, all persons share the one estimate
    // (single-target only for now) — show just the first to avoid clutter
    // from the tracker's occasional duplicate/ghost detections.
    const p = persons[0];
    const nodeCount = Array.isArray(data.nodes) ? data.nodes.length : 0;
    const ROOM_SPACE_SOURCES = {
      bistatic_velocity: 'Live bistatic-geometry estimate — real AP/node Doppler-ellipse math, '
        + 'still an unvalidated first cut (see position_uncertainty_m).',
      doppler_centroid: 'Live Doppler-weighted centroid — a heuristic estimate, not a calibrated fix.',
      motion_centroid: 'Live motion-weighted centroid — a heuristic estimate, not a calibrated fix.',
    };

    if (ROOM_SPACE_SOURCES[p.position_source] && Array.isArray(p.position)) {
      const [x, y] = p.position;
      if (Number.isFinite(x) && Number.isFinite(y)) {
        this._liveDot = { x, y };
        this._liveDotSource = p.position_source;
        this._liveStatus = ROOM_SPACE_SOURCES[p.position_source];
      } else {
        this._liveDot = null;
        this._liveDotSource = null;
        this._liveStatus = `${p.position_source} reported non-finite coordinates — not plotted.`;
      }
    } else {
      // field_peak positions live in the Observatory's own grid-centered
      // coordinate frame, not this room's meters/NW-origin frame — plotting
      // them here would be misleading, so we explain instead of drawing.
      this._liveDot = null;
      this._liveDotSource = null;
      this._liveStatus = `No room-space estimate yet — only ${nodeCount} node(s) reporting, `
        + `or not enough measured motion right now (need >= 2 positioned nodes with real `
        + `disturbance). Showing the Observatory's field-peak fallback instead, which isn't `
        + `in this room's coordinates.`;
    }
    const statusEl = this.container.querySelector('#rbLiveStatus');
    if (statusEl) statusEl.textContent = this._liveStatus;
    this._render();
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
        .rb-wall-entry { display: grid; grid-template-columns: repeat(4, 1fr) auto; gap: 6px; align-items: end; margin-top: 8px; }
        .rb-wall-entry label { display: flex; flex-direction: column; gap: 3px; font-size: 11px; color: #6b7280; }
        .rb-wall-entry input { background: #0d1117; border: 1px solid #2a2f3a; border-radius: 4px; color: #e6e9ef; padding: 5px 6px; font-size: 13px; width: 100%; box-sizing: border-box; }
        .rb-node-row select { background: #0d1117; border: 1px solid #2a2f3a; border-radius: 4px; color: #e6e9ef; padding: 5px 4px; font-size: 13px; width: 100%; box-sizing: border-box; }
        .rb-room-dims input { width: 72px; }
        .rb-room-dims select { background: #0d1117; border: 1px solid #2a2f3a; border-radius: 4px; color: #e6e9ef; padding: 5px 7px; font-size: 13px; }
        .rb-node-row { display: grid; grid-template-columns: 42px 1fr 60px 60px 60px 52px 24px; gap: 6px; align-items: center; margin-bottom: 6px; }
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
        .rb-col-headers { display: grid; grid-template-columns: 42px 1fr 60px 60px 60px 52px 24px; gap: 6px; font-size: 11px; color: #6b7280; margin-bottom: 6px; }
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
          <p class="rb-hint" id="rbLiveStatus" style="margin:8px 2px 0;">Waiting for live sensing data…</p>
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
            <div class="rb-card-title">Storeys &amp; Walls</div>
            <p class="rb-hint" style="margin-top:0;">
              The origin (0,&nbsp;0,&nbsp;0) is the <strong>north-west corner of the
              first floor</strong>, at floor level. Every storey shares it — the
              second floor is not re-zeroed — so a node's X/Y means the same
              thing on any storey and Z is height above the ground floor.
            </p>
            <div id="rbFloorControls"></div>
            <div class="rb-actions" style="margin-top:10px;">
              <button class="rb-btn secondary" id="rbAddFloor">+ Add Storey</button>
              <button class="rb-btn secondary" id="rbRemoveFloor">Remove Top Storey</button>
              <button class="rb-btn secondary" id="rbWallMode">Draw Walls</button>
            </div>
            <p class="rb-hint" id="rbWallHint" style="margin-bottom:6px;">
              In wall mode, drag on the canvas to draw a segment on the selected
              storey. Other storeys stay visible, faintly, so you can line one up
              with the floor below.
            </p>
            <div class="rb-wall-entry">
              <label><span>Start &middot; east &rarr;</span><input type="number" id="rbWallX1" step="0.25" value="0" title="Distance east from the north-west corner"></label>
              <label><span>Start &middot; south &darr;</span><input type="number" id="rbWallY1" step="0.25" value="0" title="Distance south from the north-west corner"></label>
              <label><span>End &middot; east &rarr;</span><input type="number" id="rbWallX2" step="0.25" placeholder="—" title="Distance east from the north-west corner"></label>
              <label><span>End &middot; south &darr;</span><input type="number" id="rbWallY2" step="0.25" placeholder="—" title="Distance south from the north-west corner"></label>
              <button class="rb-btn secondary" id="rbAddWall">Add</button>
            </div>
            <p class="rb-hint" id="rbWallEntryHint" style="margin:4px 2px 8px;">
              Both points are measured from the <strong>north-west corner</strong>, in
              <span class="rb-unit-label">${this._unitLabel()}</span> — east is right,
              south is down, matching the <strong>N</strong> arrow on the canvas. A wall
              along the north edge starting at the corner is
              start&nbsp;0,&nbsp;0 → end&nbsp;12,&nbsp;0.
            </p>
            <div id="rbWallList"></div>
          </div>
          <div class="rb-card">
            <div class="rb-card-title">Building Outline</div>
            <p class="rb-hint" style="margin-top:0;">
              The width and depth above are a <strong>bounding box</strong>. Trace the
              storey's real outline here and the position search stops scoring cells
              that are not part of the house — the notch of an L-shaped plan is the
              garden, not a place a person can stand. Leave it empty and the whole
              box is searched, exactly as before.
            </p>
            <p class="rb-hint" style="margin-top:0;">
              A wing <strong>west or north</strong> of the north-west corner has
              <strong>negative</strong> coordinates. That is expected: the origin is
              pinned to the block your nodes were measured against, so it never moves
              under them. The canvas widens to show whatever you trace.
            </p>
            <div class="rb-actions" style="margin-top:10px;">
              <button class="rb-btn secondary" id="rbFootprintMode">Trace Outline</button>
              <button class="rb-btn secondary" id="rbCloseRing" style="display:none;">Close Outline</button>
              <button class="rb-btn secondary" id="rbUndoPoint" style="display:none;">Undo Point</button>
            </div>
            <div class="rb-wall-entry" style="grid-template-columns:1fr 1fr auto;">
              <label><span>Corner &middot; east &rarr;</span><input type="number" id="rbRingX" step="0.5" placeholder="—" title="Distance east of the north-west corner. Negative for a wing west of it."></label>
              <label><span>Corner &middot; south &darr;</span><input type="number" id="rbRingY" step="0.5" placeholder="—" title="Distance south of the north-west corner. Negative for a wing north of it."></label>
              <button class="rb-btn secondary" id="rbAddCorner">Add Corner</button>
            </div>
            <p class="rb-hint" style="margin:4px 2px 8px;">
              Type a corner when clicking cannot reach it or is not precise enough.
              A wing <strong>west</strong> of the north-west corner has a
              <strong>negative</strong> east value — a 12&nbsp;ft wing runs
              <strong>&minus;12&nbsp;&rarr;&nbsp;0</strong>, not 0&nbsp;&rarr;&nbsp;12.
              Clicking can only reach about 2.6&nbsp;ft west of the corner, so anything
              further out has to be typed.
            </p>
            <p class="rb-hint" id="rbFootprintHint" style="margin-bottom:6px;">
              <strong>Walk the perimeter</strong>, clicking each corner in turn, then Close
              Outline. You do not cut a notch out — you route around it: a rectangle is
              4 clicks, an L-shaped plan is 6. Several outlines may share a storey — a
              wing or a detached garage is its own shape, and a person is indoors if
              they are inside any of them.
            </p>
            <div id="rbFootprintList"></div>
          </div>
          <div class="rb-card">
            <div class="rb-card-title">Access Point</div>
            <p class="rb-hint" style="margin-top:0;">
              Optional — needed for Doppler-based position geometry. One AP,
              same room-space meters as the nodes above (it's often outside
              the room itself — a hallway, another floor — and that's fine).
            </p>
            <div class="rb-room-dims" id="rbApFields" style="${this.config.ap_position ? '' : 'display:none;'}">
              <label><span>X (<span class="rb-unit-label">${this._unitLabel()}</span>)</span> <input type="number" id="rbApX" step="0.05" value="${this._toDisplay(this.config.ap_position?.[0] ?? 0).toFixed(2)}"></label>
              <label><span>Y (<span class="rb-unit-label">${this._unitLabel()}</span>)</span> <input type="number" id="rbApY" step="0.05" value="${this._toDisplay(this.config.ap_position?.[1] ?? 0).toFixed(2)}"></label>
              <label><span>Z (<span class="rb-hunit-label">${this._heightLabel()}</span>)</span> <input type="number" id="rbApZ" step="0.5" title="Height above the AP's own floor" value="${this._toHeight(this.config.ap_position?.[2] ?? 0).toFixed(1)}"></label>
              <label><span>Floor</span> <select id="rbApFloor"></select></label>
            </div>
            <div class="rb-actions" style="margin-top:${this.config.ap_position ? '10px' : '0'};">
              <button class="rb-btn secondary" id="rbAddAp" style="${this.config.ap_position ? 'display:none;' : ''}">+ Set AP Position</button>
              <button class="rb-btn secondary" id="rbRemoveAp" style="${this.config.ap_position ? '' : 'display:none;'}">Remove AP</button>
            </div>
          </div>
          <div class="rb-card">
            <div class="rb-card-title">Sensor Nodes</div>
            <div class="rb-col-headers">
              <span>ID</span><span>Label</span><span>X (<span class="rb-unit-label">${this._unitLabel()}</span>)</span><span>Y (<span class="rb-unit-label">${this._unitLabel()}</span>)</span><span>Z (<span class="rb-hunit-label">${this._heightLabel()}</span>)</span><span>Floor</span><span></span>
            </div>
            <div id="rbNodeList"></div>
            <div class="rb-actions">
              <button class="rb-btn secondary" id="rbAddNode">+ Add Node</button>
            </div>
          </div>
          <div class="rb-actions">
            <button class="rb-btn secondary" id="rbCalibrate"
                    title="Clear every node's ambient floor and re-learn it from the room as it is now">
              Recalibrate All Nodes
            </button>
          </div>
          <p class="rb-hint">
            Use after moving boards or rearranging a room. Nodes adapt on their own,
            but they climb back down to a lower floor slowly on purpose — so a person
            standing still is never mistaken for the new normal. This skips the wait.
            <strong>Leave the area first:</strong> each node re-learns from roughly the
            next 30&nbsp;seconds, so whatever is moving then becomes its idea of quiet.
          </p>
          <div class="rb-actions">
            <button class="rb-btn" id="rbSave">Save</button>
            <button class="rb-btn secondary" id="rbReload" title="Discard unsaved changes and reload the last saved config">Reload from Saved</button>
          </div>
          <p class="rb-hint">
            Drag a node — or the AP marker (violet diamond), once set — on the
            canvas to reposition it (X/Y only — set height with the Z field).
            Coordinates are in the same room-space meters used by
            <code>--node-positions</code>.
          </p>
          <p class="rb-hint">
            <strong>(0, 0)</strong> is the room's <strong>Northwest</strong> corner
            (the <strong>N</strong> arrow on the canvas points that way) — X increases
            going <strong>East</strong>, Y increases going <strong>South</strong>. Face
            the room's actual north wall and match it up when you place nodes
            and the access point.
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
    this.container.querySelector('#rbAddAp').addEventListener('click', () => {
      // Default to head height on the storey currently being edited, rather
      // than an absolute 2.5 m that would be inside the ceiling upstairs.
      this.config.ap_floor = this._activeFloor;
      this.config.ap_position = [
        this.config.width_m / 2,
        this.config.depth_m / 2,
        Math.round((this._elevationOf(this._activeFloor) + 2.2) * 10000) / 10000,
      ];
      this._renderApFields();
      this._render();
    });
    // Live preview: typing coordinates is only unclear until you can see what
    // they produce. The dashed line uses the same path as a dragged wall.
    const wallInputs = ['rbWallX1', 'rbWallY1', 'rbWallX2', 'rbWallY2'];
    const previewWall = () => {
      const raw = wallInputs.map((id) => this.container.querySelector(`#${id}`).value);
      if (raw.some((v) => v === '' || v === null)) {
        this._wallStart = null;
        this._wallPreview = null;
        this._updateWallEntryHint(null);
        this._render();
        return;
      }
      const [x1, y1, x2, y2] = raw.map((v) => this._fromDisplay(parseFloat(v) || 0));
      this._wallStart = { x: x1, y: y1 };
      const end = this._toPixel(x2, y2);
      this._wallPreview = { x: end.px, y: end.py };
      this._updateWallEntryHint(Math.hypot(x2 - x1, y2 - y1));
      this._render();
    };
    wallInputs.forEach((id) => {
      this.container.querySelector(`#${id}`).addEventListener('input', previewWall);
    });

    this.container.querySelector('#rbAddWall').addEventListener('click', () => {
      const num = (id) => parseFloat(this.container.querySelector(`#${id}`).value);
      // An empty end box used to fall through `|| 0` and draw a wall to the
      // north-west corner, which is a real coordinate and so looked deliberate.
      const missing = ['rbWallX2', 'rbWallY2'].some(
        (id) => !Number.isFinite(num(id))
      );
      if (missing) {
        toastManager.error('Fill in both end coordinates before adding the wall.');
        return;
      }
      const x1 = this._fromDisplay(num('rbWallX1') || 0);
      const y1 = this._fromDisplay(num('rbWallY1') || 0);
      const x2 = this._fromDisplay(num('rbWallX2'));
      const y2 = this._fromDisplay(num('rbWallY2'));
      if (Math.hypot(x2 - x1, y2 - y1) < 0.1) {
        toastManager.error('Start and end are the same point — a wall needs length.');
        return;
      }
      if (!Array.isArray(this.config.walls)) this.config.walls = [];
      const r = (v) => Math.round(v * 10000) / 10000;
      this.config.walls.push({
        level: this._activeFloor,
        x1: r(x1), y1: r(y1), x2: r(x2), y2: r(y2),
      });
      // Chain from the end point -- walls in a room almost always meet -- but
      // CLEAR the end. Copying both left all four boxes showing the same
      // number and staged a zero-length wall, which looked broken.
      this.container.querySelector('#rbWallX1').value = num('rbWallX2');
      this.container.querySelector('#rbWallY1').value = num('rbWallY2');
      this.container.querySelector('#rbWallX2').value = '';
      this.container.querySelector('#rbWallY2').value = '';
      this._wallStart = null;
      this._wallPreview = null;
      this._updateWallEntryHint(null);
      this._renderWallList();
      this._renderFloorControls();
      this._render();
    });
    this.container.querySelector('#rbAddFloor').addEventListener('click', () => {
      this._addFloor();
    });
    this.container.querySelector('#rbRemoveFloor').addEventListener('click', () => {
      this._removeTopFloor();
    });
    this.container.querySelector('#rbWallMode').addEventListener('click', (e) => {
      this._mode = this._mode === 'wall' ? 'select' : 'wall';
      // Abandon a half-drawn segment rather than leaving it to commit on the
      // next unrelated click. The same goes for a half-traced outline: the
      // two drawing modes are exclusive, so entering one must not leave the
      // other's unfinished shape armed.
      this._wallStart = null;
      this._wallPreview = null;
      this._ringDraft = null;
      this._ringHover = null;
      this._renderFootprintControls();
      e.target.textContent = this._mode === 'wall' ? 'Done Drawing' : 'Draw Walls';
      const canvas = this.container.querySelector('#rbCanvas');
      if (canvas) canvas.style.cursor = this._mode === 'wall' ? 'crosshair' : 'default';
      const hint = this.container.querySelector('#rbWallHint');
      if (hint) {
        hint.textContent = this._mode === 'wall'
          ? 'Drag on the canvas to draw a wall on the selected storey. Dragging no longer moves nodes.'
          : 'In wall mode, drag on the canvas to draw a segment on the selected storey. Other storeys stay visible, faintly, so you can line one up with the floor below.';
      }
      this._render();
    });
    this.container.querySelector('#rbFootprintMode').addEventListener('click', () => {
      if (this._mode === 'footprint') {
        // Leaving the mode abandons a half-traced ring rather than leaving it
        // to be closed by some later, unrelated click.
        this._ringDraft = null;
        this._ringHover = null;
        this._mode = 'select';
      } else {
        this._mode = 'footprint';
        this._ringDraft = [];
        // A wall being dragged out would otherwise commit on the next mouseup.
        this._wallStart = null;
        this._wallPreview = null;
      }
      this._renderFootprintControls();
      this._render();
    });
    this.container.querySelector('#rbCloseRing').addEventListener('click', () => {
      this._closeRing();
    });
    this.container.querySelector('#rbAddCorner').addEventListener('click', () => {
      const num = (id) => parseFloat(this.container.querySelector(`#${id}`).value);
      const [dx, dy] = [num('rbRingX'), num('rbRingY')];
      if (!Number.isFinite(dx) || !Number.isFinite(dy)) {
        toastManager.error('Fill in both corner coordinates before adding.');
        return;
      }
      // Typing a corner IS tracing, so switch into the mode rather than
      // silently dropping the point because a button was not pressed first.
      if (this._mode !== 'footprint') {
        this._mode = 'footprint';
        this._wallStart = null;
        this._wallPreview = null;
      }
      if (!Array.isArray(this._ringDraft)) this._ringDraft = [];
      const r = (v) => Math.round(v * 10000) / 10000;
      this._ringDraft.push({ x: r(this._fromDisplay(dx)), y: r(this._fromDisplay(dy)) });
      // Clear rather than chain: unlike a wall, whose next segment starts
      // where the last ended, the next corner of an outline shares no
      // coordinate with the previous one, so leaving the numbers in place
      // would only stage a duplicate point.
      this.container.querySelector('#rbRingX').value = '';
      this.container.querySelector('#rbRingY').value = '';
      this._renderFootprintControls();
      this._render();
    });
    this.container.querySelector('#rbUndoPoint').addEventListener('click', () => {
      if (this._ringDraft && this._ringDraft.length) this._ringDraft.pop();
      this._renderFootprintControls();
      this._render();
    });
    this.container.querySelector('#rbRemoveAp').addEventListener('click', () => {
      this.config.ap_position = null;
      this._renderApFields();
      this._render();
    });
    ['rbApX', 'rbApY', 'rbApZ'].forEach((id) => {
      this.container.querySelector(`#${id}`).addEventListener('input', () => {
        this._syncApFromInputs();
        this._render();
      });
    });
    this.container.querySelector('#rbCalibrate').addEventListener('click', async (e) => {
      const btn = e.target;
      btn.disabled = true;
      const was = btn.textContent;
      btn.textContent = 'Recalibrating…';
      try {
        const r = await apiService.post('/api/v1/calibrate', {});
        if (r && r.error) {
          toastManager.error(`Recalibrate failed: ${r.error}`);
        } else {
          const okList = (r && r.recalibrated) || [];
          const bad = (r && r.failed) || [];
          // Name the nodes that did not take it. "Partial success" with no
          // list means walking the house to work out which board to look at.
          if (bad.length) {
            toastManager.error(
              `Recalibrated ${okList.length}; failed on node(s) ${bad.map((f) => f.node).join(', ')}`
            );
          } else {
            toastManager.success(
              `Recalibrating ${okList.length} node(s) — they re-learn over about 30 seconds.`
            );
          }
        }
      } catch (err) {
        toastManager.error(`Recalibrate failed: ${err.message}`);
      } finally {
        btn.disabled = false;
        btn.textContent = was;
      }
    });
    this.container.querySelector('#rbSave').addEventListener('click', () => {
      this._save();
    });
    this.container.querySelector('#rbReload').addEventListener('click', async () => {
      await this._load();
      toastManager.info('Reloaded from the last saved config — unsaved changes discarded.');
    });

    const canvas = this.container.querySelector('#rbCanvas');
    canvas.addEventListener('mousedown', (e) => this._onCanvasDown(e));
    canvas.addEventListener('mousemove', (e) => this._onCanvasMove(e));
    window.addEventListener('mouseup', (e) => {
      if (this._mode === 'wall' && this._wallStart) {
        this._commitWall(e);
      }
      this._dragIndex = null;
      this._draggingAp = false;
    });
  }

  // ---- Data -----------------------------------------------------------------

  /** Storeys, ascending. An empty list means a single implicit ground floor,
   * which is how every config written before storeys existed behaves. */
  _floors() {
    if (!Array.isArray(this.config.floors) || this.config.floors.length === 0) {
      return [{ level: 1, name: 'First', elevation_m: 0, ceiling_m: DEFAULT_CEILING_M, subfloor_m: DEFAULT_SUBFLOOR_M }];
    }
    return [...this.config.floors].sort((a, b) => a.level - b.level);
  }

  _floorOf(node) {
    return Number.isFinite(node.floor) ? node.floor : 1;
  }

  /** A node's height above its OWN storey — the number a person measured. */
  _nodeHeight(node) {
    return node.z - this._elevationOf(this._floorOf(node));
  }

  /** The ONLY place node.z is written.
   *
   * z is stored absolute (above the ground floor) because geometry needs one
   * axis, but every height in this form is entered relative to its own storey.
   * That conversion used to happen in three places — the input sync, the
   * storey selector, and the elevation reflow — each adding the storey
   * elevation independently. Any two firing for one edit added it twice, which
   * is how nodes ended up at 122 and 246 inches above their own ceilings.
   *
   * Now there is one writer and one reader, and both go through the same
   * elevation lookup, so the round trip is idempotent no matter how many times
   * it runs. */
  _setNodeHeight(node, relMeters) {
    if (!Number.isFinite(relMeters)) return;
    node.z = Math.round((relMeters + this._elevationOf(this._floorOf(node))) * 10000) / 10000;
  }

  _elevationOf(level) {
    const f = this._floors().find((x) => x.level === level);
    return f ? f.elevation_m : 0;
  }

  async _load() {
    try {
      const data = await apiService.get(ENDPOINT);
      if (data && typeof data === 'object') {
        // Spread first, then override only the fields that need defaulting.
        //
        // This was an explicit allowlist of four fields, which silently
        // discarded everything the server sent that was not on it — floors,
        // walls and ap_floor were all dropped on load. The visible symptom was
        // storeys vanishing from the picker after a refresh, but the real
        // danger was the next Save: config no longer held them, so it would
        // POST an empty list and destroy the storeys on disk for real.
        //
        // Spreading means a field added to the server is carried through
        // rather than quietly deleted by a UI that predates it.
        this.config = {
          ...data,
          width_m: data.width_m > 0 ? data.width_m : 5,
          depth_m: data.depth_m > 0 ? data.depth_m : 4,
          nodes: Array.isArray(data.nodes) ? data.nodes : [],
          ap_position: Array.isArray(data.ap_position) ? data.ap_position : null,
          floors: Array.isArray(data.floors) ? data.floors : [],
          walls: Array.isArray(data.walls) ? data.walls : [],
          footprint: Array.isArray(data.footprint) ? data.footprint : [],
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
  /** Re-render every panel that depends on the loaded config. Called from the
   * same places as _renderNodeList, so storeys and walls cannot drift out of
   * sync with what was loaded or saved. */
  _renderStoreyPanels() {
    this._renderFloorControls();
    this._renderWallList();
    this._renderFootprintControls();
  }

  _refreshUnitDisplay() {
    const label = this._unitLabel();
    this.container.querySelectorAll('.rb-unit-label').forEach((el) => {
      el.textContent = label;
    });
    // Heights use their own unit (inches when imperial), so they have their
    // own label class. Missing this leaves "ft" beside a field holding inches.
    const hLabel = this._heightLabel();
    this.container.querySelectorAll('.rb-hunit-label').forEach((el) => {
      el.textContent = hLabel;
    });
    this.container.querySelector('#rbWidth').value = this._toDisplay(this.config.width_m).toFixed(2);
    this.container.querySelector('#rbDepth').value = this._toDisplay(this.config.depth_m).toFixed(2);
    this._renderNodeList();
    this._renderStoreyPanels();
    this._renderApFields();
    this._render();
  }

  /** Show/hide the AP X/Y/Z fields and Add/Remove buttons based on whether
   * ap_position is set, and refresh the input values from this.config. */
  _renderApFields() {
    const fields = this.container.querySelector('#rbApFields');
    const addBtn = this.container.querySelector('#rbAddAp');
    const removeBtn = this.container.querySelector('#rbRemoveAp');
    const has = Array.isArray(this.config.ap_position);
    fields.style.display = has ? '' : 'none';
    addBtn.style.display = has ? 'none' : '';
    removeBtn.style.display = has ? '' : 'none';
    if (has) {
      const [x, y, z] = this.config.ap_position;
      const lvl = Number.isFinite(this.config.ap_floor) ? this.config.ap_floor : 1;
      this.container.querySelector('#rbApX').value = this._toDisplay(x).toFixed(2);
      this.container.querySelector('#rbApY').value = this._toDisplay(y).toFixed(2);
      // Shown as height above the AP's own storey, like every other height in
      // this form. Storage stays absolute.
      this.container.querySelector('#rbApZ').value =
        this._toHeight(z - this._elevationOf(lvl)).toFixed(1);

      const sel = this.container.querySelector('#rbApFloor');
      if (sel) {
        sel.innerHTML = this._floors()
          .map((f) => `<option value="${f.level}" ${f.level === lvl ? 'selected' : ''}>${f.level}</option>`)
          .join('');
        if (!sel.dataset.wired) {
          sel.dataset.wired = '1';
          sel.addEventListener('change', () => {
            // Keep the measured height: moving the AP upstairs moves its
            // absolute z by the difference in storey elevations.
            const prev = this._elevationOf(
              Number.isFinite(this.config.ap_floor) ? this.config.ap_floor : 1
            );
            const next = this._elevationOf(Number(sel.value));
            this.config.ap_floor = Number(sel.value);
            if (Array.isArray(this.config.ap_position)) {
              this.config.ap_position[2] =
                Math.round((this.config.ap_position[2] - prev + next) * 10000) / 10000;
            }
            this._renderApFields();
            this._render();
          });
        }
      }
    }
  }

  /** Pull edited values out of the AP X/Y/Z inputs back into
   * this.config.ap_position, same pattern as _syncNodesFromInputs. */
  _syncApFromInputs() {
    if (!Array.isArray(this.config.ap_position)) return;
    const x = this._fromDisplay(parseFloat(this.container.querySelector('#rbApX').value) || 0);
    const y = this._fromDisplay(parseFloat(this.container.querySelector('#rbApY').value) || 0);
    const lvl = Number.isFinite(this.config.ap_floor) ? this.config.ap_floor : 1;
    const relZ = this._fromHeight(parseFloat(this.container.querySelector('#rbApZ').value) || 0);
    const z = Math.round((relZ + this._elevationOf(lvl)) * 10000) / 10000;
    this.config.ap_position = [x, y, z];
  }

  async _save() {
    // Pull the latest values out of the node-row/AP inputs before sending -
    // numeric fields don't write back into this.config until blur/save.
    this._syncNodesFromInputs();
    this._syncApFromInputs();
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
    this._renderStoreyPanels();
    this._render();
  }

  _removeNode(index) {
    this.config.nodes.splice(index, 1);
    this._renderNodeList();
    this._renderStoreyPanels();
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
      // The field holds height above this node's own storey; _setNodeHeight
      // is the single place that turns that into absolute z.
      this._setNodeHeight(node, this._fromHeight(parseFloat(row.querySelector('.rb-z').value) || 0));
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
        <input class="rb-z" type="number" step="0.5" title="Height above this node's own floor"
               value="${this._toHeight(safe(node.z) - this._elevationOf(this._floorOf(node))).toFixed(1)}">
        <select class="rb-floor" title="Which storey this node is mounted on">
          ${this._floors().map((f) => `<option value="${f.level}" ${this._floorOf(node) === f.level ? 'selected' : ''}>${f.level}</option>`).join('')}
        </select>
        <button class="rb-remove-btn" title="Remove node">&times;</button>
      `;
      const floorSel = row.querySelector('.rb-floor');
      if (floorSel) {
        floorSel.addEventListener('change', () => {
          // Store the storey, and lift z onto it so the node does not stay at
          // ground-floor height after being moved upstairs. z is measured from
          // the FIRST floor, so moving between storeys has to move z too or the
          // node silently ends up inside the ceiling below.
          // Keep the height the person measured: read it against the OLD
          // storey, change storey, write it back against the NEW one. The
          // elevation is applied by _setNodeHeight and nowhere else.
          const rel = this._nodeHeight(node);
          node.floor = Number(floorSel.value);
          this._setNodeHeight(node, rel);
          this._renderNodeList();
          this._render();
        });
      }
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

  /** The extent the canvas must show, in room coordinates.
   *
   * Deliberately not just `0..width_m x 0..depth_m`. The origin is pinned to
   * the north-west corner of the building's MAIN BLOCK, because that is what
   * every node position was measured against — so a wing to the west or north
   * of it has negative coordinates. A viewport that always started at (0, 0)
   * would render that wing off the canvas entirely: invisible, undraggable,
   * and impossible to trace an outline around.
   *
   * The room rectangle is always included even when a footprint extends past
   * it, so the box the nodes were placed in never scrolls out of view.
   */
  _view() {
    let minX = 0;
    let minY = 0;
    let maxX = this.config.width_m;
    let maxY = this.config.depth_m;
    const consider = (x, y) => {
      if (!Number.isFinite(x) || !Number.isFinite(y)) return;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    };
    (this.config.footprint || []).forEach((ring) => {
      (ring.points || []).forEach((p) => consider(p[0], p[1]));
    });
    (this._ringDraft || []).forEach((p) => consider(p.x, p.y));

    const spanX = maxX - minX;
    const spanY = maxY - minY;
    // A zero or non-finite span would make every coordinate NaN and blank the
    // canvas. Fall back to a scale of 1 px/m, which draws something wrong
    // rather than nothing at all.
    const scale =
      spanX > 0 && spanY > 0
        ? Math.min((CANVAS_W - 2 * MARGIN) / spanX, (CANVAS_H - 2 * MARGIN) / spanY)
        : 1;
    return { minX, minY, maxX, maxY, scale };
  }

  /** Room-space (x,y) meters -> canvas pixel coordinates. */
  _toPixel(x, y) {
    const { minX, minY, scale } = this._view();
    return { px: MARGIN + (x - minX) * scale, py: MARGIN + (y - minY) * scale, scale };
  }

  /** Canvas pixel coordinates -> room-space (x,y) meters. Clamped to the
   * visible extent by default (sensor nodes always live inside the building);
   * pass `clamp: false` for the AP marker, which is legitimately often outside
   * it, and for footprint and wall vertices, which trace its boundary. */
  _toRoom(px, py, { clamp = true } = {}) {
    const { minX, minY, maxX, maxY, scale } = this._view();
    let x = minX + (px - MARGIN) / scale;
    let y = minY + (py - MARGIN) / scale;
    if (clamp) {
      x = Math.min(maxX, Math.max(minX, x));
      y = Math.min(maxY, Math.max(minY, y));
    }
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

    if (this._mode === 'footprint') {
      // Not clamped: the outline traces the building's boundary, and a wing
      // west of the origin is legitimately at negative x.
      const room = this._toRoom(pos.x, pos.y, { clamp: false });
      if (!this._ringDraft) this._ringDraft = [];
      const pt = { x: Math.round(room.x * 100) / 100, y: Math.round(room.y * 100) / 100 };
      // Clicking the first vertex again is the usual way to close a polygon,
      // so honour it rather than adding a duplicate point on top of it.
      const first = this._ringDraft[0];
      if (first && this._ringDraft.length >= 3) {
        const { scale } = this._view();
        if (Math.hypot((pt.x - first.x) * scale, (pt.y - first.y) * scale) <= WALL_HIT_PX) {
          this._closeRing();
          return;
        }
      }
      this._ringDraft.push(pt);
      this._renderFootprintControls();
      this._render();
      return;
    }
    if (this._mode === 'wall') {
      // Walls are not clamped to the room rectangle. width_m/depth_m describe
      // the sensed area, and an exterior wall is legitimately on or slightly
      // outside that boundary.
      const room = this._toRoom(pos.x, pos.y, { clamp: false });
      this._wallStart = { x: room.x, y: room.y };
      this._wallPreview = { x: pos.x, y: pos.y };
      return;
    }
    if (Array.isArray(this.config.ap_position)) {
      const { px, py } = this._toPixel(this.config.ap_position[0], this.config.ap_position[1]);
      if (Math.hypot(px - pos.x, py - pos.y) <= AP_RADIUS + 4) {
        this._draggingAp = true;
        return;
      }
    }
    this.config.nodes.forEach((node, idx) => {
      const { px, py } = this._toPixel(node.x, node.y);
      if (this._dragIndex == null && Math.hypot(px - pos.x, py - pos.y) <= NODE_RADIUS + 4) {
        this._dragIndex = idx;
      }
    });
  }

  _onCanvasMove(e) {
    if (this._mode === 'footprint') {
      if (!this._ringDraft || !this._ringDraft.length) return;
      const pos = this._canvasPos(e);
      this._ringHover = { x: pos.x, y: pos.y };
      this._render();
      return;
    }
    if (this._mode === 'wall' && this._wallStart) {
      const pos = this._canvasPos(e);
      this._wallPreview = { x: pos.x, y: pos.y };
      this._render();
      return;
    }
    if (this._draggingAp) {
      const pos = this._canvasPos(e);
      const room = this._toRoom(pos.x, pos.y, { clamp: false });
      this.config.ap_position[0] = Math.round(room.x * 100) / 100;
      this.config.ap_position[1] = Math.round(room.y * 100) / 100;
      this._renderApFields();
      this._render();
      return;
    }
    if (this._dragIndex == null) return;
    const node = this.config.nodes[this._dragIndex];
    if (!node) return;
    const pos = this._canvasPos(e);
    const room = this._toRoom(pos.x, pos.y);
    node.x = Math.round(room.x * 100) / 100;
    node.y = Math.round(room.y * 100) / 100;
    this._renderNodeList();
    this._renderStoreyPanels();
    this._render();
  }

  /** Storey selector, plus the two numbers a person can actually measure.
   *
   * Ceiling height and subfloor thickness are the inputs; elevation is
   * derived from them and shown read-only. Nobody can put a tape measure on
   * "height of the second floor above the first", but anyone can measure a
   * ceiling and look up a joist depth -- and deriving it means every height
   * elsewhere in this form is relative to the floor you are standing on.
   */
  _renderFloorControls() {
    const host = this.container.querySelector('#rbFloorControls');
    if (!host) return;
    const floors = this._floors();
    const active = floors.find((f) => f.level === this._activeFloor) || floors[0];
    this._activeFloor = active.level;
    const hu = this._heightLabel();

    const opts = floors
      .map((f) => {
        const nodes = this.config.nodes.filter((n) => this._floorOf(n) === f.level).length;
        const walls = (this.config.walls || []).filter((w) => w.level === f.level).length;
        const sel = f.level === this._activeFloor ? 'selected' : '';
        const nm = f.name || `Floor ${f.level}`;
        return `<option value="${f.level}" ${sel}>${nm} — ${nodes} node(s), ${walls} wall(s)</option>`;
      })
      .join('');

    const elev = this._derivedElevation(active.level);
    host.innerHTML = `
      <div class="rb-room-dims">
        <label><span>Editing storey</span>
          <select id="rbFloorSelect">${opts}</select>
        </label>
        <label><span>Floor to ceiling (<span class="rb-hunit-label">${hu}</span>)</span>
          <input type="number" id="rbFloorCeil" min="1" step="0.5"
                 value="${this._toHeight(active.ceiling_m).toFixed(1)}"
                 title="Measured floor surface to ceiling on this storey"></label>
        <label><span>Subfloor / joists (<span class="rb-hunit-label">${hu}</span>)</span>
          <input type="number" id="rbFloorSub" min="0" step="0.5"
                 value="${this._toHeight(active.subfloor_m || 0).toFixed(1)}"
                 title="Structure between this ceiling and the floor above"></label>
      </div>
      <p class="rb-hint" style="margin:6px 2px 0;">
        This storey's floor sits <strong>${this._toHeight(elev).toFixed(1)} ${hu}</strong>
        above the ground floor${active.level === 1 ? ' (it defines the origin)' : ''}.
        Heights you enter below are measured from <em>this</em> floor, so an outlet
        20&nbsp;${hu} up is just 20.
      </p>`;

    const sel = host.querySelector('#rbFloorSelect');
    if (sel) sel.addEventListener('change', () => {
      this._activeFloor = Number(sel.value);
      this._renderFloorControls();
      this._renderNodeList();
      this._renderWallList();
      this._render();
    });

    const commit = () => {
      const f = this._floors().find((x) => x.level === this._activeFloor);
      if (!f) return;
      const c = parseFloat(host.querySelector('#rbFloorCeil').value);
      const sub = parseFloat(host.querySelector('#rbFloorSub').value);
      if (Number.isFinite(c) && c > 0) f.ceiling_m = this._fromHeight(c);
      if (Number.isFinite(sub) && sub >= 0) f.subfloor_m = this._fromHeight(sub);
      this.config.floors = this._floors();
      // Changing a ceiling moves every storey above it, and everything on them.
      this._reflowElevations();
      this._renderFloorControls();
      this._renderNodeList();
      this._renderApFields();
      this._render();
    };
    ['#rbFloorCeil', '#rbFloorSub'].forEach((id) => {
      const el = host.querySelector(id);
      if (el) el.addEventListener('change', commit);
    });
  }

  /** Show the length of the wall currently being typed, or the how-to text. */
  _updateWallEntryHint(lengthMeters) {
    const el = this.container.querySelector('#rbWallEntryHint');
    if (!el) return;
    if (lengthMeters == null) {
      el.innerHTML = `Both points are measured from the <strong>north-west corner</strong>, in
        <span class="rb-unit-label">${this._unitLabel()}</span> — east is right, south is
        down, matching the <strong>N</strong> arrow on the canvas. A wall along the north
        edge starting at the corner is start&nbsp;0,&nbsp;0 → end&nbsp;12,&nbsp;0.`;
      return;
    }
    const u = this._unitLabel();
    el.innerHTML = lengthMeters < 0.1
      ? '<span style="color:#e05561;">Start and end are the same point.</span>'
      : `This wall is <strong>${this._toDisplay(lengthMeters).toFixed(1)} ${u}</strong> long — shown dashed on the canvas.`;
  }

  /** Walls on the active storey, each removable. */
  _renderWallList() {
    const host = this.container.querySelector('#rbWallList');
    if (!host) return;
    const walls = this.config.walls || [];
    const mine = walls
      .map((w, i) => ({ w, i }))
      .filter(({ w }) => w.level === this._activeFloor);

    if (mine.length === 0) {
      host.innerHTML = '<p class="rb-hint" style="margin:4px 2px;">No walls on this storey yet.</p>';
      return;
    }
    const u = this._unitLabel();
    host.innerHTML = mine
      .map(({ w, i }) => {
        const len = Math.hypot(w.x2 - w.x1, w.y2 - w.y1);
        return `<div class="rb-hint" style="display:flex;align-items:center;gap:8px;margin:3px 2px;">
          <span style="flex:1;font-family:monospace;">
            (${this._toDisplay(w.x1).toFixed(1)}, ${this._toDisplay(w.y1).toFixed(1)})
            → (${this._toDisplay(w.x2).toFixed(1)}, ${this._toDisplay(w.y2).toFixed(1)})
            · ${this._toDisplay(len).toFixed(1)} ${u}
          </span>
          <button class="rb-btn secondary" data-wall="${i}" style="padding:2px 8px;">✕</button>
        </div>`;
      })
      .join('');

    host.querySelectorAll('button[data-wall]').forEach((b) => {
      b.addEventListener('click', () => {
        const idx = Number(b.getAttribute('data-wall'));
        this.config.walls.splice(idx, 1);
        this._renderWallList();
        this._renderFloorControls();
        this._render();
      });
    });
  }

  /** Finish the wall being dragged, if it is long enough to be a wall.
   *
   * The server rejects zero-length segments (they make any
   * line-intersection test degenerate rather than merely wrong), so a stray
   * click is dropped here rather than sent and bounced. 10 cm is below any
   * real wall and above any accidental twitch.
   */
  _commitWall(e) {
    const start = this._wallStart;
    this._wallStart = null;
    this._wallPreview = null;
    if (!start) return;

    const pos = this._canvasPos(e);
    const end = this._toRoom(pos.x, pos.y, { clamp: false });
    const len = Math.hypot(end.x - start.x, end.y - start.y);
    if (!Number.isFinite(len) || len < 0.1) {
      this._render();
      return;
    }

    const round = (v) => Math.round(v * 100) / 100;
    if (!Array.isArray(this.config.walls)) this.config.walls = [];
    this.config.walls.push({
      level: this._activeFloor,
      x1: round(start.x), y1: round(start.y),
      x2: round(end.x), y2: round(end.y),
    });
    this._renderWallList();
    this._render();
  }

  // ---- Footprint --------------------------------------------------------------

  /** Commit the traced outline to the active storey.
   *
   * Refuses anything under three vertices. Two points are a line and one is a
   * dot; neither encloses anything, and the server's point-in-polygon test
   * reports every cell OUTSIDE such a ring — so saving one would mask the
   * whole search grid away and stop position output with no error anywhere.
   * The server rejects it too; failing here means the user finds out while
   * they are still looking at the shape.
   */
  _closeRing() {
    const draft = this._ringDraft || [];
    if (draft.length < 3) {
      toastManager.error('An outline needs at least three corners before it can be closed.');
      return;
    }
    if (!Array.isArray(this.config.footprint)) this.config.footprint = [];
    this.config.footprint.push({
      level: this._activeFloor,
      points: draft.map((p) => [p.x, p.y]),
    });
    // Stay in the mode: a house with a wing needs a second outline, and
    // dropping out after every ring would make that needlessly fiddly.
    this._ringDraft = [];
    this._ringHover = null;
    this._renderFootprintControls();
    this._render();
  }

  /** Button labels and the per-storey outline list. */
  _renderFootprintControls() {
    const tracing = this._mode === 'footprint';
    const draft = this._ringDraft || [];
    const modeBtn = this.container.querySelector('#rbFootprintMode');
    if (modeBtn) modeBtn.textContent = tracing ? 'Done Tracing' : 'Trace Outline';
    const closeBtn = this.container.querySelector('#rbCloseRing');
    if (closeBtn) {
      closeBtn.style.display = tracing ? '' : 'none';
      closeBtn.disabled = draft.length < 3;
    }
    const undoBtn = this.container.querySelector('#rbUndoPoint');
    if (undoBtn) {
      undoBtn.style.display = tracing ? '' : 'none';
      undoBtn.disabled = draft.length === 0;
    }
    const canvas = this.container.querySelector('#rbCanvas');
    if (canvas && tracing) canvas.style.cursor = 'crosshair';
    else if (canvas && this._mode === 'select') canvas.style.cursor = 'default';

    const hint = this.container.querySelector('#rbFootprintHint');
    if (hint) {
      hint.textContent = tracing
        ? `Tracing on ${this._floorName(this._activeFloor)} — ${draft.length} corner(s) placed. `
          + 'Click the first corner again, or Close Outline, to finish.'
        : 'Click each corner in turn, then Close Outline. Several outlines may share '
          + 'a storey — a wing or a detached garage is its own shape, and a person is '
          + 'indoors if they are inside any of them.';
    }
    this._renderFootprintList();
  }

  _floorName(level) {
    const f = this._floors().find((x) => x.level === level);
    return (f && f.name) || `Floor ${level}`;
  }

  _renderFootprintList() {
    const host = this.container.querySelector('#rbFootprintList');
    if (!host) return;
    const rings = this.config.footprint || [];
    const draft = this._ringDraft || [];

    // The corners placed so far, with their numbers. A traced shape that is
    // 12 ft off reads as obviously wrong here long before it does on a canvas
    // where the whole plan simply rescales to fit whatever was drawn.
    const u = this._unitLabel();
    const draftHtml = draft.length
      ? `<p class="rb-hint" style="margin:6px 2px 2px;">Corners so far (${u}):</p>
         <p class="rb-hint" style="margin:0 2px 6px; color:#ffd166;">`
        + draft
          .map((p, i) => `${i + 1}: ${this._toDisplay(p.x).toFixed(1)}, ${this._toDisplay(p.y).toFixed(1)}`)
          .join(' &nbsp;·&nbsp; ')
        + '</p>'
      : '';

    if (!rings.length) {
      host.innerHTML = draftHtml
        + '<p class="rb-hint" style="margin:6px 2px 0;">No outline saved yet — until one '
        + 'is closed, the whole width &times; depth box counts as building.</p>';
      return;
    }
    host.innerHTML = draftHtml + rings
      .map((ring, idx) => {
        const active = ring.level === this._activeFloor;
        const pts = (ring.points || []).length;
        return `<div class="rb-wall-entry" style="grid-template-columns:1fr auto; opacity:${active ? 1 : 0.55};">
            <span style="font-size:12px;">${this._floorName(ring.level)} — ${pts} corner(s)${active ? '' : ' (other storey)'}</span>
            <button class="rb-btn secondary rb-ring-remove" data-index="${idx}">Remove</button>
          </div>`;
      })
      .join('');
    host.querySelectorAll('.rb-ring-remove').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.config.footprint.splice(Number(btn.dataset.index), 1);
        this._renderFootprintControls();
        this._render();
      });
    });
  }

  /** Add a storey above the highest one. */
  _addFloor() {
    const floors = this._floors();
    const top = floors[floors.length - 1];
    const next = {
      level: top.level + 1,
      name: `Floor ${top.level + 1}`,
      // Placeholder; _reflowElevations derives the real value from the
      // ceiling and subfloor of every storey below.
      elevation_m: 0,
      ceiling_m: top.ceiling_m || DEFAULT_CEILING_M,
      subfloor_m: DEFAULT_SUBFLOOR_M,
    };
    // Materialise the implicit ground floor too, otherwise saving a second
    // storey would leave the first undeclared and the server would reject
    // every node on it as living on an undefined floor.
    // A storey below with no subfloor recorded would put the new floor
    // exactly on the old ceiling, which is never true of a real building.
    const below = floors[floors.length - 1];
    if (!below.subfloor_m) below.subfloor_m = DEFAULT_SUBFLOOR_M;
    this.config.floors = [...floors, next];
    this._activeFloor = next.level;
    this._reflowElevations();
    this._renderFloorControls();
    this._renderNodeList();
    this._render();
  }

  /** Remove the top storey, and anything on it.
   *
   * Deleting a storey while nodes or walls still reference it would produce a
   * config the server refuses to save, with an error naming a floor the user
   * can no longer see. Better to say what will go, and go.
   */
  _removeTopFloor() {
    const floors = this._floors();
    if (floors.length <= 1) return;
    const doomed = floors[floors.length - 1].level;
    const nodes = this.config.nodes.filter((n) => this._floorOf(n) === doomed).length;
    const walls = (this.config.walls || []).filter((w) => w.level === doomed).length;
    const rings = (this.config.footprint || []).filter((r) => r.level === doomed).length;
    if (nodes || walls || rings) {
      const ok = window.confirm(
        `Remove floor ${doomed}? This also deletes ${nodes} node(s), ${walls} wall(s) `
        + `and ${rings} outline(s) on it.`
      );
      if (!ok) return;
    }
    this.config.floors = floors.filter((f) => f.level !== doomed);
    this.config.nodes = this.config.nodes.filter((n) => this._floorOf(n) !== doomed);
    this.config.walls = (this.config.walls || []).filter((w) => w.level !== doomed);
    // An outline left behind on a deleted storey makes the server reject every
    // subsequent save with "footprint N is on undefined floor" — an error
    // naming a storey the user can no longer see.
    this.config.footprint = (this.config.footprint || []).filter((r) => r.level !== doomed);
    if (this.config.floors.length <= 1) this.config.floors = [];
    this._activeFloor = 1;
    this._renderFloorControls();
    this._renderNodeList();
    this._renderStoreyPanels();
    this._renderWallList();
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

    // Building outline. Drawn first, filled, so it reads as ground the rest of
    // the plan stands on. The active storey's outline is solid; other storeys
    // show as a faint line, because lining a wing up with the floor below it
    // is exactly what a shared origin is for.
    (this.config.footprint || []).forEach((ring) => {
      const pts = ring.points || [];
      if (pts.length < 3) return;
      const active = ring.level === this._activeFloor;
      ctx.beginPath();
      pts.forEach((p, i) => {
        const { px, py } = this._toPixel(p[0], p[1]);
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      });
      ctx.closePath();
      ctx.fillStyle = active ? 'rgba(50,184,198,0.10)' : 'rgba(50,184,198,0.04)';
      ctx.fill();
      ctx.strokeStyle = active ? 'rgba(50,184,198,0.85)' : 'rgba(50,184,198,0.22)';
      ctx.lineWidth = active ? 2 : 1;
      ctx.stroke();
    });

    // The outline being traced right now: committed segments solid, the one
    // that would follow the next click dashed back to the pointer.
    if (this._ringDraft && this._ringDraft.length) {
      const draft = this._ringDraft;
      ctx.strokeStyle = '#ffd166';
      ctx.lineWidth = 2;
      ctx.beginPath();
      draft.forEach((p, i) => {
        const { px, py } = this._toPixel(p.x, p.y);
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      });
      ctx.stroke();

      if (this._ringHover) {
        const last = this._toPixel(draft[draft.length - 1].x, draft[draft.length - 1].y);
        const first = this._toPixel(draft[0].x, draft[0].y);
        ctx.setLineDash([6, 4]);
        ctx.beginPath();
        ctx.moveTo(last.px, last.py);
        ctx.lineTo(this._ringHover.x, this._ringHover.y);
        // Show the closing edge too, so the shape being committed is the shape
        // on screen rather than one the user has to imagine.
        if (draft.length >= 2) ctx.lineTo(first.px, first.py);
        ctx.stroke();
        ctx.setLineDash([]);
      }

      draft.forEach((p, i) => {
        const { px, py } = this._toPixel(p.x, p.y);
        ctx.beginPath();
        ctx.arc(px, py, i === 0 ? 5 : 3, 0, Math.PI * 2);
        ctx.fillStyle = i === 0 ? '#ffd166' : '#e6e9ef';
        ctx.fill();
      });
    }

    // Walls. Drawn before nodes so a node marker is never hidden behind one.
    // Other storeys show faintly: alignment between floors is exactly what a
    // shared origin is for, and hiding them would make it guesswork.
    (this.config.walls || []).forEach((w) => {
      const a = this._toPixel(w.x1, w.y1);
      const b = this._toPixel(w.x2, w.y2);
      const active = w.level === this._activeFloor;
      ctx.strokeStyle = active ? '#e6edf3' : 'rgba(230,237,243,0.18)';
      ctx.lineWidth = active ? 4 : 2;
      ctx.lineCap = 'round';
      ctx.beginPath();
      ctx.moveTo(a.px, a.py);
      ctx.lineTo(b.px, b.py);
      ctx.stroke();
    });
    ctx.lineCap = 'butt';

    // Wall being dragged out right now.
    if (this._wallStart && this._wallPreview) {
      const a = this._toPixel(this._wallStart.x, this._wallStart.y);
      ctx.strokeStyle = '#ffd166';
      ctx.lineWidth = 4;
      ctx.setLineDash([6, 4]);
      ctx.beginPath();
      ctx.moveTo(a.px, a.py);
      ctx.lineTo(this._wallPreview.x, this._wallPreview.y);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // Nodes.
    this.config.nodes.forEach((node, idx) => {
      const { px, py } = this._toPixel(node.x, node.y);
      if (!Number.isFinite(px) || !Number.isFinite(py)) {
        console.warn('[RoomBuilder] Skipping node with non-finite position:', node);
        return;
      }
      const onThisFloor = this._floorOf(node) === this._activeFloor;
      ctx.globalAlpha = onThisFloor ? 1 : 0.28;
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
    // Leaving globalAlpha faded would silently dim the AP, the live dot and
    // the compass drawn below.
    ctx.globalAlpha = 1;

    this._drawAp(ctx);
    this._drawLiveDot(ctx);
    this._drawCompass(ctx);
  }

  /** Draw the AP marker (a diamond, distinct from the round sensor nodes) —
   * intentionally NOT clamped/warned about being outside the room rectangle,
   * since a real AP very often is (see validate_room_config on the server). */
  _drawAp(ctx) {
    if (!Array.isArray(this.config.ap_position)) return;
    const [x, y] = this.config.ap_position;
    const { px, py } = this._toPixel(x, y);
    if (!Number.isFinite(px) || !Number.isFinite(py)) return;

    ctx.save();
    ctx.translate(px, py);
    ctx.rotate(Math.PI / 4);
    const s = AP_RADIUS;
    ctx.fillStyle = this._draggingAp ? '#ffd166' : '#a78bfa';
    ctx.fillRect(-s, -s, s * 2, s * 2);
    ctx.strokeStyle = '#0d1117';
    ctx.lineWidth = 2;
    ctx.strokeRect(-s, -s, s * 2, s * 2);
    ctx.restore();

    ctx.fillStyle = '#e6e9ef';
    ctx.font = '12px monospace';
    ctx.textAlign = 'center';
    ctx.fillText('AP', px, py - AP_RADIUS - 8);
  }

  /** Draw the live person-position estimate, when one exists. Pulses gently
   * so it reads as "live" rather than a static marker like the sensor nodes.
   * Label states which tier produced it (doppler vs motion) so it's never
   * mistaken for a calibrated fix. */
  _drawLiveDot(ctx) {
    if (!this._liveDot) return;
    const { px, py } = this._toPixel(this._liveDot.x, this._liveDot.y);
    if (!Number.isFinite(px) || !Number.isFinite(py)) return;

    const pulse = 1 + 0.15 * Math.sin(Date.now() / 300);
    ctx.beginPath();
    ctx.arc(px, py, LIVE_DOT_RADIUS * pulse, 0, Math.PI * 2);
    ctx.fillStyle = 'rgba(255, 209, 102, 0.9)';
    ctx.fill();
    ctx.strokeStyle = '#0d1117';
    ctx.lineWidth = 2;
    ctx.stroke();

    ctx.fillStyle = '#ffd166';
    ctx.font = 'bold 12px monospace';
    ctx.textAlign = 'center';
    const LIVE_DOT_LABELS = {
      bistatic_velocity: 'live (bistatic)',
      doppler_centroid: 'live (doppler)',
      motion_centroid: 'live (motion)',
    };
    const label = LIVE_DOT_LABELS[this._liveDotSource] || 'live (motion)';
    ctx.fillText(label, px, py - LIVE_DOT_RADIUS - 8);

    // Keep animating the pulse while a fix is present.
    if (!this._liveDotAnimHandle) {
      const animate = () => {
        if (!this._liveDot) { this._liveDotAnimHandle = null; return; }
        this._render();
        this._liveDotAnimHandle = requestAnimationFrame(animate);
      };
      this._liveDotAnimHandle = requestAnimationFrame(animate);
    }
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
