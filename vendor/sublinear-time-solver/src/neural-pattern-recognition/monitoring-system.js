/**
 * Comprehensive Monitoring and Alerting System
 * For Entity Communication Detection Infrastructure
 *
 * Provides real-time monitoring, alerting, and system health tracking
 * for all neural pattern recognition components.
 */

const EventEmitter = require('events');
const fs = require('fs').promises;
const path = require('path');

/**
 * Central monitoring hub for the entire entity communication detection system
 */
class EntityCommunicationMonitor extends EventEmitter {
    constructor(config = {}) {
        super();

        this.config = {
            alertThresholds: {
                detectionAccuracy: 0.85,
                responseTime: 1000, // ms
                memoryUsage: 0.8, // 80%
                cpuUsage: 0.9, // 90%
                errorRate: 0.05 // 5%
            },
            monitoringInterval: 1000, // 1 second
            alertCooldown: 30000, // 30 seconds
            metricsRetention: 86400000, // 24 hours
            ...config
        };

        this.metrics = new Map();
        this.alerts = new Map();
        this.systemHealth = {
            overall: 'healthy',
            components: new Map(),
            lastUpdate: Date.now()
        };

        this.alertCooldowns = new Map();
        this.isMonitoring = false;

        this.initializeMetrics();
    }

    /**
     * Initialize monitoring metrics structure
     */
    initializeMetrics() {
        const metricCategories = [
            'detection_accuracy',
            'response_times',
            'entity_communications',
            'pattern_analysis',
            'system_performance',
            'error_tracking'
        ];

        metricCategories.forEach(category => {
            this.metrics.set(category, {
                current: 0,
                history: [],
                trends: [],
                anomalies: []
            });
        });
    }

    /**
     * Start monitoring all system components
     */
    async startMonitoring() {
        if (this.isMonitoring) {
            console.log('Monitoring already active');
            return;
        }

        this.isMonitoring = true;
        console.log('🔍 Starting comprehensive entity communication monitoring...');

        // Start monitoring intervals
        this.monitoringInterval = setInterval(() => {
            this.collectMetrics();
        }, this.config.monitoringInterval);

        this.healthCheckInterval = setInterval(() => {
            this.performHealthCheck();
        }, this.config.monitoringInterval * 5);

        this.anomalyDetectionInterval = setInterval(() => {
            this.detectAnomalies();
        }, this.config.monitoringInterval * 10);

        this.emit('monitoring_started', {
            timestamp: Date.now(),
            config: this.config
        });
    }

    /**
     * Stop monitoring
     */
    stopMonitoring() {
        if (!this.isMonitoring) return;

        this.isMonitoring = false;
        clearInterval(this.monitoringInterval);
        clearInterval(this.healthCheckInterval);
        clearInterval(this.anomalyDetectionInterval);

        console.log('🛑 Monitoring stopped');
        this.emit('monitoring_stopped', { timestamp: Date.now() });
    }

    /**
     * Collect metrics from all system components
     */
    async collectMetrics() {
        try {
            const timestamp = Date.now();

            // Collect detection accuracy metrics
            await this.collectDetectionMetrics(timestamp);

            // Collect performance metrics
            await this.collectPerformanceMetrics(timestamp);

            // Collect entity communication metrics
            await this.collectCommunicationMetrics(timestamp);

            // Collect system resource metrics
            await this.collectResourceMetrics(timestamp);

            // Update health status
            this.updateSystemHealth();

        } catch (error) {
            console.error('Error collecting metrics:', error);
            this.recordError('metric_collection', error);
        }
    }

