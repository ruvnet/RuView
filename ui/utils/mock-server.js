// Mock Server for Testing WiFi DensePose UI

export class MockServer {
  constructor() {
    this.endpoints = new Map();
    this.websockets = new Set();
    this.isRunning = false;
    this.setupDefaultEndpoints();
  }

  // Set up default mock endpoints
  setupDefaultEndpoints() {
    // Health endpoints
    this.addEndpoint('GET', '/health/health', () => ({
      status: 'healthy',
      timestamp: new Date().toISOString(),
      components: {
        api: { status: 'healthy', message: 'API server running' },
        hardware: { status: 'healthy', message: 'Hardware connected' },
        inference: { status: 'healthy', message: 'Inference engine running' },
        streaming: { status: 'healthy', message: 'Streaming service active' }
      },
      metrics: {
        cpu_percent: Math.random() * 30 + 10,
        memory_percent: Math.random() * 40 + 20,
        disk_percent: Math.random() * 20 + 5,
        uptime: Math.floor(Date.now() / 1000) - 3600
      }
    }));

    this.addEndpoint('GET', '/health/ready', () => ({
      status: 'ready',
      checks: {
        database: 'ready',
        hardware: 'ready',
        inference: 'ready'
      }
    }));

    this.addEndpoint('GET', '/health/live', () => ({
      status: 'alive',
      timestamp: new Date().toISOString()
    }));

    this.addEndpoint('GET', '/health/version', () => ({
      name: 'WiFi-DensePose API',
      version: '1.0.0',
      environment: 'development',
      build: '2025-01-07-dev'
    }));

    // API info endpoints
    this.addEndpoint('GET', '/', () => ({
      name: 'WiFi-DensePose API',
      version: '1.0.0',
      environment: 'development',
      features: {
        pose_estimation: true,
        streaming: true,
        authentication: false,
        rate_limiting: true,
        metrics: true
      },
      endpoints: [
        '/health',
        '/api/v1/pose',
        '/api/v1/stream'
      ]
    }));

    this.addEndpoint('GET', '/api/v1/info', () => ({
      name: 'WiFi-DensePose API',
      version: '1.0.0',
      environment: 'development',
      zones: ['zone1', 'zone2', 'living-room'],
      routers: ['router-001', 'router-002'],
      features: {
        pose_estimation: true,
        streaming: true,
        multi_zone: true,
        real_time: true
      },
      rate_limits: {
        requests_per_minute: 60,
        burst: 10
      }
    }));

    this.addEndpoint('GET', '/api/v1/status', () => ({
      services: {
        api: 'running',
        hardware: 'connected',
        inference: 'ready',
        streaming: Math.random() > 0.5 ? 'active' : 'idle'
      },
      streaming: {
        active_connections: Math.floor(Math.random() * 5),
        total_messages: Math.floor(Math.random() * 1000),
        uptime: Math.floor(Date.now() / 1000) - 1800
      }
    }));

    // Pose endpoints
    this.addEndpoint('GET', '/api/v1/pose/current', () => ({
      timestamp: new Date().toISOString(),
      total_persons: Math.floor(Math.random() * 3),
      persons: this.generateMockPersons(Math.floor(Math.random() * 3)),
      processing_time: Math.random() * 20 + 5,
      zone_id: 'living-room'
    }));

    this.addEndpoint('GET', '/api/v1/pose/zones/summary', () => ({
      total_persons: Math.floor(Math.random() * 5),
      zones: {
        'zone1': { person_count: Math.floor(Math.random() * 2), name: 'Living Room' },
        'zone2': { person_count: Math.floor(Math.random() * 2), name: 'Kitchen' },
        'zone3': { person_count: Math.floor(Math.random() * 2), name: 'Bedroom' }
      }
    }));

    this.addEndpoint('GET', '/api/v1/pose/stats', () => ({
      total_detections: Math.floor(Math.random() * 10000),
      average_confidence: Math.random() * 0.4 + 0.6,
      peak_persons: Math.floor(Math.random() * 5) + 1,
      hours_analyzed: 24
    }));

    // Stream endpoints
    this.addEndpoint('GET', '/api/v1/stream/status', () => ({
      is_active: Math.random() > 0.3,
      connected_clients: Math.floor(Math.random() * 10),
      messages_sent: Math.floor(Math.random() * 5000),
      uptime: Math.floor(Date.now() / 1000) - 900
    }));

    this.addEndpoint('POST', '/api/v1/stream/start', () => ({
      message: 'Streaming started',
      status: 'active'
    }));

    this.addEndpoint('POST', '/api/v1/stream/stop', () => ({
      message: 'Streaming stopped',
      status: 'inactive'
    }));
  }

  // Generate mock person data
  generateMockPersons(count) {
    const persons = [];
    for (let i = 0; i < count; i++) {
      persons.push({
        person_id: `person_${i}`,
        confidence: Math.random() * 0.3 + 0.7,
        bounding_box: {
          x: Math.random() * 400,
          y: Math.random() * 300,
          width: Math.random() * 100 + 50,
          height: Math.random() * 150 + 100
        },
        keypoints: this.generateMockKeypoints(),
        zone_id: `zone${Math.floor(Math.random() * 3) + 1}`
      });
    }
    return persons;
  }

  // Generate mock keypoints (COCO format)
  generateMockKeypoints() {
    const keypoints = [];
    for (let i = 0; i < 17; i++) {
      keypoints.push({
        x: (Math.random() - 0.5) * 2, // Normalized coordinates
        y: (Math.random() - 0.5) * 2,
        confidence: Math.random() * 0.5 + 0.5
      });
    }
    return keypoints;
  }

