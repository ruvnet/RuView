import { withWsTicket } from './ws-ticket.js';
import { apiService } from './api.service.js';
/**
 * Sensing WebSocket Service
 *
 * Manages the connection to the Python sensing WebSocket server
 * (ws://localhost:8765) and provides a callback-based API for the UI.
 *
 * Falls back to simulated data only after MAX_RECONNECT_ATTEMPTS exhausted.
 * While reconnecting the service stays in "reconnecting" state and does NOT
 * emit simulated frames so the UI can clearly distinguish live vs. fallback data.
 */

const SENSING_WS_PORT_BY_HTTP_PORT = {
  // Docker image: HTTP UI/API on 3000, sensing stream on 3001.
  '3000': '3001',
  // Python sensing stack: UI on 8080, sensing stream on 8765.
  '8080': '8765',
};

// Raw `source` values emitted by ws_server.py when it falls back to a
// commodity platform WiFi adapter (ADR-013 collectors). The samples are really
// measured, but the adapter exposes RSSI only — no per-subcarrier CSI — so
// these map to the dedicated "rssi-only" state rather than "live" or
// "server-simulated". Capabilities: PRESENCE + MOTION only.
const RSSI_ONLY_SOURCES = new Set(['macos_wifi', 'linux_wifi', 'windows_wifi']);

export function buildSensingWsUrl(locationLike = (typeof window !== 'undefined' ? window.location : null)) {
  const protocol = locationLike && locationLike.protocol === 'https:' ? 'wss:' : 'ws:';
  const host = locationLike && locationLike.host ? locationLike.host : 'localhost:3001';
  const hostname = locationLike && locationLike.hostname ? locationLike.hostname : host.split(':')[0];
  const port = locationLike && locationLike.port ? locationLike.port : '';
  const wsPort = SENSING_WS_PORT_BY_HTTP_PORT[port];
  const wsHost = wsPort ? `${hostname}:${wsPort}` : host;

  return `${protocol}//${wsHost}/ws/sensing`;
}

const SENSING_WS_URL = buildSensingWsUrl();
const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000];
const MAX_RECONNECT_ATTEMPTS = 20;
// Number of failed attempts that must occur before simulation starts.
// This prevents the UI from flashing "SIMULATED" on a brief hiccup.
const SIM_FALLBACK_AFTER_ATTEMPTS = 5;
const SIMULATION_INTERVAL = 500; // ms

export const SIM_FALLBACK_STORAGE_KEY = 'ruview-client-simulation';

/**
 * Whether the client may invent sensing frames when the server is unreachable.
 *
 * Default **off**, deliberately. The generated frames are plausible — presence
 * derived from `Math.sin(t * 0.3)` crossing a fixed threshold produces a steady
 * absent/present_still alternation that reads exactly like a real classifier
 * struggling at its decision boundary. A reader with no reason to suspect the
 * server had dropped will diagnose the sensing pipeline, and be wrong. That is
 * not a hypothetical: it cost a full debugging session on 2026-08-28, chasing a
 * "flip-flop" that was this function all along while the server sat at a
 * perfectly steady presence=true.
 *
 * For an instrument, "no signal" is the honest reading and a useful one.
 * Synthetic motion is only appropriate for a demo, so it must be asked for:
 * `?simulate=1`, or `localStorage['ruview-client-simulation'] = 'true'`.
 *
 * Repository rule: never present synthetic output as measurement (CLAUDE.md).
 */
function clientSimulationAllowed() {
  try {
    const params = new URLSearchParams(window.location.search);
    if (params.get('simulate') === '1') return true;
    if (params.get('simulate') === '0') return false;
  } catch { /* no window/location — non-browser test context */ }
  try {
    return localStorage.getItem(SIM_FALLBACK_STORAGE_KEY) === 'true';
  } catch {
    return false;
  }
}

