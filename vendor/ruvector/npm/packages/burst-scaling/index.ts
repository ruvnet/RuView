/**
 * Ruvector Burst Scaling System - Main Integration
 *
 * This file demonstrates how to integrate all burst scaling components
 * into a unified system that handles predictive and reactive scaling.
 */

import { BurstPredictor, EventCalendar, PredictedBurst } from './burst-predictor';
import { ReactiveScaler, ScalingMetrics, ScalingAction } from './reactive-scaler';
import { CapacityManager, CapacityPlan } from './capacity-manager';
import { exec } from 'child_process';
import { promisify } from 'util';
import * as cron from 'node-cron';

const execAsync = promisify(exec);

/**
 * Main Burst Scaling Orchestrator
 * Integrates predictive and reactive scaling with capacity management
 */
export class BurstScalingSystem {
  private predictor: BurstPredictor;
  private scaler: ReactiveScaler;
  private manager: CapacityManager;
  private isRunning: boolean = false;
  private metricsInterval: NodeJS.Timeout | null = null;
  private orchestrationInterval: NodeJS.Timeout | null = null;

  constructor(
    private readonly regions: string[] = ['us-central1', 'europe-west1', 'asia-east1'],
    private readonly metricsIntervalMs: number = 5000, // 5 seconds
    private readonly orchestrationIntervalMs: number = 60000 // 1 minute
  ) {
    this.predictor = new BurstPredictor(regions);
    this.scaler = new ReactiveScaler(regions);
    this.manager = new CapacityManager(regions);
  }

  /**
   * Start the burst scaling system
   */
  async start(): Promise<void> {
    if (this.isRunning) {
      console.log('⚠️  Burst scaling system is already running');
      return;
    }

    console.log('🚀 Starting Ruvector Burst Scaling System...');
    this.isRunning = true;

    // Load event calendar
    await this.loadEventCalendar();

    // Start metrics collection
    this.startMetricsCollection();

    // Start orchestration
    this.startOrchestration();

    // Schedule predictive scaling checks (every 15 minutes)
    cron.schedule('*/15 * * * *', async () => {
      await this.checkPredictiveScaling();
    });

    // Schedule daily reporting (at 9 AM)
    cron.schedule('0 9 * * *', async () => {
      await this.generateDailyReport();
    });

    console.log('✅ Burst scaling system started successfully');
    console.log(`   - Metrics collection: every ${this.metricsIntervalMs / 1000}s`);
    console.log(`   - Orchestration: every ${this.orchestrationIntervalMs / 1000}s`);
    console.log(`   - Predictive checks: every 15 minutes`);
    console.log(`   - Daily reports: 9:00 AM`);
  }

  /**
   * Stop the burst scaling system
   */
  stop(): void {
    console.log('🛑 Stopping Ruvector Burst Scaling System...');
    this.isRunning = false;

    if (this.metricsInterval) {
      clearInterval(this.metricsInterval);
      this.metricsInterval = null;
    }

    if (this.orchestrationInterval) {
      clearInterval(this.orchestrationInterval);
      this.orchestrationInterval = null;
    }

    console.log('✅ Burst scaling system stopped');
  }

  /**
   * Load event calendar from external source
   */
  private async loadEventCalendar(): Promise<void> {
    // In production, fetch from API or database
    const calendar: EventCalendar = {
      events: [
        {
          id: 'example-event',
          name: 'Example Streaming Event',
          type: 'release',
          startTime: new Date(Date.now() + 2 * 60 * 60 * 1000), // 2 hours from now
          region: this.regions,
          expectedViewers: 100_000_000
        }
      ]
    };

    await this.predictor.loadEventCalendar(calendar);
    console.log(`📅 Loaded ${calendar.events.length} events into calendar`);
  }

  /**
   * Start continuous metrics collection and reactive scaling
   */
  private startMetricsCollection(): void {
    this.metricsInterval = setInterval(async () => {
      try {
        // Collect metrics from all regions
        for (const region of this.regions) {
          const metrics = await this.collectRegionMetrics(region);

          // Process with reactive scaler
          const action = await this.scaler.processMetrics(metrics);

          // Execute scaling action if needed
          if (action.action !== 'none') {
            await this.executeScalingAction(action);
          }
        }
      } catch (error) {
        console.error('❌ Error in metrics collection:', error);
      }
    }, this.metricsIntervalMs);
  }

  /**
   * Start orchestration (capacity management, cost controls, degradation)
   */
  private startOrchestration(): void {
    this.orchestrationInterval = setInterval(async () => {
      try {
        // Run capacity manager orchestration
        const plan = await this.manager.orchestrate();

        // Log capacity plan
        this.logCapacityPlan(plan);

        // Check for budget warnings
        if (plan.budgetRemaining < 0) {
          console.warn('⚠️  BUDGET WARNING: Spending exceeds hourly budget');
        }

        // Check for degradation
        if (plan.degradationLevel !== 'none') {
          console.warn(`⚠️  DEGRADATION ACTIVE: ${plan.degradationLevel}`);
        }
      } catch (error) {
        console.error('❌ Error in orchestration:', error);
      }
    }, this.orchestrationIntervalMs);
  }

