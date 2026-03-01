/**
 * Metrics Collector for RuVector Benchmarks
 *
 * Collects, aggregates, and stores comprehensive performance metrics
 */

import * as fs from 'fs';
import * as path from 'path';

// Metric types
export interface LatencyMetrics {
  min: number;
  max: number;
  mean: number;
  median: number;
  p50: number;
  p90: number;
  p95: number;
  p99: number;
  p99_9: number;
  stddev: number;
}

export interface ThroughputMetrics {
  queriesPerSecond: number;
  bytesPerSecond: number;
  connectionsPerSecond: number;
  peakQPS: number;
  averageQPS: number;
}

export interface ErrorMetrics {
  totalErrors: number;
  errorRate: number;
  errorsByType: Record<string, number>;
  errorsByRegion: Record<string, number>;
  timeouts: number;
  connectionErrors: number;
  serverErrors: number;
  clientErrors: number;
}

export interface ResourceMetrics {
  cpu: {
    average: number;
    peak: number;
    perRegion: Record<string, number>;
  };
  memory: {
    average: number;
    peak: number;
    perRegion: Record<string, number>;
  };
  network: {
    ingressBytes: number;
    egressBytes: number;
    bandwidth: number;
    perRegion: Record<string, number>;
  };
  disk: {
    reads: number;
    writes: number;
    iops: number;
  };
}

export interface CostMetrics {
  computeCost: number;
  networkCost: number;
  storageCost: number;
  totalCost: number;
  costPerMillionQueries: number;
  costPerRegion: Record<string, number>;
}

export interface ScalingMetrics {
  timeToTarget: number; // milliseconds to reach target capacity
  scaleUpRate: number; // connections/second
  scaleDownRate: number; // connections/second
  autoScaleEvents: number;
  coldStartLatency: number;
}

export interface AvailabilityMetrics {
  uptime: number; // percentage
  downtime: number; // milliseconds
  mtbf: number; // mean time between failures
  mttr: number; // mean time to recovery
  incidents: Array<{
    timestamp: number;
    duration: number;
    impact: string;
    region?: string;
  }>;
}

export interface RegionalMetrics {
  region: string;
  latency: LatencyMetrics;
  throughput: ThroughputMetrics;
  errors: ErrorMetrics;
  activeConnections: number;
  availability: number;
}

export interface ComprehensiveMetrics {
  testId: string;
  scenario: string;
  startTime: number;
  endTime: number;
  duration: number;
  latency: LatencyMetrics;
  throughput: ThroughputMetrics;
  errors: ErrorMetrics;
  resources: ResourceMetrics;
  costs: CostMetrics;
  scaling: ScalingMetrics;
  availability: AvailabilityMetrics;
  regional: RegionalMetrics[];
  slaCompliance: {
    latencySLA: boolean; // p99 < 50ms
    availabilitySLA: boolean; // 99.99%
    errorRateSLA: boolean; // < 0.01%
  };
  tags: string[];
  metadata: Record<string, any>;
}

// Time series data point
export interface DataPoint {
  timestamp: number;
  value: number;
  tags?: Record<string, string>;
}

export interface TimeSeries {
  metric: string;
  dataPoints: DataPoint[];
}

// Metrics collector class
export class MetricsCollector {
  private metrics: Map<string, TimeSeries>;
  private startTime: number;
  private outputDir: string;

  constructor(outputDir: string = './results') {
    this.metrics = new Map();
    this.startTime = Date.now();
    this.outputDir = outputDir;

    // Ensure output directory exists
    if (!fs.existsSync(outputDir)) {
      fs.mkdirSync(outputDir, { recursive: true });
    }
  }

  // Record a single metric
  record(metric: string, value: number, tags?: Record<string, string>): void {
    if (!this.metrics.has(metric)) {
      this.metrics.set(metric, {
        metric,
        dataPoints: [],
      });
    }

    this.metrics.get(metric)!.dataPoints.push({
      timestamp: Date.now(),
      value,
      tags,
    });
  }

