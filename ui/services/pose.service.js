// Pose Service for WiFi-DensePose UI

import { API_CONFIG } from '../config/api.config.js';
import { apiService } from './api.service.js';
import { wsService } from './websocket.service.js';

export class PoseService {
  constructor() {
    this.streamConnection = null;
    this.eventConnection = null;
    this.poseSubscribers = [];
    this.eventSubscribers = [];
  }

  // Get current pose estimation
  async getCurrentPose(options = {}) {
    const params = {
      zone_ids: options.zoneIds?.join(','),
      confidence_threshold: options.confidenceThreshold,
      max_persons: options.maxPersons,
      include_keypoints: options.includeKeypoints,
      include_segmentation: options.includeSegmentation
    };

    // Remove undefined values
    Object.keys(params).forEach(key => 
      params[key] === undefined && delete params[key]
    );

    return apiService.get(API_CONFIG.ENDPOINTS.POSE.CURRENT, params);
  }

  // Analyze pose (requires auth)
  async analyzePose(request) {
    return apiService.post(API_CONFIG.ENDPOINTS.POSE.ANALYZE, request);
  }

  // Get zone occupancy
  async getZoneOccupancy(zoneId) {
    const endpoint = API_CONFIG.ENDPOINTS.POSE.ZONE_OCCUPANCY.replace('{zone_id}', zoneId);
    return apiService.get(endpoint);
  }

  // Get zones summary
  async getZonesSummary() {
    return apiService.get(API_CONFIG.ENDPOINTS.POSE.ZONES_SUMMARY);
  }

  // Get historical data (requires auth)
  async getHistoricalData(request) {
    return apiService.post(API_CONFIG.ENDPOINTS.POSE.HISTORICAL, request);
  }

  // Get recent activities
  async getActivities(options = {}) {
    const params = {
      zone_id: options.zoneId,
      limit: options.limit || 50
    };

    // Remove undefined values
    Object.keys(params).forEach(key => 
      params[key] === undefined && delete params[key]
    );

    return apiService.get(API_CONFIG.ENDPOINTS.POSE.ACTIVITIES, params);
  }

  // Calibrate system (requires auth)
  async calibrate() {
    return apiService.post(API_CONFIG.ENDPOINTS.POSE.CALIBRATE);
  }

  // Get calibration status (requires auth)
  async getCalibrationStatus() {
    return apiService.get(API_CONFIG.ENDPOINTS.POSE.CALIBRATION_STATUS);
  }

  // Get pose statistics
  async getStats(hours = 24) {
    return apiService.get(API_CONFIG.ENDPOINTS.POSE.STATS, { hours });
  }

  // Start pose stream
  startPoseStream(options = {}) {
    if (this.streamConnection) {
      console.warn('Pose stream already active');
      return this.streamConnection;
    }

    const params = {
      zone_ids: options.zoneIds?.join(','),
      min_confidence: options.minConfidence || 0.5,
      max_fps: options.maxFps || 30,
      token: options.token || apiService.authToken
    };

    // Remove undefined values
    Object.keys(params).forEach(key => 
      params[key] === undefined && delete params[key]
    );

    this.streamConnection = wsService.connect(
      API_CONFIG.ENDPOINTS.STREAM.WS_POSE,
      params,
      {
        onOpen: () => {
          console.log('Pose stream connected');
          this.notifyPoseSubscribers({ type: 'connected' });
        },
        onMessage: (data) => {
          this.handlePoseMessage(data);
        },
        onError: (error) => {
          console.error('Pose stream error:', error);
          this.notifyPoseSubscribers({ type: 'error', error });
        },
        onClose: () => {
          console.log('Pose stream disconnected');
          this.streamConnection = null;
          this.notifyPoseSubscribers({ type: 'disconnected' });
        }
      }
    );

    return this.streamConnection;
  }