  /**
   * Check for predicted bursts and handle pre-warming
   */
  private async checkPredictiveScaling(): Promise<void> {
    console.log('🔮 Checking for predicted bursts...');

    try {
      // Get predictions for next 24 hours
      const predictions = await this.predictor.predictUpcomingBursts(24);

      if (predictions.length > 0) {
        console.log(`📊 Found ${predictions.length} predicted burst(s):`);

        for (const burst of predictions) {
          console.log(`   - ${burst.eventName}: ${burst.expectedMultiplier}x at ${burst.startTime.toISOString()}`);

          // Check if pre-warming should start
          const timeUntilEvent = burst.startTime.getTime() - Date.now();
          const preWarmMs = burst.preWarmTime * 1000;

          if (timeUntilEvent <= preWarmMs && timeUntilEvent > 0) {
            console.log(`🔥 Starting pre-warm for ${burst.eventName}`);
            await this.preWarmForBurst(burst);
          }
        }
      } else {
        console.log('   No bursts predicted in next 24 hours');
      }

      // Get pre-warming schedule
      const schedule = await this.predictor.getPreWarmingSchedule();
      if (schedule.length > 0) {
        console.log(`📋 Pre-warming schedule:`);
        schedule.forEach(item => {
          console.log(`   - ${item.eventName}: start ${item.preWarmStartTime.toISOString()} (${item.targetCapacity} instances)`);
        });
      }
    } catch (error) {
      console.error('❌ Error in predictive scaling check:', error);
    }
  }

  /**
   * Pre-warm capacity for predicted burst
   */
  private async preWarmForBurst(burst: PredictedBurst): Promise<void> {
    console.log(`🔥 PRE-WARMING for ${burst.eventName}:`);
    console.log(`   Expected multiplier: ${burst.expectedMultiplier}x`);
    console.log(`   Confidence: ${(burst.confidence * 100).toFixed(1)}%`);

    for (const regionPred of burst.regions) {
      console.log(`   ${regionPred.region}: scaling to ${regionPred.requiredInstances} instances`);

      // In production, call GCP API or Terraform to scale
      await this.scaleCloudRunService(
        regionPred.region,
        regionPred.requiredInstances
      );
    }

    // Notify via hooks
    await execAsync(
      `npx claude-flow@alpha hooks notify --message "PRE-WARM: ${burst.eventName} - scaling to ${burst.expectedMultiplier}x capacity"`
    );
  }

  /**
   * Collect metrics from a specific region
   * In production, fetch from Cloud Monitoring API
   */
  private async collectRegionMetrics(region: string): Promise<ScalingMetrics> {
    // Mock implementation - in production, query Cloud Monitoring
    // Example:
    // const metrics = await monitoringClient.getMetrics({
    //   project: 'ruvector-prod',
    //   metric: 'run.googleapis.com/container/cpu/utilizations',
    //   filter: `resource.labels.service_name="ruvector-${region}"`
    // });

    return {
      region,
      timestamp: new Date(),
      cpuUtilization: 0.5 + Math.random() * 0.3,
      memoryUtilization: 0.4 + Math.random() * 0.3,
      activeConnections: 10_000_000 + Math.random() * 5_000_000,
      requestRate: 50_000 + Math.random() * 20_000,
      errorRate: 0.001 + Math.random() * 0.004,
      p99Latency: 30 + Math.random() * 15,
      currentInstances: 50
    };
  }

  /**
   * Execute a scaling action
   */
  private async executeScalingAction(action: ScalingAction): Promise<void> {
    console.log(`⚡ SCALING ACTION: ${action.region}`);
    console.log(`   Action: ${action.action}`);
    console.log(`   Instances: ${action.fromInstances} -> ${action.toInstances}`);
    console.log(`   Reason: ${action.reason}`);
    console.log(`   Urgency: ${action.urgency}`);

    // In production, execute actual scaling via GCP API or Terraform
    await this.scaleCloudRunService(action.region, action.toInstances);

    // Notify via hooks
    await execAsync(
      `npx claude-flow@alpha hooks notify --message "SCALING: ${action.region} ${action.action} to ${action.toInstances} instances (${action.reason})"`
    );
  }

  /**
   * Scale Cloud Run service in a region
   */
  private async scaleCloudRunService(region: string, instances: number): Promise<void> {
    try {
      // In production, use GCP API:
      /*
      const command = `gcloud run services update ruvector-${region} \
        --region=${region} \
        --max-instances=${instances}`;
      await execAsync(command);
      */

      console.log(`   ✅ Scaled ruvector-${region} to ${instances} instances`);
    } catch (error) {
      console.error(`   ❌ Failed to scale ${region}:`, error);
    }
  }

