---
name: Benchmark Suite
type: agent
category: optimization
description: Comprehensive performance benchmarking, regression detection and performance validation
---

# Benchmark Suite Agent

## Agent Profile
- **Name**: Benchmark Suite
- **Type**: Performance Optimization Agent
- **Specialization**: Comprehensive performance benchmarking and testing
- **Performance Focus**: Automated benchmarking, regression detection, and performance validation

## Core Capabilities

### 1. Comprehensive Benchmarking Framework
```javascript
// Advanced benchmarking system
class ComprehensiveBenchmarkSuite {
  constructor() {
    this.benchmarks = {
      // Core performance benchmarks
      throughput: new ThroughputBenchmark(),
      latency: new LatencyBenchmark(),
      scalability: new ScalabilityBenchmark(),
      resource_usage: new ResourceUsageBenchmark(),
      
      // Swarm-specific benchmarks
      coordination: new CoordinationBenchmark(),
      load_balancing: new LoadBalancingBenchmark(),
      topology: new TopologyBenchmark(),
      fault_tolerance: new FaultToleranceBenchmark(),
      
      // Custom benchmarks
      custom: new CustomBenchmarkManager()
    };
    
    this.reporter = new BenchmarkReporter();
    this.comparator = new PerformanceComparator();
    this.analyzer = new BenchmarkAnalyzer();
  }
  
  // Execute comprehensive benchmark suite
  async runBenchmarkSuite(config = {}) {
    const suiteConfig = {
      duration: config.duration || 300000, // 5 minutes default
      iterations: config.iterations || 10,
      warmupTime: config.warmupTime || 30000, // 30 seconds
      cooldownTime: config.cooldownTime || 10000, // 10 seconds
      parallel: config.parallel || false,
      baseline: config.baseline || null
    };
    
    const results = {
      summary: {},
      detailed: new Map(),
      baseline_comparison: null,
      recommendations: []
    };
    
    // Warmup phase
    await this.warmup(suiteConfig.warmupTime);
    
    // Execute benchmarks
    if (suiteConfig.parallel) {
      results.detailed = await this.runBenchmarksParallel(suiteConfig);
    } else {
      results.detailed = await this.runBenchmarksSequential(suiteConfig);
    }
    
    // Generate summary
    results.summary = this.generateSummary(results.detailed);
    
    // Compare with baseline if provided
    if (suiteConfig.baseline) {
      results.baseline_comparison = await this.compareWithBaseline(
        results.detailed, 
        suiteConfig.baseline
      );
    }
    
    // Generate recommendations
    results.recommendations = await this.generateRecommendations(results);
    
    // Cooldown phase
    await this.cooldown(suiteConfig.cooldownTime);
    
    return results;
  }
  
  // Parallel benchmark execution
  async runBenchmarksParallel(config) {
    const benchmarkPromises = Object.entries(this.benchmarks).map(
      async ([name, benchmark]) => {
        const result = await this.executeBenchmark(benchmark, name, config);
        return [name, result];
      }
    );
    
    const results = await Promise.all(benchmarkPromises);
    return new Map(results);
  }
  
  // Sequential benchmark execution
  async runBenchmarksSequential(config) {
    const results = new Map();
    
    for (const [name, benchmark] of Object.entries(this.benchmarks)) {
      const result = await this.executeBenchmark(benchmark, name, config);
      results.set(name, result);
      
      // Brief pause between benchmarks
      await this.sleep(1000);
    }
    
    return results;
  }
}
```