  // Stop pose stream
  stopPoseStream() {
    if (this.streamConnection) {
      wsService.disconnect(this.streamConnection);
      this.streamConnection = null;
    }
  }

  // Subscribe to pose updates
  subscribeToPoseUpdates(callback) {
    this.poseSubscribers.push(callback);
    
    // Return unsubscribe function
    return () => {
      const index = this.poseSubscribers.indexOf(callback);
      if (index > -1) {
        this.poseSubscribers.splice(index, 1);
      }
    };
  }

  // Handle pose stream messages
  handlePoseMessage(data) {
    const { type, payload } = data;

    switch (type) {
      case 'pose_data':
        this.notifyPoseSubscribers({
          type: 'pose_update',
          data: payload
        });
        break;

      case 'historical_data':
        this.notifyPoseSubscribers({
          type: 'historical_update',
          data: payload
        });
        break;

      case 'zone_statistics':
        this.notifyPoseSubscribers({
          type: 'zone_stats',
          data: payload
        });
        break;

      case 'system_event':
        this.notifyPoseSubscribers({
          type: 'system_event',
          data: payload
        });
        break;

      default:
        console.log('Unknown pose message type:', type);
    }
  }

  // Notify pose subscribers
  notifyPoseSubscribers(update) {
    this.poseSubscribers.forEach(callback => {
      try {
        callback(update);
      } catch (error) {
        console.error('Error in pose subscriber:', error);
      }
    });
  }

  // Start event stream
  startEventStream(options = {}) {
    if (this.eventConnection) {
      console.warn('Event stream already active');
      return this.eventConnection;
    }

    const params = {
      event_types: options.eventTypes?.join(','),
      zone_ids: options.zoneIds?.join(','),
      token: options.token || apiService.authToken
    };

    // Remove undefined values
    Object.keys(params).forEach(key => 
      params[key] === undefined && delete params[key]
    );

    this.eventConnection = wsService.connect(
      API_CONFIG.ENDPOINTS.STREAM.WS_EVENTS,
      params,
      {
        onOpen: () => {
          console.log('Event stream connected');
          this.notifyEventSubscribers({ type: 'connected' });
        },
        onMessage: (data) => {
          this.handleEventMessage(data);
        },
        onError: (error) => {
          console.error('Event stream error:', error);
          this.notifyEventSubscribers({ type: 'error', error });
        },
        onClose: () => {
          console.log('Event stream disconnected');
          this.eventConnection = null;
          this.notifyEventSubscribers({ type: 'disconnected' });
        }
      }
    );

    return this.eventConnection;
  }

  // Stop event stream
  stopEventStream() {
    if (this.eventConnection) {
      wsService.disconnect(this.eventConnection);
      this.eventConnection = null;
    }
  }

  // Subscribe to events
  subscribeToEvents(callback) {
    this.eventSubscribers.push(callback);
    
    // Return unsubscribe function
    return () => {
      const index = this.eventSubscribers.indexOf(callback);
      if (index > -1) {
        this.eventSubscribers.splice(index, 1);
      }
    };
  }

  // Handle event stream messages
  handleEventMessage(data) {
    this.notifyEventSubscribers({
      type: 'event',
      data
    });
  }

  // Notify event subscribers
  notifyEventSubscribers(update) {
    this.eventSubscribers.forEach(callback => {
      try {
        callback(update);
      } catch (error) {
        console.error('Error in event subscriber:', error);
      }
    });
  }

  // Update stream configuration
  updateStreamConfig(connectionId, config) {
    wsService.sendCommand(connectionId, 'update_config', config);
  }

  // Get stream status
  requestStreamStatus(connectionId) {
    wsService.sendCommand(connectionId, 'get_status');
  }

  // Clean up
  dispose() {
    this.stopPoseStream();
    this.stopEventStream();
    this.poseSubscribers = [];
    this.eventSubscribers = [];
  }
}

// Create singleton instance
export const poseService = new PoseService();