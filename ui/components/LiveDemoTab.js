// Live Demo Tab Component - Enhanced Version

import { PoseDetectionCanvas } from './PoseDetectionCanvas.js';
import { poseService } from '../services/pose.service.js';
import { streamService } from '../services/stream.service.js';
import { wsService } from '../services/websocket.service.js';

export class LiveDemoTab {
  constructor(containerElement) {
    this.container = containerElement;
    this.state = {
      isActive: false,
      connectionState: 'disconnected',
      currentZone: 'zone_1',
      debugMode: false,
      autoReconnect: true,
      renderMode: 'skeleton'
    };
    
    this.components = {
      poseCanvas: null,
      settingsPanel: null
    };
    
    this.metrics = {
      startTime: null,
      frameCount: 0,
      errorCount: 0,
      lastUpdate: null,
      connectionAttempts: 0
    };
    
    this.subscriptions = [];
    this.logger = this.createLogger();
    
    // Configuration
    this.config = {
      defaultZone: 'zone_1',
      reconnectDelay: 3000,
      healthCheckInterval: 10000,
      maxConnectionAttempts: 5,
      enablePerformanceMonitoring: true
    };
  }

  createLogger() {
    return {
      debug: (...args) => console.debug('[LIVEDEMO-DEBUG]', new Date().toISOString(), ...args),
      info: (...args) => console.info('[LIVEDEMO-INFO]', new Date().toISOString(), ...args),
      warn: (...args) => console.warn('[LIVEDEMO-WARN]', new Date().toISOString(), ...args),
      error: (...args) => console.error('[LIVEDEMO-ERROR]', new Date().toISOString(), ...args)
    };
  }

  // Initialize component
  async init() {
    try {
      this.logger.info('Initializing LiveDemoTab component');
      
      // Create enhanced DOM structure
      this.createEnhancedStructure();
      
      // Initialize pose detection canvas
      this.initializePoseCanvas();
      
      // Set up controls and event handlers
      this.setupEnhancedControls();
      
      // Set up monitoring and health checks
      this.setupMonitoring();
      
      // Initialize state
      this.updateUI();
      
      this.logger.info('LiveDemoTab component initialized successfully');
    } catch (error) {
      this.logger.error('Failed to initialize LiveDemoTab', { error: error.message });
      this.showError(`Initialization failed: ${error.message}`);
    }
  }

  createEnhancedStructure() {
    // Check if we need to rebuild the structure
    const existingCanvas = this.container.querySelector('#pose-detection-main');
    if (!existingCanvas) {
      // Create enhanced structure if it doesn't exist
      const enhancedHTML = `
        <div class="live-demo-enhanced">
          <div class="demo-header">
            <div class="demo-title">
              <h2>Live Human Pose Detection</h2>
              <div class="demo-status">
                <span class="status-indicator" id="demo-status-indicator"></span>
                <span class="status-text" id="demo-status-text">Ready</span>
              </div>
            </div>
            <div class="demo-controls">
              <button class="btn btn--primary" id="start-enhanced-demo">Start Detection</button>
              <button class="btn btn--secondary" id="stop-enhanced-demo" disabled>Stop Detection</button>
              <button class="btn btn--primary" id="toggle-debug">Debug Mode</button>
              <select class="zone-select" id="zone-selector">
                <option value="zone_1">Zone 1</option>
                <option value="zone_2">Zone 2</option>
                <option value="zone_3">Zone 3</option>
              </select>
            </div>
          </div>
          
          <div class="demo-content">
            <div class="demo-main">
              <div id="pose-detection-main" class="pose-detection-container"></div>
            </div>
            
            <div class="demo-sidebar">
              <div class="metrics-panel">
                <h4>Performance Metrics</h4>
                <div class="metric">
                  <label>Connection Status:</label>
                  <span id="connection-status">Disconnected</span>
                </div>
                <div class="metric">
                  <label>Frames Processed:</label>
                  <span id="frame-count">0</span>
                </div>
                <div class="metric">
                  <label>Uptime:</label>
                  <span id="uptime">0s</span>
                </div>
                <div class="metric">
                  <label>Errors:</label>
                  <span id="error-count">0</span>
                </div>
                <div class="metric">
                  <label>Last Update:</label>
                  <span id="last-update">Never</span>
                </div>
              </div>
              
              <div class="health-panel">
                <h4>System Health</h4>
                <div class="health-check">
                  <label>API Health:</label>
                  <span id="api-health">Unknown</span>
                </div>
                <div class="health-check">
                  <label>WebSocket:</label>
                  <span id="websocket-health">Unknown</span>
                </div>
                <div class="health-check">
                  <label>Pose Service:</label>
                  <span id="pose-service-health">Unknown</span>
                </div>
              </div>
              
              <div class="debug-panel" id="debug-panel" style="display: none;">
                <h4>Debug Information</h4>
                <div class="debug-actions">
                  <button class="btn btn-sm" id="force-reconnect">Force Reconnect</button>
                  <button class="btn btn-sm" id="clear-errors">Clear Errors</button>
                  <button class="btn btn-sm" id="export-logs">Export Logs</button>
                </div>
                <div class="debug-info">
                  <textarea id="debug-output" readonly rows="8" cols="30"></textarea>
                </div>
              </div>
            </div>
          </div>
          
          <div class="demo-footer">
            <div class="error-display" id="error-display" style="display: none;"></div>
          </div>
        </div>
      `;
      
      this.container.innerHTML = enhancedHTML;
      this.addEnhancedStyles();
    }
  }

