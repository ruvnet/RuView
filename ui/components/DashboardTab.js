// Dashboard Tab Component

import { healthService } from '../services/health.service.js';
import { poseService } from '../services/pose.service.js';
import { sensingService } from '../services/sensing.service.js';
import { apiService } from '../services/api.service.js';
import { i18n } from '../utils/i18n.js';

export class DashboardTab {
  constructor(containerElement) {
    this.container = containerElement;
    this.statsElements = {};
    this.healthSubscription = null;
    this.statsInterval = null;
  }

  // Initialize component
  async init() {
    this.cacheElements();
    this.wireManualForm();
    this.wireBluetoothToggle();
    this._localeUnsub = i18n.onLocaleChange(() => {
      i18n.applyTranslations(this.container);
      this.updateDataSourceIndicator();
      if (this._lastHealth) this.updateHealthStatus(this._lastHealth);
      if (this._lastHybrid) this.updateHybridSummary(this._lastHybrid);
      if (this._lastBluetoothStatus) this.updateBluetoothStatus(this._lastBluetoothStatus);
      this.renderKnownDeviceRegistry(
        this._lastHybrid?.network_devices || [],
        this._lastHybridOverrides || []
      );
    });
    await this.loadInitialData();
    this.startMonitoring();
  }

  // Cache DOM elements
  cacheElements() {
    // System stats
    const statsContainer = this.container.querySelector('.system-stats');
    if (statsContainer) {
      this.statsElements = {
        bodyRegions: statsContainer.querySelector('[data-stat="body-regions"] .stat-value'),
        samplingRate: statsContainer.querySelector('[data-stat="sampling-rate"] .stat-value'),
        accuracy: statsContainer.querySelector('[data-stat="accuracy"] .stat-value'),
        hardwareCost: statsContainer.querySelector('[data-stat="hardware-cost"] .stat-value')
      };
    }

    // Status indicators
    this.statusElements = {
      apiStatus: this.container.querySelector('.api-status'),
      streamStatus: this.container.querySelector('.stream-status'),
      hardwareStatus: this.container.querySelector('.hardware-status')
    };
  }

  // Load initial data
  async loadInitialData() {
    try {
      // Get API info
      const info = await healthService.getApiInfo();
      this.updateApiInfo(info);

      // Get current stats
      const stats = await poseService.getStats(1);
      this.updateStats(stats);
      await this.refreshHybridSummary();

    } catch (error) {
      // DensePose API may not be running (sensing-only mode) — fail silently
      console.log('Dashboard: DensePose API not available (sensing-only mode)');
    }
  }

  // Start monitoring
  startMonitoring() {
    // Subscribe to health updates
    this.healthSubscription = healthService.subscribeToHealth(health => {
      this.updateHealthStatus(health);
    });

    // Subscribe to sensing service state changes for data source indicator
    this._sensingUnsub = sensingService.onStateChange(() => {
      this.updateDataSourceIndicator();
    });
    // Also update on data — catches source changes mid-stream
    this._sensingDataUnsub = sensingService.onData(() => {
      this.updateDataSourceIndicator();
    });
    // Initial update
    this.updateDataSourceIndicator();

    // Start periodic stats updates
    this.statsInterval = setInterval(() => {
      this.updateLiveStats();
    }, 5000);

    // Start health monitoring
    healthService.startHealthMonitoring(30000);
  }

  // Update the data source indicator on the dashboard
  updateDataSourceIndicator() {
    const el = this.container.querySelector('#dashboard-datasource');
    if (!el) return;
    const ds = sensingService.dataSource;
    const statusText = el.querySelector('.status-text');
    const statusMsg  = el.querySelector('.status-message');
    const config = {
      'live':              { text: 'ESP32',                    status: 'healthy', msg: i18n.t('status.realHardwareConnected') },
      'server-simulated':  { text: i18n.t('common.simulated').toUpperCase(), status: 'warning', msg: i18n.t('status.serverRunningWithoutHardware') },
      'reconnecting':      { text: i18n.t('common.reconnecting').toUpperCase(), status: 'degraded', msg: i18n.t('status.attemptingConnect') },
      'unreachable':       { text: 'NO DATA',                 status: 'unhealthy', msg: i18n.t('status.serverUnreachableStale') },
      'simulated':         { text: 'INVENTED',                status: 'unhealthy', msg: i18n.t('status.browserGeneratedData') },
    };
    const cfg = config[ds] || config['reconnecting'];
    el.className = `component-status status-${cfg.status}`;
    if (statusText) statusText.textContent = cfg.text;
    if (statusMsg)  statusMsg.textContent = cfg.msg;
  }