class SensingService {
  constructor() {
    /** @type {WebSocket|null} */
    this._ws = null;
    this._listeners = new Set();
    this._stateListeners = new Set();
    this._reconnectAttempt = 0;
    this._reconnectTimer = null;
    this._simTimer = null;
    // Guards the await window inside `_connect()` (see the comment there).
    this._connectInFlight = false;
    // Bumped per connect attempt and by `stop()`; lets a socket recognise that
    // it has been abandoned and stop driving reconnect state.
    this._connectEpoch = 0;
    // Connection state: disconnected | connecting | connected | reconnecting | simulated
    this._state = 'disconnected';
    // Data-source label exposed to the UI:
    //   "live"              — real ESP32 hardware connected
    //   "rssi-only"         — real commodity-WiFi RSSI measurements, no CSI
    //   "server-simulated"  — server is running but using synthetic data (no hardware)
    //   "reconnecting"      — WebSocket disconnected, retrying
    //   "simulated"         — client-side fallback simulation (server unreachable)
    this._dataSource = 'reconnecting';
    // The raw source string from the server (e.g. "esp32", "simulated", "simulate")
    this._serverSource = null;
    this._lastMessage = null;

    // Ring buffer of recent RSSI values for sparkline
    this._rssiHistory = [];
    this._maxHistory = 60;
  }

  // ---- Public API --------------------------------------------------------

  /** Start the service (connect or simulate). */
  start() {
    void this._connect();
  }

  /** Stop the service entirely. */
  stop() {
    // Invalidate any attempt still inside its await window, so a socket opened
    // after this call does not resurrect the service.
    this._connectEpoch++;
    this._connectInFlight = false;
    this._clearTimers();
    if (this._ws) {
      this._ws.close(1000, 'client stop');
      this._ws = null;
    }
    this._setState('disconnected');
  }

  /** Register a callback for sensing data updates. Returns unsubscribe fn. */
  onData(callback) {
    this._listeners.add(callback);
    // Immediately push last known data if available
    if (this._lastMessage) callback(this._lastMessage);
    return () => this._listeners.delete(callback);
  }

  /** Register a callback for connection state changes. Returns unsubscribe fn. */
  onStateChange(callback) {
    this._stateListeners.add(callback);
    callback(this._state);
    return () => this._stateListeners.delete(callback);
  }

  /** Get the RSSI sparkline history (array of floats). */
  getRssiHistory() {
    return [...this._rssiHistory];
  }

  /** Get per-node RSSI history (object keyed by node_id). */
  getPerNodeRssiHistory() {
    return { ...(this._perNodeRssiHistory || {}) };
  }

  /** Current connection state. */
  get state() {
    return this._state;
  }

  /**
   * Current data source label.
   * "live"         — frames are arriving from the real ESP32 over WebSocket
   * "rssi-only"    — frames are real measurements from this machine's own WiFi
   *                  adapter (RSSI only, no CSI): presence + gross motion only
   * "reconnecting" — WebSocket disconnected; actively retrying, no frames emitted
   * "unreachable"  — retries exhausted, client simulation off; NO frames emitted
   *                  and whatever is on screen is the last live reading, stale
   * "simulated"    — client simulation explicitly enabled and running; every
   *                  frame on screen is invented
   */
  get dataSource() {
    return this._dataSource;
  }

  // ---- Connection --------------------------------------------------------

