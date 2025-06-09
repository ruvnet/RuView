// PoseDetectionCanvas Component for WiFi-DensePose UI

import { PoseRenderer } from '../utils/pose-renderer.js';
import { poseService } from '../services/pose.service.js';
import { SettingsPanel } from './SettingsPanel.js';

export class PoseDetectionCanvas {
  constructor(containerId, options = {}) {
    this.containerId = containerId;
    this.container = document.getElementById(containerId);
    
    if (!this.container) {
      throw new Error(`Container with ID '${containerId}' not found`);
    }

    this.config = {
      width: 800,
      height: 600,
      autoResize: true,
      enableStats: true,
      enableControls: true,
      zoneId: 'zone_1',
      updateInterval: 50, // ms
      ...options
    };

    this.state = {
      isActive: false,
      connectionState: 'disconnected',
      lastPoseData: null,
      errorMessage: null,
      frameCount: 0,
      startTime: Date.now()
    };

    this.callbacks = {
      onStateChange: null,
      onPoseUpdate: null,
      onError: null,
      onConnectionChange: null
    };

    this.logger = this.createLogger();
    this.unsubscribeFunctions = [];
    
    // Initialize settings panel
    this.settingsPanel = null;
    
    // Initialize component
    this.initializeComponent();
  }

  createLogger() {
    return {
      debug: (...args) => console.debug('[CANVAS-DEBUG]', new Date().toISOString(), ...args),
      info: (...args) => console.info('[CANVAS-INFO]', new Date().toISOString(), ...args),
      warn: (...args) => console.warn('[CANVAS-WARN]', new Date().toISOString(), ...args),
      error: (...args) => console.error('[CANVAS-ERROR]', new Date().toISOString(), ...args)
    };
  }

  initializeComponent() {
    this.logger.info('Initializing PoseDetectionCanvas component', { containerId: this.containerId });
    
    // Create DOM structure
    this.createDOMStructure();
    
    // Initialize canvas and renderer
    this.initializeCanvas();
    
    // Set up event handlers
    this.setupEventHandlers();
    
    // Set up pose service subscription
    this.setupPoseServiceSubscription();

    this.logger.info('PoseDetectionCanvas component initialized successfully');
  }

  createDOMStructure() {
    this.container.innerHTML = `
      <div class="pose-detection-canvas-wrapper">
        <div class="pose-canvas-header">
          <div class="pose-canvas-title">
            <h3>Human Pose Detection</h3>
            <div class="connection-status">
              <span class="status-indicator" id="status-indicator-${this.containerId}"></span>
              <span class="status-text" id="status-text-${this.containerId}">Disconnected</span>
            </div>
          </div>
          <div class="pose-canvas-controls" id="controls-${this.containerId}">
            <div class="control-group primary-controls">
              <button class="btn btn-start" id="start-btn-${this.containerId}">Start</button>
              <button class="btn btn-stop" id="stop-btn-${this.containerId}" disabled>Stop</button>
              <button class="btn btn-reconnect" id="reconnect-btn-${this.containerId}" disabled>Reconnect</button>
              <button class="btn btn-demo" id="demo-btn-${this.containerId}">Demo</button>
            </div>
            <div class="control-group secondary-controls">
              <select class="mode-select" id="mode-select-${this.containerId}">
                <option value="skeleton">Skeleton</option>
                <option value="keypoints">Keypoints</option>
                <option value="heatmap">Heatmap</option>
                <option value="dense">Dense</option>
              </select>
              <button class="btn btn-settings" id="settings-btn-${this.containerId}">⚙️ Settings</button>
            </div>
          </div>
        </div>
        <div class="pose-canvas-container">
          <canvas id="pose-canvas-${this.containerId}" class="pose-canvas"></canvas>
          <div class="pose-canvas-overlay" id="overlay-${this.containerId}">
            <div class="pose-stats" id="stats-${this.containerId}"></div>
            <div class="pose-error" id="error-${this.containerId}" style="display: none;"></div>
          </div>
        </div>
      </div>
    `;

    // Add CSS styles
    this.addComponentStyles();
  }