### 2. Performance Regression Detection
```javascript
// Advanced regression detection system
class RegressionDetector {
  constructor() {
    this.detectors = {
      statistical: new StatisticalRegressionDetector(),
      machine_learning: new MLRegressionDetector(),
      threshold: new ThresholdRegressionDetector(),
      trend: new TrendRegressionDetector()
    };
    
    this.analyzer = new RegressionAnalyzer();
    this.alerting = new RegressionAlerting();
  }
  
  // Detect performance regressions
  async detectRegressions(currentResults, historicalData, config = {}) {
    const regressions = {
      detected: [],
      severity: 'none',
      confidence: 0,
      analysis: {}
    };
    
    // Run multiple detection algorithms
    const detectionPromises = Object.entries(this.detectors).map(
      async ([method, detector]) => {
        const detection = await detector.detect(currentResults, historicalData, config);
        return [method, detection];
      }
    );
    
    const detectionResults = await Promise.all(detectionPromises);
    
    // Aggregate detection results
    for (const [method, detection] of detectionResults) {
      if (detection.regression_detected) {
        regressions.detected.push({
          method,
          ...detection
        });
      }
    }
    
    // Calculate overall confidence and severity
    if (regressions.detected.length > 0) {
      regressions.confidence = this.calculateAggregateConfidence(regressions.detected);
      regressions.severity = this.calculateSeverity(regressions.detected);
      regressions.analysis = await this.analyzer.analyze(regressions.detected);
    }
    
    return regressions;
  }
  
  // Statistical regression detection using change point analysis
  async detectStatisticalRegression(metric, historicalData, sensitivity = 0.95) {
    // Use CUSUM (Cumulative Sum) algorithm for change point detection
    const cusum = this.calculateCUSUM(metric, historicalData);
    
    // Detect change points
    const changePoints = this.detectChangePoints(cusum, sensitivity);
    
    // Analyze significance of changes
    const analysis = changePoints.map(point => ({
      timestamp: point.timestamp,
      magnitude: point.magnitude,
      direction: point.direction,
      significance: point.significance,
      confidence: point.confidence
    }));
    
    return {
      regression_detected: changePoints.length > 0,
      change_points: analysis,
      cusum_statistics: cusum.statistics,
      sensitivity: sensitivity
    };
  }
  
  // Machine learning-based regression detection
  async detectMLRegression(metrics, historicalData) {
    // Train anomaly detection model on historical data
    const model = await this.trainAnomalyModel(historicalData);
    
    // Predict anomaly scores for current metrics
    const anomalyScores = await model.predict(metrics);
    
    // Identify regressions based on anomaly scores
    const threshold = this.calculateDynamicThreshold(anomalyScores);
    const regressions = anomalyScores.filter(score => score.anomaly > threshold);
    
    return {
      regression_detected: regressions.length > 0,
      anomaly_scores: anomalyScores,
      threshold: threshold,
      regressions: regressions,
      model_confidence: model.confidence
    };
  }
}
```

### 3. Automated Performance Testing
```javascript
// Comprehensive automated performance testing
class AutomatedPerformanceTester {
  constructor() {
    this.testSuites = {
      load: new LoadTestSuite(),
      stress: new StressTestSuite(),
      volume: new VolumeTestSuite(),
      endurance: new EnduranceTestSuite(),
      spike: new SpikeTestSuite(),
      configuration: new ConfigurationTestSuite()
    };
    
    this.scheduler = new TestScheduler();
    this.orchestrator = new TestOrchestrator();
    this.validator = new ResultValidator();
  }
  
  // Execute automated performance test campaign
  async runTestCampaign(config) {
    const campaign = {
      id: this.generateCampaignId(),
      config,
      startTime: Date.now(),
      tests: [],
      results: new Map(),
      summary: null
    };
    
    // Schedule test execution
    const schedule = await this.scheduler.schedule(config.tests, config.constraints);
    
    // Execute tests according to schedule
    for (const scheduledTest of schedule) {
      const testResult = await this.executeScheduledTest(scheduledTest);
      campaign.tests.push(scheduledTest);
      campaign.results.set(scheduledTest.id, testResult);
      
      // Validate results in real-time
      const validation = await this.validator.validate(testResult);
      if (!validation.valid) {
        campaign.summary = {
          status: 'failed',
          reason: validation.reason,
          failedAt: scheduledTest.name
        };
        break;
      }
    }
    
    // Generate campaign summary
    if (!campaign.summary) {
      campaign.summary = await this.generateCampaignSummary(campaign);
    }
    
    campaign.endTime = Date.now();
    campaign.duration = campaign.endTime - campaign.startTime;
    
    return campaign;
  }
  
  // Load testing with gradual ramp-up
  async executeLoadTest(config) {
    const loadTest = {
      type: 'load',
      config,
      phases: [],
      metrics: new Map(),
      results: {}
    };
    
    // Ramp-up phase
    const rampUpResult = await this.executeRampUp(config.rampUp);
    loadTest.phases.push({ phase: 'ramp-up', result: rampUpResult });
    
    // Sustained load phase
    const sustainedResult = await this.executeSustainedLoad(config.sustained);
    loadTest.phases.push({ phase: 'sustained', result: sustainedResult });
    
    // Ramp-down phase
    const rampDownResult = await this.executeRampDown(config.rampDown);
    loadTest.phases.push({ phase: 'ramp-down', result: rampDownResult });
    
    // Analyze results
    loadTest.results = await this.analyzeLoadTestResults(loadTest.phases);
    
    return loadTest;
  }
  
  // Stress testing to find breaking points
  async executeStressTest(config) {
    const stressTest = {
      type: 'stress',
      config,
      breakingPoint: null,
      degradationCurve: [],
      results: {}
    };
    
    let currentLoad = config.startLoad;
    let systemBroken = false;
    
    while (!systemBroken && currentLoad <= config.maxLoad) {
      const testResult = await this.applyLoad(currentLoad, config.duration);
      
      stressTest.degradationCurve.push({
        load: currentLoad,
        performance: testResult.performance,
        stability: testResult.stability,
        errors: testResult.errors
      });
      
      // Check if system is breaking
      if (this.isSystemBreaking(testResult, config.breakingCriteria)) {
        stressTest.breakingPoint = {
          load: currentLoad,
          performance: testResult.performance,
          reason: this.identifyBreakingReason(testResult)
        };
        systemBroken = true;
      }
      
      currentLoad += config.loadIncrement;
    }
    
    stressTest.results = await this.analyzeStressTestResults(stressTest);
    
    return stressTest;
  }
}
```