  // async because the server gates `/ws/sensing` (ADR-272) and a browser
  // cannot set an Authorization header on an upgrade — so we mint a
  // single-use ticket first. Minted per connect attempt, never cached: a
  // ticket is valid once and expires in seconds, so reusing one across
  // reconnects would fail on the second attempt.
  async _connect() {
    if (this._ws && this._ws.readyState <= WebSocket.OPEN) return;
    // The readyState guard above cannot stand alone: this function awaits
    // ticket minting before it assigns `this._ws`, so two callers entering
    // during that window BOTH pass the guard and BOTH open a socket. The
    // second assignment orphans the first — a socket nobody references, still
    // open on the server, whose `onclose` fires later and calls
    // `_scheduleReconnect()` even though a healthy socket is live. That walks
    // `_reconnectAttempt` upward with no outage and eventually trips the
    // simulation fallback. Observed on the server as connect/disconnect churn
    // on every page load.
    if (this._connectInFlight) return;
    this._connectInFlight = true;

    // Identifies this attempt for the lifetime of its socket. `stop()` and any
    // superseding attempt bump it, so a late event from an abandoned socket is
    // recognised and ignored rather than driving reconnect state.
    const epoch = ++this._connectEpoch;

    this._setState('connecting');

    let url = SENSING_WS_URL;
    try {
      url = await withWsTicket(SENSING_WS_URL);
    } catch {
      // Ticket minting is best-effort: against a server with auth off, or one
      // predating ADR-272, connecting without a ticket is correct.
    }

    if (epoch !== this._connectEpoch) {
      // Superseded or stopped while minting. The ticket is single-use and now
      // spent; the current attempt mints its own.
      this._connectInFlight = false;
      return;
    }

    let ws;
    try {
      ws = new WebSocket(url);
    } catch (err) {
      console.warn('[Sensing] WebSocket constructor failed:', err.message);
      this._connectInFlight = false;
      this._fallbackToSimulation();
      return;
    }

    this._ws = ws;
    this._connectInFlight = false;

    /** True while `ws` is still the socket this service is driving. */
    const isCurrent = () => epoch === this._connectEpoch && this._ws === ws;

    ws.onopen = () => {
      if (!isCurrent()) {
        ws.close(1000, 'superseded');
        return;
      }
      console.info('[Sensing] Connected to', SENSING_WS_URL);
      this._reconnectAttempt = 0;
      this._stopSimulation();
      this._setState('connected');
      // Don't assume "live" yet — wait for first frame's source field.
      // Fetch server status to determine actual data source immediately.
      this._detectServerSource();
    };

    ws.onmessage = (evt) => {
      if (!isCurrent()) return;
      try {
        const data = JSON.parse(evt.data);
        this._handleData(data);
      } catch (e) {
        console.warn('[Sensing] Invalid message:', e.message);
      }
    };

    ws.onerror = () => {
      // onerror is always followed by onclose, so we handle reconnect there
    };

    ws.onclose = (evt) => {
      if (!isCurrent()) {
        // An abandoned socket closing is not an outage. Reconnect state
        // belongs to whichever attempt is current.
        console.debug('[Sensing] Ignoring close from superseded socket');
        return;
      }
      console.info('[Sensing] Connection closed (code=%d)', evt.code);
      this._ws = null;
      if (evt.code !== 1000) {
        this._scheduleReconnect();
      } else {
        this._setState('disconnected');
        this._setDataSource('reconnecting');
      }
    };
  }

  _scheduleReconnect() {
    // A retry is already pending; stacking timers would multiply the attempt
    // count for a single outage.
    if (this._reconnectTimer) return;

    if (this._reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
      if (clientSimulationAllowed()) {
        console.warn('[Sensing] Max reconnect attempts (%d) reached, switching to simulation', MAX_RECONNECT_ATTEMPTS);
        this._fallbackToSimulation();
        return;
      }
      // With simulation off there is nothing to give up *for*: keep retrying at
      // the ceiling delay so the UI recovers by itself when the server returns,
      // rather than requiring a hard refresh.
      this._setDataSource('unreachable');
    }

    const delay = RECONNECT_DELAYS[Math.min(this._reconnectAttempt, RECONNECT_DELAYS.length - 1)];
    // Hold at the cap rather than counting into the thousands over a long
    // outage; the delay is already saturated and the number is only read by a
    // human in the console.
    this._reconnectAttempt = Math.min(this._reconnectAttempt + 1, MAX_RECONNECT_ATTEMPTS);
    console.info('[Sensing] Reconnecting in %dms (attempt %d/%d)', delay, this._reconnectAttempt, MAX_RECONNECT_ATTEMPTS);

    this._setState('reconnecting');
    if (this._dataSource !== 'unreachable') this._setDataSource('reconnecting');

    this._reconnectTimer = setTimeout(() => {
      this._reconnectTimer = null;
      void this._connect();
    }, delay);

    // Only start simulation after several failed attempts so a brief hiccup
    // does not immediately switch the UI to "SIMULATED DATA".
    if (this._reconnectAttempt >= SIM_FALLBACK_AFTER_ATTEMPTS && this._state !== 'simulated') {
      if (clientSimulationAllowed()) {
        this._fallbackToSimulation();
      } else {
        // Say "the server is gone" instead of inventing a room.
        this._setDataSource('unreachable');
      }
    }
  }

  // ---- Simulation fallback -----------------------------------------------