  /**
   * Log capacity plan
   */
  private logCapacityPlan(plan: CapacityPlan): void {
    console.log('📊 CAPACITY PLAN:');
    console.log(`   Total Instances: ${plan.totalInstances}`);
    console.log(`   Total Cost: $${plan.totalCost.toFixed(2)}/hour`);
    console.log(`   Budget Remaining: $${plan.budgetRemaining.toFixed(2)}/hour`);
    console.log(`   Degradation: ${plan.degradationLevel}`);

    if (plan.regions.length > 0) {
      console.log('   Regions:');
      plan.regions.forEach(r => {
        console.log(`     - ${r.region}: ${r.instances} instances ($${r.cost.toFixed(2)}/hr, ${(r.utilization * 100).toFixed(1)}%)`);
      });
    }
  }

  /**
   * Generate daily report
   */
  private async generateDailyReport(): Promise<void> {
    console.log('\n📈 === DAILY BURST SCALING REPORT ===\n');

    // Get global status
    const status = this.manager.getGlobalStatus();

    console.log('CURRENT STATUS:');
    console.log(`  Total Instances: ${status.totalInstances}`);
    console.log(`  Hourly Cost: $${status.totalCost.toFixed(2)}`);
    console.log(`  Budget Usage: ${(status.budgetUsage * 100).toFixed(1)}%`);
    console.log(`  Degradation: ${status.degradationLevel}`);

    // Get metrics summary
    const summary = this.scaler.getMetricsSummary();
    console.log('\nREGIONAL METRICS:');
    summary.forEach((metrics, region) => {
      console.log(`  ${region}:`);
      console.log(`    CPU: ${(metrics.avgCpu * 100).toFixed(1)}%`);
      console.log(`    Memory: ${(metrics.avgMemory * 100).toFixed(1)}%`);
      console.log(`    P99 Latency: ${metrics.avgLatency.toFixed(1)}ms`);
      console.log(`    Connections: ${metrics.totalConnections.toLocaleString()}`);
      console.log(`    Instances: ${metrics.instances}`);
    });

    // Get prediction accuracy
    const accuracy = await this.predictor.getPredictionAccuracy();
    console.log('\nPREDICTION ACCURACY:');
    console.log(`  Accuracy: ${(accuracy.accuracy * 100).toFixed(1)}%`);
    console.log(`  MAPE: ${(accuracy.mape * 100).toFixed(1)}%`);
    console.log(`  Predictions: ${accuracy.predictions}`);

    // Get upcoming events
    const upcoming = await this.predictor.predictUpcomingBursts(168); // 7 days
    console.log('\nUPCOMING EVENTS (7 DAYS):');
    if (upcoming.length > 0) {
      upcoming.forEach(burst => {
        console.log(`  - ${burst.eventName}: ${burst.expectedMultiplier}x on ${burst.startTime.toLocaleDateString()}`);
      });
    } else {
      console.log('  No major events predicted');
    }

    console.log('\n=== END REPORT ===\n');

    // Notify via hooks
    await execAsync(
      `npx claude-flow@alpha hooks notify --message "DAILY REPORT: ${status.totalInstances} instances, $${status.totalCost.toFixed(2)}/hr, ${(status.budgetUsage * 100).toFixed(1)}% budget used"`
    );
  }

  /**
   * Get system health status
   */
  async getHealthStatus(): Promise<{
    healthy: boolean;
    issues: string[];
    metrics: {
      totalInstances: number;
      avgLatency: number;
      errorRate: number;
      budgetUsage: number;
    };
  }> {
    const issues: string[] = [];
    const status = this.manager.getGlobalStatus();
    const summary = this.scaler.getMetricsSummary();

    // Calculate average metrics
    let totalLatency = 0;
    let totalErrorRate = 0;
    let count = 0;

    summary.forEach(metrics => {
      totalLatency += metrics.avgLatency;
      count++;
    });

    const avgLatency = count > 0 ? totalLatency / count : 0;

    // Check for issues
    if (avgLatency > 50) {
      issues.push(`High latency: ${avgLatency.toFixed(1)}ms (threshold: 50ms)`);
    }

    if (status.budgetUsage > 1.0) {
      issues.push(`Budget exceeded: ${(status.budgetUsage * 100).toFixed(1)}%`);
    }

    if (status.degradationLevel !== 'none') {
      issues.push(`Degradation active: ${status.degradationLevel}`);
    }

    return {
      healthy: issues.length === 0,
      issues,
      metrics: {
        totalInstances: status.totalInstances,
        avgLatency,
        errorRate: totalErrorRate / (count || 1),
        budgetUsage: status.budgetUsage
      }
    };
  }
}

// CLI interface
if (require.main === module) {
  const system = new BurstScalingSystem();

  // Handle graceful shutdown
  process.on('SIGINT', () => {
    console.log('\n🛑 Received SIGINT, shutting down gracefully...');
    system.stop();
    process.exit(0);
  });

  process.on('SIGTERM', () => {
    console.log('\n🛑 Received SIGTERM, shutting down gracefully...');
    system.stop();
    process.exit(0);
  });

  // Start the system
  system.start().catch(error => {
    console.error('❌ Failed to start burst scaling system:', error);
    process.exit(1);
  });

  // Keep process alive
  process.stdin.resume();
}

export default BurstScalingSystem;