  // Update API info display
  updateApiInfo(info) {
    // Update version
    const versionElement = this.container.querySelector('.api-version');
    if (versionElement && info.version) {
      versionElement.textContent = `v${info.version}`;
    }

    // Update environment
    const envElement = this.container.querySelector('.api-environment');
    if (envElement && info.environment) {
      envElement.textContent = info.environment;
      envElement.className = `api-environment env-${info.environment}`;
    }

    // Update features status
    if (info.features) {
      this.updateFeatures(info.features);
    }
  }

  // Update features display
  updateFeatures(features) {
    const featuresContainer = this.container.querySelector('.features-status');
    if (!featuresContainer) return;

    featuresContainer.innerHTML = '';
    
    Object.entries(features).forEach(([feature, enabled]) => {
      const featureElement = document.createElement('div');
      featureElement.className = `feature-item ${enabled ? 'enabled' : 'disabled'}`;
      
      // Use textContent instead of innerHTML to prevent XSS
      const featureNameSpan = document.createElement('span');
      featureNameSpan.className = 'feature-name';
      featureNameSpan.textContent = this.formatFeatureName(feature);
      
      const featureStatusSpan = document.createElement('span');
      featureStatusSpan.className = 'feature-status';
      featureStatusSpan.textContent = enabled ? '✓' : '✗';
      
      featureElement.appendChild(featureNameSpan);
      featureElement.appendChild(featureStatusSpan);
      featuresContainer.appendChild(featureElement);
    });
  }

  // Update health status
  updateHealthStatus(health) {
    if (!health) return;
    this._lastHealth = health;

    // Update overall status
    const overallStatus = this.container.querySelector('.overall-health');
    if (overallStatus) {
      overallStatus.className = `overall-health status-${health.status}`;
      overallStatus.textContent = i18n.t(`common.${health.status}`) || health.status.toUpperCase();
    }

    // Update component statuses
    if (health.components) {
      Object.entries(health.components).forEach(([component, status]) => {
        this.updateComponentStatus(component, status);
      });
    }

    // Update metrics
    if (health.metrics) {
      this.updateSystemMetrics(health.metrics);
    }
  }

  // Update component status
  updateComponentStatus(component, status) {
    // Map backend component names to UI component names
    const componentMap = {
      'pose': 'inference',
      'stream': 'streaming',
      'hardware': 'hardware'
    };
    
    const uiComponent = componentMap[component] || component;
    const element = this.container.querySelector(`[data-component="${uiComponent}"]`);
    
    if (element) {
      element.className = `component-status status-${status.status}`;
      const statusText = element.querySelector('.status-text');
      const statusMessage = element.querySelector('.status-message');
      
      if (statusText) {
        statusText.textContent = status.status.toUpperCase();
      }
      
      if (statusMessage && status.message) {
        statusMessage.textContent = status.message;
      }
    }
    
    // Also update API status based on overall health
    if (component === 'hardware') {
      const apiElement = this.container.querySelector(`[data-component="api"]`);
      if (apiElement) {
        apiElement.className = `component-status status-healthy`;
        const apiStatusText = apiElement.querySelector('.status-text');
        const apiStatusMessage = apiElement.querySelector('.status-message');
        
        if (apiStatusText) {
          apiStatusText.textContent = i18n.t('common.healthy').toUpperCase();
        }
        
        if (apiStatusMessage) {
          apiStatusMessage.textContent = i18n.t('status.apiRunningNormally');
        }
      }
    }
  }

