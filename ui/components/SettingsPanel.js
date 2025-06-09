// SettingsPanel Component for WiFi-DensePose UI

import { poseService } from '../services/pose.service.js';
import { wsService } from '../services/websocket.service.js';

export class SettingsPanel {
  constructor(containerId, options = {}) {
    this.containerId = containerId;
    this.container = document.getElementById(containerId);
    
    if (!this.container) {
      throw new Error(`Container with ID '${containerId}' not found`);
    }

    this.config = {
      enableAdvancedSettings: true,
      enableDebugControls: true,
      enableExportFeatures: true,
      allowConfigPersistence: true,
      ...options
    };

    this.settings = {
      // Connection settings
      zones: ['zone_1', 'zone_2', 'zone_3'],
      currentZone: 'zone_1',
      autoReconnect: true,
      connectionTimeout: 10000,
      
      // Pose detection settings
      confidenceThreshold: 0.3,
      keypointConfidenceThreshold: 0.1,
      maxPersons: 10,
      maxFps: 30,
      
      // Rendering settings
      renderMode: 'skeleton',
      showKeypoints: true,
      showSkeleton: true,
      showBoundingBox: false,
      showConfidence: true,
      showZones: true,
      showDebugInfo: false,
      
      // Colors
      skeletonColor: '#00ff00',
      keypointColor: '#ff0000',
      boundingBoxColor: '#0000ff',
      
      // Performance settings
      enableValidation: true,
      enablePerformanceTracking: true,
      enableDebugLogging: false,
      
      // Advanced settings
      heartbeatInterval: 30000,
      maxReconnectAttempts: 10,
      enableSmoothing: true
    };

    this.callbacks = {
      onSettingsChange: null,
      onZoneChange: null,
      onRenderModeChange: null,
      onExport: null,
      onImport: null
    };

    this.logger = this.createLogger();
    
    // Initialize component
    this.initializeComponent();
  }

  createLogger() {
    return {
      debug: (...args) => console.debug('[SETTINGS-DEBUG]', new Date().toISOString(), ...args),
      info: (...args) => console.info('[SETTINGS-INFO]', new Date().toISOString(), ...args),
      warn: (...args) => console.warn('[SETTINGS-WARN]', new Date().toISOString(), ...args),
      error: (...args) => console.error('[SETTINGS-ERROR]', new Date().toISOString(), ...args)
    };
  }

  initializeComponent() {
    this.logger.info('Initializing SettingsPanel component', { containerId: this.containerId });
    
    // Load saved settings
    this.loadSettings();
    
    // Create DOM structure
    this.createDOMStructure();
    
    // Set up event handlers
    this.setupEventHandlers();
    
    // Update UI with current settings
    this.updateUI();
    
    this.logger.info('SettingsPanel component initialized successfully');
  }

