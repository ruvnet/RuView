/**
 * Results Analyzer for RuVector Benchmarks
 *
 * Performs statistical analysis, comparisons, and generates recommendations
 */

import * as fs from 'fs';
import * as path from 'path';
import { ComprehensiveMetrics, LatencyMetrics } from './metrics-collector';

// Analysis result types
export interface StatisticalAnalysis {
  scenario: string;
  summary: {
    totalRequests: number;
    successfulRequests: number;
    failedRequests: number;
    averageLatency: number;
    medianLatency: number;
    p99Latency: number;
    throughput: number;
    errorRate: number;
    availability: number;
  };
  distribution: {
    latencyHistogram: HistogramBucket[];
    throughputOverTime: TimeSeriesData[];
    errorRateOverTime: TimeSeriesData[];
  };
  correlation: {
    latencyVsThroughput: number;
    errorsVsLoad: number;
    resourceVsLatency: number;
  };
  anomalies: Anomaly[];
}

export interface HistogramBucket {
  min: number;
  max: number;
  count: number;
  percentage: number;
}

export interface TimeSeriesData {
  timestamp: number;
  value: number;
}

export interface Anomaly {
  type: 'spike' | 'drop' | 'plateau' | 'oscillation';
  metric: string;
  timestamp: number;
  severity: 'low' | 'medium' | 'high' | 'critical';
  description: string;
  impact: string;
}

export interface Comparison {
  baseline: string;
  current: string;
  improvements: Record<string, number>; // metric -> % change
  regressions: Record<string, number>;
  summary: string;
}

export interface Bottleneck {
  component: string;
  metric: string;
  severity: 'low' | 'medium' | 'high' | 'critical';
  currentValue: number;
  threshold: number;
  impact: string;
  recommendation: string;
}

export interface Recommendation {
  category: 'performance' | 'scalability' | 'reliability' | 'cost';
  priority: 'low' | 'medium' | 'high' | 'critical';
  title: string;
  description: string;
  implementation: string;
  estimatedImpact: string;
  estimatedCost: number;
}

export interface AnalysisReport {
  testId: string;
  scenario: string;
  timestamp: number;
  statistical: StatisticalAnalysis;
  slaCompliance: SLACompliance;
  bottlenecks: Bottleneck[];
  recommendations: Recommendation[];
  comparison?: Comparison;
  score: {
    performance: number; // 0-100
    reliability: number;
    scalability: number;
    efficiency: number;
    overall: number;
  };
}

export interface SLACompliance {
  met: boolean;
  details: {
    latency: {
      target: number;
      actual: number;
      met: boolean;
    };
    availability: {
      target: number;
      actual: number;
      met: boolean;
    };
    errorRate: {
      target: number;
      actual: number;
      met: boolean;
    };
  };
  violations: Array<{
    metric: string;
    timestamp: number;
    duration: number;
    severity: string;
  }>;
}

// Results analyzer class
export class ResultsAnalyzer {
  private outputDir: string;

  constructor(outputDir: string = './results') {
    this.outputDir = outputDir;
  }

  // Perform statistical analysis
  analyzeStatistics(metrics: ComprehensiveMetrics): StatisticalAnalysis {
    const totalRequests = metrics.throughput.queriesPerSecond * (metrics.duration / 1000);
    const failedRequests = metrics.errors.totalErrors;
    const successfulRequests = totalRequests - failedRequests;

    return {
      scenario: metrics.scenario,
      summary: {
        totalRequests,
        successfulRequests,
        failedRequests,
        averageLatency: metrics.latency.mean,
        medianLatency: metrics.latency.median,
        p99Latency: metrics.latency.p99,
        throughput: metrics.throughput.queriesPerSecond,
        errorRate: metrics.errors.errorRate,
        availability: metrics.availability.uptime,
      },
      distribution: {
        latencyHistogram: this.createLatencyHistogram(metrics.latency),
        throughputOverTime: [], // TODO: Extract from time series
        errorRateOverTime: [], // TODO: Extract from time series
      },
      correlation: {
        latencyVsThroughput: 0, // TODO: Calculate correlation
        errorsVsLoad: 0,
        resourceVsLatency: 0,
      },
      anomalies: this.detectAnomalies(metrics),
    };
  }