  _fallbackToSimulation() {
    if (!clientSimulationAllowed()) {
      this._setDataSource('unreachable');
      this._scheduleReconnect();
      return;
    }
    this._setState('simulated');
    this._setDataSource('simulated');
    if (this._simTimer) return; // already running
    console.info('[Sensing] Running in simulation mode');

    this._simTimer = setInterval(() => {
      const data = this._generateSimulatedData();
      this._handleData(data);
    }, SIMULATION_INTERVAL);
  }

  _stopSimulation() {
    if (this._simTimer) {
      clearInterval(this._simTimer);
      this._simTimer = null;
    }
  }

  _generateSimulatedData() {
    const t = Date.now() / 1000;
    const baseRssi = -45;
    const variance = 1.5 + Math.sin(t * 0.1) * 1.0;
    const motionBand = 0.05 + Math.abs(Math.sin(t * 0.3)) * 0.15;
    const breathBand = 0.03 + Math.abs(Math.sin(t * 0.05)) * 0.08;
    const isPresent = variance > 0.8;
    const isActive = motionBand > 0.12;

    // Generate signal field
    const gridSize = 20;
    const values = [];
    for (let iz = 0; iz < gridSize; iz++) {
      for (let ix = 0; ix < gridSize; ix++) {
        const cx = gridSize / 2, cy = gridSize / 2;
        const dist = Math.sqrt((ix - cx) ** 2 + (iz - cy) ** 2);
        let v = Math.max(0, 1 - dist / (gridSize * 0.7)) * 0.3;
        // Body blob
        const bx = cx + 3 * Math.sin(t * 0.2);
        const by = cy + 2 * Math.cos(t * 0.15);
        const bodyDist = Math.sqrt((ix - bx) ** 2 + (iz - by) ** 2);
        if (isPresent) {
          v += Math.exp(-bodyDist * bodyDist / 8) * (0.3 + motionBand * 3);
        }
        values.push(Math.min(1, Math.max(0, v + Math.random() * 0.05)));
      }
    }

    return {
      type: 'sensing_update',
      timestamp: t,
      source: 'simulated',
      // Explicit machine-readable marker so the UI can always detect simulated
      // frames regardless of which code path produced them.
      _simulated: true,
      nodes: [{
        node_id: 1,
        rssi_dbm: baseRssi + Math.sin(t * 0.5) * 3,
        position: [2, 0, 1.5],
        amplitude: [],
        subcarrier_count: 0,
      }],
      features: {
        mean_rssi: baseRssi + Math.sin(t * 0.5) * 3,
        variance,
        std: Math.sqrt(variance),
        motion_band_power: motionBand,
        breathing_band_power: breathBand,
        dominant_freq_hz: 0.3 + Math.sin(t * 0.02) * 0.1,
        change_points: Math.floor(Math.random() * 3),
        spectral_power: motionBand + breathBand + Math.random() * 0.1,
        range: variance * 3,
        iqr: variance * 1.5,
        skewness: (Math.random() - 0.5) * 0.5,
        kurtosis: Math.random() * 2,
      },
      classification: {
        motion_level: isActive ? 'active' : (isPresent ? 'present_still' : 'absent'),
        presence: isPresent,
        confidence: isPresent ? 0.75 + Math.random() * 0.2 : 0.5 + Math.random() * 0.3,
      },
      signal_field: {
        grid_size: [gridSize, 1, gridSize],
        values,
      },
    };
  }

  // ---- Server source detection -------------------------------------------

  /**
   * Fetch `/api/v1/status` to find out if the server is using real
   * hardware or simulation. Called once on WebSocket open.
   */
  async _detectServerSource() {
    // ADR-295 (issue #1526): an unreachable or unauthorized status endpoint is
    // an *unknown* state — it must NOT collapse to "live". Prefer the canonical
    // `source_state` the server now returns; on any error stay conservative
    // (server-simulated) until a real frame's `source` field promotes us.
    //
    // Send the bearer token via `apiService.getHeaders()` so the probe can
    // actually succeed under the documented secure posture (API auth
    // enabled) instead of always 401ing and relying solely on the
    // conservative fallback below (issue #1526, suggested fix #1).
    try {
      const resp = await fetch('/api/v1/status', { headers: apiService.getHeaders() });
      if (resp.ok) {
        const json = await resp.json();
        this._applyServerSource(json.source, json.source_state);
      } else {
        this._setDataSource('server-simulated');
      }
    } catch {
      this._setDataSource('server-simulated');
    }
  }

