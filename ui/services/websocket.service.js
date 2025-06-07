// WebSocket Service for WiFi-DensePose UI

import { API_CONFIG, buildWsUrl } from '../config/api.config.js';

export class WebSocketService {
  constructor() {
    this.connections = new Map();
    this.messageHandlers = new Map();
    this.reconnectAttempts = new Map();
  }

  // Connect to WebSocket endpoint
  connect(endpoint, params = {}, handlers = {}) {
    const url = buildWsUrl(endpoint, params);
    
    // Check if already connected
    if (this.connections.has(url)) {
      console.warn(`Already connected to ${url}`);
      return this.connections.get(url);
    }

    // Create WebSocket connection
    const ws = new WebSocket(url);
    const connectionId = this.generateId();

    // Store connection
    this.connections.set(url, {
      id: connectionId,
      ws,
      url,
      handlers,
      status: 'connecting',
      lastPing: null,
      reconnectTimer: null
    });

    // Set up event handlers
    this.setupEventHandlers(url, ws, handlers);

    // Start ping interval
    this.startPingInterval(url);

    return connectionId;
  }

  // Set up WebSocket event handlers
  setupEventHandlers(url, ws, handlers) {
    const connection = this.connections.get(url);

    ws.onopen = (event) => {
      console.log(`WebSocket connected: ${url}`);
      connection.status = 'connected';
      this.reconnectAttempts.set(url, 0);
      
      if (handlers.onOpen) {
        handlers.onOpen(event);
      }
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        
        // Handle different message types
        this.handleMessage(url, data);
        
        if (handlers.onMessage) {
          handlers.onMessage(data);
        }
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error);
      }
    };

    ws.onerror = (event) => {
      console.error(`WebSocket error: ${url}`, event);
      connection.status = 'error';
      
      if (handlers.onError) {
        handlers.onError(event);
      }
    };

    ws.onclose = (event) => {
      console.log(`WebSocket closed: ${url}`);
      connection.status = 'closed';
      
      // Clear ping interval
      this.clearPingInterval(url);
      
      if (handlers.onClose) {
        handlers.onClose(event);
      }
      
      // Attempt reconnection if not intentionally closed
      if (!event.wasClean && this.shouldReconnect(url)) {
        this.scheduleReconnect(url);
      } else {
        this.connections.delete(url);
      }
    };
  }

  // Handle incoming messages
  handleMessage(url, data) {
    const { type, payload } = data;

    // Handle system messages
    switch (type) {
      case 'pong':
        this.handlePong(url);
        break;
      
      case 'connection_established':
        console.log('Connection established:', payload);
        break;
      
      case 'error':
        console.error('WebSocket error message:', payload);
        break;
    }

    // Call registered message handlers
    const handlers = this.messageHandlers.get(url) || [];
    handlers.forEach(handler => handler(data));
  }

  // Send message through WebSocket
  send(connectionId, message) {
    const connection = this.findConnectionById(connectionId);
    
    if (!connection) {
      throw new Error(`Connection ${connectionId} not found`);
    }

    if (connection.status !== 'connected') {
      throw new Error(`Connection ${connectionId} is not connected`);
    }

    const data = typeof message === 'string' 
      ? message 
      : JSON.stringify(message);

    connection.ws.send(data);
  }

  // Send command message
  sendCommand(connectionId, command, payload = {}) {
    this.send(connectionId, {
      type: command,
      payload,
      timestamp: new Date().toISOString()
    });
  }

  // Register message handler
  onMessage(connectionId, handler) {
    const connection = this.findConnectionById(connectionId);
    
    if (!connection) {
      throw new Error(`Connection ${connectionId} not found`);
    }

    if (!this.messageHandlers.has(connection.url)) {
      this.messageHandlers.set(connection.url, []);
    }

    this.messageHandlers.get(connection.url).push(handler);

    // Return unsubscribe function
    return () => {
      const handlers = this.messageHandlers.get(connection.url);
      const index = handlers.indexOf(handler);
      if (index > -1) {
        handlers.splice(index, 1);
      }
    };
  }

  // Disconnect WebSocket
  disconnect(connectionId) {
    const connection = this.findConnectionById(connectionId);
    
    if (!connection) {
      return;
    }

    // Clear reconnection timer
    if (connection.reconnectTimer) {
      clearTimeout(connection.reconnectTimer);
    }

    // Clear ping interval
    this.clearPingInterval(connection.url);

    // Close WebSocket
    if (connection.ws.readyState === WebSocket.OPEN) {
      connection.ws.close(1000, 'Client disconnect');
    }

    // Clean up
    this.connections.delete(connection.url);
    this.messageHandlers.delete(connection.url);
    this.reconnectAttempts.delete(connection.url);
  }

  // Disconnect all WebSockets
  disconnectAll() {
    const connectionIds = Array.from(this.connections.values()).map(c => c.id);
    connectionIds.forEach(id => this.disconnect(id));
  }

  // Ping/Pong handling
  startPingInterval(url) {
    const connection = this.connections.get(url);
    if (!connection) return;

    connection.pingInterval = setInterval(() => {
      if (connection.status === 'connected') {
        this.sendPing(url);
      }
    }, API_CONFIG.WS_CONFIG.PING_INTERVAL);
  }

  clearPingInterval(url) {
    const connection = this.connections.get(url);
    if (connection && connection.pingInterval) {
      clearInterval(connection.pingInterval);
    }
  }

  sendPing(url) {
    const connection = this.connections.get(url);
    if (connection && connection.status === 'connected') {
      connection.lastPing = Date.now();
      connection.ws.send(JSON.stringify({ type: 'ping' }));
    }
  }

  handlePong(url) {
    const connection = this.connections.get(url);
    if (connection) {
      const latency = Date.now() - connection.lastPing;
      console.log(`Pong received. Latency: ${latency}ms`);
    }
  }

  // Reconnection logic
  shouldReconnect(url) {
    const attempts = this.reconnectAttempts.get(url) || 0;
    return attempts < API_CONFIG.WS_CONFIG.MAX_RECONNECT_ATTEMPTS;
  }

  scheduleReconnect(url) {
    const connection = this.connections.get(url);
    if (!connection) return;

    const attempts = this.reconnectAttempts.get(url) || 0;
    const delay = API_CONFIG.WS_CONFIG.RECONNECT_DELAY * Math.pow(2, attempts);

    console.log(`Scheduling reconnect in ${delay}ms (attempt ${attempts + 1})`);

    connection.reconnectTimer = setTimeout(() => {
      this.reconnectAttempts.set(url, attempts + 1);
      
      // Get original parameters
      const params = new URL(url).searchParams;
      const paramsObj = Object.fromEntries(params);
      const endpoint = url.replace(/^wss?:\/\/[^\/]+/, '').split('?')[0];
      
      // Attempt reconnection
      this.connect(endpoint, paramsObj, connection.handlers);
    }, delay);
  }

  // Utility methods
  findConnectionById(connectionId) {
    for (const connection of this.connections.values()) {
      if (connection.id === connectionId) {
        return connection;
      }
    }
    return null;
  }

  generateId() {
    return `ws_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  getConnectionStatus(connectionId) {
    const connection = this.findConnectionById(connectionId);
    return connection ? connection.status : 'disconnected';
  }

  getActiveConnections() {
    return Array.from(this.connections.values()).map(conn => ({
      id: conn.id,
      url: conn.url,
      status: conn.status
    }));
  }
}

// Create singleton instance
export const wsService = new WebSocketService();