  addComponentStyles() {
    const style = document.createElement('style');
    style.textContent = `
      .pose-detection-canvas-wrapper {
        border: 1px solid #ddd;
        border-radius: 8px;
        overflow: hidden;
        background: #f9f9f9;
        font-family: Arial, sans-serif;
      }

      .pose-canvas-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 10px 15px;
        background: #f0f0f0;
        border-bottom: 1px solid #ddd;
      }

      .pose-canvas-title {
        display: flex;
        align-items: center;
        gap: 15px;
      }

      .pose-canvas-title h3 {
        margin: 0;
        color: #333;
        font-size: 16px;
      }

      .connection-status {
        display: flex;
        align-items: center;
        gap: 5px;
      }

      .status-indicator {
        width: 10px;
        height: 10px;
        border-radius: 50%;
        background: #ccc;
        transition: background-color 0.3s;
      }

      .status-indicator.connected { background: #28a745; }
      .status-indicator.connecting { background: #ffc107; }
      .status-indicator.error { background: #dc3545; }
      .status-indicator.disconnected { background: #6c757d; }

      .status-text {
        font-size: 12px;
        color: #666;
        min-width: 80px;
      }

      .pose-canvas-controls {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 15px;
        flex-wrap: wrap;
      }

      .control-group {
        display: flex;
        align-items: center;
        gap: 8px;
      }

      .primary-controls {
        flex: 1;
      }

      .secondary-controls {
        flex-shrink: 0;
      }

      .btn {
        padding: 8px 16px;
        border: 1px solid #ddd;
        border-radius: 6px;
        background: #ffffff;
        color: #333333;
        cursor: pointer;
        font-size: 13px;
        font-weight: 500;
        transition: all 0.2s ease;
        box-shadow: 0 1px 3px rgba(0,0,0,0.1);
        text-decoration: none;
        display: inline-block;
        min-width: 80px;
        text-align: center;
      }

      .btn:hover:not(:disabled) {
        background: #f8f9fa;
        border-color: #adb5bd;
        box-shadow: 0 2px 6px rgba(0,0,0,0.15);
        transform: translateY(-1px);
      }

      .btn:active:not(:disabled) {
        transform: translateY(0);
        box-shadow: 0 1px 3px rgba(0,0,0,0.1);
      }

      .btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
        background: #e9ecef;
        color: #6c757d;
        transform: none !important;
        box-shadow: none !important;
      }

      .btn-start { 
        background: #28a745; 
        color: white; 
        border-color: #28a745; 
      }

      .btn-start:hover:not(:disabled) { 
        background: #218838; 
        border-color: #1e7e34; 
      }

      .btn-stop { 
        background: #dc3545; 
        color: white; 
        border-color: #dc3545; 
      }

      .btn-stop:hover:not(:disabled) { 
        background: #c82333; 
        border-color: #bd2130; 
      }

      .btn-reconnect { 
        background: #17a2b8; 
        color: white; 
        border-color: #17a2b8; 
      }

      .btn-reconnect:hover:not(:disabled) { 
        background: #138496; 
        border-color: #117a8b; 
      }

      .btn-demo { 
        background: #6f42c1; 
        color: white; 
        border-color: #6f42c1; 
      }

      .btn-demo:hover:not(:disabled) { 
        background: #5a32a3; 
        border-color: #512a97; 
      }

      .btn-settings { 
        background: #6c757d; 
        color: white; 
        border-color: #6c757d; 
      }

      .btn-settings:hover:not(:disabled) { 
        background: #5a6268; 
        border-color: #545b62; 
      }

      .mode-select {
        padding: 5px 8px;
        border: 1px solid #ddd;
        border-radius: 4px;
        background: #fff;
        font-size: 12px;
      }

      .pose-canvas-container {
        position: relative;
        background: #000;
      }

      .pose-canvas {
        display: block;
        width: 100%;
        height: auto;
        background: #000;
      }

      .pose-canvas-overlay {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        pointer-events: none;
        z-index: 10;
      }

      .pose-stats {
        position: absolute;
        top: 10px;
        right: 10px;
        background: rgba(0, 0, 0, 0.7);
        color: white;
        padding: 8px;
        border-radius: 4px;
        font-size: 11px;
        line-height: 1.4;
        font-family: monospace;
        max-width: 200px;
      }

      .pose-error {
        position: absolute;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        background: rgba(220, 53, 69, 0.9);
        color: white;
        padding: 15px;
        border-radius: 4px;
        font-size: 14px;
        text-align: center;
        max-width: 80%;
      }
    `;
    
    if (!document.querySelector('#pose-canvas-styles')) {
      style.id = 'pose-canvas-styles';
      document.head.appendChild(style);
    }
  }

  initializeCanvas() {
    this.canvas = document.getElementById(`pose-canvas-${this.containerId}`);
    this.canvas.width = this.config.width;
    this.canvas.height = this.config.height;

    // Initialize renderer
    this.renderer = new PoseRenderer(this.canvas, {
      showDebugInfo: this.config.enableStats,
      mode: 'skeleton'
    });

    this.logger.debug('Canvas and renderer initialized', { 
      width: this.config.width, 
      height: this.config.height 
    });

    // Handle auto-resize
    if (this.config.autoResize) {
      this.setupAutoResize();
    }
  }

  setupAutoResize() {
    const resizeObserver = new ResizeObserver(entries => {
      const entry = entries[0];
      const { width } = entry.contentRect;
      const height = Math.round(width * 0.75); // 4:3 aspect ratio
      
      this.renderer.resize(width, height);
      this.logger.debug('Canvas auto-resized', { width, height });
    });

    resizeObserver.observe(this.container);
    this.resizeObserver = resizeObserver;
  }

  setupEventHandlers() {
    // Start button
    const startBtn = document.getElementById(`start-btn-${this.containerId}`);
    startBtn.addEventListener('click', () => this.start());

    // Stop button
    const stopBtn = document.getElementById(`stop-btn-${this.containerId}`);
    stopBtn.addEventListener('click', () => this.stop());

    // Reconnect button
    const reconnectBtn = document.getElementById(`reconnect-btn-${this.containerId}`);
    reconnectBtn.addEventListener('click', () => this.reconnect());

    // Demo button
    const demoBtn = document.getElementById(`demo-btn-${this.containerId}`);
    demoBtn.addEventListener('click', () => this.toggleDemo());

    // Settings button
    const settingsBtn = document.getElementById(`settings-btn-${this.containerId}`);
    settingsBtn.addEventListener('click', () => this.showSettings());

    // Mode selector
    const modeSelect = document.getElementById(`mode-select-${this.containerId}`);
    modeSelect.addEventListener('change', (event) => {
      this.setRenderMode(event.target.value);
    });

    this.logger.debug('Event handlers set up');
  }

