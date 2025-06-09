// WebSocket Service for WiFi-DensePose UI

import { API_CONFIG, buildWsUrl } from '../config/api.config.js';
import { backendDetector } from '../utils/backend-detector.js';

export class WebSocketService {
  constructor() {
    this.connections = new Map();
    this.messageHandlers = new Map();
    this.reconnectAttempts = new Map();
    this.connectionStateCallbacks = new Map();
    this.logger = this.createLogger();
    
    // Configuration
    this.config = {
      heartbeatInterval: 30000, // 30 seconds
      connectionTimeout: 10000, // 10 seconds
      maxReconnectAttempts: 10,
      reconnectDelays: [1000, 2000, 4000, 8000, 16000, 30000], // Exponential backoff with max 30s
      enableDebugLogging: true
    };
  }

  createLogger() {
    return {
      debug: (...args) => {
        if (this.config.enableDebugLogging) {
          console.debug('[WS-DEBUG]', new Date().toISOString(), ...args);
        }
      },
      info: (...args) => console.info('[WS-INFO]', new Date().toISOString(), ...args),
      warn: (...args) => console.warn('[WS-WARN]', new Date().toISOString(), ...args),
      error: (...args) => console.error('[WS-ERROR]', new Date().toISOString(), ...args)
    };
  }

  // Connect to WebSocket endpoint
  async connect(endpoint, params = {}, handlers = {}) {
    this.logger.debug('Attempting to connect to WebSocket', { endpoint, params });
    
    // Determine if we should use mock WebSockets
    const useMock = await backendDetector.shouldUseMockServer();
    
    let url;
    if (useMock) {
      // Use mock WebSocket URL (served from same origin as UI)
      url = buildWsUrl(endpoint, params).replace('localhost:8000', window.location.host);
      this.logger.info('Using mock WebSocket server', { url });
    } else {
      // Use real backend WebSocket URL
      url = buildWsUrl(endpoint, params);
      this.logger.info('Using real backend WebSocket server', { url });
    }
    
    // Check if already connected
    if (this.connections.has(url)) {
      this.logger.warn(`Already connected to ${url}`);
      return this.connections.get(url).id;
    }

    // Create connection data structure first
    const connectionId = this.generateId();
    const connectionData = {
      id: connectionId,
      ws: null,
      url,
      handlers,
      status: 'connecting',
      lastPing: null,
      reconnectTimer: null,
      connectionTimer: null,
      heartbeatTimer: null,
      connectionStartTime: Date.now(),
      lastActivity: Date.now(),
      messageCount: 0,
      errorCount: 0
    };

    this.connections.set(url, connectionData);

    try {
      // Create WebSocket connection with timeout
      const ws = await this.createWebSocketWithTimeout(url);
      connectionData.ws = ws;

      // Set up event handlers
      this.setupEventHandlers(url, ws, handlers);

      // Start heartbeat
      this.startHeartbeat(url);

      this.logger.info('WebSocket connection initiated', { connectionId, url });
      return connectionId;
    } catch (error) {
      this.logger.error('Failed to create WebSocket connection', { url, error: error.message });
      this.connections.delete(url);
      this.notifyConnectionState(url, 'failed', error);
      throw error;
    }
  }