  createDOMStructure() {
    this.container.innerHTML = `
      <div class="settings-panel">
        <div class="settings-header">
          <h3>Pose Detection Settings</h3>
          <div class="settings-actions">
            <button class="btn btn-sm" id="reset-settings-${this.containerId}">Reset</button>
            <button class="btn btn-sm" id="export-settings-${this.containerId}">Export</button>
            <button class="btn btn-sm" id="import-settings-${this.containerId}">Import</button>
          </div>
        </div>
        
        <div class="settings-content">
          <!-- Connection Settings -->
          <div class="settings-section">
            <h4>Connection</h4>
            <div class="setting-row">
              <label for="zone-select-${this.containerId}">Zone:</label>
              <select id="zone-select-${this.containerId}" class="setting-select">
                ${this.settings.zones.map(zone => 
                  `<option value="${zone}">${zone.replace('_', ' ').toUpperCase()}</option>`
                ).join('')}
              </select>
            </div>
            <div class="setting-row">
              <label for="auto-reconnect-${this.containerId}">Auto Reconnect:</label>
              <input type="checkbox" id="auto-reconnect-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="connection-timeout-${this.containerId}">Timeout (ms):</label>
              <input type="number" id="connection-timeout-${this.containerId}" class="setting-input" min="1000" max="30000" step="1000">
            </div>
          </div>

          <!-- Detection Settings -->
          <div class="settings-section">
            <h4>Detection</h4>
            <div class="setting-row">
              <label for="confidence-threshold-${this.containerId}">Confidence Threshold:</label>
              <input type="range" id="confidence-threshold-${this.containerId}" class="setting-range" min="0" max="1" step="0.1">
              <span id="confidence-value-${this.containerId}" class="setting-value">0.3</span>
            </div>
            <div class="setting-row">
              <label for="keypoint-confidence-${this.containerId}">Keypoint Confidence:</label>
              <input type="range" id="keypoint-confidence-${this.containerId}" class="setting-range" min="0" max="1" step="0.1">
              <span id="keypoint-confidence-value-${this.containerId}" class="setting-value">0.1</span>
            </div>
            <div class="setting-row">
              <label for="max-persons-${this.containerId}">Max Persons:</label>
              <input type="number" id="max-persons-${this.containerId}" class="setting-input" min="1" max="20">
            </div>
            <div class="setting-row">
              <label for="max-fps-${this.containerId}">Max FPS:</label>
              <input type="number" id="max-fps-${this.containerId}" class="setting-input" min="1" max="60">
            </div>
          </div>

          <!-- Rendering Settings -->
          <div class="settings-section">
            <h4>Rendering</h4>
            <div class="setting-row">
              <label for="render-mode-${this.containerId}">Mode:</label>
              <select id="render-mode-${this.containerId}" class="setting-select">
                <option value="skeleton">Skeleton</option>
                <option value="keypoints">Keypoints</option>
                <option value="heatmap">Heatmap</option>
                <option value="dense">Dense</option>
              </select>
            </div>
            <div class="setting-row">
              <label for="show-keypoints-${this.containerId}">Show Keypoints:</label>
              <input type="checkbox" id="show-keypoints-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="show-skeleton-${this.containerId}">Show Skeleton:</label>
              <input type="checkbox" id="show-skeleton-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="show-bounding-box-${this.containerId}">Show Bounding Box:</label>
              <input type="checkbox" id="show-bounding-box-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="show-confidence-${this.containerId}">Show Confidence:</label>
              <input type="checkbox" id="show-confidence-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="show-zones-${this.containerId}">Show Zones:</label>
              <input type="checkbox" id="show-zones-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="show-debug-info-${this.containerId}">Show Debug Info:</label>
              <input type="checkbox" id="show-debug-info-${this.containerId}" class="setting-checkbox">
            </div>
          </div>

          <!-- Color Settings -->
          <div class="settings-section">
            <h4>Colors</h4>
            <div class="setting-row">
              <label for="skeleton-color-${this.containerId}">Skeleton:</label>
              <input type="color" id="skeleton-color-${this.containerId}" class="setting-color">
            </div>
            <div class="setting-row">
              <label for="keypoint-color-${this.containerId}">Keypoints:</label>
              <input type="color" id="keypoint-color-${this.containerId}" class="setting-color">
            </div>
            <div class="setting-row">
              <label for="bounding-box-color-${this.containerId}">Bounding Box:</label>
              <input type="color" id="bounding-box-color-${this.containerId}" class="setting-color">
            </div>
          </div>

          <!-- Performance Settings -->
          <div class="settings-section">
            <h4>Performance</h4>
            <div class="setting-row">
              <label for="enable-validation-${this.containerId}">Enable Validation:</label>
              <input type="checkbox" id="enable-validation-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="enable-performance-tracking-${this.containerId}">Performance Tracking:</label>
              <input type="checkbox" id="enable-performance-tracking-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="enable-debug-logging-${this.containerId}">Debug Logging:</label>
              <input type="checkbox" id="enable-debug-logging-${this.containerId}" class="setting-checkbox">
            </div>
            <div class="setting-row">
              <label for="enable-smoothing-${this.containerId}">Enable Smoothing:</label>
              <input type="checkbox" id="enable-smoothing-${this.containerId}" class="setting-checkbox">
            </div>
          </div>

          <!-- Advanced Settings -->
          <div class="settings-section advanced-section" id="advanced-section-${this.containerId}" style="display: none;">
            <h4>Advanced</h4>
            <div class="setting-row">
              <label for="heartbeat-interval-${this.containerId}">Heartbeat Interval (ms):</label>
              <input type="number" id="heartbeat-interval-${this.containerId}" class="setting-input" min="5000" max="60000" step="5000">
            </div>
            <div class="setting-row">
              <label for="max-reconnect-attempts-${this.containerId}">Max Reconnect Attempts:</label>
              <input type="number" id="max-reconnect-attempts-${this.containerId}" class="setting-input" min="1" max="20">
            </div>
          </div>
          
          <div class="settings-toggle">
            <button class="btn btn-sm" id="toggle-advanced-${this.containerId}">Show Advanced</button>
          </div>
        </div>
        
        <div class="settings-footer">
          <div class="settings-status" id="settings-status-${this.containerId}">
            Settings loaded
          </div>
        </div>
      </div>
      
      <input type="file" id="import-file-${this.containerId}" accept=".json" style="display: none;">
    `;

    this.addSettingsStyles();
  }