    /**
     * Collect detection accuracy metrics
     */
    async collectDetectionMetrics(timestamp) {
        const detectionMetrics = this.metrics.get('detection_accuracy');

        // Simulate detection accuracy calculation
        const accuracy = this.calculateDetectionAccuracy();

        detectionMetrics.current = accuracy;
        detectionMetrics.history.push({
            timestamp,
            value: accuracy,
            components: {
                zeroVariance: Math.random() * 0.1 + 0.9,
                maxEntropy: Math.random() * 0.1 + 0.85,
                instructionSequence: Math.random() * 0.15 + 0.8,
                realTimeDetection: Math.random() * 0.1 + 0.88
            }
        });

        // Check threshold
        if (accuracy < this.config.alertThresholds.detectionAccuracy) {
            this.triggerAlert('low_detection_accuracy', {
                current: accuracy,
                threshold: this.config.alertThresholds.detectionAccuracy,
                timestamp
            });
        }

        // Maintain history size
        this.maintainHistorySize(detectionMetrics, 1000);
    }

    /**
     * Collect performance metrics
     */
    async collectPerformanceMetrics(timestamp) {
        const responseMetrics = this.metrics.get('response_times');

        // Simulate response time measurement
        const responseTime = this.measureResponseTime();

        responseMetrics.current = responseTime;
        responseMetrics.history.push({
            timestamp,
            value: responseTime,
            breakdown: {
                zeroVarianceDetection: Math.random() * 50 + 10,
                entropyDecoding: Math.random() * 100 + 20,
                instructionAnalysis: Math.random() * 200 + 50,
                correlation: Math.random() * 75 + 15
            }
        });

        // Check threshold
        if (responseTime > this.config.alertThresholds.responseTime) {
            this.triggerAlert('high_response_time', {
                current: responseTime,
                threshold: this.config.alertThresholds.responseTime,
                timestamp
            });
        }

        this.maintainHistorySize(responseMetrics, 1000);
    }

    /**
     * Collect entity communication metrics
     */
    async collectCommunicationMetrics(timestamp) {
        const commMetrics = this.metrics.get('entity_communications');

        const communicationData = {
            detectedCommunications: Math.floor(Math.random() * 10),
            entityTypes: ['mathematical', 'quantum', 'steganographic'],
            confidenceScores: Array.from({length: 5}, () => Math.random()),
            patternTypes: {
                zeroVariance: Math.floor(Math.random() * 3),
                maxEntropy: Math.floor(Math.random() * 4),
                impossibleSequences: Math.floor(Math.random() * 2)
            }
        };

        commMetrics.current = communicationData.detectedCommunications;
        commMetrics.history.push({
            timestamp,
            ...communicationData
        });

        this.maintainHistorySize(commMetrics, 1000);
    }

    /**
     * Collect system resource metrics
     */
    async collectResourceMetrics(timestamp) {
        const perfMetrics = this.metrics.get('system_performance');

        // Simulate resource usage
        const resourceData = {
            memoryUsage: Math.random() * 0.3 + 0.4, // 40-70%
            cpuUsage: Math.random() * 0.4 + 0.2, // 20-60%
            diskUsage: Math.random() * 0.2 + 0.1, // 10-30%
            networkThroughput: Math.random() * 1000 + 500 // MB/s
        };

        perfMetrics.current = resourceData;
        perfMetrics.history.push({
            timestamp,
            ...resourceData
        });

        // Check thresholds
        if (resourceData.memoryUsage > this.config.alertThresholds.memoryUsage) {
            this.triggerAlert('high_memory_usage', {
                current: resourceData.memoryUsage,
                threshold: this.config.alertThresholds.memoryUsage,
                timestamp
            });
        }

        if (resourceData.cpuUsage > this.config.alertThresholds.cpuUsage) {
            this.triggerAlert('high_cpu_usage', {
                current: resourceData.cpuUsage,
                threshold: this.config.alertThresholds.cpuUsage,
                timestamp
            });
        }

        this.maintainHistorySize(perfMetrics, 1000);
    }

