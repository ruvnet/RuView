/**
 * Reactive Scaler - Real-time Auto-scaling
 *
 * Handles reactive scaling based on:
 * - Real-time metrics (CPU, memory, connections)
 * - Dynamic threshold adjustment
 * - Rapid scale-out (seconds)
 * - Gradual scale-in to avoid thrashing
 */

import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export interface ScalingMetrics {
  region: string;
  timestamp: Date;
  cpuUtilization: number; // 0-1
  memoryUtilization: number; // 0-1
  activeConnections: number;
  requestRate: number; // requests per second
  errorRate: number; // 0-1
  p99Latency: number; // milliseconds
  currentInstances: number;
}

export interface ScalingThresholds {
  cpuScaleOut: number; // Scale out when CPU > this (e.g., 0.7)
  cpuScaleIn: number; // Scale in when CPU < this (e.g., 0.3)
  memoryScaleOut: number;
  memoryScaleIn: number;
  connectionsPerInstance: number;
  maxP99Latency: number; // milliseconds
  errorRateThreshold: number;
}

export interface ScalingAction {
  region: string;
  action: 'scale-out' | 'scale-in' | 'none';
  fromInstances: number;
  toInstances: number;
  reason: string;
  urgency: 'critical' | 'high' | 'normal' | 'low';
  timestamp: Date;
}

export interface ScalingConfig {
  minInstances: number;
  maxInstances: number;
  scaleOutCooldown: number; // seconds
  scaleInCooldown: number; // seconds
  scaleOutStep: number; // number of instances to add
  scaleInStep: number; // number of instances to remove
  rapidScaleOutThreshold: number; // When to do rapid scaling
}

export class ReactiveScaler {
  private thresholds: ScalingThresholds;
  private config: ScalingConfig;
  private lastScaleTime: Map<string, Date> = new Map();
  private metricsHistory: Map<string, ScalingMetrics[]> = new Map();
  private readonly historySize = 60; // Keep 60 samples (5 minutes at 5s intervals)

  constructor(
    private readonly regions: string[] = ['us-central1', 'europe-west1', 'asia-east1'],
    private readonly notifyHook: (message: string) => Promise<void> = async (msg) => {
      await execAsync(`npx claude-flow@alpha hooks notify --message "${msg.replace(/"/g, '\\"')}"`);
    }
  ) {
    // Default thresholds
    this.thresholds = {
      cpuScaleOut: 0.70, // Scale out at 70% CPU
      cpuScaleIn: 0.30, // Scale in at 30% CPU
      memoryScaleOut: 0.75,
      memoryScaleIn: 0.35,
      connectionsPerInstance: 500_000,
      maxP99Latency: 50, // 50ms p99 latency
      errorRateThreshold: 0.01 // 1% error rate
    };

    // Default config
    this.config = {
      minInstances: 10,
      maxInstances: 1000,
      scaleOutCooldown: 60, // 1 minute
      scaleInCooldown: 300, // 5 minutes
      scaleOutStep: 10, // Add 10 instances at a time
      scaleInStep: 2, // Remove 2 instances at a time
      rapidScaleOutThreshold: 0.90 // Rapid scale at 90% utilization
    };
  }

  /**
   * Update scaling thresholds
   */
  updateThresholds(thresholds: Partial<ScalingThresholds>): void {
    this.thresholds = { ...this.thresholds, ...thresholds };
  }