  addSettingsStyles() {
    const style = document.createElement('style');
    style.textContent = `
      .settings-panel {
        background: #fff;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-family: Arial, sans-serif;
        overflow: hidden;
      }

      .settings-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 15px 20px;
        background: #f8f9fa;
        border-bottom: 1px solid #ddd;
      }

      .settings-header h3 {
        margin: 0;
        color: #333;
        font-size: 16px;
        font-weight: 600;
      }

      .settings-actions {
        display: flex;
        gap: 8px;
      }

      .settings-content {
        padding: 20px;
        max-height: 400px;
        overflow-y: auto;
      }

      .settings-section {
        margin-bottom: 25px;
        padding-bottom: 20px;
        border-bottom: 1px solid #eee;
      }

      .settings-section:last-child {
        border-bottom: none;
        margin-bottom: 0;
        padding-bottom: 0;
      }

      .settings-section h4 {
        margin: 0 0 15px 0;
        color: #555;
        font-size: 14px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.5px;
      }

      .setting-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 12px;
        gap: 10px;
      }

      .setting-row label {
        flex: 1;
        color: #666;
        font-size: 13px;
        font-weight: 500;
      }

      .setting-input, .setting-select {
        flex: 0 0 120px;
        padding: 6px 8px;
        border: 1px solid #ddd;
        border-radius: 4px;
        font-size: 13px;
      }

      .setting-range {
        flex: 0 0 100px;
        margin-right: 8px;
      }

      .setting-value {
        flex: 0 0 40px;
        font-size: 12px;
        color: #666;
        text-align: center;
        background: #f8f9fa;
        padding: 2px 6px;
        border-radius: 3px;
        border: 1px solid #ddd;
      }

      .setting-checkbox {
        flex: 0 0 auto;
        width: 18px;
        height: 18px;
      }

      .setting-color {
        flex: 0 0 50px;
        height: 30px;
        border: 1px solid #ddd;
        border-radius: 4px;
        cursor: pointer;
      }

      .btn {
        padding: 6px 12px;
        border: 1px solid #ddd;
        border-radius: 4px;
        background: #fff;
        cursor: pointer;
        font-size: 12px;
        transition: all 0.2s;
      }

      .btn:hover {
        background: #f8f9fa;
        border-color: #adb5bd;
      }

      .btn-sm {
        padding: 4px 8px;
        font-size: 11px;
      }

      .settings-toggle {
        text-align: center;
        padding-top: 15px;
        border-top: 1px solid #eee;
      }

      .settings-footer {
        padding: 10px 20px;
        background: #f8f9fa;
        border-top: 1px solid #ddd;
        text-align: center;
      }

      .settings-status {
        font-size: 12px;
        color: #666;
      }

      .advanced-section {
        background: #f9f9f9;
        margin: 0 -20px 25px -20px;
        padding: 20px;
        border: none;
        border-top: 1px solid #ddd;
        border-bottom: 1px solid #ddd;
      }

      .advanced-section h4 {
        color: #dc3545;
      }
    `;
    
    if (!document.querySelector('#settings-panel-styles')) {
      style.id = 'settings-panel-styles';
      document.head.appendChild(style);
    }
  }