  setupPoseServiceSubscription() {
    // Subscribe to pose updates
    const unsubscribePose = poseService.subscribeToPoseUpdates((update) => {
      this.handlePoseUpdate(update);
    });

    this.unsubscribeFunctions.push(unsubscribePose);
    this.logger.debug('Pose service subscription set up');
  }

  handlePoseUpdate(update) {
    try {
      switch (update.type) {
        case 'pose_update':
          this.state.lastPoseData = update.data;
          this.state.frameCount++;
          this.renderPoseData(update.data);
          this.updateStats();
          this.notifyCallback('onPoseUpdate', update.data);
          break;

        case 'connected':
          this.setConnectionState('connected');
          this.clearError();
          break;

        case 'disconnected':
          this.setConnectionState('disconnected');
          break;

        case 'connecting':
          this.setConnectionState('connecting');
          break;

        case 'connection_state':
          this.setConnectionState(update.state);
          break;

        case 'error':
          this.setConnectionState('error');
          this.showError(update.error?.message || 'Connection error');
          this.notifyCallback('onError', update.error);
          break;

        default:
          this.logger.debug('Unhandled pose update type', { type: update.type });
      }
    } catch (error) {
      this.logger.error('Error handling pose update', { error: error.message, update });
      this.showError(`Update error: ${error.message}`);
    }
  }

  renderPoseData(poseData) {
    if (!this.renderer || !this.state.isActive) {
      return;
    }

    try {
      this.renderer.render(poseData, {
        frameCount: this.state.frameCount,
        connectionState: this.state.connectionState
      });
    } catch (error) {
      this.logger.error('Render error', { error: error.message });
      this.showError(`Render error: ${error.message}`);
    }
  }

  setConnectionState(state) {
    if (this.state.connectionState !== state) {
      this.logger.debug('Connection state changed', { from: this.state.connectionState, to: state });
      this.state.connectionState = state;
      this.updateConnectionIndicator();
      this.updateControls();
      this.notifyCallback('onConnectionChange', state);
    }
  }

  updateConnectionIndicator() {
    const indicator = document.getElementById(`status-indicator-${this.containerId}`);
    const text = document.getElementById(`status-text-${this.containerId}`);

    if (indicator && text) {
      indicator.className = `status-indicator ${this.state.connectionState}`;
      text.textContent = this.state.connectionState.charAt(0).toUpperCase() + 
                        this.state.connectionState.slice(1);
    }
  }

  updateControls() {
    const startBtn = document.getElementById(`start-btn-${this.containerId}`);
    const stopBtn = document.getElementById(`stop-btn-${this.containerId}`);
    const reconnectBtn = document.getElementById(`reconnect-btn-${this.containerId}`);

    if (startBtn && stopBtn && reconnectBtn) {
      const isConnected = this.state.connectionState === 'connected';
      const isActive = this.state.isActive;

      startBtn.disabled = isActive || isConnected;
      stopBtn.disabled = !isActive;
      reconnectBtn.disabled = !isActive || this.state.connectionState === 'connecting';
    }
  }

  updateStats() {
    if (!this.config.enableStats) return;

    const statsEl = document.getElementById(`stats-${this.containerId}`);
    if (!statsEl) return;

    const uptime = Math.round((Date.now() - this.state.startTime) / 1000);
    const fps = this.renderer.getPerformanceMetrics().averageFps;
    const persons = this.state.lastPoseData?.persons?.length || 0;
    const zones = Object.keys(this.state.lastPoseData?.zone_summary || {}).length;

    statsEl.innerHTML = `
      Connection: ${this.state.connectionState}<br>
      Frames: ${this.state.frameCount}<br>
      FPS: ${fps.toFixed(1)}<br>
      Persons: ${persons}<br>
      Zones: ${zones}<br>
      Uptime: ${uptime}s
    `;
  }

  showError(message) {
    this.state.errorMessage = message;
    const errorEl = document.getElementById(`error-${this.containerId}`);
    if (errorEl) {
      errorEl.textContent = message;
      errorEl.style.display = 'block';
    }
    this.logger.error('Component error', { message });
  }

  clearError() {
    this.state.errorMessage = null;
    const errorEl = document.getElementById(`error-${this.containerId}`);
    if (errorEl) {
      errorEl.style.display = 'none';
    }
  }

  // Public API methods
  async start() {
    try {
      this.logger.info('Starting pose detection');
      this.state.isActive = true;
      this.state.frameCount = 0;
      this.state.startTime = Date.now();
      
      this.clearError();
      this.updateControls();

      await poseService.startPoseStream({
        zoneIds: [this.config.zoneId],
        minConfidence: 0.3,
        maxFps: 30
      });

      this.notifyCallback('onStateChange', { isActive: true });
      this.logger.info('Pose detection started successfully');
    } catch (error) {
      this.logger.error('Failed to start pose detection', { error: error.message });
      this.state.isActive = false;
      this.updateControls();
      this.showError(`Failed to start: ${error.message}`);
      this.notifyCallback('onError', error);
    }
  }

