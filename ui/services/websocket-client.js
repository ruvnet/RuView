// WebSocket Client for Three.js Visualization - WiFi DensePose
// Connects to ws://localhost:8000/ws/pose and manages real-time data flow

export class WebSocketClient {
  constructor(options = {}) {
    this.url = options.url || 'ws://localhost:8000/ws/pose';
    this.ws = null;
    this.state = 'disconnected'; // disconnected, connecting, connected, error
    this.isRealData = false;

    // Reconnection settings
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = options.maxReconnectAttempts || 15;
    this.reconnectDelays = [500, 1000, 2000, 4000, 8000, 15000, 30000];
    this.reconnectTimer = null;
    this.autoReconnect = options.autoReconnect !== false;

    // Heartbeat
    this.heartbeatInterval = null;
    this.heartbeatFrequency = options.heartbeatFrequency || 25000;
    this.lastPong = 0;

    // Metrics
    this.metrics = {
      messageCount: 0,
      errorCount: 0,
      connectTime: null,
      lastMessageTime: null,
      latency: 0,
      bytesReceived: 0
    };

    // Callbacks
    this._onMessage = options.onMessage || (() => {});
    this._onStateChange = options.onStateChange || (() => {});
    this._onError = options.onError || (() => {});
  }

  // Attempt to connect
  connect() {
    if (this.state === 'connecting' || this.state === 'connected') {
      console.warn('[WS-VIZ] Already connected or connecting');
      return;
    }

    this._setState('connecting');
    console.log(`[WS-VIZ] Connecting to ${this.url}`);

    try {
      this.ws = new WebSocket(this.url);
      this.ws.binaryType = 'arraybuffer';

      this.ws.onopen = () => this._handleOpen();
      this.ws.onmessage = (event) => this._handleMessage(event);
      this.ws.onerror = (event) => this._handleError(event);
      this.ws.onclose = (event) => this._handleClose(event);

      // Connection timeout
      this._connectTimeout = setTimeout(() => {
        if (this.state === 'connecting') {
          console.warn('[WS-VIZ] Connection timeout');
          this.ws.close();
          this._setState('error');
          this._scheduleReconnect();
        }
      }, 8000);

    } catch (err) {
      console.error('[WS-VIZ] Failed to create WebSocket:', err);
      this._setState('error');
      this._onError(err);
      this._scheduleReconnect();
    }
  }

  disconnect() {
    this.autoReconnect = false;
    this._clearTimers();

    if (this.ws) {
      this.ws.onclose = null; // Prevent reconnect on intentional close
      if (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING) {
        this.ws.close(1000, 'Client disconnect');
      }
      this.ws = null;
    }

    this._setState('disconnected');
    this.isRealData = false;
    console.log('[WS-VIZ] Disconnected');
  }

  // Send a message
  send(data) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn('[WS-VIZ] Cannot send - not connected');
      return false;
    }

    const msg = typeof data === 'string' ? data : JSON.stringify(data);
    this.ws.send(msg);
    return true;
  }

  _handleOpen() {
    clearTimeout(this._connectTimeout);
    this.reconnectAttempts = 0;
    this.metrics.connectTime = Date.now();
    this._setState('connected');
    console.log('[WS-VIZ] Connected successfully');

    // Start heartbeat
    this._startHeartbeat();

    // Request initial state
    this.send({ type: 'get_status', timestamp: Date.now() });
  }

  _handleMessage(event) {
    this.metrics.messageCount++;
    this.metrics.lastMessageTime = Date.now();

    const rawSize = typeof event.data === 'string' ? event.data.length : event.data.byteLength;
    this.metrics.bytesReceived += rawSize;

    try {
      const data = typeof event.data === 'string' ? JSON.parse(event.data) : event.data;

      // Handle pong
      if (data.type === 'pong') {
        this.lastPong = Date.now();
        if (data.timestamp) {
          this.metrics.latency = Date.now() - data.timestamp;
        }
        return;
      }

      // Handle connection_established
      if (data.type === 'connection_established') {
        console.log('[WS-VIZ] Server confirmed connection:', data.payload);
        return;
      }

      // Detect real vs mock data from metadata
      if (data.data && data.data.metadata) {
        this.isRealData = data.data.metadata.mock_data === false && data.data.metadata.source !== 'mock';
      } else if (data.metadata) {
        this.isRealData = data.metadata.mock_data === false;
      }

      // Calculate latency from message timestamp
      if (data.timestamp) {
        const msgTime = new Date(data.timestamp).getTime();
        if (!isNaN(msgTime)) {
          this.metrics.latency = Date.now() - msgTime;
        }
      }

      // Forward to callback
      this._onMessage(data);

    } catch (err) {
      this.metrics.errorCount++;
      console.error('[WS-VIZ] Failed to parse message:', err);
    }
  }

  _handleError(event) {
    this.metrics.errorCount++;
    console.error('[WS-VIZ] WebSocket error:', event);
    this._onError(event);
  }

  _handleClose(event) {
    clearTimeout(this._connectTimeout);
    this._stopHeartbeat();
    this.ws = null;

    const wasConnected = this.state === 'connected';
    console.log(`[WS-VIZ] Connection closed: code=${event.code}, reason=${event.reason}, clean=${event.wasClean}`);

    if (event.wasClean || !this.autoReconnect) {
      this._setState('disconnected');
    } else {
      this._setState('error');
      this._scheduleReconnect();
    }
  }

  _setState(newState) {
    if (this.state === newState) return;
    const oldState = this.state;
    this.state = newState;
    this._onStateChange(newState, oldState);
  }

  _startHeartbeat() {
    this._stopHeartbeat();
    this.heartbeatInterval = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.send({ type: 'ping', timestamp: Date.now() });
      }
    }, this.heartbeatFrequency);
  }

  _stopHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }

  _scheduleReconnect() {
    if (!this.autoReconnect) return;
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('[WS-VIZ] Max reconnect attempts reached');
      this._setState('error');
      return;
    }

    const delayIdx = Math.min(this.reconnectAttempts, this.reconnectDelays.length - 1);
    const delay = this.reconnectDelays[delayIdx];
    this.reconnectAttempts++;

    console.log(`[WS-VIZ] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts})`);

    this.reconnectTimer = setTimeout(() => {
      this.connect();
    }, delay);
  }

  _clearTimers() {
    clearTimeout(this._connectTimeout);
    clearTimeout(this.reconnectTimer);
    this._stopHeartbeat();
  }

  getMetrics() {
    return {
      ...this.metrics,
      state: this.state,
      isRealData: this.isRealData,
      reconnectAttempts: this.reconnectAttempts,
      uptime: this.metrics.connectTime ? (Date.now() - this.metrics.connectTime) / 1000 : 0
    };
  }

  isConnected() {
    return this.state === 'connected';
  }

  dispose() {
    this.disconnect();
    this._onMessage = () => {};
    this._onStateChange = () => {};
    this._onError = () => {};
  }
}