  async createWebSocketWithTimeout(url) {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(url);
      const timeout = setTimeout(() => {
        ws.close();
        reject(new Error(`Connection timeout after ${this.config.connectionTimeout}ms`));
      }, this.config.connectionTimeout);

      ws.onopen = () => {
        clearTimeout(timeout);
        resolve(ws);
      };

      ws.onerror = (error) => {
        clearTimeout(timeout);
        reject(new Error(`WebSocket connection failed: ${error.message || 'Unknown error'}`));
      };
    });
  }

  // Set up WebSocket event handlers
  setupEventHandlers(url, ws, handlers) {
    const connection = this.connections.get(url);

    ws.onopen = (event) => {
      const connectionTime = Date.now() - connection.connectionStartTime;
      this.logger.info(`WebSocket connected successfully`, { url, connectionTime });
      
      connection.status = 'connected';
      connection.lastActivity = Date.now();
      this.reconnectAttempts.set(url, 0);
      
      this.notifyConnectionState(url, 'connected');
      
      if (handlers.onOpen) {
        try {
          handlers.onOpen(event);
        } catch (error) {
          this.logger.error('Error in onOpen handler', { url, error: error.message });
        }
      }
    };

    ws.onmessage = (event) => {
      connection.lastActivity = Date.now();
      connection.messageCount++;
      
      this.logger.debug('Message received', { url, messageCount: connection.messageCount });
      
      try {
        const data = JSON.parse(event.data);
        
        // Handle different message types
        this.handleMessage(url, data);
        
        if (handlers.onMessage) {
          handlers.onMessage(data);
        }
      } catch (error) {
        connection.errorCount++;
        this.logger.error('Failed to parse WebSocket message', { 
          url, 
          error: error.message, 
          rawData: event.data.substring(0, 200),
          errorCount: connection.errorCount 
        });
        
        if (handlers.onError) {
          handlers.onError(new Error(`Message parse error: ${error.message}`));
        }
      }
    };

    ws.onerror = (event) => {
      connection.errorCount++;
      this.logger.error(`WebSocket error occurred`, { 
        url, 
        errorCount: connection.errorCount,
        readyState: ws.readyState 
      });
      
      connection.status = 'error';
      this.notifyConnectionState(url, 'error', event);
      
      if (handlers.onError) {
        try {
          handlers.onError(event);
        } catch (error) {
          this.logger.error('Error in onError handler', { url, error: error.message });
        }
      }
    };

    ws.onclose = (event) => {
      const { code, reason, wasClean } = event;
      this.logger.info(`WebSocket closed`, { url, code, reason, wasClean });
      
      connection.status = 'closed';
      
      // Clear timers
      this.clearConnectionTimers(url);
      
      this.notifyConnectionState(url, 'closed', event);
      
      if (handlers.onClose) {
        try {
          handlers.onClose(event);
        } catch (error) {
          this.logger.error('Error in onClose handler', { url, error: error.message });
        }
      }
      
      // Attempt reconnection if not intentionally closed
      if (!wasClean && this.shouldReconnect(url)) {
        this.scheduleReconnect(url);
      } else {
        this.cleanupConnection(url);
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

  // Heartbeat handling (replaces ping/pong)
  startHeartbeat(url) {
    const connection = this.connections.get(url);
    if (!connection) {
      this.logger.warn('Cannot start heartbeat - connection not found', { url });
      return;
    }

    this.logger.debug('Starting heartbeat', { url, interval: this.config.heartbeatInterval });

    connection.heartbeatTimer = setInterval(() => {
      if (connection.status === 'connected') {
        this.sendHeartbeat(url);
      }
    }, this.config.heartbeatInterval);
  }

  sendHeartbeat(url) {
    const connection = this.connections.get(url);
    if (!connection || connection.status !== 'connected') {
      return;
    }

    try {
      connection.lastPing = Date.now();
      const heartbeatMessage = {
        type: 'ping',
        timestamp: connection.lastPing,
        connectionId: connection.id
      };
      
      connection.ws.send(JSON.stringify(heartbeatMessage));
      this.logger.debug('Heartbeat sent', { url, timestamp: connection.lastPing });
    } catch (error) {
      this.logger.error('Failed to send heartbeat', { url, error: error.message });
      // Heartbeat failure indicates connection issues
      if (connection.ws.readyState !== WebSocket.OPEN) {
        this.logger.warn('Heartbeat failed - connection not open', { url, readyState: connection.ws.readyState });
      }
    }
  }

  handlePong(url) {
    const connection = this.connections.get(url);
    if (connection && connection.lastPing) {
      const latency = Date.now() - connection.lastPing;
      this.logger.debug('Pong received', { url, latency });
      
      // Update connection health metrics
      connection.lastActivity = Date.now();
    }
  }

  // Reconnection logic
  shouldReconnect(url) {
    const attempts = this.reconnectAttempts.get(url) || 0;
    const maxAttempts = this.config.maxReconnectAttempts;
    this.logger.debug('Checking if should reconnect', { url, attempts, maxAttempts });
    return attempts < maxAttempts;
  }

  scheduleReconnect(url) {
    const connection = this.connections.get(url);
    if (!connection) {
      this.logger.warn('Cannot schedule reconnect - connection not found', { url });
      return;
    }

    const attempts = this.reconnectAttempts.get(url) || 0;
    const delayIndex = Math.min(attempts, this.config.reconnectDelays.length - 1);
    const delay = this.config.reconnectDelays[delayIndex];

    this.logger.info(`Scheduling reconnect`, { 
      url, 
      attempt: attempts + 1, 
      delay,
      maxAttempts: this.config.maxReconnectAttempts 
    });

    connection.reconnectTimer = setTimeout(async () => {
      this.reconnectAttempts.set(url, attempts + 1);
      
      try {
        // Get original parameters
        const urlObj = new URL(url);
        const params = Object.fromEntries(urlObj.searchParams);
        const endpoint = urlObj.pathname;
        
        this.logger.debug('Attempting reconnection', { url, endpoint, params });
        
        // Attempt reconnection
        await this.connect(endpoint, params, connection.handlers);
      } catch (error) {
        this.logger.error('Reconnection failed', { url, error: error.message });
        
        // Schedule next reconnect if we haven't exceeded max attempts
        if (this.shouldReconnect(url)) {
          this.scheduleReconnect(url);
        } else {
          this.logger.error('Max reconnection attempts reached', { url });
          this.cleanupConnection(url);
        }
      }
    }, delay);
  }

  // Connection state management
  notifyConnectionState(url, state, data = null) {
    this.logger.debug('Connection state changed', { url, state });
    
    const callbacks = this.connectionStateCallbacks.get(url) || [];
    callbacks.forEach(callback => {
      try {
        callback(state, data);
      } catch (error) {
        this.logger.error('Error in connection state callback', { url, error: error.message });
      }
    });
  }

  onConnectionStateChange(connectionId, callback) {
    const connection = this.findConnectionById(connectionId);
    if (!connection) {
      throw new Error(`Connection ${connectionId} not found`);
    }

    if (!this.connectionStateCallbacks.has(connection.url)) {
      this.connectionStateCallbacks.set(connection.url, []);
    }

    this.connectionStateCallbacks.get(connection.url).push(callback);

    // Return unsubscribe function
    return () => {
      const callbacks = this.connectionStateCallbacks.get(connection.url);
      const index = callbacks.indexOf(callback);
      if (index > -1) {
        callbacks.splice(index, 1);
      }
    };
  }

  // Timer management
  clearConnectionTimers(url) {
    const connection = this.connections.get(url);
    if (!connection) return;

    if (connection.heartbeatTimer) {
      clearInterval(connection.heartbeatTimer);
      connection.heartbeatTimer = null;
    }

    if (connection.reconnectTimer) {
      clearTimeout(connection.reconnectTimer);
      connection.reconnectTimer = null;
    }

    if (connection.connectionTimer) {
      clearTimeout(connection.connectionTimer);
      connection.connectionTimer = null;
    }
  }

  cleanupConnection(url) {
    this.logger.debug('Cleaning up connection', { url });
    
    this.clearConnectionTimers(url);
    this.connections.delete(url);
    this.messageHandlers.delete(url);
    this.reconnectAttempts.delete(url);
    this.connectionStateCallbacks.delete(url);
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
      status: conn.status,
      messageCount: conn.messageCount || 0,
      errorCount: conn.errorCount || 0,
      lastActivity: conn.lastActivity,
      connectionTime: conn.connectionStartTime ? Date.now() - conn.connectionStartTime : null
    }));
  }

  getConnectionStats(connectionId) {
    const connection = this.findConnectionById(connectionId);
    if (!connection) {
      return null;
    }

    return {
      id: connection.id,
      url: connection.url,
      status: connection.status,
      messageCount: connection.messageCount || 0,
      errorCount: connection.errorCount || 0,
      lastActivity: connection.lastActivity,
      connectionStartTime: connection.connectionStartTime,
      uptime: connection.connectionStartTime ? Date.now() - connection.connectionStartTime : null,
      reconnectAttempts: this.reconnectAttempts.get(connection.url) || 0,
      readyState: connection.ws ? connection.ws.readyState : null
    };
  }

  // Debug utilities
  enableDebugLogging() {
    this.config.enableDebugLogging = true;
    this.logger.info('Debug logging enabled');
  }

  disableDebugLogging() {
    this.config.enableDebugLogging = false;
    this.logger.info('Debug logging disabled');
  }

  getAllConnectionStats() {
    return {
      totalConnections: this.connections.size,
      connections: this.getActiveConnections(),
      config: this.config
    };
  }

  // Force reconnection for testing
  forceReconnect(connectionId) {
    const connection = this.findConnectionById(connectionId);
    if (!connection) {
      throw new Error(`Connection ${connectionId} not found`);
    }

    this.logger.info('Forcing reconnection', { connectionId, url: connection.url });
    
    // Close current connection to trigger reconnect
    if (connection.ws && connection.ws.readyState === WebSocket.OPEN) {
      connection.ws.close(1000, 'Force reconnect');
    }
  }
}

// Create singleton instance
export const wsService = new WebSocketService();