### 4. Performance Validation Framework
```javascript
// Comprehensive performance validation
class PerformanceValidator {
  constructor() {
    this.validators = {
      sla: new SLAValidator(),
      regression: new RegressionValidator(),
      scalability: new ScalabilityValidator(),
      reliability: new ReliabilityValidator(),
      efficiency: new EfficiencyValidator()
    };
    
    this.thresholds = new ThresholdManager();
    this.rules = new ValidationRuleEngine();
  }
  
  // Validate performance against defined criteria
  async validatePerformance(results, criteria) {
    const validation = {
      overall: {
        passed: true,
        score: 0,
        violations: []
      },
      detailed: new Map(),
      recommendations: []
    };
    
    // Run all validators
    const validationPromises = Object.entries(this.validators).map(
      async ([type, validator]) => {
        const result = await validator.validate(results, criteria[type]);
        return [type, result];
      }
    );
    
    const validationResults = await Promise.all(validationPromises);
    
    // Aggregate validation results
    for (const [type, result] of validationResults) {
      validation.detailed.set(type, result);
      
      if (!result.passed) {
        validation.overall.passed = false;
        validation.overall.violations.push(...result.violations);
      }
      
      validation.overall.score += result.score * (criteria[type]?.weight || 1);
    }
    
    // Normalize overall score
    const totalWeight = Object.values(criteria).reduce((sum, c) => sum + (c.weight || 1), 0);
    validation.overall.score /= totalWeight;
    
    // Generate recommendations
    validation.recommendations = await this.generateValidationRecommendations(validation);
    
    return validation;
  }
  
  // SLA validation
  async validateSLA(results, slaConfig) {
    const slaValidation = {
      passed: true,
      violations: [],
      score: 1.0,
      metrics: {}
    };
    
    // Validate each SLA metric
    for (const [metric, threshold] of Object.entries(slaConfig.thresholds)) {
      const actualValue = this.extractMetricValue(results, metric);
      const validation = this.validateThreshold(actualValue, threshold);
      
      slaValidation.metrics[metric] = {
        actual: actualValue,
        threshold: threshold.value,
        operator: threshold.operator,
        passed: validation.passed,
        deviation: validation.deviation
      };
      
      if (!validation.passed) {
        slaValidation.passed = false;
        slaValidation.violations.push({
          metric,
          actual: actualValue,
          expected: threshold.value,
          severity: threshold.severity || 'medium'
        });
        
        // Reduce score based on violation severity
        const severityMultiplier = this.getSeverityMultiplier(threshold.severity);
        slaValidation.score -= (validation.deviation * severityMultiplier);
      }
    }
    
    slaValidation.score = Math.max(0, slaValidation.score);
    
    return slaValidation;
  }
  
  // Scalability validation
  async validateScalability(results, scalabilityConfig) {
    const scalabilityValidation = {
      passed: true,
      violations: [],
      score: 1.0,
      analysis: {}
    };
    
    // Linear scalability analysis
    if (scalabilityConfig.linear) {
      const linearityAnalysis = this.analyzeLinearScalability(results);
      scalabilityValidation.analysis.linearity = linearityAnalysis;
      
      if (linearityAnalysis.coefficient < scalabilityConfig.linear.minCoefficient) {
        scalabilityValidation.passed = false;
        scalabilityValidation.violations.push({
          type: 'linearity',
          actual: linearityAnalysis.coefficient,
          expected: scalabilityConfig.linear.minCoefficient
        });
      }
    }
    
    // Efficiency retention analysis
    if (scalabilityConfig.efficiency) {
      const efficiencyAnalysis = this.analyzeEfficiencyRetention(results);
      scalabilityValidation.analysis.efficiency = efficiencyAnalysis;
      
      if (efficiencyAnalysis.retention < scalabilityConfig.efficiency.minRetention) {
        scalabilityValidation.passed = false;
        scalabilityValidation.violations.push({
          type: 'efficiency_retention',
          actual: efficiencyAnalysis.retention,
          expected: scalabilityConfig.efficiency.minRetention
        });
      }
    }
    
    return scalabilityValidation;
  }
}
```