  // Create latency histogram
  private createLatencyHistogram(latency: LatencyMetrics): HistogramBucket[] {
    // NOTE: This function cannot create accurate histograms without raw latency samples.
    // We only have percentile data (p50, p95, p99), which is insufficient for distribution.
    // Returning empty histogram to avoid fabricating data.

    console.warn(
      'Cannot generate latency histogram without raw sample data. ' +
      'Only percentile metrics (p50, p95, p99) are available. ' +
      'To get accurate histograms, modify metrics collection to store raw latency samples.'
    );

    return []; // Return empty array instead of fabricated data
  }

  // Detect anomalies
  private detectAnomalies(metrics: ComprehensiveMetrics): Anomaly[] {
    const anomalies: Anomaly[] = [];

    // Latency spikes
    if (metrics.latency.p99 > metrics.latency.mean * 5) {
      anomalies.push({
        type: 'spike',
        metric: 'latency',
        timestamp: metrics.endTime,
        severity: 'high',
        description: `P99 latency (${metrics.latency.p99}ms) is 5x higher than mean (${metrics.latency.mean}ms)`,
        impact: 'Users experiencing slow responses',
      });
    }

    // Error rate spikes
    if (metrics.errors.errorRate > 1) {
      anomalies.push({
        type: 'spike',
        metric: 'error_rate',
        timestamp: metrics.endTime,
        severity: 'critical',
        description: `Error rate (${metrics.errors.errorRate}%) exceeds acceptable threshold`,
        impact: 'Service degradation affecting users',
      });
    }

    // Throughput drops
    if (metrics.throughput.averageQPS < metrics.throughput.peakQPS * 0.5) {
      anomalies.push({
        type: 'drop',
        metric: 'throughput',
        timestamp: metrics.endTime,
        severity: 'medium',
        description: 'Throughput dropped below 50% of peak capacity',
        impact: 'Reduced capacity affecting scalability',
      });
    }

    // Resource saturation
    if (metrics.resources.cpu.peak > 90) {
      anomalies.push({
        type: 'plateau',
        metric: 'cpu',
        timestamp: metrics.endTime,
        severity: 'high',
        description: `CPU utilization at ${metrics.resources.cpu.peak}%`,
        impact: 'System approaching capacity limits',
      });
    }

    return anomalies;
  }

  // Check SLA compliance
  checkSLACompliance(metrics: ComprehensiveMetrics): SLACompliance {
    const latencyTarget = 50; // p99 < 50ms
    const availabilityTarget = 99.99; // 99.99% uptime
    const errorRateTarget = 0.01; // < 0.01% errors

    const latencyMet = metrics.latency.p99 < latencyTarget;
    const availabilityMet = metrics.availability.uptime >= availabilityTarget;
    const errorRateMet = metrics.errors.errorRate < errorRateTarget;

    const violations: Array<{
      metric: string;
      timestamp: number;
      duration: number;
      severity: string;
    }> = [];

    if (!latencyMet) {
      violations.push({
        metric: 'latency',
        timestamp: metrics.endTime,
        duration: metrics.duration,
        severity: 'high',
      });
    }

    if (!availabilityMet) {
      violations.push({
        metric: 'availability',
        timestamp: metrics.endTime,
        duration: metrics.duration,
        severity: 'critical',
      });
    }

    if (!errorRateMet) {
      violations.push({
        metric: 'error_rate',
        timestamp: metrics.endTime,
        duration: metrics.duration,
        severity: 'high',
      });
    }

    return {
      met: latencyMet && availabilityMet && errorRateMet,
      details: {
        latency: {
          target: latencyTarget,
          actual: metrics.latency.p99,
          met: latencyMet,
        },
        availability: {
          target: availabilityTarget,
          actual: metrics.availability.uptime,
          met: availabilityMet,
        },
        errorRate: {
          target: errorRateTarget,
          actual: metrics.errors.errorRate,
          met: errorRateMet,
        },
      },
      violations,
    };
  }