  stop() {
    try {
      this.logger.info('Stopping pose detection');
      this.state.isActive = false;
      
      poseService.stopPoseStream();
      this.setConnectionState('disconnected');
      this.clearError();
      this.updateControls();

      // Clear canvas
      if (this.renderer) {
        this.renderer.clearCanvas();
      }

      this.notifyCallback('onStateChange', { isActive: false });
      this.logger.info('Pose detection stopped');
    } catch (error) {
      this.logger.error('Error stopping pose detection', { error: error.message });
      this.showError(`Stop error: ${error.message}`);
    }
  }

  async reconnect() {
    try {
      this.logger.info('Reconnecting pose stream');
      await poseService.reconnectStream();
    } catch (error) {
      this.logger.error('Reconnection failed', { error: error.message });
      this.showError(`Reconnection failed: ${error.message}`);
    }
  }

  setRenderMode(mode) {
    if (this.renderer) {
      this.renderer.setMode(mode);
      this.logger.info('Render mode changed', { mode });
    }
  }

  // Toggle demo mode
  toggleDemo() {
    if (this.demoState && this.demoState.isRunning) {
      this.stopDemo();
      this.updateDemoButton(false);
    } else {
      this.runDemo();
      this.updateDemoButton(true);
    }
  }

  // Demo mode - renders animated test pose data
  runDemo() {
    this.logger.info('Running animated demo mode');
    
    // Stop any existing demo animation
    this.stopDemo();
    
    // Force enable all visual elements for demo
    this.originalConfig = { ...this.renderer.config };
    this.renderer.updateConfig({
      showKeypoints: true,
      showSkeleton: true,
      showBoundingBox: true,
      showConfidence: true,
      confidenceThreshold: 0.1,
      keypointConfidenceThreshold: 0.1
    });

    // Initialize animation state
    this.demoState = {
      isRunning: true,
      frameCount: 0,
      startTime: Date.now(),
      animations: {
        person1: { type: 'walking', phase: 0, centerX: 150, centerY: 250 },
        person2: { type: 'waving', phase: 0, centerX: 350, centerY: 270 },
        person3: { type: 'dancing', phase: 0, centerX: 550, centerY: 260 }
      }
    };
    
    // Start animation loop
    this.startDemoAnimation();
    
    // Show demo notification
    this.showDemoNotification('🎭 Animated Demo Active - Walking, Waving & Dancing');
  }

  stopDemo() {
    if (this.demoState && this.demoState.isRunning) {
      this.demoState.isRunning = false;
      if (this.demoAnimationFrame) {
        cancelAnimationFrame(this.demoAnimationFrame);
      }
      if (this.originalConfig) {
        this.renderer.updateConfig(this.originalConfig);
      }
      // Clear canvas
      if (this.renderer) {
        this.renderer.clearCanvas();
      }
      this.logger.info('Demo stopped');
    }
  }

  updateDemoButton(isRunning) {
    const demoBtn = document.getElementById(`demo-btn-${this.containerId}`);
    if (demoBtn) {
      demoBtn.textContent = isRunning ? 'Stop Demo' : 'Demo';
      demoBtn.style.background = isRunning ? '#dc3545' : '#6f42c1';
      demoBtn.style.borderColor = isRunning ? '#dc3545' : '#6f42c1';
    }
  }

  startDemoAnimation() {
    if (!this.demoState || !this.demoState.isRunning) return;

    this.demoState.frameCount++;
    const elapsed = (Date.now() - this.demoState.startTime) / 1000;
    
    // Generate animated pose data
    const animatedPoseData = this.generateAnimatedPoseData(elapsed);
    
    // Render the animated data
    this.renderPoseData(animatedPoseData);
    
    // Continue animation
    this.demoAnimationFrame = requestAnimationFrame(() => this.startDemoAnimation());
  }

  generateAnimatedPoseData(time) {
    const persons = [];
    
    // Person 1: Walking animation
    const person1 = this.generateWalkingPerson(
      this.demoState.animations.person1.centerX,
      this.demoState.animations.person1.centerY,
      time * 2 // Walking speed
    );
    persons.push(person1);
    
    // Person 2: Waving animation
    const person2 = this.generateWavingPerson(
      this.demoState.animations.person2.centerX,
      this.demoState.animations.person2.centerY,
      time * 3 // Waving speed
    );
    persons.push(person2);
    
    // Person 3: Dancing animation
    const person3 = this.generateDancingPerson(
      this.demoState.animations.person3.centerX,
      this.demoState.animations.person3.centerY,
      time * 2.5 // Dancing speed
    );
    persons.push(person3);
    
    return {
      timestamp: new Date().toISOString(),
      frame_id: `demo_frame_${this.demoState.frameCount.toString().padStart(6, '0')}`,
      persons: persons,
      zone_summary: {
        demo_zone: persons.length
      },
      processing_time_ms: 12 + Math.random() * 8,
      metadata: {
        mock_data: true,
        source: 'animated_demo',
        fps: Math.round(this.demoState.frameCount / ((Date.now() - this.demoState.startTime) / 1000))
      }
    };
  }

