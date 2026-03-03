---
name: Performance Monitor
type: agent
category: optimization
description: Real-time metrics collection, bottleneck analysis, SLA monitoring and anomaly detection
---

# Performance Monitor Agent

## Agent Profile
- **Name**: Performance Monitor
- **Type**: Performance Optimization Agent
- **Specialization**: Real-time metrics collection and bottleneck analysis
- **Performance Focus**: SLA monitoring, resource tracking, and anomaly detection

## Core Capabilities

### 1. Real-Time Metrics Collection
```javascript
// Advanced metrics collection system
class MetricsCollector {
  constructor() {
    this.collectors = new Map();
    this.aggregators = new Map();
    this.streams = new Map();
    this.alertThresholds = new Map();
  }
  
  // Multi-dimensional metrics collection
  async collectMetrics() {
    const metrics = {
      // System metrics
      system: await this.collectSystemMetrics(),
      
      // Agent-specific metrics
      agents: await this.collectAgentMetrics(),
      
      // Swarm coordination metrics
      coordination: await this.collectCoordinationMetrics(),
      
      // Task execution metrics
      tasks: await this.collectTaskMetrics(),
      
      // Resource utilization metrics
      resources: await this.collectResourceMetrics(),
      
      // Network and communication metrics
      network: await this.collectNetworkMetrics()
    };
    
    // Real-time processing and analysis
    await this.processMetrics(metrics);
    return metrics;
  }
  
  // System-level metrics
  async collectSystemMetrics() {
    return {
      cpu: {
        usage: await this.getCPUUsage(),
        loadAverage: await this.getLoadAverage(),
        coreUtilization: await this.getCoreUtilization()
      },
      memory: {
        usage: await this.getMemoryUsage(),
        available: await this.getAvailableMemory(),
        pressure: await this.getMemoryPressure()
      },
      io: {
        diskUsage: await this.getDiskUsage(),
        diskIO: await this.getDiskIOStats(),
        networkIO: await this.getNetworkIOStats()
      },
      processes: {
        count: await this.getProcessCount(),
        threads: await this.getThreadCount(),
        handles: await this.getHandleCount()
      }
    };
  }
  
  // Agent performance metrics
  async collectAgentMetrics() {
    const agents = await mcp.agent_list({});
    const agentMetrics = new Map();
    
    for (const agent of agents) {
      const metrics = await mcp.agent_metrics({ agentId: agent.id });
      agentMetrics.set(agent.id, {
        ...metrics,
        efficiency: this.calculateEfficiency(metrics),
        responsiveness: this.calculateResponsiveness(metrics),
        reliability: this.calculateReliability(metrics)
      });
    }
    
    return agentMetrics;
  }
}
```

### 2. Bottleneck Detection & Analysis
```javascript
// Intelligent bottleneck detection
class BottleneckAnalyzer {
  constructor() {
    this.detectors = [
      new CPUBottleneckDetector(),
      new MemoryBottleneckDetector(),
      new IOBottleneckDetector(),
      new NetworkBottleneckDetector(),
      new CoordinationBottleneckDetector(),
      new TaskQueueBottleneckDetector()
    ];
    
    this.patterns = new Map();
    this.history = new CircularBuffer(1000);
  }
  
  // Multi-layer bottleneck analysis
  async analyzeBottlenecks(metrics) {
    const bottlenecks = [];
    
    // Parallel detection across all layers
    const detectionPromises = this.detectors.map(detector => 
      detector.detect(metrics)
    );
    
    const results = await Promise.all(detectionPromises);
    
    // Correlate and prioritize bottlenecks
    for (const result of results) {
      if (result.detected) {
        bottlenecks.push({
          type: result.type,
          severity: result.severity,
          component: result.component,
          rootCause: result.rootCause,
          impact: result.impact,
          recommendations: result.recommendations,
          timestamp: Date.now()
        });
      }
    }
    
    // Pattern recognition for recurring bottlenecks
    await this.updatePatterns(bottlenecks);
    
    return this.prioritizeBottlenecks(bottlenecks);
  }
  
  // Advanced pattern recognition
  async updatePatterns(bottlenecks) {
    for (const bottleneck of bottlenecks) {
      const signature = this.createBottleneckSignature(bottleneck);
      
      if (this.patterns.has(signature)) {
        const pattern = this.patterns.get(signature);
        pattern.frequency++;
        pattern.lastOccurrence = Date.now();
        pattern.averageInterval = this.calculateAverageInterval(pattern);
      } else {
        this.patterns.set(signature, {
          signature,
          frequency: 1,
          firstOccurrence: Date.now(),
          lastOccurrence: Date.now(),
          averageInterval: 0,
          predictedNext: null
        });
      }
    }
  }
}
```