## MCP Integration Hooks

### Benchmark Execution Integration
```javascript
// Comprehensive MCP benchmark integration
const benchmarkIntegration = {
  // Execute performance benchmarks
  async runBenchmarks(config = {}) {
    // Run benchmark suite
    const benchmarkResult = await mcp.benchmark_run({
      suite: config.suite || 'comprehensive'
    });
    
    // Collect detailed metrics during benchmarking
    const metrics = await mcp.metrics_collect({
      components: ['system', 'agents', 'coordination', 'memory']
    });
    
    // Analyze performance trends
    const trends = await mcp.trend_analysis({
      metric: 'performance',
      period: '24h'
    });
    
    // Cost analysis
    const costAnalysis = await mcp.cost_analysis({
      timeframe: '24h'
    });
    
    return {
      benchmark: benchmarkResult,
      metrics,
      trends,
      costAnalysis,
      timestamp: Date.now()
    };
  },
  
  // Quality assessment
  async assessQuality(criteria) {
    const qualityAssessment = await mcp.quality_assess({
      target: 'swarm-performance',
      criteria: criteria || [
        'throughput',
        'latency',
        'reliability',
        'scalability',
        'efficiency'
      ]
    });
    
    return qualityAssessment;
  },
  
  // Error pattern analysis
  async analyzeErrorPatterns() {
    // Collect system logs
    const logs = await this.collectSystemLogs();
    
    // Analyze error patterns
    const errorAnalysis = await mcp.error_analysis({
      logs: logs
    });
    
    return errorAnalysis;
  }
};
```

## Operational Commands

### Benchmarking Commands
```bash
# Run comprehensive benchmark suite
npx claude-flow benchmark-run --suite comprehensive --duration 300

# Execute specific benchmark
npx claude-flow benchmark-run --suite throughput --iterations 10

# Compare with baseline
npx claude-flow benchmark-compare --current <results> --baseline <baseline>

# Quality assessment
npx claude-flow quality-assess --target swarm-performance --criteria throughput,latency

# Performance validation
npx claude-flow validate-performance --results <file> --criteria <file>
```

### Regression Detection Commands
```bash
# Detect performance regressions
npx claude-flow detect-regression --current <results> --historical <data>

# Set up automated regression monitoring
npx claude-flow regression-monitor --enable --sensitivity 0.95

# Analyze error patterns
npx claude-flow error-analysis --logs <log-files>
```

## Integration Points

### With Other Optimization Agents
- **Performance Monitor**: Provides continuous monitoring data for benchmarking
- **Load Balancer**: Validates load balancing effectiveness through benchmarks
- **Topology Optimizer**: Tests topology configurations for optimal performance

### With CI/CD Pipeline
- **Automated Testing**: Integrates with CI/CD for continuous performance validation
- **Quality Gates**: Provides pass/fail criteria for deployment decisions
- **Regression Prevention**: Catches performance regressions before production

## Performance Benchmarks

### Standard Benchmark Suite
```javascript
// Comprehensive benchmark definitions
const standardBenchmarks = {
  // Throughput benchmarks
  throughput: {
    name: 'Throughput Benchmark',
    metrics: ['requests_per_second', 'tasks_per_second', 'messages_per_second'],
    duration: 300000, // 5 minutes
    warmup: 30000,    // 30 seconds
    targets: {
      requests_per_second: { min: 1000, optimal: 5000 },
      tasks_per_second: { min: 100, optimal: 500 },
      messages_per_second: { min: 10000, optimal: 50000 }
    }
  },
  
  // Latency benchmarks
  latency: {
    name: 'Latency Benchmark',
    metrics: ['p50', 'p90', 'p95', 'p99', 'max'],
    duration: 300000,
    targets: {
      p50: { max: 100 },   // 100ms
      p90: { max: 200 },   // 200ms
      p95: { max: 500 },   // 500ms
      p99: { max: 1000 },  // 1s
      max: { max: 5000 }   // 5s
    }
  },
  
  // Scalability benchmarks
  scalability: {
    name: 'Scalability Benchmark',
    metrics: ['linear_coefficient', 'efficiency_retention'],
    load_points: [1, 2, 4, 8, 16, 32, 64],
    targets: {
      linear_coefficient: { min: 0.8 },
      efficiency_retention: { min: 0.7 }
    }
  }
};
```

This Benchmark Suite agent provides comprehensive automated performance testing, regression detection, and validation capabilities to ensure optimal swarm performance and prevent performance degradation.