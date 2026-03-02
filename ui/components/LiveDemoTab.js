// Live Demo Tab Component - Enhanced Version

import { PoseDetectionCanvas } from './PoseDetectionCanvas.js';
import { poseService } from '../services/pose.service.js';
import { streamService } from '../services/stream.service.js';
import { wsService } from '../services/websocket.service.js';
import { sensingService } from '../services/sensing.service.js';

// Optional services - loaded lazily in init() to avoid blocking module graph
let modelService = null;
let trainingService = null;

export class LiveDemoTab {
  constructor(containerElement) {
    this.container = containerElement;
    this.state = {
      isActive: false,
      connectionState: 'disconnected',
      currentZone: 'zone_1',
      debugMode: false,
      autoReconnect: true,
      renderMode: 'skeleton',
      // 'unknown' | 'signal_derived' | 'model_inference'
      poseSource: 'unknown'
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
    
    // Model control state
    this.modelState = {
      models: [],
      activeModelId: null,
      activeModelInfo: null,
      loraProfiles: [],
      selectedLoraProfile: null,
      loading: false
    };

    // Training state
    this.trainingState = {
      status: 'idle',       // 'idle' | 'training' | 'recording'
      epoch: 0,
      totalEpochs: 0,
      showTrainingPanel: false
    };

    // A/B split view state
    this.splitViewActive = false;

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

      // Load optional services (non-blocking)
      try {
        const mod = await import('../services/model.service.js');
        modelService = mod.modelService;
      } catch (e) { /* model features disabled */ }
      try {
        const mod = await import('../services/training.service.js');
        trainingService = mod.trainingService;
      } catch (e) { /* training features disabled */ }

      // Create enhanced DOM structure
      this.createEnhancedStructure();
      
      // Initialize pose detection canvas
      this.initializePoseCanvas();
      
      // Set up controls and event handlers
      this.setupEnhancedControls();
      
      // Set up monitoring and health checks
      this.setupMonitoring();
      
      // Fetch available models on init
      this.fetchModels();

      // Set up model/training event listeners
      this.setupServiceListeners();

      // Initialize state
      this.updateUI();

      // Auto-start pose detection when a backend is reachable.
      // Check after a brief delay (sensing WS may still be connecting).
      this._autoStartOnce = false;
      const tryAutoStart = () => {
        if (this._autoStartOnce || this.state.isActive) return;
        const ds = sensingService.dataSource;
        if (ds === 'live' || ds === 'server-simulated') {
          this._autoStartOnce = true;
          this.logger.info('Auto-starting pose detection (data source: ' + ds + ')');
          this.startDemo();
        }
      };
      setTimeout(tryAutoStart, 2000);
      // Also listen for sensing state changes in case server connects later
      this._autoStartUnsub = sensingService.onStateChange(tryAutoStart);

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
          <!-- Data source banner — prominent indicator for live vs simulated -->
          <div id="demo-source-banner" class="demo-source-banner demo-source-unknown" role="status" aria-live="polite">
            Detecting data source...
          </div>

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
              <button class="btn btn--accent" id="run-offline-demo">Demo</button>
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
              
              <div class="pose-source-panel">
                <h4>Estimation Mode</h4>
                <div class="pose-source-indicator" id="pose-source-indicator">
                  <span class="pose-source-badge pose-source-unknown" id="pose-source-badge">Unknown</span>
                  <p class="pose-source-description" id="pose-source-description">
                    Waiting for first frame...
                  </p>
                </div>
              </div>

              <div class="model-control-panel" id="model-control-panel">
                <h4>Model Control</h4>
                <div class="setting-row-ld">
                  <label class="ld-label">Model:</label>
                  <select class="ld-select" id="model-selector">
                    <option value="">Signal-Derived (no model)</option>
                  </select>
                </div>
                <div class="model-info-row" id="model-active-info" style="display: none;">
                  <span class="ld-label" id="model-active-name"></span>
                  <span class="model-pck-badge" id="model-active-pck"></span>
                </div>
                <div class="setting-row-ld" id="lora-profile-row" style="display: none;">
                  <label class="ld-label">LoRA Profile:</label>
                  <select class="ld-select" id="lora-profile-selector">
                    <option value="">None</option>
                  </select>
                </div>
                <div class="model-actions">
                  <button class="btn-ld btn-ld-accent" id="load-model-btn">Load Model</button>
                  <button class="btn-ld btn-ld-muted" id="unload-model-btn" disabled>Unload</button>
                </div>
                <div class="model-status-text" id="model-status-text">No model loaded</div>
              </div>

              <div class="split-view-panel">
                <div class="setting-row-ld">
                  <label class="ld-label">Compare: Signal vs Model</label>
                  <button class="btn-ld btn-ld-toggle" id="split-view-toggle" disabled>Off</button>
                </div>
              </div>

              <div class="training-quick-panel" id="training-quick-panel">
                <h4>Training</h4>
                <div class="training-status-row">
                  <span class="training-status-badge" id="training-status-badge">Idle</span>
                </div>
                <div class="training-actions">
                  <button class="btn-ld btn-ld-accent" id="open-training-panel-btn">Open Training Panel</button>
                  <button class="btn-ld btn-ld-muted" id="quick-record-btn">Record 60s</button>
                </div>
              </div>

              <div class="setup-guide-panel">
                <h4>Setup Guide</h4>
                <div class="setup-levels">
                  <div class="setup-level">
                    <span class="setup-level-icon">1x</span>
                    <div class="setup-level-info">
                      <strong>1 ESP32 + 1 AP</strong>
                      <p>Presence, breathing, gross motion</p>
                    </div>
                  </div>
                  <div class="setup-level">
                    <span class="setup-level-icon">3x</span>
                    <div class="setup-level-info">
                      <strong>2-3 ESP32s</strong>
                      <p>Body localization, motion direction</p>
                    </div>
                  </div>
                  <div class="setup-level">
                    <span class="setup-level-icon">4x+</span>
                    <div class="setup-level-info">
                      <strong>4+ ESP32s + trained model</strong>
                      <p>Individual limb tracking, full pose</p>
                    </div>
                  </div>
                </div>
                <p class="setup-note">
                  Signal-Derived mode uses aggregate CSI features.
                  For per-limb tracking, load a trained <code>.rvf</code> model
                  with <code>--model path.rvf</code> and use 4+ sensors.
                </p>
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
        background: #0a0f1a;
        color: #e0e0e0;
      }

      .demo-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 20px 24px;
        background: rgba(15, 20, 35, 0.95);
        backdrop-filter: blur(10px);
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        box-shadow: 0 2px 20px rgba(0, 0, 0, 0.3);
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
        color: #e0e0e0;
        font-size: 22px;
        font-weight: 700;
        background: linear-gradient(135deg, #667eea 0%, #a78bfa 100%);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        background-clip: text;
      }

      .demo-status {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 8px 16px;
        background: rgba(30, 40, 60, 0.8);
        border-radius: 20px;
        border: 1px solid rgba(255, 255, 255, 0.1);
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
        color: #b0b8c8;
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
        box-shadow: 0 4px 16px rgba(102, 126, 234, 0.4);
      }

      .btn--secondary {
        background: rgba(30, 40, 60, 0.8);
        color: #b0b8c8;
        border-color: rgba(255, 255, 255, 0.1);
      }

      .btn--secondary:hover:not(:disabled) {
        background: rgba(40, 50, 75, 0.9);
        transform: translateY(-1px);
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
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
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        background: rgba(30, 40, 60, 0.8);
        color: #b0b8c8;
        font-size: 14px;
        cursor: pointer;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
        transition: all 0.2s ease;
      }

      .zone-select:focus {
        outline: none;
        border-color: #667eea;
        box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.2);
      }