  generateWalkingPerson(centerX, centerY, time) {
    // Walking cycle parameters
    const walkCycle = Math.sin(time) * 0.3;
    const stepPhase = Math.sin(time * 2) * 0.2;
    
    // Base keypoint positions for walking
    const keypoints = [
      // Head (nose, eyes, ears) - slight bob
      { x: centerX, y: centerY - 80 + Math.sin(time * 4) * 2, confidence: 0.95 },
      { x: centerX - 8, y: centerY - 85 + Math.sin(time * 4) * 2, confidence: 0.92 },
      { x: centerX + 8, y: centerY - 85 + Math.sin(time * 4) * 2, confidence: 0.93 },
      { x: centerX - 15, y: centerY - 82 + Math.sin(time * 4) * 2, confidence: 0.88 },
      { x: centerX + 15, y: centerY - 82 + Math.sin(time * 4) * 2, confidence: 0.89 },
      
      // Shoulders - subtle movement
      { x: centerX - 35 + walkCycle * 5, y: centerY - 40 + Math.sin(time * 4) * 1, confidence: 0.94 },
      { x: centerX + 35 - walkCycle * 5, y: centerY - 40 + Math.sin(time * 4) * 1, confidence: 0.95 },
      
      // Elbows - arm swing
      { x: centerX - 25 + walkCycle * 20, y: centerY + 10 + walkCycle * 10, confidence: 0.91 },
      { x: centerX + 25 - walkCycle * 20, y: centerY + 10 - walkCycle * 10, confidence: 0.92 },
      
      // Wrists - follow elbows
      { x: centerX - 15 + walkCycle * 25, y: centerY + 55 + walkCycle * 15, confidence: 0.87 },
      { x: centerX + 15 - walkCycle * 25, y: centerY + 55 - walkCycle * 15, confidence: 0.88 },
      
      // Hips - slight movement
      { x: centerX - 18 + walkCycle * 3, y: centerY + 60, confidence: 0.96 },
      { x: centerX + 18 - walkCycle * 3, y: centerY + 60, confidence: 0.96 },
      
      // Knees - walking motion
      { x: centerX - 20 + stepPhase * 15, y: centerY + 120 - Math.abs(stepPhase) * 10, confidence: 0.93 },
      { x: centerX + 20 - stepPhase * 15, y: centerY + 120 - Math.abs(-stepPhase) * 10, confidence: 0.94 },
      
      // Ankles - foot placement
      { x: centerX - 22 + stepPhase * 20, y: centerY + 180, confidence: 0.90 },
      { x: centerX + 22 - stepPhase * 20, y: centerY + 180, confidence: 0.91 }
    ];
    
    return {
      person_id: 'demo_walker',
      confidence: 0.94 + Math.sin(time) * 0.03,
      bbox: this.calculateBoundingBox(keypoints),
      keypoints: keypoints,
      zone_id: 'demo_zone',
      activity: 'walking'
    };
  }

  generateWavingPerson(centerX, centerY, time) {
    // Waving parameters
    const wavePhase = Math.sin(time) * 0.8;
    const armWave = Math.sin(time * 1.5) * 30;
    
    const keypoints = [
      // Head - stable
      { x: centerX, y: centerY - 80, confidence: 0.96 },
      { x: centerX - 8, y: centerY - 85, confidence: 0.94 },
      { x: centerX + 8, y: centerY - 85, confidence: 0.94 },
      { x: centerX - 15, y: centerY - 82, confidence: 0.90 },
      { x: centerX + 15, y: centerY - 82, confidence: 0.91 },
      
      // Shoulders
      { x: centerX - 35, y: centerY - 40, confidence: 0.95 },
      { x: centerX + 35, y: centerY - 40, confidence: 0.95 },
      
      // Elbows - left arm stable, right arm waving
      { x: centerX - 55, y: centerY + 10, confidence: 0.92 },
      { x: centerX + 65 + armWave * 0.3, y: centerY - 10 - Math.abs(armWave) * 0.5, confidence: 0.93 },
      
      // Wrists - dramatic wave motion
      { x: centerX - 60, y: centerY + 60, confidence: 0.88 },
      { x: centerX + 45 + armWave, y: centerY - 30 - Math.abs(armWave) * 0.8, confidence: 0.89 },
      
      // Hips - stable
      { x: centerX - 18, y: centerY + 60, confidence: 0.97 },
      { x: centerX + 18, y: centerY + 60, confidence: 0.97 },
      
      // Knees - slight movement
      { x: centerX - 20, y: centerY + 120 + Math.sin(time * 0.5) * 5, confidence: 0.94 },
      { x: centerX + 20, y: centerY + 120 + Math.sin(time * 0.5) * 5, confidence: 0.95 },
      
      // Ankles - stable
      { x: centerX - 22, y: centerY + 180, confidence: 0.92 },
      { x: centerX + 22, y: centerY + 180, confidence: 0.93 }
    ];
    
    return {
      person_id: 'demo_waver',
      confidence: 0.91 + Math.sin(time * 0.7) * 0.05,
      bbox: this.calculateBoundingBox(keypoints),
      keypoints: keypoints,
      zone_id: 'demo_zone',
      activity: 'waving'
    };
  }