  // Record latency
  recordLatency(latency: number, region?: string): void {
    this.record('latency', latency, { region: region || 'unknown' });
  }

  // Record throughput
  recordThroughput(qps: number, region?: string): void {
    this.record('throughput', qps, { region: region || 'unknown' });
  }

  // Record error
  recordError(errorType: string, region?: string): void {
    this.record('errors', 1, { type: errorType, region: region || 'unknown' });
  }

  // Record resource usage
  recordResource(resource: string, usage: number, region?: string): void {
    this.record(`resource_${resource}`, usage, { region: region || 'unknown' });
  }

  // Calculate latency metrics from raw data
  calculateLatencyMetrics(data: number[]): LatencyMetrics {
    const sorted = [...data].sort((a, b) => a - b);
    const len = sorted.length;

    const percentile = (p: number) => {
      const index = Math.ceil(len * p) - 1;
      return sorted[Math.max(0, index)];
    };

    const mean = data.reduce((a, b) => a + b, 0) / len;
    const variance = data.reduce((a, b) => a + Math.pow(b - mean, 2), 0) / len;
    const stddev = Math.sqrt(variance);

    return {
      min: sorted[0],
      max: sorted[len - 1],
      mean,
      median: percentile(0.5),
      p50: percentile(0.5),
      p90: percentile(0.9),
      p95: percentile(0.95),
      p99: percentile(0.99),
      p99_9: percentile(0.999),
      stddev,
    };
  }

  // Calculate throughput metrics
  calculateThroughputMetrics(): ThroughputMetrics {
    const throughputSeries = this.metrics.get('throughput');
    if (!throughputSeries || throughputSeries.dataPoints.length === 0) {
      return {
        queriesPerSecond: 0,
        bytesPerSecond: 0,
        connectionsPerSecond: 0,
        peakQPS: 0,
        averageQPS: 0,
      };
    }

    const qpsValues = throughputSeries.dataPoints.map(dp => dp.value);
    const totalQueries = qpsValues.reduce((a, b) => a + b, 0);
    const duration = (Date.now() - this.startTime) / 1000; // seconds

    return {
      queriesPerSecond: totalQueries / duration,
      bytesPerSecond: 0, // TODO: Calculate from data
      connectionsPerSecond: 0, // TODO: Calculate from data
      peakQPS: Math.max(...qpsValues),
      averageQPS: totalQueries / qpsValues.length,
    };
  }

  // Calculate error metrics
  calculateErrorMetrics(): ErrorMetrics {
    const errorSeries = this.metrics.get('errors');
    if (!errorSeries || errorSeries.dataPoints.length === 0) {
      return {
        totalErrors: 0,
        errorRate: 0,
        errorsByType: {},
        errorsByRegion: {},
        timeouts: 0,
        connectionErrors: 0,
        serverErrors: 0,
        clientErrors: 0,
      };
    }

    const errorsByType: Record<string, number> = {};
    const errorsByRegion: Record<string, number> = {};

    for (const dp of errorSeries.dataPoints) {
      const type = dp.tags?.type || 'unknown';
      const region = dp.tags?.region || 'unknown';

      errorsByType[type] = (errorsByType[type] || 0) + 1;
      errorsByRegion[region] = (errorsByRegion[region] || 0) + 1;
    }

    const totalErrors = errorSeries.dataPoints.length;
    const totalRequests = this.getTotalRequests();

    return {
      totalErrors,
      errorRate: totalRequests > 0 ? (totalErrors / totalRequests) * 100 : 0,
      errorsByType,
      errorsByRegion,
      timeouts: errorsByType['timeout'] || 0,
      connectionErrors: errorsByType['connection'] || 0,
      serverErrors: errorsByType['server'] || 0,
      clientErrors: errorsByType['client'] || 0,
    };
  }