  addEnhancedStyles() {
    const style = document.createElement('style');
    style.textContent = `
      .live-demo-enhanced {
        display: flex;
        flex-direction: column;
        height: 100%;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: #333;
      }

      .demo-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 20px 24px;
        background: rgba(255, 255, 255, 0.95);
        backdrop-filter: blur(10px);
        border-bottom: 1px solid rgba(255, 255, 255, 0.2);
        box-shadow: 0 2px 20px rgba(0, 0, 0, 0.1);
        position: relative;
        z-index: 10;
      }

      .demo-title {
        display: flex;
        align-items: center;
        gap: 20px;
      }

      .demo-title h2 {
        margin: 0;
        color: #333;
        font-size: 22px;
        font-weight: 700;
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        background-clip: text;
      }

      .demo-status {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 8px 16px;
        background: rgba(248, 249, 250, 0.8);
        border-radius: 20px;
        border: 1px solid rgba(222, 226, 230, 0.5);
      }

      .status-indicator {
        width: 10px;
        height: 10px;
        border-radius: 50%;
        background: #6c757d;
        transition: all 0.3s ease;
        box-shadow: 0 0 0 2px rgba(108, 117, 125, 0.2);
      }

      .status-indicator.active { 
        background: #28a745; 
        box-shadow: 0 0 0 2px rgba(40, 167, 69, 0.2), 0 0 8px rgba(40, 167, 69, 0.4);
      }
      .status-indicator.connecting { 
        background: #ffc107; 
        box-shadow: 0 0 0 2px rgba(255, 193, 7, 0.2), 0 0 8px rgba(255, 193, 7, 0.4);
        animation: pulse 1.5s ease-in-out infinite;
      }
      .status-indicator.error { 
        background: #dc3545; 
        box-shadow: 0 0 0 2px rgba(220, 53, 69, 0.2), 0 0 8px rgba(220, 53, 69, 0.4);
      }

      @keyframes pulse {
        0%, 100% { opacity: 1; }
        50% { opacity: 0.5; }
      }

      .status-text {
        font-size: 13px;
        font-weight: 500;
        color: #495057;
      }

      .demo-controls {
        display: flex;
        align-items: center;
        gap: 12px;
      }

      .demo-controls .btn {
        padding: 10px 20px;
        border: 1px solid transparent;
        border-radius: 8px;
        font-size: 14px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s ease;
        text-decoration: none;
        display: inline-flex;
        align-items: center;
        gap: 8px;
        min-width: 120px;
        justify-content: center;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
      }

      .btn--primary {
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
        border-color: transparent;
      }

      .btn--primary:hover:not(:disabled) {
        transform: translateY(-2px);
        box-shadow: 0 4px 16px rgba(102, 126, 234, 0.3);
      }

      .btn--secondary {
        background: #f8f9fa;
        color: #495057;
        border-color: #dee2e6;
      }

      .btn--secondary:hover:not(:disabled) {
        background: #e9ecef;
        transform: translateY(-1px);
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
      }

      .btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
        transform: none !important;
        box-shadow: none !important;
      }

      .btn-sm { 
        padding: 6px 12px; 
        font-size: 12px;
        min-width: 80px;
      }

      .zone-select {
        padding: 10px 14px;
        border: 1px solid #dee2e6;
        border-radius: 8px;
        background: white;
        font-size: 14px;
        cursor: pointer;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
        transition: all 0.2s ease;
      }

      .zone-select:focus {
        outline: none;
        border-color: #667eea;
        box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
      }

      .demo-content {
        display: flex;
        flex: 1;
        gap: 24px;
        padding: 24px;
        background: rgba(255, 255, 255, 0.1);
        backdrop-filter: blur(10px);
      }

      .demo-main {
        flex: 2;
        min-height: 500px;
        background: white;
        border-radius: 12px;
        overflow: hidden;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
        border: 1px solid rgba(255, 255, 255, 0.2);
      }

      .pose-detection-container {
        height: 100%;
        position: relative;
      }

      .demo-sidebar {
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: 20px;
        max-width: 300px;
      }

      .metrics-panel, .health-panel, .debug-panel {
        background: #fff;
        border: 1px solid #ddd;
        border-radius: 8px;
        padding: 15px;
      }

      .metrics-panel h4, .health-panel h4, .debug-panel h4 {
        margin: 0 0 15px 0;
        color: #333;
        font-size: 14px;
        font-weight: 600;
      }

      .metric, .health-check {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 10px;
        font-size: 13px;
      }

      .metric label, .health-check label {
        color: #666;
      }

      .metric span, .health-check span {
        font-weight: 500;
        color: #333;
      }

      .debug-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 5px;
        margin-bottom: 10px;
      }

      .debug-info textarea {
        width: 100%;
        border: 1px solid #ddd;
        border-radius: 4px;
        padding: 8px;
        font-family: monospace;
        font-size: 11px;
        resize: vertical;
      }

      .error-display {
        background: #f8d7da;
        color: #721c24;
        border: 1px solid #f5c6cb;
        border-radius: 4px;
        padding: 12px;
        margin: 10px 20px;
      }

      .health-unknown { color: #6c757d; }
      .health-good { color: #28a745; }
      .health-poor { color: #ffc107; }
      .health-bad { color: #dc3545; }
    `;
    
    if (!document.querySelector('#live-demo-enhanced-styles')) {
      style.id = 'live-demo-enhanced-styles';
      document.head.appendChild(style);
    }
  }