  setupEventHandlers() {
    // Reset button
    const resetBtn = document.getElementById(`reset-settings-${this.containerId}`);
    resetBtn?.addEventListener('click', () => this.resetSettings());

    // Export button
    const exportBtn = document.getElementById(`export-settings-${this.containerId}`);
    exportBtn?.addEventListener('click', () => this.exportSettings());

    // Import button and file input
    const importBtn = document.getElementById(`import-settings-${this.containerId}`);
    const importFile = document.getElementById(`import-file-${this.containerId}`);
    importBtn?.addEventListener('click', () => importFile.click());
    importFile?.addEventListener('change', (e) => this.importSettings(e));

    // Advanced toggle
    const advancedToggle = document.getElementById(`toggle-advanced-${this.containerId}`);
    advancedToggle?.addEventListener('click', () => this.toggleAdvanced());

    // Setting change handlers
    this.setupSettingChangeHandlers();

    this.logger.debug('Event handlers set up');
  }

  setupSettingChangeHandlers() {
    // Zone selector
    const zoneSelect = document.getElementById(`zone-select-${this.containerId}`);
    zoneSelect?.addEventListener('change', (e) => {
      this.updateSetting('currentZone', e.target.value);
      this.notifyCallback('onZoneChange', e.target.value);
    });

    // Render mode
    const renderModeSelect = document.getElementById(`render-mode-${this.containerId}`);
    renderModeSelect?.addEventListener('change', (e) => {
      this.updateSetting('renderMode', e.target.value);
      this.notifyCallback('onRenderModeChange', e.target.value);
    });

    // Range inputs with value display
    const rangeInputs = ['confidence-threshold', 'keypoint-confidence'];
    rangeInputs.forEach(id => {
      const input = document.getElementById(`${id}-${this.containerId}`);
      const valueSpan = document.getElementById(`${id}-value-${this.containerId}`);
      
      input?.addEventListener('input', (e) => {
        const value = parseFloat(e.target.value);
        valueSpan.textContent = value.toFixed(1);
        
        const settingKey = id.replace('-', '_').replace('_threshold', 'Threshold').replace('_confidence', 'ConfidenceThreshold');
        this.updateSetting(settingKey, value);
      });
    });

    // Checkbox inputs
    const checkboxes = [
      'auto-reconnect', 'show-keypoints', 'show-skeleton', 'show-bounding-box',
      'show-confidence', 'show-zones', 'show-debug-info', 'enable-validation',
      'enable-performance-tracking', 'enable-debug-logging', 'enable-smoothing'
    ];
    
    checkboxes.forEach(id => {
      const input = document.getElementById(`${id}-${this.containerId}`);
      input?.addEventListener('change', (e) => {
        const settingKey = this.camelCase(id);
        this.updateSetting(settingKey, e.target.checked);
      });
    });

    // Number inputs
    const numberInputs = [
      'connection-timeout', 'max-persons', 'max-fps', 
      'heartbeat-interval', 'max-reconnect-attempts'
    ];
    
    numberInputs.forEach(id => {
      const input = document.getElementById(`${id}-${this.containerId}`);
      input?.addEventListener('change', (e) => {
        const settingKey = this.camelCase(id);
        this.updateSetting(settingKey, parseInt(e.target.value));
      });
    });

    // Color inputs
    const colorInputs = ['skeleton-color', 'keypoint-color', 'bounding-box-color'];
    colorInputs.forEach(id => {
      const input = document.getElementById(`${id}-${this.containerId}`);
      input?.addEventListener('change', (e) => {
        const settingKey = this.camelCase(id);
        this.updateSetting(settingKey, e.target.value);
      });
    });
  }

