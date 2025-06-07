// Stream Service for WiFi-DensePose UI

import { API_CONFIG } from '../config/api.config.js';
import { apiService } from './api.service.js';

export class StreamService {
  // Get streaming status
  async getStatus() {
    return apiService.get(API_CONFIG.ENDPOINTS.STREAM.STATUS);
  }

  // Start streaming (requires auth)
  async start() {
    return apiService.post(API_CONFIG.ENDPOINTS.STREAM.START);
  }

  // Stop streaming (requires auth)
  async stop() {
    return apiService.post(API_CONFIG.ENDPOINTS.STREAM.STOP);
  }

  // Get connected clients (requires auth)
  async getClients() {
    return apiService.get(API_CONFIG.ENDPOINTS.STREAM.CLIENTS);
  }

  // Disconnect a client (requires auth)
  async disconnectClient(clientId) {
    const endpoint = API_CONFIG.ENDPOINTS.STREAM.DISCONNECT_CLIENT.replace('{client_id}', clientId);
    return apiService.delete(endpoint);
  }

  // Broadcast message (requires auth)
  async broadcast(message, options = {}) {
    const params = {
      stream_type: options.streamType,
      zone_ids: options.zoneIds?.join(',')
    };

    // Remove undefined values
    Object.keys(params).forEach(key => 
      params[key] === undefined && delete params[key]
    );

    return apiService.post(
      API_CONFIG.ENDPOINTS.STREAM.BROADCAST, 
      message,
      { params }
    );
  }

  // Get streaming metrics
  async getMetrics() {
    return apiService.get(API_CONFIG.ENDPOINTS.STREAM.METRICS);
  }
}

// Create singleton instance
export const streamService = new StreamService();