  initializePoseCanvas() {
    try {
      this.components.poseCanvas = new PoseDetectionCanvas('pose-detection-main', {
        width: 800,
        height: 600,
        autoResize: true,
        enableStats: true,
        enableControls: false, // We'll handle controls in the parent
        zoneId: this.state.currentZone
      });

      // Set up canvas callbacks
      this.components.poseCanvas.setCallback('onStateChange', (state) => {
        this.handleCanvasStateChange(state);
      });

      this.components.poseCanvas.setCallback('onPoseUpdate', (data) => {
        this.handlePoseUpdate(data);
      });

      this.components.poseCanvas.setCallback('onError', (error) => {
        this.handleCanvasError(error);
      });

      this.components.poseCanvas.setCallback('onConnectionChange', (state) => {
        this.handleConnectionStateChange(state);
      });

      this.logger.info('Pose detection canvas initialized');
    } catch (error) {
      this.logger.error('Failed to initialize pose canvas', { error: error.message });
      throw error;
    }
  }

  setupEnhancedControls() {
    // Main controls
    const startBtn = this.container.querySelector('#start-enhanced-demo');
    const stopBtn = this.container.querySelector('#stop-enhanced-demo');
    const debugBtn = this.container.querySelector('#toggle-debug');
    const zoneSelector = this.container.querySelector('#zone-selector');

    if (startBtn) {
      startBtn.addEventListener('click', () => this.startDemo());
    }

    if (stopBtn) {
      stopBtn.addEventListener('click', () => this.stopDemo());
    }

    if (debugBtn) {
      debugBtn.addEventListener('click', () => this.toggleDebugMode());
    }

    if (zoneSelector) {
      zoneSelector.addEventListener('change', (e) => this.changeZone(e.target.value));
      zoneSelector.value = this.state.currentZone;
    }

    // Debug controls
    const forceReconnectBtn = this.container.querySelector('#force-reconnect');
    const clearErrorsBtn = this.container.querySelector('#clear-errors');
    const exportLogsBtn = this.container.querySelector('#export-logs');

    if (forceReconnectBtn) {
      forceReconnectBtn.addEventListener('click', () => this.forceReconnect());
    }

    if (clearErrorsBtn) {
      clearErrorsBtn.addEventListener('click', () => this.clearErrors());
    }

    if (exportLogsBtn) {
      exportLogsBtn.addEventListener('click', () => this.exportLogs());
    }

    this.logger.debug('Enhanced controls set up');
  }

