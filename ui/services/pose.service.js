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
    this.connectionState = 'disconnected';
    this.lastPoseData = null;
    this.performanceMetrics = {
      messageCount: 0,
      errorCount: 0,
      lastUpdateTime: null,
      averageLatency: 0,
      droppedFrames: 0
    };
    this.validationErrors = [];
    this.logger = this.createLogger();
    
    // Configuration
    this.config = {
      enableValidation: true,
      enablePerformanceTracking: true,
      maxValidationErrors: 10,
      confidenceThreshold: 0.3,
      maxPersons: 10,
      timeoutMs: 5000
    };
  }

  createLogger() {
    return {
      debug: (...args) => console.debug('[POSE-DEBUG]', new Date().toISOString(), ...args),
      info: (...args) => console.info('[POSE-INFO]', new Date().toISOString(), ...args),
      warn: (...args) => console.warn('[POSE-WARN]', new Date().toISOString(), ...args),
      error: (...args) => console.error('[POSE-ERROR]', new Date().toISOString(), ...args)
    };
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
  async startPoseStream(options = {}) {
    if (this.streamConnection) {
      this.logger.warn('Pose stream already active', { connectionId: this.streamConnection });
      return this.streamConnection;
    }

    this.logger.info('Starting pose stream', { options });
    this.resetPerformanceMetrics();

    // Validate options
    const validationResult = this.validateStreamOptions(options);
    if (!validationResult.valid) {
      throw new Error(`Invalid stream options: ${validationResult.errors.join(', ')}`);
    }

    const params = {
      zone_ids: options.zoneIds?.join(','),
      min_confidence: options.minConfidence || this.config.confidenceThreshold,
      max_fps: options.maxFps || 30,
      token: options.token || apiService.authToken
    };

    // Remove undefined values
    Object.keys(params).forEach(key => 
      params[key] === undefined && delete params[key]
    );

    try {
      this.connectionState = 'connecting';
      this.notifyConnectionState('connecting');

      this.streamConnection = await wsService.connect(
        API_CONFIG.ENDPOINTS.STREAM.WS_POSE,
        params,
        {
          onOpen: (event) => {
            this.logger.info('Pose stream connected successfully');
            this.connectionState = 'connected';
            this.notifyConnectionState('connected');
            this.notifyPoseSubscribers({ type: 'connected', event });
          },
          onMessage: (data) => {
            this.handlePoseMessage(data);
          },
          onError: (error) => {
            this.logger.error('Pose stream error occurred', { error });
            this.connectionState = 'error';
            this.performanceMetrics.errorCount++;
            this.notifyConnectionState('error', error);
            this.notifyPoseSubscribers({ type: 'error', error });
          },
          onClose: (event) => {
            this.logger.info('Pose stream disconnected', { event });
            this.connectionState = 'disconnected';
            this.streamConnection = null;
            this.notifyConnectionState('disconnected', event);
            this.notifyPoseSubscribers({ type: 'disconnected', event });
          }
        }
      );

      // Set up connection state monitoring
      if (this.streamConnection) {
        this.setupConnectionStateMonitoring();
      }

      this.logger.info('Pose stream initiated', { connectionId: this.streamConnection });
      return this.streamConnection;
    } catch (error) {
      this.logger.error('Failed to start pose stream', { error: error.message });
      this.connectionState = 'failed';
      this.notifyConnectionState('failed', error);
      throw error;
    }
  }

  validateStreamOptions(options) {
    const errors = [];

    if (options.zoneIds && !Array.isArray(options.zoneIds)) {
      errors.push('zoneIds must be an array');
    }

    if (options.minConfidence !== undefined) {
      if (typeof options.minConfidence !== 'number' || options.minConfidence < 0 || options.minConfidence > 1) {
        errors.push('minConfidence must be a number between 0 and 1');
      }
    }

    if (options.maxFps !== undefined) {
      if (typeof options.maxFps !== 'number' || options.maxFps <= 0 || options.maxFps > 60) {
        errors.push('maxFps must be a number between 1 and 60');
      }
    }

    return {
      valid: errors.length === 0,
      errors
    };
  }

  setupConnectionStateMonitoring() {
    if (!this.streamConnection) return;

    // Monitor connection state changes
    wsService.onConnectionStateChange(this.streamConnection, (state, data) => {
      this.logger.debug('WebSocket connection state changed', { state, data });
      this.connectionState = state;
      this.notifyConnectionState(state, data);
    });
  }

  notifyConnectionState(state, data = null) {
    this.logger.debug('Connection state notification', { state, data });
    this.notifyPoseSubscribers({ 
      type: 'connection_state', 
      state, 
      data,
      metrics: this.getPerformanceMetrics() 
    });
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
    const startTime = performance.now();
    this.performanceMetrics.messageCount++;
    
    this.logger.debug('Received pose message', { 
      type: data.type, 
      messageCount: this.performanceMetrics.messageCount 
    });
    
    try {
      // Validate message structure
      if (this.config.enableValidation) {
        const validationResult = this.validatePoseMessage(data);
        if (!validationResult.valid) {
          this.addValidationError(`Invalid message structure: ${validationResult.errors.join(', ')}`);
          return;
        }
      }

      const { type, payload, data: messageData, zone_id, timestamp } = data;
      
      // Handle both payload (old format) and data (new format) properties
      const actualData = payload || messageData;

      // Update performance metrics
      if (this.config.enablePerformanceTracking) {
        this.updatePerformanceMetrics(startTime, timestamp);
      }

      switch (type) {
        case 'connection_established':
          this.logger.info('WebSocket connection established');
          this.notifyPoseSubscribers({
            type: 'connected',
            data: { status: 'connected' }
          });
          break;

        case 'pose_data':
          this.logger.debug('Processing pose data', { zone_id, hasData: !!actualData });
          
          // Validate pose data
          if (this.config.enableValidation && actualData) {
            const poseValidation = this.validatePoseData(actualData);
            if (!poseValidation.valid) {
              this.addValidationError(`Invalid pose data: ${poseValidation.errors.join(', ')}`);
              return;
            }
          }
          
          // Convert zone-based WebSocket format to REST API format
          const convertedData = this.convertZoneDataToRestFormat(actualData, zone_id, data);
          this.lastPoseData = convertedData;
          
          this.logger.debug('Converted pose data', { 
            personsCount: convertedData.persons?.length || 0,
            zones: Object.keys(convertedData.zone_summary || {})
          });
          
          this.notifyPoseSubscribers({
            type: 'pose_update',
            data: convertedData
          });
          break;

        case 'historical_data':
          this.logger.debug('Historical data received');
          this.notifyPoseSubscribers({
            type: 'historical_update',
            data: actualData
          });
          break;

        case 'zone_statistics':
          this.logger.debug('Zone statistics received');
          this.notifyPoseSubscribers({
            type: 'zone_stats',
            data: actualData
          });
          break;

        case 'system_event':
          this.logger.debug('System event received');
          this.notifyPoseSubscribers({
            type: 'system_event',
            data: actualData
          });
          break;

        case 'pong':
          // Handle heartbeat response
          this.logger.debug('Heartbeat response received');
          break;

        default:
          this.logger.warn('Unknown pose message type', { type, data });
          this.notifyPoseSubscribers({
            type: 'unknown_message',
            data: { originalType: type, originalData: data }
          });
      }
    } catch (error) {
      this.logger.error('Error handling pose message', { error: error.message, data });
      this.performanceMetrics.errorCount++;
      this.addValidationError(`Message handling error: ${error.message}`);
      
      this.notifyPoseSubscribers({
        type: 'error',
        error: error,
        data: { originalMessage: data }
      });
    }
  }

  validatePoseMessage(message) {
    const errors = [];

    if (!message || typeof message !== 'object') {
      errors.push('Message must be an object');
      return { valid: false, errors };
    }

    if (!message.type || typeof message.type !== 'string') {
      errors.push('Message must have a valid type string');
    }

    return {
      valid: errors.length === 0,
      errors
    };
  }

  validatePoseData(poseData) {
    const errors = [];

    if (!poseData || typeof poseData !== 'object') {
      errors.push('Pose data must be an object');
      return { valid: false, errors };
    }

    if (poseData.pose && poseData.pose.persons) {
      const persons = poseData.pose.persons;
      if (!Array.isArray(persons)) {
        errors.push('Persons must be an array');
      } else if (persons.length > this.config.maxPersons) {
        errors.push(`Too many persons detected (${persons.length} > ${this.config.maxPersons})`);
      }

      // Validate person data
      persons.forEach((person, index) => {
        if (!person || typeof person !== 'object') {
          errors.push(`Person ${index} must be an object`);
        } else {
          if (person.confidence !== undefined && 
              (typeof person.confidence !== 'number' || person.confidence < 0 || person.confidence > 1)) {
            errors.push(`Person ${index} confidence must be between 0 and 1`);
          }
        }
      });
    }

    return {
      valid: errors.length === 0,
      errors
    };
  }

  updatePerformanceMetrics(startTime, messageTimestamp) {
    const processingTime = performance.now() - startTime;
    this.performanceMetrics.lastUpdateTime = Date.now();

    // Calculate latency if timestamp is provided
    if (messageTimestamp) {
      const messageTime = new Date(messageTimestamp).getTime();
      const currentTime = Date.now();
      const latency = currentTime - messageTime;
      
      // Update average latency (simple moving average)
      if (this.performanceMetrics.averageLatency === 0) {
        this.performanceMetrics.averageLatency = latency;
      } else {
        this.performanceMetrics.averageLatency = 
          (this.performanceMetrics.averageLatency * 0.9) + (latency * 0.1);
      }
    }
  }

  addValidationError(error) {
    this.validationErrors.push({
      error,
      timestamp: Date.now(),
      messageCount: this.performanceMetrics.messageCount
    });

    // Keep only recent errors
    if (this.validationErrors.length > this.config.maxValidationErrors) {
      this.validationErrors = this.validationErrors.slice(-this.config.maxValidationErrors);
    }

    this.logger.warn('Validation error', { error });
  }

  resetPerformanceMetrics() {
    this.performanceMetrics = {
      messageCount: 0,
      errorCount: 0,
      lastUpdateTime: null,
      averageLatency: 0,
      droppedFrames: 0
    };
    this.validationErrors = [];
    this.logger.debug('Performance metrics reset');
  }

  getPerformanceMetrics() {
    return {
      ...this.performanceMetrics,
      validationErrors: this.validationErrors.length,
      connectionState: this.connectionState
    };
  }

  // Convert zone-based WebSocket data to REST API format
  convertZoneDataToRestFormat(zoneData, zoneId, originalMessage) {
    console.log('🔧 Converting zone data:', { zoneData, zoneId, originalMessage });
    
    if (!zoneData || !zoneData.pose) {
      console.log('⚠️ No pose data in zone data, returning empty result');
      return {
        timestamp: originalMessage.timestamp || new Date().toISOString(),
        frame_id: `ws_frame_${Date.now()}`,
        persons: [],
        zone_summary: {},
        processing_time_ms: 0,
        metadata: { mock_data: false, source: 'websocket' }
      };
    }

    // Extract persons from zone data
    const persons = zoneData.pose.persons || [];
    console.log('👥 Extracted persons:', persons);
    
    // Create zone summary
    const zoneSummary = {};
    if (zoneId && persons.length > 0) {
      zoneSummary[zoneId] = persons.length;
    }
    console.log('📍 Zone summary:', zoneSummary);

    const result = {
      timestamp: originalMessage.timestamp || new Date().toISOString(),
      frame_id: zoneData.metadata?.frame_id || `ws_frame_${Date.now()}`,
      persons: persons,
      zone_summary: zoneSummary,
      processing_time_ms: zoneData.metadata?.processing_time_ms || 0,
      metadata: {
        mock_data: false,
        source: 'websocket',
        zone_id: zoneId,
        confidence: zoneData.confidence,
        activity: zoneData.activity
      }
    };
    
    console.log('✅ Final converted result:', result);
    return result;
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

  // Utility methods
  getConnectionState() {
    return this.connectionState;
  }

  getLastPoseData() {
    return this.lastPoseData;
  }

  getValidationErrors() {
    return [...this.validationErrors];
  }

  clearValidationErrors() {
    this.validationErrors = [];
    this.logger.info('Validation errors cleared');
  }

  updateConfig(newConfig) {
    this.config = { ...this.config, ...newConfig };
    this.logger.info('Configuration updated', { config: this.config });
  }

  // Health check
  async healthCheck() {
    try {
      const stats = await this.getStats(1);
      return {
        healthy: true,
        connectionState: this.connectionState,
        lastUpdate: this.performanceMetrics.lastUpdateTime,
        messageCount: this.performanceMetrics.messageCount,
        errorCount: this.performanceMetrics.errorCount,
        apiHealthy: !!stats
      };
    } catch (error) {
      return {
        healthy: false,
        error: error.message,
        connectionState: this.connectionState,
        lastUpdate: this.performanceMetrics.lastUpdateTime
      };
    }
  }

  // Force reconnection
  async reconnectStream() {
    if (!this.streamConnection) {
      throw new Error('No active stream connection to reconnect');
    }

    this.logger.info('Forcing stream reconnection');
    
    // Get current connection stats to preserve options
    const stats = wsService.getConnectionStats(this.streamConnection);
    if (!stats) {
      throw new Error('Cannot get connection stats for reconnection');
    }

    // Extract original options from URL parameters
    const url = new URL(stats.url);
    const params = Object.fromEntries(url.searchParams);
    
    const options = {
      zoneIds: params.zone_ids ? params.zone_ids.split(',') : undefined,
      minConfidence: params.min_confidence ? parseFloat(params.min_confidence) : undefined,
      maxFps: params.max_fps ? parseInt(params.max_fps) : undefined,
      token: params.token
    };

    // Stop current stream
    this.stopPoseStream();

    // Start new stream with same options
    return this.startPoseStream(options);
  }

  // Clean up
  dispose() {
    this.logger.info('Disposing pose service');
    this.stopPoseStream();
    this.stopEventStream();
    this.poseSubscribers = [];
    this.eventSubscribers = [];
    this.connectionState = 'disconnected';
    this.lastPoseData = null;
    this.resetPerformanceMetrics();
  }
}

// Create singleton instance
export const poseService = new PoseService();