  // Update system metrics
  updateSystemMetrics(metrics) {
    // Handle both flat and nested metric structures
    // Backend returns system_metrics.cpu.percent, mock returns metrics.cpu.percent
    const systemMetrics = metrics.system_metrics || metrics;
    const cpuPercent = systemMetrics.cpu?.percent || systemMetrics.cpu_percent;
    const memoryPercent = systemMetrics.memory?.percent || systemMetrics.memory_percent;
    const diskPercent = systemMetrics.disk?.percent || systemMetrics.disk_percent;

    // CPU usage
    const cpuElement = this.container.querySelector('.cpu-usage');
    if (cpuElement && cpuPercent !== undefined) {
      cpuElement.textContent = `${cpuPercent.toFixed(1)}%`;
      this.updateProgressBar('cpu', cpuPercent);
    }

    // Memory usage
    const memoryElement = this.container.querySelector('.memory-usage');
    if (memoryElement && memoryPercent !== undefined) {
      memoryElement.textContent = `${memoryPercent.toFixed(1)}%`;
      this.updateProgressBar('memory', memoryPercent);
    }

    // Disk usage
    const diskElement = this.container.querySelector('.disk-usage');
    if (diskElement && diskPercent !== undefined) {
      diskElement.textContent = `${diskPercent.toFixed(1)}%`;
      this.updateProgressBar('disk', diskPercent);
    }
  }

  // Update progress bar
  updateProgressBar(type, percent) {
    const progressBar = this.container.querySelector(`.progress-bar[data-type="${type}"]`);
    if (progressBar) {
      const fill = progressBar.querySelector('.progress-fill');
      if (fill) {
        fill.style.width = `${percent}%`;
        fill.className = `progress-fill ${this.getProgressClass(percent)}`;
      }
    }
  }

  // Get progress class based on percentage
  getProgressClass(percent) {
    if (percent >= 90) return 'critical';
    if (percent >= 75) return 'warning';
    return 'normal';
  }

  // Update live statistics
  async updateLiveStats() {
    try {
      // Get current pose data
      const currentPose = await poseService.getCurrentPose();
      this.updatePoseStats(currentPose);

      // Get zones summary
      const zonesSummary = await poseService.getZonesSummary();
      this.updateZonesDisplay(zonesSummary);
      await this.refreshHybridSummary();

    } catch (error) {
      console.error('Failed to update live stats:', error);
    }
  }

  // Update pose statistics
  updatePoseStats(poseData) {
    if (!poseData) return;

    // Update person count
    const personCount = this.container.querySelector('.person-count');
    if (personCount) {
      const count = poseData.persons ? poseData.persons.length : (poseData.total_persons || 0);
      personCount.textContent = count;
    }

    // Update average confidence
    const avgConfidence = this.container.querySelector('.avg-confidence');
    if (avgConfidence && poseData.persons && poseData.persons.length > 0) {
      const confidences = poseData.persons.map(p => p.confidence);
      const avg = confidences.length > 0
        ? (confidences.reduce((a, b) => a + b, 0) / confidences.length * 100).toFixed(1)
        : 0;
      avgConfidence.textContent = `${avg}%`;
    } else if (avgConfidence) {
      avgConfidence.textContent = '0%';
    }

    // Update total detections from stats if available
    const detectionCount = this.container.querySelector('.detection-count');
    if (detectionCount && poseData.total_detections !== undefined) {
      detectionCount.textContent = this.formatNumber(poseData.total_detections);
    }
  }

  // Update zones display
  updateZonesDisplay(zonesSummary) {
    const zonesContainer = this.container.querySelector('.zones-summary');
    if (!zonesContainer) return;

    zonesContainer.innerHTML = '';
    
    // Handle different zone summary formats
    let zones = {};
    if (zonesSummary && zonesSummary.zones) {
      zones = zonesSummary.zones;
    } else if (zonesSummary && typeof zonesSummary === 'object') {
      zones = zonesSummary;
    }
    
    // If no zones data, show default zones
    if (Object.keys(zones).length === 0) {
      ['zone_1', 'zone_2', 'zone_3', 'zone_4'].forEach(zoneId => {
        const zoneElement = document.createElement('div');
        zoneElement.className = 'zone-item';
        
        // Use textContent instead of innerHTML to prevent XSS
        const zoneNameSpan = document.createElement('span');
        zoneNameSpan.className = 'zone-name';
        zoneNameSpan.textContent = zoneId;
        
        const zoneCountSpan = document.createElement('span');
        zoneCountSpan.className = 'zone-count';
        zoneCountSpan.textContent = i18n.t('common.undefined');
        
        zoneElement.appendChild(zoneNameSpan);
        zoneElement.appendChild(zoneCountSpan);
        zonesContainer.appendChild(zoneElement);
      });
      return;
    }
    
    Object.entries(zones).forEach(([zoneId, data]) => {
      const zoneElement = document.createElement('div');
      zoneElement.className = 'zone-item';
      const count = typeof data === 'object' ? (data.person_count || data.count || 0) : data;
      
      // Use textContent instead of innerHTML to prevent XSS
      const zoneNameSpan = document.createElement('span');
      zoneNameSpan.className = 'zone-name';
      zoneNameSpan.textContent = zoneId;
      
      const zoneCountSpan = document.createElement('span');
      zoneCountSpan.className = 'zone-count';
      zoneCountSpan.textContent = String(count);
      
      zoneElement.appendChild(zoneNameSpan);
      zoneElement.appendChild(zoneCountSpan);
      zonesContainer.appendChild(zoneElement);
    });
  }