  setupMonitoring() {
    // Set up periodic health checks
    if (this.config.enablePerformanceMonitoring) {
      this.healthCheckInterval = setInterval(() => {
        this.performHealthCheck();
      }, this.config.healthCheckInterval);
    }

    // Set up periodic UI updates
    this.uiUpdateInterval = setInterval(() => {
      this.updateMetricsDisplay();
    }, 1000);

    this.logger.debug('Monitoring set up');
  }

  // Event handlers for canvas callbacks
  handleCanvasStateChange(state) {
    this.state.isActive = state.isActive;
    this.updateUI();
    this.logger.debug('Canvas state changed', { state });
  }

  handlePoseUpdate(data) {
    this.metrics.frameCount++;
    this.metrics.lastUpdate = Date.now();
    this.updateDebugOutput(`Pose update: ${data.persons?.length || 0} persons detected`);
  }

  handleCanvasError(error) {
    this.metrics.errorCount++;
    this.logger.error('Canvas error', { error: error.message });
    this.showError(`Canvas error: ${error.message}`);
  }

  handleConnectionStateChange(state) {
    this.state.connectionState = state;
    this.updateUI();
    this.logger.debug('Connection state changed', { state });
  }

  // Start demo
  async startDemo() {
    if (this.state.isActive) {
      this.logger.warn('Demo already active');
      return;
    }
    
    try {
      this.logger.info('Starting enhanced demo');
      this.metrics.startTime = Date.now();
      this.metrics.frameCount = 0;
      this.metrics.errorCount = 0;
      this.metrics.connectionAttempts++;
      
      // Update UI state
      this.setState({ isActive: true, connectionState: 'connecting' });
      this.clearError();
      
      // Start the pose detection canvas
      await this.components.poseCanvas.start();
      
      this.logger.info('Enhanced demo started successfully');
      this.updateDebugOutput('Demo started successfully');
      
    } catch (error) {
      this.logger.error('Failed to start enhanced demo', { error: error.message });
      this.showError(`Failed to start: ${error.message}`);
      this.setState({ isActive: false, connectionState: 'error' });
    }
  }

  // Stop demo
  stopDemo() {
    if (!this.state.isActive) {
      this.logger.warn('Demo not active');
      return;
    }
    
    try {
      this.logger.info('Stopping enhanced demo');
      
      // Stop the pose detection canvas
      this.components.poseCanvas.stop();
      
      // Update state
      this.setState({ isActive: false, connectionState: 'disconnected' });
      this.clearError();
      
      this.logger.info('Enhanced demo stopped successfully');
      this.updateDebugOutput('Demo stopped successfully');
      
    } catch (error) {
      this.logger.error('Error stopping enhanced demo', { error: error.message });
      this.showError(`Error stopping: ${error.message}`);
    }
  }