    /**
     * Perform comprehensive health check
     */
    async performHealthCheck() {
        const timestamp = Date.now();
        const healthResults = {};

        // Check each component
        const components = [
            'zero_variance_detector',
            'entropy_decoder',
            'instruction_analyzer',
            'real_time_detector',
            'pattern_learning_network'
        ];

        for (const component of components) {
            healthResults[component] = await this.checkComponentHealth(component);
        }

        // Determine overall health
        const healthyComponents = Object.values(healthResults)
            .filter(status => status === 'healthy').length;
        const totalComponents = Object.keys(healthResults).length;

        let overallHealth = 'healthy';
        if (healthyComponents < totalComponents * 0.8) {
            overallHealth = 'degraded';
        }
        if (healthyComponents < totalComponents * 0.6) {
            overallHealth = 'critical';
        }

        this.systemHealth = {
            overall: overallHealth,
            components: new Map(Object.entries(healthResults)),
            lastUpdate: timestamp,
            score: healthyComponents / totalComponents
        };

        this.emit('health_check_complete', this.systemHealth);

        if (overallHealth !== 'healthy') {
            this.triggerAlert('system_health_degraded', {
                health: overallHealth,
                components: healthResults,
                timestamp
            });
        }
    }

    /**
     * Check individual component health
     */
    async checkComponentHealth(component) {
        try {
            // Simulate component health check
            const metrics = {
                responseTime: Math.random() * 100 + 10,
                errorRate: Math.random() * 0.02,
                memoryUsage: Math.random() * 0.3 + 0.2,
                lastActivity: Date.now() - Math.random() * 30000
            };

            // Health determination logic
            if (metrics.errorRate > 0.01 ||
                metrics.responseTime > 500 ||
                metrics.memoryUsage > 0.8) {
                return 'degraded';
            }

            if (Date.now() - metrics.lastActivity > 60000) {
                return 'inactive';
            }

            return 'healthy';

        } catch (error) {
            console.error(`Health check failed for ${component}:`, error);
            return 'error';
        }
    }

    /**
     * Detect anomalies in metrics
     */
    detectAnomalies() {
        for (const [category, data] of this.metrics) {
            try {
                const anomalies = this.analyzeMetricAnomalies(category, data);
                if (anomalies.length > 0) {
                    data.anomalies.push(...anomalies);

                    this.triggerAlert('anomaly_detected', {
                        category,
                        anomalies,
                        timestamp: Date.now()
                    });
                }
            } catch (error) {
                console.error(`Anomaly detection failed for ${category}:`, error);
            }
        }
    }

    /**
     * Analyze metric anomalies using statistical methods
     */
    analyzeMetricAnomalies(category, data) {
        if (data.history.length < 10) return [];

        const recent = data.history.slice(-10);
        const values = recent.map(item =>
            typeof item.value === 'number' ? item.value : item.detectedCommunications || 0
        );

        const mean = values.reduce((a, b) => a + b, 0) / values.length;
        const variance = values.reduce((acc, val) => acc + Math.pow(val - mean, 2), 0) / values.length;
        const stdDev = Math.sqrt(variance);

        const anomalies = [];
        const threshold = 2.5; // Z-score threshold

        recent.forEach((item, index) => {
            const value = typeof item.value === 'number' ? item.value : item.detectedCommunications || 0;
            const zScore = Math.abs((value - mean) / (stdDev || 1));

            if (zScore > threshold) {
                anomalies.push({
                    timestamp: item.timestamp,
                    value,
                    zScore,
                    type: zScore > 3 ? 'severe' : 'moderate'
                });
            }
        });

        return anomalies;
    }

    /**
     * Trigger an alert with cooldown protection
     */
    triggerAlert(alertType, data) {
        const now = Date.now();
        const cooldownKey = alertType;

        // Check cooldown
        if (this.alertCooldowns.has(cooldownKey) &&
            now - this.alertCooldowns.get(cooldownKey) < this.config.alertCooldown) {
            return;
        }

        this.alertCooldowns.set(cooldownKey, now);

        const alert = {
            id: this.generateAlertId(),
            type: alertType,
            severity: this.determineAlertSeverity(alertType, data),
            timestamp: now,
            data,
            resolved: false
        };

        this.alerts.set(alert.id, alert);

        console.warn(`🚨 ALERT [${alert.severity}]: ${alertType}`, data);
        this.emit('alert_triggered', alert);

        // Auto-resolve certain alerts after time
        if (['high_response_time', 'high_memory_usage'].includes(alertType)) {
            setTimeout(() => {
                this.resolveAlert(alert.id);
            }, this.config.alertCooldown);
        }
    }

