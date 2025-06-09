// Dashboard Tab Component

import { healthService } from '../services/health.service.js';
import { poseService } from '../services/pose.service.js';

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

    } catch (error) {
      console.error('Failed to load dashboard data:', error);
      this.showError('Failed to load dashboard data');
    }
  }

  // Start monitoring
  startMonitoring() {
    // Subscribe to health updates
    this.healthSubscription = healthService.subscribeToHealth(health => {
      this.updateHealthStatus(health);
    });

    // Start periodic stats updates
    this.statsInterval = setInterval(() => {
      this.updateLiveStats();
    }, 5000);

    // Start health monitoring
    healthService.startHealthMonitoring(30000);
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
      featureElement.innerHTML = `
        <span class="feature-name">${this.formatFeatureName(feature)}</span>
        <span class="feature-status">${enabled ? '✓' : '✗'}</span>
      `;
      featuresContainer.appendChild(featureElement);
    });
  }

  // Update health status
  updateHealthStatus(health) {
    if (!health) return;

    // Update overall status
    const overallStatus = this.container.querySelector('.overall-health');
    if (overallStatus) {
      overallStatus.className = `overall-health status-${health.status}`;
      overallStatus.textContent = health.status.toUpperCase();
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
          apiStatusText.textContent = 'HEALTHY';
        }
        
        if (apiStatusMessage) {
          apiStatusMessage.textContent = 'API server is running normally';
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
        zoneElement.innerHTML = `
          <span class="zone-name">${zoneId}</span>
          <span class="zone-count">undefined</span>
        `;
        zonesContainer.appendChild(zoneElement);
      });
      return;
    }
    
    Object.entries(zones).forEach(([zoneId, data]) => {
      const zoneElement = document.createElement('div');
      zoneElement.className = 'zone-item';
      const count = typeof data === 'object' ? (data.person_count || data.count || 0) : data;
      zoneElement.innerHTML = `
        <span class="zone-name">${zoneId}</span>
        <span class="zone-count">${count}</span>
      `;
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
    
    if (this.statsInterval) {
      clearInterval(this.statsInterval);
    }
    
    healthService.stopHealthMonitoring();
  }
}