  // Enhanced control methods
  toggleDebugMode() {
    this.state.debugMode = !this.state.debugMode;
    const debugPanel = this.container.querySelector('#debug-panel');
    const debugBtn = this.container.querySelector('#toggle-debug');
    
    if (debugPanel) {
      debugPanel.style.display = this.state.debugMode ? 'block' : 'none';
    }
    
    if (debugBtn) {
      debugBtn.textContent = this.state.debugMode ? 'Hide Debug' : 'Debug Mode';
      debugBtn.classList.toggle('active', this.state.debugMode);
    }
    
    this.logger.info('Debug mode toggled', { enabled: this.state.debugMode });
  }

  async changeZone(zoneId) {
    this.logger.info('Changing zone', { from: this.state.currentZone, to: zoneId });
    this.state.currentZone = zoneId;
    
    // Update canvas configuration
    if (this.components.poseCanvas) {
      this.components.poseCanvas.updateConfig({ zoneId });
      
      // Restart if currently active
      if (this.state.isActive) {
        await this.components.poseCanvas.reconnect();
      }
    }
  }

  async forceReconnect() {
    if (!this.state.isActive) {
      this.showError('Cannot reconnect - demo not active');
      return;
    }
    
    try {
      this.logger.info('Forcing reconnection');
      await this.components.poseCanvas.reconnect();
      this.updateDebugOutput('Force reconnection initiated');
    } catch (error) {
      this.logger.error('Force reconnection failed', { error: error.message });
      this.showError(`Reconnection failed: ${error.message}`);
    }
  }

  clearErrors() {
    this.metrics.errorCount = 0;
    this.clearError();
    poseService.clearValidationErrors();
    this.updateDebugOutput('Errors cleared');
    this.logger.info('Errors cleared');
  }