  /**
   * Update scaling configuration
   */
  updateConfig(config: Partial<ScalingConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Process metrics and determine scaling action
   */
  async processMetrics(metrics: ScalingMetrics): Promise<ScalingAction> {
    // Store metrics in history
    this.addMetricsToHistory(metrics);

    // Check if we're in cooldown period
    const lastScale = this.lastScaleTime.get(metrics.region);
    const now = new Date();

    if (lastScale) {
      const timeSinceLastScale = (now.getTime() - lastScale.getTime()) / 1000;
      const cooldown = this.config.scaleOutCooldown;

      if (timeSinceLastScale < cooldown) {
        // Still in cooldown, no action
        return this.createNoAction(metrics, `In cooldown (${Math.round(cooldown - timeSinceLastScale)}s remaining)`);
      }
    }

    // Determine if scaling is needed
    const action = await this.determineScalingAction(metrics);

    if (action.action !== 'none') {
      this.lastScaleTime.set(metrics.region, now);
      await this.notifyHook(
        `SCALING: ${action.region} ${action.action} ${action.fromInstances} -> ${action.toInstances} (${action.reason})`
      );
    }

    return action;
  }

  /**
   * Determine what scaling action to take based on metrics
   */
  private async determineScalingAction(metrics: ScalingMetrics): Promise<ScalingAction> {
    const reasons: string[] = [];
    let shouldScaleOut = false;
    let shouldScaleIn = false;
    let urgency: 'critical' | 'high' | 'normal' | 'low' = 'normal';

    // Check CPU utilization
    if (metrics.cpuUtilization > this.thresholds.cpuScaleOut) {
      reasons.push(`CPU ${(metrics.cpuUtilization * 100).toFixed(1)}%`);
      shouldScaleOut = true;

      if (metrics.cpuUtilization > this.config.rapidScaleOutThreshold) {
        urgency = 'critical';
      } else if (metrics.cpuUtilization > 0.8) {
        urgency = 'high';
      }
    } else if (metrics.cpuUtilization < this.thresholds.cpuScaleIn) {
      if (this.isStableForScaleIn(metrics.region, 'cpu')) {
        shouldScaleIn = true;
      }
    }

    // Check memory utilization
    if (metrics.memoryUtilization > this.thresholds.memoryScaleOut) {
      reasons.push(`Memory ${(metrics.memoryUtilization * 100).toFixed(1)}%`);
      shouldScaleOut = true;
      urgency = urgency === 'critical' ? 'critical' : 'high';
    } else if (metrics.memoryUtilization < this.thresholds.memoryScaleIn) {
      if (this.isStableForScaleIn(metrics.region, 'memory')) {
        shouldScaleIn = true;
      }
    }

    // Check connection count
    const connectionsPerInstance = metrics.activeConnections / metrics.currentInstances;
    if (connectionsPerInstance > this.thresholds.connectionsPerInstance * 0.8) {
      reasons.push(`Connections ${Math.round(connectionsPerInstance)}/instance`);
      shouldScaleOut = true;

      if (connectionsPerInstance > this.thresholds.connectionsPerInstance) {
        urgency = 'critical';
      }
    }

    // Check latency
    if (metrics.p99Latency > this.thresholds.maxP99Latency) {
      reasons.push(`P99 latency ${metrics.p99Latency}ms`);
      shouldScaleOut = true;

      if (metrics.p99Latency > this.thresholds.maxP99Latency * 2) {
        urgency = 'critical';
      } else {
        urgency = 'high';
      }
    }

    // Check error rate
    if (metrics.errorRate > this.thresholds.errorRateThreshold) {
      reasons.push(`Error rate ${(metrics.errorRate * 100).toFixed(2)}%`);
      shouldScaleOut = true;
      urgency = 'high';
    }

    // Determine action
    if (shouldScaleOut && !shouldScaleIn) {
      return this.createScaleOutAction(metrics, reasons.join(', '), urgency);
    } else if (shouldScaleIn && !shouldScaleOut) {
      return this.createScaleInAction(metrics, 'Low utilization');
    } else {
      return this.createNoAction(metrics, 'Within thresholds');
    }
  }

  /**
   * Create scale-out action
   */
  private createScaleOutAction(
    metrics: ScalingMetrics,
    reason: string,
    urgency: 'critical' | 'high' | 'normal' | 'low'
  ): ScalingAction {
    const fromInstances = metrics.currentInstances;

    // Calculate how many instances to add
    let step = this.config.scaleOutStep;

    // Rapid scaling for critical situations
    if (urgency === 'critical') {
      step = Math.ceil(fromInstances * 0.5); // Add 50% capacity
    } else if (urgency === 'high') {
      step = Math.ceil(fromInstances * 0.3); // Add 30% capacity
    }

    const toInstances = Math.min(fromInstances + step, this.config.maxInstances);

    return {
      region: metrics.region,
      action: 'scale-out',
      fromInstances,
      toInstances,
      reason,
      urgency,
      timestamp: new Date()
    };
  }

  /**
   * Create scale-in action
   */
  private createScaleInAction(metrics: ScalingMetrics, reason: string): ScalingAction {
    const fromInstances = metrics.currentInstances;
    const toInstances = Math.max(
      fromInstances - this.config.scaleInStep,
      this.config.minInstances
    );

    return {
      region: metrics.region,
      action: 'scale-in',
      fromInstances,
      toInstances,
      reason,
      urgency: 'low',
      timestamp: new Date()
    };
  }

  /**
   * Create no-action result
   */
  private createNoAction(metrics: ScalingMetrics, reason: string): ScalingAction {
    return {
      region: metrics.region,
      action: 'none',
      fromInstances: metrics.currentInstances,
      toInstances: metrics.currentInstances,
      reason,
      urgency: 'low',
      timestamp: new Date()
    };
  }

  /**
   * Check if metrics have been stable enough for scale-in
   */
  private isStableForScaleIn(region: string, metric: 'cpu' | 'memory'): boolean {
    const history = this.metricsHistory.get(region);

    if (!history || history.length < 10) {
      return false; // Need at least 10 samples
    }

    // Check last 10 samples
    const recentSamples = history.slice(-10);

    for (const sample of recentSamples) {
      const value = metric === 'cpu' ? sample.cpuUtilization : sample.memoryUtilization;
      const threshold = metric === 'cpu' ? this.thresholds.cpuScaleIn : this.thresholds.memoryScaleIn;

      if (value > threshold) {
        return false; // Not stable
      }
    }

    return true; // Stable for scale-in
  }

  /**
   * Add metrics to history
   */
  private addMetricsToHistory(metrics: ScalingMetrics): void {
    let history = this.metricsHistory.get(metrics.region);

    if (!history) {
      history = [];
      this.metricsHistory.set(metrics.region, history);
    }

    history.push(metrics);

    // Keep only recent history
    if (history.length > this.historySize) {
      history.shift();
    }
  }

  /**
   * Get current metrics summary for all regions
   */
  getMetricsSummary(): Map<string, {
    avgCpu: number;
    avgMemory: number;
    avgLatency: number;
    totalConnections: number;
    instances: number;
  }> {
    const summary = new Map();

    for (const [region, history] of this.metricsHistory) {
      if (history.length === 0) continue;

      const recent = history.slice(-5); // Last 5 samples
      const avgCpu = recent.reduce((sum, m) => sum + m.cpuUtilization, 0) / recent.length;
      const avgMemory = recent.reduce((sum, m) => sum + m.memoryUtilization, 0) / recent.length;
      const avgLatency = recent.reduce((sum, m) => sum + m.p99Latency, 0) / recent.length;
      const latest = recent[recent.length - 1];

      summary.set(region, {
        avgCpu,
        avgMemory,
        avgLatency,
        totalConnections: latest.activeConnections,
        instances: latest.currentInstances
      });
    }

    return summary;
  }

  /**
   * Calculate recommended instances based on current load
   */
  calculateRecommendedInstances(metrics: ScalingMetrics): number {
    // Calculate based on connections
    const connectionBased = Math.ceil(
      metrics.activeConnections / this.thresholds.connectionsPerInstance
    );

    // Calculate based on CPU (target 60% utilization)
    const cpuBased = Math.ceil(
      (metrics.currentInstances * metrics.cpuUtilization) / 0.6
    );

    // Calculate based on memory (target 65% utilization)
    const memoryBased = Math.ceil(
      (metrics.currentInstances * metrics.memoryUtilization) / 0.65
    );

    // Take the maximum to ensure we have enough capacity
    const recommended = Math.max(connectionBased, cpuBased, memoryBased);

    // Apply min/max constraints
    return Math.max(
      this.config.minInstances,
      Math.min(recommended, this.config.maxInstances)
    );
  }

  /**
   * Get scaling recommendation for predictive scaling integration
   */
  async getScalingRecommendation(region: string): Promise<{
    currentInstances: number;
    recommendedInstances: number;
    reasoning: string[];
  }> {
    const history = this.metricsHistory.get(region);

    if (!history || history.length === 0) {
      return {
        currentInstances: this.config.minInstances,
        recommendedInstances: this.config.minInstances,
        reasoning: ['No metrics available']
      };
    }

    const latest = history[history.length - 1];
    const recommended = this.calculateRecommendedInstances(latest);
    const reasoning: string[] = [];

    if (recommended > latest.currentInstances) {
      reasoning.push(`Current load requires ${recommended} instances`);
      reasoning.push(`CPU: ${(latest.cpuUtilization * 100).toFixed(1)}%`);
      reasoning.push(`Memory: ${(latest.memoryUtilization * 100).toFixed(1)}%`);
      reasoning.push(`Connections: ${latest.activeConnections.toLocaleString()}`);
    } else if (recommended < latest.currentInstances) {
      reasoning.push(`Can scale down to ${recommended} instances`);
      reasoning.push('Low utilization detected');
    } else {
      reasoning.push('Current capacity is optimal');
    }

    return {
      currentInstances: latest.currentInstances,
      recommendedInstances: recommended,
      reasoning
    };
  }
}

// Example usage
if (require.main === module) {
  const scaler = new ReactiveScaler();

  // Simulate metrics
  const metrics: ScalingMetrics = {
    region: 'us-central1',
    timestamp: new Date(),
    cpuUtilization: 0.85, // High CPU
    memoryUtilization: 0.72,
    activeConnections: 45_000_000,
    requestRate: 150_000,
    errorRate: 0.005,
    p99Latency: 45,
    currentInstances: 50
  };

  scaler.processMetrics(metrics).then(action => {
    console.log('Scaling Action:', action);

    if (action.action !== 'none') {
      console.log(`\nAction: ${action.action.toUpperCase()}`);
      console.log(`Region: ${action.region}`);
      console.log(`Instances: ${action.fromInstances} -> ${action.toInstances}`);
      console.log(`Reason: ${action.reason}`);
      console.log(`Urgency: ${action.urgency}`);
    }
  });
}