  // Update statistics
  updateStats(stats) {
    if (!stats) return;

    // Update detection count
    const detectionCount = this.container.querySelector('.detection-count');
    if (detectionCount && stats.total_detections !== undefined) {
      detectionCount.textContent = this.formatNumber(stats.total_detections);
    }

    // Update accuracy if available
    if (this.statsElements.accuracy && stats.average_confidence !== undefined) {
      this.statsElements.accuracy.textContent = `${(stats.average_confidence * 100).toFixed(1)}%`;
    }
  }

  async refreshHybridSummary() {
    try {
      const [hybrid, overrides, bluetooth] = await Promise.all([
        apiService.get('/api/v1/hybrid/latest'),
        apiService.get('/api/v1/hybrid/overrides').catch(() => ({ items: [] })),
        apiService.get('/api/v1/bluetooth/status').catch(() => null)
      ]);
      this._lastHybridOverrides = overrides?.items || [];
      this._lastBluetoothStatus = bluetooth;
      this.updateHybridSummary(hybrid);
      this.updateBluetoothStatus(bluetooth);
      this.renderKnownDeviceRegistry(hybrid?.network_devices || [], this._lastHybridOverrides);
    } catch (error) {
      console.log('Dashboard: hybrid snapshot not available yet');
    }
  }

  updateHybridSummary(hybrid) {
    if (!hybrid) return;
    this._lastHybrid = hybrid;

    const fusion = hybrid.fusion || {};
    const devices = hybrid.network_devices || [];

    const setText = (selector, value) => {
      const el = this.container.querySelector(selector);
      if (el) el.textContent = String(value);
    };

    setText('.hybrid-humans', fusion.estimated_humans || 0);
    setText('.hybrid-devices', devices.length);
    setText('.hybrid-smartphones', fusion.likely_smartphones || 0);
    setText('.hybrid-smarttvs', fusion.likely_smart_tvs || 0);

    const list = this.container.querySelector('.hybrid-device-list');
    if (!list) return;
    list.innerHTML = '';

    if (devices.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'hybrid-device-item';
      empty.textContent = i18n.t('hybrid.empty');
      list.appendChild(empty);
      return;
    }

    devices.slice(0, 12).forEach(device => {
      const item = document.createElement('div');
      item.className = 'hybrid-device-item';

      const name = document.createElement('div');
      name.className = 'hybrid-device-name';
      name.textContent = device.display_name || device.hostname || device.ip;

      const category = document.createElement('div');
      category.className = 'hybrid-device-meta';
      const badge = device.manual_override ? i18n.t('hybrid.manualBadge') : i18n.t('hybrid.autoBadge');
      category.textContent = `${i18n.t(`hybrid.category.${device.category}`)} • ${Math.round((device.confidence || 0) * 100)}% • ${badge}`;

      const meta = document.createElement('div');
      meta.className = 'hybrid-device-meta';
      meta.textContent = [device.vendor, device.ip, device.mac].filter(Boolean).join(' • ');

      item.appendChild(name);
      item.appendChild(category);
      item.appendChild(meta);
      list.appendChild(item);
    });
  }