  // Add a mock endpoint
  addEndpoint(method, path, handler) {
    const key = `${method.toUpperCase()} ${path}`;
    this.endpoints.set(key, handler);
  }

  // Start the mock server
  start() {
    if (this.isRunning) return;
    
    this.isRunning = true;
    this.interceptFetch();
    this.interceptWebSocket();
    console.log('Mock server started');
  }

  // Stop the mock server
  stop() {
    if (!this.isRunning) return;
    
    this.isRunning = false;
    this.restoreFetch();
    this.restoreWebSocket();
    console.log('Mock server stopped');
  }

  // Intercept fetch requests
  interceptFetch() {
    this.originalFetch = window.fetch;
    
    window.fetch = async (url, options = {}) => {
      if (!this.isRunning) {
        return this.originalFetch(url, options);
      }

      const method = options.method || 'GET';
      const path = new URL(url, window.location.origin).pathname;
      const key = `${method.toUpperCase()} ${path}`;
      
      if (this.endpoints.has(key)) {
        const handler = this.endpoints.get(key);
        const delay = Math.random() * 100 + 50; // Simulate network delay
        
        await new Promise(resolve => setTimeout(resolve, delay));
        
        try {
          const data = handler(options);
          return new Response(JSON.stringify(data), {
            status: 200,
            headers: { 'Content-Type': 'application/json' }
          });
        } catch (error) {
          return new Response(JSON.stringify({ error: error.message }), {
            status: 500,
            headers: { 'Content-Type': 'application/json' }
          });
        }
      }
      
      // If no mock endpoint, fall back to original fetch
      return this.originalFetch(url, options);
    };
  }

  // Restore original fetch
  restoreFetch() {
    if (this.originalFetch) {
      window.fetch = this.originalFetch;
    }
  }

  // Intercept WebSocket connections
  interceptWebSocket() {
    this.originalWebSocket = window.WebSocket;
    
    window.WebSocket = class MockWebSocket extends EventTarget {
      constructor(url, protocols) {
        super();
        this.url = url;
        this.protocols = protocols;
        this.readyState = WebSocket.CONNECTING;
        this.bufferedAmount = 0;
        
        // Simulate connection
        setTimeout(() => {
          this.readyState = WebSocket.OPEN;
          this.dispatchEvent(new Event('open'));
          
          // Start sending mock data
          this.startMockData();
        }, 100);
      }
      
      send(data) {
        if (this.readyState !== WebSocket.OPEN) {
          throw new Error('WebSocket is not open');
        }
        
        // Echo back or handle specific commands
        try {
          const message = JSON.parse(data);
          if (message.type === 'ping') {
            setTimeout(() => {
              this.dispatchEvent(new MessageEvent('message', {
                data: JSON.stringify({ type: 'pong' })
              }));
            }, 10);
          }
        } catch (e) {
          // Not JSON, ignore
        }
      }
      
      close(code = 1000, reason = '') {
        this.readyState = WebSocket.CLOSING;
        setTimeout(() => {
          this.readyState = WebSocket.CLOSED;
          this.dispatchEvent(new CloseEvent('close', { code, reason, wasClean: true }));
        }, 50);
      }
      
      startMockData() {
        // Send connection established message
        setTimeout(() => {
          this.dispatchEvent(new MessageEvent('message', {
            data: JSON.stringify({
              type: 'connection_established',
              payload: { client_id: 'mock-client-123' }
            })
          }));
        }, 50);
        
        // Send periodic pose data if this is a pose stream
        if (this.url.includes('/stream/pose')) {
          this.poseInterval = setInterval(() => {
            if (this.readyState === WebSocket.OPEN) {
              this.dispatchEvent(new MessageEvent('message', {
                data: JSON.stringify({
                  type: 'pose_data',
                  payload: {
                    timestamp: new Date().toISOString(),
                    persons: mockServer.generateMockPersons(Math.floor(Math.random() * 3)),
                    processing_time: Math.random() * 20 + 5
                  }
                })
              }));
            }
          }, 1000);
        }
        
        // Send periodic events if this is an event stream
        if (this.url.includes('/stream/events')) {
          this.eventInterval = setInterval(() => {
            if (this.readyState === WebSocket.OPEN && Math.random() > 0.7) {
              this.dispatchEvent(new MessageEvent('message', {
                data: JSON.stringify({
                  type: 'system_event',
                  payload: {
                    event_type: 'zone_entry',
                    zone_id: 'zone1',
                    person_id: 'person_0',
                    timestamp: new Date().toISOString()
                  }
                })
              }));
            }
          }, 2000);
        }
      }
    };
    
    // Copy static properties
    window.WebSocket.CONNECTING = 0;
    window.WebSocket.OPEN = 1;
    window.WebSocket.CLOSING = 2;
    window.WebSocket.CLOSED = 3;
  }

  // Restore original WebSocket
  restoreWebSocket() {
    if (this.originalWebSocket) {
      window.WebSocket = this.originalWebSocket;
    }
  }

  // Add a custom response
  addCustomResponse(method, path, response) {
    this.addEndpoint(method, path, () => response);
  }

  // Simulate server error
  simulateError(method, path, status = 500, message = 'Internal Server Error') {
    this.addEndpoint(method, path, () => {
      throw new Error(message);
    });
  }

  // Simulate slow response
  addSlowEndpoint(method, path, handler, delay = 2000) {
    this.addEndpoint(method, path, async (...args) => {
      await new Promise(resolve => setTimeout(resolve, delay));
      return handler(...args);
    });
  }
}

// Create and export mock server instance
export const mockServer = new MockServer();