  exportLogs() {
    const logs = {
      timestamp: new Date().toISOString(),
      state: this.state,
      metrics: this.metrics,
      poseServiceMetrics: poseService.getPerformanceMetrics(),
      wsServiceStats: wsService.getAllConnectionStats(),
      canvasStats: this.components.poseCanvas?.getPerformanceMetrics()
    };
    
    const blob = new Blob([JSON.stringify(logs, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `pose-detection-logs-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
    
    this.updateDebugOutput('Logs exported');
    this.logger.info('Logs exported');
  }

  // State management
  setState(newState) {
    this.state = { ...this.state, ...newState };
    this.updateUI();
  }

  updateUI() {
    this.updateStatusIndicator();
    this.updateControls();
    this.updateMetricsDisplay();
  }

  updateStatusIndicator() {
    const indicator = this.container.querySelector('#demo-status-indicator');
    const text = this.container.querySelector('#demo-status-text');
    
    if (indicator) {
      indicator.className = `status-indicator ${this.getStatusClass()}`;
    }
    
    if (text) {
      text.textContent = this.getStatusText();
    }
  }

  getStatusClass() {
    if (this.state.isActive) {
      return this.state.connectionState === 'connected' ? 'active' : 'connecting';
    }
    return this.state.connectionState === 'error' ? 'error' : '';
  }

  getStatusText() {
    if (this.state.isActive) {
      return this.state.connectionState === 'connected' ? 'Active' : 'Connecting...';
    }
    return this.state.connectionState === 'error' ? 'Error' : 'Ready';
  }

  updateControls() {
    const startBtn = this.container.querySelector('#start-enhanced-demo');
    const stopBtn = this.container.querySelector('#stop-enhanced-demo');
    const zoneSelector = this.container.querySelector('#zone-selector');
    
    if (startBtn) {
      startBtn.disabled = this.state.isActive;
    }
    
    if (stopBtn) {
      stopBtn.disabled = !this.state.isActive;
    }
    
    if (zoneSelector) {
      zoneSelector.disabled = this.state.isActive;
    }
  }

  updateMetricsDisplay() {
    const elements = {
      connectionStatus: this.container.querySelector('#connection-status'),
      frameCount: this.container.querySelector('#frame-count'),
      uptime: this.container.querySelector('#uptime'),
      errorCount: this.container.querySelector('#error-count'),
      lastUpdate: this.container.querySelector('#last-update')
    };

    if (elements.connectionStatus) {
      elements.connectionStatus.textContent = this.state.connectionState;
      elements.connectionStatus.className = `health-${this.getHealthClass(this.state.connectionState)}`;
    }

    if (elements.frameCount) {
      elements.frameCount.textContent = this.metrics.frameCount;
    }

    if (elements.uptime) {
      const uptime = this.metrics.startTime ? 
        Math.round((Date.now() - this.metrics.startTime) / 1000) : 0;
      elements.uptime.textContent = `${uptime}s`;
    }

    if (elements.errorCount) {
      elements.errorCount.textContent = this.metrics.errorCount;
      elements.errorCount.className = this.metrics.errorCount > 0 ? 'health-bad' : 'health-good';
    }

    if (elements.lastUpdate) {
      const lastUpdate = this.metrics.lastUpdate ? 
        new Date(this.metrics.lastUpdate).toLocaleTimeString() : 'Never';
      elements.lastUpdate.textContent = lastUpdate;
    }
  }

  getHealthClass(status) {
    switch (status) {
      case 'connected': return 'good';
      case 'connecting': return 'poor';
      case 'error': return 'bad';
      default: return 'unknown';
    }
  }

  async performHealthCheck() {
    try {
      // Check pose service health
      const poseHealth = await poseService.healthCheck();
      this.updateHealthDisplay('pose-service-health', poseHealth.healthy);

      // Check WebSocket health
      const wsStats = wsService.getAllConnectionStats();
      const wsHealthy = wsStats.connections.some(conn => conn.status === 'connected');
      this.updateHealthDisplay('websocket-health', wsHealthy);

      // Check API health (simplified)
      this.updateHealthDisplay('api-health', poseHealth.apiHealthy);

    } catch (error) {
      this.logger.error('Health check failed', { error: error.message });
    }
  }

  updateHealthDisplay(elementId, isHealthy) {
    const element = this.container.querySelector(`#${elementId}`);
    if (element) {
      element.textContent = isHealthy ? 'Good' : 'Poor';
      element.className = isHealthy ? 'health-good' : 'health-poor';
    }
  }

  updateDebugOutput(message) {
    if (!this.state.debugMode) return;
    
    const debugOutput = this.container.querySelector('#debug-output');
    if (debugOutput) {
      const timestamp = new Date().toLocaleTimeString();
      const newLine = `[${timestamp}] ${message}\n`;
      debugOutput.value = (debugOutput.value + newLine).split('\n').slice(-50).join('\n');
      debugOutput.scrollTop = debugOutput.scrollHeight;
    }
  }

  showError(message) {
    const errorDisplay = this.container.querySelector('#error-display');
    if (errorDisplay) {
      errorDisplay.textContent = message;
      errorDisplay.style.display = 'block';
    }
    
    // Auto-hide after 10 seconds
    setTimeout(() => this.clearError(), 10000);
  }

  clearError() {
    const errorDisplay = this.container.querySelector('#error-display');
    if (errorDisplay) {
      errorDisplay.style.display = 'none';
    }
  }

  // Clean up
  dispose() {
    try {
      this.logger.info('Disposing LiveDemoTab component');
      
      // Stop demo if running
      if (this.state.isActive) {
        this.stopDemo();
      }
      
      // Clear intervals
      if (this.healthCheckInterval) {
        clearInterval(this.healthCheckInterval);
      }
      
      if (this.uiUpdateInterval) {
        clearInterval(this.uiUpdateInterval);
      }
      
      // Dispose canvas component
      if (this.components.poseCanvas) {
        this.components.poseCanvas.dispose();
      }
      
      // Unsubscribe from services
      this.subscriptions.forEach(unsubscribe => unsubscribe());
      this.subscriptions = [];
      
      this.logger.info('LiveDemoTab component disposed successfully');
    } catch (error) {
      this.logger.error('Error during disposal', { error: error.message });
    }
  }
}