  // Identify bottlenecks
  identifyBottlenecks(metrics: ComprehensiveMetrics): Bottleneck[] {
    const bottlenecks: Bottleneck[] = [];

    // CPU bottleneck
    if (metrics.resources.cpu.average > 80) {
      bottlenecks.push({
        component: 'compute',
        metric: 'cpu_utilization',
        severity: 'high',
        currentValue: metrics.resources.cpu.average,
        threshold: 80,
        impact: 'High CPU usage limiting throughput and increasing latency',
        recommendation: 'Scale horizontally or optimize CPU-intensive operations',
      });
    }

    // Memory bottleneck
    if (metrics.resources.memory.average > 85) {
      bottlenecks.push({
        component: 'memory',
        metric: 'memory_utilization',
        severity: 'high',
        currentValue: metrics.resources.memory.average,
        threshold: 85,
        impact: 'Memory pressure may cause swapping and degraded performance',
        recommendation: 'Increase memory allocation or optimize memory usage',
      });
    }

    // Network bottleneck
    if (metrics.resources.network.bandwidth > 8000000000) { // 8 Gbps
      bottlenecks.push({
        component: 'network',
        metric: 'bandwidth',
        severity: 'medium',
        currentValue: metrics.resources.network.bandwidth,
        threshold: 8000000000,
        impact: 'Network bandwidth saturation affecting data transfer',
        recommendation: 'Upgrade network capacity or implement compression',
      });
    }

    // Latency bottleneck
    if (metrics.latency.p99 > 100) {
      bottlenecks.push({
        component: 'latency',
        metric: 'p99_latency',
        severity: 'critical',
        currentValue: metrics.latency.p99,
        threshold: 50,
        impact: 'High tail latency affecting user experience',
        recommendation: 'Optimize query processing, add caching, or improve indexing',
      });
    }

    // Regional imbalance
    const regionalLatencies = metrics.regional.map(r => r.latency.mean);
    const maxRegionalLatency = Math.max(...regionalLatencies);
    const minRegionalLatency = Math.min(...regionalLatencies);

    if (maxRegionalLatency > minRegionalLatency * 2) {
      bottlenecks.push({
        component: 'regional_distribution',
        metric: 'latency_variance',
        severity: 'medium',
        currentValue: maxRegionalLatency / minRegionalLatency,
        threshold: 2,
        impact: 'Uneven regional performance affecting global users',
        recommendation: 'Rebalance load across regions or add capacity to slow regions',
      });
    }

    return bottlenecks;
  }

  // Generate recommendations
  generateRecommendations(
    metrics: ComprehensiveMetrics,
    bottlenecks: Bottleneck[]
  ): Recommendation[] {
    const recommendations: Recommendation[] = [];

    // Performance recommendations
    if (metrics.latency.p99 > 50) {
      recommendations.push({
        category: 'performance',
        priority: 'high',
        title: 'Optimize Query Latency',
        description: 'P99 latency exceeds target of 50ms',
        implementation: 'Add query result caching, optimize vector indexing (HNSW tuning), implement query batching',
        estimatedImpact: '30-50% latency reduction',
        estimatedCost: 5000,
      });
    }

    // Scalability recommendations
    if (bottlenecks.some(b => b.component === 'compute')) {
      recommendations.push({
        category: 'scalability',
        priority: 'high',
        title: 'Scale Compute Capacity',
        description: 'CPU utilization consistently high',
        implementation: 'Increase pod replicas, enable auto-scaling, or upgrade instance types',
        estimatedImpact: '100% throughput increase',
        estimatedCost: 10000,
      });
    }

    // Reliability recommendations
    if (metrics.errors.errorRate > 0.01) {
      recommendations.push({
        category: 'reliability',
        priority: 'critical',
        title: 'Improve Error Handling',
        description: 'Error rate exceeds acceptable threshold',
        implementation: 'Add circuit breakers, implement retry logic with backoff, improve health checks',
        estimatedImpact: '80% error reduction',
        estimatedCost: 3000,
      });
    }

    // Cost optimization
    if (metrics.costs.costPerMillionQueries > 0.50) {
      recommendations.push({
        category: 'cost',
        priority: 'medium',
        title: 'Optimize Infrastructure Costs',
        description: 'Cost per million queries higher than target',
        implementation: 'Use spot instances, implement aggressive caching, optimize resource allocation',
        estimatedImpact: '40% cost reduction',
        estimatedCost: 2000,
      });
    }

    // Regional optimization
    if (bottlenecks.some(b => b.component === 'regional_distribution')) {
      recommendations.push({
        category: 'performance',
        priority: 'medium',
        title: 'Balance Regional Load',
        description: 'Significant latency variance across regions',
        implementation: 'Rebalance traffic with intelligent routing, add capacity to slow regions',
        estimatedImpact: '25% improvement in global latency',
        estimatedCost: 8000,
      });
    }

    return recommendations;
  }