  generateDancingPerson(centerX, centerY, time) {
    // Dancing parameters - more complex movement
    const dancePhase1 = Math.sin(time * 1.2) * 0.6;
    const dancePhase2 = Math.cos(time * 1.8) * 0.4;
    const bodyBob = Math.sin(time * 3) * 8;
    const hipSway = Math.sin(time * 1.5) * 15;
    
    const keypoints = [
      // Head - dancing bob
      { x: centerX + dancePhase1 * 5, y: centerY - 80 + bodyBob, confidence: 0.96 },
      { x: centerX - 8 + dancePhase1 * 5, y: centerY - 85 + bodyBob, confidence: 0.94 },
      { x: centerX + 8 + dancePhase1 * 5, y: centerY - 85 + bodyBob, confidence: 0.94 },
      { x: centerX - 15 + dancePhase1 * 5, y: centerY - 82 + bodyBob, confidence: 0.90 },
      { x: centerX + 15 + dancePhase1 * 5, y: centerY - 82 + bodyBob, confidence: 0.91 },
      
      // Shoulders - dance movement
      { x: centerX - 35 + dancePhase1 * 10, y: centerY - 40 + bodyBob * 0.5, confidence: 0.95 },
      { x: centerX + 35 + dancePhase2 * 10, y: centerY - 40 + bodyBob * 0.5, confidence: 0.95 },
      
      // Elbows - both arms dancing
      { x: centerX - 45 + dancePhase1 * 25, y: centerY + 0 + dancePhase1 * 20, confidence: 0.92 },
      { x: centerX + 45 + dancePhase2 * 25, y: centerY + 0 + dancePhase2 * 20, confidence: 0.93 },
      
      // Wrists - expressive arm movements
      { x: centerX - 40 + dancePhase1 * 35, y: centerY + 50 + dancePhase1 * 30, confidence: 0.88 },
      { x: centerX + 40 + dancePhase2 * 35, y: centerY + 50 + dancePhase2 * 30, confidence: 0.89 },
      
      // Hips - dancing sway
      { x: centerX - 18 + hipSway * 0.3, y: centerY + 60 + bodyBob * 0.3, confidence: 0.97 },
      { x: centerX + 18 + hipSway * 0.3, y: centerY + 60 + bodyBob * 0.3, confidence: 0.97 },
      
      // Knees - dancing steps
      { x: centerX - 20 + hipSway * 0.5 + Math.sin(time * 2.5) * 10, y: centerY + 120 + Math.abs(Math.sin(time * 2.5)) * 15, confidence: 0.94 },
      { x: centerX + 20 + hipSway * 0.5 + Math.cos(time * 2.5) * 10, y: centerY + 120 + Math.abs(Math.cos(time * 2.5)) * 15, confidence: 0.95 },
      
      // Ankles - feet positioning
      { x: centerX - 22 + hipSway * 0.6 + Math.sin(time * 2.5) * 12, y: centerY + 180, confidence: 0.92 },
      { x: centerX + 22 + hipSway * 0.6 + Math.cos(time * 2.5) * 12, y: centerY + 180, confidence: 0.93 }
    ];
    
    return {
      person_id: 'demo_dancer',
      confidence: 0.89 + Math.sin(time * 1.3) * 0.07,
      bbox: this.calculateBoundingBox(keypoints),
      keypoints: keypoints,
      zone_id: 'demo_zone',
      activity: 'dancing'
    };
  }

  calculateBoundingBox(keypoints) {
    const validPoints = keypoints.filter(kp => kp.confidence > 0.1);
    if (validPoints.length === 0) return { x: 0, y: 0, width: 50, height: 50 };
    
    const xs = validPoints.map(kp => kp.x);
    const ys = validPoints.map(kp => kp.y);
    
    const minX = Math.min(...xs) - 10;
    const maxX = Math.max(...xs) + 10;
    const minY = Math.min(...ys) - 10;
    const maxY = Math.max(...ys) + 10;
    
    return {
      x: minX,
      y: minY,
      width: maxX - minX,
      height: maxY - minY
    };
  }

  generateDemoKeypoints(centerX, centerY) {
    // COCO keypoint order: nose, left_eye, right_eye, left_ear, right_ear,
    // left_shoulder, right_shoulder, left_elbow, right_elbow, left_wrist, right_wrist,
    // left_hip, right_hip, left_knee, right_knee, left_ankle, right_ankle
    const offsets = [
      [0, -80],     // nose
      [-10, -90],   // left_eye
      [10, -90],    // right_eye
      [-20, -85],   // left_ear
      [20, -85],    // right_ear
      [-40, -40],   // left_shoulder
      [40, -40],    // right_shoulder
      [-60, 10],    // left_elbow
      [60, 10],     // right_elbow
      [-65, 60],    // left_wrist
      [65, 60],     // right_wrist
      [-20, 60],    // left_hip
      [20, 60],     // right_hip
      [-25, 120],   // left_knee
      [25, 120],    // right_knee
      [-25, 180],   // left_ankle
      [25, 180]     // right_ankle
    ];
    
    return offsets.map(([dx, dy]) => ({
      x: centerX + dx,
      y: centerY + dy,
      confidence: 0.8 + (Math.random() * 0.2)
    }));
  }