  updateBluetoothStatus(status) {
    if (!status) return;

    const availability = this.container.querySelector('.hybrid-bluetooth-availability');
    const mode = this.container.querySelector('.hybrid-bluetooth-mode');
    const adapter = this.container.querySelector('.hybrid-bluetooth-adapter');
    const adapterName = this.container.querySelector('.hybrid-bluetooth-adapter-name');
    const service = this.container.querySelector('.hybrid-bluetooth-service');
    const serviceNote = this.container.querySelector('.hybrid-bluetooth-service-note');
    const note = this.container.querySelector('.hybrid-bluetooth-note');
    const toggle = this.container.querySelector('.hybrid-bluetooth-toggle');

    if (availability) availability.textContent = status.enabled
      ? (status.effective ? i18n.t('common.active') : i18n.t('common.warning'))
      : i18n.t('common.inactive');
    if (mode) mode.textContent = status.helper_mode || 'experimental';
    if (adapter) adapter.textContent = status.adapter_present ? i18n.t('common.connected') : i18n.t('common.disconnected');
    if (adapterName) adapterName.textContent = status.adapter_name || i18n.t('common.unknown');
    if (service) service.textContent = status.service_running ? i18n.t('common.active') : i18n.t('common.inactive');
    if (serviceNote) serviceNote.textContent = status.service_running
      ? i18n.t('hybrid.bluetoothServiceRunning')
      : i18n.t('hybrid.bluetoothServiceStopped');
    if (note) note.textContent = status.note || i18n.t('hybrid.bluetoothNote');
    if (toggle) toggle.textContent = status.enabled
      ? i18n.t('hybrid.bluetoothDisable')
      : i18n.t('hybrid.bluetoothEnable');
  }

  wireBluetoothToggle() {
    const button = this.container.querySelector('.hybrid-bluetooth-toggle');
    if (!button) return;

    button.addEventListener('click', async () => {
      const nextEnabled = !this._lastBluetoothStatus?.enabled;
      button.disabled = true;
      try {
        const bluetooth = await apiService.post('/api/v1/bluetooth/enabled', {
          enabled: nextEnabled
        });
        this._lastBluetoothStatus = bluetooth;
        this.updateBluetoothStatus(bluetooth);
      } catch (error) {
        this.showError(error.message || i18n.t('common.error'));
      } finally {
        button.disabled = false;
      }
    });
  }

  wireManualForm() {
    const form = this.container.querySelector('.hybrid-manual-form');
    if (!form) return;

    form.addEventListener('submit', async (event) => {
      event.preventDefault();
      const formData = new FormData(form);
      const payload = {
        display_name: String(formData.get('display_name') || '').trim() || null,
        category: String(formData.get('category') || 'unknown_device'),
        match_mac: String(formData.get('match_mac') || '').trim() || null,
        match_ip: String(formData.get('match_ip') || '').trim() || null,
        match_hostname: String(formData.get('match_hostname') || '').trim() || null,
        notes: String(formData.get('notes') || '').trim() || null
      };

      if (!payload.display_name && !payload.match_mac && !payload.match_ip && !payload.match_hostname) {
        this.showError(i18n.t('hybrid.form.validation'));
        return;
      }

      try {
        await apiService.post('/api/v1/hybrid/overrides/upsert', payload);
        form.reset();
        await this.refreshHybridSummary();
      } catch (error) {
        this.showError(error.message || i18n.t('common.error'));
      }
    });
  }

  renderKnownDeviceRegistry(devices, overrides) {
    const container = this.container.querySelector('.hybrid-known-device-list');
    if (!container) return;
    container.innerHTML = '';

    const overrideMap = new Map((overrides || []).map(entry => [entry.id, entry]));

    if (!devices.length && !overrideMap.size) {
      const empty = document.createElement('div');
      empty.className = 'hybrid-known-device-card';
      empty.textContent = i18n.t('hybrid.registryEmpty');
      container.appendChild(empty);
      return;
    }

    const matchedOverrideIds = new Set();
    devices.forEach(device => {
      if (device.matched_override_id) matchedOverrideIds.add(device.matched_override_id);
      container.appendChild(this.buildKnownDeviceCard(device, overrideMap.get(device.matched_override_id)));
    });

    overrides
      .filter(entry => !matchedOverrideIds.has(entry.id))
      .forEach(entry => {
        container.appendChild(this.buildKnownDeviceCard(null, entry));
      });
  }