  camelCase(str) {
    return str.replace(/-./g, match => match.charAt(1).toUpperCase());
  }

  updateSetting(key, value) {
    this.settings[key] = value;
    this.saveSettings();
    this.notifyCallback('onSettingsChange', { key, value, settings: this.settings });
    this.updateStatus(`Updated ${key}`);
    this.logger.debug('Setting updated', { key, value });
  }

  updateUI() {
    // Update all form elements with current settings
    Object.entries(this.settings).forEach(([key, value]) => {
      this.updateUIElement(key, value);
    });
  }

  updateUIElement(key, value) {
    const kebabKey = key.replace(/([A-Z])/g, '-$1').toLowerCase();
    
    // Handle special cases
    const elementId = `${kebabKey}-${this.containerId}`;
    const element = document.getElementById(elementId);
    
    if (!element) return;

    switch (element.type) {
      case 'checkbox':
        element.checked = value;
        break;
      case 'range':
        element.value = value;
        // Update value display
        const valueSpan = document.getElementById(`${kebabKey}-value-${this.containerId}`);
        if (valueSpan) valueSpan.textContent = value.toFixed(1);
        break;
      case 'color':
        element.value = value;
        break;
      default:
        element.value = value;
    }
  }

  toggleAdvanced() {
    const advancedSection = document.getElementById(`advanced-section-${this.containerId}`);
    const toggleBtn = document.getElementById(`toggle-advanced-${this.containerId}`);
    
    const isVisible = advancedSection.style.display !== 'none';
    advancedSection.style.display = isVisible ? 'none' : 'block';
    toggleBtn.textContent = isVisible ? 'Show Advanced' : 'Hide Advanced';
    
    this.logger.debug('Advanced settings toggled', { visible: !isVisible });
  }

  resetSettings() {
    if (confirm('Reset all settings to defaults? This cannot be undone.')) {
      this.settings = this.getDefaultSettings();
      this.updateUI();
      this.saveSettings();
      this.notifyCallback('onSettingsChange', { reset: true, settings: this.settings });
      this.updateStatus('Settings reset to defaults');
      this.logger.info('Settings reset to defaults');
    }
  }