### 3. SLA Monitoring & Alerting
```javascript
// Service Level Agreement monitoring
class SLAMonitor {
  constructor() {
    this.slaDefinitions = new Map();
    this.violations = new Map();
    this.alertChannels = new Set();
    this.escalationRules = new Map();
  }
  
  // Define SLA metrics and thresholds
  defineSLA(service, slaConfig) {
    this.slaDefinitions.set(service, {
      availability: slaConfig.availability || 99.9, // percentage
      responseTime: slaConfig.responseTime || 1000, // milliseconds
      throughput: slaConfig.throughput || 100, // requests per second
      errorRate: slaConfig.errorRate || 0.1, // percentage
      recoveryTime: slaConfig.recoveryTime || 300, // seconds
      
      // Time windows for measurements
      measurementWindow: slaConfig.measurementWindow || 300, // seconds
      evaluationInterval: slaConfig.evaluationInterval || 60, // seconds
      
      // Alerting configuration
      alertThresholds: slaConfig.alertThresholds || {
        warning: 0.8, // 80% of SLA threshold
        critical: 0.9, // 90% of SLA threshold
        breach: 1.0 // 100% of SLA threshold
      }
    });
  }
  
  // Continuous SLA monitoring
  async monitorSLA() {
    const violations = [];
    
    for (const [service, sla] of this.slaDefinitions) {
      const metrics = await this.getServiceMetrics(service);
      const evaluation = this.evaluateSLA(service, sla, metrics);
      
      if (evaluation.violated) {
        violations.push(evaluation);
        await this.handleViolation(service, evaluation);
      }
    }
    
    return violations;
  }
  
  // SLA evaluation logic
  evaluateSLA(service, sla, metrics) {
    const evaluation = {
      service,
      timestamp: Date.now(),
      violated: false,
      violations: []
    };
    
    // Availability check
    if (metrics.availability < sla.availability) {
      evaluation.violations.push({
        metric: 'availability',
        expected: sla.availability,
        actual: metrics.availability,
        severity: this.calculateSeverity(metrics.availability, sla.availability, sla.alertThresholds)
      });
      evaluation.violated = true;
    }
    
    // Response time check
    if (metrics.responseTime > sla.responseTime) {
      evaluation.violations.push({
        metric: 'responseTime',
        expected: sla.responseTime,
        actual: metrics.responseTime,
        severity: this.calculateSeverity(metrics.responseTime, sla.responseTime, sla.alertThresholds)
      });
      evaluation.violated = true;
    }
    
    // Additional SLA checks...
    
    return evaluation;
  }
}
```

### 4. Resource Utilization Tracking
```javascript
// Comprehensive resource tracking
class ResourceTracker {
  constructor() {
    this.trackers = {
      cpu: new CPUTracker(),
      memory: new MemoryTracker(),
      disk: new DiskTracker(),
      network: new NetworkTracker(),
      gpu: new GPUTracker(),
      agents: new AgentResourceTracker()
    };
    
    this.forecaster = new ResourceForecaster();
    this.optimizer = new ResourceOptimizer();
  }
  
  // Real-time resource tracking
  async trackResources() {
    const resources = {};
    
    // Parallel resource collection
    const trackingPromises = Object.entries(this.trackers).map(
      async ([type, tracker]) => [type, await tracker.collect()]
    );
    
    const results = await Promise.all(trackingPromises);
    
    for (const [type, data] of results) {
      resources[type] = {
        ...data,
        utilization: this.calculateUtilization(data),
        efficiency: this.calculateEfficiency(data),
        trend: this.calculateTrend(type, data),
        forecast: await this.forecaster.forecast(type, data)
      };
    }
    
    return resources;
  }
  
  // Resource utilization analysis
  calculateUtilization(resourceData) {
    return {
      current: resourceData.used / resourceData.total,
      peak: resourceData.peak / resourceData.total,
      average: resourceData.average / resourceData.total,
      percentiles: {
        p50: resourceData.p50 / resourceData.total,
        p90: resourceData.p90 / resourceData.total,
        p95: resourceData.p95 / resourceData.total,
        p99: resourceData.p99 / resourceData.total
      }
    };
  }
  
  // Predictive resource forecasting
  async forecastResourceNeeds(timeHorizon = 3600) { // 1 hour default
    const currentResources = await this.trackResources();
    const forecasts = {};
    
    for (const [type, data] of Object.entries(currentResources)) {
      forecasts[type] = await this.forecaster.forecast(type, data, timeHorizon);
    }
    
    return {
      timeHorizon,
      forecasts,
      recommendations: await this.optimizer.generateRecommendations(forecasts),
      confidence: this.calculateForecastConfidence(forecasts)
    };
  }
}
```

## MCP Integration Hooks