      .demo-content {
        display: flex;
        flex: 1;
        gap: 24px;
        padding: 24px;
        background: #0a0f1a;
      }

      .demo-main {
        flex: 2;
        min-height: 500px;
        background: #111827;
        border-radius: 12px;
        overflow: hidden;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.06);
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
        background: rgba(17, 24, 39, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        padding: 15px;
      }

      .metrics-panel h4, .health-panel h4, .debug-panel h4 {
        margin: 0 0 15px 0;
        color: #e0e0e0;
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
        color: #8899aa;
      }

      .metric span, .health-check span {
        font-weight: 500;
        color: #c8d0dc;
      }

      .debug-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 5px;
        margin-bottom: 10px;
      }

      .debug-info textarea {
        width: 100%;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 4px;
        padding: 8px;
        font-family: monospace;
        font-size: 11px;
        resize: vertical;
        background: #0a0f1a;
        color: #c8d0dc;
      }

      .error-display {
        background: rgba(220, 53, 69, 0.15);
        color: #f5a0a8;
        border: 1px solid rgba(220, 53, 69, 0.3);
        border-radius: 4px;
        padding: 12px;
        margin: 10px 20px;
      }

      .health-unknown { color: #6c757d; }
      .health-good { color: #28a745; }
      .health-poor { color: #ffc107; }
      .health-bad { color: #dc3545; }

      /* Pose estimation mode indicator */
      .pose-source-panel {
        background: rgba(17, 24, 39, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        padding: 15px;
      }

      .pose-source-panel h4 {
        margin: 0 0 12px 0;
        color: #e0e0e0;
        font-size: 14px;
        font-weight: 600;
      }

      .pose-source-indicator {
        display: flex;
        flex-direction: column;
        gap: 8px;
      }

      .pose-source-badge {
        display: inline-block;
        padding: 4px 12px;
        border-radius: 12px;
        font-size: 12px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        width: fit-content;
      }

      .pose-source-unknown {
        background: rgba(108, 117, 125, 0.15);
        color: #8899aa;
        border: 1px solid rgba(108, 117, 125, 0.3);
      }

      .pose-source-signal {
        background: rgba(0, 204, 136, 0.12);
        color: #00cc88;
        border: 1px solid rgba(0, 204, 136, 0.3);
      }

      .pose-source-model {
        background: rgba(102, 126, 234, 0.12);
        color: #8ea4f0;
        border: 1px solid rgba(102, 126, 234, 0.3);
      }

      .pose-source-description {
        margin: 0;
        font-size: 11px;
        color: #8899aa;
        line-height: 1.4;
      }

      .setup-guide-panel {
        background: rgba(17, 24, 39, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        padding: 15px;
      }

      .setup-guide-panel h4 {
        margin: 0 0 12px 0;
        color: #e0e0e0;
        font-size: 14px;
        font-weight: 600;
      }

      .setup-levels {
        display: flex;
        flex-direction: column;
        gap: 10px;
      }

      .setup-level {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 8px;
        border-radius: 6px;
        background: rgba(30, 40, 60, 0.6);
        border: 1px solid rgba(255, 255, 255, 0.06);
      }

      .setup-level-icon {
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
        font-size: 11px;
        font-weight: 700;
        width: 32px;
        height: 32px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        flex-shrink: 0;
      }

      .setup-level-info strong {
        font-size: 12px;
        color: #c8d0dc;
        display: block;
      }

      .setup-level-info p {
        margin: 2px 0 0;
        font-size: 11px;
        color: #8899aa;
      }

      .setup-note {
        margin: 10px 0 0;
        font-size: 11px;
        color: #6b7a8d;
        line-height: 1.5;
      }

      .setup-note code {
        background: rgba(102, 126, 234, 0.12);
        color: #8ea4f0;
        padding: 1px 4px;
        border-radius: 3px;
        font-size: 10px;
      }

      /* Model Control Panel */
      .model-control-panel,
      .split-view-panel,
      .training-quick-panel {
        background: rgba(17, 24, 39, 0.9);
        border: 1px solid rgba(56, 68, 89, 0.6);
        border-radius: 12px;
        padding: 16px;
      }

      .model-control-panel h4,
      .training-quick-panel h4 {
        margin: 0 0 12px 0;
        color: #e0e0e0;
        font-size: 14px;
        font-weight: 600;
      }

      .setting-row-ld {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 10px;
        gap: 8px;
      }

      .ld-label {
        color: #8899aa;
        font-size: 11px;
        flex-shrink: 0;
      }

      .ld-select {
        flex: 1;
        padding: 6px 10px;
        border: 1px solid rgba(56, 68, 89, 0.6);
        border-radius: 6px;
        background: rgba(15, 20, 35, 0.8);
        color: #b0b8c8;
        font-size: 12px;
        cursor: pointer;
        min-width: 0;
      }

      .ld-select:focus {
        outline: none;
        border-color: #667eea;
        box-shadow: 0 0 0 2px rgba(102, 126, 234, 0.15);
      }

      .ld-select option {
        background: #1a2234;
        color: #c8d0dc;
      }

      .model-info-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 10px;
        padding: 6px 8px;
        background: rgba(30, 40, 60, 0.6);
        border-radius: 6px;
      }

      .model-pck-badge {
        font-size: 11px;
        font-weight: 600;
        padding: 2px 8px;
        border-radius: 8px;
        background: rgba(102, 126, 234, 0.15);
        color: #8ea4f0;
      }

      .model-actions,
      .training-actions {
        display: flex;
        gap: 8px;
        margin-top: 10px;
      }

      .btn-ld {
        flex: 1;
        padding: 7px 12px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        font-size: 12px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s ease;
        text-align: center;
      }

      .btn-ld:disabled {
        opacity: 0.4;
        cursor: not-allowed;
      }

      .btn-ld-accent {
        background: rgba(102, 126, 234, 0.15);
        color: #8ea4f0;
        border-color: rgba(102, 126, 234, 0.3);
      }

      .btn-ld-accent:hover:not(:disabled) {
        background: rgba(102, 126, 234, 0.25);
        border-color: rgba(102, 126, 234, 0.5);
      }

      .btn-ld-muted {
        background: rgba(30, 40, 60, 0.8);
        color: #8899aa;
        border-color: rgba(255, 255, 255, 0.08);
      }

      .btn-ld-muted:hover:not(:disabled) {
        background: rgba(40, 50, 70, 0.9);
        color: #b0b8c8;
      }

      .btn-ld-toggle {
        min-width: 44px;
        flex: 0;
        padding: 4px 10px;
        background: rgba(30, 40, 60, 0.8);
        color: #8899aa;
        border-color: rgba(255, 255, 255, 0.08);
        border-radius: 12px;
        font-size: 11px;
      }

      .btn-ld-toggle.active {
        background: rgba(0, 212, 255, 0.15);
        color: #00d4ff;
        border-color: rgba(0, 212, 255, 0.4);
      }

      .model-status-text {
        margin-top: 8px;
        font-size: 11px;
        color: #6b7a8d;
      }

      .training-status-row {
        margin-bottom: 8px;
      }

      .training-status-badge {
        display: inline-block;
        padding: 3px 10px;
        border-radius: 10px;
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.4px;
        background: rgba(108, 117, 125, 0.15);
        color: #8899aa;
        border: 1px solid rgba(108, 117, 125, 0.3);
      }

      .training-status-badge.training {
        background: rgba(251, 191, 36, 0.12);
        color: #fbbf24;
        border-color: rgba(251, 191, 36, 0.3);
      }

      .training-status-badge.recording {
        background: rgba(239, 68, 68, 0.12);
        color: #ef4444;
        border-color: rgba(239, 68, 68, 0.3);
        animation: pulse 1.5s ease-in-out infinite;
      }

      /* A/B Split View Overlay */
      .split-view-divider {
        position: absolute;
        top: 0;
        bottom: 0;
        left: 50%;
        width: 2px;
        background: repeating-linear-gradient(
          to bottom,
          rgba(255, 255, 255, 0.4) 0px,
          rgba(255, 255, 255, 0.4) 6px,
          transparent 6px,
          transparent 12px
        );
        z-index: 15;
        pointer-events: none;
      }

      .split-view-label {
        position: absolute;
        top: 8px;
        z-index: 16;
        font-size: 10px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        padding: 3px 8px;
        border-radius: 4px;
        pointer-events: none;
      }

      .split-view-label.left {
        left: 8px;
        background: rgba(0, 204, 136, 0.2);
        color: #00cc88;
      }

      .split-view-label.right {
        right: 8px;
        background: rgba(102, 126, 234, 0.2);
        color: #8ea4f0;
      }

      /* Training modal overlay */
      .training-panel-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
      }

      .training-panel-modal {
        background: #0d1117;
        border: 1px solid rgba(56, 68, 89, 0.6);
        border-radius: 12px;
        padding: 24px;
        min-width: 400px;
        max-width: 600px;
        max-height: 80vh;
        overflow-y: auto;
        color: #e0e0e0;
      }

      .training-panel-modal h3 {
        margin: 0 0 16px 0;
        font-size: 18px;
        color: #e0e0e0;
      }

      .training-panel-modal .close-btn {
        float: right;
        background: rgba(30, 40, 60, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #8899aa;
        border-radius: 6px;
        padding: 4px 10px;
        cursor: pointer;
        font-size: 12px;
      }

      .training-panel-modal .close-btn:hover {
        background: rgba(50, 60, 80, 0.9);
        color: #c8d0dc;
      }
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

    // Offline demo button — runs client-side animated demo (no server needed)
    const offlineDemoBtn = this.container.querySelector('#run-offline-demo');
    if (offlineDemoBtn) {
      offlineDemoBtn.addEventListener('click', () => {
        if (this.components.poseCanvas) {
          this.components.poseCanvas.toggleDemo();
        }
      });
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

    // Model, training, and split-view controls
    this.setupModelTrainingControls();

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

    // Subscribe to sensing service for data-source changes
    this._sensingStateUnsub = sensingService.onStateChange(() => {
      this.updateSourceBanner();
      this.updateStatusIndicator();
    });
    // Throttle data-based banner updates (frames arrive at 10Hz)
    let lastBannerUpdate = 0;
    this._sensingDataUnsub = sensingService.onData(() => {
      const now = Date.now();
      if (now - lastBannerUpdate > 2000) {
        lastBannerUpdate = now;
        this.updateSourceBanner();
      }
    });
    // Initial banner update
    this.updateSourceBanner();

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
    // Update pose source indicator if the backend supplies it
    if (data.pose_source && data.pose_source !== this.state.poseSource) {
      this.setState({ poseSource: data.pose_source });
    }
    this.updateDebugOutput(`Pose update: ${data.persons?.length || 0} persons detected (${data.pose_source || 'unknown'})`);
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
    this.updatePoseSourceIndicator();
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
    if (!this.state.isActive) {
      return this.state.connectionState === 'error' ? 'error' : '';
    }
    const ds = sensingService.dataSource;
    if (ds === 'live') return 'active';
    if (ds === 'server-simulated') return 'sim';
    return 'connecting';
  }

  getStatusText() {
    if (!this.state.isActive) {
      return this.state.connectionState === 'error' ? 'Error' : 'Ready';
    }
    const ds = sensingService.dataSource;
    if (ds === 'live') return 'Active \u2014 ESP32 Live';
    if (ds === 'server-simulated') return 'Active \u2014 Simulated Data';
    if (ds === 'simulated') return 'Active \u2014 Offline Simulation';
    return 'Connecting...';
  }

  /** Update the prominent data-source banner at the top of Live Demo. */
  updateSourceBanner() {
    const banner = this.container.querySelector('#demo-source-banner');
    if (!banner) return;
    const ds = sensingService.dataSource;
    const config = {
      'live':             { text: 'LIVE \u2014 ESP32 Hardware Connected',           cls: 'demo-source-live' },
      'server-simulated': { text: 'SIMULATED DATA \u2014 No Hardware Detected',     cls: 'demo-source-sim' },
      'reconnecting':     { text: 'RECONNECTING TO SERVER...',                      cls: 'demo-source-reconnecting' },
      'simulated':        { text: 'OFFLINE \u2014 Server Unreachable, Local Sim',   cls: 'demo-source-offline' },
    };
    const cfg = config[ds] || config['reconnecting'];
    banner.textContent = cfg.text;
    banner.className = 'demo-source-banner ' + cfg.cls;
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
      const ds = sensingService.dataSource;
      const dsLabels = {
        'live':              'Connected \u2014 ESP32',
        'server-simulated':  'Connected \u2014 Simulated',
        'reconnecting':      'Reconnecting...',
        'simulated':         'Offline \u2014 Simulated',
      };
      const label = dsLabels[ds] || this.state.connectionState;
      elements.connectionStatus.textContent = label;
      const cls = ds === 'live' ? 'good'
        : ds === 'server-simulated' ? 'sim'
        : ds === 'simulated' ? 'bad'
        : this.getHealthClass(this.state.connectionState);
      elements.connectionStatus.className = `health-${cls}`;
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

  updatePoseSourceIndicator() {
    const badge = this.container.querySelector('#pose-source-badge');
    const description = this.container.querySelector('#pose-source-description');

    if (!badge || !description) return;

    const source = this.state.poseSource;

    if (source === 'model_inference') {
      badge.className = 'pose-source-badge pose-source-model';
      badge.textContent = 'Model Inference';
      description.textContent =
        'Pose is estimated by a trained neural network ' +
        'loaded from an RVF container.';
    } else if (source === 'signal_derived') {
      badge.className = 'pose-source-badge pose-source-signal';
      badge.textContent = 'Signal-Derived';
      description.textContent =
        'Keypoints are derived from live CSI signal features ' +
        '(motion power, breathing rate, variance).';
    } else {
      badge.className = 'pose-source-badge pose-source-unknown';
      badge.textContent = 'Unknown';
      description.textContent = 'Waiting for first frame...';
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

  // --- Model Control Methods ---

  async fetchModels() {
    if (!modelService) return;
    try {
      const data = await modelService.listModels();
      this.modelState.models = data?.models || [];
      this.populateModelSelector();
      // Check if a model is already active
      const active = await modelService.getActiveModel();
      if (active && active.model_id) {
        this.modelState.activeModelId = active.model_id;
        this.modelState.activeModelInfo = active;
        this.updateModelUI();
      }
    } catch (error) {
      this.logger.warn('Could not fetch models', { error: error.message });
    }
  }

  populateModelSelector() {
    const selector = this.container.querySelector('#model-selector');
    if (!selector) return;
    // Keep the first "Signal-Derived" option
    selector.innerHTML = '<option value="">Signal-Derived (no model)</option>';
    this.modelState.models.forEach(model => {
      const opt = document.createElement('option');
      opt.value = model.id || model.model_id || model.name;
      opt.textContent = model.name || model.id || 'Unknown Model';
      selector.appendChild(opt);
    });
    if (this.modelState.activeModelId) {
      selector.value = this.modelState.activeModelId;
    }
  }

  async handleLoadModel() {
    if (!modelService) return;
    const selector = this.container.querySelector('#model-selector');
    const modelId = selector?.value;
    if (!modelId) {
      this.setModelStatus('Select a model first');
      return;
    }
    try {
      this.modelState.loading = true;
      this.setModelStatus('Loading...');
      const loadBtn = this.container.querySelector('#load-model-btn');
      if (loadBtn) loadBtn.disabled = true;

      await modelService.loadModel(modelId);
      this.modelState.activeModelId = modelId;

      // Try to fetch full info
      try {
        const info = await modelService.getModel(modelId);
        this.modelState.activeModelInfo = info;
      } catch (e) {
        this.modelState.activeModelInfo = { model_id: modelId };
      }

      // Fetch LoRA profiles
      try {
        const profiles = await modelService.getLoraProfiles();
        this.modelState.loraProfiles = profiles || [];
      } catch (e) {
        this.modelState.loraProfiles = [];
      }

      this.modelState.loading = false;
      this.updateModelUI();
      this.updateSplitViewAvailability();

      // Update pose source badge to model inference
      this.setState({ poseSource: 'model_inference' });

    } catch (error) {
      this.modelState.loading = false;
      this.setModelStatus(`Error: ${error.message}`);
      const loadBtn = this.container.querySelector('#load-model-btn');
      if (loadBtn) loadBtn.disabled = false;
      this.logger.error('Failed to load model', { error: error.message });
    }
  }

  async handleUnloadModel() {
    if (!modelService) return;
    try {
      await modelService.unloadModel();
      this.modelState.activeModelId = null;
      this.modelState.activeModelInfo = null;
      this.modelState.loraProfiles = [];
      this.modelState.selectedLoraProfile = null;
      this.updateModelUI();
      this.updateSplitViewAvailability();
      this.disableSplitView();
      this.setState({ poseSource: 'signal_derived' });
    } catch (error) {
      this.setModelStatus(`Error: ${error.message}`);
      this.logger.error('Failed to unload model', { error: error.message });
    }
  }

  async handleLoraProfileChange(profileName) {
    if (!modelService || !this.modelState.activeModelId) return;
    if (!profileName) return;
    try {
      await modelService.activateLoraProfile(this.modelState.activeModelId, profileName);
      this.modelState.selectedLoraProfile = profileName;
      this.setModelStatus(`LoRA: ${profileName} active`);
    } catch (error) {
      this.setModelStatus(`LoRA error: ${error.message}`);
    }
  }

  updateModelUI() {
    const loadBtn = this.container.querySelector('#load-model-btn');
    const unloadBtn = this.container.querySelector('#unload-model-btn');
    const infoRow = this.container.querySelector('#model-active-info');
    const nameEl = this.container.querySelector('#model-active-name');
    const pckEl = this.container.querySelector('#model-active-pck');
    const loraRow = this.container.querySelector('#lora-profile-row');
    const loraSel = this.container.querySelector('#lora-profile-selector');

    const isLoaded = !!this.modelState.activeModelId;

    if (loadBtn) loadBtn.disabled = isLoaded;
    if (unloadBtn) unloadBtn.disabled = !isLoaded;

    if (infoRow) {
      infoRow.style.display = isLoaded ? 'flex' : 'none';
    }

    if (isLoaded && this.modelState.activeModelInfo) {
      const info = this.modelState.activeModelInfo;
      const name = info.name || info.model_id || this.modelState.activeModelId;
      const version = info.version ? ` v${info.version}` : '';
      const pck = info.pck_score != null ? info.pck_score.toFixed(2) : '--';
      if (nameEl) nameEl.textContent = `${name}${version}`;
      if (pckEl) pckEl.textContent = `PCK: ${pck}`;
      this.setModelStatus(`Model: ${name} (PCK: ${pck})`);
    } else if (!isLoaded) {
      this.setModelStatus('No model loaded');
    }

    // LoRA profiles
    if (loraRow && loraSel) {
      if (isLoaded && this.modelState.loraProfiles.length > 0) {
        loraRow.style.display = 'flex';
        loraSel.innerHTML = '<option value="">None</option>';
        this.modelState.loraProfiles.forEach(profile => {
          const opt = document.createElement('option');
          opt.value = profile.name || profile;
          opt.textContent = profile.name || profile;
          loraSel.appendChild(opt);
        });
      } else {
        loraRow.style.display = 'none';
      }
    }
  }

  setModelStatus(text) {
    const el = this.container.querySelector('#model-status-text');
    if (el) el.textContent = text;
  }

  // --- A/B Split View Methods ---

  updateSplitViewAvailability() {
    const toggle = this.container.querySelector('#split-view-toggle');
    if (toggle) {
      toggle.disabled = !this.modelState.activeModelId;
    }
  }

  toggleSplitView() {
    if (!this.modelState.activeModelId) return;
    this.splitViewActive = !this.splitViewActive;
    const toggle = this.container.querySelector('#split-view-toggle');
    if (toggle) {
      toggle.textContent = this.splitViewActive ? 'On' : 'Off';
      toggle.classList.toggle('active', this.splitViewActive);
    }
    this.updateSplitViewOverlay();
  }

  disableSplitView() {
    this.splitViewActive = false;
    const toggle = this.container.querySelector('#split-view-toggle');
    if (toggle) {
      toggle.textContent = 'Off';
      toggle.classList.remove('active');
    }
    this.updateSplitViewOverlay();
  }

  updateSplitViewOverlay() {
    const mainContainer = this.container.querySelector('.pose-detection-container');
    if (!mainContainer) return;

    // Remove existing overlays
    mainContainer.querySelectorAll('.split-view-divider, .split-view-label').forEach(el => el.remove());

    if (this.splitViewActive) {
      const divider = document.createElement('div');
      divider.className = 'split-view-divider';
      mainContainer.appendChild(divider);

      const leftLabel = document.createElement('div');
      leftLabel.className = 'split-view-label left';
      leftLabel.textContent = 'Signal-Derived';
      mainContainer.appendChild(leftLabel);

      const rightLabel = document.createElement('div');
      rightLabel.className = 'split-view-label right';
      rightLabel.textContent = 'Model Inference';
      mainContainer.appendChild(rightLabel);
    }
  }

  // --- Training Quick-Panel Methods ---

  updateTrainingStatus() {
    const badge = this.container.querySelector('#training-status-badge');
    if (!badge) return;

    const state = this.trainingState.status;
    badge.classList.remove('training', 'recording');

    if (state === 'training') {
      badge.classList.add('training');
      badge.textContent = `Training epoch ${this.trainingState.epoch}/${this.trainingState.totalEpochs}`;
    } else if (state === 'recording') {
      badge.classList.add('recording');
      badge.textContent = 'Recording...';
    } else {
      badge.textContent = 'Idle';
    }
  }

  async handleQuickRecord() {
    if (!trainingService) {
      this.logger.warn('Training service not available');
      return;
    }
    try {
      await trainingService.startRecording({ session_name: `quick_${Date.now()}`, duration_secs: 60 });
      this.trainingState.status = 'recording';
      this.updateTrainingStatus();
      // Auto-reset after ~65 seconds
      setTimeout(() => {
        if (this.trainingState.status === 'recording') {
          this.trainingState.status = 'idle';
          this.updateTrainingStatus();
        }
      }, 65000);
    } catch (error) {
      this.logger.error('Quick record failed', { error: error.message });
    }
  }

  showTrainingPanel() {
    // Create a simple modal overlay for the training panel
    const existing = document.querySelector('.training-panel-overlay');
    if (existing) existing.remove();

    const overlay = document.createElement('div');
    overlay.className = 'training-panel-overlay';
    overlay.innerHTML = `
      <div class="training-panel-modal">
        <button class="close-btn" id="close-training-modal">Close</button>
        <h3>Training Panel</h3>
        <p style="color: #8899aa; font-size: 13px; margin-bottom: 16px;">
          Configure and start model training from here. Connect to the backend training API to manage epochs, datasets, and checkpoints.
        </p>
        <div style="display: flex; flex-direction: column; gap: 10px;">
          <div class="setting-row-ld">
            <label class="ld-label" style="flex: 1;">Status:</label>
            <span style="color: #c8d0dc; font-size: 12px;">${this.trainingState.status}</span>
          </div>
          <div class="setting-row-ld">
            <label class="ld-label" style="flex: 1;">Training service:</label>
            <span style="color: ${trainingService ? '#00cc88' : '#ef4444'}; font-size: 12px;">${trainingService ? 'Connected' : 'Not available'}</span>
          </div>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);

    // Close handler
    overlay.querySelector('#close-training-modal').addEventListener('click', () => overlay.remove());
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) overlay.remove();
    });
  }

  // --- Service Event Listeners ---

  setupServiceListeners() {
    if (modelService) {
      const unsub1 = modelService.on('model-loaded', (data) => {
        this.logger.info('Model loaded event', data);
      });
      const unsub2 = modelService.on('model-unloaded', () => {
        this.modelState.activeModelId = null;
        this.modelState.activeModelInfo = null;
        this.updateModelUI();
        this.disableSplitView();
      });
      this.subscriptions.push(unsub1, unsub2);
    }

    if (trainingService) {
      const unsub3 = trainingService.on('progress', (data) => {
        if (data && data.epoch != null) {
          this.trainingState.epoch = data.epoch;
          this.trainingState.totalEpochs = data.total_epochs || data.totalEpochs || this.trainingState.totalEpochs;
          this.trainingState.status = 'training';
          this.updateTrainingStatus();
        }
      });
      const unsub4 = trainingService.on('training-stopped', () => {
        this.trainingState.status = 'idle';
        this.updateTrainingStatus();
      });
      this.subscriptions.push(unsub3, unsub4);
    }
  }

  // --- Enhanced Controls Setup ---

  setupModelTrainingControls() {
    // Model control buttons
    const loadBtn = this.container.querySelector('#load-model-btn');
    const unloadBtn = this.container.querySelector('#unload-model-btn');
    const loraSel = this.container.querySelector('#lora-profile-selector');
    const splitToggle = this.container.querySelector('#split-view-toggle');
    const openTrainingBtn = this.container.querySelector('#open-training-panel-btn');
    const quickRecordBtn = this.container.querySelector('#quick-record-btn');

    if (loadBtn) loadBtn.addEventListener('click', () => this.handleLoadModel());
    if (unloadBtn) unloadBtn.addEventListener('click', () => this.handleUnloadModel());
    if (loraSel) loraSel.addEventListener('change', (e) => this.handleLoraProfileChange(e.target.value));
    if (splitToggle) splitToggle.addEventListener('click', () => this.toggleSplitView());
    if (openTrainingBtn) openTrainingBtn.addEventListener('click', () => this.showTrainingPanel());
    if (quickRecordBtn) quickRecordBtn.addEventListener('click', () => this.handleQuickRecord());
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
      if (this._sensingStateUnsub) this._sensingStateUnsub();
      if (this._sensingDataUnsub) this._sensingDataUnsub();
      if (this._autoStartUnsub) this._autoStartUnsub();
      
      this.logger.info('LiveDemoTab component disposed successfully');
    } catch (error) {
      this.logger.error('Error during disposal', { error: error.message });
    }
  }
}