  // Calculate resource metrics
  calculateResourceMetrics(): ResourceMetrics {
    const cpuSeries = this.metrics.get('resource_cpu');
    const memorySeries = this.metrics.get('resource_memory');
    const networkSeries = this.metrics.get('resource_network');

    const cpu = {
      average: this.average(cpuSeries?.dataPoints.map(dp => dp.value) || []),
      peak: Math.max(...(cpuSeries?.dataPoints.map(dp => dp.value) || [0])),
      perRegion: this.aggregateByRegion(cpuSeries),
    };

    const memory = {
      average: this.average(memorySeries?.dataPoints.map(dp => dp.value) || []),
      peak: Math.max(...(memorySeries?.dataPoints.map(dp => dp.value) || [0])),
      perRegion: this.aggregateByRegion(memorySeries),
    };

    const network = {
      ingressBytes: 0, // TODO: Calculate
      egressBytes: 0, // TODO: Calculate
      bandwidth: 0, // TODO: Calculate
      perRegion: this.aggregateByRegion(networkSeries),
    };

    return {
      cpu,
      memory,
      network,
      disk: {
        reads: 0,
        writes: 0,
        iops: 0,
      },
    };
  }

  // Calculate cost metrics
  calculateCostMetrics(duration: number): CostMetrics {
    const resources = this.calculateResourceMetrics();
    const throughput = this.calculateThroughputMetrics();

    // GCP pricing estimates (as of 2024)
    const computeCostPerHour = 0.50; // per vCPU-hour
    const networkCostPerGB = 0.12;
    const storageCostPerGB = 0.02;

    const durationHours = duration / (1000 * 60 * 60);

    const computeCost = resources.cpu.average * computeCostPerHour * durationHours;
    const networkCost = (resources.network.ingressBytes + resources.network.egressBytes) / (1024 * 1024 * 1024) * networkCostPerGB;
    const storageCost = 0; // TODO: Calculate based on storage usage

    const totalCost = computeCost + networkCost + storageCost;
    const totalQueries = throughput.queriesPerSecond * (duration / 1000);
    const costPerMillionQueries = (totalCost / totalQueries) * 1000000;

    return {
      computeCost,
      networkCost,
      storageCost,
      totalCost,
      costPerMillionQueries,
      costPerRegion: {}, // TODO: Calculate per-region costs
    };
  }

  // Calculate scaling metrics
  calculateScalingMetrics(): ScalingMetrics {
    // TODO: Implement based on collected scaling events
    return {
      timeToTarget: 0,
      scaleUpRate: 0,
      scaleDownRate: 0,
      autoScaleEvents: 0,
      coldStartLatency: 0,
    };
  }

  // Calculate availability metrics
  calculateAvailabilityMetrics(duration: number): AvailabilityMetrics {
    const errors = this.calculateErrorMetrics();
    const downtime = 0; // TODO: Calculate from incident data

    return {
      uptime: ((duration - downtime) / duration) * 100,
      downtime,
      mtbf: 0, // TODO: Calculate
      mttr: 0, // TODO: Calculate
      incidents: [], // TODO: Collect incidents
    };
  }

  // Calculate regional metrics
  calculateRegionalMetrics(): RegionalMetrics[] {
    const regions = this.getRegions();
    const metrics: RegionalMetrics[] = [];

    for (const region of regions) {
      const latencyData = this.getMetricsByRegion('latency', region);
      const throughputData = this.getMetricsByRegion('throughput', region);
      const errorData = this.getMetricsByRegion('errors', region);

      metrics.push({
        region,
        latency: this.calculateLatencyMetrics(latencyData),
        throughput: {
          queriesPerSecond: this.average(throughputData),
          bytesPerSecond: 0,
          connectionsPerSecond: 0,
          peakQPS: Math.max(...throughputData, 0),
          averageQPS: this.average(throughputData),
        },
        errors: {
          totalErrors: errorData.length,
          errorRate: 0, // TODO: Calculate
          errorsByType: {},
          errorsByRegion: {},
          timeouts: 0,
          connectionErrors: 0,
          serverErrors: 0,
          clientErrors: 0,
        },
        activeConnections: 0, // TODO: Track
        availability: 99.99, // TODO: Calculate
      });
    }

    return metrics;
  }