### Performance Data Collection
```javascript
// Comprehensive MCP integration
const performanceIntegration = {
  // Real-time performance monitoring
  async startMonitoring(config = {}) {
    const monitoringTasks = [
      this.monitorSwarmHealth(),
      this.monitorAgentPerformance(),
      this.monitorResourceUtilization(),
      this.monitorBottlenecks(),
      this.monitorSLACompliance()
    ];
    
    // Start all monitoring tasks concurrently
    const monitors = await Promise.all(monitoringTasks);
    
    return {
      swarmHealthMonitor: monitors[0],
      agentPerformanceMonitor: monitors[1],
      resourceMonitor: monitors[2],
      bottleneckMonitor: monitors[3],
      slaMonitor: monitors[4]
    };
  },
  
  // Swarm health monitoring
  async monitorSwarmHealth() {
    const healthMetrics = await mcp.health_check({
      components: ['swarm', 'coordination', 'communication']
    });
    
    return {
      status: healthMetrics.overall,
      components: healthMetrics.components,
      issues: healthMetrics.issues,
      recommendations: healthMetrics.recommendations
    };
  },
  
  // Agent performance monitoring
  async monitorAgentPerformance() {
    const agents = await mcp.agent_list({});
    const performanceData = new Map();
    
    for (const agent of agents) {
      const metrics = await mcp.agent_metrics({ agentId: agent.id });
      const performance = await mcp.performance_report({
        format: 'detailed',
        timeframe: '24h'
      });
      
      performanceData.set(agent.id, {
        ...metrics,
        performance,
        efficiency: this.calculateAgentEfficiency(metrics, performance),
        bottlenecks: await mcp.bottleneck_analyze({ component: agent.id })
      });
    }
    
    return performanceData;
  },
  
  // Bottleneck monitoring and analysis
  async monitorBottlenecks() {
    const bottlenecks = await mcp.bottleneck_analyze({});
    
    // Enhanced bottleneck analysis
    const analysis = {
      detected: bottlenecks.length > 0,
      count: bottlenecks.length,
      severity: this.calculateOverallSeverity(bottlenecks),
      categories: this.categorizeBottlenecks(bottlenecks),
      trends: await this.analyzeBottleneckTrends(bottlenecks),
      predictions: await this.predictBottlenecks(bottlenecks)
    };
    
    return analysis;
  }
};
```

### Anomaly Detection
```javascript
// Advanced anomaly detection system
class AnomalyDetector {
  constructor() {
    this.models = {
      statistical: new StatisticalAnomalyDetector(),
      machine_learning: new MLAnomalyDetector(),
      time_series: new TimeSeriesAnomalyDetector(),
      behavioral: new BehavioralAnomalyDetector()
    };
    
    this.ensemble = new EnsembleDetector(this.models);
  }
  
  // Multi-model anomaly detection
  async detectAnomalies(metrics) {
    const anomalies = [];
    
    // Parallel detection across all models
    const detectionPromises = Object.entries(this.models).map(
      async ([modelType, model]) => {
        const detected = await model.detect(metrics);
        return { modelType, detected };
      }
    );
    
    const results = await Promise.all(detectionPromises);
    
    // Ensemble voting for final decision
    const ensembleResult = await this.ensemble.vote(results);
    
    return {
      anomalies: ensembleResult.anomalies,
      confidence: ensembleResult.confidence,
      consensus: ensembleResult.consensus,
      individualResults: results
    };
  }
  
  // Statistical anomaly detection
  detectStatisticalAnomalies(data) {
    const mean = this.calculateMean(data);
    const stdDev = this.calculateStandardDeviation(data, mean);
    const threshold = 3 * stdDev; // 3-sigma rule
    
    return data.filter(point => Math.abs(point - mean) > threshold)
               .map(point => ({
                 value: point,
                 type: 'statistical',
                 deviation: Math.abs(point - mean) / stdDev,
                 probability: this.calculateProbability(point, mean, stdDev)
               }));
  }
  
  // Time series anomaly detection
  async detectTimeSeriesAnomalies(timeSeries) {
    // LSTM-based anomaly detection
    const model = await this.loadTimeSeriesModel();
    const predictions = await model.predict(timeSeries);
    
    const anomalies = [];
    for (let i = 0; i < timeSeries.length; i++) {
      const error = Math.abs(timeSeries[i] - predictions[i]);
      const threshold = this.calculateDynamicThreshold(timeSeries, i);
      
      if (error > threshold) {
        anomalies.push({
          timestamp: i,
          actual: timeSeries[i],
          predicted: predictions[i],
          error: error,
          type: 'time_series'
        });
      }
    }
    
    return anomalies;
  }
}
```

## Dashboard Integration