  // Calculate performance score
  calculateScore(metrics: ComprehensiveMetrics, sla: SLACompliance): {
    performance: number;
    reliability: number;
    scalability: number;
    efficiency: number;
    overall: number;
  } {
    // Performance score (based on latency)
    const latencyScore = Math.max(0, 100 - (metrics.latency.p99 / 50) * 100);
    const throughputScore = Math.min(100, (metrics.throughput.queriesPerSecond / 50000000) * 100);
    const performance = (latencyScore + throughputScore) / 2;

    // Reliability score (based on availability and error rate)
    const availabilityScore = metrics.availability.uptime;
    const errorScore = Math.max(0, 100 - metrics.errors.errorRate * 100);
    const reliability = (availabilityScore + errorScore) / 2;

    // Scalability score (based on resource utilization)
    const cpuScore = Math.max(0, 100 - metrics.resources.cpu.average);
    const memoryScore = Math.max(0, 100 - metrics.resources.memory.average);
    const scalability = (cpuScore + memoryScore) / 2;

    // Efficiency score (based on cost)
    const costScore = Math.max(0, 100 - (metrics.costs.costPerMillionQueries / 0.10) * 10);
    const efficiency = costScore;

    // Overall score (weighted average)
    const overall = (
      performance * 0.35 +
      reliability * 0.35 +
      scalability * 0.20 +
      efficiency * 0.10
    );

    return {
      performance: Math.round(performance),
      reliability: Math.round(reliability),
      scalability: Math.round(scalability),
      efficiency: Math.round(efficiency),
      overall: Math.round(overall),
    };
  }

  // Compare two test results
  compare(baseline: ComprehensiveMetrics, current: ComprehensiveMetrics): Comparison {
    const improvements: Record<string, number> = {};
    const regressions: Record<string, number> = {};

    // Latency comparison
    const latencyChange = ((current.latency.p99 - baseline.latency.p99) / baseline.latency.p99) * 100;
    if (latencyChange < 0) {
      improvements['p99_latency'] = Math.abs(latencyChange);
    } else {
      regressions['p99_latency'] = latencyChange;
    }

    // Throughput comparison
    const throughputChange = ((current.throughput.queriesPerSecond - baseline.throughput.queriesPerSecond) / baseline.throughput.queriesPerSecond) * 100;
    if (throughputChange > 0) {
      improvements['throughput'] = throughputChange;
    } else {
      regressions['throughput'] = Math.abs(throughputChange);
    }

    // Error rate comparison
    const errorChange = ((current.errors.errorRate - baseline.errors.errorRate) / baseline.errors.errorRate) * 100;
    if (errorChange < 0) {
      improvements['error_rate'] = Math.abs(errorChange);
    } else {
      regressions['error_rate'] = errorChange;
    }

    // Generate summary
    const improvementCount = Object.keys(improvements).length;
    const regressionCount = Object.keys(regressions).length;

    let summary = '';
    if (improvementCount > regressionCount) {
      summary = `Overall improvement: ${improvementCount} metrics improved, ${regressionCount} regressed`;
    } else if (regressionCount > improvementCount) {
      summary = `Overall regression: ${regressionCount} metrics regressed, ${improvementCount} improved`;
    } else {
      summary = 'Mixed results: equal improvements and regressions';
    }

    return {
      baseline: baseline.scenario,
      current: current.scenario,
      improvements,
      regressions,
      summary,
    };
  }

  // Generate full analysis report
  generateReport(metrics: ComprehensiveMetrics, baseline?: ComprehensiveMetrics): AnalysisReport {
    const statistical = this.analyzeStatistics(metrics);
    const slaCompliance = this.checkSLACompliance(metrics);
    const bottlenecks = this.identifyBottlenecks(metrics);
    const recommendations = this.generateRecommendations(metrics, bottlenecks);
    const score = this.calculateScore(metrics, slaCompliance);
    const comparison = baseline ? this.compare(baseline, metrics) : undefined;

    return {
      testId: metrics.testId,
      scenario: metrics.scenario,
      timestamp: Date.now(),
      statistical,
      slaCompliance,
      bottlenecks,
      recommendations,
      comparison,
      score,
    };
  }

