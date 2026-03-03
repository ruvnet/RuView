/**
 * Metrics Collection and Monitoring
 * Prometheus-compatible metrics for AIMDS gateway
 */

import { Counter, Histogram, Gauge, register, collectDefaultMetrics } from 'prom-client';
import { DefenseResult, MetricsSnapshot, ThreatLevel } from '../types';
import { Logger } from '../utils/logger';

export class MetricsCollector {
  private logger: Logger;

  // Counters
  private requestsTotal: Counter;
  private requestsAllowed: Counter;
  private requestsBlocked: Counter;
  private requestsErrored: Counter;
  private threatsDetected: Counter;
  private falsePositives: Counter;

  // Histograms
  private detectionLatency: Histogram;
  private vectorSearchLatency: Histogram;
  private verificationLatency: Histogram;

  // Gauges
  private activeRequests: Gauge;
  private threatLevel: Gauge;
  private cacheHitRate: Gauge;

  // In-memory stats for snapshots
  private stats: {
    requests: number;
    allowed: number;
    blocked: number;
    errored: number;
    latencies: number[];
    threats: Map<ThreatLevel, number>;
    falsePositives: number;
    falseNegatives: number;
  };

  constructor(logger: Logger) {
    this.logger = logger;

    // Initialize counters
    this.requestsTotal = new Counter({
      name: 'aimds_requests_total',
      help: 'Total number of defense requests processed',
      labelNames: ['path']
    });

    this.requestsAllowed = new Counter({
      name: 'aimds_requests_allowed_total',
      help: 'Total number of requests allowed'
    });

    this.requestsBlocked = new Counter({
      name: 'aimds_requests_blocked_total',
      help: 'Total number of requests blocked'
    });

    this.requestsErrored = new Counter({
      name: 'aimds_requests_errored_total',
      help: 'Total number of requests that errored'
    });

    this.threatsDetected = new Counter({
      name: 'aimds_threats_detected_total',
      help: 'Total number of threats detected',
      labelNames: ['level']
    });

    this.falsePositives = new Counter({
      name: 'aimds_false_positives_total',
      help: 'Total number of false positives'
    });

    // Initialize histograms
    this.detectionLatency = new Histogram({
      name: 'aimds_detection_latency_ms',
      help: 'Detection latency in milliseconds',
      labelNames: ['path'],
      buckets: [1, 2, 5, 10, 20, 35, 50, 100, 200, 500, 1000, 5000]
    });

    this.vectorSearchLatency = new Histogram({
      name: 'aimds_vector_search_latency_ms',
      help: 'Vector search latency in milliseconds',
      buckets: [0.5, 1, 2, 5, 10, 20, 50]
    });

    this.verificationLatency = new Histogram({
      name: 'aimds_verification_latency_ms',
      help: 'Formal verification latency in milliseconds',
      buckets: [1, 5, 10, 50, 100, 500, 1000, 5000]
    });

    // Initialize gauges
    this.activeRequests = new Gauge({
      name: 'aimds_active_requests',
      help: 'Number of currently active requests'
    });

    this.threatLevel = new Gauge({
      name: 'aimds_current_threat_level',
      help: 'Current system threat level (0-4)',
      labelNames: ['level']
    });

    this.cacheHitRate = new Gauge({
      name: 'aimds_cache_hit_rate',
      help: 'Cache hit rate (0-1)'
    });

    // Initialize stats
    this.stats = {
      requests: 0,
      allowed: 0,
      blocked: 0,
      errored: 0,
      latencies: [],
      threats: new Map(),
      falsePositives: 0,
      falseNegatives: 0
    };
  }

  /**
   * Initialize metrics collection
   */
  async initialize(): Promise<void> {
    // Enable default Node.js metrics
    collectDefaultMetrics({ register });

    this.logger.info('Metrics collector initialized');
  }

