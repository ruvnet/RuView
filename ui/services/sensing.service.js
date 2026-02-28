/**
 * Sensing WebSocket Service
 *
 * Manages the connection to the Python sensing WebSocket server
 * (ws://localhost:8765) and provides a callback-based API for the UI.
 *
 * Falls back to simulated data if the server is unreachable so the UI
 * always shows something.
 */

const SENSING_WS_URL = 'ws://localhost:8765';
const RECONNECT_DELAYS = [1000, 2000, 4000, 8000, 16000];
const MAX_RECONNECT_ATTEMPTS = 10;
const SIMULATION_INTERVAL = 500; // ms

class SensingService {
  constructor() {
    /** @type {WebSocket|null} */
    this._ws = null;
    this._listeners = new Set();
    this._stateListeners = new Set();
    this._reconnectAttempt = 0;
    this._reconnectTimer = null;
    this._simTimer = null;
    this._state = 'disconnected'; // disconnected | connecting | connected | simulated
    this._lastMessage = null;

    // Ring buffer of recent RSSI values for sparkline
    this._rssiHistory = [];
    this._maxHistory = 60;
  }

  // ---- Public API --------------------------------------------------------

  /** Start the service (connect or simulate). */
  start() {
    this._connect();
  }

  /** Stop the service entirely. */
  stop() {
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

  /** Current connection state. */
  get state() {
    return this._state;
  }

  // ---- Connection --------------------------------------------------------

  _connect() {
    if (this._ws && this._ws.readyState <= WebSocket.OPEN) return;

    this._setState('connecting');

    try {
      this._ws = new WebSocket(SENSING_WS_URL);
    } catch (err) {
      console.warn('[Sensing] WebSocket constructor failed:', err.message);
      this._fallbackToSimulation();
      return;
    }

    this._ws.onopen = () => {
      console.info('[Sensing] Connected to', SENSING_WS_URL);
      this._reconnectAttempt = 0;
      this._stopSimulation();
      this._setState('connected');
    };

    this._ws.onmessage = (evt) => {
      try {
        const data = JSON.parse(evt.data);
        this._handleData(data);
      } catch (e) {
        console.warn('[Sensing] Invalid message:', e.message);
      }
    };

    this._ws.onerror = () => {
      // onerror is always followed by onclose, so we handle reconnect there
    };

    this._ws.onclose = (evt) => {
      console.info('[Sensing] Connection closed (code=%d)', evt.code);
      this._ws = null;
      if (evt.code !== 1000) {
        this._scheduleReconnect();
      } else {
        this._setState('disconnected');
      }
    };
  }

  _scheduleReconnect() {
    if (this._reconnectAttempt >= MAX_RECONNECT_ATTEMPTS) {
      console.warn('[Sensing] Max reconnect attempts reached, switching to simulation');
      this._fallbackToSimulation();
      return;
    }

    const delay = RECONNECT_DELAYS[Math.min(this._reconnectAttempt, RECONNECT_DELAYS.length - 1)];
    this._reconnectAttempt++;
    console.info('[Sensing] Reconnecting in %dms (attempt %d)', delay, this._reconnectAttempt);

    this._reconnectTimer = setTimeout(() => {
      this._reconnectTimer = null;
      this._connect();
    }, delay);

    // Start simulation while waiting
    if (this._state !== 'simulated') {
      this._fallbackToSimulation();
    }
  }

  // ---- Simulation fallback -----------------------------------------------

  _fallbackToSimulation() {
    this._setState('simulated');
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

  // ---- Data handling -----------------------------------------------------

  _handleData(data) {
    this._lastMessage = data;

    // Update RSSI history for sparkline
    if (data.features && data.features.mean_rssi != null) {
      this._rssiHistory.push(data.features.mean_rssi);
      if (this._rssiHistory.length > this._maxHistory) {
        this._rssiHistory.shift();
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