  exportSettings() {
    const data = {
      timestamp: new Date().toISOString(),
      version: '1.0',
      settings: this.settings
    };
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `pose-detection-settings-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
    
    this.updateStatus('Settings exported');
    this.notifyCallback('onExport', data);
    this.logger.info('Settings exported');
  }

  importSettings(event) {
    const file = event.target.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      try {
        const data = JSON.parse(e.target.result);
        
        if (data.settings) {
          this.settings = { ...this.getDefaultSettings(), ...data.settings };
          this.updateUI();
          this.saveSettings();
          this.notifyCallback('onSettingsChange', { imported: true, settings: this.settings });
          this.notifyCallback('onImport', data);
          this.updateStatus('Settings imported successfully');
          this.logger.info('Settings imported successfully');
        } else {
          throw new Error('Invalid settings file format');
        }
      } catch (error) {
        this.updateStatus('Error importing settings');
        this.logger.error('Error importing settings', { error: error.message });
        alert('Error importing settings: ' + error.message);
      }
    };
    
    reader.readAsText(file);
    event.target.value = ''; // Reset file input
  }

  saveSettings() {
    if (this.config.allowConfigPersistence) {
      try {
        localStorage.setItem(`pose-settings-${this.containerId}`, JSON.stringify(this.settings));
      } catch (error) {
        this.logger.warn('Failed to save settings to localStorage', { error: error.message });
      }
    }
  }

  loadSettings() {
    if (this.config.allowConfigPersistence) {
      try {
        const saved = localStorage.getItem(`pose-settings-${this.containerId}`);
        if (saved) {
          this.settings = { ...this.getDefaultSettings(), ...JSON.parse(saved) };
          this.logger.debug('Settings loaded from localStorage');
        }
      } catch (error) {
        this.logger.warn('Failed to load settings from localStorage', { error: error.message });
      }
    }
  }

  getDefaultSettings() {
    return {
      zones: ['zone_1', 'zone_2', 'zone_3'],
      currentZone: 'zone_1',
      autoReconnect: true,
      connectionTimeout: 10000,
      confidenceThreshold: 0.3,
      keypointConfidenceThreshold: 0.1,
      maxPersons: 10,
      maxFps: 30,
      renderMode: 'skeleton',
      showKeypoints: true,
      showSkeleton: true,
      showBoundingBox: false,
      showConfidence: true,
      showZones: true,
      showDebugInfo: false,
      skeletonColor: '#00ff00',
      keypointColor: '#ff0000',
      boundingBoxColor: '#0000ff',
      enableValidation: true,
      enablePerformanceTracking: true,
      enableDebugLogging: false,
      heartbeatInterval: 30000,
      maxReconnectAttempts: 10,
      enableSmoothing: true
    };
  }

  updateStatus(message) {
    const statusElement = document.getElementById(`settings-status-${this.containerId}`);
    if (statusElement) {
      statusElement.textContent = message;
      
      // Clear status after 3 seconds
      setTimeout(() => {
        statusElement.textContent = 'Settings ready';
      }, 3000);
    }
  }

  // Public API methods
  getSettings() {
    return { ...this.settings };
  }

  setSetting(key, value) {
    this.updateSetting(key, value);
  }

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

  // Apply settings to services
  applyToServices() {
    try {
      // Apply pose service settings
      poseService.updateConfig({
        enableValidation: this.settings.enableValidation,
        enablePerformanceTracking: this.settings.enablePerformanceTracking,
        confidenceThreshold: this.settings.confidenceThreshold,
        maxPersons: this.settings.maxPersons
      });

      // Apply WebSocket service settings
      if (wsService.updateConfig) {
        wsService.updateConfig({
          enableDebugLogging: this.settings.enableDebugLogging,
          heartbeatInterval: this.settings.heartbeatInterval,
          maxReconnectAttempts: this.settings.maxReconnectAttempts
        });
      }

      this.updateStatus('Settings applied to services');
      this.logger.info('Settings applied to services');
    } catch (error) {
      this.logger.error('Error applying settings to services', { error: error.message });
      this.updateStatus('Error applying settings');
    }
  }

  // Get render configuration for PoseRenderer
  getRenderConfig() {
    return {
      mode: this.settings.renderMode,
      showKeypoints: this.settings.showKeypoints,
      showSkeleton: this.settings.showSkeleton,
      showBoundingBox: this.settings.showBoundingBox,
      showConfidence: this.settings.showConfidence,
      showZones: this.settings.showZones,
      showDebugInfo: this.settings.showDebugInfo,
      skeletonColor: this.settings.skeletonColor,
      keypointColor: this.settings.keypointColor,
      boundingBoxColor: this.settings.boundingBoxColor,
      confidenceThreshold: this.settings.confidenceThreshold,
      keypointConfidenceThreshold: this.settings.keypointConfidenceThreshold,
      enableSmoothing: this.settings.enableSmoothing
    };
  }

  // Get stream configuration for PoseService
  getStreamConfig() {
    return {
      zoneIds: [this.settings.currentZone],
      minConfidence: this.settings.confidenceThreshold,
      maxFps: this.settings.maxFps
    };
  }

  // Cleanup
  dispose() {
    this.logger.info('Disposing SettingsPanel component');
    
    try {
      // Save settings before disposing
      this.saveSettings();
      
      // Clear container
      if (this.container) {
        this.container.innerHTML = '';
      }
      
      this.logger.info('SettingsPanel component disposed successfully');
    } catch (error) {
      this.logger.error('Error during disposal', { error: error.message });
    }
  }
}