  /**
   * Record a detection event
   */
  recordDetection(latencyMs: number, result: DefenseResult): void {
    // Increment counters
    this.requestsTotal.inc();

    if (result.allowed) {
      this.requestsAllowed.inc();
      this.stats.allowed++;
    } else {
      this.requestsBlocked.inc();
      this.stats.blocked++;
    }

    // Record threat detection
    if (result.threatLevel > ThreatLevel.NONE) {
      this.threatsDetected.inc({ level: ThreatLevel[result.threatLevel] });

      const current = this.stats.threats.get(result.threatLevel) || 0;
      this.stats.threats.set(result.threatLevel, current + 1);
    }

    // Record latencies
    this.detectionLatency.observe({ path: result.metadata.pathTaken }, latencyMs);
    this.vectorSearchLatency.observe(result.metadata.vectorSearchTime);

    if (result.metadata.verificationTime > 0) {
      this.verificationLatency.observe(result.metadata.verificationTime);
    }

    // Update stats
    this.stats.requests++;
    this.stats.latencies.push(latencyMs);

    // Keep only last 10000 latencies for percentile calculation
    if (this.stats.latencies.length > 10000) {
      this.stats.latencies = this.stats.latencies.slice(-10000);
    }
  }

  /**
   * Record an error
   */
  recordError(): void {
    this.requestsErrored.inc();
    this.stats.errored++;
  }

  /**
   * Record a false positive
   */
  recordFalsePositive(): void {
    this.falsePositives.inc();
    this.stats.falsePositives++;
  }

  /**
   * Update active requests gauge
   */
  updateActiveRequests(count: number): void {
    this.activeRequests.set(count);
  }

  /**
   * Update threat level gauge
   */
  updateThreatLevel(level: ThreatLevel): void {
    this.threatLevel.set({ level: ThreatLevel[level] }, level);
  }

  /**
   * Update cache hit rate
   */
  updateCacheHitRate(rate: number): void {
    this.cacheHitRate.set(rate);
  }

  /**
   * Get current metrics snapshot
   */
  async getSnapshot(): Promise<MetricsSnapshot> {
    const latencies = [...this.stats.latencies].sort((a, b) => a - b);

    return {
      timestamp: Date.now(),
      requests: {
        total: this.stats.requests,
        allowed: this.stats.allowed,
        blocked: this.stats.blocked,
        errored: this.stats.errored
      },
      latency: {
        p50: this.percentile(latencies, 0.5),
        p95: this.percentile(latencies, 0.95),
        p99: this.percentile(latencies, 0.99),
        avg: latencies.length > 0
          ? latencies.reduce((a, b) => a + b, 0) / latencies.length
          : 0,
        max: latencies.length > 0 ? Math.max(...latencies) : 0
      },
      threats: {
        byLevel: {
          [ThreatLevel.NONE]: this.stats.threats.get(ThreatLevel.NONE) || 0,
          [ThreatLevel.LOW]: this.stats.threats.get(ThreatLevel.LOW) || 0,
          [ThreatLevel.MEDIUM]: this.stats.threats.get(ThreatLevel.MEDIUM) || 0,
          [ThreatLevel.HIGH]: this.stats.threats.get(ThreatLevel.HIGH) || 0,
          [ThreatLevel.CRITICAL]: this.stats.threats.get(ThreatLevel.CRITICAL) || 0
        },
        falsePositives: this.stats.falsePositives,
        falseNegatives: this.stats.falseNegatives
      },
      agentdb: {
        vectorSearchAvg: 0, // Updated externally
        syncLatency: 0,     // Updated externally
        memoryUsage: 0      // Updated externally
      },
      verification: {
        proofsGenerated: 0,  // Updated externally
        avgProofTime: 0,     // Updated externally
        cacheHitRate: 0      // Updated externally
      }
    };
  }

  /**
   * Export Prometheus metrics
   */
  async exportPrometheus(): Promise<string> {
    return register.metrics();
  }

  /**
   * Reset all metrics
   */
  reset(): void {
    register.resetMetrics();
    this.stats = {
      requests: 0,
      allowed: 0,
      blocked: 0,
      errored: 0,
      latencies: [],
      threats: new Map(),
      falsePositives: 0,
      falseNegatives: 0
    };
  }

  /**
   * Shutdown metrics collector
   */
  async shutdown(): Promise<void> {
    register.clear();
    this.logger.info('Metrics collector shutdown complete');
  }

  // ============================================================================
  // Private Helper Methods
  // ============================================================================

  private percentile(sorted: number[], p: number): number {
    if (sorted.length === 0) return 0;
    const index = Math.ceil(sorted.length * p) - 1;
    return sorted[Math.max(0, index)];
  }
}