  buildKnownDeviceCard(device, overrideEntry) {
    const card = document.createElement('div');
    card.className = 'hybrid-known-device-card';

    const title = document.createElement('div');
    title.className = 'hybrid-device-name';
    title.textContent = overrideEntry?.display_name || device?.display_name || device?.hostname || device?.ip || i18n.t('hybrid.manualEntry');

    const identity = document.createElement('div');
    identity.className = 'hybrid-device-meta';
    identity.textContent = [
      device?.mac || overrideEntry?.match_mac,
      device?.ip || overrideEntry?.match_ip,
      device?.hostname || overrideEntry?.match_hostname
    ].filter(Boolean).join(' • ') || i18n.t('hybrid.noIdentity');

    const controls = document.createElement('div');
    controls.className = 'hybrid-known-device-controls';

    const nameInput = document.createElement('input');
    nameInput.className = 'hybrid-manual-input';
    nameInput.value = overrideEntry?.display_name || device?.display_name || device?.hostname || '';
    nameInput.placeholder = i18n.t('hybrid.form.namePlaceholder');

    const categorySelect = document.createElement('select');
    categorySelect.className = 'hybrid-manual-select';
    ['smartphone', 'smart_tv', 'computer', 'iot_or_media', 'unknown_device'].forEach(category => {
      const option = document.createElement('option');
      option.value = category;
      option.textContent = i18n.t(`hybrid.category.${category}`);
      option.selected = (overrideEntry?.category || device?.category || 'unknown_device') === category;
      categorySelect.appendChild(option);
    });

    const notesInput = document.createElement('input');
    notesInput.className = 'hybrid-manual-input';
    notesInput.value = overrideEntry?.notes || '';
    notesInput.placeholder = i18n.t('hybrid.form.notesPlaceholder');

    const actions = document.createElement('div');
    actions.className = 'hybrid-known-device-actions';

    const saveButton = document.createElement('button');
    saveButton.type = 'button';
    saveButton.className = 'hybrid-inline-btn hybrid-inline-btn--primary';
    saveButton.textContent = i18n.t('hybrid.save');
    saveButton.addEventListener('click', async () => {
      const payload = {
        id: overrideEntry?.id || device?.matched_override_id || '',
        display_name: nameInput.value.trim() || null,
        category: categorySelect.value,
        match_mac: device?.mac || overrideEntry?.match_mac || null,
        match_ip: device?.ip || overrideEntry?.match_ip || null,
        match_hostname: device?.hostname || overrideEntry?.match_hostname || null,
        notes: notesInput.value.trim() || null
      };

      try {
        await apiService.post('/api/v1/hybrid/overrides/upsert', payload);
        await this.refreshHybridSummary();
      } catch (error) {
        this.showError(error.message || i18n.t('common.error'));
      }
    });

    const deleteButton = document.createElement('button');
    deleteButton.type = 'button';
    deleteButton.className = 'hybrid-inline-btn';
    deleteButton.textContent = i18n.t('hybrid.delete');
    deleteButton.disabled = !(overrideEntry?.id || device?.matched_override_id);
    deleteButton.addEventListener('click', async () => {
      const id = overrideEntry?.id || device?.matched_override_id;
      if (!id) return;
      try {
        await apiService.delete('/api/v1/hybrid/overrides/delete', {
          body: JSON.stringify({ id })
        });
        await this.refreshHybridSummary();
      } catch (error) {
        this.showError(error.message || i18n.t('common.error'));
      }
    });

    controls.appendChild(nameInput);
    controls.appendChild(categorySelect);
    controls.appendChild(notesInput);
    actions.appendChild(saveButton);
    actions.appendChild(deleteButton);

    card.appendChild(title);
    card.appendChild(identity);
    card.appendChild(controls);
    card.appendChild(actions);
    return card;
  }

  // Format feature name
  formatFeatureName(name) {
    return name.replace(/_/g, ' ')
      .split(' ')
      .map(word => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
  }

  // Format large numbers
  formatNumber(num) {
    if (num >= 1000000) {
      return `${(num / 1000000).toFixed(1)}M`;
    }
    if (num >= 1000) {
      return `${(num / 1000).toFixed(1)}K`;
    }
    return num.toString();
  }

  // Show error message
  showError(message) {
    const errorContainer = this.container.querySelector('.error-container');
    if (errorContainer) {
      errorContainer.textContent = message;
      errorContainer.style.display = 'block';
      
      setTimeout(() => {
        errorContainer.style.display = 'none';
      }, 5000);
    }
  }

  // Clean up
  dispose() {
    if (this.healthSubscription) {
      this.healthSubscription();
    }
    if (this._sensingUnsub) this._sensingUnsub();
    if (this._sensingDataUnsub) this._sensingDataUnsub();
    if (this._localeUnsub) this._localeUnsub();

    if (this.statsInterval) {
      clearInterval(this.statsInterval);
    }

    healthService.stopHealthMonitoring();
  }
}