  showDemoNotification(message = '🎭 Demo Mode Active') {
    const notification = document.createElement('div');
    notification.style.cssText = `
      position: absolute;
      top: 10px;
      left: 10px;
      background: rgba(111, 66, 193, 0.9);
      color: white;
      padding: 10px 15px;
      border-radius: 4px;
      font-size: 14px;
      z-index: 20;
      pointer-events: none;
      box-shadow: 0 2px 8px rgba(0,0,0,0.3);
    `;
    notification.textContent = message;
    
    const overlay = document.getElementById(`overlay-${this.containerId}`);
    
    // Remove any existing notifications
    const existingNotifications = overlay.querySelectorAll('div[style*="background: rgba(111, 66, 193"]');
    existingNotifications.forEach(n => n.remove());
    
    overlay.appendChild(notification);
    
    // Remove notification after 3 seconds
    setTimeout(() => {
      if (notification.parentNode) {
        notification.parentNode.removeChild(notification);
      }
    }, 3000);
  }

  // Configuration methods
  updateConfig(newConfig) {
    this.config = { ...this.config, ...newConfig };
    
    if (this.renderer) {
      this.renderer.updateConfig(newConfig);
    }
    
    this.logger.debug('Component configuration updated', { config: this.config });
  }

  // Callback management
  setCallback(eventName, callback) {
    if (eventName in this.callbacks) {
      this.callbacks[eventName] = callback;
    }
  }

  notifyCallback(eventName, data) {
    if (this.callbacks[eventName]) {
      try {
        this.callbacks[eventName](data);
      } catch (error) {
        this.logger.error('Callback error', { eventName, error: error.message });
      }
    }
  }

  // Utility methods
  getState() {
    return { ...this.state };
  }

  getPerformanceMetrics() {
    return this.renderer ? this.renderer.getPerformanceMetrics() : null;
  }

  exportFrame(format = 'png') {
    return this.renderer ? this.renderer.exportFrame(format) : null;
  }

  // Test method for debugging
  renderTestShape() {
    if (this.renderer) {
      this.renderer.renderTestShape();
    }
  }

  // Show settings modal
  showSettings() {
    this.logger.info('Opening settings modal');
    
    if (!this.settingsPanel) {
      this.createSettingsModal();
    }
    
    this.settingsPanel.show();
  }

  createSettingsModal() {
    // Create a temporary container for the settings panel
    const modalContainer = document.createElement('div');
    modalContainer.id = `settings-modal-${this.containerId}`;
    modalContainer.className = 'settings-modal-wrapper';
    modalContainer.innerHTML = `
      <div class="settings-modal-overlay">
        <div class="settings-modal-dialog">
          <div class="settings-modal-header">
            <h2>⚙️ Pose Detection Settings</h2>
            <button class="settings-modal-close" type="button">×</button>
          </div>
          <div class="settings-modal-body" id="settings-container-${this.containerId}">
            <!-- Settings panel will be inserted here -->
          </div>
        </div>
      </div>
    `;
    
    document.body.appendChild(modalContainer);
    
    // Create the settings panel inside the modal
    this.settingsPanel = new SettingsPanel(`settings-container-${this.containerId}`, {
      enableAdvancedSettings: true,
      enableDebugControls: true,
      enableExportFeatures: true,
      allowConfigPersistence: true,
      initialSettings: this.getInitialSettings()
    });
    
    // Set up settings panel callbacks
    this.settingsPanel.setCallback('onSettingsChange', (data) => {
      this.handleSettingsChange(data);
    });
    
    this.settingsPanel.setCallback('onRenderModeChange', (mode) => {
      this.setRenderMode(mode);
    });
    
    // Set up modal event handlers
    this.setupModalEventHandlers(modalContainer);
    
    // Add modal styles
    this.addModalStyles();
    
    // Add show/hide methods to the modal
    modalContainer.show = () => {
      modalContainer.style.display = 'flex';
      modalContainer.classList.add('active');
      document.body.style.overflow = 'hidden';
    };
    
    modalContainer.hide = () => {
      modalContainer.style.display = 'none';
      modalContainer.classList.remove('active');
      document.body.style.overflow = '';
    };
    
    this.settingsPanel.show = () => modalContainer.show();
    this.settingsPanel.hide = () => modalContainer.hide();
    
    this.logger.debug('Settings modal created');
  }