    /**
     * Determine alert severity
     */
    determineAlertSeverity(alertType, data) {
        const severityMap = {
            'low_detection_accuracy': 'critical',
            'system_health_degraded': 'high',
            'anomaly_detected': 'medium',
            'high_response_time': 'medium',
            'high_memory_usage': 'low',
            'high_cpu_usage': 'medium'
        };

        return severityMap[alertType] || 'low';
    }

    /**
     * Resolve an alert
     */
    resolveAlert(alertId) {
        const alert = this.alerts.get(alertId);
        if (alert) {
            alert.resolved = true;
            alert.resolvedAt = Date.now();
            this.emit('alert_resolved', alert);
        }
    }

    /**
     * Calculate overall detection accuracy
     */
    calculateDetectionAccuracy() {
        // Simulate weighted accuracy calculation
        const components = {
            zeroVariance: { accuracy: Math.random() * 0.1 + 0.9, weight: 0.3 },
            maxEntropy: { accuracy: Math.random() * 0.1 + 0.85, weight: 0.25 },
            instructionSequence: { accuracy: Math.random() * 0.15 + 0.8, weight: 0.25 },
            realTime: { accuracy: Math.random() * 0.1 + 0.88, weight: 0.2 }
        };

        let weightedSum = 0;
        let totalWeight = 0;

        for (const [component, data] of Object.entries(components)) {
            weightedSum += data.accuracy * data.weight;
            totalWeight += data.weight;
        }

        return weightedSum / totalWeight;
    }

    /**
     * Measure system response time
     */
    measureResponseTime() {
        // Simulate response time with realistic variation
        const baseTime = 150; // Base response time in ms
        const variation = Math.random() * 200; // Random variation
        const loadFactor = Math.random() * 0.5 + 0.5; // System load factor

        return Math.round(baseTime + variation * loadFactor);
    }

    /**
     * Update system health status
     */
    updateSystemHealth() {
        const accuracy = this.metrics.get('detection_accuracy').current;
        const responseTime = this.metrics.get('response_times').current;
        const resources = this.metrics.get('system_performance').current;

        let healthScore = 1.0;

        // Factor in detection accuracy
        if (accuracy < this.config.alertThresholds.detectionAccuracy) {
            healthScore *= 0.7;
        }

        // Factor in response time
        if (responseTime > this.config.alertThresholds.responseTime) {
            healthScore *= 0.8;
        }

        // Factor in resource usage
        if (resources && resources.memoryUsage > this.config.alertThresholds.memoryUsage) {
            healthScore *= 0.9;
        }

        // Determine overall status
        let overallStatus = 'healthy';
        if (healthScore < 0.8) overallStatus = 'degraded';
        if (healthScore < 0.6) overallStatus = 'critical';

        this.systemHealth.overall = overallStatus;
        this.systemHealth.score = healthScore;
        this.systemHealth.lastUpdate = Date.now();
    }

    /**
     * Record system errors
     */
    recordError(source, error) {
        const errorMetrics = this.metrics.get('error_tracking');

        const errorData = {
            timestamp: Date.now(),
            source,
            message: error.message,
            stack: error.stack,
            severity: this.classifyErrorSeverity(error)
        };

        errorMetrics.history.push(errorData);

        // Calculate error rate
        const recentErrors = errorMetrics.history.filter(
            err => Date.now() - err.timestamp < 300000 // Last 5 minutes
        );
        const errorRate = recentErrors.length / 300; // Errors per second

        if (errorRate > this.config.alertThresholds.errorRate) {
            this.triggerAlert('high_error_rate', {
                rate: errorRate,
                threshold: this.config.alertThresholds.errorRate,
                recentErrors: recentErrors.slice(-5)
            });
        }

        this.maintainHistorySize(errorMetrics, 1000);
    }