  // Generate comprehensive metrics report
  generateReport(testId: string, scenario: string): ComprehensiveMetrics {
    const endTime = Date.now();
    const duration = endTime - this.startTime;

    const latencySeries = this.metrics.get('latency');
    const latencyData = latencySeries?.dataPoints.map(dp => dp.value) || [];

    const latency = this.calculateLatencyMetrics(latencyData);
    const throughput = this.calculateThroughputMetrics();
    const errors = this.calculateErrorMetrics();
    const resources = this.calculateResourceMetrics();
    const costs = this.calculateCostMetrics(duration);
    const scaling = this.calculateScalingMetrics();
    const availability = this.calculateAvailabilityMetrics(duration);
    const regional = this.calculateRegionalMetrics();

    const slaCompliance = {
      latencySLA: latency.p99 < 50,
      availabilitySLA: availability.uptime >= 99.99,
      errorRateSLA: errors.errorRate < 0.01,
    };

    return {
      testId,
      scenario,
      startTime: this.startTime,
      endTime,
      duration,
      latency,
      throughput,
      errors,
      resources,
      costs,
      scaling,
      availability,
      regional,
      slaCompliance,
      tags: [],
      metadata: {},
    };
  }

  // Save metrics to file
  save(filename: string, metrics: ComprehensiveMetrics): void {
    const filepath = path.join(this.outputDir, filename);
    fs.writeFileSync(filepath, JSON.stringify(metrics, null, 2));
    console.log(`Metrics saved to ${filepath}`);
  }

  // Export to CSV
  exportCSV(filename: string): void {
    const filepath = path.join(this.outputDir, filename);
    const headers = ['timestamp', 'metric', 'value', 'region'];
    const rows = [headers.join(',')];

    for (const [metric, series] of this.metrics) {
      for (const dp of series.dataPoints) {
        const row = [
          dp.timestamp,
          metric,
          dp.value,
          dp.tags?.region || 'unknown',
        ];
        rows.push(row.join(','));
      }
    }

    fs.writeFileSync(filepath, rows.join('\n'));
    console.log(`CSV exported to ${filepath}`);
  }

  // Helper methods
  private getTotalRequests(): number {
    const throughputSeries = this.metrics.get('throughput');
    if (!throughputSeries) return 0;
    return throughputSeries.dataPoints.reduce((sum, dp) => sum + dp.value, 0);
  }

  private average(values: number[]): number {
    if (values.length === 0) return 0;
    return values.reduce((a, b) => a + b, 0) / values.length;
  }

  private aggregateByRegion(series?: TimeSeries): Record<string, number> {
    const result: Record<string, number> = {};
    if (!series) return result;

    for (const dp of series.dataPoints) {
      const region = dp.tags?.region || 'unknown';
      if (!result[region]) result[region] = 0;
      result[region] += dp.value;
    }

    return result;
  }

  private getRegions(): string[] {
    const regions = new Set<string>();

    for (const series of this.metrics.values()) {
      for (const dp of series.dataPoints) {
        if (dp.tags?.region) {
          regions.add(dp.tags.region);
        }
      }
    }

    return Array.from(regions);
  }

  private getMetricsByRegion(metric: string, region: string): number[] {
    const series = this.metrics.get(metric);
    if (!series) return [];

    return series.dataPoints
      .filter(dp => dp.tags?.region === region)
      .map(dp => dp.value);
  }
}

// K6 integration - collect metrics from K6 output
export function collectFromK6Output(outputFile: string): MetricsCollector {
  const collector = new MetricsCollector();

  try {
    const data = fs.readFileSync(outputFile, 'utf-8');
    const lines = data.split('\n');

    for (const line of lines) {
      if (!line.trim()) continue;

      try {
        const metric = JSON.parse(line);

        switch (metric.type) {
          case 'Point':
            collector.record(metric.metric, metric.data.value, metric.data.tags);
            break;
          case 'Metric':
            // Handle metric definitions
            break;
        }
      } catch (e) {
        // Skip invalid lines
      }
    }
  } catch (e) {
    console.error('Error reading K6 output:', e);
  }

  return collector;
}

export default MetricsCollector;