  /**
   * Map a raw server source string (and optional canonical ADR-295
   * `source_state`) to the UI data-source label.
   */
  _applyServerSource(rawSource, sourceState) {
    this._serverSource = rawSource;
    // ADR-295: only the verified/unverified live states may show "live"; any
    // synthetic/stale/disconnected state must not.
    if (sourceState) {
      if (sourceState === 'live_verified' || sourceState === 'live_unverified') {
        this._setDataSource('live');
      } else if (sourceState === 'synthetic') {
        this._setDataSource('server-simulated');
      } else {
        this._setDataSource('server-simulated');
      }
      return;
    }
    if (rawSource === 'esp32' || rawSource === 'wifi' || rawSource === 'live') {
      this._setDataSource('live');
    } else if (RSSI_ONLY_SOURCES.has(rawSource)) {
      // Real measurements from a commodity platform WiFi adapter (ADR-013 /
      // ADR-049 collectors). These are genuinely measured — collapsing them to
      // "server-simulated" would label real data as invented — but they carry
      // no CSI, so they must not be promoted to "live" either. Third state.
      this._setDataSource('rssi-only');
    } else if (rawSource === 'simulated' || rawSource === 'simulate') {
      this._setDataSource('server-simulated');
    } else {
      // Unknown source — show as server-simulated to be safe
      this._setDataSource('server-simulated');
    }
  }

  /** @return {string|null} Raw server source (e.g. "esp32", "simulated") */
  get serverSource() {
    return this._serverSource;
  }

  // ---- Data handling -----------------------------------------------------

  _handleData(data) {
    this._lastMessage = data;

    // Track the server's source field from each frame so the UI
    // can react if the server switches between esp32 ↔ simulated at runtime.
    if (data.source && this._state === 'connected') {
      const raw = data.source;
      if (raw !== this._serverSource) {
        this._applyServerSource(raw);
      }
    }

    // Update RSSI history for sparkline
    if (data.features && data.features.mean_rssi != null) {
      this._rssiHistory.push(data.features.mean_rssi);
      if (this._rssiHistory.length > this._maxHistory) {
        this._rssiHistory.shift();
      }
    }

    // Per-node RSSI tracking
    if (!this._perNodeRssiHistory) this._perNodeRssiHistory = {};
    if (data.node_features) {
      for (const nf of data.node_features) {
        if (!this._perNodeRssiHistory[nf.node_id]) {
          this._perNodeRssiHistory[nf.node_id] = [];
        }
        this._perNodeRssiHistory[nf.node_id].push(nf.rssi_dbm);
        if (this._perNodeRssiHistory[nf.node_id].length > this._maxHistory) {
          this._perNodeRssiHistory[nf.node_id].shift();
        }
      }
    }

    // Notify all listeners
    for (const cb of this._listeners) {
      try {
        cb(data);
      } catch (e) {
        console.error('[Sensing] Listener error:', e);
      }
    }
  }

  // ---- State management --------------------------------------------------

  _setState(newState) {
    if (newState === this._state) return;
    this._state = newState;
    for (const cb of this._stateListeners) {
      try { cb(newState); } catch (e) { /* ignore */ }
    }
  }

  /**
   * Update the dataSource label and notify state listeners so the UI can
   * react without needing a separate subscription.
   * @param {'live'|'rssi-only'|'server-simulated'|'reconnecting'|'simulated'} source
   */
  _setDataSource(source) {
    if (source === this._dataSource) return;
    this._dataSource = source;
    // Re-use the same state-listener channel — listeners receive the
    // connection state but can read dataSource via service.dataSource.
    for (const cb of this._stateListeners) {
      try { cb(this._state); } catch (e) { /* ignore */ }
    }
  }

  _clearTimers() {
    this._stopSimulation();
    if (this._reconnectTimer) {
      clearTimeout(this._reconnectTimer);
      this._reconnectTimer = null;
    }
  }
}

// Singleton
export const sensingService = new SensingService();