  // Save analysis report
  save(filename: string, report: AnalysisReport): void {
    const filepath = path.join(this.outputDir, filename);
    fs.writeFileSync(filepath, JSON.stringify(report, null, 2));
    console.log(`Analysis report saved to ${filepath}`);
  }

  // Generate markdown report
  generateMarkdown(report: AnalysisReport): string {
    let md = `# Benchmark Analysis Report\n\n`;
    md += `**Test ID:** ${report.testId}\n`;
    md += `**Scenario:** ${report.scenario}\n`;
    md += `**Timestamp:** ${new Date(report.timestamp).toISOString()}\n\n`;

    // Executive Summary
    md += `## Executive Summary\n\n`;
    md += `**Overall Score:** ${report.score.overall}/100\n\n`;
    md += `- Performance: ${report.score.performance}/100\n`;
    md += `- Reliability: ${report.score.reliability}/100\n`;
    md += `- Scalability: ${report.score.scalability}/100\n`;
    md += `- Efficiency: ${report.score.efficiency}/100\n\n`;

    // SLA Compliance
    md += `## SLA Compliance\n\n`;
    md += `**Status:** ${report.slaCompliance.met ? '✅ PASSED' : '❌ FAILED'}\n\n`;
    md += `| Metric | Target | Actual | Status |\n`;
    md += `|--------|--------|--------|--------|\n`;
    md += `| Latency (p99) | <${report.slaCompliance.details.latency.target}ms | ${report.slaCompliance.details.latency.actual.toFixed(2)}ms | ${report.slaCompliance.details.latency.met ? '✅' : '❌'} |\n`;
    md += `| Availability | >${report.slaCompliance.details.availability.target}% | ${report.slaCompliance.details.availability.actual.toFixed(2)}% | ${report.slaCompliance.details.availability.met ? '✅' : '❌'} |\n`;
    md += `| Error Rate | <${report.slaCompliance.details.errorRate.target}% | ${report.slaCompliance.details.errorRate.actual.toFixed(4)}% | ${report.slaCompliance.details.errorRate.met ? '✅' : '❌'} |\n\n`;

    // Bottlenecks
    if (report.bottlenecks.length > 0) {
      md += `## Bottlenecks\n\n`;
      for (const bottleneck of report.bottlenecks) {
        md += `### ${bottleneck.component} - ${bottleneck.metric}\n`;
        md += `**Severity:** ${bottleneck.severity.toUpperCase()}\n`;
        md += `**Current Value:** ${bottleneck.currentValue}\n`;
        md += `**Threshold:** ${bottleneck.threshold}\n`;
        md += `**Impact:** ${bottleneck.impact}\n`;
        md += `**Recommendation:** ${bottleneck.recommendation}\n\n`;
      }
    }

    // Recommendations
    if (report.recommendations.length > 0) {
      md += `## Recommendations\n\n`;
      for (const rec of report.recommendations) {
        md += `### ${rec.title}\n`;
        md += `**Priority:** ${rec.priority.toUpperCase()} | **Category:** ${rec.category}\n`;
        md += `**Description:** ${rec.description}\n`;
        md += `**Implementation:** ${rec.implementation}\n`;
        md += `**Estimated Impact:** ${rec.estimatedImpact}\n`;
        md += `**Estimated Cost:** $${rec.estimatedCost}\n\n`;
      }
    }

    // Comparison
    if (report.comparison) {
      md += `## Comparison vs Baseline\n\n`;
      md += `**Baseline:** ${report.comparison.baseline}\n`;
      md += `**Current:** ${report.comparison.current}\n\n`;
      md += `**Summary:** ${report.comparison.summary}\n\n`;

      if (Object.keys(report.comparison.improvements).length > 0) {
        md += `### Improvements\n`;
        for (const [metric, change] of Object.entries(report.comparison.improvements)) {
          md += `- ${metric}: +${change.toFixed(2)}%\n`;
        }
        md += `\n`;
      }

      if (Object.keys(report.comparison.regressions).length > 0) {
        md += `### Regressions\n`;
        for (const [metric, change] of Object.entries(report.comparison.regressions)) {
          md += `- ${metric}: -${change.toFixed(2)}%\n`;
        }
        md += `\n`;
      }
    }

    return md;
  }
}

export default ResultsAnalyzer;