    /**
     * Classify error severity
     */
    classifyErrorSeverity(error) {
        const criticalPatterns = [
            /out of memory/i,
            /segmentation fault/i,
            /neural.*crash/i
        ];

        const highPatterns = [
            /detection.*fail/i,
            /connection.*lost/i,
            /timeout/i
        ];

        const message = error.message.toLowerCase();

        if (criticalPatterns.some(pattern => pattern.test(message))) {
            return 'critical';
        }
        if (highPatterns.some(pattern => pattern.test(message))) {
            return 'high';
        }

        return 'medium';
    }

    /**
     * Maintain metric history size
     */
    maintainHistorySize(metrics, maxSize) {
        if (metrics.history.length > maxSize) {
            metrics.history = metrics.history.slice(-maxSize);
        }
        if (metrics.anomalies && metrics.anomalies.length > maxSize / 10) {
            metrics.anomalies = metrics.anomalies.slice(-maxSize / 10);
        }
    }

    /**
     * Generate unique alert ID
     */
    generateAlertId() {
        return `alert_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    }

    /**
     * Get current system status
     */
    getSystemStatus() {
        return {
            health: this.systemHealth,
            metrics: Object.fromEntries(
                Array.from(this.metrics.entries()).map(([key, value]) => [
                    key,
                    {
                        current: value.current,
                        historyLength: value.history.length,
                        anomaliesCount: value.anomalies ? value.anomalies.length : 0
                    }
                ])
            ),
            alerts: {
                active: Array.from(this.alerts.values()).filter(alert => !alert.resolved),
                total: this.alerts.size
            },
            monitoring: this.isMonitoring
        };
    }

    /**
     * Export metrics for analysis
     */
    async exportMetrics(filePath = null) {
        const exportData = {
            timestamp: Date.now(),
            systemHealth: this.systemHealth,
            metrics: Object.fromEntries(this.metrics),
            alerts: Object.fromEntries(this.alerts),
            configuration: this.config
        };

        if (filePath) {
            await fs.writeFile(filePath, JSON.stringify(exportData, null, 2));
            console.log(`📊 Metrics exported to ${filePath}`);
        }

        return exportData;
    }
}

/**
 * Real-time dashboard for monitoring entity communication detection
 */
class RealTimeDashboard extends EventEmitter {
    constructor(monitor) {
        super();
        this.monitor = monitor;
        this.display = {
            width: 120,
            height: 30,
            refreshRate: 1000
        };

        this.charts = new Map();
        this.isDisplaying = false;

        this.setupEventListeners();
    }

    /**
     * Setup event listeners for monitor updates
     */
    setupEventListeners() {
        this.monitor.on('alert_triggered', (alert) => {
            this.displayAlert(alert);
        });

        this.monitor.on('health_check_complete', (health) => {
            this.updateHealthDisplay(health);
        });
    }

    /**
     * Start real-time dashboard display
     */
    startDashboard() {
        if (this.isDisplaying) return;

        this.isDisplaying = true;
        console.log('🖥️  Starting real-time entity communication dashboard...');

        this.displayInterval = setInterval(() => {
            this.refreshDisplay();
        }, this.display.refreshRate);

        // Initial display
        this.refreshDisplay();
    }

    /**
     * Stop dashboard display
     */
    stopDashboard() {
        if (!this.isDisplaying) return;

        this.isDisplaying = false;
        clearInterval(this.displayInterval);
        console.log('🛑 Dashboard stopped');
    }

    /**
     * Refresh the entire dashboard display
     */
    refreshDisplay() {
        const status = this.monitor.getSystemStatus();

        console.clear();
        console.log(this.generateDashboard(status));
    }

    /**
     * Generate formatted dashboard content
     */
    generateDashboard(status) {
        const lines = [];
        const width = this.display.width;

        // Header
        lines.push('═'.repeat(width));
        lines.push(`🛸 ENTITY COMMUNICATION DETECTION SYSTEM - ${new Date().toLocaleTimeString()}`);
        lines.push('═'.repeat(width));

        // System Health
        const healthIcon = this.getHealthIcon(status.health.overall);
        lines.push(`${healthIcon} System Health: ${status.health.overall.toUpperCase()} (Score: ${(status.health.score || 0).toFixed(2)})`);
        lines.push('─'.repeat(width));

        // Key Metrics
        lines.push('📊 KEY METRICS:');

        if (status.metrics.detection_accuracy) {
            const accuracy = (status.metrics.detection_accuracy.current * 100).toFixed(1);
            lines.push(`   🎯 Detection Accuracy: ${accuracy}%`);
        }

        if (status.metrics.response_times) {
            const responseTime = status.metrics.response_times.current;
            lines.push(`   ⚡ Response Time: ${responseTime}ms`);
        }

        if (status.metrics.entity_communications) {
            const comms = status.metrics.entity_communications.current;
            lines.push(`   📡 Active Communications: ${comms}`);
        }

        lines.push('─'.repeat(width));

        // Component Status
        lines.push('🔧 COMPONENT STATUS:');
        if (status.health.components) {
            for (const [component, health] of status.health.components) {
                const icon = this.getHealthIcon(health);
                lines.push(`   ${icon} ${component.replace(/_/g, ' ').toUpperCase()}: ${health}`);
            }
        }

        lines.push('─'.repeat(width));

        // Active Alerts
        lines.push(`🚨 ACTIVE ALERTS: ${status.alerts.active.length}`);
        if (status.alerts.active.length > 0) {
            status.alerts.active.slice(0, 5).forEach(alert => {
                const severityIcon = this.getSeverityIcon(alert.severity);
                lines.push(`   ${severityIcon} ${alert.type}: ${alert.severity}`);
            });
        } else {
            lines.push('   ✅ No active alerts');
        }

        lines.push('─'.repeat(width));

        // System Resources
        if (status.metrics.system_performance) {
            const perf = status.metrics.system_performance.current;
            if (perf && typeof perf === 'object') {
                lines.push('💻 SYSTEM RESOURCES:');
                lines.push(`   Memory: ${this.createProgressBar((perf.memoryUsage || 0) * 100, 50)}${((perf.memoryUsage || 0) * 100).toFixed(1)}%`);
                lines.push(`   CPU: ${this.createProgressBar((perf.cpuUsage || 0) * 100, 50)}${((perf.cpuUsage || 0) * 100).toFixed(1)}%`);
            }
        }

        lines.push('═'.repeat(width));

        return lines.join('\n');
    }

    /**
     * Get health status icon
     */
    getHealthIcon(health) {
        const icons = {
            'healthy': '🟢',
            'degraded': '🟡',
            'critical': '🔴',
            'error': '❌',
            'inactive': '⚫'
        };
        return icons[health] || '❓';
    }

    /**
     * Get severity icon
     */
    getSeverityIcon(severity) {
        const icons = {
            'critical': '🔴',
            'high': '🟠',
            'medium': '🟡',
            'low': '🟢'
        };
        return icons[severity] || '📋';
    }

    /**
     * Create ASCII progress bar
     */
    createProgressBar(percentage, width = 20) {
        const filled = Math.round(percentage / 100 * width);
        const empty = width - filled;
        return '[' + '█'.repeat(filled) + '░'.repeat(empty) + '] ';
    }

    /**
     * Display alert notification
     */
    displayAlert(alert) {
        const icon = this.getSeverityIcon(alert.severity);
        console.log(`\n${icon} ALERT: ${alert.type} (${alert.severity})`);
        console.log(`Time: ${new Date(alert.timestamp).toLocaleTimeString()}`);
        if (alert.data) {
            console.log(`Details: ${JSON.stringify(alert.data, null, 2)}`);
        }
        console.log('─'.repeat(60));
    }

    /**
     * Update health display
     */
    updateHealthDisplay(health) {
        if (this.isDisplaying) {
            // Will be included in next refresh
            return;
        }

        console.log(`🏥 Health Update: ${health.overall} (Score: ${health.score.toFixed(2)})`);
    }
}

module.exports = {
    EntityCommunicationMonitor,
    RealTimeDashboard
};