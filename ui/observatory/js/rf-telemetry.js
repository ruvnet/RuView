/**
 * WebSocket & Physical Telemetry Stream Manager.
 * Connects to ws://localhost:8765/ws/sensing, computes link metrics, and dispatches updates.
 * Complies with ELMAS.md: <= 350 lines, guard clauses, robust error recovery.
 */

export class RfTelemetryStream {
  /**
   * @param {Object} options
   */
  constructor(options = {}) {
    this.wsUrl = options.wsUrl || this.resolveDefaultWsUrl();
    this.onUpdate = options.onUpdate || (() => {});
    this.onStatusChange = options.onStatusChange || (() => {});

    this.ws = null;
    this.reconnectAttempts = 0;
    this.reconnectTimer = null;
    this.isPaused = false;

    // Metrics tracking
    this.frameCount = 0;
    this.lastFpsCalcTime = performance.now();
    this.currentFps = 0.0;
    this.lastPacketTimestamp = 0;
    this.jitterMs = 0.0;
  }

  resolveDefaultWsUrl() {
    const loc = window.location;
    const protocol = loc.protocol === 'https:' ? 'wss:' : 'ws:';
    // The Rust SensingServer exposes the sensing stream at port 8765
    return `${protocol}//${loc.hostname || 'localhost'}:8765/ws/sensing`;
  }

  connect() {
    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
      return;
    }

    try {
      this.ws = new WebSocket(this.wsUrl);
      this.ws.onopen = () => this.handleOpen();
      this.ws.onmessage = (event) => this.handleMessage(event);
      this.ws.onclose = () => this.handleClose();
      this.ws.onerror = (err) => this.handleError(err);
    } catch (e) {
      this.scheduleReconnect();
    }
  }

  handleOpen() {
    this.reconnectAttempts = 0;
    this.onStatusChange({ connected: true, state: 'CONNECTED' });
  }

  handleMessage(event) {
    if (this.isPaused) return;

    const now = performance.now();
    this.frameCount++;

    // Calculate real measured FPS once per second
    if (now - this.lastFpsCalcTime >= 1000) {
      this.currentFps = (this.frameCount * 1000) / (now - this.lastFpsCalcTime);
      this.frameCount = 0;
      this.lastFpsCalcTime = now;
    }

    // Packet jitter estimation
    if (this.lastPacketTimestamp > 0) {
      const delta = now - this.lastPacketTimestamp;
      this.jitterMs = this.jitterMs * 0.9 + Math.abs(delta - 50.0) * 0.1;
    }
    this.lastPacketTimestamp = now;

    try {
      const payload = JSON.parse(event.data);
      this.onUpdate(payload, {
        fps: this.currentFps,
        jitterMs: this.jitterMs,
        provenance: payload.source || 'UNKNOWN'
      });
    } catch (e) {
      // Ignore malformed packets silently
    }
  }

  handleClose() {
    this.onStatusChange({ connected: false, state: 'RECONNECTING' });
    this.scheduleReconnect();
  }

  handleError() {
    if (this.ws) {
      this.ws.close();
    }
  }

  scheduleReconnect() {
    if (this.reconnectTimer) return;
    const delay = Math.min(10000, 1000 * Math.pow(1.5, this.reconnectAttempts++));
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  togglePause() {
    this.isPaused = !this.isPaused;
    return this.isPaused;
  }
}