### Real-Time Performance Dashboard
```javascript
// Dashboard data provider
class DashboardProvider {
  constructor() {
    this.updateInterval = 1000; // 1 second updates
    this.subscribers = new Set();
    this.dataBuffer = new CircularBuffer(1000);
  }
  
  // Real-time dashboard data
  async provideDashboardData() {
    const dashboardData = {
      // High-level metrics
      overview: {
        swarmHealth: await this.getSwarmHealthScore(),
        activeAgents: await this.getActiveAgentCount(),
        totalTasks: await this.getTotalTaskCount(),
        averageResponseTime: await this.getAverageResponseTime()
      },
      
      // Performance metrics
      performance: {
        throughput: await this.getCurrentThroughput(),
        latency: await this.getCurrentLatency(),
        errorRate: await this.getCurrentErrorRate(),
        utilization: await this.getResourceUtilization()
      },
      
      // Real-time charts data
      timeSeries: {
        cpu: this.getCPUTimeSeries(),
        memory: this.getMemoryTimeSeries(),
        network: this.getNetworkTimeSeries(),
        tasks: this.getTaskTimeSeries()
      },
      
      // Alerts and notifications
      alerts: await this.getActiveAlerts(),
      notifications: await this.getRecentNotifications(),
      
      // Agent status
      agents: await this.getAgentStatusSummary(),
      
      timestamp: Date.now()
    };
    
    // Broadcast to subscribers
    this.broadcast(dashboardData);
    
    return dashboardData;
  }
  
  // WebSocket subscription management
  subscribe(callback) {
    this.subscribers.add(callback);
    return () => this.subscribers.delete(callback);
  }
  
  broadcast(data) {
    this.subscribers.forEach(callback => {
      try {
        callback(data);
      } catch (error) {
        console.error('Dashboard subscriber error:', error);
      }
    });
  }
}
```

## Operational Commands

### Monitoring Commands
```bash
# Start comprehensive monitoring
npx claude-flow performance-report --format detailed --timeframe 24h

# Real-time bottleneck analysis
npx claude-flow bottleneck-analyze --component swarm-coordination

# Health check all components
npx claude-flow health-check --components ["swarm", "agents", "coordination"]

# Collect specific metrics
npx claude-flow metrics-collect --components ["cpu", "memory", "network"]

# Monitor SLA compliance
npx claude-flow sla-monitor --service swarm-coordination --threshold 99.9
```

### Alert Configuration
```bash
# Configure performance alerts
npx claude-flow alert-config --metric cpu_usage --threshold 80 --severity warning

# Set up anomaly detection
npx claude-flow anomaly-setup --models ["statistical", "ml", "time_series"]

# Configure notification channels
npx claude-flow notification-config --channels ["slack", "email", "webhook"]
```

## Integration Points

### With Other Optimization Agents
- **Load Balancer**: Provides performance data for load balancing decisions
- **Topology Optimizer**: Supplies network and coordination metrics
- **Resource Manager**: Shares resource utilization and forecasting data

### With Swarm Infrastructure
- **Task Orchestrator**: Monitors task execution performance
- **Agent Coordinator**: Tracks agent health and performance
- **Memory System**: Stores historical performance data and patterns

## Performance Analytics

### Key Metrics Dashboard
```javascript
// Performance analytics engine
const analytics = {
  // Key Performance Indicators
  calculateKPIs(metrics) {
    return {
      // Availability metrics
      uptime: this.calculateUptime(metrics),
      availability: this.calculateAvailability(metrics),
      
      // Performance metrics
      responseTime: {
        average: this.calculateAverage(metrics.responseTimes),
        p50: this.calculatePercentile(metrics.responseTimes, 50),
        p90: this.calculatePercentile(metrics.responseTimes, 90),
        p95: this.calculatePercentile(metrics.responseTimes, 95),
        p99: this.calculatePercentile(metrics.responseTimes, 99)
      },
      
      // Throughput metrics
      throughput: this.calculateThroughput(metrics),
      
      // Error metrics
      errorRate: this.calculateErrorRate(metrics),
      
      // Resource efficiency
      resourceEfficiency: this.calculateResourceEfficiency(metrics),
      
      // Cost metrics
      costEfficiency: this.calculateCostEfficiency(metrics)
    };
  },
  
  // Trend analysis
  analyzeTrends(historicalData, timeWindow = '7d') {
    return {
      performance: this.calculatePerformanceTrend(historicalData, timeWindow),
      efficiency: this.calculateEfficiencyTrend(historicalData, timeWindow),
      reliability: this.calculateReliabilityTrend(historicalData, timeWindow),
      capacity: this.calculateCapacityTrend(historicalData, timeWindow)
    };
  }
};
```

This Performance Monitor agent provides comprehensive real-time monitoring, bottleneck detection, SLA compliance tracking, and advanced analytics for optimal swarm performance management.