  setupModalEventHandlers(modalContainer) {
    // Close button
    const closeBtn = modalContainer.querySelector('.settings-modal-close');
    closeBtn.addEventListener('click', () => {
      this.settingsPanel.hide();
    });
    
    // Overlay click to close
    const overlay = modalContainer.querySelector('.settings-modal-overlay');
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) {
        this.settingsPanel.hide();
      }
    });
    
    // Escape key to close
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && modalContainer.classList.contains('active')) {
        this.settingsPanel.hide();
      }
    });
  }

  addModalStyles() {
    if (document.querySelector('#pose-canvas-modal-styles')) return;
    
    const style = document.createElement('style');
    style.id = 'pose-canvas-modal-styles';
    style.textContent = `
      .settings-modal-wrapper {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        z-index: 10000;
        display: none;
        opacity: 0;
        transition: opacity 0.3s ease;
      }

      .settings-modal-wrapper.active {
        opacity: 1;
      }

      .settings-modal-overlay {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.6);
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 20px;
        backdrop-filter: blur(5px);
      }

      .settings-modal-dialog {
        background: white;
        border-radius: 12px;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
        width: 100%;
        max-width: 800px;
        max-height: 90vh;
        overflow: hidden;
        transform: scale(0.9);
        transition: transform 0.3s ease;
        display: flex;
        flex-direction: column;
      }

      .settings-modal-wrapper.active .settings-modal-dialog {
        transform: scale(1);
      }

      .settings-modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 20px 24px;
        border-bottom: 1px solid #e9ecef;
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
      }

      .settings-modal-header h2 {
        margin: 0;
        font-size: 20px;
        font-weight: 600;
      }

      .settings-modal-close {
        background: rgba(255, 255, 255, 0.2);
        border: none;
        color: white;
        font-size: 24px;
        cursor: pointer;
        padding: 8px;
        width: 40px;
        height: 40px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: all 0.2s ease;
        backdrop-filter: blur(10px);
      }

      .settings-modal-close:hover {
        background: rgba(255, 255, 255, 0.3);
        transform: scale(1.1);
      }

      .settings-modal-body {
        flex: 1;
        overflow-y: auto;
        padding: 0;
      }

      /* Override settings panel styles for modal */
      .settings-modal-body .settings-panel {
        border: none;
        border-radius: 0;
        box-shadow: none;
      }

      .settings-modal-body .settings-header {
        display: none;
      }

      .settings-modal-body .settings-content {
        max-height: none;
        padding: 24px;
      }

      /* Custom scrollbar for modal */
      .settings-modal-body::-webkit-scrollbar {
        width: 8px;
      }

      .settings-modal-body::-webkit-scrollbar-track {
        background: #f1f1f1;
        border-radius: 4px;
      }

      .settings-modal-body::-webkit-scrollbar-thumb {
        background: #c1c1c1;
        border-radius: 4px;
      }

      .settings-modal-body::-webkit-scrollbar-thumb:hover {
        background: #a8a8a8;
      }

      /* Mobile responsive */
      @media (max-width: 768px) {
        .settings-modal-overlay {
          padding: 10px;
        }
        
        .settings-modal-dialog {
          max-width: 100%;
          max-height: 95vh;
        }
        
        .settings-modal-header {
          padding: 15px 20px;
        }
        
        .settings-modal-body .settings-content {
          padding: 20px;
        }
      }
    `;
    
    document.head.appendChild(style);
  }

  getInitialSettings() {
    return {
      // Get current renderer config
      ...(this.renderer ? this.renderer.getConfig() : {}),
      // Add other relevant settings
      currentZone: this.config.zoneId || 'zone_1',
      maxFps: 30,
      autoReconnect: true,
      connectionTimeout: 10000
    };
  }

  handleSettingsChange(data) {
    this.logger.debug('Settings changed', data);
    
    if (data.settings && this.renderer) {
      // Apply render settings
      const renderConfig = {
        mode: data.settings.renderMode,
        showKeypoints: data.settings.showKeypoints,
        showSkeleton: data.settings.showSkeleton,
        showBoundingBox: data.settings.showBoundingBox,
        showConfidence: data.settings.showConfidence,
        showZones: data.settings.showZones,
        showDebugInfo: data.settings.showDebugInfo,
        skeletonColor: data.settings.skeletonColor,
        keypointColor: data.settings.keypointColor,
        boundingBoxColor: data.settings.boundingBoxColor,
        confidenceThreshold: data.settings.confidenceThreshold,
        keypointConfidenceThreshold: data.settings.keypointConfidenceThreshold,
        enableSmoothing: data.settings.enableSmoothing
      };
      
      this.renderer.updateConfig(renderConfig);
      this.logger.info('Renderer config updated from settings');
    }
  }

  // Cleanup
  dispose() {
    this.logger.info('Disposing PoseDetectionCanvas component');
    
    try {
      // Stop pose detection
      if (this.state.isActive) {
        this.stop();
      }

      // Stop demo animation
      this.stopDemo();

      // Dispose settings panel
      if (this.settingsPanel) {
        this.settingsPanel.dispose();
        const modalContainer = document.getElementById(`settings-modal-${this.containerId}`);
        if (modalContainer) {
          modalContainer.remove();
        }
      }

      // Unsubscribe from pose service
      this.unsubscribeFunctions.forEach(unsubscribe => unsubscribe());
      this.unsubscribeFunctions = [];

      // Clean up resize observer
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
      }

      // Clear DOM
      if (this.container) {
        this.container.innerHTML = '';
      }

      this.logger.info('PoseDetectionCanvas component disposed successfully');
    } catch (error) {
      this.logger.error('Error during disposal', { error: error.message });
    }
  }
}