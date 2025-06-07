// API Configuration for WiFi-DensePose UI

export const API_CONFIG = {
  BASE_URL: 'http://localhost:8000',  // FastAPI backend port
  API_VERSION: '/api/v1',
  WS_PREFIX: 'ws://',
  WSS_PREFIX: 'wss://',
  
  // Mock server configuration (only for testing)
  MOCK_SERVER: {
    ENABLED: false,  // Set to true only for testing without backend
    AUTO_DETECT: true,  // Automatically detect if backend is available
  },
  
  // API Endpoints
  ENDPOINTS: {
    // Root & Info
    ROOT: '/',
    INFO: '/api/v1/info',
    STATUS: '/api/v1/status',
    METRICS: '/api/v1/metrics',
    
    // Health
    HEALTH: {
      SYSTEM: '/health/health',
      READY: '/health/ready',
      LIVE: '/health/live',
      METRICS: '/health/metrics',
      VERSION: '/health/version'
    },
    
    // Pose
    POSE: {
      CURRENT: '/api/v1/pose/current',
      ANALYZE: '/api/v1/pose/analyze',
      ZONE_OCCUPANCY: '/api/v1/pose/zones/{zone_id}/occupancy',
      ZONES_SUMMARY: '/api/v1/pose/zones/summary',
      HISTORICAL: '/api/v1/pose/historical',
      ACTIVITIES: '/api/v1/pose/activities',
      CALIBRATE: '/api/v1/pose/calibrate',
      CALIBRATION_STATUS: '/api/v1/pose/calibration/status',
      STATS: '/api/v1/pose/stats'
    },
    
    // Streaming
    STREAM: {
      STATUS: '/api/v1/stream/status',
      START: '/api/v1/stream/start',
      STOP: '/api/v1/stream/stop',
      CLIENTS: '/api/v1/stream/clients',
      DISCONNECT_CLIENT: '/api/v1/stream/clients/{client_id}',
      BROADCAST: '/api/v1/stream/broadcast',
      METRICS: '/api/v1/stream/metrics',
      // WebSocket endpoints
      WS_POSE: '/api/v1/stream/pose',
      WS_EVENTS: '/api/v1/stream/events'
    },
    
    // Development (only in dev mode)
    DEV: {
      CONFIG: '/api/v1/dev/config',
      RESET: '/api/v1/dev/reset'
    }
  },
  
  // Default request options
  DEFAULT_HEADERS: {
    'Content-Type': 'application/json',
    'Accept': 'application/json'
  },
  
  // Rate limiting
  RATE_LIMITS: {
    REQUESTS_PER_MINUTE: 60,
    BURST_LIMIT: 10
  },
  
  // WebSocket configuration
  WS_CONFIG: {
    RECONNECT_DELAY: 5000,
    MAX_RECONNECT_ATTEMPTS: 5,
    PING_INTERVAL: 30000,
    MESSAGE_TIMEOUT: 10000
  }
};

// Helper function to build API URLs
export function buildApiUrl(endpoint, params = {}) {
  let url = `${API_CONFIG.BASE_URL}${endpoint}`;
  
  // Replace path parameters
  Object.keys(params).forEach(key => {
    if (url.includes(`{${key}}`)) {
      url = url.replace(`{${key}}`, params[key]);
      delete params[key];
    }
  });
  
  // Add query parameters
  const queryParams = new URLSearchParams(params);
  if (queryParams.toString()) {
    url += `?${queryParams.toString()}`;
  }
  
  return url;
}

// Helper function to build WebSocket URLs
export function buildWsUrl(endpoint, params = {}) {
  const protocol = window.location.protocol === 'https:' 
    ? API_CONFIG.WSS_PREFIX 
    : API_CONFIG.WS_PREFIX;
  
  // Use localhost:8000 for WebSocket connections to match FastAPI backend
  const host = 'localhost:8000';
  let url = `${protocol}${host}${endpoint}`;
  
  // Add query parameters
  const queryParams = new URLSearchParams(params);
  if (queryParams.toString()) {
    url += `?${queryParams.toString()}`;
  }
  
  return url;
}